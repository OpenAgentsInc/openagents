//! Single instance management.
//!
//! Ensures only one autopilot process runs at a time by killing
//! any existing instances when a new one starts.

use sysinfo::{ProcessRefreshKind, RefreshKind, System};

/// Kill any other running autopilot instances.
///
/// This function finds all processes with "autopilot" in their name
/// (excluding the current process) and terminates them.
pub fn kill_other_instances() {
    let current_pid = std::process::id();
    let s = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );

    for (pid, proc) in s.processes() {
        if proc.name().to_string_lossy().contains("autopilot") && pid.as_u32() != current_pid {
            tracing::debug!("Killing existing autopilot instance (PID: {})", pid);
            proc.kill();
        }
    }
}
