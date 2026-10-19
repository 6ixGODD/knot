#![allow(clippy::unwrap_used)] // Test assertions.
use knot_server::{
    admission::Admission, catalog::Catalog, lifecycle::Lifecycle,
    protocol::configuration_service_server::ConfigurationServiceServer, service::Service,
    transport::Incoming,
};
use std::{sync::Arc, time::Duration};
use tonic::codegen::http;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_bodies_hold_global_slots_before_decoding() {
    let life = Lifecycle::new(4);
    let admission = Admission::new(life.clone(), Duration::from_secs(20));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .max_concurrent_streams(8)
            .layer(admission.clone())
            .add_service(ConfigurationServiceServer::new(Service {
                catalog: Arc::new(Catalog::new()),
                lifecycle: life.clone(),
            }))
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let mut connections = Vec::new();
    let mut pending = Vec::new();
    for _ in 0..8 {
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (mut client, connection) = h2::client::handshake(stream).await.unwrap();
        connections.push(tokio::spawn(connection));
        for request_index in 0..8 {
            client = client.ready().await.unwrap();
            let request = http::Request::builder()
                .method("POST")
                .uri(format!(
                    "http://{addr}/knot.v1.ConfigurationService/{}",
                    if request_index % 2 == 0 {
                        "Initial"
                    } else {
                        "Quote"
                    }
                ))
                .header("content-type", "application/grpc")
                .header("te", "trailers")
                .body(())
                .unwrap();
            let (response, mut body) = client.send_request(request, false).unwrap();
            body.send_data(bytes::Bytes::from_static(&[0, 0, 0, 0, 73]), false)
                .unwrap();
            pending.push((response, body));
        }
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while admission.available_business() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        life.compute.available_permits(),
        4,
        "no Core task should have started"
    );
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut client, connection) = h2::client::handshake(stream).await.unwrap();
    connections.push(tokio::spawn(connection));
    let request = http::Request::builder()
        .method("POST")
        .uri(format!(
            "http://{addr}/knot.v1.ConfigurationService/Initial"
        ))
        .header("content-type", "application/grpc")
        .body(())
        .unwrap();
    let (response, _body) = client.send_request(request, false).unwrap();
    let rejected = tokio::time::timeout(Duration::from_secs(2), response)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rejected.headers()["grpc-status"], "8");
    for (_, mut body) in pending {
        body.send_reset(h2::Reason::CANCEL);
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while admission.available_business() != 64 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    for task in connections {
        task.abort();
        let _ = task.await;
    }
    drop(client);
    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_watch_has_separate_capacity_and_bounded_lifetime() {
    let life = Lifecycle::new(1);
    let admission = Admission::new(life.clone(), Duration::from_secs(2));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (reporter, health) = tonic_health::server::health_reporter();
    reporter
        .set_service_status(
            "knot.v1.ConfigurationService",
            tonic_health::ServingStatus::Serving,
        )
        .await;
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .layer(admission.clone())
            .add_service(health)
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = tonic_health::pb::health_client::HealthClient::new(channel);
    let mut watches = Vec::new();
    for _ in 0..8 {
        let mut watch = client
            .watch(tonic_health::pb::HealthCheckRequest {
                service: "knot.v1.ConfigurationService".into(),
            })
            .await
            .unwrap()
            .into_inner();
        assert!(watch.message().await.unwrap().is_some());
        watches.push(watch);
    }
    assert_eq!(admission.available_health(), 0);
    assert_eq!(admission.available_business(), 64);
    assert_eq!(life.compute.available_permits(), 1);
    assert_eq!(
        client
            .watch(tonic_health::pb::HealthCheckRequest {
                service: "knot.v1.ConfigurationService".into()
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::ResourceExhausted
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), watches[0].message())
            .await
            .unwrap()
            .unwrap_err()
            .code(),
        tonic::Code::DeadlineExceeded
    );
    drop(watches);
    drop(client);
    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}
