//! Restricted ZEN execution; no loader, scripts, custom nodes or runtime bridge.
#![forbid(unsafe_code)]
pub mod program;
pub mod sync;
pub use program::compile_probe;
mod adapter;
mod isolation;
mod lowering;
pub use adapter::ZenCompiler;
