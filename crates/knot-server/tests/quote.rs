#![allow(clippy::unwrap_used, clippy::expect_used)]
use knot_compiler::CompiledModel;
use knot_schema::{quote::*, wire::*};
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
use std::{sync::Arc, time::Duration};
fn request(state: ConfigurationState) -> QuoteRequest {
    QuoteRequest {
        schema_version: Version::V1,
        state,
        focus_question_id: None,
        candidate_value: None,
    }
}
fn event(state: &ConfigurationState, id: Id, value: Option<Answer>) -> ConfigurationEvent {
    match value {
        Some(value) => ConfigurationEvent::SetAnswer {
            schema_version: Version::V1,
            expected_revision: state.revision,
            question_id: id,
            value,
        },
        None => ConfigurationEvent::SkipAnswer {
            schema_version: Version::V1,
            expected_revision: state.revision,
            question_id: id,
        },
    }
}
// Test oracle deliberately uses public Apply and its own explicit answer list,
// never compiler quote domains or the production search implementation.
fn oracle(m: &CompiledModel, state: &ConfigurationState) -> Vec<i64> {
    let result = knot_core::evaluate(m, state).unwrap();
    if result.snapshot.flow.status == FlowStatus::Complete {
        return vec![result.snapshot.price.total_minor.unwrap().get()];
    }
    if result.snapshot.flow.status == FlowStatus::Blocked {
        return vec![];
    }
    let (id, required, values) = match result.snapshot.flow.current_question.unwrap() {
        CurrentQuestion::Single {
            id,
            required,
            options,
            ..
        } => (
            id,
            required,
            options
                .into_iter()
                .filter(|o| o.enabled)
                .map(|o| Answer::Text(o.id.as_str().into()))
                .collect::<Vec<_>>(),
        ),
        CurrentQuestion::Multi {
            id,
            required,
            options,
            ..
        } => {
            let ids: Vec<_> = options
                .into_iter()
                .filter(|o| o.enabled)
                .map(|o| o.id)
                .collect();
            let values = (0..1usize << ids.len())
                .filter(|v| !required || *v != 0)
                .map(|mask| {
                    Answer::Multi(
                        ids.iter()
                            .enumerate()
                            .filter(|(i, _)| mask & (1 << i) != 0)
                            .map(|(_, id)| id.clone())
                            .collect(),
                    )
                })
                .collect();
            (id, required, values)
        }
        CurrentQuestion::Color { id, required, .. } => (
            id,
            required,
            ["#000000", "#000001", "#FF0000", "#00FF00", "#315A79"]
                .into_iter()
                .map(|s| Answer::Text(s.into()))
                .collect(),
        ),
        _ => panic!("explicit oracle only supports these fixture domains"),
    };
    let mut result = vec![];
    for v in values {
        let next = knot_core::apply(m, state, &event(state, id.clone(), Some(v))).unwrap();
        result.extend(oracle(m, &next.state));
    }
    if !required {
        let next = knot_core::apply(m, state, &event(state, id, None)).unwrap();
        result.extend(oracle(m, &next.state));
    }
    result
}
fn compare(m: &CompiledModel, state: &ConfigurationState, q: &QuoteResult) {
    let values = oracle(m, state);
    assert_eq!(
        q.price.min_minor.map(Integer::get),
        values.iter().min().copied(),
        "{q:?}"
    );
    assert_eq!(
        q.price.max_minor.map(Integer::get),
        values.iter().max().copied()
    );
    if let Some(f) = &q.focus {
        let id = match &f.question {
            CurrentQuestion::Single { id, .. }
            | CurrentQuestion::Multi { id, .. }
            | CurrentQuestion::Color { id, .. } => id,
            _ => panic!(),
        };
        for c in &f.candidates {
            if !c.enabled {
                assert!(c.price.is_none());
                continue;
            }
            match knot_core::apply(
                m,
                state,
                &event(state, id.clone(), Some(c.candidate_value.clone())),
            ) {
                Ok(next) => {
                    let values = oracle(m, &next.state);
                    let p = c.price.as_ref().unwrap();
                    assert_eq!(p.min_minor.map(Integer::get), values.iter().min().copied());
                    assert_eq!(p.max_minor.map(Integer::get), values.iter().max().copied());
                }
                Err(e) => assert_eq!(c.error.as_ref().unwrap().error.code, e.code),
            }
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_tcp_zen_quotes_match_independent_oracle_and_strict_errors() {
    let mut catalog = Catalog::new();
    let mut models = vec![];
    for bytes in [
        include_bytes!("../../../contracts/examples/speaker.json").as_slice(),
        include_bytes!("../../../contracts/examples/quote/addons.json").as_slice(),
        include_bytes!("../../../contracts/examples/quote/color.json").as_slice(),
    ] {
        let m =
            Arc::new(knot_compiler::compile_product(bytes, &knot_rules_zen::ZenCompiler).unwrap());
        catalog.insert(m.digest().into(), m.clone());
        models.push(m);
    }
    let huge = serde_json::json!({"schemaVersion":"1.0","productId":"huge","productVersion":"1","questions":[{"id":"q","type":"integer","label":"Q"}],"flow":{"start":"q","edges":[{"from":"q","to":"END"}]},"pricing":{"currency":"CNY","components":[{"id":"base","category":"base","amountMinor":100}]},"visual":{"targets":[],"slots":[]}});
    let huge = Arc::new(
        knot_compiler::compile_product(
            &serde_json::to_vec(&huge).unwrap(),
            &knot_rules_zen::ZenCompiler,
        )
        .unwrap(),
    );
    catalog.insert(huge.digest().into(), huge.clone());
    models.push(huge);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let life = Lifecycle::new(4);
    let service = ConfigurationServiceServer::new(Service {
        catalog: Arc::new(catalog),
        lifecycle: life.clone(),
    })
    .max_decoding_message_size(1048576)
    .max_encoding_message_size(4195328);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .layer(Admission::new(life, Duration::from_secs(30)))
            .add_service(service)
            .serve_with_incoming_shutdown(Incoming::new(listener, 64), async {
                let _ = rx.await;
            }),
    );
    let mut client = ConfigurationServiceClient::connect(format!("http://{address}"))
        .await
        .unwrap()
        .max_decoding_message_size(4195328);
    for model in &models {
        let state = knot_core::initial(model).unwrap().state;
        let req = request(state.clone());
        let reply = client
            .quote(protocol::QuoteRequest {
                model_digest: Some(model.digest().into()),
                quote_json: Some(serde_json::to_vec(&req).unwrap()),
            })
            .await
            .unwrap()
            .into_inner();
        let raw = match reply.outcome.unwrap() {
            protocol::quote_response::Outcome::QuoteJson(v) => v,
            other => panic!("{other:?}"),
        };
        let q: QuoteResult = serde_json::from_slice(&raw).unwrap();
        assert_eq!(q, knot_core::quote(model, &req).unwrap());
        if model.product().product_id.as_str() == "huge" {
            assert_eq!(q.price.status, QuoteStatus::Unavailable);
            assert_eq!(q.price.reason, Some(QuoteReason::DomainLimit));
            assert!(q.price.min_minor.is_none() && q.price.max_minor.is_none());
        } else {
            compare(model, &state, &q);
        }
        assert_eq!(q.revision, state.revision);
    }
    let m = &models[0];
    let mut journey = knot_core::initial(m).unwrap().state;
    for (id, value, minimum, maximum) in [
        (
            "speaker_type",
            Answer::Text("active".into()),
            159900,
            302700,
        ),
        ("material", Answer::Text("aluminum".into()), 179900, 302700),
        (
            "panel_color",
            Answer::Text("#315A79".into()),
            179900,
            302700,
        ),
        ("power", Answer::Text("200w".into()), 269900, 302700),
        (
            "extras",
            Answer::Multi(vec![Id::new("led".into()).unwrap()]),
            282800,
            282800,
        ),
        ("material", Answer::Text("walnut".into()), 159900, 232700),
    ] {
        journey = knot_core::apply(
            m,
            &journey,
            &event(&journey, Id::new(id.into()).unwrap(), Some(value)),
        )
        .unwrap()
        .state;
        let response = client
            .quote(protocol::QuoteRequest {
                model_digest: Some(m.digest().into()),
                quote_json: Some(serde_json::to_vec(&request(journey.clone())).unwrap()),
            })
            .await
            .unwrap()
            .into_inner();
        let raw = match response.outcome.unwrap() {
            protocol::quote_response::Outcome::QuoteJson(bytes) => bytes,
            other => panic!("{other:?}"),
        };
        let actual: QuoteResult = serde_json::from_slice(&raw).unwrap();
        compare(m, &journey, &actual);
        assert_eq!(
            (
                actual.price.min_minor.unwrap().get(),
                actual.price.max_minor.unwrap().get()
            ),
            (minimum, maximum)
        );
    }
    assert!(
        !journey
            .answers
            .contains_key(&Id::new("power".into()).unwrap())
    );
    assert!(
        !journey
            .answers
            .contains_key(&Id::new("extras".into()).unwrap())
    );
    let state = knot_core::initial(m).unwrap().state;
    for (bytes, code) in [
        (vec![], "BAD_JSON"),
        (b"{}".to_vec(), "BAD_SCHEMA"),
        (b"null".to_vec(), "BAD_SCHEMA"),
        (vec![b' '; 768 * 1024 + 1], "RESOURCE_LIMIT"),
    ] {
        let r = client
            .quote(protocol::QuoteRequest {
                model_digest: Some(m.digest().into()),
                quote_json: Some(bytes),
            })
            .await
            .unwrap()
            .into_inner();
        match r.outcome.unwrap() {
            protocol::quote_response::Outcome::ErrorJson(v) => assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&v).unwrap()["error"]["code"],
                code
            ),
            o => panic!("{o:?}"),
        }
    }
    let mut wrong = request(state.clone());
    wrong.state.product_id = Id::new("wrong".into()).unwrap();
    let r = client
        .quote(protocol::QuoteRequest {
            model_digest: Some(m.digest().into()),
            quote_json: Some(serde_json::to_vec(&wrong).unwrap()),
        })
        .await
        .unwrap()
        .into_inner();
    match r.outcome.unwrap() {
        protocol::quote_response::Outcome::ErrorJson(v) => assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&v).unwrap()["error"]["code"],
            "MODEL_MISMATCH"
        ),
        o => panic!("{o:?}"),
    }
    assert_eq!(
        client
            .quote(protocol::QuoteRequest {
                model_digest: Some(m.digest().into()),
                quote_json: None
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    assert_eq!(
        client
            .quote(protocol::QuoteRequest {
                model_digest: None,
                quote_json: Some(vec![])
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[test]
fn color_equivalence_classes_match_public_apply_prices_and_reachability() {
    let bytes = include_bytes!("../../../contracts/examples/quote/color.json");
    let model = knot_compiler::compile_product(bytes, &knot_rules_zen::ZenCompiler).unwrap();
    let state = knot_core::initial(&model).unwrap().state;
    let q = knot_core::quote(&model, &request(state.clone())).unwrap();
    compare(&model, &state, &q);
    assert_eq!(q.price.min_minor.unwrap().get(), 9900);
    assert_eq!(q.price.max_minor.unwrap().get(), 10250);
    for (color, price) in [("#FF0000", 10250), ("#00FF00", 9950), ("#123456", 9900)] {
        let next = knot_core::apply(
            &model,
            &state,
            &event(
                &state,
                Id::new("color".into()).unwrap(),
                Some(Answer::Text(color.into())),
            ),
        )
        .unwrap();
        assert_eq!(next.snapshot.price.total_minor.unwrap().get(), price);
    }
    let mut product: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    product["questions"].as_array_mut().unwrap().push(serde_json::json!({"id":"finish","type":"single","label":"Finish","options":[{"id":"only","label":"Only","enabledWhen":{"op":"in","field":"color","value":["#FF0000","#00FF00"]}}]}));
    product["flow"]["edges"] =
        serde_json::json!([{"from":"color","to":"finish"},{"from":"finish","to":"END"}]);
    let model = knot_compiler::compile_product(
        &serde_json::to_vec(&product).unwrap(),
        &knot_rules_zen::ZenCompiler,
    )
    .unwrap();
    let state = knot_core::initial(&model).unwrap().state;
    let q = knot_core::quote(&model, &request(state.clone())).unwrap();
    compare(&model, &state, &q);
    assert_eq!(q.price.min_minor.unwrap().get(), 9950);
    assert_eq!(q.price.max_minor.unwrap().get(), 10250);
    let other = knot_core::apply(
        &model,
        &state,
        &event(
            &state,
            Id::new("color".into()).unwrap(),
            Some(Answer::Text("#123456".into())),
        ),
    )
    .unwrap();
    assert_eq!(other.snapshot.flow.status, FlowStatus::Blocked);
    assert_eq!(
        knot_core::quote(&model, &request(other.state))
            .unwrap()
            .price
            .status,
        QuoteStatus::NoCompletion
    );
}

struct CountCompiler(std::sync::Arc<Count>);
#[derive(Default)]
struct Count {
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
    cost: std::sync::atomic::AtomicU64,
}
struct CountProgram {
    inner: std::sync::Arc<dyn knot_rules::RuleProgram>,
    count: std::sync::Arc<Count>,
}
impl knot_rules::RuleCompiler for CountCompiler {
    fn compile(
        &self,
        fields: &[knot_rules::FieldKind],
        conditions: &[knot_rules::Condition],
        sources: &knot_rules::SourceMap,
        budget: &mut knot_rules::CompileBudget,
    ) -> Result<std::sync::Arc<dyn knot_rules::RuleProgram>, knot_schema::KnotError> {
        Ok(std::sync::Arc::new(CountProgram {
            inner: knot_rules::RuleCompiler::compile(
                &knot_rules_zen::ZenCompiler,
                fields,
                conditions,
                sources,
                budget,
            )?,
            count: self.0.clone(),
        }))
    }
}
impl knot_rules::RuleProgram for CountProgram {
    fn condition_count(&self) -> usize {
        self.inner.condition_count()
    }
    fn evaluate(
        &self,
        facts: &knot_rules::Facts,
        requested: &[knot_rules::ConditionId],
        budget: &mut knot_rules::CallBudget,
    ) -> Result<knot_rules::BooleanEffects, knot_schema::KnotError> {
        use std::sync::atomic::Ordering::SeqCst;
        if requested.is_empty() {
            self.count.hits.fetch_add(1, SeqCst);
        } else {
            self.count.misses.fetch_add(requested.len() as u64, SeqCst);
        }
        let before = budget.remaining();
        let result = self.inner.evaluate(facts, requested, budget);
        self.count
            .cost
            .fetch_add(before - budget.remaining(), SeqCst);
        result
    }
}
#[test]
fn speaker_quote_real_backend_cost_profile() {
    use std::sync::atomic::Ordering::SeqCst;
    let count = std::sync::Arc::new(Count::default());
    let model = knot_compiler::compile_product(
        include_bytes!("../../../contracts/examples/speaker.json"),
        &CountCompiler(count.clone()),
    )
    .unwrap();
    let state = knot_core::initial(&model).unwrap().state;
    count.hits.store(0, SeqCst);
    count.misses.store(0, SeqCst);
    count.cost.store(0, SeqCst);
    let result = knot_core::quote(&model, &request(state));
    println!(
        "backendHealth={} backendMisses={} backendCharged={} result={:?}",
        count.hits.load(SeqCst),
        count.misses.load(SeqCst),
        count.cost.load(SeqCst),
        result.as_ref().map(|q| &q.price)
    );
    assert_eq!(result.unwrap().price.status, QuoteStatus::Range);
}
