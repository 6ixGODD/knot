#![allow(clippy::unwrap_used)]
use super::*;
use knot_rules::{
    BooleanEffects, CompileBudget, Condition, FieldKind, RuleCompiler, RuleProgram, SourceMap,
};
use knot_schema::wire::Id;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[derive(Default)]
struct Probe {
    calls: AtomicUsize,
    health: AtomicUsize,
    poison: AtomicBool,
    error: AtomicBool,
}
struct Backend(Arc<Probe>);
struct Program {
    probe: Arc<Probe>,
    count: usize,
}
impl RuleCompiler for Backend {
    fn compile(
        &self,
        _: &[FieldKind],
        c: &[Condition],
        _: &SourceMap,
        _: &mut CompileBudget,
    ) -> Result<Arc<dyn RuleProgram>, KnotError> {
        Ok(Arc::new(Program {
            probe: self.0.clone(),
            count: c.len(),
        }))
    }
}
impl RuleProgram for Program {
    fn condition_count(&self) -> usize {
        self.count
    }
    fn evaluate(
        &self,
        f: &Facts,
        r: &[ConditionId],
        b: &mut CallBudget,
    ) -> Result<BooleanEffects, KnotError> {
        if self.probe.poison.load(Ordering::SeqCst) {
            return Err(KnotError::new("INTERNAL", ""));
        }
        if r.is_empty() {
            self.probe.health.fetch_add(1, Ordering::SeqCst);
            b.charge(17)?;
        } else {
            self.probe.calls.fetch_add(1, Ordering::SeqCst);
            b.charge(1000)?;
            if self.probe.error.load(Ordering::SeqCst) {
                return Err(KnotError::new("RULE_EVAL", ""));
            }
        }
        BooleanEffects::new(
            r,
            r.iter().map(|id| (*id, f.values()[0].is_some())).collect(),
        )
    }
}
fn compiled(count: usize, multi: bool) -> (CompiledModel, Arc<Probe>) {
    let probe = Arc::new(Probe::default());
    let mut components = vec![serde_json::json!({"id":"base","category":"base","amountMinor":100})];
    for i in 0..count {
        components.push(serde_json::json!({"id":format!("c{i}"),"category":"adjustment","amountMinor":0,"when":{"op":"exists","field":"a"}}));
    }
    let first = if multi {
        serde_json::json!({"id":"a","type":"multi","label":"A","options":[{"id":"x","label":"X"}]})
    } else {
        serde_json::json!({"id":"a","type":"color","label":"A"})
    };
    let product = serde_json::json!({"schemaVersion":"1.0","productId":"cache","productVersion":"1","questions":[first,{"id":"b","type":"color","label":"B"}],"flow":{"start":"a","edges":[{"from":"a","to":"b"},{"from":"b","to":"END"}]},"pricing":{"currency":"CNY","components":components},"visual":{"targets":[],"slots":[]}});
    (
        knot_compiler::compile_product(
            &serde_json::to_vec(&product).unwrap(),
            &Backend(probe.clone()),
        )
        .unwrap(),
        probe,
    )
}
fn facts(a: Option<FactValue>, b: Option<FactValue>) -> Facts {
    Facts::new(vec![a, b], &[FieldKind::Color, FieldKind::Color]).unwrap()
}
fn color(s: &str) -> Option<FactValue> {
    Some(FactValue::Scalar(Scalar::Color(s.into())))
}
#[test]
fn proven_projection_presence_identity_cost_and_poison() {
    let (m, p) = compiled(2, false);
    let mut e = Evaluation::quoting();
    let absent = facts(None, None);
    let before = e.budget.remaining();
    assert!(!e.condition(&m, &absent, ConditionId(0)).unwrap());
    assert!(before - e.budget.remaining() >= 1000);
    let before = e.budget.remaining();
    assert!(
        !e.condition(&m, &facts(None, color("#FF0000")), ConditionId(0))
            .unwrap()
    );
    let hit = before - e.budget.remaining();
    assert!((17..1000).contains(&hit));
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    assert!(
        e.condition(&m, &facts(color("#FF0000"), None), ConditionId(0))
            .unwrap()
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 2);
    e.condition(&m, &absent, ConditionId(1)).unwrap();
    assert_eq!(p.calls.load(Ordering::SeqCst), 3);
    let poison = p.clone();
    std::thread::spawn(move || poison.poison.store(true, Ordering::SeqCst))
        .join()
        .unwrap();
    assert_eq!(
        e.condition(&m, &absent, ConditionId(0)).unwrap_err().code,
        "INTERNAL"
    );
    assert_eq!(e.health(&m, &absent).unwrap_err().code, "INTERNAL");
}
#[test]
fn errors_and_unproven_models_are_never_cached() {
    let (m, p) = compiled(1, false);
    let f = facts(None, None);
    let mut e = Evaluation::quoting();
    p.error.store(true, Ordering::SeqCst);
    assert!(e.condition(&m, &f, ConditionId(0)).is_err());
    assert!(e.cache.as_ref().unwrap().entries.is_empty());
    p.error.store(false, Ordering::SeqCst);
    e.condition(&m, &f, ConditionId(0)).unwrap();
    assert_eq!(p.calls.load(Ordering::SeqCst), 2);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut product =
        crate::quote::support::product(vec![crate::quote::support::single("a", true)]);
    if let knot_schema::wire::PriceComponent::Fixed { when, .. } =
        &mut product.pricing.components[0]
    {
        *when = Some(crate::quote::support::condition());
    }
    let m = crate::quote::support::controlled(
        product,
        move |_, _| {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        },
        1,
    );
    let f = Facts::new(vec![None], &[FieldKind::Single]).unwrap();
    let mut e = Evaluation::quoting();
    e.condition(&m, &f, ConditionId(0)).unwrap();
    e.condition(&m, &f, ConditionId(0)).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(e.cache.as_ref().unwrap().entries.is_empty());
}
#[test]
fn entry_and_storage_caps_fall_back_to_real_misses() {
    let (m, p) = compiled(257, false);
    let f = facts(None, None);
    let mut e = Evaluation::quoting();
    for i in 0..257 {
        e.condition(&m, &f, ConditionId(i)).unwrap();
    }
    assert_eq!(e.cache.as_ref().unwrap().entries.len(), 256);
    assert!(e.cache.as_ref().unwrap().closed);
    e.condition(&m, &f, ConditionId(256)).unwrap();
    assert_eq!(p.calls.load(Ordering::SeqCst), 258);
    let (m, p) = compiled(10, true);
    let mut e = Evaluation::quoting();
    for run in 0..10 {
        let values = (0..256)
            .map(|i| Id::new(format!("{run:02}{i:03}{}", "x".repeat(123))).unwrap())
            .collect();
        let f = Facts::new(
            vec![Some(FactValue::Multi(values)), None],
            &[FieldKind::Multi, FieldKind::Color],
        )
        .unwrap();
        e.condition(&m, &f, ConditionId(run)).unwrap();
        assert!(e.cache.as_ref().unwrap().bytes <= BYTES);
    }
    assert!(e.cache.as_ref().unwrap().closed);
    assert!(e.cache.as_ref().unwrap().entries.len() < 10);
    assert_eq!(p.calls.load(Ordering::SeqCst), 10);
}
#[test]
fn projected_keys_distinguish_empty_missing_types_and_multi_order() {
    fn key(value: Option<FactValue>, kind: FieldKind) -> Vec<u8> {
        let f = Facts::new(vec![value], &[kind]).unwrap();
        let mut v = vec![];
        project(&f, &[0], |p| {
            v.extend_from_slice(p);
            Ok(())
        })
        .unwrap();
        v
    }
    assert_ne!(
        key(None, FieldKind::Multi),
        key(Some(FactValue::Multi(vec![])), FieldKind::Multi)
    );
    let a = Id::new("a".into()).unwrap();
    let b = Id::new("b".into()).unwrap();
    assert_ne!(
        key(
            Some(FactValue::Multi(vec![a.clone(), b.clone()])),
            FieldKind::Multi
        ),
        key(Some(FactValue::Multi(vec![b, a])), FieldKind::Multi)
    );
    assert_ne!(
        key(
            Some(FactValue::Scalar(Scalar::Integer(
                knot_schema::wire::Integer::new(1).unwrap()
            ))),
            FieldKind::Integer
        ),
        key(
            Some(FactValue::Scalar(Scalar::Decimal(
                knot_schema::Decimal::parse("1").unwrap()
            ))),
            FieldKind::Decimal
        )
    );
}
