//! Authenticated health client; certificate contents never enter diagnostics.
use crate::config;
use clap::ArgMatches;
use std::{fs::File, io::Read, path::Path, time::Duration};
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
pub fn pem(path: &str) -> Result<Vec<u8>, &'static str> {
    let file = File::open(Path::new(path)).map_err(|_| "certificate unreadable")?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "certificate unreadable")?;
    if bytes.is_empty() || bytes.len() > 1024 * 1024 {
        return Err("certificate size");
    }
    Ok(bytes)
}
pub async fn check(m: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    config::validate("healthcheck", m)?;
    let url = config::value(m, "endpoint").ok_or("endpoint")?;
    let timeout = Duration::from_millis(config::number(m, "timeout-ms")?);
    let mut endpoint = Endpoint::from_shared(url.to_owned())?
        .connect_timeout(timeout)
        .timeout(timeout);
    if url.starts_with("https:") {
        endpoint = endpoint.tls_config(
            ClientTlsConfig::new()
                .ca_certificate(Certificate::from_pem(pem(
                    config::value(m, "ca").ok_or("ca")?
                )?))
                .identity(Identity::from_pem(
                    pem(config::value(m, "cert").ok_or("cert")?)?,
                    pem(config::value(m, "key").ok_or("key")?)?,
                ))
                .domain_name(config::value(m, "server-name").ok_or("server name")?),
        )?;
    }
    let future = async {
        let mut client =
            tonic_health::pb::health_client::HealthClient::new(endpoint.connect().await?);
        let response = client
            .check(tonic_health::pb::HealthCheckRequest {
                service: "knot.v1.ConfigurationService".into(),
            })
            .await?;
        if response.into_inner().status
            != tonic_health::pb::health_check_response::ServingStatus::Serving as i32
        {
            return Err("not serving".into());
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };
    tokio::time::timeout(timeout, future).await??;
    Ok(())
}

/// Select the ring provider explicitly when domain dependencies also enable AWS-LC.
pub fn initialize_tls() -> Result<(), &'static str> {
    static INITIALIZED: std::sync::OnceLock<Result<(), &'static str>> = std::sync::OnceLock::new();
    *INITIALIZED.get_or_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| "TLS provider already selected")
    })
}
