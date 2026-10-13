//! Typed condition contracts. Rule programs preserve requested ID order and share call fuel.
#![forbid(unsafe_code)]
pub mod budget;
pub mod facts;
pub mod ir;
pub mod program;
pub use budget::{CallBudget, CompileBudget};
pub use facts::{FactValue, Facts};
pub use ir::{Condition, ConditionId, Field, FieldKind, RuleSource, Scalar, SourceMap};
pub use program::{BooleanEffects, RuleCompiler, RuleProgram};
