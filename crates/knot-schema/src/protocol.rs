//! Shared strict wire admission and bounded output for native front ends.
use crate::{KnotError, parse, validation::Validator, wire};
use std::{io, sync::OnceLock};

pub const OUTPUT_BYTES: usize = 4 * 1024 * 1024;
static STATE: OnceLock<Result<Validator, KnotError>> = OnceLock::new();
static EVENT: OnceLock<Result<Validator, KnotError>> = OnceLock::new();

fn schema<'a>(
    slot: &'a OnceLock<Result<Validator, KnotError>>,
    bytes: &[u8],
) -> Result<&'a Validator, KnotError> {
    slot.get_or_init(|| {
        let value = serde_json::from_slice(bytes).map_err(|_| KnotError::new("INTERNAL", ""))?;
        Validator::new(&value)
    })
    .as_ref()
    .map_err(Clone::clone)
}

pub fn state(bytes: &[u8]) -> Result<wire::ConfigurationState, KnotError> {
    wire::decode(
        bytes,
        parse::STATE_BYTES,
        schema(
            &STATE,
            include_bytes!("../../../contracts/schemas/state.schema.json"),
        )?,
    )
}

pub fn event(bytes: &[u8]) -> Result<wire::ConfigurationEvent, KnotError> {
    wire::decode(
        bytes,
        parse::EVENT_BYTES,
        schema(
            &EVENT,
            include_bytes!("../../../contracts/schemas/event.schema.json"),
        )?,
    )
}

/// Serialization never grows the output buffer beyond the wire ceiling.
pub fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, KnotError> {
    struct Sink {
        bytes: Vec<u8>,
        exceeded: bool,
    }
    impl io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > OUTPUT_BYTES - self.bytes.len() {
                self.exceeded = true;
                return Err(io::Error::other("output byte limit"));
            }
            if self.bytes.try_reserve(bytes.len()).is_err() {
                return Err(io::Error::other("allocation failure"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink {
        bytes: Vec::new(),
        exceeded: false,
    };
    if serde_json::to_writer(&mut sink, value).is_err() {
        return Err(if sink.exceeded {
            KnotError::limit("outputBytes", OUTPUT_BYTES, OUTPUT_BYTES + 1)
        } else {
            KnotError::new("INTERNAL", "")
        });
    }
    Ok(sink.bytes)
}
