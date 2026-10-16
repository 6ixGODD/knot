//! Bounded, nonblocking event delivery to a dedicated stderr writer.
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
};
#[derive(Clone)]
pub struct Telemetry {
    sender: SyncSender<serde_json::Value>,
    pub overload: Arc<AtomicU64>,
    pub deadline: Arc<AtomicU64>,
    pub domain: Arc<AtomicU64>,
    pub fault: Arc<AtomicU64>,
}
impl Default for Telemetry {
    fn default() -> Self {
        let (sender, receiver) = sync_channel::<serde_json::Value>(64);
        let _ = std::thread::Builder::new()
            .name("knot-log".into())
            .spawn(move || {
                for event in receiver {
                    if let Ok(mut bytes) = serde_json::to_vec(&event) {
                        bytes.push(b'\n');
                        let _ = std::io::stderr().write_all(&bytes);
                    }
                }
            });
        Self {
            sender,
            overload: Arc::new(AtomicU64::new(0)),
            deadline: Arc::new(AtomicU64::new(0)),
            domain: Arc::new(AtomicU64::new(0)),
            fault: Arc::new(AtomicU64::new(0)),
        }
    }
}
impl Telemetry {
    pub fn ready(&self) {
        let _ = self
            .sender
            .try_send(serde_json::json!({"event":"ready","version":env!("CARGO_PKG_VERSION")}));
    }
    pub fn stopped(&self, code: i32) {
        let _ = self
            .sender
            .try_send(serde_json::json!({"event":"stopped","exitCode":code}));
    }

    pub fn panic(&self) {
        let _ = self
            .sender
            .try_send(serde_json::json!({"event":"panic","result":"internal"}));
    }

    pub fn request(
        &self,
        operation: &'static str,
        result: &'static str,
        digest: &str,
        duration_ms: u128,
    ) {
        let _=self.sender.try_send(serde_json::json!({"event":"request","version":env!("CARGO_PKG_VERSION"),"operation":operation,"result":result,"modelDigest":digest,"durationMs":duration_ms}));
    }
    pub fn snapshot(&self, active_rpc: usize, active_compute: usize) {
        let _=self.sender.try_send(serde_json::json!({"event":"metrics","activeRpc":active_rpc,"activeCompute":active_compute,"overload":self.overload.load(Ordering::Relaxed),"deadline":self.deadline.load(Ordering::Relaxed),"domainError":self.domain.load(Ordering::Relaxed),"fault":self.fault.load(Ordering::Relaxed)}));
    }
}

/// Install the redacted panic hook at an executable's process boundary.
pub fn install_panic_hook() {
    let log = Telemetry::default();
    std::panic::set_hook(Box::new(move |_| log.panic()));
}
