fn main() -> Result<(), std::env::VarError> {
    let target = std::env::var("CARGO_CFG_TARGET_OS")?;
    if target == "windows"
        && std::env::var("CARGO_CFG_TARGET_ENV")? == "msvc"
        && std::env::var("PROFILE")? == "release"
    {
        // Packaging creates the public knot.dll import library from the seven
        // ABI entries. Cargo's private-name import/EXP files are not shipped.
        println!("cargo:rustc-cdylib-link-arg=/NOIMPLIB");
        println!("cargo:rustc-cdylib-link-arg=/NOEXP");
        // Reference and benchmark executables have no import-library users.
        println!("cargo:rustc-link-arg-examples=/NOIMPLIB");
        println!("cargo:rustc-link-arg-examples=/NOEXP");
    } else if target == "linux" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libknot.so");
    } else if target == "macos" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libknot.dylib");
    }
    Ok(())
}
