mod common;
use common::*;
use knot_compiler::compile_product;
use knot_rules_zen::ZenCompiler;
use serde_json::{Value, json};

#[test]
fn lexical_schema_and_safe_integer_boundaries() {
    for (bytes, code) in [
        (b"".as_slice(), "BAD_JSON"),
        (&[255], "INVALID_UTF8"),
        (b"{\"x\":0,\"x\":1}", "BAD_JSON"),
    ] {
        assert_eq!(
            compile_product(bytes, &ZenCompiler).err().map(|e| e.code),
            Some(code.into())
        );
    }
    let text = base()
        .to_string()
        .replace("\"amountMinor\":100", "\"amountMinor\":1.0");
    assert!(compile_product(text.as_bytes(), &ZenCompiler).is_ok());
    for value in ["9007199254740991.1", "9007199254740992", "0.5"] {
        let text = base()
            .to_string()
            .replace("\"amountMinor\":100", &format!("\"amountMinor\":{value}"));
        assert_eq!(
            compile_product(text.as_bytes(), &ZenCompiler)
                .err()
                .map(|e| e.code),
            Some("BAD_SCHEMA".into())
        );
    }
    let mut p = base();
    p["future"] = json!(true);
    assert_eq!(error(&p).code, "BAD_SCHEMA");
    p.as_object_mut()
        .unwrap_or_else(|| panic!("expected fixture object"))
        .remove("future");
    p["questions"][0]["type"] = json!("future");
    assert_eq!(error(&p).code, "BAD_SCHEMA");
}

