mod support;
use knot_schema::wire::*;
use support::*;

#[test]
fn required_empty_multi_and_invalid_skip_clear_are_rejected() {
    let mut p = product(vec![
        Question::Multi {
            id: id("a"),
            label: "a".into(),
            required: Some(true),
            options: vec![option("x", false)],
        },
        color("b", false),
    ]);
    let m = model(p.clone());
    let s = initial(&m).state;
    fails(
        &m,
        &s,
        &set(&s, "a", Answer::Multi(vec![])),
        "INVALID_ANSWER",
    );
    fails(&m, &s, &skip(&s, "a"), "INACTIVE_QUESTION");
    fails(&m, &s, &clear(&s, "a"), "INACTIVE_QUESTION");
    let mut bad = set(&s, "a", Answer::Multi(vec![id("x")]));
    if let ConfigurationEvent::SetAnswer {
        expected_revision, ..
    } = &mut bad
    {
        *expected_revision = n(-1);
    }
    fails(&m, &s, &bad, "BAD_SCHEMA");
    if let Question::Multi {
        required, options, ..
    } = &mut p.questions[0]
    {
        *required = Some(false);
        options[0].enabled_when = Some(condition());
    }
    let m = controlled(p, |_, _| Ok(false), 1);
    let s = initial(&m);
    assert_eq!(s.snapshot.flow.status, FlowStatus::InProgress);
    let s = answer(&m, &s.state, "a", Answer::Multi(vec![]));
    assert_eq!(s.snapshot.flow.next_question_id, Some(id("b")));
    let mut invalid = s.state.clone();
    invalid.skipped = vec![id("a")];
    fails(
        &m,
        &invalid,
        &set(&invalid, "a", Answer::Multi(vec![])),
        "INVALID_STATE",
    );
}

#[test]
fn initial_prefix_optional_skip_then_answer_and_noop() {
    let m = model(product(vec![color("a", false), color("b", true)]));
    let s = initial(&m);
    assert_eq!(s.state.revision, n(0));
    assert!(s.state.answers.is_empty());
    assert_eq!(s.snapshot.flow.path, vec![id("a")]);
    assert!(s.changes.answer_changes.is_empty());
    fails(
        &m,
        &s.state,
        &set(&s.state, "b", text("#112233")),
        "INACTIVE_QUESTION",
    );
    let s = knot_core::apply(&m, &s.state, &skip(&s.state, "a")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.state.skipped, vec![id("a")]);
    assert_eq!(s.snapshot.flow.path, vec![id("a"), id("b")]);
    let s = answer(&m, &s.state, "a", text("#abcdef"));
    assert!(s.state.skipped.is_empty());
    assert_eq!(s.state.answers[&id("a")], text("#ABCDEF"));
    assert_eq!(s.changes.answer_changes.len(), 1);
    let replay = set(&s.state, "a", text("#abcdef"));
    let same = knot_core::apply(&m, &s.state, &replay).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(same.state, s.state);
    assert!(same.changes.answer_changes.is_empty());
    let changed = answer(&m, &s.state, "b", text("#000000"));
    assert_eq!(changed.snapshot.flow.status, FlowStatus::Complete);
    fails(&m, &changed.state, &replay, "REVISION_CONFLICT");
}

#[test]
fn strict_state_identity_normalization_suffix_and_revision() {
    let m = model(product(vec![color("a", true), color("b", false)]));
    let s = initial(&m).state;
    for mutation in 0..6 {
        let mut bad = s.clone();
        let code = match mutation {
            0 => {
                bad.model_digest = "sha256:wrong".into();
                "MODEL_MISMATCH"
            }
            1 => {
                bad.answers.insert(id("a"), text("#abcdef"));
                "INVALID_STATE"
            }
            2 => {
                bad.answers.insert(id("b"), text("#000000"));
                "INVALID_STATE"
            }
            3 => {
                bad.skipped.push(id("a"));
                "INVALID_STATE"
            }
            4 => {
                bad.revision = n(-1);
                "INVALID_STATE"
            }
            _ => {
                bad.answers.insert(id("unknown"), text("#000000"));
                "INVALID_STATE"
            }
        };
        fails(&m, &bad, &set(&bad, "a", text("#000000")), code);
        let before = bad.clone();
        assert_eq!(
            knot_core::evaluate(&m, &bad)
                .err()
                .unwrap_or_else(|| panic!("error"))
                .code,
            code
        );
        assert_eq!(bad, before);
    }
}

