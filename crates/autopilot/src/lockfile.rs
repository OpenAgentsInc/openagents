//! Lockfile management for autopilot
//!
//! Provides lockfile creation, cleanup, and crash detection functionality
//! to prevent concurrent autopilot runs and track crashed sessions.

use anyhow::Result;
use colored::*;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global storage for .mcp.json path to enable cleanup on panic/signal
pub static MCP_JSON_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Global storage for lockfile path to enable cleanup on panic/signal
pub static LOCKFILE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Lockfile data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Lockfile {
    pub issue_number: Option<i32>,
    pub session_id: Option<String>,
    pub rlog_path: Option<String>,
    pub started_at: String,
}

/// Get the lockfile path in ~/.autopilot/run.lock
pub fn get_lockfile_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".autopilot").join("run.lock")
}

/// Write lockfile with run information
pub fn write_lockfile(
    issue_number: Option<i32>,
    session_id: Option<String>,
    rlog_path: Option<PathBuf>,
) -> std::io::Result<()> {
    let lockfile_path = get_lockfile_path();

    // Ensure parent directory exists
    if let Some(parent) = lockfile_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let lockfile = Lockfile {
        issue_number,
        session_id,
        rlog_path: rlog_path.map(|p| p.display().to_string()),
        started_at: chrono::Utc::now().to_rfc3339(),
    };

    let json = serde_json::to_string_pretty(&lockfile)?;
    std::fs::write(&lockfile_path, json)?;

    // Store path for cleanup
    LOCKFILE_PATH.set(lockfile_path).ok();

    Ok(())
}

/// Clean up .mcp.json file if it exists
pub fn cleanup_mcp_json() {
    if let Some(path) = MCP_JSON_PATH.get() {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Clean up lockfile if it exists
pub fn cleanup_lockfile() {
    if let Some(path) = LOCKFILE_PATH.get() {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Setup signal handlers and panic hook for cleanup
pub fn setup_cleanup_handlers() {
    // Setup panic hook to cleanup .mcp.json and lockfile
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup_mcp_json();
        // Note: lockfile intentionally NOT cleaned up here - stale lockfile indicates crash
        default_panic(info);
    }));

    // Setup signal handlers for SIGINT and SIGTERM
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));

    // Use iterator-based signal handling for cleanup
    std::thread::spawn(|| {
        match signal_hook::iterator::Signals::new(&[
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
        ]) {
            Ok(mut signals) => {
                if let Some(sig) = signals.forever().next() {
                    cleanup_mcp_json();
                    // Note: lockfile intentionally NOT cleaned up here - stale lockfile indicates crash
                    // Re-raise signal to ensure proper exit
                    signal_hook::low_level::raise(sig).ok();
                    std::process::exit(128 + sig);
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to create signal handler: {}", e);
                // Continue without signal handling
            }
        }
    });
}

/// Check for stale lockfile and block issue if found
pub async fn check_and_handle_stale_lockfile(cwd: &PathBuf) -> Result<()> {
    let lockfile_path = get_lockfile_path();

    if !lockfile_path.exists() {
        return Ok(());
    }

    // Read the lockfile
    let content = std::fs::read_to_string(&lockfile_path)?;
    let lockfile: Lockfile = serde_json::from_str(&content)?;

    eprintln!("{} Found stale lockfile from {}", "Warning:".yellow(), lockfile.started_at);

    // If there's an issue number, block it via MCP
    if let Some(issue_num) = lockfile.issue_number {
        eprintln!("{} Attempting to block issue #{} due to crash", "Crash:".red().bold(), issue_num);

        // Try to use the issues MCP to block the issue
        // Check if .mcp.json exists (issues tracking enabled)
        let mcp_json_path = cwd.join(".mcp.json");
        if mcp_json_path.exists() {
            // Use the handle_issue_command to block
            let reason = format!(
                "Autopilot crashed during execution. Session started at {}. Rlog: {:?}",
                lockfile.started_at,
                lockfile.rlog_path
            );

            use issues::{db, issue};
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let db_path = std::path::PathBuf::from(&home).join(".autopilot").join("autopilot.db");
            let conn = db::init_db(&db_path)?;

            if let Some(i) = issue::get_issue_by_number(&conn, issue_num)? {
                if issue::block_issue(&conn, &i.id, &reason)? {
                    eprintln!("{} Blocked issue #{}", "✓".green(), issue_num);

                    // Print resume hint
                    if let Some(ref rlog) = lockfile.rlog_path {
                        eprintln!();
                        eprintln!("{}", "=".repeat(60).yellow());
                        eprintln!("{} To resume crashed session:", "→".cyan());
                        eprintln!("  {}", format!("autopilot resume {}", rlog).cyan());
                        eprintln!("{}", "=".repeat(60).yellow());
                    }
                } else {
                    eprintln!("{} Could not block issue #{}", "✗".red(), issue_num);
                }
            }
        }
    }

    // Remove the stale lockfile
    std::fs::remove_file(&lockfile_path)?;
    eprintln!("{} Removed stale lockfile", "Cleanup:".cyan());

    Ok(())
}
