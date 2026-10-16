use knot_schema::{
    KnotError, parse, protocol,
    wire::{ModelCreateResult, Version},
};
use serde_json::json;
use std::{
    ffi::OsString,
    fs::File,
    io::{self, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

enum Failure {
    Io(io::Error),
    Usage,
    Domain(KnotError),
}
impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<KnotError> for Failure {
    fn from(e: KnotError) -> Self {
        Self::Domain(e)
    }
}

fn file(path: &Path, limit: usize) -> Result<Vec<u8>, Failure> {
    let input = File::open(path)?;
    if input.metadata()?.len() > limit as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "input byte limit exceeded").into());
    }
    Ok(crate::read_bounded(input, limit)?)
}
fn emit(out: &mut impl Write, value: &impl serde::Serialize) -> Result<(), Failure> {
    let bytes = protocol::encode(value)?;
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
fn status(error: &KnotError) -> u8 {
    if error.status() == 1999 { 4 } else { 3 }
}

fn command(args: &[OsString], out: &mut impl Write, step: &mut usize) -> Result<u8, Failure> {
    let Some(name) = args.first().and_then(|s| s.to_str()) else {
        return Err(Failure::Usage);
    };
    let valid = match name {
        "validate" | "inspect" => args.len() == 2,
        "simulate" => args.len() == 4 && args[2] == "--events",
        "explain" => args.len() == 4 && args[2] == "--state",
        _ => false,
    };
    if !valid {
        return Err(Failure::Usage);
    }
    let model = knot_compiler::compile_product(
        &file(Path::new(&args[1]), parse::PRODUCT_BYTES)?,
        &knot_rules_zen::ZenCompiler,
    )?;
    match name {
        "validate" => emit(
            out,
            &ModelCreateResult {
                schema_version: Version::V1,
                product_id: model.product().product_id.clone(),
                product_version: model.product().product_version.clone(),
                model_digest: model.digest().into(),
                diagnostics: model.diagnostics().to_vec(),
            },
        )?,
        "inspect" => emit(
            out,
            &json!({"schemaVersion":"1.0", "productId":model.product().product_id, "productVersion":model.product().product_version, "modelDigest":model.digest(), "questionCount":model.product().questions.len(), "ruleCount":model.sources().len(), "questions":model.product().questions, "flow":model.product().flow, "pricing":model.product().pricing, "visual":model.product().visual, "diagnostics":model.diagnostics()}),
        )?,
        "explain" => emit(
            out,
            &knot_core::evaluate(
                &model,
                &protocol::state(&file(Path::new(&args[3]), parse::STATE_BYTES)?)?,
            )?,
        )?,
        "simulate" => {
            let bytes = file(Path::new(&args[3]), 16 * 1024 * 1024)?;
            let events = knot_schema::parse_json(&bytes, 16 * 1024 * 1024)?;
            let events = events
                .as_array()
                .ok_or_else(|| KnotError::new("BAD_SCHEMA", ""))?;
            if events.len() > 10_000 {
                return Err(KnotError::limit("simulateEvents", 10_000, events.len()).into());
            }
            let initial = match knot_core::initial(&model) {
                Ok(value) => value,
                Err(error) => {
                    emit(
                        out,
                        &json!({"schemaVersion":"1.0","step":0,"result":error.envelope()}),
                    )?;
                    return Ok(status(&error));
                }
            };
            emit(
                out,
                &json!({"schemaVersion":"1.0","step":0,"result":initial}),
            )?;
            let mut state = initial.state;
            for (i, value) in events.iter().enumerate() {
                *step = i + 1;
                let result = protocol::encode(value)
                    .and_then(|bytes| protocol::event(&bytes))
                    .and_then(|event| knot_core::apply(&model, &state, &event));
                match result {
                    Ok(value) => {
                        emit(
                            out,
                            &json!({"schemaVersion":"1.0","step":i+1,"result":value}),
                        )?;
                        state = value.state;
                    }
                    Err(error) => {
                        emit(
                            out,
                            &json!({"schemaVersion":"1.0","step":i+1,"result":error.envelope()}),
                        )?;
                        return Ok(status(&error));
                    }
                }
            }
        }
        _ => return Err(Failure::Usage),
    }
    Ok(0)
}

/// Run one command. All business diagnostics are JSON; usage and I/O use stderr.
/// A failed output write always produces a nonzero exit status.
pub fn run(args: &[OsString], out: &mut impl Write, err: &mut impl Write) -> u8 {
    let simulation = args.first().is_some_and(|arg| arg == "simulate");
    let mut step = 0;
    let result = catch_unwind(AssertUnwindSafe(|| command(args, out, &mut step)));
    match result {
        Ok(Ok(code)) => code,
        Ok(Err(Failure::Usage)) => {
            let _ = writeln!(
                err,
                "usage: knot validate|inspect <product> | simulate <product> --events <events> | explain <product> --state <state>"
            );
            2
        }
        Ok(Err(Failure::Io(error))) => {
            let _ = writeln!(err, "I/O error: {error}");
            2
        }
        Ok(Err(Failure::Domain(error))) => {
            let diagnostic = if simulation {
                json!({"schemaVersion":"1.0", "step":step, "result":error.envelope()})
            } else {
                error.envelope()
            };
            if emit(out, &diagnostic).is_err() {
                2
            } else {
                status(&error)
            }
        }
        Err(payload) => {
            if let Err(nested) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(nested);
            }
            let written = (|| -> io::Result<()> {
                if simulation {
                    write!(
                        out,
                        "{{\"schemaVersion\":\"1.0\",\"step\":{step},\"result\":"
                    )?;
                }
                out.write_all(b"{\"schemaVersion\":\"1.0\",\"error\":{\"code\":\"INTERNAL\",\"message\":\"INTERNAL\",\"path\":\"\",\"ruleIds\":[],\"details\":{}}}")?;
                out.write_all(if simulation { b"}\n" } else { b"\n" })?;
                out.flush()
            })();
            if written.is_err() { 2 } else { 4 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct PanicOnce {
        remaining: usize,
        bytes: Vec<u8>,
    }
    impl Write for PanicOnce {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.first() == Some(&b'{') && self.remaining > 0 {
                self.remaining -= 1;
                assert_ne!(self.remaining, 0, "injected CLI boundary panic");
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn internal_failure_keeps_the_current_simulation_step() -> Result<(), Box<dyn std::error::Error>>
    {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/examples");
        let args = vec![
            "simulate".into(),
            root.join("speaker.json").into_os_string(),
            "--events".into(),
            root.join("speaker-events.json").into_os_string(),
        ];
        let mut out = PanicOnce {
            remaining: 3,
            bytes: vec![],
        };
        assert_eq!(run(&args, &mut out, &mut Vec::new()), 4);
        let records = out
            .bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(serde_json::from_slice::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(records.len(), 3);
        assert_eq!(records[2]["step"], 2);
        assert_eq!(records[2]["result"]["error"]["code"], "INTERNAL");
        Ok(())
    }
}
