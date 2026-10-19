//! Draft 2020-12 validation with external resource retrieval explicitly disabled.
use crate::KnotError;
use serde_json::Value;

struct Offline;
impl jsonschema::Retrieve for Offline {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(format!("external schema resource prohibited: {uri}").into())
    }
}

/// Compile a schema without filesystem or network resource resolution.
pub struct Validator(jsonschema::Validator);
impl Validator {
    pub fn new(schema: &Value) -> Result<Self, KnotError> {
        jsonschema::draft202012::options()
            .with_retriever(Offline)
            .with_format("knot-id", |s| {
                !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control)
            })
            .with_format("knot-decimal", |s| crate::Decimal::parse(s).is_ok())
            .should_validate_formats(true)
            .build(schema)
            .map(Self)
            .map_err(|_| KnotError::new("BAD_SCHEMA", ""))
    }

    /// Select the first error by JSON Pointer, independently of library traversal order.
    pub fn validate(&self, value: &Value) -> Result<(), KnotError> {
        match self
            .0
            .iter_errors(value)
            .map(|error| error.instance_path().to_string())
            .min()
        {
            Some(path) => Err(KnotError::new("BAD_SCHEMA", &path)),
            None => Ok(()),
        }
    }
}
