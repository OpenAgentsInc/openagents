pub mod cli;
pub mod runtime;

pub use runtime::{
    AutopilotRuntime, LogSection, RuntimeSnapshot, SdkSessionIds, SessionEvent, SessionPhase,
};

/// Minimal stub for DaemonStatus - the daemon server was never implemented.
/// This exists only to keep autopilot-shell compiling.
#[derive(Debug, Clone, Default)]
pub struct DaemonStatus {
    pub connected: bool,
    pub worker_status: String,
    pub worker_pid: Option<u32>,
    pub uptime_seconds: u64,
    pub total_restarts: u64,
    pub consecutive_failures: u32,
    pub memory_available_bytes: u64,
    pub memory_total_bytes: u64,
    pub error: Option<String>,
}
