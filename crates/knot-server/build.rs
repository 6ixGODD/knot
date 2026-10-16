use std::{env, error::Error, path::PathBuf, process::Command};
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-changed=../../contracts/grpc/knot/v1/configuration.proto");
    println!("cargo:rerun-if-changed=../../tools/server/toolchain-lock.json");
    let protoc = env::var_os("PROTOC").ok_or("PROTOC must name the verified protoc 36.2 binary")?;
    let version = Command::new(&protoc).arg("--version").output()?;
    if !version.status.success()
        || version.stdout != b"libprotoc 36.2\n" && version.stdout != b"libprotoc 36.2\r\n"
    {
        return Err("expected libprotoc 36.2".into());
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR missing")?);
    tonic_prost_build::configure()
        .file_descriptor_set_path(out.join("knot.bin"))
        .codec_path("crate::codec::Codec")
        .compile_protos(
            &["../../contracts/grpc/knot/v1/configuration.proto"],
            &["../../contracts/grpc"],
        )?;
    Ok(())
}
