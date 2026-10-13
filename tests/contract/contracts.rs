use knot_schema::{Decimal, exact_integer, parse_json};

#[test]
fn raw_number_boundaries_do_not_round_through_binary64() {
    assert_eq!(exact_integer("1.0"), Ok(1));
    assert_eq!(exact_integer("10e-1"), Ok(1));
    assert_eq!(exact_integer("-9007199254740991"), Ok(-9007199254740991));
    for token in ["9007199254740991.1", "9007199254740992", "0.1", "1e308"] {
        assert!(exact_integer(token).is_err(), "{token}");
    }
    let input = b"{\"value\":9007199254740991.1}";
    let parsed = parse_json(input, 1024).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(parsed["value"].to_string(), "9007199254740991.1");
}

#[test]
fn hostile_json_is_rejected_before_unbounded_construction() {
    for input in [
        br#"{"a":1,"\u0061":2}"#.as_slice(),
        br#""\uD800""#,
        br#"[1,]"#,
        br#"{}{}"#,
        br#"01"#,
        &[255],
    ] {
        assert!(parse_json(input, 1024).is_err());
    }
    assert!(
        parse_json(
            format!("{}0{}", "[".repeat(65), "]".repeat(65)).as_bytes(),
            1024
        )
        .is_err()
    );
    assert!(
        parse_json(
            format!("\"{}\"", "\\u0061".repeat(16385)).as_bytes(),
            200000
        )
        .is_err()
    );
    assert!(parse_json(b"1e309", 1024).is_err());
    assert!(parse_json(b"{}", 1).is_err());
    assert!(parse_json(br#""\ud83d\ude00""#, 1024).is_ok());
}

#[test]
fn decimal_precision_normalization_and_rounding() {
    let parse = |s| Decimal::parse(s).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(parse("00012.5000").to_string(), "12.5");
    assert_eq!(parse("-0.000").to_string(), "0");
    assert_eq!(parse("0.335").multiply_minor(100), Ok(34));
    assert_eq!(parse("0.335").multiply_minor(-100), Ok(-34));
    assert!(parse("1.25").fits_step(parse("0.25"), parse("0.5")));
    assert!(!parse("1.2").fits_step(parse("0.25"), parse("0.5")));
    assert!(parse("10") > parse("2"));
    for s in [
        "1e3",
        "+1",
        "1.",
        ".1",
        "1.0000000001",
        "9007199254740991.1",
        " ",
    ] {
        assert!(Decimal::parse(s).is_err());
    }
}

#[test]
fn offline_schemas_preserve_exact_numbers_and_reject_extensions() {
    use knot_schema::validation::Validator;
    use serde_json::json;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
    for name in ["product", "state", "event", "result", "visual", "error"] {
        let bytes = std::fs::read(root.join(format!("schemas/{name}.schema.json")))
            .unwrap_or_else(|e| panic!("{e}"));
        let schema = parse_json(&bytes, 4 * 1024 * 1024).unwrap_or_else(|e| panic!("{e}"));
        let validator = Validator::new(&schema).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(validator.validate(&json!({"unexpected":true})).is_err());
        if name == "product" {
            let bytes =
                std::fs::read(root.join("examples/speaker.json")).unwrap_or_else(|e| panic!("{e}"));
            let mut value = parse_json(&bytes, 4 * 1024 * 1024).unwrap_or_else(|e| panic!("{e}"));
            assert!(validator.validate(&value).is_ok());
            value["unexpected"] = json!(true);
            assert!(validator.validate(&value).is_err());
        }
    }
    let validator = Validator::new(&json!({"type":"integer","maximum":9007199254740991i64}))
        .unwrap_or_else(|e| panic!("{e}"));
    for token in ["9007199254740991.1", "9007199254740992"] {
        let value = parse_json(token.as_bytes(), 128).unwrap_or_else(|e| panic!("{e}"));
        assert!(validator.validate(&value).is_err(), "{token}");
    }
    assert!(Validator::new(&json!({"$ref":"https://invalid.example/schema"})).is_err());
    assert!(Validator::new(&json!({"$ref":"file:///not-a-schema"})).is_err());
}

#[test]
fn jcs_official_number_and_utf16_vectors() {
    // RFC 8785 sections 3.2.2 and 3.2.3. Raw number tokens remain exact until JCS.
    let input = br#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001]}"#;
    let value = parse_json(input, 1024).unwrap_or_else(|e| panic!("{e}"));
    let actual = serde_jcs::to_string(&value).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        actual,
        r#"{"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27]}"#
    );
    let value = serde_json::json!({"\u{20ac}":1,"\r":2,"\u{fb33}":3,"1":4,"\u{1f600}":5,"\u{80}":6,"\u{f6}":7});
    assert_eq!(
        serde_jcs::to_string(&value).unwrap_or_else(|e| panic!("{e}")),
        "{\"\\r\":2,\"1\":4,\"\u{80}\":6,\"ö\":7,\"€\":1,\"😀\":5,\"דּ\":3}"
    );
}

#[test]
fn wire_product_and_events_roundtrip_against_schemas() {
    use knot_schema::{
        validation::Validator,
        wire::{Answer, ConfigurationEvent, Id, Integer, Product},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
    let load = |p: &str| {
        let bytes = std::fs::read(root.join(p)).unwrap_or_else(|e| panic!("{e}"));
        parse_json(&bytes, 4 * 1024 * 1024).unwrap_or_else(|e| panic!("{e}"))
    };
    let value = load("examples/speaker.json");
    let typed: Product = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        serde_json::to_value(typed).unwrap_or_else(|e| panic!("{e}")),
        value
    );
    let schema =
        Validator::new(&load("schemas/event.schema.json")).unwrap_or_else(|e| panic!("{e}"));
    let events = load("examples/speaker-events.json");
    for event in events.as_array().unwrap_or_else(|| panic!("events array")) {
        let typed: ConfigurationEvent =
            serde_json::from_value(event.clone()).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            schema
                .validate(&serde_json::to_value(typed).unwrap_or_else(|e| panic!("{e}")))
                .is_ok()
        );
    }
    for token in ["1.0", "10e-1", "9007199254740991"] {
        let value = parse_json(token.as_bytes(), 128).unwrap_or_else(|e| panic!("{e}"));
        assert!(serde_json::from_value::<Answer>(value).is_ok(), "{token}");
    }
    assert!(
        serde_json::from_value::<Integer>(
            parse_json(b"9007199254740991.1", 128).unwrap_or_else(|e| panic!("{e}"))
        )
        .is_err()
    );
    assert!(Id::new("é".repeat(65)).is_err());
    assert!(Id::new("a\u{85}".into()).is_err());
}

