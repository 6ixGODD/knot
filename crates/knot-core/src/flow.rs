use crate::evaluation::Evaluation;
use crate::state;
use knot_compiler::CompiledModel;
use knot_rules::{ConditionId, Facts};
use knot_schema::{KnotError, wire::*};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Walk {
    pub flow: Flow,
    pub validation: Validation,
    pub removals: BTreeMap<usize, AnswerChange>,
}
pub(crate) fn condition(
    model: &CompiledModel,
    facts: &Facts,
    id: ConditionId,
    budget: &mut Evaluation,
) -> Result<bool, KnotError> {
    budget.charge(1)?;
    budget.condition(model, facts, id)
}
pub(crate) fn at(
    model: &CompiledModel,
    facts: &Facts,
    path: &str,
    budget: &mut Evaluation,
) -> Result<bool, KnotError> {
    let id = model
        .condition_id(path)
        .ok_or_else(|| KnotError::new("INVALID_MODEL", path))?;
    condition(model, facts, id, budget)
}
pub(crate) fn availability(
    model: &CompiledModel,
    index: usize,
    facts: &Facts,
    selected: Option<&Answer>,
    budget: &mut Evaluation,
) -> Result<Vec<AvailableOption>, KnotError> {
    state::options(&model.product().questions[index])
        .iter()
        .enumerate()
        .map(|(i, o)| {
            budget.charge((o.label.len() + o.id.as_str().len() + 1) as u64)?;
            Ok(AvailableOption {
                id: o.id.clone(),
                label: o.label.clone(),
                // Answer validation needs only selected options. A current question passes None
                // and obtains the complete availability list for its public snapshot.
                enabled: if o.enabled_when.is_some()
                    && selected.is_none_or(|answer| match answer {
                        Answer::Text(value) => value == o.id.as_str(),
                        Answer::Multi(values) => values.contains(&o.id),
                        _ => false,
                    }) {
                    at(
                        model,
                        facts,
                        &format!("/questions/{index}/options/{i}/enabledWhen"),
                        budget,
                    )?
                } else {
                    true
                },
            })
        })
        .collect()
}
pub(crate) fn disabled(
    model: &CompiledModel,
    index: usize,
    answer: &Answer,
    options: &[AvailableOption],
) -> Vec<String> {
    options
        .iter()
        .enumerate()
        .filter(|(_, o)| {
            !o.enabled
                && match answer {
                    Answer::Text(s) => s == o.id.as_str(),
                    Answer::Multi(v) => v.contains(&o.id),
                    _ => false,
                }
        })
        .filter_map(|(i, _)| {
            model
                .condition_id(&format!("/questions/{index}/options/{i}/enabledWhen"))
                .map(|c| model.sources()[c.0].rule_id().to_owned())
        })
        .collect()
}
pub(crate) fn current(q: &Question, options: Vec<AvailableOption>) -> CurrentQuestion {
    let required = state::required(q);
    match q {
        Question::Single { id, label, .. } => CurrentQuestion::Single {
            id: id.clone(),
            label: label.clone(),
            required,
            options,
        },
        Question::Multi { id, label, .. } => CurrentQuestion::Multi {
            id: id.clone(),
            label: label.clone(),
            required,
            options,
        },
        Question::Integer {
            id,
            label,
            min,
            max,
            step,
            ..
        } => CurrentQuestion::Integer {
            id: id.clone(),
            label: label.clone(),
            required,
            min: *min,
            max: *max,
            step: *step,
        },
        Question::Decimal {
            id,
            label,
            scale,
            min,
            max,
            step,
            ..
        } => CurrentQuestion::Decimal {
            id: id.clone(),
            label: label.clone(),
            required,
            scale: *scale,
            min: min.clone(),
            max: max.clone(),
            step: step.clone(),
        },
        Question::Color { id, label, .. } => CurrentQuestion::Color {
            id: id.clone(),
            label: label.clone(),
            required,
        },
    }
}
pub(crate) fn walk(
    model: &CompiledModel,
    state: &ConfigurationState,
    budget: &mut Evaluation,
) -> Result<Walk, KnotError> {
    let product = model.product();
    let mut cursor = Some(
        *model
            .question_index()
            .get(&product.flow.start)
            .ok_or_else(|| KnotError::new("INVALID_MODEL", "/flow/start"))?,
    );
    let mut answers = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut path = Vec::new();
    let mut removals = BTreeMap::new();
    let mut branches = Vec::new();
    let mut flow = Flow {
        status: FlowStatus::Complete,
        path: Vec::new(),
        next_question_id: None,
        current_question: None,
    };
    let mut validation = Validation {
        valid: true,
        issues: Vec::new(),
    };
    while let Some(index) = cursor {
        budget.charge((product.questions.len() + model.edges().len()) as u64)?;
        if !visited.insert(index) {
            return Err(KnotError::new("INVALID_MODEL", "/flow"));
        }
        let q = &product.questions[index];
        path.push(q.id().clone());
        let facts = state::facts(model, &answers, budget)?;
        let answer = state.answers.get(q.id());
        let mut options = availability(model, index, &facts, answer, budget)?;
        let disabled = answer
            .map(|a| disabled(model, index, a, &options))
            .unwrap_or_default();
        if !disabled.is_empty() {
            // A cleared answer makes this the current question: restore all visible choices.
            options = availability(model, index, &facts, None, budget)?;
            removals.insert(
                index,
                AnswerChange {
                    question_id: q.id().clone(),
                    kind: ChangeKind::Cleared,
                    reason: ChangeReason::BecameDisabled,
                    rule_ids: Some(disabled),
                },
            );
        }
        let answered = answer.is_some() && !removals.contains_key(&index);
        if !answered && !state.skipped.contains(q.id()) {
            let blocked = state::required(q)
                && matches!(q, Question::Single { .. } | Question::Multi { .. })
                && !options.iter().any(|o| o.enabled);
            flow.status = if blocked {
                FlowStatus::Blocked
            } else {
                FlowStatus::InProgress
            };
            flow.next_question_id = Some(q.id().clone());
            if blocked {
                validation.valid = false;
                validation.issues.push(ValidationIssue {
                    code: "INVALID_OPTION".into(),
                    question_id: Some(q.id().clone()),
                    path: format!("/questions/{index}/options"),
                    rule_ids: state::options(q)
                        .iter()
                        .enumerate()
                        .filter_map(|(i, _)| {
                            model.condition_id(&format!(
                                "/questions/{index}/options/{i}/enabledWhen"
                            ))
                        })
                        .map(|c| model.sources()[c.0].rule_id().into())
                        .collect(),
                });
            }
            flow.current_question = Some(current(q, options));
            break;
        }
        if let Some(a) = answer {
            answers.insert(q.id().clone(), a.clone());
        }
        let facts = state::facts(model, &answers, budget)?;
        let mut chosen = None;
        let mut branch_rules = Vec::new();
        for (i, edge) in model
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, e)| e.from == index)
        {
            if let Some(id) = edge.condition {
                branch_rules.push(id);
                if !condition(model, &facts, id, budget)? {
                    continue;
                }
            }
            chosen = Some((i, edge.to));
            break;
        }
        cursor = chosen
            .ok_or_else(|| KnotError::new("NO_BRANCH", "/flow/edges"))?
            .1;
        branches.push((index, cursor, branch_rules));
    }
    for (i, q) in product.questions.iter().enumerate() {
        if !visited.contains(&i)
            && (state.answers.contains_key(q.id()) || state.skipped.contains(q.id()))
        {
            // Only branch decisions that exclude this question are causal.
            // A retained branch followed by an incomplete question has no rule ID.
            let mut ids: Vec<_> = branches
                .iter()
                .filter(|(from, to, _)| {
                    model.successors()[*from].contains(&i)
                        && !to.is_some_and(|to| to == i || model.successors()[to].contains(&i))
                })
                .flat_map(|(_, _, ids)| ids.iter().copied())
                .collect();
            ids.sort_by_key(|id| id.0);
            ids.dedup();
            let rules = ids
                .iter()
                .map(|id| model.sources()[id.0].rule_id().into())
                .collect();
            removals.insert(
                i,
                AnswerChange {
                    question_id: q.id().clone(),
                    kind: ChangeKind::Cleared,
                    reason: ChangeReason::BranchUnreachable,
                    rule_ids: Some(rules),
                },
            );
        }
    }
    flow.path = path;
    Ok(Walk {
        flow,
        validation,
        removals,
    })
}
