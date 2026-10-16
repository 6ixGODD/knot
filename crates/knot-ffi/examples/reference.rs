//! Direct Rust reference consumer for cross-language comparisons.
use knot_schema::{
    KnotError, protocol,
    wire::{ModelCreateResult, Version},
};
use serde_json::Value;
fn operation(args: &[String]) -> Result<Value, KnotError> {
    let read = |path: &str| std::fs::read(path).map_err(|_| KnotError::new("INTERNAL", ""));
    let model = knot_compiler::compile_product(&read(&args[1])?, &knot_rules_zen::ZenCompiler)?;
    match args[0].as_str() {
        "model_create" => serde_json::to_value(ModelCreateResult {
            schema_version: Version::V1,
            product_id: model.product().product_id.clone(),
            product_version: model.product().product_version.clone(),
            model_digest: model.digest().into(),
            diagnostics: model.diagnostics().to_vec(),
        }),
        "initial" => serde_json::to_value(knot_core::initial(&model)?),
        "evaluate" => serde_json::to_value(knot_core::evaluate(
            &model,
            &protocol::state(&read(&args[2])?)?,
        )?),
        "apply" => serde_json::to_value(knot_core::apply(
            &model,
            &protocol::state(&read(&args[2])?)?,
            &protocol::event(&read(&args[3])?)?,
        )?),
        _ => return Err(KnotError::new("INTERNAL", "")),
    }
    .map_err(|_| KnotError::new("INTERNAL", ""))
}
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return std::process::ExitCode::from(2);
    }
    let (status, result) = match operation(&args) {
        Ok(value) => (0, value),
        Err(error) => (error.status(), error.envelope()),
    };
    match protocol::encode(&serde_json::json!({"status":status,"result":result})) {
        Ok(bytes) if std::io::stdout().write_all(&bytes).is_ok() => std::process::ExitCode::SUCCESS,
        _ => std::process::ExitCode::from(1),
    }
}