#[test]
fn schema_formats_assert_utf8_id_and_exact_decimal_limits() {
    use knot_schema::validation::Validator;
    let schema = serde_json::json!({"type":"string","format":"knot-id"});
    let validator = Validator::new(&schema).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        validator
            .validate(&serde_json::json!("é".repeat(64)))
            .is_ok()
    );
    assert!(
        validator
            .validate(&serde_json::json!("é".repeat(65)))
            .is_err()
    );
    let schema = serde_json::json!({"type":"string","format":"knot-decimal"});
    let validator = Validator::new(&schema).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        validator
            .validate(&serde_json::json!("9007199254740991.000"))
            .is_ok()
    );
    assert!(
        validator
            .validate(&serde_json::json!("9007199254740991.001"))
            .is_err()
    );
}

#[test]
fn visual_numbers_accept_json_exponents_through_typed_roundtrip() {
    use knot_schema::{
        validation::Validator,
        wire::{Product, VisualValue, decode},
    };
    use serde_json::json;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
    let load = |path: &str| {
        parse_json(
            &std::fs::read(root.join(path)).unwrap_or_else(|e| panic!("{e}")),
            4 * 1024 * 1024,
        )
        .unwrap_or_else(|e| panic!("{e}"))
    };
    let validator =
        Validator::new(&load("schemas/product.schema.json")).unwrap_or_else(|e| panic!("{e}"));
    for token in ["1e-3", "0.1234567890123456789", "0.5"] {
        let value = parse_json(token.as_bytes(), 128).unwrap_or_else(|e| panic!("{e}"));
        let mut model = load("examples/speaker.json");
        model["visual"]["targets"][0]["defaults"]["opacity"] = value.clone();
        model["visual"]["targets"][0]["defaults"]["transform.position"] = json!([value, 0, 1]);
        // Exercise both direct properties and a nested untagged VisualCase.
        model["visual"]["slots"][0]["property"] = json!("opacity");
        model["visual"]["slots"][0]["cases"] = json!([{
            "when": {"field":"speaker_type","op":"exists"},
            "value": parse_json(token.as_bytes(), 128).unwrap_or_else(|e| panic!("{e}"))
        }]);
        let bytes = serde_json::to_vec(&model).unwrap_or_else(|e| panic!("{e}"));
        let typed: Product =
            decode(&bytes, 4 * 1024 * 1024, &validator).unwrap_or_else(|e| panic!("{token}: {e}"));
        let canonical = serde_json::to_value(&typed).unwrap_or_else(|e| panic!("{e}"));
        assert!(validator.validate(&canonical).is_ok());
        let decoded: Product = serde_json::from_value(canonical).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(typed, decoded);
        let expected = token.parse::<f64>().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            typed.visual.targets[0].defaults["opacity"],
            VisualValue::Number(expected)
        );
    }
    for token in ["null", "{}", "[1,2]", "[1,true,3]", "1e999"] {
        let value: serde_json::Value =
            serde_json::from_str(token).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            serde_json::from_value::<VisualValue>(value).is_err(),
            "{token}"
        );
    }
}

