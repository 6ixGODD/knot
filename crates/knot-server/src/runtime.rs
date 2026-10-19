//! Shared server assembly and process-wide health/drain transitions.
use crate::{
    admission::Admission, catalog::Catalog, lifecycle::Lifecycle,
    protocol::configuration_service_server::ConfigurationServiceServer, service::Service,
    transport::Incoming,
};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tonic::transport::Server;

/// Limits validated by the executable's configuration parser.
pub struct Limits {
    pub max_compute: usize,
    pub max_connections: usize,
    pub request_timeout: Duration,
    pub shutdown_grace: Duration,
}

/// Serve a fully compiled catalog until a signal or process-wide fault.
pub async fn serve(
    builder: Server,
    listener: TcpListener,
    catalog: Arc<Catalog>,
    limits: Limits,
    shutdown: impl Future<Output = ()>,
) -> i32 {
    let max = limits.max_compute;
    let life = Lifecycle::new(max);
    let service = ConfigurationServiceServer::new(Service {
        catalog,
        lifecycle: life.clone(),
    })
    .max_decoding_message_size(1024 * 1024)
    .max_encoding_message_size(4 * 1024 * 1024 + 1024);
    let (reporter, health) = tonic_health::server::health_reporter();
    reporter
        .set_serving::<ConfigurationServiceServer<Service>>()
        .await;
    let incoming = Incoming::new(listener, limits.max_connections);
    let admission = Admission::new(life.clone(), limits.request_timeout);
    let metrics_life = life.clone();
    let metrics_admission = admission.clone();
    let metrics = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            metrics_life.telemetry.snapshot(
                64 - metrics_admission.available_business(),
                max - metrics_life.compute.available_permits(),
            );
        }
    });
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let network = builder
        .max_concurrent_streams(8)
        .http2_max_header_list_size(16384)
        .initial_stream_window_size(65535)
        .initial_connection_window_size(1048576)
        .layer(admission)
        .add_service(service)
        .add_service(health)
        .serve_with_incoming_shutdown(incoming, async {
            let _ = shutdown_rx.await;
        });
    tokio::pin!(network);
    life.telemetry.ready();
    tokio::select! {
        result=&mut network=>{metrics.abort(); return if result.is_ok(){0}else{4};},
        _=shutdown=>{life.drain();},
        _=life.changed.notified()=>{}
    }
    metrics.abort();
    reporter
        .set_not_serving::<ConfigurationServiceServer<Service>>()
        .await;
    let _ = shutdown_tx.send(());
    let code = crate::lifecycle::finish_shutdown(network, &life, max, limits.shutdown_grace).await;
    life.telemetry.stopped(code);
    code
}
