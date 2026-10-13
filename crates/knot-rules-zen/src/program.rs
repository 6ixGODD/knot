//! Minimal P0 graph builder used to verify the pinned engine before IR lowering.
use knot_schema::KnotError;
use serde_json::json;
use zen_engine::{Decision, model::GraphContent};

/// Compile one internal expression with ZEN's fallible compiler before caching the graph.
/// This is a P0 probe utility, not a product DSL input API.
pub fn compile_probe(expression: &str) -> Result<Decision, KnotError> {
    if expression.len() > 65536 {
        return Err(KnotError::limit("expressionBytes", 65536, expression.len()));
    }
    let mut isolate = zen_expression::Isolate::new();
    isolate
        .compile_standard(expression)
        .map_err(|_| KnotError::new("RULE_COMPILE", ""))?;
    let content:GraphContent=serde_json::from_value(json!({
        "nodes":[
            {"id":"input","name":"input","type":"inputNode","content":{}},
            {"id":"expression","name":"expression","type":"expressionNode","content":{"expressions":[{"id":"result","key":"result","value":expression}]}},
            {"id":"output","name":"output","type":"outputNode","content":{}}
        ],
        "edges":[{"id":"a","sourceId":"input","targetId":"expression"},{"id":"b","sourceId":"expression","targetId":"output"}]
    })).map_err(|_|KnotError::new("RULE_COMPILE",""))?;
    let mut decision = Decision::from(content);
    decision
        .validate()
        .map_err(|_| KnotError::new("RULE_COMPILE", ""))?;
    decision.compile();
    Ok(decision)
}
