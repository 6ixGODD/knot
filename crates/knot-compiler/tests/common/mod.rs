#![allow(dead_code)]
use knot_compiler::{CompiledModel, compile_product};
use knot_rules_zen::ZenCompiler;
use knot_schema::KnotError;
use serde_json::{Value, json};

pub fn base() -> Value {
    json!({"schemaVersion":"1.0","productId":"test","productVersion":"1",
        "questions":[{"id":"q","type":"integer","label":"Q"}],
        "flow":{"start":"q","edges":[{"from":"q","to":"END"}]},
        "pricing":{"currency":"CNY","components":[{"id":"base","category":"base","amountMinor":100}]},
        "visual":{"targets":[],"slots":[]}})
}
pub fn compile(v: &Value) -> Result<CompiledModel, KnotError> {
    let bytes = serde_json::to_vec(v).unwrap_or_else(|e| panic!("{e}"));
    compile_product(&bytes, &ZenCompiler)
}
pub fn good(v: &Value) -> CompiledModel {
    compile(v).unwrap_or_else(|e| panic!("{e}"))
}
pub fn error(v: &Value) -> KnotError {
    match compile(v) {
        Err(e) => e,
        Ok(_) => panic!("invalid model accepted: {v}"),
    }
}
pub fn question(id: &str) -> Value {
    json!({"id":id,"type":"integer","label":id})
}
pub fn condition(c: Value) -> Value {
    let mut p = base();
    p["pricing"]["components"][0]["when"] = c;
    p
}
