//! Computer agent daemon, environment probing, security policy engine, and execution journal

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerProbeResult {
    pub os: String,
    pub arch: String,
    pub num_cpus: usize,
    pub total_memory_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerPolicy {
    pub allow_shell: bool,
    pub allow_filesystem_write: bool,
    pub allow_network: bool,
}

impl Default for ComputerPolicy {
    fn default() -> Self {
        Self {
            allow_shell: true,
            allow_filesystem_write: true,
            allow_network: true,
        }
    }
}

pub fn probe_host() -> ComputerProbeResult {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    ComputerProbeResult {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        num_cpus: sys.cpus().len(),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
    }
}
