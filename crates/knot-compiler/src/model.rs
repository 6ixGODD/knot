use knot_rules::{ConditionId, RuleProgram, RuleSource, SourceMap};
use knot_schema::{
    KnotError,
    wire::{Id, PriceComponent, Product, Question},
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
    /// Transitive structural successors, sorted by question index (not just direct edges).
    pub successors: Vec<Vec<usize>>,
    pub target_order: Vec<usize>,
    pub sources: SourceMap,
    pub program: Arc<dyn RuleProgram>,
    pub digest: String,
}
pub struct CompiledModel {
    parts: ModelParts,
    condition_ids: BTreeMap<String, ConditionId>,
    quote_colors: BTreeMap<Id, Vec<String>>,
    quote_dependencies: Option<Vec<Vec<usize>>>,
    diagnostics: Vec<knot_schema::wire::Diagnostic>,
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
        let mut option_count = 0;
        for (index, q) in product.questions.iter().enumerate() {
            if parts.question_index.get(q.id()) != Some(&index) || q.id().as_str() == "END" {
                return Err(bad());
            }
            if let Question::Single { options, .. } | Question::Multi { options, .. } = q {
                if options.is_empty() || options.len() > 256 {
                    return Err(bad());
                }
                option_count += options.len();
            }
        }
        if option_count > 8192 {
            return Err(bad());
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
        if product.pricing.components.len() > 1024
            || product.visual.targets.len() > 256
            || product.visual.slots.len() > 2048
        {
            return Err(bad());
        }
        let mut case_count = 0;
        for slot in &product.visual.slots {
            if slot.cases.len() > 4096 {
                return Err(bad());
            }
            case_count += slot.cases.len();
        }
        if case_count > 4096 {
            return Err(bad());
        }
        let mut expected_sources = Vec::new();
        for (q, question) in product.questions.iter().enumerate() {
            if let Question::Single { options, .. } | Question::Multi { options, .. } = question {
                for (o, option) in options.iter().enumerate() {
                    if option.enabled_when.is_some() {
                        expected_sources.push(RuleSource::new(
                            format!("/questions/{q}/options/{o}/enabledWhen"),
                            None,
                        )?);
                    }
                }
            }
        }
        for (i, edge) in product.flow.edges.iter().enumerate() {
            if edge.when.is_some() {
                expected_sources.push(RuleSource::new(format!("/flow/edges/{i}/when"), None)?);
            }
        }
        for (i, component) in product.pricing.components.iter().enumerate() {
            let (id, when) = match component {
                PriceComponent::Fixed { id, when, .. }
                | PriceComponent::Quantity { id, when, .. } => (id, when),
            };
            if when.is_some() {
                expected_sources.push(RuleSource::new(
                    format!("/pricing/components/{i}/when"),
                    Some(id),
                )?);
            }
        }
        for (s, slot) in product.visual.slots.iter().enumerate() {
            for c in 0..slot.cases.len() {
                expected_sources.push(RuleSource::new(
                    format!("/visual/slots/{s}/cases/{c}/when"),
                    Some(&slot.id),
                )?);
            }
        }
        if expected_sources != parts.sources {
            return Err(bad());
        }
        if parts
            .sources
            .iter()
            .map(|s| s.path())
            .collect::<BTreeSet<_>>()
            .len()
            != parts.sources.len()
        {
            return Err(bad());
        }
        let mut successors = vec![BTreeSet::new(); count];
        for (index, (edge, wire)) in parts.edges.iter().zip(&product.flow.edges).enumerate() {
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
            if let Some(id) = edge.condition
                && parts.sources[id.0].path() != format!("/flow/edges/{index}/when")
            {
                return Err(bad());
            }
            if let Some(to) = edge.to {
                successors[edge.from].insert(to);
            }
        }
        // Each vertex is visited at most once per source; count <= 256, edges <= 2048.
        // The stored closure is used by dependency validation, so direct-only sets
        // would incorrectly permit reads of questions two or more steps ahead.
        for (source, stored) in parts.successors.iter().enumerate() {
            let mut reachable = BTreeSet::new();
            let mut pending: Vec<_> = successors[source].iter().copied().collect();
            while let Some(next) = pending.pop() {
                if reachable.insert(next) {
                    pending.extend(successors[next].iter().copied());
                }
            }
            if reachable.contains(&source) || stored.iter().copied().ne(reachable) {
                return Err(bad());
            }
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
        let condition_ids = parts
            .sources
            .iter()
            .enumerate()
            .map(|(i, source)| (source.path().to_owned(), ConditionId(i)))
            .collect();
        let start = parts.question_index[&product.flow.start];
        let diagnostics = product
            .questions
            .iter()
            .enumerate()
            .filter(|(i, q)| {
                *i != start
                    && !crate::semantic::required(q)
                    && parts.successors[start].binary_search(i).is_err()
            })
            .map(|(i, _)| knot_schema::wire::Diagnostic {
                level: knot_schema::wire::DiagnosticLevel::Warning,
                code: "UNREACHABLE_QUESTION".into(),
                path: format!("/questions/{i}"),
                rule_ids: vec![],
            })
            .collect();
        let quote_colors = crate::quote_domains::colors(product);
        Ok(Self {
            quote_colors,
            quote_dependencies: None,
            parts,
            condition_ids,
            diagnostics,
        })
    }
    pub(crate) fn install_quote_dependencies(&mut self, dependencies: Vec<Vec<usize>>) {
        self.quote_dependencies = Some(dependencies);
    }
    /// Dependency proof from the exact IR passed to the backend; absent for ModelParts.
    pub fn quote_dependencies(&self, id: ConditionId) -> Option<&[usize]> {
        self.quote_dependencies
            .as_ref()?
            .get(id.0)
            .map(Vec::as_slice)
    }
    /// Proven color equivalence classes for completion search.
    pub fn quote_colors(&self, id: &Id) -> Option<&[String]> {
        self.quote_colors.get(id).map(Vec::as_slice)
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
    /// Resolve a compiled condition by its Product JSON Pointer, never by a
    /// potentially shared diagnostic rule ID. None means no condition at that path.
    pub fn condition_id(&self, path: &str) -> Option<ConditionId> {
        self.condition_ids.get(path).copied()
    }
    pub fn program(&self) -> &dyn RuleProgram {
        self.parts.program.as_ref()
    }
    pub fn digest(&self) -> &str {
        &self.parts.digest
    }
    /// Compile-time warnings in definition order (at most 255 optional islands).
    /// Existing typed assembly and model accessors keep their frozen signatures.
    pub fn diagnostics(&self) -> &[knot_schema::wire::Diagnostic] {
        &self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knot_rules::{BooleanEffects, CallBudget, Facts};
    use knot_schema::wire::*;
    struct StorageOnlyProgram(usize);
    impl RuleProgram for StorageOnlyProgram {
        fn condition_count(&self) -> usize {
            self.0
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
            program: Arc::new(StorageOnlyProgram(0)),
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

    #[test]
    fn successor_storage_is_transitive_and_cycle_free() {
        let mut p = parts();
        for name in ["second", "third"] {
            let index = p.product.questions.len();
            p.question_index.insert(id(name), index);
            p.product.questions.push(Question::Color {
                id: id(name),
                label: name.into(),
                required: Some(true),
            });
        }
        p.product.flow.edges = vec![
            Edge {
                from: id("color"),
                to: id("second"),
                when: None,
            },
            Edge {
                from: id("second"),
                to: id("third"),
                when: None,
            },
            Edge {
                from: id("third"),
                to: id("END"),
                when: None,
            },
        ];
        p.edges = vec![
            IndexedEdge {
                from: 0,
                to: Some(1),
                condition: None,
            },
            IndexedEdge {
                from: 1,
                to: Some(2),
                condition: None,
            },
            IndexedEdge {
                from: 2,
                to: None,
                condition: None,
            },
        ];
        p.successors = vec![vec![1, 2], vec![2], vec![]];
        let model = CompiledModel::new(p).unwrap_or_else(|e| panic!("{e}"));
        let mut p = model.parts;
        p.successors[0] = vec![1];
        assert!(CompiledModel::new(p).is_err());
        let mut p = parts();
        p.product.flow.edges[0].to = id("color");
        p.edges[0].to = Some(0);
        p.successors[0] = vec![0];
        assert!(CompiledModel::new(p).is_err());
    }

    #[test]
    fn source_table_and_edge_ids_agree_with_product_locations() {
        let mut p = parts();
        let condition = knot_schema::wire::Condition::Leaf(Leaf::Exists { field: id("color") });
        p.product.flow.edges[0].when = Some(condition.clone());
        p.edges[0].condition = Some(ConditionId(0));
        p.product.pricing.components = vec![PriceComponent::Fixed {
            id: id("base"),
            category: PriceCategory::Base,
            amount_minor: Integer::new(100).unwrap_or_else(|e| panic!("{e}")),
            when: Some(condition),
        }];
        p.sources = vec![
            RuleSource::new("/flow/edges/0/when".into(), None).unwrap_or_else(|e| panic!("{e}")),
            RuleSource::new("/pricing/components/0/when".into(), Some(&id("base")))
                .unwrap_or_else(|e| panic!("{e}")),
        ];
        p.program = Arc::new(StorageOnlyProgram(2));
        let model = CompiledModel::new(p).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            model.condition_id("/pricing/components/0/when"),
            Some(ConditionId(1))
        );
        assert_eq!(model.condition_id("base"), None);
        let mut p = model.parts;
        p.edges[0].condition = Some(ConditionId(1));
        assert!(CompiledModel::new(p).is_err());
    }
}
