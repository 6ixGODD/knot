use knot_rules::{ConditionId, RuleProgram, SourceMap};
use knot_schema::{
    KnotError,
    wire::{Id, Product},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedEdge {
    pub from: usize,
    pub to: Option<usize>,
    pub condition: Option<ConditionId>,
}
/// Explicit typed assembly, not a bypass for public JSON compilation.
pub struct ModelParts {
    pub product: Product,
    pub question_index: BTreeMap<Id, usize>,
    pub edges: Vec<IndexedEdge>,
    pub successors: Vec<Vec<usize>>,
    pub target_order: Vec<usize>,
    pub sources: SourceMap,
    pub program: Arc<dyn RuleProgram>,
    pub digest: String,
}
pub struct CompiledModel {
    parts: ModelParts,
}
impl CompiledModel {
    /// Check storage consistency before publishing read-only shared model data.
    /// Full reference/type/dependency/cycle validation occurs in the semantic compiler.
    pub fn new(parts: ModelParts) -> Result<Self, KnotError> {
        let bad = || KnotError::new("INVALID_MODEL", "");
        let product = &parts.product;
        let count = product.questions.len();
        if count == 0
            || count > 256
            || parts.question_index.len() != count
            || parts.successors.len() != count
        {
            return Err(bad());
        }
        for (index, q) in product.questions.iter().enumerate() {
            if parts.question_index.get(q.id()) != Some(&index) || q.id().as_str() == "END" {
                return Err(bad());
            }
        }
        if !parts.question_index.contains_key(&product.flow.start) {
            return Err(bad());
        }
        if parts.edges.len() != product.flow.edges.len() || parts.edges.len() > 2048 {
            return Err(bad());
        }
        if parts.sources.len() != parts.program.condition_count() || parts.sources.len() > 4096 {
            return Err(bad());
        }
        if parts
            .sources
            .iter()
            .any(|s| s.is_empty() || s.len() > 16384)
        {
            return Err(bad());
        }
        let mut successors = vec![BTreeSet::new(); count];
        for (edge, wire) in parts.edges.iter().zip(&product.flow.edges) {
            if parts.question_index.get(&wire.from) != Some(&edge.from) || edge.from >= count {
                return Err(bad());
            }
            let expected = if wire.to.as_str() == "END" {
                None
            } else {
                Some(*parts.question_index.get(&wire.to).ok_or_else(bad)?)
            };
            if expected != edge.to || wire.when.is_some() != edge.condition.is_some() {
                return Err(bad());
            }
            if edge.condition.is_some_and(|id| id.0 >= parts.sources.len()) {
                return Err(bad());
            }
            if let Some(to) = edge.to {
                successors[edge.from].insert(to);
            }
        }
        for (stored, expected) in parts.successors.iter().zip(successors) {
            if stored.iter().copied().ne(expected) {
                return Err(bad());
            }
        }
        if product.pricing.components.len() > 1024
            || product.visual.targets.len() > 256
            || product.visual.slots.len() > 2048
        {
            return Err(bad());
        }
        let mut expected: Vec<_> = (0..product.visual.targets.len()).collect();
        expected.sort_by(|a, b| {
            let a = &product.visual.targets[*a];
            let b = &product.visual.targets[*b];
            a.order
                .map(|n| n.get())
                .unwrap_or(0)
                .cmp(&b.order.map(|n| n.get()).unwrap_or(0))
                .then_with(|| a.id.as_str().as_bytes().cmp(b.id.as_str().as_bytes()))
        });
        if parts.target_order != expected {
            return Err(bad());
        }
        if parts.digest.len() != 71
            || !parts.digest.starts_with("sha256:")
            || !parts.digest[7..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(bad());
        }
        Ok(Self { parts })
    }
    pub fn product(&self) -> &Product {
        &self.parts.product
    }
    pub fn question_index(&self) -> &BTreeMap<Id, usize> {
        &self.parts.question_index
    }
    pub fn edges(&self) -> &[IndexedEdge] {
        &self.parts.edges
    }
    pub fn successors(&self) -> &[Vec<usize>] {
        &self.parts.successors
    }
    pub fn target_order(&self) -> &[usize] {
        &self.parts.target_order
    }
    pub fn sources(&self) -> &SourceMap {
        &self.parts.sources
    }
    pub fn program(&self) -> &dyn RuleProgram {
        self.parts.program.as_ref()
    }
    pub fn digest(&self) -> &str {
        &self.parts.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knot_rules::{BooleanEffects, CallBudget, Facts};
    use knot_schema::wire::*;
    struct EmptyProgram;
    impl RuleProgram for EmptyProgram {
        fn condition_count(&self) -> usize {
            0
        }
        fn evaluate(
            &self,
            _: &Facts,
            requested: &[ConditionId],
            _: &mut CallBudget,
        ) -> Result<BooleanEffects, KnotError> {
            BooleanEffects::new(requested, vec![])
        }
    }
    fn id(value: &str) -> Id {
        Id::new(value.into()).unwrap_or_else(|e| panic!("{e}"))
    }
    fn parts() -> ModelParts {
        let product = Product {
            schema_version: Version::V1,
            product_id: id("test"),
            product_version: id("1"),
            questions: vec![Question::Color {
                id: id("color"),
                label: "Color".into(),
                required: Some(true),
            }],
            flow: FlowDefinition {
                start: id("color"),
                edges: vec![Edge {
                    from: id("color"),
                    to: id("END"),
                    when: None,
                }],
            },
            pricing: Pricing {
                currency: "CNY".into(),
                components: vec![PriceComponent::Fixed {
                    id: id("base"),
                    category: PriceCategory::Base,
                    amount_minor: Integer::new(100).unwrap_or_else(|e| panic!("{e}")),
                    when: None,
                }],
            },
            visual: VisualDefinition {
                targets: vec![],
                slots: vec![],
            },
            metadata: None,
        };
        ModelParts {
            product,
            question_index: BTreeMap::from([(id("color"), 0)]),
            edges: vec![IndexedEdge {
                from: 0,
                to: None,
                condition: None,
            }],
            successors: vec![vec![]],
            target_order: vec![],
            sources: vec![],
            program: Arc::new(EmptyProgram),
            digest: format!("sha256:{}", "0".repeat(64)),
        }
    }
    #[test]
    fn typed_storage_rejects_bad_indices_conditions_and_order() {
        assert!(CompiledModel::new(parts()).is_ok());
        let mut p = parts();
        p.question_index.insert(id("color"), 1);
        assert!(CompiledModel::new(p).is_err());
        let mut p = parts();
        p.edges[0].to = Some(1);
        assert!(CompiledModel::new(p).is_err());
        let mut p = parts();
        p.edges[0].condition = Some(ConditionId(0));
        assert!(CompiledModel::new(p).is_err());
        let mut p = parts();
        p.successors[0].push(0);
        assert!(CompiledModel::new(p).is_err());
        let mut p = parts();
        p.target_order.push(0);
        assert!(CompiledModel::new(p).is_err());
        let mut p = parts();
        p.digest = "sha256:invalid".into();
        assert!(CompiledModel::new(p).is_err());
    }
}
