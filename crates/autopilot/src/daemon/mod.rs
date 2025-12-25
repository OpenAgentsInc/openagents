//! Autopilot Daemon - Supervisor for autopilot worker processes
//!
//! This module provides a daemon that spawns, monitors, and restarts
//! autopilot worker processes. It handles memory monitoring, crash
//! recovery, and integrates with systemd.

pub mod config;
pub mod control;
pub mod memory;
pub mod metrics;
pub mod nostr_trigger;
pub mod state;
pub mod supervisor;

pub use config::DaemonConfig;
pub use control::ControlServer;
pub use memory::MemoryMonitor;
pub use metrics::{DaemonMetrics, DaemonMetricsCollector, RestartEvent, RestartReasons};
pub use nostr_trigger::{NostrTrigger, TriggerEvent};
pub use state::{WorkerState, WorkerStatus};
pub use supervisor::WorkerSupervisor;