#[test]
fn integer_decimal_step_and_multi_normalization() {
    let questions = vec![
        Question::Integer {
            id: id("a"),
            label: "a".into(),
            required: Some(true),
            min: Some(n(-3)),
            max: Some(n(9)),
            step: Some(n(2)),
        },
        Question::Decimal {
            id: id("b"),
            label: "b".into(),
            required: Some(true),
            scale: n(2),
            min: Some("0.1".into()),
            max: Some("1".into()),
            step: Some("0.2".into()),
        },
        Question::Multi {
            id: id("c"),
            label: "c".into(),
            required: Some(false),
            options: vec![option("z", false), option("a", false)],
        },
    ];
    let m = model(product(questions));
    let s = initial(&m).state;
    fails(
        &m,
        &s,
        &set(&s, "a", Answer::Integer(n(0))),
        "INVALID_ANSWER",
    );
    let s = answer(&m, &s, "a", Answer::Integer(n(-1))).state;
    fails(&m, &s, &set(&s, "b", text("0.2")), "INVALID_ANSWER");
    fails(&m, &s, &set(&s, "b", text("0.301")), "INVALID_ANSWER");
    let s = answer(&m, &s, "b", text("00.3000")).state;
    assert_eq!(s.answers[&id("b")], text("0.3"));
    fails(
        &m,
        &s,
        &set(&s, "c", Answer::Multi(vec![id("a"), id("a")])),
        "INVALID_ANSWER",
    );
    let s = answer(&m, &s, "c", Answer::Multi(vec![id("a"), id("z")]));
    assert_eq!(
        s.state.answers[&id("c")],
        Answer::Multi(vec![id("z"), id("a")])
    );
    let s = answer(&m, &s.state, "c", Answer::Multi(vec![]));
    assert_eq!(s.snapshot.flow.status, FlowStatus::Complete);
}

#[test]
fn blocked_is_recoverable_and_optional_is_not_blocked() {
    for required in [true, false] {
        let p = product(vec![
            single("a", true),
            Question::Single {
                id: id("b"),
                label: "b".into(),
                required: Some(required),
                options: vec![option("only", true)],
            },
        ]);
        let m = controlled(p, |_, f| Ok(single_fact(f, 0, "yes")), 1);
        let s = answer(&m, &initial(&m).state, "a", text("no"));
        assert_eq!(
            s.snapshot.flow.status,
            if required {
                FlowStatus::Blocked
            } else {
                FlowStatus::InProgress
            }
        );
        assert_eq!(s.snapshot.validation.valid, !required);
        let restored = knot_core::evaluate(&m, &s.state).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(s.snapshot, restored.snapshot);
        fails(
            &m,
            &s.state,
            &set(&s.state, "b", text("only")),
            "INVALID_OPTION",
        );
        if !required {
            assert_eq!(
                knot_core::apply(&m, &s.state, &skip(&s.state, "b"))
                    .unwrap_or_else(|e| panic!("{e}"))
                    .snapshot
                    .flow
                    .status,
                FlowStatus::Complete
            );
        }
        let s = answer(&m, &s.state, "a", text("yes"));
        assert!(s.snapshot.validation.valid);
        assert_eq!(
            answer(&m, &s.state, "b", text("only")).snapshot.flow.status,
            FlowStatus::Complete
        );
    }
}
