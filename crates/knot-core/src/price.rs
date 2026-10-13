use crate::flow;
use knot_compiler::CompiledModel;
use knot_rules::{CallBudget, Facts};
use knot_schema::{Decimal, KnotError, SAFE_INTEGER, wire::*};

pub(crate) fn derive(
    model: &CompiledModel,
    state: &ConfigurationState,
    facts: &Facts,
    status: FlowStatus,
    budget: &mut CallBudget,
) -> Result<Price, KnotError> {
    let pricing = &model.product().pricing;
    knot_schema::currency::minor_units(&pricing.currency)?;
    let mut matched = Vec::new();
    let mut bases = Vec::new();
    for (i, component) in pricing.components.iter().enumerate() {
        budget.charge(128)?;
        let (id, category, when) = match component {
            PriceComponent::Fixed {
                id, category, when, ..
            }
            | PriceComponent::Quantity {
                id, category, when, ..
            } => (id, category, when),
        };
        if when.is_some()
            && !flow::at(
                model,
                facts,
                &format!("/pricing/components/{i}/when"),
                budget,
            )?
        {
            continue;
        }
        if *category == PriceCategory::Base {
            bases.push(id.as_str().to_owned());
            if bases.len() > 1 {
                let mut error = KnotError::new("BASE_PRICE_CONFLICT", "/pricing/components");
                error.rule_ids = bases;
                return Err(error);
            }
        }
        matched.push((i, component));
    }
    let mut components = Vec::new();
    let mut unresolved = false;
    let mut total = 0i128;
    for (i, component) in matched {
        let path = format!("/pricing/components/{i}");
        let (id, amount) = match component {
            PriceComponent::Fixed {
                id, amount_minor, ..
            } => (id, Some(amount_minor.get())),
            PriceComponent::Quantity {
                id,
                unit_minor,
                quantity_from,
                ..
            } => {
                let quantity = match state.answers.get(quantity_from) {
                    None => None,
                    Some(Answer::Integer(n)) => Some(Decimal::parse(&n.get().to_string())?),
                    Some(Answer::Text(s)) => Some(Decimal::parse(s)?),
                    _ => return Err(KnotError::new("INVALID_MODEL", &path)),
                };
                let amount = if let Some(q) = quantity {
                    if q.coefficient() < 0 {
                        return Err(KnotError::new(
                            "INVALID_ANSWER",
                            &crate::state::pointer(quantity_from),
                        ));
                    }
                    Some(q.multiply_minor(unit_minor.get()).map_err(|mut e| {
                        e.path = path.clone();
                        e.rule_ids = vec![id.as_str().into()];
                        e
                    })?)
                } else {
                    None
                };
                (id, amount)
            }
        };
        if let Some(amount) = amount {
            total = total
                .checked_add(i128::from(amount))
                .ok_or_else(|| KnotError::new("NUMERIC_OVERFLOW", &path))?;
            components.push(PricedComponent {
                id: id.clone(),
                amount_minor: Integer::new(amount)?,
            });
        } else {
            unresolved = true;
        }
    }
    if bases.is_empty() || unresolved {
        if status == FlowStatus::Complete {
            return Err(KnotError::new("UNPRICED", "/pricing"));
        }
        return Ok(Price {
            status: PriceStatus::Unknown,
            currency: pricing.currency.clone(),
            total_minor: None,
            components: Vec::new(),
        });
    }
    if total.abs() > i128::from(SAFE_INTEGER) {
        return Err(KnotError::new("NUMERIC_OVERFLOW", "/pricing"));
    }
    if total < 0 {
        return Err(KnotError::new("NEGATIVE_TOTAL", "/pricing"));
    }
    Ok(Price {
        status: if status == FlowStatus::Complete {
            PriceStatus::Final
        } else {
            PriceStatus::Estimated
        },
        currency: pricing.currency.clone(),
        total_minor: Some(Integer::new(total as i64)?),
        components,
    })
}
