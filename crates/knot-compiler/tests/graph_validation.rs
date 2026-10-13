mod common;
use common::*;
use serde_json::{Value, json};

fn graph(edges: Value) -> Value {
    let mut p = base();
    p["questions"] = json!([
        question("q"),
        question("a"),
        question("b"),
        question("join")
    ]);
    p["flow"]["edges"] = edges;
    p
}
fn diamond() -> Value {
    graph(json!([
        {"from":"q","to":"a","when":{"field":"q","op":"eq","value":1}},
        {"from":"q","to":"b"},{"from":"a","to":"join"},
        {"from":"b","to":"join"},{"from":"join","to":"END"}]))
}
#[test]
fn structural_closure_includes_transitive_successors_in_declaration_order() {
    let p = diamond();
    let m = good(&p);
    assert_eq!(m.successors(), &[vec![1, 2, 3], vec![3], vec![3], vec![]]);
    assert_eq!(
        m.edges().iter().map(|e| e.from).collect::<Vec<_>>(),
        [0, 0, 1, 2, 3]
    );
    let mut p = p;
    p["flow"]["edges"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .swap(1, 2);
    good(&p); // grouping preserves relative order, not contiguous source placement
}
#[test]
fn cycles_missing_outgoing_and_start_references_fail() {
    let mut p = base();
    p["flow"]["edges"][0]["to"] = json!("q");
    assert_eq!(error(&p).path, "/flow");
    let mut p = diamond();
    p["flow"]["edges"][4]["to"] = json!("q");
    assert_eq!(error(&p).path, "/flow");
    let mut p = diamond();
    p["flow"]["edges"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .pop();
    assert_eq!(error(&p).path, "/questions/3");
    for key in ["from", "to"] {
        let mut p = base();
        p["flow"]["edges"][0][key] = json!("missing");
        assert_eq!(error(&p).path, format!("/flow/edges/0/{key}"));
    }
    let mut p = base();
    p["flow"]["start"] = json!("END");
    assert_eq!(error(&p).path, "/flow/start");
}
#[test]
fn single_conditional_and_exact_last_fallback() {
    let mut p = base();
    p["flow"]["edges"][0]["when"] = json!({"field":"q","op":"exists"});
    good(&p);
    let mut p = diamond();
    p["flow"]["edges"][0]
        .as_object_mut()
        .unwrap_or_else(|| panic!("expected fixture object"))
        .remove("when");
    assert_eq!(error(&p).path, "/flow/edges/0/when");
    let mut p = diamond();
    p["flow"]["edges"][1]["when"] = json!({"field":"q","op":"exists"});
    assert_eq!(error(&p).path, "/flow/edges/1/when");
}
#[test]
fn optional_islands_warn_required_islands_reject_even_if_acyclic() {
    let mut p = base();
    p["questions"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .push(question("island"));
    p["flow"]["edges"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("expected fixture array"))
        .push(json!({"from":"island","to":"END"}));
    assert_eq!(error(&p).path, "/questions/1");
    p["questions"][1]["required"] = json!(false);
    let m = good(&p);
    assert_eq!(m.diagnostics().len(), 1);
    assert_eq!(m.diagnostics()[0].path, "/questions/1");
    assert_eq!(m.diagnostics()[0].code, "UNREACHABLE_QUESTION");
}
#[test]
fn future_reads_reject_and_non_dominating_branches_are_legal() {
    let mut p = diamond();
    p["flow"]["edges"][0]["when"]["field"] = json!("join");
    let e = error(&p);
    assert_eq!(e.path, "/flow/edges/0/when/field");
    assert_eq!(e.rule_ids, ["rule:/flow/edges/0/when"]);
    let mut p = diamond();
    p["flow"]["edges"][2]["when"] = json!({"field":"b","op":"exists"});
    good(&p); // b does not dominate a and is not its successor
    p["questions"][1] = json!({"id":"a","type":"single","label":"A","options":[{"id":"yes","label":"Yes","enabledWhen":{"field":"b","op":"exists"}}]});
    good(&p);
    p["questions"][1]["options"][0]["enabledWhen"]["field"] = json!("a");
    assert_eq!(error(&p).path, "/questions/1/options/0/enabledWhen/field");
    p["questions"][1]["options"][0]["enabledWhen"]["field"] = json!("join");
    assert_eq!(error(&p).path, "/questions/1/options/0/enabledWhen/field");
}
#[test]
fn declaration_order_is_not_a_topological_dependency_test() {
    let mut p = base();
    p["questions"] = json!([question("last"), question("q"), question("middle")]);
    p["flow"]["edges"] = json!([
        {"from":"q","to":"middle"}, {"from":"middle","to":"last"},
        {"from":"last","to":"END","when":{"field":"q","op":"exists"}}]);
    let m = good(&p);
    assert_eq!(m.successors(), &[vec![], vec![0, 2], vec![0]]);
    p["flow"]["edges"][0]["when"] = json!({"field":"last","op":"exists"});
    assert_eq!(error(&p).path, "/flow/edges/0/when/field");
}

#[test]
fn maximum_question_graph_crosses_all_bitset_words() {
    let mut p = base();
    p["questions"] = Value::Array((0..256).map(|i| question(&format!("q{i}"))).collect());
    p["flow"]["start"] = json!("q0");
    p["flow"]["edges"] = Value::Array((0..256).map(|i| json!({"from":format!("q{i}"),"to":if i == 255 { "END".into() } else { format!("q{}",i+1) }})).collect());
    let m = good(&p);
    assert_eq!(m.successors()[0], (1..256).collect::<Vec<_>>());
    assert_eq!(m.successors()[254], [255]);
    assert!(m.successors()[255].is_empty());
    p["flow"]["edges"][0]["when"] = json!({"field":"q255","op":"exists"});
    assert_eq!(error(&p).path, "/flow/edges/0/when/field");
}
