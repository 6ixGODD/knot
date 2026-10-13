//! Offline contract validation. All diagnostics are JSON on stdout.
use knot_schema::{KnotError, parse_json, validation::Validator};
use std::{fs::File, io::Read, path::PathBuf};

fn read(path: &PathBuf) -> Result<serde_json::Value, KnotError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| KnotError::new("BAD_JSON", ""))?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| KnotError::new("BAD_JSON", ""))?;
    parse_json(&bytes, 4 * 1024 * 1024)
}

fn run() -> Result<(), KnotError> {
    let mut args = std::env::args_os().skip(1);
    let mut schema = None;
    let mut input = None;
    while let Some(arg) = args.next() {
        let slot = if arg == "--schema" {
            &mut schema
        } else if arg == "--input" {
            &mut input
        } else {
            return Err(KnotError::new("BAD_SCHEMA", ""));
        };
        if slot.is_some() {
            return Err(KnotError::new("BAD_SCHEMA", ""));
        }
        *slot = Some(PathBuf::from(
            args.next()
                .ok_or_else(|| KnotError::new("BAD_SCHEMA", ""))?,
        ));
    }
    let schema = read(&schema.ok_or_else(|| KnotError::new("BAD_SCHEMA", ""))?)?;
    let input = read(&input.ok_or_else(|| KnotError::new("BAD_JSON", ""))?)?;
    Validator::new(&schema)?.validate(&input)
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => {
            println!("{{\"schemaVersion\":\"1.0\",\"valid\":true}}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", error.envelope());
            std::process::ExitCode::FAILURE
        }
    }
}
