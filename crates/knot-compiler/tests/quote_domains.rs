#![allow(clippy::unwrap_used)]
mod common;
use common::*;
use knot_schema::wire::Id;
use serde_json::json;
#[test]
fn every_color_condition_location_contributes_classes_without_digest_change() {
    let mut p = base();
    p["questions"] = json!([{"id":"q","label":"Color","type":"color"}]);
    p["pricing"]["components"].as_array_mut().unwrap().push(json!({"id":"discount","category":"adjustment","amountMinor":-1,"when":{"op":"in","field":"q","value":["#000000","#FF0000"]}}));
    p["flow"]["edges"] = json!([{"from":"q","to":"END","when":{"op":"ne","field":"q","value":"#000001"}},{"from":"q","to":"END"}]);
    let m = good(&p);
    let q = Id::new("q".into()).unwrap();
    assert_eq!(
        m.quote_colors(&q).unwrap(),
        ["#000000", "#000001", "#000002", "#FF0000"]
    );
    assert_eq!(m.digest(), good(&p).digest());
    let mut speaker: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../contracts/examples/speaker.json")).unwrap();
    let m = good(&speaker);
    assert_eq!(
        m.digest(),
        "sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541"
    );
    speaker["productVersion"] = json!("other");
    assert_ne!(m.digest(), good(&speaker).digest());
}

#[test]
fn proof_uses_all_ir_branches_and_distinct_condition_ids() {
    let mut p = base();
    p["questions"] =
        json!([{"id":"q","type":"color","label":"Q"},{"id":"r","type":"color","label":"R"}]);
    p["flow"] = json!({"start":"q","edges":[{"from":"q","to":"r"},{"from":"r","to":"END"}]});
    p["pricing"]["components"].as_array_mut().unwrap().push(json!({"id":"both","category":"adjustment","amountMinor":0,"when":{"any":[{"op":"exists","field":"q"},{"not":{"op":"exists","field":"r"}}]}}));
    p["visual"] = json!({"targets":[{"id":"t","order":0,"defaults":{}}],"slots":[{"id":"shared","target":"t","property":"visible","cases":[{"when":{"op":"exists","field":"q"},"value":true},{"when":{"op":"exists","field":"r"},"value":false}]}]});
    let m = good(&p);
    let both = m.condition_id("/pricing/components/1/when").unwrap();
    assert_eq!(m.quote_dependencies(both), Some([0, 1].as_slice()));
    let a = m.condition_id("/visual/slots/0/cases/0/when").unwrap();
    let b = m.condition_id("/visual/slots/0/cases/1/when").unwrap();
    assert_ne!(a, b);
    assert_eq!(m.sources()[a.0].rule_id(), m.sources()[b.0].rule_id());
    assert_eq!(m.quote_dependencies(a), Some([0].as_slice()));
    assert_eq!(m.quote_dependencies(b), Some([1].as_slice()));
}
