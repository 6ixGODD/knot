#![forbid(unsafe_code)]
use knot_server::{catalog, config, health};
use std::{
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
fn main() {
    knot_server::telemetry::install_panic_hook();
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
    let listener =
        tokio::net::TcpListener::bind(config::value(m, "listen").ok_or("listen")?).await?;
    let limits = knot_server::runtime::Limits {
        max_compute: config::number(m, "max-compute")? as usize,
        max_connections: config::number(m, "max-connections")? as usize,
        request_timeout: Duration::from_millis(config::number(m, "request-timeout-ms")?),
        shutdown_grace: Duration::from_millis(config::number(m, "shutdown-grace-ms")?),
    };
    Ok(knot_server::runtime::serve(builder, listener, catalog, limits, signal()).await)
}
