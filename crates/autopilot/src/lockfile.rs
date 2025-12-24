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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Use a mutex to serialize tests that modify environment variables
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// Setup test environment with temp directory
    fn setup_test_env() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    /// Override HOME to temp directory for testing
    fn with_temp_home<F, R>(f: F) -> R
    where
        F: FnOnce(&TempDir) -> R,
    {
        // Lock to prevent parallel test execution (ignore poison errors)
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let temp_dir = setup_test_env();
        let old_home = std::env::var("HOME").ok();

        unsafe {
            std::env::set_var("HOME", temp_dir.path());
        }

        let result = f(&temp_dir);

        // Restore original HOME
        unsafe {
            match old_home {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }

        result
    }

    #[test]
    fn test_get_lockfile_path() {
        with_temp_home(|temp_dir| {
            let path = get_lockfile_path();
            assert_eq!(
                path,
                temp_dir.path().join(".autopilot").join("run.lock")
            );
        });
    }

    #[test]
    fn test_write_lockfile_creates_directory() {
        with_temp_home(|_temp_dir| {
            let lockfile_path = get_lockfile_path();
            assert!(!lockfile_path.parent().unwrap().exists());

            write_lockfile(Some(42), Some("test-session".to_string()), None)
                .expect("Failed to write lockfile");

            assert!(lockfile_path.parent().unwrap().exists());
        });
    }

    #[test]
    fn test_write_lockfile_content() {
        with_temp_home(|_temp_dir| {
            let issue_num = 42;
            let session_id = "test-session-123".to_string();
            let rlog_path = PathBuf::from("/tmp/test.rlog");

            write_lockfile(Some(issue_num), Some(session_id.clone()), Some(rlog_path.clone()))
                .expect("Failed to write lockfile");

            let lockfile_path = get_lockfile_path();
            let content = fs::read_to_string(&lockfile_path).expect("Failed to read lockfile");
            let lockfile: Lockfile = serde_json::from_str(&content).expect("Invalid JSON");

            assert_eq!(lockfile.issue_number, Some(issue_num));
            assert_eq!(lockfile.session_id, Some(session_id));
            assert_eq!(lockfile.rlog_path, Some("/tmp/test.rlog".to_string()));
            assert!(!lockfile.started_at.is_empty());
        });
    }

    #[test]
    fn test_write_lockfile_with_none_values() {
        with_temp_home(|_temp_dir| {
            write_lockfile(None, None, None).expect("Failed to write lockfile");

            let lockfile_path = get_lockfile_path();
            let content = fs::read_to_string(&lockfile_path).expect("Failed to read lockfile");
            let lockfile: Lockfile = serde_json::from_str(&content).expect("Invalid JSON");

            assert_eq!(lockfile.issue_number, None);
            assert_eq!(lockfile.session_id, None);
            assert_eq!(lockfile.rlog_path, None);
            assert!(!lockfile.started_at.is_empty());
        });
    }

    #[test]
    fn test_cleanup_lockfile() {
        with_temp_home(|_temp_dir| {
            write_lockfile(Some(1), Some("test".to_string()), None)
                .expect("Failed to write lockfile");

            let lockfile_path = get_lockfile_path();
            assert!(lockfile_path.exists());

            cleanup_lockfile();
            assert!(!lockfile_path.exists());
        });
    }

    #[test]
    fn test_cleanup_lockfile_when_not_exists() {
        with_temp_home(|_temp_dir| {
            // Calling cleanup when no lockfile exists should not panic
            cleanup_lockfile();
        });
    }

    #[test]
    fn test_cleanup_mcp_json() {
        with_temp_home(|temp_dir| {
            let mcp_json_path = temp_dir.path().join(".mcp.json");
            fs::write(&mcp_json_path, "{}").expect("Failed to write test MCP file");

            MCP_JSON_PATH.set(mcp_json_path.clone()).ok();
            assert!(mcp_json_path.exists());

            cleanup_mcp_json();
            assert!(!mcp_json_path.exists());
        });
    }

    #[test]
    fn test_cleanup_mcp_json_when_not_exists() {
        // Calling cleanup when no MCP file exists should not panic
        cleanup_mcp_json();
    }

    #[test]
    fn test_lockfile_serialization() {
        let lockfile = Lockfile {
            issue_number: Some(42),
            session_id: Some("test-session".to_string()),
            rlog_path: Some("/tmp/test.rlog".to_string()),
            started_at: "2025-12-23T10:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&lockfile).expect("Failed to serialize");
        let deserialized: Lockfile = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(lockfile.issue_number, deserialized.issue_number);
        assert_eq!(lockfile.session_id, deserialized.session_id);
        assert_eq!(lockfile.rlog_path, deserialized.rlog_path);
        assert_eq!(lockfile.started_at, deserialized.started_at);
    }

    #[test]
    fn test_write_lockfile_sets_global_path() {
        with_temp_home(|_temp_dir| {
            write_lockfile(Some(1), None, None).expect("Failed to write lockfile");

            // Verify the global path is set (may have been set by previous tests)
            assert!(LOCKFILE_PATH.get().is_some());

            // The global path should point to a valid lockfile location
            // (Note: OnceLock can only be set once, so we can't guarantee it's this test's path)
            let global_path = LOCKFILE_PATH.get().unwrap();
            assert!(global_path.ends_with(".autopilot/run.lock"));
        });
    }

    #[test]
    fn test_lockfile_contains_valid_timestamp() {
        with_temp_home(|_temp_dir| {
            write_lockfile(Some(1), None, None).expect("Failed to write lockfile");

            let content = fs::read_to_string(get_lockfile_path()).expect("Failed to read lockfile");
            let lockfile: Lockfile = serde_json::from_str(&content).expect("Invalid JSON");

            // Verify timestamp is valid RFC3339
            chrono::DateTime::parse_from_rfc3339(&lockfile.started_at)
                .expect("Invalid timestamp format");
        });
    }
}
