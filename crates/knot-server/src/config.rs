//! One declaration drives command parsing, help and configuration metadata.
use clap::{Arg, ArgMatches, Command};
use serde::Serialize;
use serde_json::{Value, json};
use std::{ffi::OsString, net::SocketAddr};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionSpec {
    pub flag: &'static str,
    pub env: &'static str,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub default: Option<&'static str>,
    pub required_when: Option<&'static str>,
    pub min: Option<u64>,
    pub max: Option<u64>,
    #[serde(rename = "enum")]
    pub choices: &'static [&'static str],
}
macro_rules! spec {
    ($flag:literal,$env:literal,$kind:literal,$default:expr,$required:expr,$min:expr,$max:expr,$choices:expr) => {
        OptionSpec {
            flag: $flag,
            env: $env,
            kind: $kind,
            default: $default,
            required_when: $required,
            min: $min,
            max: $max,
            choices: $choices,
        }
    };
}
pub const SERVE: &[OptionSpec] = &[
    spec!(
        "listen",
        "KNOT_LISTEN",
        "socket",
        Some("127.0.0.1:50051"),
        None,
        None,
        None,
        &[]
    ),
    spec!(
        "model-dir",
        "KNOT_MODEL_DIR",
        "path",
        None,
        Some("always"),
        None,
        None,
        &[]
    ),
    spec!(
        "transport",
        "KNOT_TRANSPORT",
        "enum",
        Some("mtls"),
        None,
        None,
        None,
        &["mtls", "plaintext"]
    ),
    spec!(
        "tls-cert",
        "KNOT_TLS_CERT",
        "path",
        None,
        Some("mtls"),
        None,
        None,
        &[]
    ),
    spec!(
        "tls-key",
        "KNOT_TLS_KEY",
        "path",
        None,
        Some("mtls"),
        None,
        None,
        &[]
    ),
    spec!(
        "client-ca",
        "KNOT_CLIENT_CA",
        "path",
        None,
        Some("mtls"),
        None,
        None,
        &[]
    ),
    spec!(
        "max-compute",
        "KNOT_MAX_COMPUTE",
        "integer",
        Some("4"),
        None,
        Some(1),
        Some(64),
        &[]
    ),
    spec!(
        "max-connections",
        "KNOT_MAX_CONNECTIONS",
        "integer",
        Some("64"),
        None,
        Some(1),
        Some(256),
        &[]
    ),
    spec!(
        "request-timeout-ms",
        "KNOT_REQUEST_TIMEOUT_MS",
        "integer",
        Some("30000"),
        None,
        Some(1),
        Some(300000),
        &[]
    ),
    spec!(
        "shutdown-grace-ms",
        "KNOT_SHUTDOWN_GRACE_MS",
        "integer",
        Some("10000"),
        None,
        Some(1),
        Some(120000),
        &[]
    ),
];
pub const HEALTH: &[OptionSpec] = &[
    spec!(
        "endpoint",
        "KNOT_HEALTH_ENDPOINT",
        "url",
        None,
        Some("always"),
        None,
        None,
        &[]
    ),
    spec!(
        "ca",
        "KNOT_HEALTH_CA",
        "path",
        None,
        Some("https"),
        None,
        None,
        &[]
    ),
    spec!(
        "cert",
        "KNOT_HEALTH_CERT",
        "path",
        None,
        Some("https"),
        None,
        None,
        &[]
    ),
    spec!(
        "key",
        "KNOT_HEALTH_KEY",
        "path",
        None,
        Some("https"),
        None,
        None,
        &[]
    ),
    spec!(
        "server-name",
        "KNOT_HEALTH_SERVER_NAME",
        "string",
        None,
        Some("https"),
        None,
        None,
        &[]
    ),
    spec!(
        "timeout-ms",
        "KNOT_HEALTH_TIMEOUT_MS",
        "integer",
        Some("2000"),
        None,
        Some(1),
        Some(30000),
        &[]
    ),
];
pub fn metadata() -> Value {
    json!({"schemaVersion":"1.0", "subcommands":[
        {"name":"serve","options":SERVE}, {"name":"healthcheck","options":HEALTH},
        {"name":"describe-config","options":[]}],
        "fixedLimits":{"models":64,"directoryEntries":1024,"modelInputBytes":67108864,
            "decodeBytes":1048576,"encodeBytes":4195328,"headerBytes":16384,
            "streamsPerConnection":8,"businessRpc":64,"healthStreams":8,"handshakeMs":5000},
        "exitCodes":{"serve":{"normal":0,"configuration":2,"model":3,"internal":4,"drainTimeout":5},
            "healthcheck":{"serving":0,"failure":1}}})
}
fn subcommand(name: &'static str, specs: &[OptionSpec], environment: bool) -> Command {
    let mut command = Command::new(name);
    for spec in specs {
        let mut arg = Arg::new(spec.flag).long(spec.flag).num_args(1);
        if environment {
            arg = arg.env(spec.env).hide_env_values(true);
        }
        if let Some(value) = spec.default {
            arg = arg.default_value(value);
        }
        command = command.arg(arg);
    }
    command
}
pub fn parse(
    args: impl IntoIterator<Item = OsString>,
    environment: bool,
) -> Result<ArgMatches, clap::Error> {
    let args: Vec<_> = args.into_iter().collect();
    // Do not even read environment values for metadata and help.
    let environment = environment
        && !args
            .iter()
            .any(|v| v == "describe-config" || v == "--help" || v == "-h");
    Command::new("knot-server")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(subcommand("serve", SERVE, environment))
        .subcommand(subcommand("healthcheck", HEALTH, environment))
        .subcommand(Command::new("describe-config"))
        .try_get_matches_from(args)
}
pub fn value<'a>(m: &'a ArgMatches, key: &str) -> Option<&'a str> {
    m.get_one::<String>(key).map(String::as_str)
}
pub fn number(m: &ArgMatches, key: &str) -> Result<u64, &'static str> {
    value(m, key)
        .ok_or("missing number")?
        .parse()
        .map_err(|_| "invalid number")
}
pub fn validate(name: &str, m: &ArgMatches) -> Result<(), &'static str> {
    let specs = if name == "serve" { SERVE } else { HEALTH };
    for spec in specs {
        match value(m, spec.flag) {
            None if spec.required_when == Some("always") => return Err("missing required option"),
            Some("") => return Err("empty option"),
            Some(v) => {
                if !spec.choices.is_empty() && !spec.choices.contains(&v) {
                    return Err("invalid choice");
                }
                if spec.kind == "integer" {
                    if !v.bytes().all(|b| b.is_ascii_digit()) {
                        return Err("invalid integer");
                    }
                    let n = number(m, spec.flag)?;
                    if spec.min.is_some_and(|min| n < min) || spec.max.is_some_and(|max| n > max) {
                        return Err("number outside range");
                    }
                }
            }
            None => {}
        }
    }
    if name == "serve" {
        let addr: SocketAddr = value(m, "listen")
            .ok_or("missing listen")?
            .parse()
            .map_err(|_| "invalid listen")?;
        if addr.port() == 0 {
            return Err("invalid listen port");
        }
        let tls = value(m, "transport") == Some("mtls");
        for key in ["tls-cert", "tls-key", "client-ca"] {
            if value(m, key).is_some() != tls {
                return Err("invalid TLS option combination");
            }
        }
    } else {
        let endpoint = value(m, "endpoint").ok_or("missing endpoint")?;
        let uri: tonic::codegen::http::Uri = endpoint.parse().map_err(|_| "invalid endpoint")?;
        let tls = match uri.scheme_str() {
            Some("https") => true,
            Some("http") => false,
            _ => return Err("invalid scheme"),
        };
        if uri.authority().is_none()
            || uri.host().is_none()
            || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
        {
            return Err("invalid endpoint");
        }
        for key in ["ca", "cert", "key", "server-name"] {
            if value(m, key).is_some() != tls {
                return Err("invalid health TLS combination");
            }
        }
    }
    Ok(())
}
