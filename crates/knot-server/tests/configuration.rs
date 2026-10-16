#![allow(clippy::unwrap_used)] // Test assertions.
use std::process::Command;
#[test]
fn metadata_ignores_environment_and_health_errors_are_one() {
    let executable = env!("CARGO_BIN_EXE_knot-server");
    let base = Command::new(executable)
        .arg("describe-config")
        .output()
        .unwrap();
    let secret = Command::new(executable)
        .arg("describe-config")
        .env("KNOT_TLS_KEY", "SECRET_NOT_FOR_OUTPUT")
        .env("KNOT_MAX_COMPUTE", "invalid")
        .output()
        .unwrap();
    assert!(base.status.success());
    assert_eq!(base.stdout, secret.stdout);
    assert!(secret.stderr.is_empty());
    let invalid = Command::new(executable)
        .args(["healthcheck", "--key", "SECRET_NOT_FOR_OUTPUT"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("SECRET_NOT_FOR_OUTPUT"));
    let checked = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/grpc/server-config.json");
    assert_eq!(base.stdout, std::fs::read(checked).unwrap());
}
