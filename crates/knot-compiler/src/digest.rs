use knot_schema::{KnotError, canonical, currency, wire::Product};

pub(crate) fn digest(product: &Product) -> Result<String, KnotError> {
    canonical::model_digest(product, &currency::table_digest())
}
