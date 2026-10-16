//! Pure, atomic configuration transitions over immutable models.
//!
//! The frozen plan03 entry signatures are `initial(&CompiledModel)`,
//! `apply(&CompiledModel, &ConfigurationState, &ConfigurationEvent)` and
//! `evaluate(&CompiledModel, &ConfigurationState)`, each returning
//! `Result<TransitionResult, KnotError>`. Model lives in knot-compiler, wire values
//! and errors in knot-schema. Inputs are borrowed, never mutated; one CallBudget
//! covers all backend calls in a transition. Initial/evaluate emit empty Changes.
#![forbid(unsafe_code)]
mod changes;
mod flow;
mod output;
mod price;
mod reconcile;
pub mod revision;
mod state;
mod transition;
mod visual;

use knot_compiler::CompiledModel;
use knot_rules::CallBudget;
use knot_schema::{KnotError, wire::*};
pub use transition::apply;

/// Start at revision zero, without default answers or skips.
pub fn initial(model: &CompiledModel) -> Result<TransitionResult, KnotError> {
    evaluate(
        model,
        &ConfigurationState {
            schema_version: Version::V1,
            product_id: model.product().product_id.clone(),
            model_digest: model.digest().into(),
            revision: Integer::new(0)?,
            answers: Default::default(),
            skipped: Vec::new(),
        },
    )
}

/// Recompute a strictly normalized state, without repairing or advancing it.
/// Blocked states are accepted. Hosts must use CAS when saving concurrent results;
/// revision checks only concern the state supplied to this call.
pub fn evaluate(
    model: &CompiledModel,
    state: &ConfigurationState,
) -> Result<TransitionResult, KnotError> {
    let mut budget = CallBudget::new();
    let snapshot = snapshot(model, state, &mut budget)?;
    output::finish(
        TransitionResult {
            schema_version: Version::V1,
            state: state.clone(),
            snapshot,
            changes: changes::empty(),
            diagnostics: Vec::new(),
        },
        &mut budget,
    )
}

fn snapshot(
    model: &CompiledModel,
    state: &ConfigurationState,
    budget: &mut CallBudget,
) -> Result<Snapshot, KnotError> {
    state::validate(model, state, budget)?;
    let walked = flow::walk(model, state, budget)?;
    if let Some((_, removal)) = walked.removals.first_key_value() {
        let path = if state.answers.contains_key(&removal.question_id) {
            state::pointer(&removal.question_id)
        } else {
            "/skipped".into()
        };
        return Err(KnotError::new("INVALID_STATE", &path));
    }
    let facts = state::facts(model, &state.answers, budget)?;
    let price = price::derive(model, state, &facts, walked.flow.status, budget)?;
    let visual = visual::derive(model, state, &facts, budget)?;
    Ok(Snapshot {
        price,
        visual,
        flow: walked.flow,
        validation: walked.validation,
    })
}
