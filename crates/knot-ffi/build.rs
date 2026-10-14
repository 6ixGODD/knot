fn main() -> Result<(), std::env::VarError> {
    let target = std::env::var("CARGO_CFG_TARGET_OS")?;
    if target == "linux" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libknot.so");
    } else if target == "macos" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libknot.dylib");
    }
    Ok(())
}
