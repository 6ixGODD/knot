#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| knot_fuzz::operations(data));
