//! Connection permits include TLS negotiation and the first HTTP/2 headers.
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
    time::Sleep,
};
use tokio_stream::Stream;
#[derive(Clone)]
pub struct ConnectionInfo(Arc<AtomicBool>);
impl ConnectionInfo {
    pub fn headers_received(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
pub struct Connection {
    stream: TcpStream,
    _permit: OwnedSemaphorePermit,
    info: ConnectionInfo,
    deadline: Pin<Box<Sleep>>,
}
impl tonic::transport::server::Connected for Connection {
    type ConnectInfo = ConnectionInfo;
    fn connect_info(&self) -> Self::ConnectInfo {
        self.info.clone()
    }
}
impl AsyncRead for Connection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.info.0.load(Ordering::Relaxed) && self.deadline.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "initial headers deadline",
            )));
        }
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}
impl AsyncWrite for Connection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
pub struct Incoming {
    listener: TcpListener,
    slots: Arc<Semaphore>,
}
impl Incoming {
    pub fn new(listener: TcpListener, max: usize) -> Self {
        Self {
            listener,
            slots: Arc::new(Semaphore::new(max)),
        }
    }
}
impl Stream for Incoming {
    type Item = io::Result<Connection>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        // Bound work per poll even under a flood of excess connections.
        for _ in 0..32 {
            match this.listener.poll_accept(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => return Poll::Ready(Some(Err(error))),
                Poll::Ready(Ok((stream, _))) => {
                    if let Ok(permit) = this.slots.clone().try_acquire_owned() {
                        return Poll::Ready(Some(Ok(Connection {
                            stream,
                            _permit: permit,
                            info: ConnectionInfo(Arc::new(AtomicBool::new(false))),
                            deadline: Box::pin(tokio::time::sleep(Duration::from_secs(5))),
                        })));
                    }
                }
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
