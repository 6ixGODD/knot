use knot_rules_zen::{compile_probe, sync::poll_once};
#[test]
fn numeric_templates_distinguish_false_true_and_closed_boundaries() {
    use knot_rules::*;
    use knot_rules_zen::ZenCompiler;
    use knot_schema::{Decimal, SAFE_INTEGER, wire::Integer};
    // Each present value exercises both true and false outcomes. This catches
    // a template that merely tests presence, reverses an operator or opens a range.
    for kind in [FieldKind::Integer, FieldKind::Decimal] {
        let field = Field { index: 0, kind };
        let scalar = |n: i64| match kind {
            FieldKind::Integer => {
                Scalar::Integer(Integer::new(n).unwrap_or_else(|e| panic!("{e}")))
            }
            _ => Scalar::Decimal(Decimal::parse(&n.to_string()).unwrap_or_else(|e| panic!("{e}"))),
        };
        let conditions = vec![
            Condition::Eq(field, scalar(0)),
            Condition::Ne(field, scalar(0)),
            Condition::Gt(field, scalar(0)),
            Condition::Gte(field, scalar(0)),
            Condition::Lt(field, scalar(0)),
            Condition::Lte(field, scalar(0)),
            Condition::Between(field, scalar(-1), scalar(1)),
            Condition::In(field, vec![scalar(-1), scalar(1)]),
        ];
        let sources = (0..conditions.len())
            .map(|i| {
                RuleSource::new(format!("/review/{i}"), None).unwrap_or_else(|e| panic!("{e}"))
            })
            .collect();
        let program = ZenCompiler
            .compile(
                &[kind],
                &conditions,
                &sources,
                &mut CompileBudget::default(),
            )
            .unwrap_or_else(|e| panic!("{e}"));
        let requested: Vec<_> = (0..conditions.len()).map(ConditionId).collect();
        for n in [-SAFE_INTEGER, -2, -1, 0, 1, 2, SAFE_INTEGER] {
            let expected = [
                n == 0,
                n != 0,
                n > 0,
                n >= 0,
                n < 0,
                n <= 0,
                (-1..=1).contains(&n),
                n == -1 || n == 1,
            ];
            let facts = Facts::new(vec![Some(FactValue::Scalar(scalar(n)))], &[kind])
                .unwrap_or_else(|e| panic!("{e}"));
            let effects = program
                .evaluate(&facts, &requested, &mut CallBudget::new())
                .unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(effects.values().len(), expected.len());
            for (index, ((id, actual), expected)) in
                effects.values().iter().zip(expected).enumerate()
            {
                assert_eq!(*id, ConditionId(index));
                assert_eq!(*actual, expected, "{kind:?} {n} condition {}", id.0);
            }
        }
    }
}

