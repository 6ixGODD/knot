//! Missing is None, never a null answer or an empty/default scalar.
use crate::{FieldKind, Scalar};
use knot_schema::{KnotError, wire::Id};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactValue {
    Scalar(Scalar),
    Multi(Vec<Id>),
}
#[derive(Debug, Clone)]
pub struct Facts {
    values: Vec<Option<FactValue>>,
}
impl Facts {
    pub fn new(values: Vec<Option<FactValue>>, fields: &[FieldKind]) -> Result<Self, KnotError> {
        if values.len() != fields.len() || values.len() > 256 {
            return Err(KnotError::new("RULE_EVAL", ""));
        }
        for (value, kind) in values.iter().zip(fields) {
            match value {
                None => {}
                Some(FactValue::Scalar(s)) if s.kind() == *kind => {
                    if let Scalar::Color(color) = s
                        && (color.len() != 7
                            || !color.starts_with('#')
                            || !color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
                    {
                        return Err(KnotError::new("RULE_EVAL", ""));
                    }
                }
                Some(FactValue::Multi(v)) if *kind == FieldKind::Multi && v.len() <= 256 => {
                    let set: std::collections::BTreeSet<_> = v.iter().collect();
                    if set.len() != v.len() {
                        return Err(KnotError::new("RULE_EVAL", ""));
                    }
                }
                _ => return Err(KnotError::new("RULE_EVAL", "")),
            }
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &[Option<FactValue>] {
        &self.values
    }
}
