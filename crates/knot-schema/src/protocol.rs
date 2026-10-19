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

static QUOTE: OnceLock<Result<Validator, KnotError>> = OnceLock::new();
/// Admit an independent Quote request, preserving nested State limits.
pub fn quote(bytes: &[u8]) -> Result<crate::quote::QuoteRequest, KnotError> {
    let value = crate::parse_json(bytes, 768 * 1024)?;
    schema(
        &QUOTE,
        include_bytes!("../../../contracts/schemas/quote-request.schema.json"),
    )?
    .validate(&value)?;
    let state_value = value
        .get("state")
        .ok_or_else(|| KnotError::new("BAD_SCHEMA", "/state"))?;
    let state_bytes = encode(state_value)?;
    let raw_state_length = nested_state_length(bytes)?;
    if raw_state_length > parse::STATE_BYTES {
        return Err(KnotError::limit(
            "inputBytes",
            parse::STATE_BYTES,
            raw_state_length,
        ));
    }
    state(&state_bytes)?;
    serde_json::from_value(value).map_err(|_| KnotError::new("BAD_SCHEMA", ""))
}

// Called only after strict JSON admission. Count the original nested object's
// bytes, including its whitespace, rather than permitting whitespace compaction
// to bypass the independent State ceiling.
fn nested_state_length(bytes: &[u8]) -> Result<usize, KnotError> {
    let mut depth = 0usize;
    let mut i = 0usize;
    let mut state_start = None;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        break;
                    }
                    i += 1;
                }
                if depth == 1 {
                    let key: Result<String, _> = serde_json::from_slice(&bytes[start..=i]);
                    let mut next = i + 1;
                    while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                        next += 1;
                    }
                    if matches!(key.as_deref(), Ok("state")) && bytes.get(next) == Some(&b':') {
                        next += 1;
                        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                            next += 1;
                        }
                        state_start = Some(next);
                    }
                }
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 1
                    && let Some(start) = state_start
                {
                    return Ok(i + 1 - start);
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err(KnotError::new("BAD_SCHEMA", "/state"))
}
