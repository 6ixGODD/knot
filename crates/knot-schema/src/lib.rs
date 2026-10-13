//! Bounded JSON, exact protocol values, and offline contract validation.
#![forbid(unsafe_code)]
pub mod canonical;

pub mod error;
pub mod parse;
pub mod validation;
pub mod value;
pub mod wire;

pub use error::KnotError;
pub use parse::parse_json;
pub use value::{Decimal, SAFE_INTEGER, exact_integer};
