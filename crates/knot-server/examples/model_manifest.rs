//! Generate a package's model identities using the same compiler and backend.
use std::{io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        return Err("provide model paths".into());
    }
    let mut models = Vec::new();
    for path in paths {
        let model =
            knot_compiler::compile_product(&std::fs::read(&path)?, &knot_rules_zen::ZenCompiler)?;
        models.push(serde_json::json!({"file":path.file_name().ok_or("file name")?,"modelDigest":model.digest(),"productId":model.product().product_id}));
    }
    serde_json::to_writer_pretty(std::io::stdout().lock(), &models)?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}
