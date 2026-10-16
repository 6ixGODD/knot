//! Global admission before protobuf decoding, retained through response streaming.
use crate::lifecycle::Lifecycle;
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{Instant, Sleep},
};
use tonic::{Status, body::Body, codegen::http};
use tower::{Layer, Service};
#[derive(Clone)]
pub struct Admission {
    business: Arc<Semaphore>,
    health: Arc<Semaphore>,
    life: Lifecycle,
    timeout: Duration,
}
impl Admission {
    pub fn available_health(&self) -> usize {
        self.health.available_permits()
    }
    pub fn available_business(&self) -> usize {
        self.business.available_permits()
    }
    pub fn new(life: Lifecycle, timeout: Duration) -> Self {
        Self {
            business: Arc::new(Semaphore::new(64)),
            health: Arc::new(Semaphore::new(8)),
            life,
            timeout,
        }
    }
}
impl<S> Layer<S> for Admission {
    type Service = Admitted<S>;
    fn layer(&self, inner: S) -> Self::Service {
        Admitted {
            inner,
            limits: self.clone(),
        }
    }
}
#[derive(Clone)]
pub struct Admitted<S> {
    inner: S,
    limits: Admission,
}
impl<S, B> Service<http::Request<B>> for Admitted<S>
where
    S: Service<http::Request<B>, Response = http::Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    B: Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        if let Some(info) = request
            .extensions()
            .get::<crate::transport::ConnectionInfo>()
        {
            info.headers_received();
        }
        if let Some(info) = request
            .extensions()
            .get::<tonic::transport::server::TlsConnectInfo<crate::transport::ConnectionInfo>>()
        {
            info.get_ref().headers_received();
        }
        let health = request.uri().path().starts_with("/grpc.health.v1.Health/");
        let reject = if health {
            Ok(())
        } else {
            self.limits.life.admit()
        };
        let slots = if health {
            &self.limits.health
        } else {
            &self.limits.business
        };
        let permit = reject.and_then(|()| {
            slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| Status::resource_exhausted("RPC capacity"))
        });
        let permit = match permit {
            Ok(p) => p,
            Err(status) => {
                self.limits
                    .life
                    .telemetry
                    .overload
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Box::pin(async move { Ok(status.into_http()) });
            }
        };
        let duration = request
            .headers()
            .get("grpc-timeout")
            .and_then(|v| v.to_str().ok())
            .and_then(timeout)
            .map_or(self.limits.timeout, |v| v.min(self.limits.timeout));
        let deadline = Instant::now() + duration;
        let future = self.inner.call(request);
        let life = self.limits.life.clone();
        Box::pin(async move {
            match tokio::time::timeout_at(deadline, future).await {
                Ok(Ok(response)) => {
                    let (mut parts, body) = response.into_parts();
                    normalize_size_status(&mut parts.headers);
                    Ok(http::Response::from_parts(
                        parts,
                        Body::new(BoundedBody {
                            inner: body,
                            permit: Some(permit),
                            life: if health { None } else { Some(life.clone()) },
                            deadline: Box::pin(tokio::time::sleep_until(deadline)),
                        }),
                    ))
                }
                Ok(Err(error)) => Err(error),
                Err(_) => {
                    life.telemetry
                        .deadline
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(Status::deadline_exceeded("request deadline").into_http())
                }
            }
        })
    }
}
fn timeout(value: &str) -> Option<Duration> {
    if !(2..=9).contains(&value.len()) {
        return None;
    }
    let (number, unit) = value.split_at(value.len() - 1);
    if !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = number.parse::<u64>().ok()?;
    match unit {
        "H" => n.checked_mul(3600).map(Duration::from_secs),
        "M" => n.checked_mul(60).map(Duration::from_secs),
        "S" => Some(Duration::from_secs(n)),
        "m" => Some(Duration::from_millis(n)),
        "u" => Some(Duration::from_micros(n)),
        "n" => Some(Duration::from_nanos(n)),
        _ => None,
    }
}
struct BoundedBody {
    inner: Body,
    permit: Option<OwnedSemaphorePermit>,
    deadline: Pin<Box<Sleep>>,
    life: Option<Lifecycle>,
}
impl HttpBody for BoundedBody {
    type Data = Bytes;
    type Error = Status;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Status>>> {
        if self.permit.is_none() {
            return Poll::Ready(None);
        }
        let status = if self.life.as_ref().is_some_and(Lifecycle::faulted) {
            Some(Status::internal("service fault"))
        } else if self.deadline.as_mut().poll(cx).is_ready() {
            if let Some(life) = &self.life {
                life.telemetry
                    .deadline
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            Some(Status::deadline_exceeded("response deadline"))
        } else {
            None
        };
        if let Some(status) = status {
            self.permit.take();
            let mut trailers = http::HeaderMap::new();
            return Poll::Ready(Some(match status.add_header(&mut trailers) {
                Ok(()) => Ok(Frame::trailers(trailers)),
                Err(error) => Err(error),
            }));
        }
        let result = Pin::new(&mut self.inner).poll_frame(cx);
        let result = match result {
            Poll::Ready(Some(Ok(mut frame))) => {
                if let Some(trailers) = frame.trailers_mut() {
                    normalize_size_status(trailers);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(status))) if status.code() == tonic::Code::OutOfRange => {
                Poll::Ready(Some(Err(Status::resource_exhausted("message size"))))
            }
            other => other,
        };
        if matches!(result, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            self.permit.take();
        }
        result
    }
    fn is_end_stream(&self) -> bool {
        self.permit.is_none() || self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

// tonic uses OUT_OF_RANGE for codec size ceilings; this service freezes RESOURCE_EXHAUSTED.
fn normalize_size_status(headers: &mut http::HeaderMap) {
    if headers
        .get("grpc-status")
        .is_some_and(|value| value == "11")
    {
        headers.insert("grpc-status", http::HeaderValue::from_static("8"));
    }
}
