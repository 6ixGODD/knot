use crate::evaluation::Evaluation;
use knot_compiler::CompiledModel;
use knot_rules::{FactValue, Facts, FieldKind, Scalar};
use knot_schema::{Decimal, KnotError, wire::*};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn pointer(id: &Id) -> String {
    format!(
        "/answers/{}",
        id.as_str().replace('~', "~0").replace('/', "~1")
    )
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
pub(crate) fn options(q: &Question) -> &[OptionDefinition] {
    match q {
        Question::Single { options, .. } | Question::Multi { options, .. } => options,
        _ => &[],
    }
}
pub(crate) fn normalize(q: &Question, value: &Answer) -> Result<Answer, KnotError> {
    let bad = || KnotError::new("INVALID_ANSWER", &pointer(q.id()));
    let option_bad = || KnotError::new("INVALID_OPTION", &pointer(q.id()));
    match (q, value) {
        (Question::Single { options, .. }, Answer::Text(s)) => {
            if !options.iter().any(|o| o.id.as_str() == s) {
                return Err(option_bad());
            }
            Ok(value.clone())
        }
        (Question::Multi { options, .. }, Answer::Multi(ids)) => {
            if ids.len() > options.len() || (required(q) && ids.is_empty()) {
                return Err(bad());
            }
            let set: BTreeSet<_> = ids.iter().collect();
            if set.len() != ids.len() {
                return Err(bad());
            }
            let sorted: Vec<_> = options
                .iter()
                .filter(|o| set.contains(&o.id))
                .map(|o| o.id.clone())
                .collect();
            if sorted.len() != ids.len() {
                return Err(option_bad());
            }
            Ok(Answer::Multi(sorted))
        }
        (Question::Integer { min, max, step, .. }, Answer::Integer(n)) => {
            let n = n.get();
            if min.is_some_and(|m| n < m.get())
                || max.is_some_and(|m| n > m.get())
                || step.is_some_and(|s| {
                    s.get() <= 0
                        || (i128::from(n) - i128::from(min.map(|m| m.get()).unwrap_or(0)))
                            % i128::from(s.get())
                            != 0
                })
            {
                return Err(bad());
            }
            Ok(value.clone())
        }
        (
            Question::Decimal {
                scale,
                min,
                max,
                step,
                ..
            },
            Answer::Text(s),
        ) => {
            let n = Decimal::parse(s).map_err(|_| bad())?;
            let min = min.as_deref().map(Decimal::parse).transpose()?;
            let max = max.as_deref().map(Decimal::parse).transpose()?;
            let step = step.as_deref().map(Decimal::parse).transpose()?;
            let anchor = match min {
                Some(m) => m,
                None => Decimal::parse("0")?,
            };
            if i64::from(n.scale()) > scale.get()
                || min.is_some_and(|m| n < m)
                || max.is_some_and(|m| n > m)
                || step.is_some_and(|s| !n.fits_step(anchor, s))
            {
                return Err(bad());
            }
            Ok(Answer::Text(n.to_string()))
        }
        (Question::Color { .. }, Answer::Text(s)) => {
            if s.len() != 7
                || !s.starts_with('#')
                || !s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(bad());
            }
            Ok(Answer::Text(s.to_ascii_uppercase()))
        }
        _ => Err(bad()),
    }
}
pub(crate) fn validate(
    model: &CompiledModel,
    state: &ConfigurationState,
    budget: &mut Evaluation,
) -> Result<(), KnotError> {
    if state.product_id != model.product().product_id || state.model_digest != model.digest() {
        return Err(KnotError::new("MODEL_MISMATCH", "/modelDigest"));
    }
    if state.revision.get() < 0 {
        return Err(KnotError::new("INVALID_STATE", "/revision"));
    }
    let questions = &model.product().questions;
    if state.answers.len() + state.skipped.len() > questions.len() {
        return Err(KnotError::new("INVALID_STATE", ""));
    }
    budget.charge((questions.len() * 512) as u64)?;
    for (id, value) in &state.answers {
        let index = model
            .question_index()
            .get(id)
            .ok_or_else(|| KnotError::new("INVALID_STATE", &pointer(id)))?;
        if normalize(&questions[*index], value).ok().as_ref() != Some(value) {
            return Err(KnotError::new("INVALID_STATE", &pointer(id)));
        }
    }
    let mut previous = None;
    for id in &state.skipped {
        let i = *model
            .question_index()
            .get(id)
            .ok_or_else(|| KnotError::new("INVALID_STATE", "/skipped"))?;
        if previous.is_some_and(|p| p >= i)
            || required(&questions[i])
            || state.answers.contains_key(id)
        {
            return Err(KnotError::new("INVALID_STATE", "/skipped"));
        }
        previous = Some(i);
    }
    Ok(())
}
pub(crate) fn facts(
    model: &CompiledModel,
    answers: &BTreeMap<Id, Answer>,
    budget: &mut Evaluation,
) -> Result<Facts, KnotError> {
    budget.charge(
        (model.product().questions.len() * 4
            + answers
                .values()
                .map(|a| match a {
                    Answer::Text(s) => s.len(),
                    Answer::Multi(v) => v.iter().map(|i| i.as_str().len()).sum(),
                    Answer::Integer(_) => 16,
                })
                .sum::<usize>()) as u64,
    )?;
    let mut fields = Vec::new();
    let mut values = Vec::new();
    for q in &model.product().questions {
        fields.push(match q {
            Question::Single { .. } => FieldKind::Single,
            Question::Multi { .. } => FieldKind::Multi,
            Question::Integer { .. } => FieldKind::Integer,
            Question::Decimal { .. } => FieldKind::Decimal,
            Question::Color { .. } => FieldKind::Color,
        });
        values.push(match (q, answers.get(q.id())) {
            (_, None) => None,
            (Question::Single { .. }, Some(Answer::Text(s))) => {
                Some(FactValue::Scalar(Scalar::Single(Id::new(s.clone())?)))
            }
            (Question::Multi { .. }, Some(Answer::Multi(v))) => Some(FactValue::Multi(v.clone())),
            (Question::Integer { .. }, Some(Answer::Integer(n))) => {
                Some(FactValue::Scalar(Scalar::Integer(*n)))
            }
            (Question::Decimal { .. }, Some(Answer::Text(s))) => {
                Some(FactValue::Scalar(Scalar::Decimal(Decimal::parse(s)?)))
            }
            (Question::Color { .. }, Some(Answer::Text(s))) => {
                Some(FactValue::Scalar(Scalar::Color(s.clone())))
            }
            _ => return Err(KnotError::new("INVALID_STATE", &pointer(q.id()))),
        });
    }
    Facts::new(values, &fields)
}
