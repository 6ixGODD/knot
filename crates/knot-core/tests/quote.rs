#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;
use knot_compiler::CompiledModel;
use knot_core::{apply, initial, quote};
use knot_schema::{SAFE_INTEGER, quote::*, wire::*};
use support::*;
fn req(state: ConfigurationState) -> QuoteRequest {
    QuoteRequest {
        schema_version: Version::V1,
        state,
        focus_question_id: None,
        candidate_value: None,
    }
}
// Independent oracle traverses real public Apply, with explicit test domains.
fn oracle(model: &CompiledModel, state: &ConfigurationState, values: &[Answer]) -> Vec<i64> {
    let result = knot_core::evaluate(model, state).unwrap();
    if result.snapshot.flow.status == FlowStatus::Complete {
        return vec![result.snapshot.price.total_minor.unwrap().get()];
    }
    if result.snapshot.flow.status == FlowStatus::Blocked {
        return vec![];
    }
    let q = result.snapshot.flow.current_question.unwrap();
    let (qid, required, choices) = match q {
        CurrentQuestion::Single {
            id,
            required,
            options,
            ..
        } => (
            id,
            required,
            options
                .into_iter()
                .filter(|o| o.enabled)
                .map(|o| text(o.id.as_str()))
                .collect::<Vec<_>>(),
        ),
        CurrentQuestion::Multi {
            id,
            required,
            options,
            ..
        } => {
            let opts: Vec<_> = options
                .into_iter()
                .filter(|o| o.enabled)
                .map(|o| o.id)
                .collect();
            let choices = (0..(1u64 << opts.len()))
                .filter(|mask| !required || *mask != 0)
                .map(|mask| {
                    Answer::Multi(
                        opts.iter()
                            .enumerate()
                            .filter(|(i, _)| mask & (1u64 << i) != 0)
                            .map(|(_, id)| id.clone())
                            .collect(),
                    )
                })
                .collect();
            (id, required, choices)
        }
        CurrentQuestion::Color { id, required, .. }
        | CurrentQuestion::Integer { id, required, .. }
        | CurrentQuestion::Decimal { id, required, .. } => (id, required, values.to_vec()),
    };
    let mut prices = Vec::new();
    for value in choices {
        let next = apply(model, state, &set(state, qid.as_str(), value)).unwrap();
        prices.extend(oracle(model, &next.state, values));
    }
    if !required {
        prices.extend(oracle(
            model,
            &apply(model, state, &skip(state, qid.as_str()))
                .unwrap()
                .state,
            values,
        ));
    }
    prices
}
fn compare(model: &CompiledModel, state: &ConfigurationState, values: &[Answer]) {
    let original = state.clone();
    let q = quote(model, &req(state.clone())).unwrap();
    let all = oracle(model, state, values);
    assert_eq!(&original, state);
    assert_eq!(
        q.price.min_minor.map(Integer::get),
        all.iter().min().copied()
    );
    assert_eq!(
        q.price.max_minor.map(Integer::get),
        all.iter().max().copied()
    );
    if let Some(f) = q.focus {
        for c in f.candidates {
            if !c.enabled {
                continue;
            }
            let question = match &f.question {
                CurrentQuestion::Single { id, .. } | CurrentQuestion::Multi { id, .. } => id,
                _ => panic!(),
            };
            match apply(
                model,
                state,
                &set(state, question.as_str(), c.candidate_value.clone()),
            ) {
                Ok(next) => {
                    let all = oracle(model, &next.state, values);
                    assert_eq!(
                        c.price.as_ref().unwrap().min_minor.map(Integer::get),
                        all.iter().min().copied()
                    );
                    assert_eq!(
                        c.price.as_ref().unwrap().max_minor.map(Integer::get),
                        all.iter().max().copied()
                    );
                }
                Err(e) => {
                    assert_eq!(c.error.unwrap().error.code, e.code);
                    assert!(c.price.is_none());
                }
            }
        }
    }
}
#[test]
fn independent_finite_oracle_and_readonly_max_revision() {
    let m = speaker::speaker();
    let mut s = initial(&m).unwrap().state;
    compare(&m, &s, &[text("#315A79")]);
    s.revision = n(SAFE_INTEGER);
    let q = quote(&m, &req(s.clone())).unwrap();
    assert_eq!(q.revision, n(SAFE_INTEGER));
    assert_eq!(q.price.status, QuoteStatus::Range);
    assert_eq!(q.price.min_minor, Some(n(119900)));
    assert_eq!(q.price.max_minor, Some(n(302700)));
}
#[test]
fn numeric_grid_and_component_rounding_internal_extreme() {
    let q = Question::Decimal {
        id: id("quantity"),
        label: "Quantity".into(),
        required: Some(true),
        scale: n(1),
        min: Some("0".into()),
        max: Some("1".into()),
        step: Some("0.1".into()),
    };
    let mut p = product(vec![q]);
    for (name, unit) in [("positive", 1), ("positive2", 1), ("negative", -2)] {
        p.pricing.components.push(PriceComponent::Quantity {
            id: id(name),
            category: PriceCategory::Adjustment,
            unit_minor: n(unit),
            quantity_from: id("quantity"),
            when: None,
        });
    }
    let m = model(p);
    let s = initial(&m).unwrap().state;
    let values = (0..=10)
        .map(|v| text(&format!("{}", v as f64 / 10.0)))
        .collect::<Vec<_>>();
    compare(&m, &s, &values);
    let q = quote(&m, &req(s)).unwrap();
    assert_eq!(q.price.min_minor, Some(n(99)));
    assert_eq!(q.price.max_minor, Some(n(101)));
}
#[test]
fn domain_limit_blocked_and_candidate_empty_are_distinct() {
    let m = model(product(vec![Question::Integer {
        id: id("q"),
        label: "Q".into(),
        required: None,
        min: None,
        max: None,
        step: None,
    }]));
    let q = quote(&m, &req(initial(&m).unwrap().state)).unwrap();
    assert_eq!(q.price.reason, Some(QuoteReason::DomainLimit));
    assert_eq!(q.price.min_minor, None);
    let p = product(vec![Question::Multi {
        id: id("q"),
        label: "Q".into(),
        required: Some(true),
        options: vec![option("x", false)],
    }]);
    let m = model(p);
    let s = initial(&m).unwrap().state;
    let s = apply(&m, &s, &set(&s, "q", Answer::Multi(vec![id("x")])))
        .unwrap()
        .state;
    let mut r = req(s);
    r.focus_question_id = Some(id("q"));
    let q = quote(&m, &r).unwrap();
    assert_eq!(q.price.status, QuoteStatus::Final);
    let c = &q.focus.unwrap().candidates[0];
    assert_eq!(c.error.as_ref().unwrap().error.code, "INVALID_ANSWER");
    assert!(c.price.is_none());
    let p = product(vec![Question::Single {
        id: id("q"),
        label: "Q".into(),
        required: Some(true),
        options: vec![option("x", true)],
    }]);
    let m = controlled(p, |_, _| Ok(false), 1);
    let q = quote(&m, &req(initial(&m).unwrap().state)).unwrap();
    assert_eq!(q.price.status, QuoteStatus::NoCompletion);
    assert!(!q.focus.unwrap().candidates[0].enabled);
}
#[test]
fn errors_are_not_silently_excluded_and_search_is_bounded() {
    let mut p = product(vec![single("a", true)]);
    p.pricing.components.push(PriceComponent::Quantity {
        id: id("bad"),
        category: PriceCategory::Adjustment,
        unit_minor: n(1),
        quantity_from: id("a"),
        when: Some(condition()),
    });
    let m = controlled(
        p,
        |_, facts| Ok(facts.values().iter().any(Option::is_some)),
        1,
    );
    let s = initial(&m).unwrap().state;
    assert!(quote(&m, &req(s)).is_err());
    let p = product((0..20).map(|i| single(&format!("q{i}"), true)).collect());
    let m = model(p);
    // The real callCost ceiling is exhausted before the node ceiling in this
    // deep model, leaving no fuel to serialize an unavailable result.
    let error = quote(&m, &req(initial(&m).unwrap().state)).unwrap_err();
    assert_eq!(error.code, "RESOURCE_LIMIT");
    assert_eq!(error.details["limit"], "callCost");
}

