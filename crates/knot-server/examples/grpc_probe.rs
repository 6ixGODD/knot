//! Exercise a running server with the frozen corpus; never regenerate expectations.
use clap::Parser;
use knot_server::protocol::{self, configuration_service_client::ConfigurationServiceClient};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Duration};
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    endpoint: String,
    #[arg(long)]
    contracts: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    ca: Option<PathBuf>,
    #[arg(long)]
    cert: Option<PathBuf>,
    #[arg(long)]
    key: Option<PathBuf>,
    #[arg(long)]
    server_name: Option<String>,
}
fn compare(mut actual: Value, mut expected: Value) -> Result<(), Box<dyn std::error::Error>> {
    if expected.get("error").is_some() {
        let schema: Value = serde_json::from_slice(include_bytes!(
            "../../../contracts/schemas/error.schema.json"
        ))?;
        knot_schema::validation::Validator::new(&schema)?.validate(&actual)?;
        if !actual["error"]["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty() && m.len() <= 16384)
        {
            return Err("invalid error message".into());
        }
        actual["error"]
            .as_object_mut()
            .ok_or("error object")?
            .remove("message");
        expected["error"]
            .as_object_mut()
            .ok_or("error object")?
            .remove("message");
    }
    if actual != expected {
        return Err("frozen result mismatch".into());
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    knot_server::health::initialize_tls()?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("evidence.json"), b"{\"passed\":false}\n")?;
    let mut endpoint = Endpoint::from_shared(args.endpoint.clone())?
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30));
    if args.endpoint.starts_with("https:") {
        endpoint = endpoint.tls_config(
            ClientTlsConfig::new()
                .ca_certificate(Certificate::from_pem(fs::read(
                    args.ca.ok_or("CA required")?,
                )?))
                .identity(Identity::from_pem(
                    fs::read(args.cert.ok_or("client certificate required")?)?,
                    fs::read(args.key.ok_or("client key required")?)?,
                ))
                .domain_name(args.server_name.ok_or("server name required")?),
        )?;
    } else if args.ca.is_some()
        || args.cert.is_some()
        || args.key.is_some()
        || args.server_name.is_some()
    {
        return Err("TLS options with plaintext".into());
    }
    let mut client = ConfigurationServiceClient::new(endpoint.connect().await?)
        .max_decoding_message_size(4195328);
    let root = args.contracts.join("golden");
    let index: Value = serde_json::from_slice(&fs::read(root.join("index.json"))?)?;
    let mut records = Vec::new();
    for case in index["cases"].as_array().ok_or("case array")? {
        let id = case["id"].as_str().ok_or("case id")?;
        for key in case.as_object().ok_or("case")?.keys() {
            if ![
                "id",
                "title",
                "operation",
                "expectedStatus",
                "modelValidation",
                "stateValidation",
                "eventValidation",
                "roundtripState",
                "threads",
                "repetitions",
                "alternateCase",
                "bindings",
                "releaseOriginalBufferTwice",
            ]
            .contains(&key.as_str())
            {
                return Err(format!("unknown harness field {key}").into());
            }
        }
        if case["operation"] == "model_create" {
            records.push(json!({"id":id,"scope":"startup verification"}));
            continue;
        }
        if case.get("releaseOriginalBufferTwice").is_some() {
            records.push(json!({"id":id,"scope":"native ABI ownership; not a gRPC operation"}));
            continue;
        }
        let dir = root.join(id);
        let model = knot_compiler::compile_product(
            &fs::read(dir.join("model.json"))?,
            &knot_rules_zen::ZenCompiler,
        )?;
        let digest = Some(model.digest().to_owned());
        let state = if dir.join("initial-state.json").exists() {
            Some(fs::read(dir.join("initial-state.json"))?)
        } else {
            None
        };
        let state = if case["roundtripState"] == true {
            Some(knot_schema::protocol::encode(
                &knot_schema::protocol::state(state.as_deref().ok_or("state")?)?,
            )?)
        } else {
            state
        };
        let reply = match case["operation"].as_str().ok_or("operation")? {
            "initial" => {
                client
                    .initial(protocol::InitialRequest {
                        model_digest: digest.clone(),
                    })
                    .await?
            }
            "apply" => {
                client
                    .apply(protocol::ApplyRequest {
                        model_digest: digest.clone(),
                        state_json: state.clone(),
                        event_json: Some(fs::read(dir.join("event.json"))?),
                    })
                    .await?
            }
            "evaluate" => {
                client
                    .evaluate(protocol::EvaluateRequest {
                        model_digest: digest.clone(),
                        state_json: state.clone(),
                    })
                    .await?
            }
            _ => return Err("unknown operation".into()),
        }
        .into_inner();
        let bytes = match reply.outcome.ok_or("missing outcome")? {
            protocol::configuration_response::Outcome::ResultJson(b)
                if case["expectedStatus"] == 0 =>
            {
                b
            }
            protocol::configuration_response::Outcome::ErrorJson(b)
                if case["expectedStatus"] != 0 =>
            {
                b
            }
            _ => return Err("wrong outcome branch".into()),
        };
        let expected: Value = serde_json::from_slice(&fs::read(dir.join("expected-result.json"))?)?;
        compare(serde_json::from_slice(&bytes)?, expected.clone())?;
        if case["expectedStatus"] == 0 {
            let quoted_state =
                knot_schema::protocol::state(&serde_json::to_vec(&expected["state"])?)?;
            let request = knot_schema::quote::QuoteRequest {
                schema_version: knot_schema::wire::Version::V1,
                state: quoted_state,
                focus_question_id: None,
                candidate_value: None,
            };
            let expected_quote = match knot_core::quote(&model, &request) {
                Ok(q) => serde_json::to_value(q)?,
                Err(e) => e.envelope(),
            };
            let response = client
                .quote(protocol::QuoteRequest {
                    model_digest: digest.clone(),
                    quote_json: Some(serde_json::to_vec(&request)?),
                })
                .await?
                .into_inner();
            let raw = match response.outcome.ok_or("missing Quote outcome")? {
                protocol::quote_response::Outcome::QuoteJson(bytes)
                    if expected_quote.get("error").is_none() =>
                {
                    bytes
                }
                protocol::quote_response::Outcome::ErrorJson(bytes)
                    if expected_quote.get("error").is_some() =>
                {
                    bytes
                }
                _ => return Err("wrong Quote outcome".into()),
            };
            compare(serde_json::from_slice(&raw)?, expected_quote)?;
            records.push(json!({"id":id,"operation":"Quote","passed":true}));
        }
        if let Some(threads) = case["threads"].as_u64() {
            let alternate = root.join(case["alternateCase"].as_str().ok_or("alternate case")?);
            let alternate_expected: Value =
                serde_json::from_slice(&fs::read(alternate.join("expected-result.json"))?)?;
            let alternate_state = serde_json::to_vec(&alternate_expected["state"])?;
            let mut tasks = Vec::new();
            for _ in 0..threads {
                let mut client = client.clone();
                let digest = digest.clone();
                let state = state.clone();
                let expected = expected.clone();
                let alternate_state = alternate_state.clone();
                let alternate_expected = alternate_expected.clone();
                let repetitions = case["repetitions"].as_u64().ok_or("repetitions")?;
                tasks.push(tokio::spawn(async move {
                    for i in 0..repetitions {
                        let (state, expected) = if i % 2 == 0 {
                            (state.clone(), expected.clone())
                        } else {
                            (Some(alternate_state.clone()), alternate_expected.clone())
                        };
                        let reply = client
                            .evaluate(protocol::EvaluateRequest {
                                model_digest: digest.clone(),
                                state_json: state,
                            })
                            .await
                            .map_err(|_| "parallel RPC failed")?
                            .into_inner();
                        let Some(protocol::configuration_response::Outcome::ResultJson(bytes)) =
                            reply.outcome
                        else {
                            return Err("parallel outcome");
                        };
                        compare(
                            serde_json::from_slice(&bytes).map_err(|_| "parallel JSON")?,
                            expected,
                        )
                        .map_err(|_| "parallel mismatch")?;
                    }
                    Ok::<(), &'static str>(())
                }));
            }
            for task in tasks {
                task.await??;
            }
        }
        records.push(json!({"id":id,"passed":true}));
    }
    fs::write(
        args.out.join("evidence.json"),
        serde_json::to_vec_pretty(&json!({"passed":true,"cases":records}))?,
    )?;
    Ok(())
}
