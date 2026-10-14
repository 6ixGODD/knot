use crate::{abi::KnotResult, buffer, error};
use knot_compiler::CompiledModel;
use knot_schema::KnotError;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
};

/// Opaque, immutable model storage. Only the C lifecycle functions own it.
pub struct Model {
    pub(crate) compiled: CompiledModel,
    faulted: AtomicBool,
}

impl Model {
    pub(crate) fn new(compiled: CompiledModel) -> Self {
        Self {
            compiled,
            faulted: AtomicBool::new(false),
        }
    }
    pub(crate) fn call(
        &self,
        operation: impl FnOnce(&CompiledModel) -> Result<KnotResult, KnotError>,
    ) -> Result<KnotResult, KnotError> {
        if self.faulted.load(Ordering::Acquire) {
            return Err(KnotError::new("INTERNAL", ""));
        }
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                if std::thread::panicking() {
                    self.0.store(true, Ordering::Release);
                }
            }
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = Guard(&self.faulted);
            operation(&self.compiled)
        }));
        match result {
            Err(payload) => {
                self.faulted.store(true, Ordering::Release);
                error::discard(payload);
                Err(KnotError::new("INTERNAL", ""))
            }
            Ok(result) => {
                if result.as_ref().is_err_and(|error| error.code == "INTERNAL") {
                    self.faulted.store(true, Ordering::Release);
                }
                if self.faulted.load(Ordering::Acquire) {
                    if let Ok(mut result) = result {
                        // SAFETY: this call exclusively owns the unreturned allocation.
                        #[allow(unsafe_code)]
                        unsafe {
                            buffer::release(&mut result.json);
                        }
                    }
                    Err(KnotError::new("INTERNAL", ""))
                } else {
                    result
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Assertions and deliberate panic injection are restricted to unit tests.
    #[allow(clippy::unwrap_used)]
    fn model() -> Model {
        Model::new(
            knot_compiler::compile_product(
                include_bytes!("../../../contracts/examples/speaker.json"),
                &knot_rules_zen::ZenCompiler,
            )
            .unwrap(),
        )
    }
    #[test]
    fn panic_poison_and_inflight_recheck() {
        let model = model();
        let entered = std::sync::Barrier::new(2);
        let released = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                model
                    .call(|_| {
                        entered.wait();
                        released.wait();
                        error::success(&serde_json::json!({"mustNotEscape":true}))
                    })
                    .map(|mut result| {
                        // SAFETY: an unexpectedly successful result is still uniquely owned here.
                        // Release locally rather than transferring raw allocation pointers between threads.
                        #[allow(unsafe_code)]
                        unsafe {
                            buffer::release(&mut result.json);
                        }
                    })
            });
            entered.wait();
            assert!(model.call(|_| panic!("injected boundary panic")).is_err());
            released.wait();
            assert!(matches!(worker.join(), Ok(Err(_))));
        });
        assert!(
            model
                .call(|_| panic!("must not run after poisoning"))
                .is_err()
        );
    }
    #[test]
    fn panicking_payload_is_contained() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("injected payload destructor");
            }
        }
        let model = model();
        assert!(model.call(|_| std::panic::panic_any(Payload)).is_err());
        assert!(model.call(|_| panic!("must not run")).is_err());
    }
}
