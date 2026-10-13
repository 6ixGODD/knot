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
