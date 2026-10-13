mod support;
use knot_schema::{KnotError, wire::*};
use support::*;

fn checked_result(result: &TransitionResult, validator: &knot_schema::validation::Validator) {
    let json = serde_json::to_value(result).unwrap_or_else(|e| panic!("{e}"));
    validator
        .validate(&json)
        .unwrap_or_else(|e| panic!("{e}: {json}"));
    let restored: TransitionResult = serde_json::from_value(json).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(*result, restored);
}

// Independent finite-state oracle: enumerate every optional prefix, including
// a traversal order different from declaration order. No Core helper is used
// to calculate the expected facts, revision, or automatic deletion order.
#[test]
fn exhaustive_optional_prefix_transitions_and_wire_roundtrips() {
    let mut p = product(vec![
        color("a", false),
        color("b", false),
        color("c", false),
    ]);
    p.flow.start = id("c");
    p.flow.edges = [("c", "a"), ("a", "b"), ("b", "END")]
        .into_iter()
        .map(|(from, to)| Edge {
            from: id(from),
            to: id(to),
            when: None,
        })
        .collect();
    let m = model(p);
    let schema = serde_json::from_str(include_str!(
        "../../../contracts/schemas/result.schema.json"
    ))
    .unwrap_or_else(|e| panic!("{e}"));
    let validator =
        knot_schema::validation::Validator::new(&schema).unwrap_or_else(|e| panic!("{e}"));
    let path = ["c", "a", "b"];
    let mut count = 0;
    for length in 0..=3usize {
        for encoding in 0..3usize.pow(length as u32) {
            let mut old = initial(&m).state;
            old.revision = n(17);
            let mut digits = encoding;
            for q in path.iter().take(length) {
                match digits % 3 {
                    0 => old.skipped.push(id(q)),
                    1 => {
                        old.answers.insert(id(q), text("#ABCDEF"));
                    }
                    _ => {
                        old.answers.insert(id(q), text("#112233"));
                    }
                }
                digits /= 3;
            }
            old.skipped.sort();
            let evaluated = knot_core::evaluate(&m, &old).unwrap_or_else(|e| panic!("{e}"));
            checked_result(&evaluated, &validator);
            for (position, q) in path.iter().enumerate() {
                for action in 0..3 {
                    let event = match action {
                        0 => set(&old, q, text("#abcdef")),
                        1 => clear(&old, q),
                        _ => skip(&old, q),
                    };
                    let allowed = match action {
                        0 => position <= length,
                        1 => old.answers.contains_key(&id(q)),
                        _ => position == length,
                    };
                    if !allowed {
                        fails(&m, &old, &event, "INACTIVE_QUESTION");
                        continue;
                    }
                    let before =
                        serde_json::to_vec(&(&old, &event)).unwrap_or_else(|e| panic!("{e}"));
                    let mut expected = old.clone();
                    let mut changes = Vec::new();
                    let kind = match action {
                        0 => {
                            expected.answers.insert(id(q), text("#ABCDEF"));
                            expected.skipped.retain(|i| i != &id(q));
                            ChangeKind::Set
                        }
                        1 => {
                            expected.answers.remove(&id(q));
                            ChangeKind::Cleared
                        }
                        _ => {
                            expected.skipped.push(id(q));
                            expected.skipped.sort();
                            ChangeKind::Skipped
                        }
                    };
                    if expected != old {
                        changes.push(AnswerChange {
                            question_id: id(q),
                            kind,
                            reason: ChangeReason::User,
                            rule_ids: None,
                        });
                    }
                    if action == 1 {
                        let mut removed: Vec<_> = path
                            .iter()
                            .skip(position + 1)
                            .filter(|q| {
                                old.answers.contains_key(&id(q)) || old.skipped.contains(&id(q))
                            })
                            .copied()
                            .collect();
                        removed.sort();
                        for removed in removed {
                            expected.answers.remove(&id(removed));
                            expected.skipped.retain(|i| i != &id(removed));
                            changes.push(AnswerChange {
                                question_id: id(removed),
                                kind: ChangeKind::Cleared,
                                reason: ChangeReason::BranchUnreachable,
                                rule_ids: Some(vec![]),
                            });
                        }
                    }
                    if expected != old {
                        expected.revision = n(18);
                    }
                    let result = knot_core::apply(&m, &old, &event)
                        .unwrap_or_else(|e| panic!("{encoding}/{length}/{q}/{action}: {e}"));
                    assert_eq!(result.state, expected);
                    assert_eq!(result.changes.answer_changes, changes);
                    let completed = if action == 1 {
                        position
                    } else {
                        length.max(position + 1)
                    };
                    assert_eq!(
                        result.snapshot.flow.path,
                        path.iter()
                            .take((completed + 1).min(3))
                            .map(|q| id(q))
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(
                        result.snapshot.flow.next_question_id,
                        path.get(completed).map(|q| id(q))
                    );
                    assert_eq!(
                        result.snapshot.price.status,
                        if completed == 3 {
                            PriceStatus::Final
                        } else {
                            PriceStatus::Estimated
                        }
                    );
                    assert_eq!(
                        before,
                        serde_json::to_vec(&(&old, &event)).unwrap_or_else(|e| panic!("{e}"))
                    );
                    checked_result(&result, &validator);
                    let restored =
                        knot_core::evaluate(&m, &result.state).unwrap_or_else(|e| panic!("{e}"));
                    assert_eq!(restored.snapshot, result.snapshot);
                    count += 1;
                }
            }
        }
    }
    assert!(
        count > 150,
        "the oracle must exercise the full state space: {count}"
    );
}

#[test]
fn component_rounding_happens_before_sum_for_positive_and_negative_halves() {
    for unit in [1, -1] {
        let mut p = product(vec![Question::Decimal {
            id: id("a"),
            label: "quantity".into(),
            required: Some(true),
            scale: n(1),
            min: None,
            max: None,
            step: None,
        }]);
        for component in ["first", "second"] {
            p.pricing.components.push(PriceComponent::Quantity {
                id: id(component),
                category: PriceCategory::Adjustment,
                unit_minor: n(unit),
                quantity_from: id("a"),
                when: None,
            });
        }
        let m = model(p);
        let result = answer(&m, &initial(&m).state, "a", text("0.5"));
        assert_eq!(result.snapshot.price.total_minor, Some(n(100 + 2 * unit)));
        assert_eq!(
            result
                .snapshot
                .price
                .components
                .iter()
                .map(|c| c.amount_minor.get())
                .collect::<Vec<_>>(),
            vec![100, unit, unit]
        );
    }
}

#[test]
fn edge_first_match_short_circuits_later_failures_and_old_state_errors_win() {
    let mut p = product(vec![single("a", true)]);
    p.flow.edges = vec![
        Edge {
            from: id("a"),
            to: id("END"),
            when: Some(condition())
        };
        2
    ];
    p.flow.edges.push(Edge {
        from: id("a"),
        to: id("END"),
        when: None,
    });
    let m = controlled(
        p,
        |condition, facts| {
            assert!(single_fact(facts, 0, "yes"));
            if condition == 0 {
                Ok(true)
            } else {
                Err(KnotError::new("RULE_EVAL", "/flow/edges/1/when"))
            }
        },
        1,
    );
    let old = initial(&m).state;
    assert_eq!(
        answer(&m, &old, "a", text("yes")).snapshot.flow.status,
        FlowStatus::Complete
    );
    let mut invalid = old.clone();
    invalid.answers.insert(id("a"), text("unknown"));
    let mut event = set(&invalid, "a", text("yes"));
    if let ConfigurationEvent::SetAnswer {
        expected_revision, ..
    } = &mut event
    {
        *expected_revision = n(123);
    }
    fails(&m, &invalid, &event, "INVALID_STATE");
}
