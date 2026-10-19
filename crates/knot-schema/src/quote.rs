//! Independent read-only completion quote wire contract.
use crate::{KnotError, wire::*};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuoteRequest {
    pub schema_version: Version,
    pub state: ConfigurationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_question_id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_value: Option<Answer>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    Range,
    Final,
    NoCompletion,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteReason {
    DomainLimit,
    SearchLimit,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PriceQuote {
    pub status: QuoteStatus,
    pub currency: String,
    pub min_minor: Option<Integer>,
    pub max_minor: Option<Integer>,
    pub reason: Option<QuoteReason>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuoteError {
    pub schema_version: Version,
    pub error: KnotError,
}
impl From<KnotError> for QuoteError {
    fn from(error: KnotError) -> Self {
        Self {
            schema_version: Version::V1,
            error,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuoteCandidate {
    pub option_id: Id,
    pub candidate_value: Answer,
    pub enabled: bool,
    pub rule_ids: Vec<String>,
    pub price: Option<PriceQuote>,
    pub error: Option<QuoteError>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotePreview {
    pub candidate_value: Answer,
    pub price: Option<PriceQuote>,
    pub error: Option<QuoteError>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuoteFocus {
    pub question: CurrentQuestion,
    pub answer: Option<Answer>,
    pub skipped: bool,
    pub candidates: Vec<QuoteCandidate>,
    pub preview: Option<QuotePreview>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuoteResult {
    pub schema_version: Version,
    pub model_digest: String,
    pub revision: Integer,
    pub price: PriceQuote,
    pub focus: Option<QuoteFocus>,
}
