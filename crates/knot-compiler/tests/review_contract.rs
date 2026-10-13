//! Independent acceptance probes: graph oracle, round trips and boundary failures.
mod common;
use common::*;
use knot_compiler::compile_product;
use knot_rules::{CallBudget, ConditionId, Facts, FieldKind};
use knot_rules_zen::ZenCompiler;
use serde_json::{Value, json};

#[test]
fn every_four_vertex_dag_matches_independent_reachability() {
    // Deliberately scramble declaration and topological order. The oracle walks
    // edges independently from the compiler's reverse-topological bitsets.
    let order = [2, 0, 3, 1];
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    for mask in 0..64 {
        let mut p = base();
        p["questions"] = json!(
            (0..4)
                .map(|i| {
                    let mut q = question(&format!("q{i}"));
                    q["required"] = json!(false);
                    q
                })
                .collect::<Vec<_>>()
        );
        p["flow"]["start"] = json!("q2");
        let mut adjacency = [vec![], vec![], vec![], vec![]];
        let mut edges = vec![];
        for (bit, &(a, b)) in pairs.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                let (a, b) = (order[a], order[b]);
                adjacency[a].push(b);
                edges.push(json!({"from":format!("q{a}"), "to":format!("q{b}"),
                    "when":{"field":format!("q{a}"),"op":"exists"}}));
            }
        }
        for i in 0..4 {
            edges.push(json!({"from":format!("q{i}"),"to":"END"}));
        }
        p["flow"]["edges"] = json!(edges);
        let model = good(&p);
        for from in 0..4 {
            let mut seen = [false; 4];
            let mut queue = adjacency[from].clone();
            while let Some(to) = queue.pop() {
                if !seen[to] {
                    seen[to] = true;
                    queue.extend(&adjacency[to]);
                }
            }
            let expected: Vec<_> = (0..4).filter(|&i| seen[i]).collect();
            assert_eq!(
                model.successors()[from],
                expected,
                "mask={mask}, from={from}"
            );
        }
        let reachable = &model.successors()[2];
        let expected: Vec<_> = (0..4)
            .filter(|i| *i != 2 && !reachable.contains(i))
            .map(|i| format!("/questions/{i}"))
            .collect();
        assert_eq!(
            model
                .diagnostics()
                .iter()
                .map(|d| d.path.clone())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn normalized_model_roundtrips_preserving_identity_and_condition_sources() {
    let bytes = include_bytes!("../../../contracts/examples/speaker.json");
    let model = compile_product(bytes, &ZenCompiler).unwrap_or_else(|e| panic!("{e}"));
    let serialized = serde_json::to_vec(model.product()).unwrap_or_else(|e| panic!("{e}"));
    let restored = compile_product(&serialized, &ZenCompiler).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(model.digest(), restored.digest());
    assert_eq!(model.product(), restored.product());
    assert_eq!(model.sources(), restored.sources());
    assert_eq!(model.edges(), restored.edges());
    assert_eq!(model.successors(), restored.successors());
    for (i, source) in model.sources().iter().enumerate() {
        assert_eq!(restored.condition_id(source.path()), Some(ConditionId(i)));
    }
}

#[test]
fn requested_output_order_and_cross_model_facts_are_checked() {
    let mut p = base();
    p["pricing"]["components"] = json!([
        {"id":"present","category":"base","amountMinor":1,"when":{"field":"q","op":"exists"}},
        {"id":"absent","category":"adjustment","amountMinor":1,"when":{"not":{"field":"q","op":"exists"}}}
    ]);
    let model = good(&p);
    let facts = Facts::new(vec![None], &[FieldKind::Integer]).unwrap_or_else(|e| panic!("{e}"));
    let effects = model
        .program()
        .evaluate(
            &facts,
            &[ConditionId(1), ConditionId(0)],
            &mut CallBudget::new(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        effects.values(),
        &[(ConditionId(1), true), (ConditionId(0), false)]
    );
    let wrong = Facts::new(vec![], &[]).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        model
            .program()
            .evaluate(&wrong, &[ConditionId(0)], &mut CallBudget::new())
            .err()
            .map(|e| e.code),
        Some("RULE_EVAL".into())
    );
}

#[test]
fn numeric_token_boundaries_and_unicode_errors_never_panic_or_compile() {
    for token in [
        "9007199254740991.1",
        "-9007199254740991.1",
        "1e308",
        "-1e308",
        "1e-308",
    ] {
        let bytes = base()
            .to_string()
            .replace("\"amountMinor\":100", &format!("\"amountMinor\":{token}"));
        let error = compile_product(bytes.as_bytes(), &ZenCompiler)
            .err()
            .unwrap_or_else(|| panic!("accepted {token}"));
        assert_eq!(error.code, "BAD_SCHEMA", "{token}");
    }
    for raw in [r#"{"x":"\ud800"}"#, r#"{"x":"\udc00"}"#, r#"{} {}"#] {
        assert_eq!(
            compile_product(raw.as_bytes(), &ZenCompiler)
                .err()
                .map(|e| e.code),
            Some("BAD_JSON".into())
        );
    }
}

#[test]
fn metadata_numbers_do_not_panic_and_jcs_rounding_is_explicit() {
    for token in ["1e308", "1e-308", "9007199254740993"] {
        let mut p = base();
        p["metadata"] =
            serde_json::from_str(&format!("{{\"n\":{token}}}")).unwrap_or_else(|e| panic!("{e}"));
        let model = good(&p);
        let normalized = serde_json::to_value(model.product()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(model.digest(), good(&normalized).digest());
    }
    let mut p = base();
    p["metadata"] = serde_json::from_str(r#"{"n":9e308}"#).unwrap_or_else(|e| panic!("{e}"));
    assert!(compile(&p).is_err());
}

#[test]
fn all_visual_properties_enforce_their_registered_type_and_limits() {
    for (property, valid, invalid) in [
        ("visible", json!(false), json!(0)),
        ("assetRef", json!("mesh/雪"), json!("")),
        ("color", json!("#aabbcc"), json!("#abcd")),
        ("emission.strength", json!(1), json!(1.0001)),
        ("light.intensity", json!(0), json!(-0.0001)),
        (
            "transform.scale",
            json!([-1e9, 0, 1e9]),
            json!([0, 0, 1e9 + 1.0]),
        ),
    ] {
        for (value, accepted) in [(valid, true), (invalid, false)] {
            let mut p = base();
            let mut defaults = serde_json::Map::new();
            defaults.insert(property.into(), value);
            p["visual"]["targets"] = json!([{"id":"t","defaults":Value::Object(defaults)}]);
            assert_eq!(compile(&p).is_ok(), accepted, "{property}");
        }
    }
}
