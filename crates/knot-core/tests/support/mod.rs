#![allow(dead_code)]
pub mod speaker;
// Test-only explicit typed assembly. Predicates are controlled condition-ID
// responses, never an AST interpreter or a production backend.
use knot_compiler::{CompiledModel, IndexedEdge, ModelParts};
use knot_rules::{BooleanEffects, CallBudget, ConditionId, Facts, RuleProgram, RuleSource};
use knot_schema::{KnotError, wire::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub fn id(s: &str) -> Id {
    Id::new(s.into()).unwrap_or_else(|e| panic!("{e}"))
}
pub fn n(n: i64) -> Integer {
    Integer::new(n).unwrap_or_else(|e| panic!("{e}"))
}
pub fn text(s: &str) -> Answer {
    Answer::Text(s.into())
}
pub fn condition() -> Condition {
    Condition::Leaf(Leaf::Exists { field: id("a") })
}
pub fn color(name: &str, required: bool) -> Question {
    Question::Color {
        id: id(name),
        label: name.into(),
        required: Some(required),
    }
}
pub fn single(name: &str, required: bool) -> Question {
    Question::Single {
        id: id(name),
        label: name.into(),
        required: Some(required),
        options: vec![option("yes", false), option("no", false)],
    }
}
pub fn option(name: &str, controlled: bool) -> OptionDefinition {
    OptionDefinition {
        id: id(name),
        label: name.into(),
        enabled_when: controlled.then(condition),
    }
}
pub fn fixed(name: &str, category: PriceCategory, amount: i64) -> PriceComponent {
    PriceComponent::Fixed {
        id: id(name),
        category,
        amount_minor: n(amount),
        when: None,
    }
}
pub fn product(questions: Vec<Question>) -> Product {
    let edges = questions
        .iter()
        .enumerate()
        .map(|(i, q)| Edge {
            from: q.id().clone(),
            to: questions
                .get(i + 1)
                .map(|q| q.id().clone())
                .unwrap_or_else(|| id("END")),
            when: None,
        })
        .collect();
    Product {
        schema_version: Version::V1,
        product_id: id("test"),
        product_version: id("1"),
        flow: FlowDefinition {
            start: questions[0].id().clone(),
            edges,
        },
        questions,
        pricing: Pricing {
            currency: "CNY".into(),
            components: vec![fixed("base", PriceCategory::Base, 100)],
        },
        visual: VisualDefinition {
            targets: vec![],
            slots: vec![],
        },
        metadata: None,
    }
}
pub type Response = dyn Fn(usize, &Facts) -> Result<bool, KnotError> + Send + Sync;
pub struct Controlled {
    pub count: usize,
    pub response: Arc<Response>,
    pub cost: u64,
}
impl RuleProgram for Controlled {
    fn condition_count(&self) -> usize {
        self.count
    }
    fn evaluate(
        &self,
        facts: &Facts,
        requested: &[ConditionId],
        budget: &mut CallBudget,
    ) -> Result<BooleanEffects, KnotError> {
        budget.charge(self.cost)?;
        BooleanEffects::new(
            requested,
            requested
                .iter()
                .map(|id| Ok((*id, (self.response)(id.0, facts)?)))
                .collect::<Result<_, KnotError>>()?,
        )
    }
}
pub fn model(product: Product) -> CompiledModel {
    controlled(product, |_, _| Ok(true), 1)
}
pub fn controlled(
    product: Product,
    response: impl Fn(usize, &Facts) -> Result<bool, KnotError> + Send + Sync + 'static,
    cost: u64,
) -> CompiledModel {
    let question_index: BTreeMap<_, _> = product
        .questions
        .iter()
        .enumerate()
        .map(|(i, q)| (q.id().clone(), i))
        .collect();
    let mut sources = vec![];
    for (q, question) in product.questions.iter().enumerate() {
        if let Question::Single { options, .. } | Question::Multi { options, .. } = question {
            for (o, option) in options.iter().enumerate() {
                if option.enabled_when.is_some() {
                    sources.push(
                        RuleSource::new(format!("/questions/{q}/options/{o}/enabledWhen"), None)
                            .unwrap_or_else(|e| panic!("{e}")),
                    );
                }
            }
        }
    }
    let mut edges = vec![];
    for (i, e) in product.flow.edges.iter().enumerate() {
        let condition = if e.when.is_some() {
            let c = Some(ConditionId(sources.len()));
            sources.push(
                RuleSource::new(format!("/flow/edges/{i}/when"), None)
                    .unwrap_or_else(|e| panic!("{e}")),
            );
            c
        } else {
            None
        };
        edges.push(IndexedEdge {
            from: question_index[&e.from],
            to: question_index.get(&e.to).copied(),
            condition,
        });
    }
    for (i, c) in product.pricing.components.iter().enumerate() {
        let (id, when) = match c {
            PriceComponent::Fixed { id, when, .. } | PriceComponent::Quantity { id, when, .. } => {
                (id, when)
            }
        };
        if when.is_some() {
            sources.push(
                RuleSource::new(format!("/pricing/components/{i}/when"), Some(id))
                    .unwrap_or_else(|e| panic!("{e}")),
            );
        }
    }
    for (i, s) in product.visual.slots.iter().enumerate() {
        for c in 0..s.cases.len() {
            sources.push(
                RuleSource::new(format!("/visual/slots/{i}/cases/{c}/when"), Some(&s.id))
                    .unwrap_or_else(|e| panic!("{e}")),
            );
        }
    }
    let successors = (0..product.questions.len())
        .map(|i| {
            let mut reached = BTreeSet::new();
            let mut pending = vec![i];
            while let Some(n) = pending.pop() {
                for e in edges.iter().filter(|e| e.from == n) {
                    if let Some(to) = e.to
                        && reached.insert(to)
                    {
                        pending.push(to);
                    }
                }
            }
            reached.into_iter().collect()
        })
        .collect();
    let mut target_order: Vec<_> = (0..product.visual.targets.len()).collect();
    target_order.sort_by_key(|i| {
        (
            product.visual.targets[*i]
                .order
                .map(|n| n.get())
                .unwrap_or(0),
            product.visual.targets[*i].id.clone(),
        )
    });
    let count = sources.len();
    CompiledModel::new(ModelParts {
        product,
        question_index,
        edges,
        successors,
        target_order,
        sources,
        program: Arc::new(Controlled {
            count,
            response: Arc::new(response),
            cost,
        }),
        digest: format!("sha256:{}", "0".repeat(64)),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}
pub fn set(state: &ConfigurationState, question: &str, value: Answer) -> ConfigurationEvent {
    ConfigurationEvent::SetAnswer {
        schema_version: Version::V1,
        question_id: id(question),
        expected_revision: state.revision,
        value,
    }
}
pub fn clear(state: &ConfigurationState, question: &str) -> ConfigurationEvent {
    ConfigurationEvent::ClearAnswer {
        schema_version: Version::V1,
        question_id: id(question),
        expected_revision: state.revision,
    }
}
pub fn skip(state: &ConfigurationState, question: &str) -> ConfigurationEvent {
    ConfigurationEvent::SkipAnswer {
        schema_version: Version::V1,
        question_id: id(question),
        expected_revision: state.revision,
    }
}
pub fn answer(
    model: &CompiledModel,
    state: &ConfigurationState,
    q: &str,
    value: Answer,
) -> TransitionResult {
    knot_core::apply(model, state, &set(state, q, value)).unwrap_or_else(|e| panic!("{e}"))
}
pub fn fails(
    model: &CompiledModel,
    state: &ConfigurationState,
    event: &ConfigurationEvent,
    code: &str,
) {
    let before = serde_json::to_vec(&(state, event)).unwrap_or_else(|e| panic!("{e}"));
    let error = knot_core::apply(model, state, event)
        .err()
        .unwrap_or_else(|| panic!("expected {code}"));
    assert_eq!(error.code, code);
    assert_eq!(
        before,
        serde_json::to_vec(&(state, event)).unwrap_or_else(|e| panic!("{e}"))
    );
}
pub fn initial(model: &CompiledModel) -> TransitionResult {
    knot_core::initial(model).unwrap_or_else(|e| panic!("{e}"))
}
pub fn single_fact(facts: &Facts, index: usize, value: &str) -> bool {
    matches!(&facts.values()[index],Some(knot_rules::FactValue::Scalar(knot_rules::Scalar::Single(id))) if id.as_str()==value)
}
