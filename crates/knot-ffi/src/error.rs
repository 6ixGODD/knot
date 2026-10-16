use crate::{
    abi::{KnotBuffer, KnotResult},
    buffer,
};
use knot_schema::{KnotError, protocol};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub fn empty_internal() -> KnotResult {
    KnotResult {
        status: 1999,
        json: KnotBuffer {
            data: std::ptr::null_mut(),
            len: 0,
        },
    }
}

pub fn discard(payload: Box<dyn std::any::Any + Send>) {
    if let Err(nested) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(nested);
    }
}

pub fn failure(error: &KnotError) -> KnotResult {
    // The error formatter and allocator have their own boundary: never unwind across C.
    match catch_unwind(AssertUnwindSafe(|| {
        protocol::encode(&error.envelope()).map(|bytes| KnotResult {
            status: error.status(),
            json: buffer::allocate(bytes),
        })
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => empty_internal(),
        Err(payload) => {
            discard(payload);
            empty_internal()
        }
    }
}

pub fn success(value: &impl serde::Serialize) -> Result<KnotResult, KnotError> {
    Ok(KnotResult {
        status: 0,
        json: buffer::allocate(protocol::encode(value)?),
    })
}

pub fn boundary(operation: impl FnOnce() -> Result<KnotResult, KnotError>) -> KnotResult {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => failure(&error),
        Err(payload) => {
            discard(payload);
            empty_internal()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unexpected_panic_uses_no_json_fallback() {
        let result = boundary(|| panic!("injected operation failure"));
        assert_eq!(result.status, 1999);
        assert!(result.json.data.is_null());
        assert_eq!(result.json.len, 0);
    }
}
