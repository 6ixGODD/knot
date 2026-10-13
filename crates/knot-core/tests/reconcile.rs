mod support;
use knot_schema::{KnotError, SAFE_INTEGER, wire::*};
use support::*;

#[test]
fn malformed_backend_effects_are_rejected_without_state_mutation() {
    use knot_rules::{BooleanEffects, CallBudget, ConditionId, Facts, RuleProgram};
    struct Malformed;
    impl RuleProgram for Malformed {
        fn condition_count(&self) -> usize {
            1
        }
        fn evaluate(
            &self,
            _: &Facts,
            _: &[ConditionId],
            _: &mut CallBudget,
        ) -> Result<BooleanEffects, KnotError> {
            BooleanEffects::new(&[], vec![])
        }
    }
    let mut p = product(vec![single("a", true)]);
    p.flow.edges[0].when = Some(condition());
    let good = model(p);
    let m = knot_compiler::CompiledModel::new(knot_compiler::ModelParts {
        product: good.product().clone(),
        question_index: good.question_index().clone(),
        edges: good.edges().to_vec(),
        successors: good.successors().to_vec(),
        target_order: good.target_order().to_vec(),
        sources: good.sources().clone(),
        program: std::sync::Arc::new(Malformed),
        digest: good.digest().into(),
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let state = initial(&m).state;
    fails(&m, &state, &set(&state, "a", text("yes")), "RULE_EVAL");
}

#[test]
fn speaker_prices_reconciliation_defaults_and_branch_provenance() {
    let m = support::speaker::speaker();
    let s = support::speaker::complete(&m);
    assert_eq!(s.snapshot.price.total_minor, Some(n(282800)));
    assert_eq!(s.snapshot.price.status, PriceStatus::Final);
    let changed = answer(&m, &s.state, "material", text("walnut"));
    assert_eq!(changed.snapshot.price.total_minor, Some(n(159900)));
    assert_eq!(changed.snapshot.price.status, PriceStatus::Estimated);
    assert_eq!(changed.state.answers.len(), 3);
    assert_eq!(changed.state.answers[&id("panel_color")], text("#315A79"));
    assert_eq!(changed.snapshot.flow.next_question_id, Some(id("power")));
    assert_eq!(
        changed.changes.answer_changes[1].reason,
        ChangeReason::BecameDisabled
    );
    assert_eq!(
        changed.changes.answer_changes[1].rule_ids,
        Some(vec!["rule:/questions/3/options/2/enabledWhen".into()])
    );
    assert_eq!(changed.changes.answer_changes[2].rule_ids, Some(vec![]));
    assert_eq!(
        changed.snapshot.visual.targets[2].properties["visible"],
        VisualValue::Bool(false)
    );
    assert_eq!(
        changed.changes.visual_changed_targets,
        vec![id("cabinet"), id("led")]
    );
    let branch = answer(&m, &s.state, "speaker_type", text("passive"));
    assert_eq!(branch.snapshot.flow.next_question_id, Some(id("cable")));
    for c in branch.changes.answer_changes.iter().skip(1) {
        assert_eq!(c.reason, ChangeReason::BranchUnreachable);
        assert_eq!(c.rule_ids, Some(vec!["rule:/flow/edges/2/when".into()]));
    }
    let again = answer(&m, &branch.state, "speaker_type", text("active"));
    assert!(!again.state.answers.contains_key(&id("power")));
    assert!(!again.state.answers.contains_key(&id("extras")));
}

#[test]
fn merge_point_observes_missing_on_unselected_branch() {
    let mut p = product(vec![
        single("a", true),
        color("b", true),
        color("c", true),
        Question::Single {
            id: id("d"),
            label: "d".into(),
            required: Some(true),
            options: vec![option("yes", true)],
        },
    ]);
    p.flow.edges = vec![
        ("a", "b", true),
        ("a", "c", false),
        ("b", "d", false),
        ("c", "d", false),
        ("d", "END", false),
    ]
    .into_iter()
    .map(|(from, to, c)| Edge {
        from: id(from),
        to: id(to),
        when: c.then(condition),
    })
    .collect();
    let m = controlled(
        p,
        |id, f| {
            Ok(match id {
                0 => f.values()[1].is_some(),
                1 => single_fact(f, 0, "yes"),
                _ => panic!("condition"),
            })
        },
        1,
    );
    let s = answer(&m, &initial(&m).state, "a", text("yes"));
    let s = answer(&m, &s.state, "b", text("#000000"));
    let s = answer(&m, &s.state, "d", text("yes"));
    let changed = answer(&m, &s.state, "a", text("no"));
    assert_eq!(changed.state.answers.len(), 1);
    let changed = answer(&m, &changed.state, "c", text("#000000"));
    assert_eq!(changed.snapshot.flow.status, FlowStatus::Blocked);
    assert!(!changed.state.answers.contains_key(&id("b")));
    assert!(!changed.state.answers.contains_key(&id("d")));
}

#[test]
fn clear_middle_removes_suffix_and_skip_in_declaration_order() {
    let m = model(product(vec![
        color("a", true),
        color("b", true),
        color("c", false),
        color("d", true),
    ]));
    let s = answer(&m, &initial(&m).state, "a", text("#000000"));
    let s = answer(&m, &s.state, "b", text("#000000"));
    let s = knot_core::apply(&m, &s.state, &skip(&s.state, "c")).unwrap_or_else(|e| panic!("{e}"));
    let s = answer(&m, &s.state, "d", text("#000000"));
    let cleared =
        knot_core::apply(&m, &s.state, &clear(&s.state, "b")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(cleared.state.answers.len(), 1);
    assert!(cleared.state.skipped.is_empty());
    assert_eq!(
        cleared
            .changes
            .answer_changes
            .iter()
            .map(|c| c.question_id.clone())
            .collect::<Vec<_>>(),
        vec![id("b"), id("c"), id("d")]
    );
    assert_eq!(cleared.snapshot.flow.next_question_id, Some(id("b")));
}

#[test]
fn multi_loses_whole_answer_and_never_reads_suffix_facts() {
    let p = product(vec![
        single("a", true),
        Question::Multi {
            id: id("b"),
            label: "b".into(),
            required: Some(false),
            options: vec![option("safe", false), option("conditional", true)],
        },
        color("c", true),
    ]);
    let m = controlled(
        p,
        |_, f| {
            assert!(f.values()[1].is_none());
            assert!(f.values()[2].is_none());
            Ok(single_fact(f, 0, "yes"))
        },
        1,
    );
    let s = answer(&m, &initial(&m).state, "a", text("yes"));
    let s = answer(
        &m,
        &s.state,
        "b",
        Answer::Multi(vec![id("safe"), id("conditional")]),
    );
    let s = answer(&m, &s.state, "c", text("#000000"));
    let changed = answer(&m, &s.state, "a", text("no"));
    assert_eq!(changed.state.answers.len(), 1);
    assert_eq!(
        changed.changes.answer_changes[1].reason,
        ChangeReason::BecameDisabled
    );
    assert_eq!(
        changed.changes.answer_changes[1].rule_ids,
        Some(vec!["rule:/questions/1/options/1/enabledWhen".into()])
    );
    assert_eq!(
        changed.changes.answer_changes[2].reason,
        ChangeReason::BranchUnreachable
    );
}

#[test]
fn revision_max_noop_and_atomic_overflow() {
    let m = model(product(vec![color("a", true)]));
    let mut s = answer(&m, &initial(&m).state, "a", text("#ABCDEF")).state;
    s.revision = n(SAFE_INTEGER);
    assert_eq!(answer(&m, &s, "a", text("#abcdef")).state, s);
    fails(&m, &s, &set(&s, "a", text("#000000")), "REVISION_OVERFLOW");
    let mut event = set(&s, "a", text("#ABCDEF"));
    if let ConfigurationEvent::SetAnswer {
        expected_revision, ..
    } = &mut event
    {
        *expected_revision = n(SAFE_INTEGER - 1);
    }
    fails(&m, &s, &event, "REVISION_CONFLICT");
}

#[test]
fn no_branch_and_backend_failure_are_atomic() {
    let mut p = product(vec![single("a", true)]);
    p.flow.edges[0].when = Some(condition());
    let m = controlled(p.clone(), |_, _| Ok(false), 1);
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("yes")), "NO_BRANCH");
    let m = controlled(
        p,
        |_, _| Err(KnotError::new("RULE_EVAL", "/flow/edges/0/when")),
        1,
    );
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("yes")), "RULE_EVAL");
}

#[test]
fn budget_is_shared_between_old_reconciliation_and_final_snapshot() {
    let mut p = product(vec![single("a", true)]);
    if let PriceComponent::Fixed { when, .. } = &mut p.pricing.components[0] {
        *when = Some(condition());
    }
    let m = controlled(p, |_, _| Ok(true), 1_100_000);
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("yes")), "RESOURCE_LIMIT");
}
