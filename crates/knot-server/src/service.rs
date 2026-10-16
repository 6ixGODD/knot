//! Convert network envelopes without introducing domain semantics.
use crate::{
    catalog::Catalog,
    lifecycle::Lifecycle,
    protocol::{self, configuration_service_server::ConfigurationService},
};
use knot_schema::{KnotError, protocol as json};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use tonic::{Request, Response, Status};
#[derive(Clone)]
pub struct Service {
    pub catalog: Arc<Catalog>,
    pub lifecycle: Lifecycle,
}
fn digest(value: Option<String>) -> Result<String, Status> {
    let value = value.ok_or_else(|| Status::invalid_argument("missing model_digest"))?;
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return Err(Status::invalid_argument("invalid model_digest"));
    }
    Ok(value)
}
impl Service {
    async fn call(
        &self,
        digest_value: Option<String>,
        state: Option<Vec<u8>>,
        event: Option<Vec<u8>>,
        operation: u8,
    ) -> Result<Response<protocol::ConfigurationResponse>, Status> {
        self.lifecycle.admit()?;
        let digest = digest(digest_value)?;
        if operation > 0 && state.is_none() || operation == 1 && event.is_none() {
            return Err(Status::invalid_argument("missing JSON field"));
        }
        let model = self
            .catalog
            .get(&digest)
            .cloned()
            .ok_or_else(|| Status::not_found("model not deployed"))?;
        let life = self.lifecycle.clone();
        let started = std::time::Instant::now();
        let task = self.lifecycle.spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<u8>, KnotError> {
                let result = match operation {
                    0 => knot_core::initial(&model)?,
                    1 => {
                        let state = json::state(
                            state
                                .as_deref()
                                .ok_or_else(|| KnotError::new("INTERNAL", ""))?,
                        )?;
                        let event = json::event(
                            event
                                .as_deref()
                                .ok_or_else(|| KnotError::new("INTERNAL", ""))?,
                        )?;
                        knot_core::apply(&model, &state, &event)?
                    }
                    _ => knot_core::evaluate(
                        &model,
                        &json::state(
                            state
                                .as_deref()
                                .ok_or_else(|| KnotError::new("INTERNAL", ""))?,
                        )?,
                    )?,
                };
                json::encode(&result)
            }));
            let outcome = match result {
                Ok(Ok(bytes)) => Ok(protocol::configuration_response::Outcome::ResultJson(bytes)),
                Ok(Err(error)) if error.status() != 1999 => match json::encode(&error.envelope()) {
                    Ok(bytes) => Ok(protocol::configuration_response::Outcome::ErrorJson(bytes)),
                    Err(_) => {
                        life.fault();
                        Err(Status::internal("error encoding failed"))
                    }
                },
                Ok(Err(_)) => {
                    life.fault();
                    Err(Status::internal("internal failure"))
                }
                Err(payload) => {
                    life.fault();
                    // A panic payload can itself panic in Drop. Do not unwind past the fault boundary.
                    if let Err(second) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                        std::mem::forget(second);
                    }
                    Err(Status::internal("internal failure"))
                }
            };
            if life.faulted() {
                return Err(Status::internal("service fault"));
            }
            let classification = match &outcome {
                Ok(protocol::configuration_response::Outcome::ResultJson(_)) => "success",
                Ok(_) => {
                    life.telemetry
                        .domain
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    "domainError"
                }
                Err(_) => "fault",
            };
            life.telemetry.request(
                match operation {
                    0 => "Initial",
                    1 => "Apply",
                    _ => "Evaluate",
                },
                classification,
                &digest,
                started.elapsed().as_millis(),
            );
            outcome
        })?;
        let outcome = match task.await {
            Ok(result) => result?,
            Err(_) => {
                self.lifecycle.fault();
                return Err(Status::internal("worker failure"));
            }
        };
        if self.lifecycle.faulted() {
            return Err(Status::internal("service fault"));
        }
        Ok(Response::new(protocol::ConfigurationResponse {
            outcome: Some(outcome),
        }))
    }
}
#[tonic::async_trait]
impl ConfigurationService for Service {
    async fn initial(
        &self,
        request: Request<protocol::InitialRequest>,
    ) -> Result<Response<protocol::ConfigurationResponse>, Status> {
        let r = request.into_inner();
        self.call(r.model_digest, None, None, 0).await
    }
    async fn apply(
        &self,
        request: Request<protocol::ApplyRequest>,
    ) -> Result<Response<protocol::ConfigurationResponse>, Status> {
        let r = request.into_inner();
        self.call(r.model_digest, r.state_json, r.event_json, 1)
            .await
    }
    async fn evaluate(
        &self,
        request: Request<protocol::EvaluateRequest>,
    ) -> Result<Response<protocol::ConfigurationResponse>, Status> {
        let r = request.into_inner();
        self.call(r.model_digest, r.state_json, None, 2).await
    }
}
