use knot_rules::{CompileBudget, Condition, Field, FieldKind, RuleCompiler, RuleSource};
use knot_rules_zen::ZenCompiler;
#[test]
fn invalid_ir_and_expression_budgets_retain_original_source() {
    let path = "/pricing/components/7/when";
    let source = RuleSource::new(
        path.into(),
        Some(&knot_schema::wire::Id::new("price".into()).unwrap_or_else(|e| panic!("{e}"))),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let bad = Condition::Exists(Field {
        index: 1,
        kind: FieldKind::Integer,
    });
    let error = ZenCompiler
        .compile(
            &[FieldKind::Integer],
            &[bad],
            &vec![source.clone()],
            &mut CompileBudget::default(),
        )
        .err()
        .unwrap_or_else(|| panic!("invalid IR accepted"));
    assert_eq!(error.code, "RULE_COMPILE");
    assert_eq!(error.path, path);
    assert_eq!(error.rule_ids, ["price"]);
    let leaf = Condition::Exists(Field {
        index: 0,
        kind: FieldKind::Integer,
    });
    let condition = Condition::All(vec![leaf; 16000]);
    let error = ZenCompiler
        .compile(
            &[FieldKind::Integer],
            &[condition],
            &vec![source],
            &mut CompileBudget::default(),
        )
        .err()
        .unwrap_or_else(|| panic!("oversized expression accepted"));
    assert_eq!(error.code, "RESOURCE_LIMIT");
    assert_eq!(error.path, path);
    assert_eq!(error.rule_ids, ["price"]);
    assert_eq!(error.details["limit"], "expressionBytes");
}
