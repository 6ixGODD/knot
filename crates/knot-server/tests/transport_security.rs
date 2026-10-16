#![allow(clippy::unwrap_used)] // Test assertions.
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
use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity, ServerTlsConfig};
fn cert(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/server/certs")
            .join(name),
    )
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mutual_tls_requires_valid_identity_and_name() {
    knot_server::health::initialize_tls().unwrap();
    let model = knot_compiler::compile_product(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden/T03/model.json"),
        )
        .unwrap(),
        &knot_rules_zen::ZenCompiler,
    )
    .unwrap();
    let digest = model.digest().to_owned();
    let mut catalog = Catalog::new();
    catalog.insert(digest.clone(), Arc::new(model));
    let life = Lifecycle::new(4);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(
        tonic::transport::Server::builder()
            .tls_config(
                ServerTlsConfig::new()
                    .identity(Identity::from_pem(
                        cert("server.pem"),
                        cert("server-key.pem"),
                    ))
                    .client_ca_root(Certificate::from_pem(cert("ca.pem"))),
            )
            .unwrap()
            .layer(Admission::new(life.clone(), Duration::from_secs(5)))
            .add_service(ConfigurationServiceServer::new(Service {
                catalog: Arc::new(catalog),
                lifecycle: life.clone(),
            }))
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    for (identity, ca, domain, valid) in [
        (Some("client"), "ca.pem", "localhost", true),
        (Some("health"), "ca.pem", "localhost", true),
        (None, "ca.pem", "localhost", false),
        (Some("client"), "wrong-ca.pem", "localhost", false),
        (Some("client"), "ca.pem", "wrong.example", false),
        (Some("expired"), "ca.pem", "localhost", false),
        (Some("wrong-purpose"), "ca.pem", "localhost", false),
    ] {
        let mut tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(cert(ca)))
            .domain_name(domain);
        if let Some(id) = identity {
            tls = tls.identity(Identity::from_pem(
                cert(&format!("{id}.pem")),
                cert(&format!("{id}-key.pem")),
            ));
        }
        let endpoint = Endpoint::from_shared(format!("https://{addr}"))
            .unwrap()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(2))
            .tls_config(tls)
            .unwrap();
        let result = async {
            let channel = endpoint.connect().await.map_err(|_| ())?;
            let mut client = ConfigurationServiceClient::new(channel);
            client
                .initial(protocol::InitialRequest {
                    model_digest: Some(digest.clone()),
                })
                .await
                .map_err(|_| ())
        };
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), result)
                .await
                .unwrap()
                .is_ok(),
            valid,
            "{identity:?}/{ca}/{domain}"
        );
        assert!(!life.faulted());
    }
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}
