//! Memory monitoring for the daemon

use crate::daemon::config::MemoryConfig;
use sysinfo::{Pid, Signal, System};

/// Result of a memory check
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryStatus {
    /// Memory is fine
    Ok(u64),
    /// Memory is low, should try killing hogs
    Low(u64),
    /// Memory is critical, need to restart worker
    Critical(u64),
}

/// Memory monitor that tracks system memory and can kill memory hogs
pub struct MemoryMonitor {
    config: MemoryConfig,
    sys: System,
}

impl MemoryMonitor {
    /// Create a new memory monitor
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            sys: System::new_all(),
        }
    }

    /// Check current memory status
    pub fn check(&mut self) -> MemoryStatus {
        self.sys.refresh_memory();
        let available = self.sys.available_memory();

        if available < self.config.critical_threshold_bytes {
            MemoryStatus::Critical(available)
        } else if available < self.config.min_available_bytes {
            MemoryStatus::Low(available)
        } else {
            MemoryStatus::Ok(available)
        }
    }

    /// Get available memory in bytes
    pub fn available_memory(&mut self) -> u64 {
        self.sys.refresh_memory();
        self.sys.available_memory()
    }

    /// Get total memory in bytes
    pub fn total_memory(&self) -> u64 {
        self.sys.total_memory()
    }

    /// Kill memory-hogging node processes
    /// Returns the number of processes killed
    ///
    /// # Arguments
    /// * `exclude_pids` - PIDs to exclude from killing (e.g., worker and its children)
    pub fn kill_memory_hogs(&mut self, exclude_pids: &[u32]) -> u32 {
        self.sys.refresh_all();

        let current_pid = std::process::id();
        let mut killed = 0;

        // Collect processes with memory info
        let mut processes: Vec<_> = self
            .sys
            .processes()
            .iter()
            .map(|(pid, proc)| {
                let mem = proc.memory();
                let name = proc.name().to_string_lossy().to_string();
                (*pid, name, mem)
            })
            .collect();

        // Sort by memory usage descending
        processes.sort_by(|a, b| b.2.cmp(&a.2));

        // Log top processes
        eprintln!("Top memory consumers:");
        for (i, (pid, name, mem)) in processes.iter().take(10).enumerate() {
            let is_node = name.to_lowercase().contains("node");
            let is_excluded = exclude_pids.contains(&pid.as_u32());
            let marker = if is_excluded {
                " <- PROTECTED (worker)"
            } else if is_node {
                " <- NODE"
            } else {
                ""
            };
            eprintln!(
                "  {:2}. {:>10}  PID {:6}  {}{}",
                i + 1,
                format_bytes(*mem),
                pid,
                name,
                marker
            );
        }

        // Kill node processes using too much memory
        for (pid, name, mem) in processes.iter() {
            let name_lower = name.to_lowercase();

            // Kill node processes using more than threshold
            if name_lower.contains("node") && *mem > self.config.node_kill_threshold_bytes {
                // Skip our own process
                if pid.as_u32() == current_pid {
                    continue;
                }

                // Skip excluded PIDs (worker and its children)
                if exclude_pids.contains(&pid.as_u32()) {
                    eprintln!(
                        "Skipping protected process {} (PID {}, using {})",
                        name,
                        pid,
                        format_bytes(*mem)
                    );
                    continue;
                }

                if let Some(proc) = self.sys.process(*pid) {
                    eprintln!(
                        "Killing {} (PID {}, using {})",
                        name,
                        pid,
                        format_bytes(*mem)
                    );
                    if proc.kill_with(Signal::Term).unwrap_or(false) {
                        killed += 1;
                    }
                }
            }
        }

        if killed > 0 {
            eprintln!("Killed {} memory hog processes", killed);
            // Give processes time to die
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        killed
    }

    /// Kill a specific process by PID
    pub fn kill_process(&mut self, pid: u32) -> bool {
        self.sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        if let Some(proc) = self.sys.process(Pid::from_u32(pid)) {
            proc.kill_with(Signal::Term).unwrap_or(false)
        } else {
            false
        }
    }

    /// Kill a process group (worker and all its children)
    #[cfg(unix)]
    pub fn kill_process_group(&self, pgid: u32) -> bool {
        // Validate pgid before unsafe libc call
        // pgid must be > 0 (0 would target calling process's group)
        if pgid == 0 {
            eprintln!("Invalid pgid 0: would target calling process group");
            return false;
        }

        // Check pgid fits in i32 (required by killpg)
        let pgid_i32 = match i32::try_from(pgid) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Invalid pgid {}: exceeds i32::MAX", pgid);
                return false;
            }
        };

        // Send SIGTERM to the entire process group
        unsafe { libc::killpg(pgid_i32, libc::SIGTERM) == 0 }
    }

    #[cfg(not(unix))]
    pub fn kill_process_group(&self, _pgid: u32) -> bool {
        false
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}
