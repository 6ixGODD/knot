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
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Gate {
    panic_second: bool,
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
            _ => {}
        }
        self.inner.evaluate(facts, requested, budget)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fault_is_published_before_inflight_success() {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let gate = Arc::new(Gate {
        panic_second: true,
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
        .await
        .unwrap_err();
    assert_eq!(failure.code(), tonic::Code::Internal);
    assert!(life.faulted());
    release_tx.send(()).unwrap();
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
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let gate = Arc::new(Gate {
        panic_second: false,
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
    in_flight.abort();
    let _ = in_flight.await;
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