#[test]
fn decimal_step_intersects_answer_precision() {
    let q = Question::Decimal {
        id: id("quantity"),
        label: "Quantity".into(),
        required: Some(true),
        scale: n(1),
        min: Some("0.05".into()),
        max: Some("0.35".into()),
        step: Some("0.05".into()),
    };
    let mut p = product(vec![q]);
    p.pricing.components.push(PriceComponent::Quantity {
        id: id("cost"),
        category: PriceCategory::Adjustment,
        unit_minor: n(100),
        quantity_from: id("quantity"),
        when: None,
    });
    let m = model(p);
    compare(
        &m,
        &initial(&m).unwrap().state,
        &[text("0.1"), text("0.2"), text("0.3")],
    );
}
#[test]
fn optional_empty_and_skip_have_distinct_exists_prices() {
    let mut p = product(vec![Question::Multi {
        id: id("q"),
        label: "Q".into(),
        required: Some(false),
        options: vec![option("x", false)],
    }]);
    p.pricing.components.push(PriceComponent::Fixed {
        id: id("answered"),
        category: PriceCategory::Adjustment,
        amount_minor: n(50),
        when: Some(condition()),
    });
    let m = controlled(p, |_, f| Ok(f.values()[0].is_some()), 1);
    let s = initial(&m).unwrap().state;
    compare(&m, &s, &[]);
    let q = quote(&m, &req(s)).unwrap();
    assert_eq!(q.price.min_minor, Some(n(100)));
    assert_eq!(q.price.max_minor, Some(n(150)));
}
#[test]
fn focus_preview_and_base_state_errors_have_strict_priority() {
    let m = model(product(vec![color("a", true), color("b", true)]));
    let s = initial(&m).unwrap().state;
    let mut r = req(s);
    r.focus_question_id = Some(id("b"));
    assert_eq!(quote(&m, &r).unwrap_err().code, "INACTIVE_QUESTION");
    r.focus_question_id = Some(id("a"));
    r.candidate_value = Some(text("red"));
    let q = quote(&m, &r).unwrap();
    let p = q.focus.unwrap().preview.unwrap();
    assert!(p.price.is_none());
    assert_eq!(p.error.unwrap().error.code, "INVALID_ANSWER");
    r.state.product_id = id("other");
    assert_eq!(quote(&m, &r).unwrap_err().code, "MODEL_MISMATCH");
}
#[test]
fn model_failure_leaves_fail_the_whole_quote() {
    for code in [
        "BASE_PRICE_CONFLICT",
        "NEGATIVE_TOTAL",
        "NUMERIC_OVERFLOW",
        "UNPRICED",
        "RULE_EVAL",
        "NO_BRANCH",
        "VISUAL_CONFLICT",
    ] {
        let mut p = product(vec![single("a", true)]);
        match code {
            "BASE_PRICE_CONFLICT" => p.pricing.components.push(PriceComponent::Fixed {
                id: id("other"),
                category: PriceCategory::Base,
                amount_minor: n(1),
                when: Some(condition()),
            }),
            "NEGATIVE_TOTAL" | "NUMERIC_OVERFLOW" => {
                p.pricing.components.push(PriceComponent::Fixed {
                    id: id("other"),
                    category: PriceCategory::Adjustment,
                    amount_minor: n(if code == "NEGATIVE_TOTAL" {
                        -1000
                    } else {
                        SAFE_INTEGER
                    }),
                    when: Some(condition()),
                })
            }
            "UNPRICED" => p.pricing.components.clear(),
            "NO_BRANCH" => p.flow.edges[0].when = Some(condition()),
            "VISUAL_CONFLICT" => {
                p.visual.targets.push(TargetDefinition {
                    id: id("target"),
                    order: Some(n(0)),
                    defaults: Default::default(),
                });
                p.visual.slots.push(VisualSlot {
                    id: id("slot"),
                    target: id("target"),
                    property: "visible".into(),
                    cases: vec![
                        VisualCase::Constant {
                            when: condition(),
                            value: VisualValue::Bool(true),
                        },
                        VisualCase::Constant {
                            when: condition(),
                            value: VisualValue::Bool(false),
                        },
                    ],
                });
            }
            _ => {
                if let PriceComponent::Fixed { when, .. } = &mut p.pricing.components[0] {
                    *when = Some(condition());
                }
            }
        }
        let m = controlled(
            p,
            move |_, facts| {
                if code == "RULE_EVAL" && facts.values().iter().any(Option::is_some) {
                    Err(knot_schema::KnotError::new("RULE_EVAL", ""))
                } else {
                    Ok(code != "NO_BRANCH" && facts.values().iter().any(Option::is_some))
                }
            },
            1,
        );
        let s = initial(&m).unwrap().state;
        assert_eq!(quote(&m, &req(s)).unwrap_err().code, code);
    }
}
#[test]
fn legal_candidate_without_a_continuation_is_not_an_invalid_answer() {
    let p = product(vec![
        single("a", true),
        Question::Single {
            id: id("b"),
            label: "B".into(),
            required: Some(true),
            options: vec![option("x", true)],
        },
    ]);
    let m = controlled(p, |_, _| Ok(false), 1);
    let s = initial(&m).unwrap().state;
    let q = quote(&m, &req(s)).unwrap();
    assert_eq!(q.price.status, QuoteStatus::NoCompletion);
    for c in q.focus.unwrap().candidates {
        assert!(c.error.is_none());
        assert_eq!(c.price.unwrap().status, QuoteStatus::NoCompletion);
    }
}

#[test]
fn maximum_explicit_candidate_array_still_obeys_shared_fuel() {
    let mut p = product(vec![Question::Single {
        id: id("q"),
        label: "Q".into(),
        required: Some(true),
        options: (0..256).map(|i| option(&format!("o{i}"), false)).collect(),
    }]);
    // Explicit backend work makes this a hard shared-fuel test, independent
    // of how much duplicate transition work a correct search eliminates.
    p.pricing.components.push(PriceComponent::Fixed {
        id: id("work"),
        category: PriceCategory::Adjustment,
        amount_minor: n(0),
        when: Some(condition()),
    });
    let m = controlled(p, |_, _| Ok(true), 10_000);
    let s = initial(&m).unwrap().state;
    let error = quote(&m, &req(s)).unwrap_err();
    assert_eq!(error.code, "RESOURCE_LIMIT");
    assert_eq!(error.details["limit"], "callCost");
}
