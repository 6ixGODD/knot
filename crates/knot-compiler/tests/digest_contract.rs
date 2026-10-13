mod common;
use common::*;
use serde_json::json;

#[test]
fn explicit_defaults_decimal_color_and_integer_spelling_normalize() {
    let p = base();
    let digest = good(&p).digest().to_owned();
    let mut p = p;
    p["questions"][0]["required"] = json!(true);
    assert_eq!(good(&p).digest(), digest);
    let mut p = base();
    p["questions"][0] =
        json!({"id":"q","label":"Q","type":"decimal","scale":2,"min":"000.1000","max":"1.00"});
    p["pricing"]["components"][0]["when"] = json!({"field":"q","op":"eq","value":"00.100"});
    p["visual"]["targets"] = json!([{"id":"t","defaults":{"color":"#aAbBcC","opacity":1.0}}]);
    let m = good(&p);
    let normalized = serde_json::to_value(m.product()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(normalized["questions"][0]["min"], "0.1");
    assert_eq!(
        normalized["pricing"]["components"][0]["when"]["value"],
        "0.1"
    );
    assert_eq!(
        normalized["visual"]["targets"][0]["defaults"]["color"],
        "#AABBCC"
    );
    assert_eq!(normalized["visual"]["targets"][0]["order"], 0);
    assert_eq!(good(&normalized).digest(), m.digest());
}
#[test]
fn object_key_order_including_utf16_and_jcs_numbers_is_irrelevant() {
    let mut p = base();
    p["metadata"] = json!({"\u{e000}":1,"\u{10000}":2,"numbers":[1e-7, 1e21, -0.0]});
    let digest = good(&p).digest().to_owned();
    // Independently hashed explicit RFC8785 bytes: supplementary key precedes
    // U+E000 in UTF-16 order; numbers are 1e-7,1e+21,0 (including negative zero).
    assert_eq!(
        digest,
        "sha256:c5b5a103e6bd8d76b88a8d23853d045b49a12b16b3e5ce41d20b2b464c71e213"
    );
    let alternate =
        r#"{"numbers":[0.0000001,1000000000000000000000,0],"\ud800\udc00":2,"\ue000":1}"#;
    p["metadata"] = serde_json::from_str(alternate).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(good(&p).digest(), digest);
}
#[test]
fn arrays_labels_metadata_and_target_order_remain_semantic() {
    let mut p = base();
    p["questions"][0] = json!({"id":"q","label":"Q","type":"single","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]});
    p["visual"]["targets"] = json!([{"id":"z","order":0,"defaults":{}},{"id":"a","order":0,"defaults":{}},{"id":"first","order":-1,"defaults":{}}]);
    let m = good(&p);
    assert_eq!(m.target_order(), [2, 1, 0]);
    for mutation in 0..4 {
        let mut changed = p.clone();
        match mutation {
            0 => {
                changed["questions"][0]["options"]
                    .as_array_mut()
                    .unwrap_or_else(|| panic!("expected fixture array"))
                    .swap(0, 1);
            }
            1 => changed["questions"][0]["label"] = json!("different"),
            2 => changed["metadata"] = json!({"label":"different"}),
            _ => changed["visual"]["targets"][0]["order"] = json!(5),
        }
        assert_ne!(good(&changed).digest(), m.digest());
    }
}
