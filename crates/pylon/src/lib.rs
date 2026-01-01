//! Pylon - Local runtime for sovereign AI agents
//!
//! Pylon is a single binary that runs on your device and does two things:
//!
//! 1. **Host Mode**: Run your own sovereign agents that pay for their own compute
//! 2. **Provider Mode**: Earn Bitcoin by selling compute to agents on the network
//!
//! Both modes can run simultaneously. Your machine hosts your agents AND earns
//! sats from other agents.
//!
//! ## Inference Backends
//!
//! - Ollama (localhost:11434) - any platform
//! - Llama.cpp/GPT-OSS (localhost:8080) - any platform
//! - Apple Foundation Models (localhost:11435) - macOS Apple Silicon only

pub mod cli;
pub mod claude_tunnel;
pub mod config;
pub mod daemon;
pub mod db;
pub mod host;
pub mod neobank_service;
pub mod provider;

pub use config::PylonConfig;
pub use daemon::{PidFile, is_daemon_running, pid_path, runtime_dir, socket_path};
pub use db::PylonDb;
pub use host::{AgentHandle, AgentRunner};
pub use neobank_service::{NeobankConfig, NeobankError, NeobankService, TreasuryStatus};
pub use provider::PylonProvider;
