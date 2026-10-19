use crate::semantic::{bad, color, decimal, kind};
use knot_rules::{
    Condition as Ir, ConditionId, Field, FieldKind, RuleSource, Scalar as IrScalar, SourceMap,
};
use knot_schema::{KnotError, wire::*};
use std::collections::BTreeMap;

pub(crate) struct Lowered {
    pub fields: Vec<FieldKind>,
    pub conditions: Vec<Ir>,
    pub sources: SourceMap,
    pub edge_conditions: Vec<Option<ConditionId>>,
}
struct FieldSpec {
    kind: FieldKind,
    options: Vec<Id>,
}
struct Context<'a> {
    index: &'a BTreeMap<Id, usize>,
    fields: Vec<FieldSpec>,
    successors: &'a [Vec<usize>],
}
// Availability cannot read itself; outgoing edges can. Other branches need not
// dominate this question and remain legitimate missing facts at evaluation.
#[derive(Clone, Copy)]
enum Scope {
    Availability(usize),
    Edge(usize),
    Any,
}

pub(crate) fn product(
    product: &mut Product,
    index: &BTreeMap<Id, usize>,
    successors: &[Vec<usize>],
) -> Result<Lowered, KnotError> {
    let fields: Vec<_> = product
        .questions
        .iter()
        .map(|q| FieldSpec {
            kind: kind(q),
            options: match q {
                Question::Single { options, .. } | Question::Multi { options, .. } => {
                    options.iter().map(|o| o.id.clone()).collect()
                }
                _ => vec![],
            },
        })
        .collect();
    let context = Context {
        index,
        fields,
        successors,
    };
    let mut result = Lowered {
        fields: context.fields.iter().map(|f| f.kind).collect(),
        conditions: vec![],
        sources: vec![],
        edge_conditions: vec![],
    };
    for (q, question) in product.questions.iter_mut().enumerate() {
        if let Question::Single { options, .. } | Question::Multi { options, .. } = question {
            for (o, option) in options.iter_mut().enumerate() {
                if let Some(c) = &mut option.enabled_when {
                    result.push(
                        &context,
                        c,
                        format!("/questions/{q}/options/{o}/enabledWhen"),
                        None,
                        Scope::Availability(q),
                    )?;
                }
            }
        }
    }
    for (i, edge) in product.flow.edges.iter_mut().enumerate() {
        let id = if let Some(c) = &mut edge.when {
            let q = *index
                .get(&edge.from)
                .ok_or_else(|| bad(&format!("/flow/edges/{i}/from")))?;
            Some(result.push(
                &context,
                c,
                format!("/flow/edges/{i}/when"),
                None,
                Scope::Edge(q),
            )?)
        } else {
            None
        };
        result.edge_conditions.push(id);
    }
    for (i, component) in product.pricing.components.iter_mut().enumerate() {
        let (id, when) = match component {
            PriceComponent::Fixed { id, when, .. } | PriceComponent::Quantity { id, when, .. } => {
                (id, when)
            }
        };
        if let Some(c) = when {
            result.push(
                &context,
                c,
                format!("/pricing/components/{i}/when"),
                Some(id),
                Scope::Any,
            )?;
        }
    }
    for (s, slot) in product.visual.slots.iter_mut().enumerate() {
        for (c, case) in slot.cases.iter_mut().enumerate() {
            let when = match case {
                VisualCase::Constant { when, .. } | VisualCase::FromAnswer { when, .. } => when,
            };
            result.push(
                &context,
                when,
                format!("/visual/slots/{s}/cases/{c}/when"),
                Some(&slot.id),
                Scope::Any,
            )?;
        }
    }
    Ok(result)
}
impl Lowered {
    fn push(
        &mut self,
        context: &Context<'_>,
        c: &mut Condition,
        path: String,
        explicit: Option<&Id>,
        scope: Scope,
    ) -> Result<ConditionId, KnotError> {
        let source = RuleSource::new(path.clone(), explicit)?;
        let condition = context.node(c, &path, scope).map_err(|mut e| {
            e.rule_ids = vec![source.rule_id().into()];
            e
        })?;
        let id = ConditionId(self.conditions.len());
        self.conditions.push(condition);
        self.sources.push(source);
        Ok(id)
    }
}
impl Context<'_> {
    fn field(&self, id: &Id, path: &str, scope: Scope) -> Result<Field, KnotError> {
        let index = *self.index.get(id).ok_or_else(|| bad(path))?;
        let forbidden = match scope {
            Scope::Availability(q) => {
                index == q || self.successors[q].binary_search(&index).is_ok()
            }
            Scope::Edge(q) => index != q && self.successors[q].binary_search(&index).is_ok(),
            Scope::Any => false,
        };
        if forbidden {
            return Err(bad(path));
        }
        Ok(Field {
            index,
            kind: self.fields[index].kind,
        })
    }
    fn scalar(&self, field: Field, value: &mut Scalar, path: &str) -> Result<IrScalar, KnotError> {
        match (field.kind, value) {
            (FieldKind::Single, Scalar::Text(s)) => {
                let id = Id::new(s.clone()).map_err(|_| bad(path))?;
                if !self.fields[field.index].options.contains(&id) {
                    return Err(bad(path));
                }
                Ok(IrScalar::Single(id))
            }
            (FieldKind::Integer, Scalar::Integer(n)) => Ok(IrScalar::Integer(*n)),
            (FieldKind::Decimal, Scalar::Text(s)) => decimal(s, path).map(IrScalar::Decimal),
            (FieldKind::Color, Scalar::Text(s)) => {
                color(s, path)?;
                Ok(IrScalar::Color(s.clone()))
            }
            _ => Err(bad(path)),
        }
    }
    fn node(&self, c: &mut Condition, path: &str, scope: Scope) -> Result<Ir, KnotError> {
        match c {
            Condition::All { all } => all
                .iter_mut()
                .enumerate()
                .map(|(i, c)| self.node(c, &format!("{path}/all/{i}"), scope))
                .collect::<Result<_, _>>()
                .map(Ir::All),
            Condition::Any { any } => any
                .iter_mut()
                .enumerate()
                .map(|(i, c)| self.node(c, &format!("{path}/any/{i}"), scope))
                .collect::<Result<_, _>>()
                .map(Ir::Any),
            Condition::Not { not } => self
                .node(not, &format!("{path}/not"), scope)
                .map(|c| Ir::Not(Box::new(c))),
            Condition::Leaf(leaf) => self.leaf(leaf, path, scope),
        }
    }
    fn leaf(&self, leaf: &mut Leaf, path: &str, scope: Scope) -> Result<Ir, KnotError> {
        let id = match &*leaf {
            Leaf::Eq { field, .. }
            | Leaf::Ne { field, .. }
            | Leaf::Gt { field, .. }
            | Leaf::Gte { field, .. }
            | Leaf::Lt { field, .. }
            | Leaf::Lte { field, .. }
            | Leaf::Between { field, .. }
            | Leaf::In { field, .. }
            | Leaf::Contains { field, .. }
            | Leaf::Exists { field } => field,
        };
        let f = self.field(id, &format!("{path}/field"), scope)?;
        let numeric = matches!(f.kind, FieldKind::Integer | FieldKind::Decimal);
        if matches!(
            leaf,
            Leaf::Gt { .. }
                | Leaf::Gte { .. }
                | Leaf::Lt { .. }
                | Leaf::Lte { .. }
                | Leaf::Between { .. }
        ) && !numeric
        {
            return Err(bad(&format!("{path}/op")));
        }
        let value_path = format!("{path}/value");
        Ok(match leaf {
            Leaf::Eq { value, .. } => Ir::Eq(f, self.scalar(f, value, &value_path)?),
            Leaf::Ne { value, .. } => Ir::Ne(f, self.scalar(f, value, &value_path)?),
            Leaf::Gt { value, .. } => Ir::Gt(f, self.scalar(f, value, &value_path)?),
            Leaf::Gte { value, .. } => Ir::Gte(f, self.scalar(f, value, &value_path)?),
            Leaf::Lt { value, .. } => Ir::Lt(f, self.scalar(f, value, &value_path)?),
            Leaf::Lte { value, .. } => Ir::Lte(f, self.scalar(f, value, &value_path)?),
            Leaf::Between { value, .. } => {
                let lo = self.scalar(f, &mut value[0], &format!("{value_path}/0"))?;
                let hi = self.scalar(f, &mut value[1], &format!("{value_path}/1"))?;
                let ordered = match (&lo, &hi) {
                    (IrScalar::Integer(a), IrScalar::Integer(b)) => a <= b,
                    (IrScalar::Decimal(a), IrScalar::Decimal(b)) => a <= b,
                    _ => false,
                };
                if !ordered {
                    return Err(bad(&value_path));
                }
                Ir::Between(f, lo, hi)
            }
            Leaf::In { value, .. } => Ir::In(
                f,
                value
                    .iter_mut()
                    .enumerate()
                    .map(|(i, v)| self.scalar(f, v, &format!("{value_path}/{i}")))
                    .collect::<Result<_, _>>()?,
            ),
            Leaf::Contains { value, .. } => {
                if f.kind != FieldKind::Multi {
                    return Err(bad(&format!("{path}/op")));
                }
                if !self.fields[f.index].options.contains(value) {
                    return Err(bad(&value_path));
                }
                Ir::Contains(f, value.clone())
            }
            Leaf::Exists { .. } => Ir::Exists(f),
        })
    }
}
