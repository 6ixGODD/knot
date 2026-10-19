use crate::evaluation::Evaluation;
use crate::flow;
use knot_compiler::CompiledModel;
use knot_schema::{KnotError, wire::*};
use std::collections::BTreeMap;

pub(crate) fn run(
    model: &CompiledModel,
    candidate: &mut ConfigurationState,
    budget: &mut Evaluation,
) -> Result<Vec<AnswerChange>, KnotError> {
    let maximum = candidate.answers.len() + candidate.skipped.len() + 1;
    let mut changes = BTreeMap::new();
    for _ in 0..maximum {
        budget.charge((model.product().questions.len() * 8) as u64)?;
        let walked = flow::walk(model, candidate, budget)?;
        if walked.removals.is_empty() {
            return Ok(changes.into_values().collect());
        }
        let before = candidate.answers.len() + candidate.skipped.len();
        for (index, change) in walked.removals {
            candidate.answers.remove(&change.question_id);
            candidate.skipped.retain(|id| *id != change.question_id);
            changes.entry(index).or_insert(change);
        }
        if candidate.answers.len() + candidate.skipped.len() >= before {
            return Err(KnotError::new("INTERNAL", ""));
        }
    }
    Err(KnotError::new("INTERNAL", ""))
}
