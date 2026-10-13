//! Layouts mirror contracts/abi/knot.h. Status is exactly int32_t.
pub type KnotStatus = i32;
pub const ABI_VERSION: u32 = 1;
#[repr(C)]
#[derive(Debug)]
pub struct KnotBuffer {
    pub data: *mut u8,
    pub len: usize,
}
#[repr(C)]
#[derive(Debug)]
pub struct KnotResult {
    pub status: KnotStatus,
    pub json: KnotBuffer,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_layout() {
        assert_eq!(size_of::<KnotStatus>(), 4);
        assert_eq!(std::mem::offset_of!(KnotBuffer, len), size_of::<usize>());
        assert_eq!(size_of::<KnotBuffer>(), 2 * size_of::<usize>());
        assert_eq!(std::mem::offset_of!(KnotResult, json), align_of::<usize>());
        assert_eq!(size_of::<KnotResult>(), 3 * size_of::<usize>());
    }
}
