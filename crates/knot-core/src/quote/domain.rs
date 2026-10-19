use knot_compiler::CompiledModel;
use knot_schema::{Decimal, KnotError, wire::*};
/// Cursor storage is linear even for a 256-option multi question.
pub(super) enum Domain {
    Values(std::vec::IntoIter<Answer>),
    Multi {
        ids: Vec<Id>,
        bits: Vec<bool>,
        first: bool,
        exhausted: bool,
        required: bool,
    },
    Number {
        next: i128,
        end: i128,
        step: i128,
        decimal: bool,
    },
}
impl Domain {
    pub(super) fn new(
        model: &CompiledModel,
        q: &Question,
        options: &[AvailableOption],
        budget: &mut crate::evaluation::Evaluation,
    ) -> Result<Option<Self>, KnotError> {
        // Prepay owned cursor storage/copies before allocating any domain.
        let bytes = match q {
            Question::Single { .. } | Question::Multi { .. } => options
                .iter()
                .filter(|o| o.enabled)
                .map(|o| o.id.as_str().len() + std::mem::size_of::<Answer>() + 1)
                .sum::<usize>(),
            Question::Color { id, .. } => model.quote_colors(id).map_or(0, |colors| {
                colors
                    .iter()
                    .map(|c| c.len() + std::mem::size_of::<Answer>())
                    .sum()
            }),
            _ => std::mem::size_of::<Self>(),
        };
        budget.charge((bytes + 1) as u64)?;
        Ok(Some(match q {
            Question::Single { .. } => Self::Values(
                options
                    .iter()
                    .filter(|o| o.enabled)
                    .map(|o| Answer::Text(o.id.as_str().into()))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            Question::Multi { .. } => {
                let ids: Vec<_> = options
                    .iter()
                    .filter(|o| o.enabled)
                    .map(|o| o.id.clone())
                    .collect();
                Self::Multi {
                    bits: vec![false; ids.len()],
                    ids,
                    first: true,
                    exhausted: false,
                    required: crate::state::required(q),
                }
            }
            Question::Color { id, .. } => {
                let Some(colors) = model.quote_colors(id) else {
                    return Ok(None);
                };
                Self::Values(
                    colors
                        .iter()
                        .cloned()
                        .map(Answer::Text)
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
            }
            Question::Integer { .. } | Question::Decimal { .. } => {
                let d = knot_compiler::quote_domains::numeric(q)?
                    .ok_or_else(|| KnotError::new("INVALID_MODEL", ""))?;
                let count = d
                    .last
                    .checked_sub(d.first)
                    .and_then(|n| n.checked_div(d.step))
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| KnotError::new("NUMERIC_OVERFLOW", ""))?;
                if count > 10000 {
                    return Ok(None);
                }
                Self::Number {
                    next: d.first,
                    end: d.last,
                    step: d.step,
                    decimal: d.decimal,
                }
            }
        }))
    }
    pub(super) fn next(&mut self) -> Result<Option<Answer>, KnotError> {
        match self {
            Self::Values(v) => Ok(v.next()),
            Self::Number {
                next,
                end,
                step,
                decimal,
            } => {
                if *next > *end {
                    return Ok(None);
                }
                let n = *next;
                *next = next
                    .checked_add(*step)
                    .ok_or_else(|| KnotError::new("NUMERIC_OVERFLOW", ""))?;
                if *decimal {
                    let magnitude = n.abs();
                    let s = format!(
                        "{}{}.{:09}",
                        if n < 0 { "-" } else { "" },
                        magnitude / 1_000_000_000,
                        magnitude % 1_000_000_000
                    );
                    Ok(Some(Answer::Text(Decimal::parse(&s)?.to_string())))
                } else {
                    Ok(Some(Answer::Integer(Integer::new(
                        i64::try_from(n).map_err(|_| KnotError::new("NUMERIC_OVERFLOW", ""))?,
                    )?)))
                }
            }
            Self::Multi {
                ids,
                bits,
                first,
                exhausted,
                required,
            } => {
                if *exhausted {
                    return Ok(None);
                }
                if *first {
                    *first = false;
                    if !*required {
                        return Ok(Some(Answer::Multi(vec![])));
                    }
                }
                let mut carried = true;
                for bit in bits.iter_mut().rev() {
                    if *bit {
                        *bit = false
                    } else {
                        *bit = true;
                        carried = false;
                        break;
                    }
                }
                if carried {
                    *exhausted = true;
                    return Ok(None);
                }
                Ok(Some(Answer::Multi(
                    ids.iter()
                        .zip(bits.iter())
                        .filter(|(_, b)| **b)
                        .map(|(id, _)| id.clone())
                        .collect(),
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multi_stays_exhausted_after_all_subsets() {
        for size in [0, 1, 3] {
            for required in [false, true] {
                let mut domain = Domain::Multi {
                    ids: (0..size)
                        .map(|v| Id::new(format!("a{v}")).unwrap_or_else(|e| panic!("{e}")))
                        .collect(),
                    bits: vec![false; size],
                    first: true,
                    exhausted: false,
                    required,
                };
                let expected = (1usize << size) - usize::from(required);
                let mut seen = Vec::new();
                for _ in 0..expected {
                    let value = domain
                        .next()
                        .unwrap_or_else(|e| panic!("{e}"))
                        .unwrap_or_else(|| panic!("missing subset"));
                    assert!(!seen.contains(&value));
                    seen.push(value);
                }
                assert_eq!(
                    seen.iter().filter(|a| **a == Answer::Multi(vec![])).count(),
                    usize::from(!required)
                );
                for _ in 0..3 {
                    assert!(domain.next().unwrap_or_else(|e| panic!("{e}")).is_none());
                }
            }
        }
    }
}
