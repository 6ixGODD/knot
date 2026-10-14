//! Repeatable release measurements, without claiming a latency SLO.
#![allow(unsafe_code)] // Measurement-only allocator in the FFI example, never in the SDK.
use knot_schema::{KnotError, wire::*};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
struct Counting;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
// SAFETY: all allocation/deallocation is delegated unchanged to System; counters allocate nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc caller provides the layout; System receives it unchanged.
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: GlobalAlloc caller returns this live allocation with its original layout.
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
fn measure(path: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let mut compile_ms = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        let compiled = std::hint::black_box(knot_compiler::compile_product(
            &bytes,
            &knot_rules_zen::ZenCompiler,
        )?);
        compile_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        drop(compiled);
    }
    let model = knot_compiler::compile_product(&bytes, &knot_rules_zen::ZenCompiler)?;
    let mut apply_ms = Vec::new();
    let mut sequence_ms = Vec::new();
    let mut state = knot_core::initial(&model)?.state;
    for _ in 0..3 {
        state = knot_core::initial(&model)?.state;
        let sequence = Instant::now();
        for i in 0..100 {
            let event = ConfigurationEvent::SetAnswer {
                schema_version: Version::V1,
                question_id: Id::new(format!("q{i}"))?,
                expected_revision: state.revision,
                value: Answer::Text("a".into()),
            };
            let start = Instant::now();
            let result = knot_core::apply(&model, &state, &event)?;
            apply_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            state = result.state;
        }
        sequence_ms.push(sequence.elapsed().as_secs_f64() * 1000.0);
    }
    let mut evaluate_ms = Vec::new();
    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = BYTES.load(Ordering::Relaxed);
    for _ in 0..100 {
        let start = Instant::now();
        let result = knot_core::evaluate(&model, &state)?;
        evaluate_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        if result.snapshot.price.total_minor.map(Integer::get) != Some(10000) {
            return Err(KnotError::new("INTERNAL", "").into());
        }
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;
    let allocated_bytes = BYTES.load(Ordering::Relaxed) - bytes_before;
    let parallel_start = Instant::now();
    std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..4 {
            workers.push(scope.spawn(|| {
                for _ in 0..100 {
                    std::hint::black_box(knot_core::evaluate(&model, &state)?);
                }
                Ok::<_, KnotError>(())
            }));
        }
        for worker in workers {
            worker
                .join()
                .map_err(|_| KnotError::new("INTERNAL", ""))??;
        }
        Ok::<_, KnotError>(())
    })?;
    let parallel_seconds = parallel_start.elapsed().as_secs_f64();
    Ok(
        serde_json::json!({"compileMs":compile_ms,"applyMs":apply_ms,"sequenceMs":sequence_ms,"evaluateMs":evaluate_ms,"rustAllocationsFor100Evaluations":allocations,"rustAllocatedBytesFor100Evaluations":allocated_bytes,"parallel":{"threads":4,"operations":400,"seconds":parallel_seconds,"operationsPerSecond":400.0/parallel_seconds}}),
    )
}
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        return std::process::ExitCode::from(2);
    }
    match measure(&args[1]) {
        Ok(result) => {
            println!("{result}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(1)
        }
    }
}
