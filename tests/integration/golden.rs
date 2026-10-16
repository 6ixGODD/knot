//! Independent frozen expectations through the production compiler and ZEN.
#![allow(clippy::unwrap_used, clippy::expect_used)] // Contract test assertions only.
use knot_schema::{
    protocol,
    wire::{ModelCreateResult, Version},
};
use serde_json::Value;
use std::{fs, path::PathBuf};

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden")
}
fn compare(mut actual: Value, mut expected: Value, id: &str) {
    if expected.get("error").is_some() {
        assert!(
            actual["error"]["message"]
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.len() <= 16384),
            "{id}"
        );
        actual["error"].as_object_mut().unwrap().remove("message");
        expected["error"].as_object_mut().unwrap().remove("message");
    }
    assert_eq!(actual, expected, "{id}");
}

#[test]
fn full_frozen_corpus_real_zen() {
    let root = corpus();
    let index: Value = serde_json::from_slice(&fs::read(root.join("index.json")).unwrap()).unwrap();
    assert_eq!(index["cases"].as_array().unwrap().len(), 43);
    for case in index["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let dir = root.join(id);
        let result = (|| {
            let model = knot_compiler::compile_product(
                &fs::read(dir.join("model.json")).unwrap(),
                &knot_rules_zen::ZenCompiler,
            )?;
            let value = match case["operation"].as_str().unwrap() {
                "model_create" => serde_json::to_value(ModelCreateResult {
                    schema_version: Version::V1,
                    product_id: model.product().product_id.clone(),
                    product_version: model.product().product_version.clone(),
                    model_digest: model.digest().into(),
                    diagnostics: model.diagnostics().to_vec(),
                })
                .unwrap(),
                "initial" => serde_json::to_value(knot_core::initial(&model)?).unwrap(),
                "apply" => {
                    let state =
                        protocol::state(&fs::read(dir.join("initial-state.json")).unwrap())?;
                    let event = protocol::event(&fs::read(dir.join("event.json")).unwrap())?;
                    serde_json::to_value(knot_core::apply(&model, &state, &event)?).unwrap()
                }
                "evaluate" => {
                    let state =
                        protocol::state(&fs::read(dir.join("initial-state.json")).unwrap())?;
                    let state = protocol::state(&protocol::encode(&state)?)?;
                    serde_json::to_value(knot_core::evaluate(&model, &state)?).unwrap()
                }
                other => panic!("unsupported operation {other}"),
            };
            Ok::<_, knot_schema::KnotError>(value)
        })();
        let (status, value) = match result {
            Ok(value) => (0, value),
            Err(error) => (error.status(), error.envelope()),
        };
        assert_eq!(
            status,
            case["expectedStatus"].as_i64().unwrap() as i32,
            "{id}"
        );
        compare(
            value,
            serde_json::from_slice(&fs::read(dir.join("expected-result.json")).unwrap()).unwrap(),
            id,
        );
    }
}

#[test]
fn shared_model_inside_host_runtimes() {
    let bytes = fs::read(corpus().join("T22/model.json")).unwrap();
    let model = knot_compiler::compile_product(&bytes, &knot_rules_zen::ZenCompiler).unwrap();
    let a = protocol::state(&fs::read(corpus().join("T22/initial-state.json")).unwrap()).unwrap();
    let b = knot_core::initial(&model).unwrap().state;
    let expected_a = knot_core::evaluate(&model, &a).unwrap();
    let expected_b = knot_core::evaluate(&model, &b).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for i in 0..100 {
                    let (state, expected) = if i % 2 == 0 {
                        (&a, &expected_a)
                    } else {
                        (&b, &expected_b)
                    };
                    assert_eq!(knot_core::evaluate(&model, state).unwrap(), *expected);
                }
            });
        }
    });
    for runtime in [
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap(),
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .build()
            .unwrap(),
    ] {
        runtime.block_on(async {
            assert_eq!(knot_core::evaluate(&model, &a).unwrap(), expected_a);
        });
    }
}
