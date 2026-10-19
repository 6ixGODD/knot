//! Stable, bounded error envelope shared by all kernel layers.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnotError {
    pub code: String,
    pub message: String,
    pub path: String,
    pub rule_ids: Vec<String>,
    pub details: Box<Value>,
}

impl KnotError {
    pub fn new(code: &str, path: &str) -> Self {
        Self {
            code: code.into(),
            message: code.into(),
            path: path.into(),
            rule_ids: Vec::new(),
            details: Box::new(json!({})),
        }
    }

    pub fn limit(name: &str, maximum: usize, observed: usize) -> Self {
        let mut error = Self::new("RESOURCE_LIMIT", "");
        error.details = Box::new(json!({"limit":name,"maximum":maximum,"observed":observed}));
        error
    }

    pub fn envelope(&self) -> Value {
        json!({"schemaVersion":"1.0","error":self})
    }

    pub fn status(&self) -> i32 {
        match self.code.as_str() {
            "NULL_ARGUMENT" => 1001,
            "BAD_JSON" | "INVALID_UTF8" => 1002,
            "BAD_SCHEMA" | "INVALID_MODEL" => 1003,
            "RULE_COMPILE" => 1004,
            "INVALID_STATE" | "MODEL_MISMATCH" => 1005,
            "REVISION_CONFLICT" => 1006,
            "INVALID_ANSWER" | "INVALID_OPTION" => 1007,
            "INACTIVE_QUESTION" => 1008,
            "RULE_EVAL" | "MISSING_VALUE" => 1009,
            "BASE_PRICE_CONFLICT"
            | "VISUAL_CONFLICT"
            | "UNPRICED"
            | "NO_BRANCH"
            | "NEGATIVE_TOTAL" => 1010,
            "RESOURCE_LIMIT" | "NUMERIC_OVERFLOW" | "REVISION_OVERFLOW" => 1011,
            _ => 1999,
        }
    }
}

impl std::fmt::Display for KnotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.code, self.path)
    }
}
impl std::error::Error for KnotError {}
