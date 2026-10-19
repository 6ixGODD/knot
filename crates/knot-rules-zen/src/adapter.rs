use crate::{compile_probe, lowering, sync::poll_once};
use knot_rules::{
    BooleanEffects, CallBudget, CompileBudget, Condition, ConditionId, FactValue, Facts, FieldKind,
    RuleCompiler, RuleProgram, SourceMap,
};
use knot_schema::KnotError;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct ZenCompiler;
struct Entry {
    decision: zen_engine::Decision,
    constants: Vec<Value>,
    cost: u64,
    fields: Vec<usize>,
}
struct Program {
    entries: Vec<Entry>,
    fields: Vec<FieldKind>,
    faulted: AtomicBool,
}
impl RuleCompiler for ZenCompiler {
    fn compile(
        &self,
        fields: &[FieldKind],
        conditions: &[Condition],
        sources: &SourceMap,
        budget: &mut CompileBudget,
    ) -> Result<Arc<dyn RuleProgram>, KnotError> {
        if fields.len() > 256 || conditions.len() != sources.len() || conditions.len() > 4096 {
            return Err(KnotError::new("RULE_COMPILE", ""));
        }
        for (index, condition) in conditions.iter().enumerate() {
            budget
                .root()
                .and_then(|()| condition.validate(fields, budget, 1))
                .map_err(|e| at_source(e, &sources[index]))?;
        }
        let mut entries = Vec::with_capacity(conditions.len());
        for (index, condition) in conditions.iter().enumerate() {
            let lowered = lowering::lower(condition).map_err(|e| at_source(e, &sources[index]))?;
            budget
                .expression(lowered.expression.len())
                .map_err(|e| at_source(e, &sources[index]))?;
            entries.push(Entry {
                decision: compile_probe(&lowered.expression)
                    .map_err(|e| at_source(e, &sources[index]))?,
                constants: lowered.constants,
                cost: lowered.cost,
                fields: lowered.fields,
            });
        }
        Ok(Arc::new(Program {
            entries,
            fields: fields.to_vec(),
            faulted: AtomicBool::new(false),
        }))
    }
}
fn at_source(mut error: KnotError, source: &knot_rules::RuleSource) -> KnotError {
    error.path = source.path().into();
    error.rule_ids = vec![source.rule_id().into()];
    error
}
impl RuleProgram for Program {
    fn condition_count(&self) -> usize {
        self.entries.len()
    }
    fn evaluate(
        &self,
        facts: &Facts,
        requested: &[ConditionId],
        budget: &mut CallBudget,
    ) -> Result<BooleanEffects, KnotError> {
        if self.faulted.load(Ordering::Acquire) {
            return Err(KnotError::new("INTERNAL", ""));
        }
        if requested.len() > 4096 {
            return Err(KnotError::limit("conditionRoots", 4096, requested.len()));
        }
        budget.charge(256)?;
        // Revalidate the shape against this program, not only the Facts constructor's caller.
        if facts.values().len() != self.fields.len() {
            return Err(KnotError::new("RULE_EVAL", ""));
        }
        for (value, kind) in facts.values().iter().zip(&self.fields) {
            match value {
                None => {}
                Some(FactValue::Scalar(v)) if v.kind() == *kind => {}
                Some(FactValue::Multi(_)) if *kind == FieldKind::Multi => {}
                _ => return Err(KnotError::new("RULE_EVAL", "")),
            }
        }
        // Prepay all requested graphs, even when a condition short-circuits.
        if requested.is_empty() {
            return crate::isolation::isolate(&self.faulted, || {
                BooleanEffects::new(requested, vec![])
            });
        }
        for id in requested {
            let entry = self
                .entries
                .get(id.0)
                .ok_or_else(|| KnotError::new("RULE_EVAL", ""))?;
            // Every context keeps stable field indices, but unrelated values remain null.
            // Preserve the existing per-slot/scalar/member costs for the data actually copied.
            let mut data_cost = 256 + self.fields.len() as u64 * 16;
            for index in &entry.fields {
                data_cost += match &facts.values()[*index] {
                    None => 0,
                    Some(FactValue::Scalar(_)) => 1024 - 16,
                    Some(FactValue::Multi(values)) => values.len() as u64 * 1024,
                };
            }
            budget.charge(entry.cost + data_cost)?;
        }
        crate::isolation::isolate(&self.faulted, || {
            let mut output = Vec::with_capacity(requested.len());
            for id in requested {
                let entry = &self.entries[id.0];
                let mut present = vec![false; self.fields.len()];
                let mut values = vec![Value::Null; self.fields.len()];
                for index in &entry.fields {
                    let value = &facts.values()[*index];
                    present[*index] = value.is_some();
                    values[*index] = match value {
                        None => Value::Null,
                        Some(FactValue::Scalar(v)) => lowering::scalar(v)?,
                        Some(FactValue::Multi(v)) => Value::Array(
                            v.iter()
                                .map(|id| Value::String(id.as_str().into()))
                                .collect(),
                        ),
                    };
                }
                let context =
                    serde_json::from_value(json!({"p":present,"v":values,"c":entry.constants}))
                        .map_err(|_| KnotError::new("RULE_EVAL", ""))?;
                let response = poll_once(entry.decision.evaluate(context))?
                    .map_err(|_| KnotError::new("RULE_EVAL", ""))?;
                let value = serde_json::to_value(response.result)
                    .map_err(|_| KnotError::new("RULE_EVAL", ""))?;
                let object = value
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .ok_or_else(|| KnotError::new("RULE_EVAL", ""))?;
                let value = object
                    .get("result")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| KnotError::new("RULE_EVAL", ""))?;
                output.push((*id, value));
            }
            BooleanEffects::new(requested, output)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isolation::isolate;
    use std::sync::Barrier;
    #[test]
    fn backend_boundary_poison_rejects_inflight_and_later_calls() {
        let program = Arc::new(Program {
            entries: vec![],
            fields: vec![],
            faulted: AtomicBool::new(false),
        });
        let entered = Arc::new(Barrier::new(2));
        let released = Arc::new(Barrier::new(2));
        let p = program.clone();
        let a = entered.clone();
        let b = released.clone();
        let in_flight = std::thread::spawn(move || {
            isolate(&p.faulted, || {
                a.wait();
                b.wait();
                Ok(42)
            })
        });
        entered.wait();
        let failure = isolate::<()>(&program.faulted, || panic!("injected backend panic"));
        assert_eq!(failure.err().map(|e| e.code), Some("INTERNAL".into()));
        released.wait();
        let result = in_flight
            .join()
            .unwrap_or_else(|_| panic!("worker escaped boundary"));
        assert_eq!(result.err().map(|e| e.code), Some("INTERNAL".into()));
        let facts = Facts::new(vec![], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            program
                .evaluate(&facts, &[], &mut CallBudget::new())
                .err()
                .map(|e| e.code),
            Some("INTERNAL".into())
        );
        let rebuilt = Program {
            entries: vec![],
            fields: vec![],
            faulted: AtomicBool::new(false),
        };
        assert!(
            rebuilt
                .evaluate(&facts, &[], &mut CallBudget::new())
                .is_ok()
        );
    }
    #[test]
    fn panicking_payload_destructor_does_not_escape() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("injected payload destructor panic");
            }
        }
        let faulted = AtomicBool::new(false);
        let result = isolate::<()>(&faulted, || std::panic::panic_any(Payload));
        assert_eq!(result.err().map(|e| e.code), Some("INTERNAL".into()));
        assert!(faulted.load(Ordering::Acquire));
    }
}
