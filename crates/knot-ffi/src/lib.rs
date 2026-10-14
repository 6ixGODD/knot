//! Native C ABI over the production compiler, ZEN backend and safe Core.
pub mod abi;
pub mod buffer;
mod entry;
mod error;
mod model;
pub use entry::*;
pub use model::Model;
