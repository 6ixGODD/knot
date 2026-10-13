use knot_rules::FieldKind;
use knot_schema::{Decimal, KnotError, SAFE_INTEGER, currency, wire::*};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn bad(path: &str) -> KnotError {
    KnotError::new("INVALID_MODEL", path)
}
pub(crate) fn required(q: &Question) -> bool {
    match q {
        Question::Single { required, .. }
        | Question::Multi { required, .. }
        | Question::Integer { required, .. }
        | Question::Decimal { required, .. }
        | Question::Color { required, .. } => required.unwrap_or(true),
    }
}
pub(crate) fn kind(q: &Question) -> FieldKind {
    match q {
        Question::Single { .. } => FieldKind::Single,
        Question::Multi { .. } => FieldKind::Multi,
        Question::Integer { .. } => FieldKind::Integer,
        Question::Decimal { .. } => FieldKind::Decimal,
        Question::Color { .. } => FieldKind::Color,
    }
}
fn unique<'a>(set: &mut BTreeSet<&'a Id>, id: &'a Id, path: &str) -> Result<(), KnotError> {
    if id.as_str() == "END" || !set.insert(id) {
        Err(bad(path))
    } else {
        Ok(())
    }
}
pub(crate) fn color(value: &mut str, path: &str) -> Result<(), KnotError> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(bad(path));
    }
    value.make_ascii_uppercase();
    Ok(())
}
pub(crate) fn decimal(value: &mut String, path: &str) -> Result<Decimal, KnotError> {
    let parsed = Decimal::parse(value).map_err(|_| bad(path))?;
    *value = parsed.to_string();
    Ok(parsed)
}
pub(crate) fn definitions(product: &mut Product) -> Result<BTreeMap<Id, usize>, KnotError> {
    let mut index = BTreeMap::new();
    for (i, q) in product.questions.iter_mut().enumerate() {
        let path = format!("/questions/{i}");
        if q.id().as_str() == "END" || index.insert(q.id().clone(), i).is_some() {
            return Err(bad(&format!("{path}/id")));
        }
        match q {
            Question::Single {
                required, options, ..
            }
            | Question::Multi {
                required, options, ..
            } => {
                *required = Some(required.unwrap_or(true));
                let mut ids = BTreeSet::new();
                for (j, option) in options.iter().enumerate() {
                    unique(&mut ids, &option.id, &format!("{path}/options/{j}/id"))?;
                }
            }
            Question::Color { required, .. } => *required = Some(required.unwrap_or(true)),
            Question::Integer {
                required,
                min,
                max,
                step,
                ..
            } => {
                *required = Some(required.unwrap_or(true));
                domain(
                    min.map(|v| i128::from(v.get())),
                    max.map(|v| i128::from(v.get())),
                    step.map(|v| i128::from(v.get())),
                    1,
                    i128::from(SAFE_INTEGER),
                    &path,
                )?;
            }
            Question::Decimal {
                required,
                min,
                max,
                step,
                scale,
                ..
            } => {
                *required = Some(required.unwrap_or(true));
                let parse =
                    |v: &mut Option<String>, suffix: &str| -> Result<Option<i128>, KnotError> {
                        v.as_mut()
                            .map(|v| decimal(v, &format!("{path}/{suffix}")).map(Decimal::scaled))
                            .transpose()
                    };
                let lo = parse(min, "min")?;
                let hi = parse(max, "max")?;
                let stride = parse(step, "step")?;
                let scale =
                    u32::try_from(scale.get()).map_err(|_| bad(&format!("{path}/scale")))?;
                if scale > 9 {
                    return Err(bad(&format!("{path}/scale")));
                }
                domain(
                    lo,
                    hi,
                    stride,
                    10i128.pow(9 - scale),
                    i128::from(SAFE_INTEGER) * 1_000_000_000,
                    &path,
                )?;
            }
        }
    }
    Ok(index)
}

// Intersect the min-anchored step lattice with the permitted answer precision.
// All operands are fixed-scale i128; the largest intermediate is < 10^35.
fn domain(
    min: Option<i128>,
    max: Option<i128>,
    step: Option<i128>,
    quantum: i128,
    bound: i128,
    path: &str,
) -> Result<(), KnotError> {
    let lo = min.unwrap_or(-bound);
    let hi = max.unwrap_or(bound);
    if lo > hi {
        return Err(bad(&format!("{path}/max")));
    }
    if step.is_some_and(|s| s <= 0) {
        return Err(bad(&format!("{path}/step")));
    }
    let (origin, period) = if let Some(step) = step {
        let anchor = min.unwrap_or(0);
        let (gcd, inverse) = extended_gcd(step, quantum);
        if anchor % gcd != 0 {
            return Err(bad(path));
        }
        let modulus = quantum / gcd;
        let t = ((-anchor / gcd) * inverse).rem_euclid(modulus);
        (anchor + step * t, step * modulus)
    } else {
        (0, quantum)
    };
    let first = lo + (origin - lo).rem_euclid(period);
    if first > hi { Err(bad(path)) } else { Ok(()) }
}
fn extended_gcd(mut a: i128, mut b: i128) -> (i128, i128) {
    let (mut x, mut y) = (1, 0);
    while b != 0 {
        let quotient = a / b;
        (a, b) = (b, a % b);
        (x, y) = (y, x - quotient * y);
    }
    (a, x)
}

