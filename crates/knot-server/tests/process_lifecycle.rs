#![allow(clippy::unwrap_used)] // Child-process test assertions only.
use knot_server::lifecycle::{Lifecycle, exit_runtime, finish_shutdown};
use std::{
    process::Command,
    time::{Duration, Instant},
};
#[test]
fn process_exit_is_bounded_with_real_blocking_work() {
    if let Ok(mode) = std::env::var("KNOT_TEST_CHILD_DRAIN") {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let code = runtime.block_on(async {
            let life = Lifecycle::new(1);
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let _worker = life
                .spawn(move || {
                    started_tx.send(()).unwrap();
                    std::thread::park();
                })
                .unwrap();
            started_rx.await.unwrap();
            if mode == "fault" {
                life.fault();
            } else {
                life.drain();
            }
            finish_shutdown(
                std::future::pending::<Result<(), ()>>(),
                &life,
                1,
                Duration::from_millis(20),
            )
            .await
        });
        exit_runtime(runtime, code);
    }
    for (mode, code) in [("normal", 5), ("fault", 4)] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_exit_is_bounded_with_real_blocking_work",
                "--nocapture",
            ])
            .env("KNOT_TEST_CHILD_DRAIN", mode)
            .spawn()
            .unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() > Duration::from_secs(3) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("child failed bounded runtime shutdown");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(code));
    }
}
