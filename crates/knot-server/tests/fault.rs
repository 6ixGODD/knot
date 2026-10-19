#![allow(clippy::unwrap_used)] // Test fault injection only; not a production feature.
use knot_rules::{
    BooleanEffects, CallBudget, CompileBudget, Condition, ConditionId, Facts, FieldKind,
    RuleCompiler, RuleProgram, SourceMap,
};
use knot_schema::KnotError;
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
use prost::Message;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Gate {
    panic_second: bool,
    fault_code: Option<&'static str>,
    calls: AtomicUsize,
    started: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}
struct Compiler(Arc<Gate>);
struct Program {
    inner: Arc<dyn RuleProgram>,
    gate: Arc<Gate>,
}
impl RuleCompiler for Compiler {
    fn compile(
        &self,
        fields: &[FieldKind],
        conditions: &[Condition],
        sources: &SourceMap,
        budget: &mut CompileBudget,
    ) -> Result<Arc<dyn RuleProgram>, KnotError> {
        Ok(Arc::new(Program {
            inner: knot_rules_zen::ZenCompiler.compile(fields, conditions, sources, budget)?,
            gate: self.0.clone(),
        }))
    }
}
impl RuleProgram for Program {
    fn condition_count(&self) -> usize {
        self.inner.condition_count()
    }
    fn evaluate(
        &self,
        facts: &Facts,
        requested: &[ConditionId],
        budget: &mut CallBudget,
    ) -> Result<BooleanEffects, KnotError> {
        match self.gate.calls.fetch_add(1, Ordering::SeqCst) {
            0 => {
                self.gate
                    .started
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(())
                    .unwrap();
                self.gate.release.lock().unwrap().recv().unwrap();
            }
            1 if self.gate.panic_second => panic!("synthetic backend failure"),
            1 if self.gate.fault_code.is_some() => {
                return Err(KnotError::new(
                    self.gate.fault_code.unwrap(),
                    "synthetic fault",
                ));
            }
            _ => {}
        }
        self.inner.evaluate(facts, requested, budget)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fault_is_published_before_inflight_success() {
    run_fault_case(true, None).await;
    run_fault_case(false, Some("UNRECOGNIZED_TEST_CODE")).await;
    for code in ["NULL_ARGUMENT", "INVALID_MODEL", "RULE_COMPILE"] {
        run_fault_case(false, Some(code)).await;
    }
}
async fn run_fault_case(panic_second: bool, fault_code: Option<&'static str>) {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let gate = Arc::new(Gate {
        panic_second,
        fault_code,
        calls: AtomicUsize::new(0),
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(release_rx),
    });
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/golden/T03/model.json"),
    )
    .unwrap();
    let model = knot_compiler::compile_product(&bytes, &Compiler(gate)).unwrap();
    let digest = model.digest().to_owned();
    let mut catalog = Catalog::new();
    catalog.insert(digest.clone(), Arc::new(model));
    let life = Lifecycle::new(2);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .layer(Admission::new(life.clone(), Duration::from_secs(10)))
            .add_service(ConfigurationServiceServer::new(Service {
                catalog: Arc::new(catalog),
                lifecycle: life.clone(),
            }))
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let mut client = ConfigurationServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();
    let mut first = client.clone();
    let first_digest = digest.clone();
    let in_flight = tokio::spawn(async move {
        first
            .initial(protocol::InitialRequest {
                model_digest: Some(first_digest),
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), started_rx)
        .await
        .unwrap()
        .unwrap();
    let failure = client
        .initial(protocol::InitialRequest {
            model_digest: Some(digest.clone()),
        })
        .await;
    // Always unblock the worker before assertions, including a failing regression.
    release_tx.send(()).unwrap();
    assert_eq!(failure.unwrap_err().code(), tonic::Code::Internal);
    assert!(life.faulted());
    assert_eq!(
        in_flight.await.unwrap().unwrap_err().code(),
        tonic::Code::Internal
    );
    assert_eq!(
        client
            .initial(protocol::InitialRequest {
                model_digest: Some(digest)
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Internal
    );
    drop(client);
    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnected_rpc_retains_cpu_until_real_completion() {
    for quote in [false, true] {
        run_detached_case(false, quote).await;
        run_detached_case(true, quote).await;
    }
}
async fn run_detached_case(deadline: bool, quote: bool) {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let gate = Arc::new(Gate {
        panic_second: false,
        fault_code: None,
        calls: AtomicUsize::new(0),
        started: Mutex::new(Some(started_tx)),
        release: Mutex::new(release_rx),
    });
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/golden/T03/model.json"),
    )
    .unwrap();
    let model = knot_compiler::compile_product(&bytes, &Compiler(gate)).unwrap();
    let digest = model.digest().to_owned();
    let quote_json = serde_json::to_vec(&serde_json::json!({"schemaVersion":"1.0","state":{"schemaVersion":"1.0","productId":model.product().product_id,"modelDigest":model.digest(),"revision":0,"answers":{},"skipped":[]}})).unwrap();
    let mut catalog = Catalog::new();
    catalog.insert(digest.clone(), Arc::new(model));
    let life = Lifecycle::new(1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .layer(Admission::new(life.clone(), Duration::from_secs(10)))
            .add_service(ConfigurationServiceServer::new(Service {
                catalog: Arc::new(catalog),
                lifecycle: life.clone(),
            }))
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let mut client = ConfigurationServiceClient::connect(format!("http://{addr}"))
        .await
        .unwrap();
    let mut first = client.clone();
    let first_digest = digest.clone();
    let in_flight = tokio::spawn(async move {
        let request = protocol::InitialRequest {
            model_digest: Some(first_digest.clone()),
        };
        let quote_request = protocol::QuoteRequest {
            model_digest: Some(first_digest),
            quote_json: Some(quote_json),
        };
        if deadline {
            // Read the server's wire status without tonic's local deadline middleware.
            let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (mut wire, connection) = h2::client::handshake(socket).await.unwrap();
            let driver = tokio::spawn(connection);
            let message = if quote {
                quote_request.encode_to_vec()
            } else {
                request.encode_to_vec()
            };
            let mut frame = vec![0];
            frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
            frame.extend_from_slice(&message);
            let headers = tonic::codegen::http::Request::builder()
                .method("POST")
                .uri(format!(
                    "http://{addr}/knot.v1.ConfigurationService/{}",
                    if quote { "Quote" } else { "Initial" }
                ))
                .header("content-type", "application/grpc")
                .header("grpc-timeout", "100m")
                .body(())
                .unwrap();
            let (response, mut body) = wire.send_request(headers, false).unwrap();
            body.send_data(bytes::Bytes::from(frame), true).unwrap();
            let response = response.await.unwrap();
            let code = response.headers()["grpc-status"]
                .to_str()
                .unwrap()
                .parse::<i32>()
                .unwrap();
            driver.abort();
            let _ = driver.await;
            Err(tonic::Status::new(
                tonic::Code::from_i32(code),
                "observed wire status",
            ))
        } else if quote {
            first.quote(quote_request).await.map(|_| ())
        } else {
            first.initial(request).await.map(|_| ())
        }
    });
    tokio::time::timeout(Duration::from_secs(5), started_rx)
        .await
        .unwrap()
        .unwrap();
    if deadline {
        let status = tokio::time::timeout(Duration::from_secs(3), in_flight)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    } else {
        in_flight.abort();
        let _ = in_flight.await;
    }
    assert_eq!(
        client
            .initial(protocol::InitialRequest {
                model_digest: Some(digest.clone())
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::ResourceExhausted
    );
    assert_eq!(life.compute.available_permits(), 0);
    release_tx.send(()).unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(5), life.compute.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert!(
        client
            .initial(protocol::InitialRequest {
                model_digest: Some(digest)
            })
            .await
            .is_ok()
    );
    drop(client);
    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[test]
fn process_fault_changes_health_and_exits_four() {
    if let Some(endpoint_path) = std::env::var_os("KNOT_TEST_FAULT_ENDPOINT") {
        knot_server::telemetry::install_panic_hook();
        let endpoint_path = std::path::PathBuf::from(endpoint_path);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let code = runtime.block_on(async {
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (_release_tx, release_rx) = std::sync::mpsc::channel();
            let gate = Arc::new(Gate {
                panic_second: true,
                fault_code: None,
                calls: AtomicUsize::new(0),
                started: Mutex::new(Some(started_tx)),
                release: Mutex::new(release_rx),
            });
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../contracts/golden/T03/model.json"),
            )
            .unwrap();
            let model = knot_compiler::compile_product(&bytes, &Compiler(gate)).unwrap();
            let mut catalog = Catalog::new();
            catalog.insert(model.digest().to_owned(), Arc::new(model));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            std::fs::write(&endpoint_path, listener.local_addr().unwrap().to_string()).unwrap();
            let marker = endpoint_path.with_extension("started");
            tokio::spawn(async move {
                started_rx.await.unwrap();
                std::fs::write(marker, b"started").unwrap();
            });
            knot_server::runtime::serve(
                tonic::transport::Server::builder(),
                listener,
                Arc::new(catalog),
                knot_server::runtime::Limits {
                    max_compute: 2,
                    max_connections: 64,
                    request_timeout: Duration::from_secs(5),
                    shutdown_grace: Duration::from_millis(250),
                },
                std::future::pending(),
            )
            .await
        });
        knot_server::lifecycle::exit_runtime(runtime, code);
    }
    let output =
        std::path::PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap()).join(format!(
            "fault-process-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir(&output).unwrap();
    let endpoint_path = output.join("endpoint");
    let log = std::fs::File::create(output.join("server.log")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process_fault_changes_health_and_exits_four",
            "--nocapture",
        ])
        .env("KNOT_TEST_FAULT_ENDPOINT", &endpoint_path)
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !endpoint_path.exists() {
                    assert!(
                        child.try_wait().unwrap().is_none(),
                        "child exited before bind"
                    );
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            let endpoint = format!(
                "http://{}",
                std::fs::read_to_string(&endpoint_path).unwrap()
            );
            let channel = tonic::transport::Endpoint::from_shared(endpoint)
                .unwrap()
                .connect()
                .await
                .unwrap();
            let mut client = ConfigurationServiceClient::new(channel.clone());
            let mut health = tonic_health::pb::health_client::HealthClient::new(channel);
            let mut watch = health
                .watch(tonic_health::pb::HealthCheckRequest {
                    service: "knot.v1.ConfigurationService".into(),
                })
                .await
                .unwrap()
                .into_inner();
            assert_eq!(watch.message().await.unwrap().unwrap().status, 1);
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../contracts/golden/T03/model.json"),
            )
            .unwrap();
            let digest = knot_compiler::compile_product(&bytes, &knot_rules_zen::ZenCompiler)
                .unwrap()
                .digest()
                .to_owned();
            let mut first = client.clone();
            let first_digest = digest.clone();
            let pending = tokio::spawn(async move {
                first
                    .initial(protocol::InitialRequest {
                        model_digest: Some(first_digest),
                    })
                    .await
            });
            let marker = endpoint_path.with_extension("started");
            tokio::time::timeout(Duration::from_secs(5), async {
                while !marker.exists() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                client
                    .initial(protocol::InitialRequest {
                        model_digest: Some(digest)
                    })
                    .await
                    .unwrap_err()
                    .code(),
                tonic::Code::Internal
            );
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(3), watch.message())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()
                    .status,
                2
            );
            assert!(
                tokio::time::timeout(Duration::from_secs(3), pending)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err()
            );
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Some(status) = child.try_wait().unwrap() {
                        assert_eq!(status.code(), Some(4));
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
        });
        let log = std::fs::read_to_string(output.join("server.log")).unwrap();
        assert!(log.contains("\"event\":\"panic\""));
        assert!(!log.contains("synthetic backend failure"));
    }));
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
    }
    if let Err(payload) = checked {
        std::panic::resume_unwind(payload);
    }
}
