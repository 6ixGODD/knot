//! Per-call rule evaluation. Only Quote enables proven, bounded memoization.
use knot_compiler::CompiledModel;
use knot_rules::{CallBudget, ConditionId, FactValue, Facts, Scalar};
use knot_schema::KnotError;
use std::mem::size_of;
const ENTRIES: usize = 256;
const BYTES: usize = 262144;
struct Entry {
    id: ConditionId,
    key: Vec<u8>,
    value: bool,
}
struct Cache {
    entries: Vec<Entry>,
    bytes: usize,
    closed: bool,
}
pub(crate) struct Evaluation {
    budget: CallBudget,
    cache: Option<Cache>,
}
impl Evaluation {
    pub(crate) fn new() -> Self {
        Self {
            budget: CallBudget::new(),
            cache: None,
        }
    }
    pub(crate) fn quoting() -> Self {
        Self {
            budget: CallBudget::new(),
            cache: Some(Cache {
                entries: Vec::new(),
                bytes: size_of::<Cache>(),
                closed: false,
            }),
        }
    }
    pub(crate) fn charge(&mut self, cost: u64) -> Result<(), KnotError> {
        self.budget.charge(cost)
    }
    pub(crate) fn health(&mut self, model: &CompiledModel, facts: &Facts) -> Result<(), KnotError> {
        let effects = model.program().evaluate(facts, &[], &mut self.budget)?;
        if effects.values().is_empty() {
            Ok(())
        } else {
            Err(KnotError::new("RULE_EVAL", ""))
        }
    }
    fn real(
        &mut self,
        model: &CompiledModel,
        facts: &Facts,
        id: ConditionId,
    ) -> Result<bool, KnotError> {
        let effects = model.program().evaluate(facts, &[id], &mut self.budget)?;
        match effects.values() {
            [(actual, value)] if *actual == id => Ok(*value),
            _ => Err(KnotError::new("RULE_EVAL", "")),
        }
    }
    pub(crate) fn condition(
        &mut self,
        model: &CompiledModel,
        facts: &Facts,
        id: ConditionId,
    ) -> Result<bool, KnotError> {
        let Some(fields) = model
            .quote_dependencies(id)
            .filter(|_| self.cache.is_some())
        else {
            return self.real(model, facts, id);
        };
        let mut length = 0usize;
        project(facts, fields, |part| {
            self.budget.charge((part.len() + 1) as u64)?;
            length = length
                .checked_add(part.len())
                .ok_or_else(|| KnotError::new("NUMERIC_OVERFLOW", ""))?;
            Ok(())
        })?;
        let cache = self
            .cache
            .as_mut()
            .ok_or_else(|| KnotError::new("INTERNAL", ""))?;
        self.budget.charge(1)?; // capacity check, including unsuccessful insertion
        if cache.entries.capacity() == 0 && !cache.closed {
            let bytes = ENTRIES * size_of::<Entry>();
            if cache
                .bytes
                .checked_add(bytes)
                .and_then(|n| n.checked_add(length))
                .and_then(|n| n.checked_add(size_of::<Vec<u8>>()))
                .is_none_or(|n| n > BYTES)
            {
                cache.closed = true;
            } else {
                self.budget.charge(bytes as u64)?;
                if cache.entries.try_reserve_exact(ENTRIES).is_err() {
                    cache.closed = true;
                }
                let actual = cache.entries.capacity() * size_of::<Entry>();
                if actual > bytes {
                    self.budget.charge((actual - bytes) as u64)?;
                }
                cache.bytes += actual;
                if cache.bytes > BYTES {
                    cache.entries = Vec::new();
                    cache.bytes = size_of::<Cache>();
                    cache.closed = true;
                }
            }
        }
        // The bounded temporary key is included while owned, before allocation.
        if cache.entries.capacity() == 0
            || cache
                .bytes
                .checked_add(length)
                .and_then(|n| n.checked_add(size_of::<Vec<u8>>()))
                .is_none_or(|n| n > BYTES)
        {
            cache.closed = true;
            return self.real(model, facts, id);
        }
        let mut key = Vec::new();
        if key.try_reserve_exact(length).is_err() {
            cache.closed = true;
            return self.real(model, facts, id);
        }
        if cache
            .bytes
            .checked_add(key.capacity())
            .and_then(|n| n.checked_add(size_of::<Vec<u8>>()))
            .is_none_or(|n| n > BYTES)
        {
            cache.closed = true;
            return self.real(model, facts, id);
        }
        self.budget.charge((key.capacity() + 1) as u64)?;
        project(facts, fields, |part| {
            self.budget.charge((part.len() + 1) as u64)?;
            key.extend_from_slice(part);
            Ok(())
        })?;
        for entry in &cache.entries {
            self.budget.charge(1)?;
            if entry.id == id {
                self.budget
                    .charge((key.len() + entry.key.len() + 1) as u64)?;
                if entry.key == key {
                    let value = entry.value;
                    self.health(model, facts)?;
                    return Ok(value);
                }
            }
        }
        let value = self.real(model, facts, id)?;
        let cache = self
            .cache
            .as_mut()
            .ok_or_else(|| KnotError::new("INTERNAL", ""))?;
        self.budget.charge(1)?;
        if !cache.closed && cache.entries.len() < ENTRIES {
            cache.bytes += key.capacity();
            cache.entries.push(Entry { id, key, value });
        } else {
            cache.closed = true;
        }
        Ok(value)
    }
}
// An injective typed byte encoding, not a hash or another condition evaluator.
// Missing, empty multi, scalar kind, field index and every length are explicit.
fn project(
    facts: &Facts,
    fields: &[usize],
    mut emit: impl FnMut(&[u8]) -> Result<(), KnotError>,
) -> Result<(), KnotError> {
    for index in fields {
        emit(&index.to_le_bytes())?;
        let value = facts
            .values()
            .get(*index)
            .ok_or_else(|| KnotError::new("RULE_EVAL", ""))?;
        match value {
            None => emit(&[0])?,
            Some(FactValue::Scalar(Scalar::Single(v))) => {
                emit(&[1])?;
                emit(&v.as_str().len().to_le_bytes())?;
                emit(v.as_str().as_bytes())?;
            }
            Some(FactValue::Scalar(Scalar::Integer(v))) => {
                emit(&[2])?;
                emit(&v.get().to_le_bytes())?;
            }
            Some(FactValue::Scalar(Scalar::Decimal(v))) => {
                emit(&[3])?;
                emit(&v.scaled().to_le_bytes())?;
            }
            Some(FactValue::Scalar(Scalar::Color(v))) => {
                emit(&[4])?;
                emit(&v.len().to_le_bytes())?;
                emit(v.as_bytes())?;
            }
            Some(FactValue::Multi(values)) => {
                emit(&[5])?;
                emit(&values.len().to_le_bytes())?;
                for v in values {
                    emit(&v.as_str().len().to_le_bytes())?;
                    emit(v.as_str().as_bytes())?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
