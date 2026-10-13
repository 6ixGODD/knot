//! Pure state arithmetic. Full transitions are implemented after the P0 gate.
//!
//! The frozen plan03 entry signatures are `initial(&CompiledModel)`,
//! `apply(&CompiledModel, &ConfigurationState, &ConfigurationEvent)` and
//! `evaluate(&CompiledModel, &ConfigurationState)`, each returning
//! `Result<TransitionResult, KnotError>`. Model lives in knot-compiler, wire values
//! and errors in knot-schema. Inputs are borrowed, never mutated; one CallBudget
//! covers all backend calls in a transition. Initial/evaluate emit empty Changes.
#![forbid(unsafe_code)]
pub mod revision;