#[test]
fn wide_boolean_tree_and_declared_depth_bound() {
    use knot_rules::*;
    use knot_rules_zen::ZenCompiler;
    let field = Field {
        index: 0,
        kind: FieldKind::Color,
    };
    let condition = Condition::All(vec![Condition::Exists(field); 2048]);
    let program = ZenCompiler
        .compile(
            &[field.kind],
            &[condition],
            &vec![RuleSource::new("/wide".into(), None).unwrap_or_else(|e| panic!("{e}"))],
            &mut CompileBudget::default(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let facts = Facts::new(vec![None], &[field.kind]).unwrap_or_else(|e| panic!("{e}"));
    let result = program
        .evaluate(&facts, &[ConditionId(0)], &mut CallBudget::new())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(result.values(), &[(ConditionId(0), false)]);
    let mut too_deep = Condition::Exists(field);
    for _ in 0..32 {
        too_deep = Condition::Not(Box::new(too_deep));
    }
    assert!(
        ZenCompiler
            .compile(
                &[field.kind],
                &[too_deep],
                &vec![RuleSource::new("/deep".into(), None).unwrap_or_else(|e| panic!("{e}"))],
                &mut CompileBudget::default()
            )
            .is_err()
    );
}
#[test]
fn existing_tokio_runtimes_need_no_nested_runtime() {
    for multithread in [false, true] {
        let mut builder = if multithread {
            tokio::runtime::Builder::new_multi_thread()
        } else {
            tokio::runtime::Builder::new_current_thread()
        };
        let runtime = builder
            .worker_threads(2)
            .build()
            .unwrap_or_else(|e| panic!("{e}"));
        runtime.block_on(async {
            real_graph_is_ready_and_preflight_rejects_invalid_expression();
        });
    }
}
#[test]
fn real_graph_is_ready_and_preflight_rejects_invalid_expression() {
    let decision = compile_probe("a == b").unwrap_or_else(|e| panic!("{e}"));
    let context =
        serde_json::from_value(serde_json::json!({"a":1,"b":1})).unwrap_or_else(|e| panic!("{e}"));
    let response = poll_once(decision.evaluate(context))
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|e| panic!("{e}"));
    let result = serde_json::to_value(response.result).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(result, serde_json::json!({"result":true}));
    assert!(compile_probe("a == (").is_err());
    assert!(poll_once(std::future::pending::<()>()).is_err());
}

#[test]
fn all_operators_missing_exact_decimals_and_data_only_constants() {
    use knot_rules::*;
    use knot_rules_zen::ZenCompiler;
    use knot_schema::{
        Decimal,
        wire::{Id, Integer},
    };
    let integer = |n| Scalar::Integer(Integer::new(n).unwrap_or_else(|e| panic!("{e}")));
    let decimal = |s| Scalar::Decimal(Decimal::parse(s).unwrap_or_else(|e| panic!("{e}")));
    let id = |s: &str| Id::new(s.into()).unwrap_or_else(|e| panic!("{e}"));
    let n = Field {
        index: 0,
        kind: FieldKind::Integer,
    };
    let d = Field {
        index: 1,
        kind: FieldKind::Decimal,
    };
    let s = Field {
        index: 2,
        kind: FieldKind::Single,
    };
    let m = Field {
        index: 3,
        kind: FieldKind::Multi,
    };
    let fields = [n.kind, d.kind, s.kind, m.kind];
    let conditions = vec![
        Condition::Eq(n, integer(2)),
        Condition::Ne(n, integer(3)),
        Condition::Gt(n, integer(1)),
        Condition::Gte(n, integer(2)),
        Condition::Lt(n, integer(3)),
        Condition::Lte(n, integer(2)),
        Condition::Between(n, integer(1), integer(2)),
        Condition::In(n, vec![integer(1), integer(2)]),
        Condition::Contains(m, id("led")),
        Condition::Exists(n),
        Condition::All(vec![Condition::Exists(n), Condition::Eq(n, integer(2))]),
        Condition::Any(vec![
            Condition::Eq(n, integer(1)),
            Condition::Eq(n, integer(2)),
        ]),
        Condition::Not(Box::new(Condition::Eq(n, integer(3)))),
        Condition::Gt(d, decimal("9007199254740990.123456788")),
        Condition::Lt(d, decimal("9007199254740990.123456790")),
        Condition::Eq(s, Scalar::Single(id("') || true || ('"))),
    ];
    let sources = (0..conditions.len())
        .map(|i| RuleSource::new(format!("/test/{i}"), None).unwrap_or_else(|e| panic!("{e}")))
        .collect();
    let program = ZenCompiler
        .compile(
            &fields,
            &conditions,
            &sources,
            &mut CompileBudget::default(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let ids = (0..conditions.len()).map(ConditionId).collect::<Vec<_>>();
    let facts = Facts::new(
        vec![
            Some(FactValue::Scalar(integer(2))),
            Some(FactValue::Scalar(decimal("9007199254740990.123456789"))),
            Some(FactValue::Scalar(Scalar::Single(id("') || true || ('")))),
            Some(FactValue::Multi(vec![id("led")])),
        ],
        &fields,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let actual = program
        .evaluate(&facts, &ids, &mut CallBudget::new())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(actual.values().iter().all(|(_, v)| *v), "{actual:?}");
    let missing = Facts::new(vec![None; 4], &fields).unwrap_or_else(|e| panic!("{e}"));
    let actual = program
        .evaluate(&missing, &ids, &mut CallBudget::new())
        .unwrap_or_else(|e| panic!("{e}"));
    for (id, value) in actual.values() {
        assert_eq!(*value, id.0 == 12, "missing condition {}", id.0);
    }
    let mut budget = CallBudget::new();
    assert!(budget.charge(2_000_000).is_ok());
    assert!(program.evaluate(&facts, &ids, &mut budget).is_err());
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let program = &program;
            let facts = &facts;
            let ids = &ids;
            scope.spawn(move || {
                for _ in 0..20 {
                    assert!(
                        program
                            .evaluate(facts, ids, &mut CallBudget::new())
                            .unwrap_or_else(|e| panic!("{e}"))
                            .values()
                            .iter()
                            .all(|(_, v)| *v)
                    );
                }
            });
        }
    });
}
