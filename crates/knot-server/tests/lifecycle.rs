#![allow(clippy::unwrap_used)] // Test assertions.
use knot_server::lifecycle::Lifecycle;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_waiter_does_not_release_worker_permit() {
    let life = Lifecycle::new(1);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let worker = life
        .spawn(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        })
        .unwrap();
    started_rx.await.unwrap();
    worker.abort();
    drop(worker);
    assert_eq!(life.compute.available_permits(), 0);
    assert_eq!(
        life.spawn(|| ()).unwrap_err().code(),
        tonic::Code::ResourceExhausted
    );
    release_tx.send(()).unwrap();
    let permit = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        life.compute.clone().acquire_owned(),
    )
    .await
    .unwrap()
    .unwrap();
    drop(permit);
    life.spawn(|| ()).unwrap().await.unwrap();
}
#[tokio::test]
async fn fault_precedes_draining_and_is_shared() {
    let life = Lifecycle::new(1);
    let other = life.clone();
    life.fault();
    assert!(other.faulted());
    assert!(other.draining());
    assert_eq!(other.admit().unwrap_err().code(), tonic::Code::Internal);
    life.drain();
    assert_eq!(life.admit().unwrap_err().code(), tonic::Code::Internal);
}
