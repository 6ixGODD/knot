//! Bounded Product compilation into immutable, backend-independent model storage.
#![forbid(unsafe_code)]
mod compile;
mod dependencies;
mod digest;
mod flow;
mod lower;
pub mod model;
mod semantic;
pub use compile::compile_product;
pub use model::{CompiledModel, IndexedEdge, ModelParts};

pub mod quote_domains;