#[test]
fn ids_references_currency_and_static_output_errors() {
    let mut p = base();
    p["questions"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .push(question("q"));
    assert_eq!(error(&p).path, "/questions/1/id");
    let mut p = base();
    p["questions"][0]["id"] = json!("END");
    assert_eq!(error(&p).path, "/questions/0/id");
    for currency in ["XXX", "XTS", "ZZZ", "XAU"] {
        let mut p = base();
        p["pricing"]["currency"] = json!(currency);
        assert_eq!(error(&p).path, "/pricing/currency");
    }
    let mut p = base();
    p["pricing"]["components"] = json!([
        {"id":"x","category":"base","amountMinor":1},
        {"id":"x","category":"adjustment","amountMinor":2}]);
    assert_eq!(error(&p).path, "/pricing/components/1/id");
    let mut p = base();
    p["pricing"]["components"] =
        json!([{ "id":"x","category":"base","unitMinor":1,"quantityFrom":"absent" }]);
    assert_eq!(error(&p).path, "/pricing/components/0/quantityFrom");
    p["pricing"]["components"][0]["quantityFrom"] = json!("q");
    good(&p);
    p["questions"][0]["type"] = json!("color");
    assert_eq!(error(&p).path, "/pricing/components/0/quantityFrom");
}

#[test]
fn numeric_domains_intersect_precision_and_step_exactly() {
    for (min, max, step, accepted) in [
        (1, 0, 1, false),
        (0, 1, 0, false),
        (0, 1, -1, false),
        (-3, -1, 2, true),
    ] {
        let mut p = base();
        p["questions"][0]["min"] = json!(min);
        p["questions"][0]["max"] = json!(max);
        p["questions"][0]["step"] = json!(step);
        assert_eq!(compile(&p).is_ok(), accepted);
    }
    for (min, max, step, accepted) in [
        ("0.001", "0.009", "0.001", false),
        ("0.001", "0.020", "0.003", true),
        ("0.001", "0.020", "0.002", false),
        ("-0.021", "-0.009", "0.003", false),
        ("-0.022", "-0.009", "0.003", true),
        ("0", "1", "0", false),
        ("2", "1", "0.1", false),
    ] {
        let mut p = base();
        p["questions"][0] = json!({"id":"q","type":"decimal","label":"Q","scale":2,"min":min,"max":max,"step":step});
        assert_eq!(compile(&p).is_ok(), accepted, "{min}..{max} step {step}");
    }
}

fn visual_product() -> Value {
    let mut p = base();
    p["visual"] = json!({"targets":[{"id":"t","defaults":{"visible":false}}],"slots":[
        {"id":"s","target":"t","property":"opacity","cases":[{"when":{"field":"q","op":"exists"},"valueFrom":"q"}]}]});
    p
}
#[test]
fn visual_type_ranges_unique_writers_and_runtime_conflict_boundary() {
    let mut p = visual_product();
    good(&p);
    p["visual"]["slots"][0]["property"] = json!("color");
    assert_eq!(error(&p).path, "/visual/slots/0/cases/0/valueFrom");
    let mut p = visual_product();
    p["visual"]["slots"][0]["target"] = json!("missing");
    assert_eq!(error(&p).path, "/visual/slots/0/target");
    let mut p = visual_product();
    let mut slot = p["visual"]["slots"][0].clone();
    slot["id"] = json!("s2");
    p["visual"]["slots"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .push(slot);
    assert_eq!(error(&p).path, "/visual/slots/1/property");
    for value in [json!(true), json!(1.1), json!(-0.1), json!("0.5")] {
        let mut p = visual_product();
        p["visual"]["slots"][0]["cases"][0] =
            json!({"when":{"field":"q","op":"exists"},"value":value});
        assert!(compile(&p).is_err());
    }
    let mut p = visual_product();
    let case = p["visual"]["slots"][0]["cases"][0].clone();
    p["visual"]["slots"][0]["cases"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .push(case);
    p["pricing"]["components"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .push(json!({"id":"second","category":"base","amountMinor":5}));
    let model = good(&p); // simultaneous cases/bases belong to runtime, never first-match compilation
    assert_eq!(model.sources().len(), 2);
    assert_eq!(model.sources()[0].rule_id(), "s");
    assert_eq!(model.sources()[1].rule_id(), "s");
    assert_ne!(model.sources()[0].path(), model.sources()[1].path());
}

#[test]
fn compile_limits_precede_typed_ast_and_backend() {
    let mut c = json!({"field":"q","op":"exists"});
    for _ in 1..32 {
        c = json!({"not":c});
    }
    good(&condition(c.clone()));
    let e = error(&condition(json!({"not":c})));
    assert_eq!(e.code, "RESOURCE_LIMIT");
    assert_eq!(e.details["limit"], "astDepth");
    let e = error(&condition(
        json!({"field":"q","op":"in","value":vec![0;257]}),
    ));
    assert_eq!(e.details["limit"], "inMembers");
    let mut p = base();
    p["questions"] = json!(vec![question("q"); 257]);
    assert_eq!(error(&p).details["limit"], "questions");
    let mut p = base();
    p["flow"]["edges"] = json!(vec![json!({"from":"q","to":"END"}); 2049]);
    assert_eq!(error(&p).details["limit"], "flowEdges");
    let leaf = json!({"field":"q","op":"exists"});
    let e = error(&condition(json!({"all":vec![leaf;32768]})));
    assert_eq!(e.details["limit"], "astNodes");
}

#[test]
fn aggregate_definition_and_input_limits_are_not_schema_fallbacks() {
    let e = compile_product(&vec![b' '; 4 * 1024 * 1024 + 1], &ZenCompiler)
        .err()
        .unwrap_or_else(|| panic!("oversized bytes accepted"));
    assert_eq!(e.details["limit"], "inputBytes");
    for (pointer, maximum, name) in [
        ("/pricing/components", 1024, "priceComponents"),
        ("/visual/targets", 256, "visualTargets"),
        ("/visual/slots", 2048, "visualSlots"),
    ] {
        let mut p = base();
        *p.pointer_mut(pointer)
            .unwrap_or_else(|| panic!("fixture path")) = json!(vec![json!({}); maximum + 1]);
        assert_eq!(error(&p).details["limit"], name);
    }
    let mut p = base();
    p["questions"][0]["options"] = json!(vec![json!({}); 257]);
    assert_eq!(error(&p).details["limit"], "optionsPerQuestion");
    let mut q = question("q");
    q["options"] = json!(vec![json!({}); 256]);
    p["questions"] = json!(vec![q; 33]);
    assert_eq!(error(&p).details["limit"], "optionsTotal");
    let mut p = base();
    p["visual"]["slots"] = json!([{"cases":vec![json!({});4097]}]);
    assert_eq!(error(&p).details["limit"], "visualCases");
    let mut p = base();
    let q = json!({"options":vec![json!({"enabledWhen":{"field":"q","op":"exists"}});256]});
    p["questions"] = json!(vec![q; 17]);
    assert_eq!(error(&p).details["limit"], "conditionRoots");
}

#[test]
fn backend_failure_is_propagated_without_a_partial_model() {
    struct Reject;
    impl knot_rules::RuleCompiler for Reject {
        fn compile(
            &self,
            _: &[knot_rules::FieldKind],
            _: &[knot_rules::Condition],
            _: &knot_rules::SourceMap,
            _: &mut knot_rules::CompileBudget,
        ) -> Result<std::sync::Arc<dyn knot_rules::RuleProgram>, knot_schema::KnotError> {
            Err(knot_schema::KnotError::new(
                "RULE_COMPILE",
                "/pricing/components/0/when",
            ))
        }
    }
    let bytes = base().to_string();
    assert_eq!(
        compile_product(bytes.as_bytes(), &Reject)
            .err()
            .map(|e| e.code),
        Some("RULE_COMPILE".into())
    );
    assert_eq!(
        compile_product(b"", &Reject).err().map(|e| e.code),
        Some("BAD_JSON".into())
    );
}

#[test]
fn golden_compile_success_and_rejections_are_unchanged() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden");
    for (case, code) in [
        ("T01", "BAD_JSON"),
        ("T16", "INVALID_MODEL"),
        ("T17", "INVALID_MODEL"),
        ("T27", "INVALID_UTF8"),
        ("T28", "BAD_JSON"),
    ] {
        let bytes =
            std::fs::read(root.join(case).join("model.json")).unwrap_or_else(|e| panic!("{e}"));
        let actual = compile_product(&bytes, &ZenCompiler)
            .err()
            .unwrap_or_else(|| panic!("{case} accepted"));
        assert_eq!(actual.code, code, "{case}");
        let expected: Value = serde_json::from_slice(
            &std::fs::read(root.join(case).join("expected-result.json"))
                .unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(actual.envelope(), expected, "{case}");
    }
    let bytes = std::fs::read(root.join("T02/model.json")).unwrap_or_else(|e| panic!("{e}"));
    let model = compile_product(&bytes, &ZenCompiler).unwrap_or_else(|e| panic!("{e}"));
    let expected: Value = serde_json::from_slice(
        &std::fs::read(root.join("T02/expected-result.json")).unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(model.digest(), expected["modelDigest"]);
    assert!(model.diagnostics().is_empty());
}

#[test]
fn every_golden_marked_valid_compiles_without_changing_the_corpus() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden");
    let mut cases: Vec<_> = std::fs::read_dir(root)
        .unwrap_or_else(|e| panic!("{e}"))
        .map(|entry| entry.unwrap_or_else(|e| panic!("{e}")).path())
        .filter(|p| p.join("case.json").is_file())
        .collect();
    cases.sort();
    let mut valid = 0;
    for path in cases {
        let descriptor: Value = serde_json::from_slice(
            &std::fs::read(path.join("case.json")).unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        if descriptor["modelValidation"] != "valid" {
            continue;
        }
        let bytes = std::fs::read(path.join("model.json")).unwrap_or_else(|e| panic!("{e}"));
        compile_product(&bytes, &ZenCompiler)
            .unwrap_or_else(|e| panic!("{}: {e}", descriptor["id"]));
        valid += 1;
    }
    assert_eq!(valid, 37, "frozen valid Golden inventory changed");
}
