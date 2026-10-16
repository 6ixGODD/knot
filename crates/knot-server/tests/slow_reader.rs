#![allow(clippy::unwrap_used)] // Real network boundary assertions.
use knot_server::{
    admission::Admission, catalog::Catalog, lifecycle::Lifecycle, protocol,
    protocol::configuration_service_server::ConfigurationServiceServer, service::Service,
    transport::Incoming,
};
use prost::Message;
use std::{sync::Arc, time::Duration};
use tonic::codegen::http;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unread_unary_responses_hold_rpc_slots_after_compute_finishes() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/golden/T03/model.json"),
    )
    .unwrap();
    let model = knot_compiler::compile_product(&bytes, &knot_rules_zen::ZenCompiler).unwrap();
    let digest = model.digest().to_owned();
    let mut catalog = Catalog::new();
    catalog.insert(digest.clone(), Arc::new(model));
    let life = Lifecycle::new(4);
    let admission = Admission::new(life.clone(), Duration::from_secs(20));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .max_concurrent_streams(8)
            .layer(admission.clone())
            .add_service(ConfigurationServiceServer::new(Service {
                catalog: Arc::new(catalog),
                lifecycle: life.clone(),
            }))
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let encoded = protocol::InitialRequest {
        model_digest: Some(digest),
    }
    .encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
    frame.extend_from_slice(&encoded);
    let mut drivers = Vec::new();
    let mut responses = Vec::new();
    for _ in 0..8 {
        let socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let (mut client, connection) = h2::client::Builder::new()
            .initial_window_size(0)
            .handshake(socket)
            .await
            .unwrap();
        drivers.push(tokio::spawn(connection));
        for _ in 0..8 {
            client = client.ready().await.unwrap();
            let request = http::Request::builder()
                .method("POST")
                .uri(format!(
                    "http://{address}/knot.v1.ConfigurationService/Initial"
                ))
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .body(())
                .unwrap();
            let (response, mut body) = client.send_request(request, false).unwrap();
            body.send_data(bytes::Bytes::copy_from_slice(&frame), true)
                .unwrap();
            let response = tokio::time::timeout(Duration::from_secs(3), response)
                .await
                .unwrap()
                .unwrap();
            assert!(response.headers().get("grpc-status").is_none());
            responses.push(response.into_body());
        }
    }
    assert_eq!(life.compute.available_permits(), 4);
    assert_eq!(
        admission.available_business(),
        0,
        "HTTP/2 flow control must retain the RPC permit"
    );
    drop(responses);
    for driver in drivers {
        driver.abort();
        let _ = driver.await;
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while admission.available_business() != 64 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
