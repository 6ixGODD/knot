//! Only valid allocations are used; dangling pointers are never a supported error case.
#![allow(unsafe_code, clippy::unwrap_used)] // Raw C boundary and test assertions.
use knot_ffi::abi::KnotResult;
use knot_ffi::*;
use std::{fs, path::PathBuf, ptr};

fn bytes(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/golden")
            .join(name),
    )
    .unwrap()
}
fn take(mut result: KnotResult) -> (i32, serde_json::Value) {
    let status = result.status;
    // SAFETY: result is the unique live return value, copied before freeing its original descriptor.
    unsafe {
        let value = if result.json.data.is_null() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(std::slice::from_raw_parts(
                result.json.data,
                result.json.len,
            ))
            .unwrap()
        };
        knot_result_free(&mut result);
        knot_result_free(&mut result);
        assert!(result.json.data.is_null());
        assert_eq!(result.json.len, 0);
        (status, value)
    }
}
#[test]
fn null_length_encoding_and_ownership() {
    let product = bytes("T02/model.json");
    let mut model = ptr::null_mut();
    // SAFETY: all non-NULL buffers/slots are owned and valid; overlong lengths are rejected before borrowing.
    unsafe {
        assert_eq!(knot_abi_version(), 1);
        knot_result_free(ptr::null_mut());
        knot_model_free(ptr::null_mut());
        assert_eq!(
            take(knot_model_create(
                product.as_ptr(),
                product.len(),
                ptr::null_mut()
            ))
            .0,
            1001
        );
        assert_eq!(take(knot_model_create(ptr::null(), 0, &mut model)).0, 1001);
        assert!(model.is_null());
        assert_eq!(
            take(knot_model_create(product.as_ptr(), usize::MAX, &mut model)).0,
            1011
        );
        assert_eq!(
            take(knot_model_create(product.as_ptr(), 0, &mut model)).0,
            1002
        );
        assert_eq!(
            take(knot_model_create([255].as_ptr(), 1, &mut model)).0,
            1002
        );
        assert_eq!(take(knot_initial(ptr::null())).0, 1001);
        assert_eq!(
            take(knot_evaluate(ptr::null(), product.as_ptr(), 0)).0,
            1001
        );
        assert_eq!(
            take(knot_apply(
                ptr::null(),
                product.as_ptr(),
                0,
                product.as_ptr(),
                0
            ))
            .0,
            1001
        );
        assert_eq!(
            take(knot_model_create(
                product.as_ptr(),
                product.len(),
                &mut model
            ))
            .0,
            0
        );
        let state = bytes("T04/initial-state.json");
        let event = bytes("T04/event.json");
        let mut failed_slot = model;
        assert_eq!(
            take(knot_model_create(product.as_ptr(), 0, &mut failed_slot)).0,
            1002
        );
        assert!(failed_slot.is_null());
        assert_eq!(take(knot_evaluate(model, [255].as_ptr(), 1)).0, 1002);
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                state.len(),
                [255].as_ptr(),
                1
            ))
            .0,
            1002
        );
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                usize::MAX,
                ptr::null(),
                0
            ))
            .0,
            1001
        );
        assert_eq!(take(knot_evaluate(model, ptr::null(), 0)).0, 1001);
        assert_eq!(
            take(knot_evaluate(model, state.as_ptr(), usize::MAX)).0,
            1011
        );
        assert_eq!(take(knot_evaluate(model, state.as_ptr(), 0)).0, 1002);
        assert_eq!(
            take(knot_apply(
                model,
                ptr::null(),
                0,
                event.as_ptr(),
                event.len()
            ))
            .0,
            1001
        );
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                state.len(),
                ptr::null(),
                0
            ))
            .0,
            1001
        );
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                usize::MAX,
                event.as_ptr(),
                event.len()
            ))
            .0,
            1011
        );
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                state.len(),
                event.as_ptr(),
                usize::MAX
            ))
            .0,
            1011
        );
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                state.len(),
                event.as_ptr(),
                0
            ))
            .0,
            1002
        );
        assert_eq!(
            take(knot_apply(
                model,
                state.as_ptr(),
                state.len(),
                event.as_ptr(),
                event.len()
            ))
            .0,
            0
        );
        for _ in 0..1000 {
            assert_eq!(take(knot_initial(model)).0, 0);
        }
        knot_model_free(model);
        for _ in 0..20 {
            model = ptr::null_mut();
            assert_eq!(
                take(knot_model_create(
                    product.as_ptr(),
                    product.len(),
                    &mut model
                ))
                .0,
                0
            );
            knot_model_free(model);
        }
    }
}
