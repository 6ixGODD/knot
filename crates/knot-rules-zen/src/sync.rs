//! Poll once on the calling thread. Pending is an explicit backend failure.
use knot_schema::KnotError;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};
pub fn poll_once<F: Future>(future: F) -> Result<F::Output, KnotError> {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => Ok(result),
        Poll::Pending => Err(KnotError::new("RULE_EVAL", "")),
    }
}
