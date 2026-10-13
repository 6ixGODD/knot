mod support;
use knot_schema::wire::*;
use support::*;

#[test]
fn fixed_seed_prefix_determinism_roundtrip_and_component_sum() {
    let m = model(product(
        (0..8).map(|i| color(&format!("q{i}"), false)).collect(),
    ));
    let mut state = initial(&m).state;
    let mut seed = 0x504c414e3033u64;
    let mut counterexample = serde_json::Value::Null;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for step in 0..400 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let old = knot_core::evaluate(&m, &state)
                .unwrap_or_else(|e| panic!("seed={seed} step={step} {e}"));
            let index = (seed as usize) % old.snapshot.flow.path.len();
            let q = &old.snapshot.flow.path[index];
            let event = if seed.is_multiple_of(3) && state.answers.contains_key(q) {
                clear(&state, q.as_str())
            } else if seed % 3 == 1 && old.snapshot.flow.next_question_id.as_ref() == Some(q) {
                skip(&state, q.as_str())
            } else {
                set(
                    &state,
                    q.as_str(),
                    text(if seed & 8 == 0 { "#abcdef" } else { "#112233" }),
                )
            };
            let before = state.clone();
            counterexample = serde_json::json!({"seed":seed.to_string(),"step":step,"state":state,"event":event});
            let first = knot_core::apply(&m, &state, &event)
                .unwrap_or_else(|e| panic!("seed={seed} step={step} {e}"));
            let second = knot_core::apply(&m, &state, &event).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(first, second, "seed={seed} step={step}");
            assert_eq!(state, before);
            for id in first.state.answers.keys().chain(first.state.skipped.iter()) {
                assert!(
                    first.snapshot.flow.path.contains(id),
                    "seed={seed} step={step}"
                );
            }
            for change in first.changes.answer_changes.iter().skip(1) {
                assert_eq!(change.kind, ChangeKind::Cleared);
                assert!(
                    before.answers.contains_key(&change.question_id)
                        || before.skipped.contains(&change.question_id)
                );
            }
            assert!(
                first.state.answers.len() + first.state.skipped.len()
                    <= before.answers.len() + before.skipped.len() + 1
            );
            let bytes = serde_json::to_vec(&first.state).unwrap_or_else(|e| panic!("{e}"));
            let decoded: ConfigurationState =
                serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{e}"));
            let restored = knot_core::evaluate(&m, &decoded).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(restored.snapshot, first.snapshot);
            assert_eq!(decoded, first.state);
            assert!(restored.changes.answer_changes.is_empty());
            assert_eq!(
                first.snapshot.price.total_minor.map(|n| n.get()),
                Some(
                    first
                        .snapshot
                        .price
                        .components
                        .iter()
                        .map(|c| c.amount_minor.get())
                        .sum()
                )
            );
            state = first.state;
        }
    }));
    if let Err(payload) = outcome {
        eprintln!("counterexample: {counterexample}");
        if let Some(directory) = std::env::var_os("KNOT_TEST_EVIDENCE_DIR") {
            let path = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&path)
                .unwrap_or_else(|e| panic!("save evidence directory: {e}"));
            std::fs::write(
                path.join("properties-counterexample.json"),
                counterexample.to_string(),
            )
            .unwrap_or_else(|e| panic!("save counterexample: {e}"));
        }
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn concurrent_calls_do_not_share_answers_or_revisions() {
    let m = model(product(vec![color("a", true)]));
    let state = initial(&m).state;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..12)
            .map(|i| {
                let m = &m;
                let s = &state;
                scope.spawn(move || answer(m, s, "a", text(&format!("#{i:06X}"))))
            })
            .collect();
        for (i, h) in handles.into_iter().enumerate() {
            let r = h.join().unwrap_or_else(|_| panic!("worker"));
            assert_eq!(r.state.revision, n(1));
            assert_eq!(r.state.answers[&id("a")], text(&format!("#{i:06X}")));
        }
    });
    assert!(state.answers.is_empty());
    assert_eq!(state.revision, n(0));
}
