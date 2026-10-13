//! One mutable call budget belongs to the caller for the entire transition.
use crate::{CallBudget, CompileBudget, Condition, ConditionId, Facts, FieldKind, SourceMap};
use knot_schema::KnotError;
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanEffects(Vec<(ConditionId, bool)>);
impl BooleanEffects {
    /// Reject missing, extra or reordered backend outputs instead of treating them as false.
    pub fn new(
        requested: &[ConditionId],
        values: Vec<(ConditionId, bool)>,
    ) -> Result<Self, KnotError> {
        if requested.len() != values.len()
            || requested.iter().zip(&values).any(|(a, (b, _))| a != b)
        {
            return Err(KnotError::new("RULE_EVAL", ""));
        }
        Ok(Self(values))
    }
    pub fn values(&self) -> &[(ConditionId, bool)] {
        &self.0
    }
}
pub trait RuleProgram: Send + Sync {
    fn condition_count(&self) -> usize;
    fn evaluate(
        &self,
        facts: &Facts,
        requested: &[ConditionId],
        budget: &mut CallBudget,
    ) -> Result<BooleanEffects, KnotError>;
}
pub trait RuleCompiler: Send + Sync {
    fn compile(
        &self,
        fields: &[FieldKind],
        conditions: &[Condition],
        sources: &SourceMap,
        budget: &mut CompileBudget,
    ) -> Result<Arc<dyn RuleProgram>, KnotError>;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effect_order_is_not_silently_repaired() {
        let ids = [ConditionId(1), ConditionId(0)];
        assert!(
            BooleanEffects::new(&ids, vec![(ConditionId(0), true), (ConditionId(1), false)])
                .is_err()
        );
        assert!(BooleanEffects::new(&ids, vec![(ConditionId(1), true)]).is_err());
        assert!(
            BooleanEffects::new(&ids, vec![(ConditionId(1), true), (ConditionId(0), false)])
                .is_ok()
        );
    }
}
