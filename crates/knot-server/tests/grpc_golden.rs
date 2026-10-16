#![allow(clippy::unwrap_used, clippy::expect_used)] // Test assertions.
use knot_server::{
    admission::Admission,
    catalog::Catalog,
    lifecycle::Lifecycle,
    protocol::{
        self, configuration_service_client::ConfigurationServiceClient,
        configuration_service_server::ConfigurationServiceServer,
    },
    service::Service,
    transport::Incoming,
};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc, time::Duration};
fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden")
}
fn compare(mut actual: Value, mut expected: Value, id: &str) {
    if expected.get("error").is_some() {
        assert!(
            actual["error"]["message"]
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.len() <= 16384)
        );
        actual["error"].as_object_mut().unwrap().remove("message");
        expected["error"].as_object_mut().unwrap().remove("message");
    }
    assert_eq!(actual, expected, "{id}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frozen_runtime_corpus_over_tcp() {
    let root = corpus();
    let index: Value = serde_json::from_slice(&fs::read(root.join("index.json")).unwrap()).unwrap();
    let mut catalog = Catalog::new();
    for case in index["cases"].as_array().unwrap() {
        let dir = root.join(case["id"].as_str().unwrap());
        if let Ok(model) = knot_compiler::compile_product(
            &fs::read(dir.join("model.json")).unwrap(),
            &knot_rules_zen::ZenCompiler,
        ) {
            catalog
                .entry(model.digest().to_owned())
                .or_insert_with(|| Arc::new(model));
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let life = Lifecycle::new(4);
    let service = ConfigurationServiceServer::new(Service {
        catalog: Arc::new(catalog),
        lifecycle: life.clone(),
    })
    .max_decoding_message_size(1048576)
    .max_encoding_message_size(4195328);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .layer(Admission::new(life, Duration::from_secs(30)))
            .add_service(service)
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let mut client = ConfigurationServiceClient::connect(format!("http://{address}"))
        .await
        .unwrap()
        .max_decoding_message_size(4195328);
    let mut runtime_cases = 0;
    for case in index["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let dir = root.join(id);
        if case["operation"] == "model_create" {
            continue;
        }
        let model = knot_compiler::compile_product(
            &fs::read(dir.join("model.json")).unwrap(),
            &knot_rules_zen::ZenCompiler,
        )
        .unwrap();
        let digest = Some(model.digest().to_owned());
        let reply = match case["operation"].as_str().unwrap() {
            "initial" => {
                client
                    .initial(protocol::InitialRequest {
                        model_digest: digest,
                    })
                    .await
            }
            "apply" => {
                client
                    .apply(protocol::ApplyRequest {
                        model_digest: digest,
                        state_json: Some(fs::read(dir.join("initial-state.json")).unwrap()),
                        event_json: Some(fs::read(dir.join("event.json")).unwrap()),
                    })
                    .await
            }
            "evaluate" => {
                client
                    .evaluate(protocol::EvaluateRequest {
                        model_digest: digest,
                        state_json: Some(fs::read(dir.join("initial-state.json")).unwrap()),
                    })
                    .await
            }
            other => panic!("unsupported {other}"),
        }
        .unwrap()
        .into_inner();
        let (bytes, is_error) = match reply.outcome.unwrap() {
            protocol::configuration_response::Outcome::ResultJson(b) => (b, false),
            protocol::configuration_response::Outcome::ErrorJson(b) => (b, true),
        };
        assert_eq!(is_error, case["expectedStatus"] != 0, "{id}");
        compare(
            serde_json::from_slice(&bytes).unwrap(),
            serde_json::from_slice(&fs::read(dir.join("expected-result.json")).unwrap()).unwrap(),
            id,
        );
        runtime_cases += 1;
    }
    assert!(runtime_cases > 25);
    assert_eq!(
        client
            .initial(protocol::InitialRequest { model_digest: None })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    assert_eq!(
        client
            .initial(protocol::InitialRequest {
                model_digest: Some(format!("sha256:{}", "0".repeat(64)))
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::NotFound
    );
    // Actual HTTP/2 gRPC framing, bypassing generated client validation.
    for (payload, declared, expected_status) in [
        (vec![0x0a, 0x01, 0xff], 3u32, "3"),
        (vec![], 1048577u32, "8"),
    ] {
        let stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let (mut wire, connection) = h2::client::handshake(stream).await.unwrap();
        let connection = tokio::spawn(connection);
        let request = tonic::codegen::http::Request::builder()
            .method("POST")
            .uri(format!(
                "http://{address}/knot.v1.ConfigurationService/Initial"
            ))
            .header("content-type", "application/grpc")
            .body(())
            .unwrap();
        let (response, mut body) = wire.send_request(request, false).unwrap();
        let mut frame = vec![0];
        frame.extend_from_slice(&declared.to_be_bytes());
        frame.extend_from_slice(&payload);
        body.send_data(bytes::Bytes::from(frame), true).unwrap();
        let response = response.await.unwrap();
        let header = response.headers().get("grpc-status").cloned();
        let mut body = response.into_body();
        while let Some(data) = body.data().await {
            let data = data.unwrap();
            body.flow_control().release_capacity(data.len()).unwrap();
        }
        let trailers = body.trailers().await.unwrap();
        let status = header
            .or_else(|| trailers.and_then(|t| t.get("grpc-status").cloned()))
            .unwrap();
        assert_eq!(status, expected_status);
        drop(wire);
        connection.abort();
        let _ = connection.await;
        // A malformed message is not a process fault.
        assert_eq!(
            client
                .initial(protocol::InitialRequest { model_digest: None })
                .await
                .unwrap_err()
                .code(),
            tonic::Code::InvalidArgument
        );
    }
    drop(client);
    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}
#[test]
fn compile_failures_are_real_process_failures() {
    let root = corpus();
    let index: Value = serde_json::from_slice(&fs::read(root.join("index.json")).unwrap()).unwrap();
    let target =
        PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("explicit target required"));
    let models = target.join(format!("server-tests-models-{}", std::process::id()));
    fs::create_dir_all(&models).unwrap();
    for case in index["cases"].as_array().unwrap() {
        if case["operation"] != "model_create" || case["expectedStatus"] == 0 {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let dir = root.join(id);
        fs::copy(dir.join("model.json"), models.join("model.json")).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_knot-server"))
            .args(["serve", "--transport", "plaintext", "--model-dir"])
            .arg(&models)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(3), "{id}");
        let event: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(event["event"], "model_compile_failed");
        compare(
            event["error"].clone(),
            serde_json::from_slice(&fs::read(dir.join("expected-result.json")).unwrap()).unwrap(),
            id,
        );
    }
    fs::remove_file(models.join("model.json")).unwrap();
    fs::remove_dir(models).unwrap();
}
