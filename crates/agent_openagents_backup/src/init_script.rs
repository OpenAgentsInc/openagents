//! Init Script Runner
//!
//! Runs `.openagents/init.sh` at session start to verify the workspace.
//! Follows the pi-mono pattern: run if present, skip silently if missing.
//!
//! Exit code semantics:
//! - 0: All checks passed → success=true
//! - 1: Fatal error → success=false (abort session)
//! - 2: Warnings only → success=true, hasWarnings=true (continue with caution)

use crate::types::{get_init_script_path, InitScriptFailureType, InitScriptResult, OrchestratorEvent};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

/// Default timeout for init script (2 minutes)
pub const DEFAULT_INIT_SCRIPT_TIMEOUT_MS: u64 = 120_000;

/// Detect the type of failure from init script output.
/// This enables safe mode to determine the appropriate recovery strategy.
pub fn detect_failure_type(output: &str) -> (InitScriptFailureType, bool) {
    let lower = output.to_lowercase();

    // TypeScript/type errors - can self-heal by spawning Claude Code to fix
    if (lower.contains("ts") && (lower.contains("error") || lower.contains("type")))
        || lower.contains("typecheck")
        || lower.contains("tsc")
        || output.contains("TS2") || output.contains("TS7") // Common TS error codes
        || lower.contains("cannot find name")
        || lower.contains("property") && lower.contains("does not exist")
        || lower.contains("argument of type")
    {
        return (InitScriptFailureType::TypecheckFailed, true);
    }

    // Test failures - can attempt to fix
    if lower.contains("test failed")
        || lower.contains("tests failed")
        || lower.contains("test failure")
        || lower.contains("assertion")
        || lower.contains("expect(")
        || (lower.contains("fail") && (lower.contains("test") || lower.contains("spec")))
    {
        return (InitScriptFailureType::TestFailed, true);
    }

    // Network errors - can continue in offline/degraded mode
    if lower.contains("network")
        || lower.contains("enotfound")
        || lower.contains("econnrefused")
        || lower.contains("etimedout")
        || lower.contains("unable to connect")
        || lower.contains("could not resolve")
    {
        return (InitScriptFailureType::NetworkError, false);
    }

    // Disk full - cannot self-heal
    if lower.contains("no space left")
        || lower.contains("disk full")
        || lower.contains("enospc")
        || lower.contains("quota exceeded")
    {
        return (InitScriptFailureType::DiskFull, false);
    }

    // Permission errors - cannot self-heal
    if lower.contains("permission denied")
        || lower.contains("eacces")
        || lower.contains("eperm")
        || lower.contains("operation not permitted")
    {
        return (InitScriptFailureType::PermissionDenied, false);
    }

    // Unknown error - fallback
    (InitScriptFailureType::Unknown, false)
}

/// Run the init script and return the result
pub fn run_init_script(
    openagents_dir: &str,
    cwd: &str,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
    timeout_ms: Option<u64>,
) -> InitScriptResult {
    let init_path = get_init_script_path(openagents_dir);
    let _timeout = timeout_ms.unwrap_or(DEFAULT_INIT_SCRIPT_TIMEOUT_MS);

    // Check if init script exists
    if !Path::new(&init_path).exists() {
        return InitScriptResult {
            ran: false,
            success: true,
            exit_code: Some(0),
            ..Default::default()
        };
    }

    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::InitScriptStart {
            path: init_path.clone(),
        });
    }

    let start = Instant::now();

    // Run the script
    let output = Command::new("bash")
        .arg(&init_path)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let duration_ms = start.elapsed().as_millis() as u64;

    let result = match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(1);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let output_str = format!("{}\n{}", stdout, stderr).trim().to_string();

            // Exit code semantics:
            // 0 = success
            // 1 = fatal error (abort)
            // 2 = warnings only (continue)
            let success = exit_code == 0 || exit_code == 2;
            let has_warnings = exit_code == 2;

            let (failure_type, can_self_heal) = if !success && !output_str.is_empty() {
                let (ft, csh) = detect_failure_type(&output_str);
                (Some(ft), Some(csh))
            } else {
                (None, None)
            };

            let error = if exit_code == 1 {
                Some("Preflight check failed (exit 1)".to_string())
            } else {
                None
            };

            InitScriptResult {
                ran: true,
                success,
                has_warnings: Some(has_warnings),
                exit_code: Some(exit_code),
                output: if output_str.is_empty() { None } else { Some(output_str) },
                duration_ms: Some(duration_ms),
                error,
                failure_type,
                can_self_heal,
            }
        }
        Err(e) => InitScriptResult {
            ran: true,
            success: false,
            exit_code: Some(1),
            error: Some(e.to_string()),
            duration_ms: Some(duration_ms),
            failure_type: Some(InitScriptFailureType::Unknown),
            can_self_heal: Some(false),
            ..Default::default()
        },
    };

    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::InitScriptComplete {
            result: result.clone(),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_failure_type_typecheck() {
        let (ft, can_heal) = detect_failure_type("error TS2322: Type 'string' is not assignable");
        assert_eq!(ft, InitScriptFailureType::TypecheckFailed);
        assert!(can_heal);
    }

    #[test]
    fn test_detect_failure_type_test() {
        let (ft, can_heal) = detect_failure_type("5 tests failed");
        assert_eq!(ft, InitScriptFailureType::TestFailed);
        assert!(can_heal);
    }

    #[test]
    fn test_detect_failure_type_network() {
        let (ft, can_heal) = detect_failure_type("ECONNREFUSED: connection refused");
        assert_eq!(ft, InitScriptFailureType::NetworkError);
        assert!(!can_heal);
    }

    #[test]
    fn test_detect_failure_type_disk() {
        let (ft, can_heal) = detect_failure_type("ENOSPC: no space left on device");
        assert_eq!(ft, InitScriptFailureType::DiskFull);
        assert!(!can_heal);
    }

    #[test]
    fn test_detect_failure_type_permission() {
        let (ft, can_heal) = detect_failure_type("EACCES: permission denied");
        assert_eq!(ft, InitScriptFailureType::PermissionDenied);
        assert!(!can_heal);
    }

    #[test]
    fn test_detect_failure_type_unknown() {
        let (ft, can_heal) = detect_failure_type("some random error");
        assert_eq!(ft, InitScriptFailureType::Unknown);
        assert!(!can_heal);
    }

    #[test]
    fn test_run_init_script_not_found() {
        let result = run_init_script("/nonexistent", ".", None::<fn(OrchestratorEvent)>, None);
        assert!(!result.ran);
        assert!(result.success);
    }
}
