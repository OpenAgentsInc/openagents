//! Memory management utilities for autopilot
//!
//! Provides functions to check available system memory, format memory sizes,
//! and optionally kill memory-intensive processes when memory is low.

use colored::*;
use sysinfo::{Signal, System};

/// Get minimum available memory threshold from environment or use default (500 MB)
/// Note: macOS reports "available" memory conservatively - it doesn't count
/// reclaimable cached/inactive memory. 500MB is enough to start Claude.
pub fn min_available_memory_bytes() -> u64 {
    std::env::var("AUTOPILOT_MIN_MEMORY_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500 * 1024 * 1024)
}

/// Get memory cleanup threshold from environment or use default (1.5 GB)
/// We try to free memory when we drop below this
pub fn memory_cleanup_threshold_bytes() -> u64 {
    std::env::var("AUTOPILOT_CLEANUP_THRESHOLD_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1536 * 1024 * 1024)
}

fn memory_cleanup_enabled() -> bool {
    match std::env::var("AUTOPILOT_ENABLE_MEMORY_CLEANUP") {
        Ok(value) => matches!(
            value.trim().to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// Check if system has enough available memory
/// Returns (available_bytes, needs_cleanup, is_critical)
pub fn check_memory() -> (u64, bool, bool) {
    let sys = System::new_all();
    let available = sys.available_memory();
    // If we get 0, something went wrong - don't abort, just return ok
    if available == 0 {
        return (0, false, false);
    }
    let needs_cleanup = available < memory_cleanup_threshold_bytes();
    let is_critical = available < min_available_memory_bytes();
    (available, needs_cleanup, is_critical)
}

/// List top memory-consuming processes and optionally kill Claude-related ones
///
/// WARNING: This function kills ANY process with "node" in the name that uses >500MB.
/// This is a fallback for non-daemon mode. The daemon supervisor has more precise
/// tracking of worker PIDs and only kills untracked processes.
pub fn check_and_kill_memory_hogs() -> u64 {
    let mut sys = System::new_all();
    sys.refresh_all();

    let available = sys.available_memory();
    let total = sys.total_memory();
    let used = total - available;

    println!("\n{}", "=".repeat(60).yellow());
    println!("{} Memory Status", "MEM:".yellow().bold());
    println!("  Total:     {}", format_bytes(total));
    println!("  Used:      {}", format_bytes(used));
    println!("  Available: {}", format_bytes(available));
    println!();

    // Collect processes with memory info
    let mut processes: Vec<_> = sys
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

    println!("{} Top 15 Memory Hogs:", "PROCS:".yellow().bold());
    for (i, (pid, name, mem)) in processes.iter().take(15).enumerate() {
        let is_claude =
            name.to_lowercase().contains("claude") || name.to_lowercase().contains("node");
        let marker = if is_claude {
            " ← CLAUDE/NODE".red().bold().to_string()
        } else {
            String::new()
        };
        println!(
            "  {:2}. {:>10}  {:6}  {}{}",
            i + 1,
            format_bytes(*mem),
            pid,
            name,
            marker
        );
    }

    // Cleanup is opt-in to avoid killing unrelated Node.js processes.
    if !memory_cleanup_enabled() {
        println!(
            "{} Memory cleanup disabled. Set AUTOPILOT_ENABLE_MEMORY_CLEANUP=1 to enable.",
            "SKIP:".yellow().bold()
        );
        return available;
    }

    if std::env::var("AUTOPILOT_NO_MEMORY_CLEANUP").is_ok() {
        println!(
            "{} Memory cleanup disabled via AUTOPILOT_NO_MEMORY_CLEANUP",
            "SKIP:".yellow().bold()
        );
        return available;
    }

    // Find and kill stale claude/node processes (but not ourselves)
    let current_pid = std::process::id();
    let mut killed = 0;

    for (pid, name, mem) in processes.iter() {
        let name_lower = name.to_lowercase();
        // Kill node processes using > 500MB that aren't critical
        if name_lower.contains("node") && *mem > 500 * 1024 * 1024 {
            // Skip if it might be our parent process
            if pid.as_u32() == current_pid {
                continue;
            }

            if let Some(proc) = sys.process(*pid) {
                println!(
                    "{} Killing {} (PID {}, using {})",
                    "KILL:".red().bold(),
                    name,
                    pid,
                    format_bytes(*mem)
                );
                println!(
                    "    {} This may kill unrelated Node.js processes. Set AUTOPILOT_ENABLE_MEMORY_CLEANUP=1 to enable.",
                    "WARN:".yellow()
                );
                if proc.kill_with(Signal::Term).unwrap_or(false) {
                    killed += 1;
                }
            }
        }
    }

    if killed > 0 {
        println!(
            "{} Killed {} memory hog processes",
            "CLEANUP:".green().bold(),
            killed
        );
        // Give processes time to die and memory to be reclaimed
        std::thread::sleep(std::time::Duration::from_secs(3));

        // Re-check memory after cleanup
        sys.refresh_memory();
        let new_available = sys.available_memory();
        println!(
            "{} Memory after cleanup: {} (was {})",
            "MEM:".green().bold(),
            format_bytes(new_available),
            format_bytes(available)
        );
        println!("{}", "=".repeat(60).yellow());
        return new_available;
    }

    println!("{}", "=".repeat(60).yellow());

    available
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env(value: Option<&str>, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap();
        let original = std::env::var("AUTOPILOT_ENABLE_MEMORY_CLEANUP").ok();

        unsafe {
            if let Some(val) = value {
                std::env::set_var("AUTOPILOT_ENABLE_MEMORY_CLEANUP", val);
            } else {
                std::env::remove_var("AUTOPILOT_ENABLE_MEMORY_CLEANUP");
            }
        }

        f();

        unsafe {
            if let Some(val) = original {
                std::env::set_var("AUTOPILOT_ENABLE_MEMORY_CLEANUP", val);
            } else {
                std::env::remove_var("AUTOPILOT_ENABLE_MEMORY_CLEANUP");
            }
        }
    }

    #[test]
    fn test_memory_cleanup_enabled_default_false() {
        with_env(None, || {
            assert!(!memory_cleanup_enabled());
        });
    }

    #[test]
    fn test_memory_cleanup_enabled_truthy_values() {
        for value in ["1", "true", "yes", "on"] {
            with_env(Some(value), || {
                assert!(memory_cleanup_enabled());
            });
        }
    }
}
