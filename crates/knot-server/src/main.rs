#![forbid(unsafe_code)]
use knot_server::{
    admission::Admission, catalog, config, health, lifecycle::Lifecycle,
    protocol::configuration_service_server::ConfigurationServiceServer, service::Service,
    transport::Incoming,
};
use std::{
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
fn main() {
    let panic_log = knot_server::telemetry::Telemetry::default();
    // The executable owns the process hook; never log panic payloads from model evaluation.
    std::panic::set_hook(Box::new(move |_| panic_log.panic()));
    let args: Vec<_> = std::env::args_os().collect();
    let is_health = args.get(1).is_some_and(|v| v == "healthcheck");
    let matches = match config::parse(args, true) {
        Ok(m) => m,
        Err(error) => {
            if error.kind() == clap::error::ErrorKind::DisplayHelp
                || error.kind() == clap::error::ErrorKind::DisplayVersion
            {
                let _ = error.print();
                return;
            }
            // Clap diagnostics can include supplied values. Do not echo runtime configuration.
            eprintln!("invalid configuration; use --help");
            if is_health {
                println!("NOT_SERVING");
            }
            std::process::exit(if is_health { 1 } else { 2 });
        }
    };
    if matches.subcommand_name() == Some("describe-config") {
        let result = serde_json::to_writer_pretty(io::stdout().lock(), &config::metadata())
            .and_then(|()| io::stdout().write_all(b"\n").map_err(serde_json::Error::io));
        if result.is_err() {
            std::process::exit(2);
        }
        return;
    }
    if health::initialize_tls().is_err() {
        eprintln!("TLS initialization failed");
        std::process::exit(if is_health { 1 } else { 4 });
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(_) => std::process::exit(if is_health { 1 } else { 4 }),
    };
    let code = if let Some(m) = matches.subcommand_matches("healthcheck") {
        if runtime.block_on(health::check(m)).is_ok() {
            println!("SERVING");
            0
        } else {
            println!("NOT_SERVING");
            eprintln!("healthcheck failed");
            1
        }
    } else if let Some(m) = matches.subcommand_matches("serve") {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.block_on(serve(m))))
        {
            Ok(code) => code,
            Err(payload) => {
                if let Err(second) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
                {
                    std::mem::forget(second);
                }
                4
            }
        }
    } else {
        2
    };
    knot_server::lifecycle::exit_runtime(runtime, code);
}
async fn signal() {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {_=term.recv()=>{},_=tokio::signal::ctrl_c()=>{}}
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(windows)]
    {
        match tokio::signal::windows::ctrl_break() {
            Ok(mut stop) => {
                tokio::select! {_=stop.recv()=>{},_=tokio::signal::ctrl_c()=>{}}
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
}
async fn serve(m: &clap::ArgMatches) -> i32 {
    if config::validate("serve", m).is_err() {
        eprintln!("invalid configuration");
        return 2;
    }
    match run(m).await {
        Ok(code) => code,
        Err(_) => {
            eprintln!("server initialization failed");
            4
        }
    }
}
async fn run(m: &clap::ArgMatches) -> Result<i32, Box<dyn std::error::Error>> {
    let mut builder = Server::builder();
    if config::value(m, "transport") == Some("mtls") {
        let tls = (|| -> Result<_, Box<dyn std::error::Error>> {
            Ok(ServerTlsConfig::new()
                .identity(Identity::from_pem(
                    health::pem(config::value(m, "tls-cert").ok_or("cert")?)?,
                    health::pem(config::value(m, "tls-key").ok_or("key")?)?,
                ))
                .client_ca_root(Certificate::from_pem(health::pem(
                    config::value(m, "client-ca").ok_or("ca")?,
                )?))
                .client_auth_optional(false))
        })();
        builder = match tls.and_then(|tls| builder.tls_config(tls).map_err(Into::into)) {
            Ok(b) => b,
            Err(_) => {
                eprintln!("invalid TLS configuration");
                return Ok(2);
            }
        };
    }
    let catalog = match catalog::load(Path::new(config::value(m, "model-dir").ok_or("model-dir")?))
    {
        Ok(c) => Arc::new(c),
        Err(mut error) => {
            error.message = error.code.clone();
            let event =
                serde_json::json!({"event":"model_compile_failed","error":error.envelope()});
            if let Ok(bytes) = knot_schema::protocol::encode(&event) {
                let _ = io::stderr().write_all(&bytes);
                let _ = io::stderr().write_all(b"\n");
            }
            return Ok(3);
        }
    };
    let max = config::number(m, "max-compute")? as usize;
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
    let listener =
        tokio::net::TcpListener::bind(config::value(m, "listen").ok_or("listen")?).await?;
    let incoming = Incoming::new(listener, config::number(m, "max-connections")? as usize);
    let admission = Admission::new(
        life.clone(),
        Duration::from_millis(config::number(m, "request-timeout-ms")?),
    );
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
        result=&mut network=>{return Ok(if result.is_ok(){0}else{4});},
        _=signal()=>{life.drain();},
        _=life.changed.notified()=>{}
    }
    metrics.abort();
    reporter
        .set_not_serving::<ConfigurationServiceServer<Service>>()
        .await;
    let _ = shutdown_tx.send(());
    let code = knot_server::lifecycle::finish_shutdown(
        network,
        &life,
        max,
        Duration::from_millis(config::number(m, "shutdown-grace-ms")?),
    )
    .await;
    life.telemetry.stopped(code);
    Ok(code)
}
