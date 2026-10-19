use crate::evaluation::Evaluation;
use crate::flow;
use knot_compiler::CompiledModel;
use knot_rules::Facts;
use knot_schema::{KnotError, wire::*};

fn validate(property: &str, value: &VisualValue, path: &str) -> Result<(), KnotError> {
    let valid = match (property, value) {
        ("visible", VisualValue::Bool(_)) => true,
        ("variant" | "assetRef" | "surface.textureRef", VisualValue::Text(s)) => {
            Id::new(s.clone()).is_ok()
        }
        ("color" | "emission.color", VisualValue::Text(s)) => {
            s.len() == 7
                && s.starts_with('#')
                && s.as_bytes()[1..]
                    .iter()
                    .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
        }
        (
            "opacity" | "surface.metallic" | "surface.roughness" | "emission.strength"
            | "light.intensity",
            VisualValue::Number(n),
        ) => n.is_finite() && (0.0..=1.0).contains(n),
        ("transform.position" | "transform.scale", VisualValue::Vector(v)) => {
            v.iter().all(|n| n.is_finite() && n.abs() <= 1e9)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(KnotError::new("INVALID_ANSWER", path))
    }
}
pub(crate) fn derive(
    model: &CompiledModel,
    state: &ConfigurationState,
    facts: &Facts,
    budget: &mut Evaluation,
) -> Result<Visual, KnotError> {
    let definition = &model.product().visual;
    let mut targets = Vec::new();
    for &i in model.target_order() {
        let t = &definition.targets[i];
        for (property, value) in &t.defaults {
            budget.charge(256)?;
            validate(property, value, &format!("/visual/targets/{i}/defaults"))?;
        }
        targets.push(VisualTarget {
            id: t.id.clone(),
            order: match t.order {
                Some(n) => n,
                None => Integer::new(0)?,
            },
            properties: t.defaults.clone(),
        });
    }
    for (i, slot) in definition.slots.iter().enumerate() {
        budget.charge((targets.len() + slot.cases.len()) as u64)?;
        let path = format!("/visual/slots/{i}");
        let mut selected = None;
        for (c, case) in slot.cases.iter().enumerate() {
            if flow::at(model, facts, &format!("{path}/cases/{c}/when"), budget)? {
                if selected.is_some() {
                    let mut e = KnotError::new("VISUAL_CONFLICT", &path);
                    e.rule_ids.push(slot.id.as_str().into());
                    return Err(e);
                }
                selected = Some((c, case));
            }
        }
        let Some((case_index, case)) = selected else {
            continue;
        };
        let value = match case {
            VisualCase::Constant { value, .. } => value.clone(),
            VisualCase::FromAnswer { value_from, .. } => {
                let a = state.answers.get(value_from).ok_or_else(|| {
                    let mut e = KnotError::new(
                        "MISSING_VALUE",
                        &format!("{path}/cases/{case_index}/valueFrom"),
                    );
                    e.rule_ids.push(slot.id.as_str().into());
                    e
                })?;
                let q = &model.product().questions[*model
                    .question_index()
                    .get(value_from)
                    .ok_or_else(|| KnotError::new("INVALID_MODEL", &path))?];
                match (q, a) {
                    (Question::Integer { .. }, Answer::Integer(n)) => {
                        VisualValue::Number(n.get() as f64)
                    }
                    (Question::Decimal { .. }, Answer::Text(s)) => VisualValue::Number(
                        s.parse::<f64>()
                            .map_err(|_| KnotError::new("NUMERIC_OVERFLOW", &path))?,
                    ),
                    (Question::Single { .. } | Question::Color { .. }, Answer::Text(s)) => {
                        VisualValue::Text(s.clone())
                    }
                    _ => return Err(KnotError::new("INVALID_ANSWER", &path)),
                }
            }
        };
        validate(&slot.property, &value, &path)?;
        let target = targets
            .iter_mut()
            .find(|t| t.id == slot.target)
            .ok_or_else(|| KnotError::new("INVALID_MODEL", &path))?;
        target.properties.insert(slot.property.clone(), value);
    }
    Ok(Visual { targets })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn property_domains_reject_nonfinite_unknown_and_out_of_range() {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
            let error = validate("opacity", &VisualValue::Number(n), "/visual")
                .err()
                .unwrap_or_else(|| panic!("invalid visual number"));
            assert_eq!(error.code, "INVALID_ANSWER");
        }
        for value in [
            VisualValue::Bool(true),
            VisualValue::Text("#abcdef".into()),
            VisualValue::Number(0.5),
        ] {
            assert_eq!(
                validate("unknown", &value, "/visual").err().map(|e| e.code),
                Some("INVALID_ANSWER".into())
            );
        }
        assert!(
            validate(
                "transform.scale",
                &VisualValue::Vector([-1e9, 0.0, 1e9]),
                "/visual"
            )
            .is_ok()
        );
        assert_eq!(
            validate(
                "transform.scale",
                &VisualValue::Vector([0.0, 1e9 + 1.0, 0.0]),
                "/visual"
            )
            .err()
            .map(|e| e.code),
            Some("INVALID_ANSWER".into())
        );
        assert!(validate("opacity", &VisualValue::Number(0.0), "/visual").is_ok());
        assert!(validate("opacity", &VisualValue::Number(1.0), "/visual").is_ok());
    }
}
