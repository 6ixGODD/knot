//! Complete finite color partitions derived from every condition location.
use knot_schema::wire::*;
use std::collections::{BTreeMap, BTreeSet};
/// Each listed constant is a separate class; the least remaining RGB value
/// represents all other colors. Conditions are still evaluated by the backend.
pub(crate) fn colors(product: &Product) -> BTreeMap<Id, Vec<String>> {
    let mut sets: BTreeMap<Id, BTreeSet<String>> = product
        .questions
        .iter()
        .filter(|q| matches!(q, Question::Color { .. }))
        .map(|q| (q.id().clone(), BTreeSet::new()))
        .collect();
    let mut pending = Vec::new();
    let mut unsupported = BTreeSet::new();
    for q in &product.questions {
        if let Question::Single { options, .. } | Question::Multi { options, .. } = q {
            for o in options {
                if let Some(c) = &o.enabled_when {
                    pending.push(c);
                }
            }
        }
    }
    for e in &product.flow.edges {
        if let Some(c) = &e.when {
            pending.push(c);
        }
    }
    for p in &product.pricing.components {
        let (PriceComponent::Fixed { when, .. } | PriceComponent::Quantity { when, .. }) = p;
        if let Some(c) = when {
            pending.push(c);
        }
    }
    for s in &product.visual.slots {
        for c in &s.cases {
            let (VisualCase::Constant { when, .. } | VisualCase::FromAnswer { when, .. }) = c;
            pending.push(when);
        }
    }
    while let Some(c) = pending.pop() {
        match c {
            Condition::All { all } => pending.extend(all),
            Condition::Any { any } => pending.extend(any),
            Condition::Not { not } => pending.push(not),
            Condition::Leaf(Leaf::Eq { field, value } | Leaf::Ne { field, value }) => {
                if let (Some(set), Scalar::Text(s)) = (sets.get_mut(field), value) {
                    set.insert(s.to_uppercase());
                }
            }
            Condition::Leaf(Leaf::In { field, value }) => {
                if let Some(set) = sets.get_mut(field) {
                    for v in value {
                        if let Scalar::Text(s) = v {
                            set.insert(s.to_uppercase());
                        }
                    }
                }
            }
            Condition::Leaf(Leaf::Exists { .. }) => {}
            Condition::Leaf(
                Leaf::Gt { field, .. }
                | Leaf::Gte { field, .. }
                | Leaf::Lt { field, .. }
                | Leaf::Lte { field, .. }
                | Leaf::Between { field, .. }
                | Leaf::Contains { field, .. },
            ) => {
                unsupported.insert(field.clone());
            }
        }
    }
    sets.into_iter()
        .filter(|(id, _)| !unsupported.contains(id))
        .map(|(id, mut set)| {
            for n in 0..=0xFFFFFFu32 {
                let c = format!("#{n:06X}");
                if !set.contains(&c) {
                    set.insert(c);
                    break;
                }
            }
            (id, set.into_iter().collect())
        })
        .collect()
}

/// Finite lattice bounds in integer units or fixed scale nine decimal units.
#[derive(Debug, Clone, Copy)]
pub struct NumericDomain {
    pub first: i128,
    pub last: i128,
    pub step: i128,
    pub decimal: bool,
}
/// Reuses the compiler's precision/step intersection, including min anchors
/// that themselves cannot be represented at the answer scale.
pub fn numeric(q: &Question) -> Result<Option<NumericDomain>, knot_schema::KnotError> {
    use knot_schema::{Decimal, SAFE_INTEGER};
    let (min, max, step, quantum, bound, decimal) = match q {
        Question::Integer { min, max, step, .. } => (
            min.map(|v| i128::from(v.get())),
            max.map(|v| i128::from(v.get())),
            step.map(|v| i128::from(v.get())),
            1,
            i128::from(SAFE_INTEGER),
            false,
        ),
        Question::Decimal {
            min,
            max,
            step,
            scale,
            ..
        } => (
            min.as_deref()
                .map(Decimal::parse)
                .transpose()?
                .map(Decimal::scaled),
            max.as_deref()
                .map(Decimal::parse)
                .transpose()?
                .map(Decimal::scaled),
            step.as_deref()
                .map(Decimal::parse)
                .transpose()?
                .map(Decimal::scaled),
            10i128.pow(
                9 - u32::try_from(scale.get())
                    .ok()
                    .filter(|s| *s <= 9)
                    .ok_or_else(|| knot_schema::KnotError::new("INVALID_MODEL", "/scale"))?,
            ),
            i128::from(SAFE_INTEGER) * 1_000_000_000,
            true,
        ),
        _ => return Ok(None),
    };
    let (first, last, step) = crate::semantic::domain(min, max, step, quantum, bound, "")?;
    Ok(Some(NumericDomain {
        first,
        last,
        step,
        decimal,
    }))
}

// Derive only from the validated IR actually supplied to RuleCompiler.
pub(crate) fn dependencies(conditions: &[knot_rules::Condition]) -> Vec<Vec<usize>> {
    use knot_rules::Condition as C;
    conditions
        .iter()
        .map(|root| {
            let mut fields = BTreeSet::new();
            let mut pending = vec![root];
            while let Some(node) = pending.pop() {
                match node {
                    C::All(nodes) | C::Any(nodes) => pending.extend(nodes),
                    C::Not(n) => pending.push(n),
                    C::Eq(f, _)
                    | C::Ne(f, _)
                    | C::Gt(f, _)
                    | C::Gte(f, _)
                    | C::Lt(f, _)
                    | C::Lte(f, _)
                    | C::Between(f, _, _)
                    | C::In(f, _)
                    | C::Contains(f, _)
                    | C::Exists(f) => {
                        fields.insert(f.index);
                    }
                }
            }
            fields.into_iter().collect()
        })
        .collect()
}
