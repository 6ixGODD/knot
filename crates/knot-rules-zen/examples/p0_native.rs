//! P0-only FFI probe. Never package this library as the KNOT SDK.
#![allow(unsafe_code)]
use knot_rules::{
    CallBudget, CompileBudget, Condition, ConditionId, FactValue, Facts, Field, FieldKind,
    RuleCompiler, RuleProgram, Scalar,
};
use knot_rules_zen::ZenCompiler;
use knot_schema::{KnotError, exact_integer, parse_json, wire::Integer};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

pub struct Probe {
    program: Arc<dyn RuleProgram>,
}
#[repr(C)]
pub struct Buffer {
    pub data: *mut u8,
    pub len: usize,
}
#[repr(C)]
pub struct Response {
    pub status: i32,
    pub json: Buffer,
}
fn response(result: Result<bool, KnotError>) -> Response {
    let (status, value) = match result {
        Ok(value) => (
            0,
            serde_json::json!({"value":i32::from(value),"text":"zen"}),
        ),
        Err(error) => (error.status(), error.envelope()),
    };
    let bytes = value.to_string().into_bytes().into_boxed_slice();
    let len = bytes.len();
    Response {
        status,
        json: Buffer {
            data: Box::into_raw(bytes).cast::<u8>(),
            len,
        },
    }
}
fn boundary<F: FnOnce() -> Result<bool, KnotError>>(f: F) -> Response {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => response(result),
        Err(payload) => {
            if let Err(nested) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(nested);
            }
            response(Err(KnotError::new("INTERNAL", "")))
        }
    }
}
/// # Safety
/// `out` points to one writable model pointer, initially owned by this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p0_probe_create(out: *mut *mut Probe) -> Response {
    boundary(|| {
        if out.is_null() {
            return Err(KnotError::new("NULL_ARGUMENT", ""));
        }
        // SAFETY: out validity is required by the caller; clear it before fallible work.
        unsafe {
            *out = std::ptr::null_mut();
        }
        let scalar = Scalar::Integer(Integer::new(7)?);
        let program = ZenCompiler.compile(
            &[FieldKind::Integer],
            &[Condition::Eq(
                Field {
                    index: 0,
                    kind: FieldKind::Integer,
                },
                scalar,
            )],
            &vec![knot_rules::RuleSource::new("/probe".into(), None)?],
            &mut CompileBudget::default(),
        )?;
        // SAFETY: caller owns the returned unique Box until p0_probe_destroy.
        unsafe {
            *out = Box::into_raw(Box::new(Probe { program }));
        }
        Ok(true)
    })
}
/// # Safety
/// Model must remain live throughout this call. Input points to len readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p0_probe_evaluate(
    model: *const Probe,
    input: *const u8,
    len: usize,
) -> Response {
    boundary(|| {
        if model.is_null() || input.is_null() {
            return Err(KnotError::new("NULL_ARGUMENT", ""));
        }
        if len > 65536 {
            return Err(KnotError::limit("eventBytes", 65536, len));
        }
        // SAFETY: pointers/lifetimes are caller obligations; length bounded before slice creation.
        let (model, input) = unsafe { (&*model, std::slice::from_raw_parts(input, len)) };
        let value = parse_json(input, 65536)?;
        let object = value
            .as_object()
            .ok_or_else(|| KnotError::new("BAD_SCHEMA", ""))?;
        if object.len() > 1 || object.keys().any(|k| k != "value") {
            return Err(KnotError::new("BAD_SCHEMA", ""));
        }
        let fact = match object.get("value") {
            None => None,
            Some(serde_json::Value::Number(n)) => Some(FactValue::Scalar(Scalar::Integer(
                Integer::new(exact_integer(&n.to_string())?)?,
            ))),
            _ => return Err(KnotError::new("BAD_SCHEMA", "/value")),
        };
        let facts = Facts::new(vec![fact], &[FieldKind::Integer])?;
        let effects = model
            .program
            .evaluate(&facts, &[ConditionId(0)], &mut CallBudget::new())?;
        effects
            .values()
            .first()
            .map(|(_, v)| *v)
            .ok_or_else(|| KnotError::new("RULE_EVAL", ""))
    })
}
/// # Safety
/// Result points to its original exclusively owned descriptor; copies must not be released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p0_probe_result_free(result: *mut Response) {
    if result.is_null() {
        return;
    }
    // SAFETY: caller supplies a writable original result with its unchanged allocation length.
    unsafe {
        let result = &mut *result;
        let data = std::mem::replace(&mut result.json.data, std::ptr::null_mut());
        let len = std::mem::replace(&mut result.json.len, 0);
        result.status = 0;
        if !data.is_null() {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len)));
        }
    }
}
/// # Safety
/// Pointer is from create, freed exactly once, with no active concurrent calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p0_probe_destroy(model: *mut Probe) {
    if !model.is_null() {
        // SAFETY: ownership and quiescence are caller obligations documented above.
        unsafe {
            drop(Box::from_raw(model));
        }
    }
}