#[test]
fn fixed_currency_snapshot_and_golden_contracts() {
    use knot_schema::{canonical::model_digest, currency, validation::Validator, wire::*};
    use serde_json::Value;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
    let load = |path: &std::path::Path| -> Value {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{e}"));
        parse_json(&bytes, 4 * 1024 * 1024).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    let source = load(&root.join("currency/provenance.json"));
    assert_eq!(source["tableDigest"], currency::table_digest());
    for (code, units) in [("CNY", 2), ("JPY", 0), ("KWD", 3), ("CLF", 4)] {
        assert_eq!(currency::minor_units(code), Ok(units));
    }
    for code in ["XXX", "XTS", "XAU", "ZZZ", "cny", "", "USD "] {
        assert!(currency::minor_units(code).is_err(), "{code}");
    }
    let index = load(&root.join("golden/index.json"));
    assert_eq!(index["currencyTableDigest"], currency::table_digest());
    let cases = index["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("case array"));
    for number in 1..=28 {
        let id = format!("T{number:02}");
        assert_eq!(cases.iter().filter(|c| c["id"] == id).count(), 1, "{id}");
    }
    let validator = |name: &str| {
        Validator::new(&load(&root.join(format!("schemas/{name}.schema.json"))))
            .unwrap_or_else(|e| panic!("{e}"))
    };
    let product_schema = validator("product");
    let state_schema = validator("state");
    let event_schema = validator("event");
    let result_schema = validator("result");
    let error_schema = validator("error");
    for meta in cases {
        let id = meta["id"].as_str().unwrap_or_else(|| panic!("id"));
        let directory = root.join("golden").join(id);
        assert_eq!(load(&directory.join("case.json")), *meta);
        let bytes = std::fs::read(directory.join("model.json")).unwrap_or_else(|e| panic!("{e}"));
        let parsed = parse_json(&bytes, 4 * 1024 * 1024);
        let mut front_end_error = None;
        let digest = if meta["modelValidation"] == "parse-error" {
            assert!(parsed.is_err(), "{id}");
            front_end_error = parsed.err();
            None
        } else {
            let model = parsed.unwrap_or_else(|e| panic!("{id}: {e}"));
            if meta["modelValidation"] == "schema-error" {
                assert!(product_schema.validate(&model).is_err(), "{id}");
                front_end_error = product_schema.validate(&model).err();
                None
            } else {
                product_schema
                    .validate(&model)
                    .unwrap_or_else(|e| panic!("{id}: {e}"));
                let mut normalized = model.clone();
                for q in normalized["questions"]
                    .as_array_mut()
                    .unwrap_or_else(|| panic!("questions"))
                {
                    if q.get("required").is_none() {
                        q["required"] = true.into();
                    }
                }
                for target in normalized["visual"]["targets"]
                    .as_array_mut()
                    .unwrap_or_else(|| panic!("targets"))
                {
                    if target.get("order").is_none() {
                        target["order"] = 0.into();
                    }
                }
                let typed: Product =
                    serde_json::from_value(normalized).unwrap_or_else(|e| panic!("{id}: {e}"));
                Some(
                    model_digest(&typed, &currency::table_digest())
                        .unwrap_or_else(|e| panic!("{id}: {e}")),
                )
            }
        };
        let state = load(&directory.join("initial-state.json"));
        if !state.is_null() {
            state_schema
                .validate(&state)
                .unwrap_or_else(|e| panic!("{id} state: {e}"));
            let typed: ConfigurationState =
                serde_json::from_value(state.clone()).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(
                serde_json::to_value(typed).unwrap_or_else(|e| panic!("{e}")),
                state
            );
            if meta["stateValidation"] != "semantic-error" {
                assert_eq!(state["modelDigest"].as_str(), digest.as_deref(), "{id}");
            }
        }
        let event = load(&directory.join("event.json"));
        if !event.is_null() {
            if meta["eventValidation"] == "schema-error" {
                assert!(event_schema.validate(&event).is_err(), "{id}");
                front_end_error = event_schema.validate(&event).err();
            } else {
                event_schema
                    .validate(&event)
                    .unwrap_or_else(|e| panic!("{id} event: {e}"));
                let _: ConfigurationEvent =
                    serde_json::from_value(event).unwrap_or_else(|e| panic!("{id}: {e}"));
            }
        }
        let expected = load(&directory.join("expected-result.json"));
        if meta["expectedStatus"] != 0 {
            error_schema
                .validate(&expected)
                .unwrap_or_else(|e| panic!("{id} error: {e}"));
            let error: knot_schema::KnotError =
                serde_json::from_value(expected["error"].clone()).unwrap_or_else(|e| panic!("{e}"));
            if let Some(actual) = front_end_error {
                assert_eq!(
                    (actual.code, actual.path),
                    (error.code.clone(), error.path.clone()),
                    "{id}"
                );
            }
            assert_eq!(
                error.status() as i64,
                meta["expectedStatus"].as_i64().unwrap_or_default(),
                "{id}"
            );
        } else {
            result_schema
                .validate(&expected)
                .unwrap_or_else(|e| panic!("{id} result: {e}"));
            if meta["operation"] == "model_create" {
                let typed: ModelCreateResult = serde_json::from_value(expected.clone())
                    .unwrap_or_else(|e| panic!("{id}: {e}"));
                assert_eq!(Some(typed.model_digest.as_str()), digest.as_deref(), "{id}");
            } else {
                let typed: TransitionResult = serde_json::from_value(expected.clone())
                    .unwrap_or_else(|e| panic!("{id}: {e}"));
                assert_eq!(
                    Some(typed.state.model_digest.as_str()),
                    digest.as_deref(),
                    "{id}"
                );
                assert_eq!(
                    serde_json::to_value(typed).unwrap_or_else(|e| panic!("{e}")),
                    expected,
                    "{id}"
                );
                let p = &expected["snapshot"]["price"];
                if p["status"] != "unknown" {
                    let sum: i64 = p["components"]
                        .as_array()
                        .unwrap_or_else(|| panic!("components"))
                        .iter()
                        .map(|c| {
                            c["amountMinor"]
                                .as_i64()
                                .unwrap_or_else(|| panic!("amount"))
                        })
                        .sum();
                    assert_eq!(Some(sum), p["totalMinor"].as_i64(), "{id}");
                }
            }
        }
    }
}

#[test]
fn schema_shape_matrix_rejects_partial_results_and_mixed_variants() {
    use knot_schema::{validation::Validator, wire::Product};
    use serde_json::{Value, json};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
    let load = |path: &str| -> Value {
        let bytes = std::fs::read(root.join(path)).unwrap_or_else(|e| panic!("{e}"));
        parse_json(&bytes, 4 * 1024 * 1024).unwrap_or_else(|e| panic!("{e}"))
    };
    let schema = |name: &str| {
        Validator::new(&load(&format!("schemas/{name}.schema.json")))
            .unwrap_or_else(|e| panic!("{e}"))
    };
    let product = schema("product");
    let base = load("golden/T07/model.json");
    for op in [
        "eq", "ne", "gt", "gte", "lt", "lte", "between", "in", "exists",
    ] {
        let mut leaf = json!({"field":"n","op":op});
        match op {
            "exists" => {}
            "between" | "in" => leaf["value"] = json!([0, 10]),
            _ => leaf["value"] = json!(2),
        }
        let mut input = base.clone();
        input["flow"]["edges"][0]["when"] = json!({"all":[{"any":[leaf.clone()]},{"not":leaf}]});
        assert!(product.validate(&input).is_ok(), "{op}");
        assert!(serde_json::from_value::<Product>(input).is_ok(), "{op}");
    }
    for bad in [
        json!({"all":[]}),
        json!({"any":[]}),
        json!({"field":"n","op":"exists","value":1}),
        json!({"field":"n","op":"between","value":[1]}),
        json!({"field":"n","op":"in","value":[]}),
        json!({"all":[{"field":"n","op":"exists"}],"not":{"field":"n","op":"exists"}}),
    ] {
        let mut input = base.clone();
        input["flow"]["edges"][0]["when"] = bad;
        assert!(product.validate(&input).is_err());
    }
    let mut mixed = base.clone();
    mixed["pricing"]["components"][0]["unitMinor"] = json!(1);
    mixed["pricing"]["components"][0]["quantityFrom"] = json!("n");
    assert!(product.validate(&mixed).is_err());
    let state = schema("state");
    for bad in [json!(null), json!(true), json!(["a", "a"]), json!(0.5)] {
        let mut input = load("golden/T04/initial-state.json");
        input["answers"]["q"] = bad;
        assert!(state.validate(&input).is_err());
    }
    let event = schema("event");
    for kind in ["clear_answer", "skip_answer"] {
        let mut input =
            json!({"schemaVersion":"1.0","type":kind,"questionId":"q","expectedRevision":0});
        assert!(event.validate(&input).is_ok());
        input["value"] = json!("unexpected");
        assert!(event.validate(&input).is_err());
    }
    let result = schema("result");
    let full = load("golden/T12/expected-result.json");
    for key in ["state", "snapshot", "changes", "diagnostics"] {
        let mut partial = full.clone();
        partial
            .as_object_mut()
            .unwrap_or_else(|| panic!("object"))
            .remove(key);
        assert!(result.validate(&partial).is_err(), "missing {key}");
    }
    let visual = schema("visual");
    for (property, value) in [
        ("visible", json!(true)),
        ("color", json!("#123ABC")),
        ("opacity", json!(0.5)),
        ("transform.scale", json!([-1, 0, 1])),
        ("assetRef", json!("mesh")),
    ] {
        let mut input = json!({"targets":[{"id":"t","order":0,"properties":{}}]});
        input["targets"][0]["properties"][property] = value;
        assert!(visual.validate(&input).is_ok(), "{property}");
        input["targets"][0]["properties"][property] = json!(null);
        assert!(visual.validate(&input).is_err(), "{property}");
    }
    for (property, value) in [
        ("unknown", json!(1)),
        ("opacity", json!(1.01)),
        ("transform.scale", json!([1, 2])),
        ("color", json!("#fff")),
    ] {
        let mut input = json!({"targets":[{"id":"t","order":0,"properties":{}}]});
        input["targets"][0]["properties"][property] = value;
        assert!(visual.validate(&input).is_err(), "{property}");
    }
    let error = schema("error");
    for record in load("errors.json")["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("errors"))
    {
        let code = record["code"].as_str().unwrap_or_else(|| panic!("code"));
        let mut actual = knot_schema::KnotError::new(code, "");
        if code == "RESOURCE_LIMIT" {
            actual = knot_schema::KnotError::limit("questions", 256, 257);
        }
        if code == "REVISION_CONFLICT" {
            *actual.details = json!({"expectedRevision":0,"actualRevision":1});
        }
        assert_eq!(json!(actual.status()), record["status"]);
        let mut envelope = actual.envelope();
        assert!(error.validate(&envelope).is_ok(), "{code}");
        envelope["error"]["details"]["unregistered"] = json!(true);
        assert!(error.validate(&envelope).is_err(), "{code}");
    }
}
