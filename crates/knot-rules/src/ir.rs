//! Validated model indices, never user expression strings.
use knot_schema::{
    Decimal, KnotError,
    wire::{Id, Integer},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConditionId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Single,
    Multi,
    Integer,
    Decimal,
    Color,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub index: usize,
    pub kind: FieldKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Single(Id),
    Integer(Integer),
    Decimal(Decimal),
    Color(String),
}
impl Scalar {
    pub fn kind(&self) -> FieldKind {
        match self {
            Self::Single(_) => FieldKind::Single,
            Self::Integer(_) => FieldKind::Integer,
            Self::Decimal(_) => FieldKind::Decimal,
            Self::Color(_) => FieldKind::Color,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),
    Eq(Field, Scalar),
    Ne(Field, Scalar),
    Gt(Field, Scalar),
    Gte(Field, Scalar),
    Lt(Field, Scalar),
    Lte(Field, Scalar),
    Between(Field, Scalar, Scalar),
    In(Field, Vec<Scalar>),
    Contains(Field, Id),
    Exists(Field),
}
pub type SourceMap = Vec<String>;
impl Condition {
    /// Validate types, cardinality, indices and node budgets before backend allocation.
    pub fn validate(
        &self,
        fields: &[FieldKind],
        budget: &mut crate::CompileBudget,
        depth: usize,
    ) -> Result<(), KnotError> {
        budget.node(depth)?;
        let invalid = || KnotError::new("RULE_COMPILE", "");
        let field = |f: &Field| -> Result<(), KnotError> {
            if fields.get(f.index) == Some(&f.kind) {
                Ok(())
            } else {
                Err(invalid())
            }
        };
        let scalar = |f: &Field, s: &Scalar, numeric: bool| -> Result<(), KnotError> {
            field(f)?;
            if s.kind() != f.kind
                || (numeric && !matches!(f.kind, FieldKind::Integer | FieldKind::Decimal))
            {
                return Err(invalid());
            }
            if let Scalar::Color(c) = s
                && (c.len() != 7
                    || !c.starts_with('#')
                    || !c[1..].bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Err(invalid());
            }
            Ok(())
        };
        match self {
            Self::All(nodes) | Self::Any(nodes) => {
                if nodes.is_empty() {
                    return Err(invalid());
                }
                for node in nodes {
                    node.validate(fields, budget, depth + 1)?;
                }
                Ok(())
            }
            Self::Not(node) => node.validate(fields, budget, depth + 1),
            Self::Eq(f, s) | Self::Ne(f, s) => scalar(f, s, false),
            Self::Gt(f, s) | Self::Gte(f, s) | Self::Lt(f, s) | Self::Lte(f, s) => {
                scalar(f, s, true)
            }
            Self::Between(f, lo, hi) => {
                scalar(f, lo, true)?;
                scalar(f, hi, true)?;
                let ordered = match (lo, hi) {
                    (Scalar::Integer(a), Scalar::Integer(b)) => a <= b,
                    (Scalar::Decimal(a), Scalar::Decimal(b)) => a <= b,
                    _ => false,
                };
                if ordered { Ok(()) } else { Err(invalid()) }
            }
            Self::In(f, values) => {
                if values.is_empty() || values.len() > 256 {
                    return Err(invalid());
                }
                for s in values {
                    scalar(f, s, false)?;
                }
                Ok(())
            }
            Self::Contains(f, _) => {
                field(f)?;
                if f.kind != FieldKind::Multi {
                    return Err(invalid());
                }
                Ok(())
            }
            Self::Exists(f) => field(f),
        }
    }
}