#[derive(Clone, Copy)]
enum Property {
    Bool,
    Id,
    Color,
    Number,
    Vector,
}
fn property(name: &str, path: &str) -> Result<Property, KnotError> {
    match name {
        "visible" => Ok(Property::Bool),
        "variant" | "assetRef" | "surface.textureRef" => Ok(Property::Id),
        "color" | "emission.color" => Ok(Property::Color),
        "opacity" | "surface.metallic" | "surface.roughness" | "emission.strength"
        | "light.intensity" => Ok(Property::Number),
        "transform.position" | "transform.scale" => Ok(Property::Vector),
        _ => Err(bad(path)),
    }
}
fn visual(value: &mut VisualValue, property: Property, path: &str) -> Result<(), KnotError> {
    match (property, value) {
        (Property::Bool, VisualValue::Bool(_)) => Ok(()),
        (Property::Id, VisualValue::Text(s)) => {
            Id::new(s.clone()).map(|_| ()).map_err(|_| bad(path))
        }
        (Property::Color, VisualValue::Text(s)) => color(s, path),
        (Property::Number, VisualValue::Number(n)) if n.is_finite() && (0.0..=1.0).contains(n) => {
            Ok(())
        }
        (Property::Vector, VisualValue::Vector(v))
            if v.iter().all(|n| n.is_finite() && n.abs() <= 1e9) =>
        {
            Ok(())
        }
        _ => Err(bad(path)),
    }
}
fn pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn outputs(product: &mut Product, index: &BTreeMap<Id, usize>) -> Result<(), KnotError> {
    currency::minor_units(&product.pricing.currency)?;
    let mut ids = BTreeSet::new();
    for (i, component) in product.pricing.components.iter().enumerate() {
        let path = format!("/pricing/components/{i}");
        let id = match component {
            PriceComponent::Fixed { id, .. } | PriceComponent::Quantity { id, .. } => id,
        };
        unique(&mut ids, id, &format!("{path}/id"))?;
        if let PriceComponent::Quantity { quantity_from, .. } = component {
            let path = format!("{path}/quantityFrom");
            let q = &product.questions[*index.get(quantity_from).ok_or_else(|| bad(&path))?];
            match q {
                Question::Integer { max, .. } if !max.is_some_and(|v| v.get() < 0) => (),
                Question::Decimal { max, .. } => {
                    if let Some(max) = max
                        && Decimal::parse(max).map_err(|_| bad(&path))?.coefficient() < 0
                    {
                        return Err(bad(&path));
                    }
                }
                _ => return Err(bad(&path)),
            }
        }
    }
    let mut targets = BTreeSet::new();
    for (i, target) in product.visual.targets.iter_mut().enumerate() {
        let path = format!("/visual/targets/{i}");
        unique(&mut targets, &target.id, &format!("{path}/id"))?;
        if target.order.is_none() {
            target.order = Some(Integer::new(0)?);
        }
        for (key, value) in &mut target.defaults {
            let path = format!("{path}/defaults/{}", pointer(key));
            visual(value, property(key, &path)?, &path)?;
        }
    }
    let mut ids = BTreeSet::new();
    let mut writes = BTreeSet::new();
    for (s, slot) in product.visual.slots.iter_mut().enumerate() {
        let path = format!("/visual/slots/{s}");
        unique(&mut ids, &slot.id, &format!("{path}/id"))?;
        if !targets.contains(&slot.target) {
            return Err(bad(&format!("{path}/target")));
        }
        let prop = property(&slot.property, &format!("{path}/property"))?;
        if !writes.insert((&slot.target, &slot.property)) {
            return Err(bad(&format!("{path}/property")));
        }
        for (c, case) in slot.cases.iter_mut().enumerate() {
            let path = format!("{path}/cases/{c}");
            match case {
                VisualCase::Constant { value, .. } => {
                    visual(value, prop, &format!("{path}/value"))?
                }
                VisualCase::FromAnswer { value_from, .. } => {
                    let path = format!("{path}/valueFrom");
                    let q = &product.questions[*index.get(value_from).ok_or_else(|| bad(&path))?];
                    if !matches!(
                        (prop, kind(q)),
                        (Property::Id, FieldKind::Single)
                            | (Property::Color, FieldKind::Color)
                            | (Property::Number, FieldKind::Integer | FieldKind::Decimal)
                    ) {
                        return Err(bad(&path));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::domain;
    #[test]
    fn precision_step_intersection_matches_small_exhaustive_domains() {
        for min in -12..=12 {
            for max in min..=12 {
                for step in 1..=12 {
                    for quantum in [1, 10] {
                        let expected =
                            (min..=max).any(|x| x % quantum == 0 && (x - min) % step == 0);
                        assert_eq!(
                            domain(Some(min), Some(max), Some(step), quantum, 100, "").is_ok(),
                            expected
                        );
                    }
                }
            }
        }
        assert!(domain(None, Some(-1), Some(10), 1, 9, "").is_err());
        assert!(domain(None, Some(-1), Some(9), 1, 9, "").is_ok());
    }
}
