//! RFC 8785 serialization and domain-separated model identity.
use crate::{KnotError, wire::Product};
use sha2::{Digest, Sha256};

/// Input Product must have already undergone compiler normalization. Arrays retain order.
pub fn model_digest(product: &Product, currency_table_digest: &str) -> Result<String, KnotError> {
    if currency_table_digest.len() != 71
        || !currency_table_digest.starts_with("sha256:")
        || !currency_table_digest[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(KnotError::new("INVALID_MODEL", ""));
    }
    let value = serde_json::json!({"semantics":"knot-v0.1-contract-1","currencyTableDigest":currency_table_digest,"product":product});
    let bytes = serde_jcs::to_vec(&value).map_err(|_| KnotError::new("INVALID_MODEL", ""))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}
