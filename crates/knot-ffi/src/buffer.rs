//! A boxed slice has exactly the length needed for deallocation; Vec capacity never crosses ABI.
#![allow(unsafe_code)]
use crate::abi::KnotBuffer;

pub fn allocate(bytes: Vec<u8>) -> KnotBuffer {
    if bytes.is_empty() {
        return KnotBuffer {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    let bytes = bytes.into_boxed_slice();
    let len = bytes.len();
    KnotBuffer {
        data: Box::into_raw(bytes).cast::<u8>(),
        len,
    }
}

/// Release one allocation and clear this descriptor, making repeated release of it harmless.
///
/// # Safety
/// The descriptor must be returned by `allocate`, have its original pointer/length, and
/// still exclusively own the allocation. Copies must not be freed or accessed afterwards.
pub unsafe fn release(buffer: &mut KnotBuffer) {
    let data = std::mem::replace(&mut buffer.data, std::ptr::null_mut());
    let len = std::mem::replace(&mut buffer.len, 0);
    if !data.is_null() {
        // SAFETY: caller guarantees this exact pointer and slice length came from allocate.
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_repeated_release() {
        for len in [0, 1, 17, 4096] {
            let mut bytes = Vec::with_capacity(len + 127);
            bytes.resize(len, 42);
            let mut buffer = allocate(bytes);
            if len > 0 {
                // SAFETY: buffer is exclusively owned and still live, with original length.
                assert_eq!(
                    unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) },
                    vec![42; len]
                );
            }
            // SAFETY: original live descriptor; second call sees the cleared empty descriptor.
            unsafe {
                release(&mut buffer);
                release(&mut buffer);
            }
            assert!(buffer.data.is_null());
            assert_eq!(buffer.len, 0);
        }
    }
}
