#![forbid(unsafe_code)]
use knot_schema::{KnotError, protocol};
use std::sync::OnceLock;
pub fn model() -> &'static knot_compiler::CompiledModel {
    static MODEL: OnceLock<Result<knot_compiler::CompiledModel, KnotError>> = OnceLock::new();
    match MODEL.get_or_init(|| {
        knot_compiler::compile_product(
            include_bytes!("../../../contracts/examples/speaker.json"),
            &knot_rules_zen::ZenCompiler,
        )
    }) {
        Ok(model) => model,
        Err(error) => panic!("fixed fuzz seed model failed: {error}"),
    }
}
pub fn product(data: &[u8]) {
    let _ = knot_compiler::compile_product(data, &knot_rules_zen::ZenCompiler);
}
pub fn state(data: &[u8]) {
    if let Ok(state) = protocol::state(data) {
        let before = protocol::encode(&state).expect("bounded admitted state");
        if let Ok(result) = knot_core::evaluate(model(), &state) {
            assert_eq!(result.state, state);
            assert!(protocol::encode(&result).is_ok());
        }
        assert_eq!(protocol::encode(&state).expect("state unchanged"), before);
    }
}
pub fn event(data: &[u8]) {
    if let Ok(event) = protocol::event(data) {
        let state = knot_core::initial(model()).expect("fixed initial").state;
        if let Ok(result) = knot_core::apply(model(), &state, &event) {
            let restored =
                knot_core::evaluate(model(), &result.state).expect("successful state must restore");
            assert_eq!(result.snapshot, restored.snapshot);
        }
    }
}
pub fn ast(data: &[u8]) {
    if let Ok(value) = knot_schema::parse_json(data, knot_schema::parse::EVENT_BYTES) {
        let mut product: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../contracts/examples/speaker.json"))
                .expect("fixed JSON");
        product["questions"][3]["options"][2]["enabledWhen"] = value;
        if let Ok(bytes) = protocol::encode(&product) {
            self::product(&bytes);
        }
    }
}
pub fn operations(data: &[u8]) {
    if let Ok(value) = knot_schema::parse_json(data, 1024 * 1024) {
        if let Some(events) = value.as_array() {
            let model = model();
            let mut state = knot_core::initial(model).expect("fixed initial").state;
            for value in events.iter().take(100) {
                if let Ok(bytes) = protocol::encode(value) {
                    if let Ok(event) = protocol::event(&bytes) {
                        let before = protocol::encode(&state).expect("state bytes");
                        match knot_core::apply(model, &state, &event) {
                            Ok(result) => {
                                let restored =
                                    knot_core::evaluate(model, &result.state).expect("restore");
                                assert_eq!(restored.snapshot, result.snapshot);
                                assert!(result.state.revision >= state.revision);
                                state = result.state;
                            }
                            Err(_) => {
                                assert_eq!(protocol::encode(&state).expect("state bytes"), before)
                            }
                        }
                    }
                }
            }
        }
    }
}
