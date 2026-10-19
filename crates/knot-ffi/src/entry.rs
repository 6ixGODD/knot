#![allow(unsafe_code)]
use crate::{
    Model,
    abi::{ABI_VERSION, KnotResult},
    buffer, error,
};
use knot_schema::{
    KnotError,
    parse::{EVENT_BYTES, PRODUCT_BYTES, STATE_BYTES},
    protocol,
    wire::{ModelCreateResult, Version},
};

fn pointer(input: *const u8, len: usize, maximum: usize) -> Result<(), KnotError> {
    if input.is_null() {
        return Err(KnotError::new("NULL_ARGUMENT", ""));
    }
    if len > maximum || len > isize::MAX as usize {
        return Err(KnotError::limit("inputBytes", maximum, len));
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn knot_abi_version() -> u32 {
    ABI_VERSION
}

/// Compile a model and transfer ownership only after constructing the success response.
/// # Safety
/// Input covers len readable, immutable bytes. Out is a writable pointer slot, disjoint
/// from the input; its previous value is not owned or freed by this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn knot_model_create(
    input: *const u8,
    len: usize,
    out: *mut *mut Model,
) -> KnotResult {
    if !out.is_null() {
        // SAFETY: caller guarantees a writable, non-overlapping slot.
        unsafe {
            *out = std::ptr::null_mut();
        }
    }
    error::boundary(|| {
        if out.is_null() {
            return Err(KnotError::new("NULL_ARGUMENT", ""));
        }
        pointer(input, len, PRODUCT_BYTES)?;
        // SAFETY: validity belongs to caller, null and length were checked above.
        let bytes = unsafe { std::slice::from_raw_parts(input, len) };
        let compiled = knot_compiler::compile_product(bytes, &knot_rules_zen::ZenCompiler)?;
        let info = ModelCreateResult {
            schema_version: Version::V1,
            product_id: compiled.product().product_id.clone(),
            product_version: compiled.product().product_version.clone(),
            model_digest: compiled.digest().into(),
            diagnostics: compiled.diagnostics().to_vec(),
        };
        let model = Box::new(Model::new(compiled));
        let response = error::success(&info)?;
        // SAFETY: response and model are complete; the caller receives unique ownership.
        unsafe {
            *out = Box::into_raw(model);
        }
        Ok(response)
    })
}

/// # Safety
/// Model must be a live handle from this library; free must not overlap this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn knot_initial(model: *const Model) -> KnotResult {
    error::boundary(|| {
        if model.is_null() {
            return Err(KnotError::new("NULL_ARGUMENT", ""));
        }
        // SAFETY: caller keeps the immutable model live throughout this call.
        unsafe { &*model }.call(|model| error::success(&knot_core::initial(model)?))
    })
}

/// # Safety
/// Model is live. Inputs cover their lengths and remain immutable until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn knot_apply(
    model: *const Model,
    state: *const u8,
    state_len: usize,
    event: *const u8,
    event_len: usize,
) -> KnotResult {
    error::boundary(|| {
        if model.is_null() || state.is_null() || event.is_null() {
            return Err(KnotError::new("NULL_ARGUMENT", ""));
        }
        pointer(state, state_len, STATE_BYTES)?;
        pointer(event, event_len, EVENT_BYTES)?;
        // SAFETY: required pointers and lengths checked; allocation validity is caller's obligation.
        let (model, state, event) = unsafe {
            (
                &*model,
                std::slice::from_raw_parts(state, state_len),
                std::slice::from_raw_parts(event, event_len),
            )
        };
        model.call(|model| {
            error::success(&knot_core::apply(
                model,
                &protocol::state(state)?,
                &protocol::event(event)?,
            )?)
        })
    })
}

/// # Safety
/// Model is live. State covers state_len immutable readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn knot_evaluate(
    model: *const Model,
    state: *const u8,
    state_len: usize,
) -> KnotResult {
    error::boundary(|| {
        if model.is_null() {
            return Err(KnotError::new("NULL_ARGUMENT", ""));
        }
        pointer(state, state_len, STATE_BYTES)?;
        // SAFETY: checked required pointers and lengths; caller keeps allocations live.
        let (model, state) = unsafe { (&*model, std::slice::from_raw_parts(state, state_len)) };
        model.call(|model| error::success(&knot_core::evaluate(model, &protocol::state(state)?)?))
    })
}

/// # Safety
/// Result is NULL or points to the unique original live/cleared descriptor from this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn knot_result_free(result: *mut KnotResult) {
    if !result.is_null() {
        // SAFETY: caller owns this original descriptor; release clears its allocation.
        unsafe {
            buffer::release(&mut (*result).json);
            (*result).status = 0;
        }
    }
}

/// # Safety
/// Model is NULL or a unique live handle. All calls must have finished; never free twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn knot_model_free(model: *mut Model) {
    if !model.is_null() {
        // SAFETY: caller transfers unique ownership after all calls have finished.
        // Contain destructor panics without changing the process panic hook.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(model));
        }));
        if let Err(payload) = result {
            error::discard(payload);
        }
    }
}
