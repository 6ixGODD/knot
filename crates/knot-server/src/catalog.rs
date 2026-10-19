//! Bounded, startup-only model catalog.
use knot_compiler::CompiledModel;
use knot_schema::{KnotError, parse::PRODUCT_BYTES};
use std::{
    collections::BTreeMap,
    fs::{self, File, Metadata},
    io::Read,
    path::Path,
    sync::Arc,
};
pub type Catalog = BTreeMap<String, Arc<CompiledModel>>;
fn redirect(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn invalid() -> KnotError {
    KnotError::new("INVALID_MODEL", "")
}
pub fn load(directory: &Path) -> Result<Catalog, KnotError> {
    let meta = fs::symlink_metadata(directory).map_err(|_| invalid())?;
    if !meta.is_dir() || redirect(&meta) {
        return Err(invalid());
    }
    let root = directory.canonicalize().map_err(|_| invalid())?;
    let mut paths = Vec::new();
    for (index, entry) in fs::read_dir(&root).map_err(|_| invalid())?.enumerate() {
        if index >= 1024 {
            return Err(KnotError::limit("directoryEntries", 1024, index + 1));
        }
        let entry = entry.map_err(|_| invalid())?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let meta = fs::symlink_metadata(&path).map_err(|_| invalid())?;
            if redirect(&meta)
                || !meta.is_file()
                || !path
                    .canonicalize()
                    .map_err(|_| invalid())?
                    .starts_with(&root)
            {
                return Err(invalid());
            }
            if paths.len() >= 64 {
                return Err(KnotError::limit("models", 64, paths.len() + 1));
            }
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Err(invalid());
    }
    paths.sort();
    let mut total = 0usize;
    let mut catalog = Catalog::new();
    for path in paths {
        let meta = fs::symlink_metadata(&path).map_err(|_| invalid())?;
        if redirect(&meta) || !meta.is_file() {
            return Err(invalid());
        }
        let file = File::open(&path).map_err(|_| invalid())?;
        let opened = file.metadata().map_err(|_| invalid())?;
        if !opened.is_file() || opened.len() != meta.len() {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        file.take(PRODUCT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() > PRODUCT_BYTES {
            return Err(KnotError::limit("inputBytes", PRODUCT_BYTES, bytes.len()));
        }
        total += bytes.len();
        if total > 64 * 1024 * 1024 {
            return Err(KnotError::limit("modelInputBytes", 64 * 1024 * 1024, total));
        }
        let model = knot_compiler::compile_product(&bytes, &knot_rules_zen::ZenCompiler)?;
        catalog
            .entry(model.digest().to_owned())
            .or_insert_with(|| Arc::new(model));
    }
    Ok(catalog)
}
