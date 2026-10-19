use crate::evaluation::Evaluation;
use crate::{changes, flow, output, reconcile, revision, snapshot, state};
use knot_compiler::CompiledModel;
use knot_schema::{KnotError, wire::*};

/// Apply one event atomically. Every backend call, including old-state validation
/// and all reconciliation rounds, shares one budget. Inputs are never modified.
pub fn apply(
    model: &CompiledModel,
    old: &ConfigurationState,
    event: &ConfigurationEvent,
) -> Result<TransitionResult, KnotError> {
    let mut budget = Evaluation::new();
    apply_internal(model, old, event, &mut budget, true)
}

pub(crate) fn apply_internal(
    model: &CompiledModel,
    old: &ConfigurationState,
    event: &ConfigurationEvent,
    budget: &mut Evaluation,
    advance: bool,
) -> Result<TransitionResult, KnotError> {
    let old_snapshot = snapshot(model, old, budget)?;
    apply_validated(model, old, &old_snapshot, event, budget, advance)
}
// Only called with the snapshot produced for this exact state by this request.
pub(crate) fn apply_validated(
    model: &CompiledModel,
    old: &ConfigurationState,
    old_snapshot: &Snapshot,
    event: &ConfigurationEvent,
    budget: &mut Evaluation,
    advance: bool,
) -> Result<TransitionResult, KnotError> {
    let (id, expected, kind) = match event {
        ConfigurationEvent::SetAnswer {
            question_id,
            expected_revision,
            ..
        } => (question_id, expected_revision, ChangeKind::Set),
        ConfigurationEvent::ClearAnswer {
            question_id,
            expected_revision,
            ..
        } => (question_id, expected_revision, ChangeKind::Cleared),
        ConfigurationEvent::SkipAnswer {
            question_id,
            expected_revision,
            ..
        } => (question_id, expected_revision, ChangeKind::Skipped),
    };
    if advance {
        revision::check_revision(old.revision.get(), expected.get())?;
    }
    let index = *model
        .question_index()
        .get(id)
        .ok_or_else(|| KnotError::new("INACTIVE_QUESTION", "/questionId"))?;
    let q = &model.product().questions[index];
    if !old_snapshot.flow.path.contains(id)
        || (kind == ChangeKind::Cleared && !old.answers.contains_key(id))
        || (kind == ChangeKind::Skipped
            && (state::required(q) || old_snapshot.flow.next_question_id.as_ref() != Some(id)))
    {
        return Err(KnotError::new("INACTIVE_QUESTION", "/questionId"));
    }
    let mut candidate = old.clone();
    let mut set = None;
    if let ConfigurationEvent::SetAnswer { value, .. } = event {
        let value = state::normalize(q, value)?;
        let preceding = old_snapshot
            .flow
            .path
            .iter()
            .take_while(|other| *other != id)
            .filter_map(|other| old.answers.get(other).map(|a| (other.clone(), a.clone())))
            .collect();
        let facts = state::facts(model, &preceding, budget)?;
        let available = flow::availability(model, index, &facts, Some(&value), budget)?;
        let disabled = flow::disabled(model, index, &value, &available);
        if !disabled.is_empty() {
            let mut e = KnotError::new("INVALID_OPTION", &state::pointer(id));
            e.rule_ids = disabled;
            return Err(e);
        }
        candidate.answers.insert(id.clone(), value.clone());
        candidate.skipped.retain(|other| other != id);
        set = Some(value);
    } else if kind == ChangeKind::Cleared {
        candidate.answers.remove(id);
    } else {
        candidate.skipped.push(id.clone());
        candidate
            .skipped
            .sort_by_key(|id| model.question_index().get(id).copied());
    }
    let user_changed = candidate.answers != old.answers || candidate.skipped != old.skipped;
    let automatic = reconcile::run(model, &mut candidate, budget)?;
    if let Some(value) = set
        && candidate.answers.get(id) != Some(&value)
    {
        return Err(KnotError::new("INVALID_ANSWER", &state::pointer(id)));
    }
    let snapshot = snapshot(model, &candidate, budget)?;
    let changed = old.answers != candidate.answers || old.skipped != candidate.skipped;
    if advance {
        candidate.revision = Integer::new(revision::next_revision(old.revision.get(), changed)?)?;
    }
    let mut answer_changes = Vec::new();
    if user_changed {
        answer_changes.push(AnswerChange {
            question_id: id.clone(),
            kind,
            reason: ChangeReason::User,
            rule_ids: None,
        });
    }
    answer_changes.extend(automatic);
    let changes = changes::derive(old_snapshot, &snapshot, answer_changes);
    let result = TransitionResult {
        schema_version: Version::V1,
        state: candidate,
        snapshot,
        changes,
        diagnostics: Vec::new(),
    };
    if advance {
        output::finish(result, budget)
    } else {
        Ok(result)
    }
}
