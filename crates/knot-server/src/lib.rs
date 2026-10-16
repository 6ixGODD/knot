//! Stateless gRPC transport for the configuration core.
#![forbid(unsafe_code)]
pub mod config;
pub mod protocol {
    tonic::include_proto!("knot.v1");
    pub const DESCRIPTOR: &[u8] = tonic::include_file_descriptor_set!("knot");
}

pub mod admission;
pub mod catalog;
pub mod codec;
pub mod health;
pub mod lifecycle;
pub mod service;
pub mod telemetry;
pub mod transport;

pub mod runtime;
