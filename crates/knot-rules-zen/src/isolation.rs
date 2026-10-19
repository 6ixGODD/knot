//! Shared-handle poison boundary. It cannot recover abort, allocation failure or process damage.
use knot_schema::KnotError;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
};
struct FaultGuard<'a>(&'a AtomicBool);
impl Drop for FaultGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.store(true, Ordering::Release);
        }
    }
}
pub(crate) fn isolate<T>(
    faulted: &AtomicBool,
    operation: impl FnOnce() -> Result<T, KnotError>,
) -> Result<T, KnotError> {
    if faulted.load(Ordering::Acquire) {
        return Err(KnotError::new("INTERNAL", ""));
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = FaultGuard(faulted);
        operation()
    }));
    match result {
        Err(payload) => {
            faulted.store(true, Ordering::Release);
            // Catch a panic during payload destruction outside the original unwind.
            if let Err(nested) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(nested);
            }
            Err(KnotError::new("INTERNAL", ""))
        }
        Ok(result) => {
            if faulted.load(Ordering::Acquire) {
                Err(KnotError::new("INTERNAL", ""))
            } else {
                result
            }
        }
    }
}
