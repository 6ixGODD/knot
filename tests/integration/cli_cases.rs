#![allow(clippy::unwrap_used)] // Subprocess test assertions.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts")
        .join(name)
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_knot"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn four_commands_and_failures() {
    assert_eq!(run(&[]).status.code(), Some(2));
    assert_eq!(
        run(&["validate", "missing-file.json"]).status.code(),
        Some(2)
    );
    let model = path("examples/speaker.json");
    let model = model.to_str().unwrap();
    for operation in ["validate", "inspect"] {
        let output = run(&[operation, model]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["productId"], "speaker-pro");
        if operation == "inspect" {
            assert_eq!(result["ruleCount"], 15);
        }
    }
    let state = path("golden/T24/initial-state.json");
    let output = run(&["explain", model, "--state", state.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let expected = std::fs::read(path("golden/T24/expected-result.json")).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&expected).unwrap()
    );
    let output = run(&["validate", path("golden/T27/model.json").to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
}

#[test]
fn closed_output_pipe_is_not_success() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_knot"))
        .arg("inspect")
        .arg(path("examples/speaker.json"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    assert!(!child.wait().unwrap().success());
}

#[test]
fn speaker_sequence_and_first_error_stop() {
    let model = path("examples/speaker.json");
    let events = path("examples/speaker-events.json");
    let output = run(&[
        "simulate",
        model.to_str().unwrap(),
        "--events",
        events.to_str().unwrap(),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<serde_json::Value> = output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|v| !v.is_empty())
        .map(|v| serde_json::from_slice(v).unwrap())
        .collect();
    assert_eq!(lines.len(), 7);
    assert_eq!(
        lines[5]["result"]["snapshot"]["price"]["totalMinor"],
        282800
    );
    assert_eq!(
        lines[6]["result"]["snapshot"]["price"]["totalMinor"],
        159900
    );
    let output_dir = PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap()).join("cli-cases");
    std::fs::create_dir_all(&output_dir).unwrap();
    let mut events: serde_json::Value =
        serde_json::from_slice(&std::fs::read(events).unwrap()).unwrap();
    events[1]["expectedRevision"] = serde_json::json!(999);
    let file = output_dir.join("first-error.json");
    std::fs::write(&file, serde_json::to_vec(&events).unwrap()).unwrap();
    let output = run(&[
        "simulate",
        model.to_str().unwrap(),
        "--events",
        file.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    let lines: Vec<serde_json::Value> = output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|v| !v.is_empty())
        .map(|v| serde_json::from_slice(v).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[2]["result"]["error"]["code"], "REVISION_CONFLICT");
}

#[test]
fn simulation_admission_and_file_limits() {
    let model = path("examples/speaker.json");
    let directory =
        PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap()).join("cli-admission");
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("events.json");
    let cases = [
        (b"[".to_vec(), "BAD_JSON"),
        (b"{}".to_vec(), "BAD_SCHEMA"),
        (
            serde_json::to_vec(&vec![serde_json::Value::Null; 10001]).unwrap(),
            "RESOURCE_LIMIT",
        ),
    ];
    for (input, code) in cases {
        std::fs::write(&file, input).unwrap();
        let output = run(&[
            "simulate",
            model.to_str().unwrap(),
            "--events",
            file.to_str().unwrap(),
        ]);
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["step"], 0);
        assert_eq!(value["result"]["error"]["code"], code);
    }
    std::fs::File::create(&file)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let output = run(&[
        "simulate",
        model.to_str().unwrap(),
        "--events",
        file.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
