use super::*;
// Explicit typed Speaker component scenario. Condition ID truth is prescribed
// here; plan05 must separately compile speaker.json and execute actual ZEN.
pub fn speaker() -> CompiledModel {
    let choice = |name: &str, names: &[&str]| Question::Single {
        id: id(name),
        label: name.into(),
        required: Some(true),
        options: names.iter().map(|s| option(s, false)).collect(),
    };
    let mut questions = vec![
        choice("speaker_type", &["active", "passive"]),
        choice("material", &["walnut", "aluminum"]),
        color("panel_color", true),
        choice("power", &["50w", "100w", "200w"]),
        Question::Multi {
            id: id("extras"),
            label: "extras".into(),
            required: Some(false),
            options: vec![option("bluetooth", false), option("led", false)],
        },
        choice("cable", &["banana", "spade"]),
    ];
    if let Question::Single { options, .. } = &mut questions[3] {
        options[2].enabled_when = Some(condition());
    }
    let mut p = product(questions);
    p.flow.edges = vec![
        ("speaker_type", "material", false),
        ("material", "panel_color", false),
        ("panel_color", "power", true),
        ("panel_color", "cable", false),
        ("power", "extras", false),
        ("extras", "END", false),
        ("cable", "END", false),
    ]
    .into_iter()
    .map(|(from, to, c)| Edge {
        from: id(from),
        to: id(to),
        when: c.then(condition),
    })
    .collect();
    p.pricing.components = [
        ("base_active", 129900),
        ("base_passive", 89900),
        ("material_walnut", 30000),
        ("material_aluminum", 50000),
        ("power_100", 40000),
        ("power_200", 90000),
        ("extra_bluetooth", 19900),
        ("extra_led", 12900),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, amount))| PriceComponent::Fixed {
        id: id(name),
        category: if i < 2 {
            PriceCategory::Base
        } else {
            PriceCategory::Adjustment
        },
        amount_minor: n(amount),
        when: Some(condition()),
    })
    .collect();
    p.visual.targets = vec![
        TargetDefinition {
            id: id("cabinet"),
            order: Some(n(10)),
            defaults: BTreeMap::from([
                (
                    "assetRef".into(),
                    VisualValue::Text("cabinet-default".into()),
                ),
                ("surface.metallic".into(), VisualValue::Number(0.0)),
            ]),
        },
        TargetDefinition {
            id: id("panel"),
            order: Some(n(20)),
            defaults: BTreeMap::from([("color".into(), VisualValue::Text("#26272B".into()))]),
        },
        TargetDefinition {
            id: id("led"),
            order: Some(n(30)),
            defaults: BTreeMap::from([("visible".into(), VisualValue::Bool(false))]),
        },
    ];
    let case = |value| VisualCase::Constant {
        when: condition(),
        value,
    };
    p.visual.slots = vec![
        VisualSlot {
            id: id("cabinet_asset"),
            target: id("cabinet"),
            property: "assetRef".into(),
            cases: vec![
                case(VisualValue::Text("cabinet-walnut".into())),
                case(VisualValue::Text("cabinet-aluminum".into())),
            ],
        },
        VisualSlot {
            id: id("cabinet_metallic"),
            target: id("cabinet"),
            property: "surface.metallic".into(),
            cases: vec![case(VisualValue::Number(0.9))],
        },
        VisualSlot {
            id: id("panel_color"),
            target: id("panel"),
            property: "color".into(),
            cases: vec![VisualCase::FromAnswer {
                when: condition(),
                value_from: id("panel_color"),
            }],
        },
        VisualSlot {
            id: id("led_visibility"),
            target: id("led"),
            property: "visible".into(),
            cases: vec![case(VisualValue::Bool(true))],
        },
    ];
    controlled(
        p,
        |id, f| {
            let active = single_fact(f, 0, "active");
            let walnut = single_fact(f, 1, "walnut");
            let aluminum = single_fact(f, 1, "aluminum");
            let extra = |name: &str| matches!(&f.values()[4],Some(knot_rules::FactValue::Multi(ids)) if ids.iter().any(|id|id.as_str()==name));
            Ok(match id {
                0 => active && aluminum,
                1 | 2 => active,
                3 => single_fact(f, 0, "passive"),
                4 | 10 => walnut,
                5 | 11 | 12 => aluminum,
                6 => single_fact(f, 3, "100w"),
                7 => single_fact(f, 3, "200w"),
                8 => extra("bluetooth"),
                9 | 14 => extra("led"),
                13 => f.values()[2].is_some(),
                _ => panic!("unexpected condition {id}"),
            })
        },
        1,
    )
}
pub fn complete(model: &CompiledModel) -> TransitionResult {
    let mut s = initial(model);
    for (q, v) in [
        ("speaker_type", text("active")),
        ("material", text("aluminum")),
        ("panel_color", text("#315a79")),
        ("power", text("200w")),
        ("extras", Answer::Multi(vec![id("led")])),
    ] {
        s = answer(model, &s.state, q, v);
    }
    s
}
