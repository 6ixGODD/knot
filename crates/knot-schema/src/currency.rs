//! Fixed code-to-minor-unit snapshot. No runtime downloads or platform defaults.
use crate::KnotError;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::OnceLock};

const TABLE: &[u8] = include_bytes!("../../../contracts/currency/minor-units.json");
static UNITS: OnceLock<Result<BTreeMap<String, u8>, KnotError>> = OnceLock::new();

pub fn table_digest() -> String {
    format!("sha256:{:x}", Sha256::digest(TABLE))
}

/// Reject unassigned, test, no-currency, N.A. and non-uppercase codes.
pub fn minor_units(code: &str) -> Result<u8, KnotError> {
    let table = UNITS
        .get_or_init(|| serde_json::from_slice(TABLE).map_err(|_| KnotError::new("INTERNAL", "")));
    table
        .as_ref()
        .map_err(Clone::clone)?
        .get(code)
        .copied()
        .ok_or_else(|| KnotError::new("INVALID_MODEL", "/pricing/currency"))
}
