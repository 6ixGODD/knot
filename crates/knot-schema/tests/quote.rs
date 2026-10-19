#![allow(clippy::unwrap_used)]
use knot_schema::protocol;
use serde_json::json;
fn request() -> serde_json::Value {
    json!({"schemaVersion":"1.0","state":{"schemaVersion":"1.0","productId":"test","modelDigest":format!("sha256:{}","0".repeat(64)),"revision":0,"answers":{},"skipped":[]}})
}
#[test]
fn independent_strict_request_admission() {
    let base = request();
    assert!(protocol::quote(&serde_json::to_vec(&base).unwrap()).is_ok());
    for (key, val) in [
        ("unknown", json!(1)),
        ("focusQuestionId", json!(null)),
        ("candidateValue", json!(null)),
        ("candidateValue", json!("#000000")),
    ] {
        let mut q = base.clone();
        q[key] = val;
        assert!(
            protocol::quote(&serde_json::to_vec(&q).unwrap()).is_err(),
            "{key}"
        );
    }
    let mut q = base.clone();
    q["state"]["revision"] = json!(9007199254740992u64);
    assert!(protocol::quote(&serde_json::to_vec(&q).unwrap()).is_err());
    let bytes = serde_json::to_string(&base).unwrap().replacen(
        "\"revision\":0",
        "\"revision\":0,\"revision\":0",
        1,
    );
    assert_eq!(
        protocol::quote(bytes.as_bytes()).unwrap_err().code,
        "BAD_JSON"
    );
    let mut q = base.clone();
    q["state"]["answers"] = serde_json::Value::Object(
        (0..40)
            .map(|i| (format!("q{i}"), json!("x".repeat(14000))))
            .collect(),
    );
    assert_eq!(
        protocol::quote(&serde_json::to_vec(&q).unwrap())
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    let padded = serde_json::to_string(&base).unwrap().replace(
        "\"revision\":0",
        &format!("\"revision\":0{}", " ".repeat(524288)),
    );
    assert_eq!(
        protocol::quote(padded.as_bytes()).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    let bytes = serde_json::to_string(&base)
        .unwrap()
        .replace("\"revision\":0", "\"revision\":9007199254740991.1");
    assert!(protocol::quote(bytes.as_bytes()).is_err());
}
