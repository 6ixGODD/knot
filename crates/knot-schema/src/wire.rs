//! Versioned wire structures. Decode through `decode` so Schema and lexical limits apply.
//! Optional definition fields stay absent until the compiler explicitly normalizes defaults.
use crate::{KnotError, exact_integer, parse_json, validation::Validator};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Version {
    #[serde(rename = "1.0")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Integer(i64);
impl Integer {
    pub fn new(value: i64) -> Result<Self, KnotError> {
        exact_integer(&value.to_string()).map(Self)
    }
    pub fn get(self) -> i64 {
        self.0
    }
}
impl<'de> Deserialize<'de> for Integer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(d)?;
        exact_integer(&number.to_string())
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Id(String);
impl Id {
    pub fn new(value: String) -> Result<Self, KnotError> {
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(KnotError::new("BAD_SCHEMA", ""));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Answer {
    Text(String),
    Integer(Integer),
    Multi(Vec<Id>),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Text(String),
    Integer(Integer),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Condition {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Not { not: Box<Condition> },
    Leaf(Leaf),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Leaf {
    Eq { field: Id, value: Scalar },
    Ne { field: Id, value: Scalar },
    Gt { field: Id, value: Scalar },
    Gte { field: Id, value: Scalar },
    Lt { field: Id, value: Scalar },
    Lte { field: Id, value: Scalar },
    Between { field: Id, value: [Scalar; 2] },
    In { field: Id, value: Vec<Scalar> },
    Contains { field: Id, value: Id },
    Exists { field: Id },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptionDefinition {
    pub id: Id,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_when: Option<Condition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Question {
    Single {
        id: Id,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<bool>,
        options: Vec<OptionDefinition>,
    },
    Multi {
        id: Id,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<bool>,
        options: Vec<OptionDefinition>,
    },
    Integer {
        id: Id,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Integer>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Integer>,
        #[serde(skip_serializing_if = "Option::is_none")]
        step: Option<Integer>,
    },
    Decimal {
        id: Id,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<bool>,
        scale: Integer,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        step: Option<String>,
    },
    Color {
        id: Id,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<bool>,
    },
}
impl Question {
    pub fn id(&self) -> &Id {
        match self {
            Self::Single { id, .. }
            | Self::Multi { id, .. }
            | Self::Integer { id, .. }
            | Self::Decimal { id, .. }
            | Self::Color { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub from: Id,
    pub to: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowDefinition {
    pub start: Id,
    pub edges: Vec<Edge>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceCategory {
    Base,
    Adjustment,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum PriceComponent {
    Fixed {
        id: Id,
        category: PriceCategory,
        #[serde(rename = "amountMinor")]
        amount_minor: Integer,
        #[serde(skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Quantity {
        id: Id,
        category: PriceCategory,
        #[serde(rename = "unitMinor")]
        unit_minor: Integer,
        #[serde(rename = "quantityFrom")]
        quantity_from: Id,
        #[serde(skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pricing {
    pub currency: String,
    pub components: Vec<PriceComponent>,
}

/// Property names and values are validated together by the visual Schema.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum VisualValue {
    Bool(bool),
    Text(String),
    Number(f64),
    Vector([f64; 3]),
}
impl<'de> Deserialize<'de> for VisualValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Preserve serde_json's arbitrary-precision number representation until
        // this visual-only boundary explicitly converts to finite binary64.
        // Serde's untagged f64 visitor does not recognize every such number.
        let value = serde_json::Value::deserialize(d)?;
        let invalid = || serde::de::Error::custom("invalid visual value");
        let number = |value: &serde_json::Value| {
            value.as_f64().filter(|n| n.is_finite()).ok_or_else(invalid)
        };
        match value {
            serde_json::Value::Bool(v) => Ok(Self::Bool(v)),
            serde_json::Value::String(v) => Ok(Self::Text(v)),
            serde_json::Value::Number(_) => number(&value).map(Self::Number),
            serde_json::Value::Array(v) if v.len() == 3 => Ok(Self::Vector([
                number(&v[0])?,
                number(&v[1])?,
                number(&v[2])?,
            ])),
            _ => Err(invalid()),
        }
    }
}
pub type Properties = BTreeMap<String, VisualValue>;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDefinition {
    pub id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Integer>,
    pub defaults: Properties,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum VisualCase {
    Constant {
        when: Condition,
        value: VisualValue,
    },
    FromAnswer {
        when: Condition,
        #[serde(rename = "valueFrom")]
        value_from: Id,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualSlot {
    pub id: Id,
    pub target: Id,
    pub property: String,
    pub cases: Vec<VisualCase>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualDefinition {
    pub targets: Vec<TargetDefinition>,
    pub slots: Vec<VisualSlot>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Product {
    pub schema_version: Version,
    pub product_id: Id,
    pub product_version: Id,
    pub questions: Vec<Question>,
    pub flow: FlowDefinition,
    pub pricing: Pricing,
    pub visual: VisualDefinition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationState {
    pub schema_version: Version,
    pub product_id: Id,
    pub model_digest: String,
    pub revision: Integer,
    pub answers: BTreeMap<Id, Answer>,
    pub skipped: Vec<Id>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfigurationEvent {
    SetAnswer {
        #[serde(rename = "schemaVersion")]
        schema_version: Version,
        #[serde(rename = "questionId")]
        question_id: Id,
        #[serde(rename = "expectedRevision")]
        expected_revision: Integer,
        value: Answer,
    },
    ClearAnswer {
        #[serde(rename = "schemaVersion")]
        schema_version: Version,
        #[serde(rename = "questionId")]
        question_id: Id,
        #[serde(rename = "expectedRevision")]
        expected_revision: Integer,
    },
    SkipAnswer {
        #[serde(rename = "schemaVersion")]
        schema_version: Version,
        #[serde(rename = "questionId")]
        question_id: Id,
        #[serde(rename = "expectedRevision")]
        expected_revision: Integer,
    },
}

/// Parse limits and schema checks always precede typed deserialization.
pub fn decode<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    maximum: usize,
    schema: &Validator,
) -> Result<T, KnotError> {
    let value = parse_json(bytes, maximum)?;
    schema.validate(&value)?;
    serde_json::from_value(value).map_err(|_| KnotError::new("BAD_SCHEMA", ""))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub code: String,
    pub path: String,
    pub rule_ids: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationIssue {
    pub code: String,
    pub question_id: Option<Id>,
    pub path: String,
    pub rule_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PricedComponent {
    pub id: Id,
    pub amount_minor: Integer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceStatus {
    Unknown,
    Estimated,
    Final,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Price {
    pub status: PriceStatus,
    pub currency: String,
    pub total_minor: Option<Integer>,
    pub components: Vec<PricedComponent>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvailableOption {
    pub id: Id,
    pub label: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CurrentQuestion {
    Single {
        id: Id,
        label: String,
        required: bool,
        options: Vec<AvailableOption>,
    },
    Multi {
        id: Id,
        label: String,
        required: bool,
        options: Vec<AvailableOption>,
    },
    Integer {
        id: Id,
        label: String,
        required: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Integer>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Integer>,
        #[serde(skip_serializing_if = "Option::is_none")]
        step: Option<Integer>,
    },
    Decimal {
        id: Id,
        label: String,
        required: bool,
        scale: Integer,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        step: Option<String>,
    },
    Color {
        id: Id,
        label: String,
        required: bool,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowStatus {
    InProgress,
    Complete,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Flow {
    pub status: FlowStatus,
    pub path: Vec<Id>,
    pub next_question_id: Option<Id>,
    pub current_question: Option<CurrentQuestion>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualTarget {
    pub id: Id,
    pub order: Integer,
    pub properties: Properties,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Visual {
    pub targets: Vec<VisualTarget>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub price: Price,
    pub flow: Flow,
    pub visual: Visual,
    pub validation: Validation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Set,
    Cleared,
    Skipped,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeReason {
    User,
    BecameDisabled,
    BranchUnreachable,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerChange {
    pub question_id: Id,
    pub kind: ChangeKind,
    pub reason: ChangeReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_ids: Option<Vec<String>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Changes {
    pub answer_changes: Vec<AnswerChange>,
    pub price_changed: bool,
    pub visual_changed_targets: Vec<Id>,
    pub flow_changed: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransitionResult {
    pub schema_version: Version,
    pub state: ConfigurationState,
    pub snapshot: Snapshot,
    pub changes: Changes,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCreateResult {
    pub schema_version: Version,
    pub product_id: Id,
    pub product_version: Id,
    pub model_digest: String,
    pub diagnostics: Vec<Diagnostic>,
}
