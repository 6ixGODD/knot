mod support;
use knot_schema::{SAFE_INTEGER, wire::*};
use support::*;

#[test]
fn second_base_conflicts_before_later_backend_failure() {
    let mut p = quantity(100);
    p.pricing
        .components
        .push(fixed("second", PriceCategory::Base, 200));
    p.pricing.components.push(PriceComponent::Fixed {
        id: id("later"),
        category: PriceCategory::Adjustment,
        amount_minor: n(1),
        when: Some(condition()),
    });
    let m = controlled(
        p,
        |_, _| Err(knot_schema::KnotError::new("RULE_EVAL", "")),
        1,
    );
    let state = ConfigurationState {
        schema_version: Version::V1,
        product_id: id("test"),
        model_digest: m.digest().into(),
        revision: n(0),
        answers: Default::default(),
        skipped: vec![],
    };
    fails(
        &m,
        &state,
        &set(&state, "a", text("1")),
        "BASE_PRICE_CONFLICT",
    );
}

fn quantity(unit: i64) -> Product {
    let mut p = product(vec![Question::Decimal {
        id: id("a"),
        label: "a".into(),
        required: Some(false),
        scale: n(9),
        min: None,
        max: None,
        step: None,
    }]);
    p.pricing.components = vec![PriceComponent::Quantity {
        id: id("base"),
        category: PriceCategory::Base,
        unit_minor: n(unit),
        quantity_from: id("a"),
        when: None,
    }];
    p
}
#[test]
fn exact_component_rounding_unknown_and_unpriced() {
    for (unit, quantity, expected) in [
        (100, "0.335", 34),
        (100, "0.334999999", 33),
        (100, "0.345", 35),
        (-100, "0.335", -34),
    ] {
        let mut p = quantity_product(unit);
        p.pricing
            .components
            .push(fixed("offset", PriceCategory::Adjustment, 100));
        let m = model(p);
        let s = initial(&m);
        assert_eq!(s.snapshot.price.status, PriceStatus::Unknown);
        assert_eq!(s.snapshot.price.total_minor, None);
        assert!(s.snapshot.price.components.is_empty());
        fails(&m, &s.state, &skip(&s.state, "a"), "UNPRICED");
        let r = answer(&m, &s.state, "a", text(quantity));
        assert_eq!(r.snapshot.price.components[0].amount_minor, n(expected));
        assert_eq!(r.snapshot.price.total_minor, Some(n(expected + 100)));
        assert_eq!(r.snapshot.price.status, PriceStatus::Final);
    }
}
fn quantity_product(unit: i64) -> Product {
    quantity(unit)
}

#[test]
fn quantity_safe_boundary_overflow_negative_and_base_conflict() {
    let m = model(quantity(1));
    let s = initial(&m).state;
    assert_eq!(
        answer(&m, &s, "a", text(&SAFE_INTEGER.to_string()))
            .snapshot
            .price
            .total_minor,
        Some(n(SAFE_INTEGER))
    );
    fails(&m, &s, &set(&s, "a", text("-1")), "INVALID_ANSWER");
    let m = model(quantity(SAFE_INTEGER));
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("2")), "NUMERIC_OVERFLOW");
    fails(
        &m,
        &s,
        &set(&s, "a", text("9007199254740990.999999999")),
        "NUMERIC_OVERFLOW",
    );
    let m = model(quantity(-100));
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("1")), "NEGATIVE_TOTAL");
    let mut p = quantity(100);
    p.pricing
        .components
        .push(fixed("other", PriceCategory::Base, 100));
    let m = model(p);
    let e = knot_core::initial(&m)
        .err()
        .unwrap_or_else(|| panic!("conflict"));
    assert_eq!(e.code, "BASE_PRICE_CONFLICT");
    assert_eq!(e.rule_ids, vec!["base", "other"]);
}

fn visual_product() -> Product {
    let mut p = product(vec![single("a", true)]);
    p.visual.targets = vec![
        TargetDefinition {
            id: id("z"),
            order: Some(n(1)),
            defaults: Default::default(),
        },
        TargetDefinition {
            id: id("a"),
            order: Some(n(1)),
            defaults: std::collections::BTreeMap::from([(
                "visible".into(),
                VisualValue::Bool(false),
            )]),
        },
    ];
    p.visual.slots = vec![VisualSlot {
        id: id("visibility"),
        target: id("a"),
        property: "visible".into(),
        cases: vec![VisualCase::Constant {
            when: condition(),
            value: VisualValue::Bool(true),
        }],
    }];
    p
}
#[test]
fn visual_defaults_restore_and_targets_sort_by_order_then_id() {
    let m = controlled(visual_product(), |_, f| Ok(single_fact(f, 0, "yes")), 1);
    let s = initial(&m);
    assert_eq!(
        s.snapshot
            .visual
            .targets
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        vec![id("a"), id("z")]
    );
    let s = answer(&m, &s.state, "a", text("yes"));
    assert_eq!(
        s.snapshot.visual.targets[0].properties["visible"],
        VisualValue::Bool(true)
    );
    let s = answer(&m, &s.state, "a", text("no"));
    assert_eq!(
        s.snapshot.visual.targets[0].properties["visible"],
        VisualValue::Bool(false)
    );
    assert_eq!(s.changes.visual_changed_targets, vec![id("a")]);
    assert!(s.snapshot.visual.targets[1].properties.is_empty());
}

#[test]
fn visual_conflict_missing_and_numeric_projection_are_atomic() {
    let mut p = visual_product();
    p.visual.slots[0].cases.push(VisualCase::Constant {
        when: condition(),
        value: VisualValue::Bool(false),
    });
    let m = controlled(p, |_, f| Ok(single_fact(f, 0, "yes")), 1);
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("yes")), "VISUAL_CONFLICT");
    let mut p = product(vec![single("a", true), color("b", false)]);
    p.visual.targets = vec![TargetDefinition {
        id: id("target"),
        order: None,
        defaults: Default::default(),
    }];
    p.visual.slots = vec![VisualSlot {
        id: id("color"),
        target: id("target"),
        property: "color".into(),
        cases: vec![VisualCase::FromAnswer {
            when: condition(),
            value_from: id("b"),
        }],
    }];
    let m = controlled(p, |_, f| Ok(single_fact(f, 0, "yes")), 1);
    let s = initial(&m).state;
    fails(&m, &s, &set(&s, "a", text("yes")), "MISSING_VALUE");
    let missing = knot_core::apply(&m, &s, &set(&s, "a", text("yes")))
        .err()
        .unwrap_or_else(|| panic!("missing source"));
    assert_eq!(missing.path, "/visual/slots/0/cases/0/valueFrom");
    assert_eq!(missing.rule_ids, vec!["color"]);
    let mut p = quantity(100);
    p.visual = visual_product().visual;
    p.visual.slots[0].property = "opacity".into();
    p.visual.slots[0].cases = vec![VisualCase::FromAnswer {
        when: condition(),
        value_from: id("a"),
    }];
    let m = controlled(p, |_, f| Ok(f.values()[0].is_some()), 1);
    let s = initial(&m).state;
    let r = answer(&m, &s, "a", text("0.335"));
    assert_eq!(
        r.snapshot.visual.targets[0].properties["opacity"],
        VisualValue::Number(0.335)
    );
    fails(&m, &s, &set(&s, "a", text("1.1")), "INVALID_ANSWER");
}
