use knot_rules::{
    CallBudget, CompileBudget, Condition, ConditionId, FactValue, Facts, Field, FieldKind,
    RuleCompiler, RuleSource, Scalar,
};
use knot_rules_zen::ZenCompiler;
use knot_schema::wire::Id;

#[test]
fn unrelated_facts_are_not_copied_but_referenced_members_remain_metered()
-> Result<(), Box<dyn std::error::Error>> {
    let fields = [FieldKind::Multi, FieldKind::Integer, FieldKind::Single];
    let selected = Field {
        index: 2,
        kind: FieldKind::Single,
    };
    let member = Id::new("member-0".into())?;
    let program = ZenCompiler.compile(
        &fields,
        &[
            Condition::All(vec![
                Condition::Exists(selected),
                Condition::Eq(selected, Scalar::Single(Id::new("yes".into())?)),
            ]),
            Condition::Contains(
                Field {
                    index: 0,
                    kind: FieldKind::Multi,
                },
                member,
            ),
        ],
        &vec![
            RuleSource::new("/pricing/components/0/when".into(), None)?,
            RuleSource::new("/pricing/components/1/when".into(), None)?,
        ],
        &mut CompileBudget::default(),
    )?;
    let members = (0..256)
        .map(|i| Id::new(format!("member-{i}")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected_remaining = None;
    for unrelated in [None, Some(FactValue::Multi(members.clone()))] {
        let facts = Facts::new(
            vec![
                unrelated,
                None,
                Some(FactValue::Scalar(Scalar::Single(Id::new("yes".into())?))),
            ],
            &fields,
        )?;
        let mut budget = CallBudget::new();
        assert_eq!(
            program
                .evaluate(&facts, &[ConditionId(0)], &mut budget)?
                .values(),
            &[(ConditionId(0), true)]
        );
        if let Some(expected) = expected_remaining {
            assert_eq!(budget.remaining(), expected);
        }
        expected_remaining = Some(budget.remaining());
    }
    let facts = Facts::new(vec![Some(FactValue::Multi(members)), None, None], &fields)?;
    let mut budget = CallBudget::new();
    assert_eq!(
        program
            .evaluate(&facts, &[ConditionId(0)], &mut budget)?
            .values(),
        &[(ConditionId(0), false)]
    );
    let before = budget.remaining();
    assert_eq!(
        program
            .evaluate(&facts, &[ConditionId(1)], &mut budget)?
            .values(),
        &[(ConditionId(1), true)]
    );
    assert!(before - budget.remaining() >= 256 * 1024);
    Ok(())
}
