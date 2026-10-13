//! Bounded input primitives; this library does not expose a product command yet.
#![forbid(unsafe_code)]
use std::io::{self, Read};

/// Reads at most maximum + 1 bytes, rejecting oversized input without reading the rest.
pub fn read_bounded(reader: impl Read, maximum: usize) -> io::Result<Vec<u8>> {
    let limit = u64::try_from(maximum)
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid byte limit"))?;
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input byte limit exceeded",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_stream_and_io_failure() {
        assert_eq!(read_bounded(&b"abc"[..], 3).ok(), Some(b"abc".to_vec()));
        let mut bytes = &b"abcdef"[..];
        assert!(read_bounded(&mut bytes, 3).is_err());
        assert_eq!(bytes, b"ef");
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("test read failure"))
            }
        }
        assert!(read_bounded(Broken, 10).is_err());
        assert!(read_bounded(io::empty(), usize::MAX).is_err());
    }
}
