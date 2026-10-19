use knot_server::{config, protocol};
use prost::Message;
fn parse(args: &[&str]) -> Result<clap::ArgMatches, clap::Error> {
    config::parse(args.iter().map(std::ffi::OsString::from), false)
}
#[test]
fn descriptor_contract() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor = prost_types::FileDescriptorSet::decode(protocol::DESCRIPTOR)?;
    let file = descriptor
        .file
        .iter()
        .find(|f| f.package.as_deref() == Some("knot.v1"))
        .ok_or("package")?;
    assert_eq!(file.service.len(), 1);
    let service = &file.service[0];
    assert_eq!(service.name.as_deref(), Some("ConfigurationService"));
    assert_eq!(
        service
            .method
            .iter()
            .map(|m| m.name.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("Initial"),
            Some("Apply"),
            Some("Evaluate"),
            Some("Quote")
        ]
    );
    for message in &file.message_type {
        for (index, field) in message.field.iter().enumerate() {
            assert_eq!(field.number, Some(index as i32 + 1));
            if !matches!(
                message.name.as_deref(),
                Some("ConfigurationResponse" | "QuoteResponse")
            ) {
                assert_eq!(field.proto3_optional, Some(true));
            }
        }
    }
    let reply = file
        .message_type
        .iter()
        .find(|m| m.name.as_deref() == Some("ConfigurationResponse"))
        .ok_or("response")?;
    assert_eq!(reply.oneof_decl.len(), 1);
    assert_eq!(
        reply
            .field
            .iter()
            .map(|f| f.oneof_index)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(0)]
    );
    let quote = file
        .message_type
        .iter()
        .find(|m| m.name.as_deref() == Some("QuoteRequest"))
        .ok_or("quote request")?;
    assert_eq!(
        quote
            .field
            .iter()
            .map(|f| (f.name.as_deref(), f.number, f.r#type))
            .collect::<Vec<_>>(),
        vec![
            (Some("model_digest"), Some(1), Some(9)),
            (Some("quote_json"), Some(2), Some(12))
        ]
    );
    let quote_reply = file
        .message_type
        .iter()
        .find(|m| m.name.as_deref() == Some("QuoteResponse"))
        .ok_or("quote response")?;
    assert_eq!(quote_reply.oneof_decl.len(), 1);
    assert_eq!(
        quote_reply
            .field
            .iter()
            .map(|f| (f.name.as_deref(), f.number, f.oneof_index))
            .collect::<Vec<_>>(),
        vec![
            (Some("quote_json"), Some(1), Some(0)),
            (Some("error_json"), Some(2), Some(0))
        ]
    );
    let quote_method = service
        .method
        .iter()
        .find(|m| m.name.as_deref() == Some("Quote"))
        .ok_or("quote method")?;
    assert_eq!(
        quote_method.input_type.as_deref(),
        Some(".knot.v1.QuoteRequest")
    );
    assert_eq!(
        quote_method.output_type.as_deref(),
        Some(".knot.v1.QuoteResponse")
    );
    let missing = protocol::ApplyRequest::decode(&[][..])?;
    assert!(missing.state_json.is_none());
    let empty = protocol::ApplyRequest {
        model_digest: Some(String::new()),
        state_json: Some(vec![]),
        event_json: Some(vec![]),
    };
    assert_eq!(
        protocol::ApplyRequest::decode(empty.encode_to_vec().as_slice())?.state_json,
        Some(vec![])
    );
    Ok(())
}
#[test]
fn configuration_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    for spec in config::SERVE {
        if let (Some(min), Some(max)) = (spec.min, spec.max) {
            for (n, valid) in [(min - 1, false), (min, true), (max, true), (max + 1, false)] {
                let flag = format!("--{}", spec.flag);
                let n = n.to_string();
                let m = parse(&[
                    "knot-server",
                    "serve",
                    "--model-dir",
                    "models",
                    "--transport",
                    "plaintext",
                    &flag,
                    &n,
                ])?;
                assert_eq!(
                    config::validate("serve", m.subcommand_matches("serve").ok_or("serve")?)
                        .is_ok(),
                    valid,
                    "{} {}",
                    flag,
                    n
                );
            }
        }
    }
    for args in [
        vec!["--listen", "127.0.0.1:0"],
        vec!["--tls-key", "key"],
        vec!["--max-compute", ""],
        vec!["--max-compute", "+1"],
    ] {
        let mut values = vec![
            "knot-server",
            "serve",
            "--model-dir",
            "models",
            "--transport",
            "plaintext",
        ];
        values.extend(args);
        let m = parse(&values)?;
        assert!(config::validate("serve", m.subcommand_matches("serve").ok_or("serve")?).is_err());
    }
    let m = parse(&["knot-server", "serve", "--model-dir", "models"])?;
    assert!(config::validate("serve", m.subcommand_matches("serve").ok_or("serve")?).is_err());
    assert!(parse(&["knot-server", "describe-config", "--model-dir", "secret"]).is_err());
    assert_eq!(config::metadata()["schemaVersion"], "1.0");
    Ok(())
}

#[test]
fn health_configuration_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    for spec in config::HEALTH {
        if let (Some(min), Some(max)) = (spec.min, spec.max) {
            for (n, valid) in [(min - 1, false), (min, true), (max, true), (max + 1, false)] {
                let flag = format!("--{}", spec.flag);
                let n = n.to_string();
                let m = parse(&[
                    "knot-server",
                    "healthcheck",
                    "--endpoint",
                    "http://127.0.0.1:50051",
                    &flag,
                    &n,
                ])?;
                assert_eq!(
                    config::validate(
                        "healthcheck",
                        m.subcommand_matches("healthcheck").ok_or("healthcheck")?
                    )
                    .is_ok(),
                    valid
                );
            }
        }
    }
    Ok(())
}
