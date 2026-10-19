//! Validate bounded serialized responses against the canonical wire schemas.
use knot_schema::{KnotError, protocol, validation::Validator};
use std::sync::OnceLock;

static RESULT: OnceLock<Result<Validator, KnotError>> = OnceLock::new();
static ERROR: OnceLock<Result<Validator, KnotError>> = OnceLock::new();

fn encode(
    value: &impl serde::Serialize,
    slot: &OnceLock<Result<Validator, KnotError>>,
    schema: &[u8],
) -> Result<Vec<u8>, KnotError> {
    let internal = || KnotError::new("INTERNAL", "");
    let bytes = protocol::encode(value).map_err(|_| internal())?;
    let validator = slot
        .get_or_init(|| {
            let schema = serde_json::from_slice(schema).map_err(|_| internal())?;
            Validator::new(&schema)
        })
        .as_ref()
        .map_err(|_| internal())?;
    let instance = serde_json::from_slice(&bytes).map_err(|_| internal())?;
    validator.validate(&instance).map_err(|_| internal())?;
    Ok(bytes)
}

pub fn result(value: &impl serde::Serialize) -> Result<Vec<u8>, KnotError> {
    encode(
        value,
        &RESULT,
        include_bytes!("../../../contracts/schemas/result.schema.json"),
    )
}

pub fn error(value: &impl serde::Serialize) -> Result<Vec<u8>, KnotError> {
    encode(
        value,
        &ERROR,
        include_bytes!("../../../contracts/schemas/error.schema.json"),
    )
}

static QUOTE: OnceLock<Result<Validator, KnotError>> = OnceLock::new();
pub fn quote(value: &knot_schema::quote::QuoteResult) -> Result<Vec<u8>, KnotError> {
    encode(
        value,
        &QUOTE,
        include_bytes!("../../../contracts/schemas/quote-result.schema.json"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn invalid_output_is_an_internal_fault() {
        assert!(
            matches!(result(&json!({"schemaVersion":"1.0", "state":{}, "snapshot":{}, "changes":{}, "diagnostics":[]})), Err(e) if e.code == "INTERNAL")
        );
        assert!(
            matches!(error(&KnotError::new("UNKNOWN", "").envelope()), Err(e) if e.code == "INTERNAL")
        );
        assert!(error(&KnotError::new("BAD_JSON", "").envelope()).is_ok());
    }
}
