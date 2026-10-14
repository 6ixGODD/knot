//! Only internal numeric indices enter expression syntax. All constants are input data.
use knot_rules::{Condition, Field, Scalar};
use knot_schema::KnotError;
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn scalar(value: &Scalar) -> Result<Value, KnotError> {
    match value {
        Scalar::Single(id) => Ok(Value::String(id.as_str().into())),
        Scalar::Color(color) => Ok(Value::String(color.to_ascii_uppercase())),
        Scalar::Integer(n) => Ok(Value::Number(n.get().into())),
        Scalar::Decimal(n) => n
            .to_string()
            .parse::<serde_json::Number>()
            .map(Value::Number)
            .map_err(|_| KnotError::new("RULE_COMPILE", "")),
    }
}
pub(crate) struct Lowered {
    pub expression: String,
    pub constants: Vec<Value>,
    pub cost: u64,
    pub fields: Vec<usize>,
}
pub(crate) fn lower(condition: &Condition) -> Result<Lowered, KnotError> {
    let mut constants = vec![];
    let mut cost = 256;
    let mut fields = BTreeSet::new();
    let expression = node(condition, &mut constants, &mut cost, &mut fields)?;
    // Expression parsing/graph work and immutable constant conversion are also prepaid.
    cost += expression.len() as u64 * 8
        + constants
            .iter()
            .map(|v| v.to_string().len() as u64 * 8)
            .sum::<u64>();
    Ok(Lowered {
        expression,
        constants,
        cost,
        fields: fields.into_iter().collect(),
    })
}
fn comparison(
    field: Field,
    value: &Scalar,
    op: &str,
    constants: &mut Vec<Value>,
    cost: &mut u64,
    fields: &mut BTreeSet<usize>,
) -> Result<String, KnotError> {
    fields.insert(field.index);
    let index = constants.len();
    constants.push(scalar(value)?);
    *cost += 256; // >= 128 UTF-8 bytes, fixed decimal compare and indexing/boolean overhead.
    Ok(format!(
        "(p[{}] and v[{}] {op} c[{index}])",
        field.index, field.index
    ))
}
fn node(
    condition: &Condition,
    constants: &mut Vec<Value>,
    cost: &mut u64,
    fields: &mut BTreeSet<usize>,
) -> Result<String, KnotError> {
    *cost += 16;
    Ok(match condition {
        Condition::All(nodes) | Condition::Any(nodes) => {
            let op = if matches!(condition, Condition::All(_)) {
                " and "
            } else {
                " or "
            };
            let parts = nodes
                .iter()
                .map(|n| node(n, constants, cost, fields))
                .collect::<Result<Vec<_>, _>>()?;
            balanced(&parts, op)
        }
        Condition::Not(n) => format!("not({})", node(n, constants, cost, fields)?),
        Condition::Exists(f) => {
            fields.insert(f.index);
            format!("p[{}]", f.index)
        }
        Condition::Eq(f, v) => comparison(*f, v, "==", constants, cost, fields)?,
        Condition::Ne(f, v) => comparison(*f, v, "!=", constants, cost, fields)?,
        Condition::Gt(f, v) => comparison(*f, v, ">", constants, cost, fields)?,
        Condition::Gte(f, v) => comparison(*f, v, ">=", constants, cost, fields)?,
        Condition::Lt(f, v) => comparison(*f, v, "<", constants, cost, fields)?,
        Condition::Lte(f, v) => comparison(*f, v, "<=", constants, cost, fields)?,
        Condition::Between(f, lo, hi) => format!(
            "({} and {})",
            comparison(*f, lo, ">=", constants, cost, fields)?,
            comparison(*f, hi, "<=", constants, cost, fields)?
        ),
        Condition::In(f, values) => {
            let parts = values
                .iter()
                .map(|v| comparison(*f, v, "==", constants, cost, fields))
                .collect::<Result<Vec<_>, _>>()?;
            balanced(&parts, " or ")
        }
        Condition::Contains(f, id) => {
            fields.insert(f.index);
            let index = constants.len();
            constants.push(Value::String(id.as_str().into()));
            *cost += 256 * 256; // bounded full scan, 256 members of at most 128 UTF-8 bytes.
            format!("(p[{}] and contains(v[{}], c[{index}]))", f.index, f.index)
        }
    })
}

// Avoid a linear-depth backend AST for wide all/any/in nodes. Validation ensures nonempty input.
fn balanced(parts: &[String], op: &str) -> String {
    if parts.len() == 1 {
        return parts[0].clone();
    }
    let middle = parts.len() / 2;
    format!(
        "({}{}{})",
        balanced(&parts[..middle], op),
        op,
        balanced(&parts[middle..], op)
    )
}
