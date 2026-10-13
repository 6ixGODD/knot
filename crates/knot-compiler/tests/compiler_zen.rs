mod common;
use common::*;
use knot_rules::{CallBudget, ConditionId, FactValue, Facts, FieldKind, Scalar};
use knot_schema::{
    Decimal,
    wire::{Id, Integer},
};
use serde_json::{Value, json};
use std::sync::Arc;

fn integer(n: i64) -> Scalar {
    Scalar::Integer(Integer::new(n).unwrap_or_else(|e| panic!("{e}")))
}
fn eval(model: &knot_compiler::CompiledModel, kind: FieldKind, value: Option<FactValue>) -> bool {
    let facts = Facts::new(vec![value], &[kind]).unwrap_or_else(|e| panic!("{e}"));
    model
        .program()
        .evaluate(&facts, &[ConditionId(0)], &mut CallBudget::new())
        .unwrap_or_else(|e| panic!("{e}"))
        .values()[0]
        .1
}
#[test]
fn product_numeric_ops_distinguish_true_false_missing_and_closed_bounds() {
    for decimal in [false, true] {
        for op in [
            "eq", "ne", "gt", "gte", "lt", "lte", "between", "in", "exists",
        ] {
            let scalar = |n: i64| {
                if decimal {
                    json!(n.to_string())
                } else {
                    json!(n)
                }
            };
            let mut c = json!({"field":"q","op":op});
            if op != "exists" {
                c["value"] = match op {
                    "between" | "in" => json!([scalar(-1), scalar(1)]),
                    _ => scalar(0),
                };
            }
            let mut p = condition(c);
            if decimal {
                p["questions"][0] = json!({"id":"q","type":"decimal","label":"Q","scale":9});
            }
            let model = good(&p);
            let kind = if decimal {
                FieldKind::Decimal
            } else {
                FieldKind::Integer
            };
            for n in [
                -9_007_199_254_740_991,
                -2,
                -1,
                0,
                1,
                2,
                9_007_199_254_740_991,
            ] {
                let value = if decimal {
                    Scalar::Decimal(
                        Decimal::parse(&n.to_string()).unwrap_or_else(|e| panic!("{e}")),
                    )
                } else {
                    integer(n)
                };
                let expected = match op {
                    "eq" => n == 0,
                    "ne" => n != 0,
                    "gt" => n > 0,
                    "gte" => n >= 0,
                    "lt" => n < 0,
                    "lte" => n <= 0,
                    "between" => (-1..=1).contains(&n),
                    "in" => n == -1 || n == 1,
                    _ => true,
                };
                assert_eq!(
                    eval(&model, kind, Some(FactValue::Scalar(value))),
                    expected,
                    "{decimal}/{op}/{n}"
                );
            }
            assert!(!eval(&model, kind, None), "missing {op}");
        }
    }
}
#[test]
fn strings_colors_multis_boolean_composition_and_exact_fraction() {
    let evil = "x\"\\'); true or ('雪/~";
    for op in ["eq", "ne", "in"] {
        let mut p = condition(
            json!({"field":"q","op":op,"value":if op=="in" {json!([evil])} else {json!(evil)}}),
        );
        p["questions"][0] = json!({"id":"q","type":"single","label":"Q","options":[{"id":evil,"label":"Evil"},{"id":"other","label":"Other"}]});
        let model = good(&p);
        for value in [evil, "other"] {
            let id = Id::new(value.into()).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(
                eval(
                    &model,
                    FieldKind::Single,
                    Some(FactValue::Scalar(Scalar::Single(id)))
                ),
                (value == evil) != (op == "ne")
            );
        }
        assert!(!eval(&model, FieldKind::Single, None));
    }
    let mut p = condition(
        json!({"all":[{"field":"q","op":"contains","value":evil},{"not":{"field":"q","op":"contains","value":"other"}}]}),
    );
    p["questions"][0] = json!({"id":"q","type":"multi","label":"Q","options":[{"id":evil,"label":"Evil"},{"id":"other","label":"Other"}]});
    let model = good(&p);
    assert!(eval(
        &model,
        FieldKind::Multi,
        Some(FactValue::Multi(vec![
            Id::new(evil.into()).unwrap_or_else(|e| panic!("{e}"))
        ]))
    ));
    assert!(!eval(
        &model,
        FieldKind::Multi,
        Some(FactValue::Multi(vec![]))
    ));
    let mut p = condition(
        json!({"any":[{"field":"q","op":"eq","value":"#aabbcc"},{"not":{"field":"q","op":"exists"}}]}),
    );
    p["questions"][0] = json!({"id":"q","type":"color","label":"Q"});
    let model = good(&p);
    assert!(eval(
        &model,
        FieldKind::Color,
        Some(FactValue::Scalar(Scalar::Color("#AABBCC".into())))
    ));
    assert!(!eval(
        &model,
        FieldKind::Color,
        Some(FactValue::Scalar(Scalar::Color("#000000".into())))
    ));
    assert!(eval(&model, FieldKind::Color, None));
    let mut p = condition(json!({"field":"q","op":"eq","value":"9007199254740990.000000001"}));
    p["questions"][0] = json!({"id":"q","type":"decimal","label":"Q","scale":9});
    let model = good(&p);
    for (value, expected) in [
        ("9007199254740990.000000001", true),
        ("9007199254740990.000000002", false),
    ] {
        assert_eq!(
            eval(
                &model,
                FieldKind::Decimal,
                Some(FactValue::Scalar(Scalar::Decimal(
                    Decimal::parse(value).unwrap_or_else(|e| panic!("{e}"))
                )))
            ),
            expected
        );
    }
}
#[test]
fn invalid_operands_have_precise_source_and_never_compile() {
    for (condition, path) in [
        (json!({"field":"missing","op":"eq","value":0}), "/field"),
        (json!({"field":"q","op":"eq","value":"0"}), "/value"),
        (json!({"field":"q","op":"between","value":[2,1]}), "/value"),
        (json!({"field":"q","op":"contains","value":"x"}), "/op"),
    ] {
        let e = error(&common::condition(condition));
        assert_eq!(e.code, "INVALID_MODEL");
        assert_eq!(e.path, format!("/pricing/components/0/when{path}"));
        assert_eq!(e.rule_ids, ["base"]);
    }
}
#[test]
fn compiled_model_is_shared_across_threads_and_budget_is_monotonic() {
    let model = Arc::new(good(&condition(
        json!({"not":{"field":"q","op":"eq","value":0}}),
    )));
    std::thread::scope(|scope| {
        for n in 0..8 {
            let model = Arc::clone(&model);
            scope.spawn(move || {
                for _ in 0..16 {
                    assert_eq!(
                        eval(
                            &model,
                            FieldKind::Integer,
                            Some(FactValue::Scalar(integer(n)))
                        ),
                        n != 0
                    );
                }
            });
        }
    });
    assert!(eval(&model, FieldKind::Integer, None)); // not(eq(missing)) is true
    let facts = Facts::new(vec![None], &[FieldKind::Integer]).unwrap_or_else(|e| panic!("{e}"));
    let mut budget = CallBudget::new();
    budget.charge(1_999_999).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        model
            .program()
            .evaluate(&facts, &[ConditionId(0)], &mut budget)
            .err()
            .map(|e| e.code),
        Some("RESOURCE_LIMIT".into())
    );
    assert_eq!(budget.remaining(), 1);
    assert_eq!(
        model
            .program()
            .evaluate(&facts, &[ConditionId(9)], &mut CallBudget::new())
            .err()
            .map(|e| e.code),
        Some("RULE_EVAL".into())
    );
}
#[test]
fn full_product_scale_precompiles_every_root_with_stable_source_order() {
    let mut p = base();
    p["questions"] = Value::Array((0..100).map(|i| question(&format!("q{i}"))).collect());
    p["flow"]["start"] = json!("q0");
    p["flow"]["edges"] = Value::Array((0..100).map(|i| json!({"from":format!("q{i}"),"to":if i ==99 {"END".into()} else {format!("q{}",i+1)},"when":{"field":format!("q{i}"),"op":"exists"}})).collect());
    p["pricing"]["components"] = Value::Array((0..100).map(|i| json!({"id":format!("p{i}"),"category":"base","amountMinor":i,"when":{"field":format!("q{i}"),"op":"exists"}})).collect());
    p["visual"]["targets"] = Value::Array(
        (0..100)
            .map(|i| json!({"id":format!("t{i}"),"defaults":{"visible":false}}))
            .collect(),
    );
    p["visual"]["slots"] = Value::Array((0..100).map(|i| json!({"id":format!("s{i}"),"target":format!("t{i}"),"property":"visible","cases":(0..3).map(|n| json!({"when":{"field":format!("q{i}"),"op":"eq","value":n},"value":true})).collect::<Vec<_>>()})).collect());
    let model = good(&p);
    assert_eq!(model.sources().len(), 500);
    assert_eq!(model.sources()[100].rule_id(), "p0");
    assert_eq!(model.sources()[200].rule_id(), "s0");
    assert_eq!(model.sources()[499].path(), "/visual/slots/99/cases/2/when");
    let kinds = vec![FieldKind::Integer; 100];
    let facts = Facts::new(vec![Some(FactValue::Scalar(integer(1))); 100], &kinds)
        .unwrap_or_else(|e| panic!("{e}"));
    for i in 0..500 {
        let result = model
            .program()
            .evaluate(&facts, &[ConditionId(i)], &mut CallBudget::new())
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            result.values(),
            &[(ConditionId(i), i < 200 || (i - 200) % 3 == 1)]
        );
    }
}
