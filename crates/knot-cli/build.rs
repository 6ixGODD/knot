fn main() -> Result<(), std::env::VarError> {
    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows"
        && std::env::var("CARGO_CFG_TARGET_ENV")? == "msvc"
    {
        // A standalone CLI has no import-library consumers, even when a native
        // dependency carries dllexport annotations into the executable.
        println!("cargo:rustc-link-arg-bin=knot=/NOIMPLIB");
        println!("cargo:rustc-link-arg-bin=knot=/NOEXP");
    }
    Ok(())
}
