//! Process-wide fault and drain state shared by all models.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, Semaphore};
#[derive(Clone)]
pub struct Lifecycle {
    pub telemetry: crate::telemetry::Telemetry,
    pub compute: Arc<Semaphore>,
    fault: Arc<AtomicBool>,
    draining: Arc<AtomicBool>,
    pub changed: Arc<Notify>,
}
impl Lifecycle {
    pub fn new(max: usize) -> Self {
        Self {
            telemetry: Default::default(),
            compute: Arc::new(Semaphore::new(max)),
            fault: Arc::new(AtomicBool::new(false)),
            draining: Arc::new(AtomicBool::new(false)),
            changed: Arc::new(Notify::new()),
        }
    }
    pub fn spawn<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<tokio::task::JoinHandle<T>, tonic::Status> {
        self.admit()?;
        let permit = self.compute.clone().try_acquire_owned().map_err(|_| {
            self.telemetry.overload.fetch_add(1, Ordering::Relaxed);
            tonic::Status::resource_exhausted("compute capacity")
        })?;
        Ok(tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        }))
    }
    pub fn fault(&self) {
        self.telemetry.fault.fetch_add(1, Ordering::Relaxed);
        self.fault.store(true, Ordering::SeqCst);
        self.drain();
    }
    pub fn drain(&self) {
        self.draining.store(true, Ordering::SeqCst);
        self.changed.notify_one();
    }
    pub fn faulted(&self) -> bool {
        self.fault.load(Ordering::SeqCst)
    }
    pub fn draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }
    pub fn admit(&self) -> Result<(), tonic::Status> {
        if self.faulted() {
            Err(tonic::Status::internal("service fault"))
        } else if self.draining() {
            Err(tonic::Status::unavailable("service draining"))
        } else {
            Ok(())
        }
    }
}

/// Bound network draining and detached blocking work with the same process deadline.
pub async fn finish_shutdown<E>(
    network: impl std::future::Future<Output = Result<(), E>>,
    life: &Lifecycle,
    max: usize,
    grace: std::time::Duration,
) -> i32 {
    let drain = async {
        let result = network.await;
        let _permits = life.compute.clone().acquire_many_owned(max as u32).await;
        result
    };
    let finished = tokio::time::timeout(grace, drain).await;
    if life.faulted() {
        4
    } else {
        match finished {
            Ok(Ok(())) => 0,
            Ok(Err(_)) => 4,
            Err(_) => 5,
        }
    }
}
/// A server process must not wait forever in Runtime::drop for detached CPU work.
pub fn exit_runtime(runtime: tokio::runtime::Runtime, code: i32) -> ! {
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    std::process::exit(code)
}
