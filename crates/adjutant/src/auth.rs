//! Authentication detection for Claude CLI.
//!
//! Uses the CLI's own auth - we just check if it's installed.
//! The Claude CLI handles its own authentication state.

use std::path::PathBuf;

/// Known locations where Claude CLI might be installed.
const CLAUDE_PATHS: &[&str] = &[
    // Standard installation location
    ".claude/local/claude",
    // Homebrew on macOS
    "/opt/homebrew/bin/claude",
    "/usr/local/bin/claude",
    // Linux standard paths
    "/usr/bin/claude",
];

/// Check if Claude CLI is available.
///
/// This doesn't verify authentication - the CLI handles that itself.
/// If the CLI is not authenticated, ClaudeExecutor will fail and
/// we'll fall back to Cerebras TieredExecutor.
///
/// # Checked Paths
///
/// 1. System PATH (via `which::which("claude")`)
/// 2. `~/.claude/local/claude` - Standard installation location
/// 3. `/opt/homebrew/bin/claude` - Homebrew on macOS (Apple Silicon)
/// 4. `/usr/local/bin/claude` - Homebrew on macOS (Intel) or manual install
/// 5. `/usr/bin/claude` - Linux system-wide installation
pub fn has_claude_cli() -> bool {
    // First try PATH lookup
    if which::which("claude").is_ok() {
        return true;
    }

    // Check known installation locations
    if let Some(home) = dirs::home_dir() {
        for path in CLAUDE_PATHS {
            let full_path = if path.starts_with('/') {
                PathBuf::from(path)
            } else {
                home.join(path)
            };
            if full_path.exists() && full_path.is_file() {
                return true;
            }
        }
    }

    false
}

/// Get the path to the Claude CLI executable.
pub fn get_claude_path() -> Option<PathBuf> {
    // First try PATH lookup
    if let Ok(path) = which::which("claude") {
        return Some(path);
    }

    // Check known installation locations
    if let Some(home) = dirs::home_dir() {
        for path in CLAUDE_PATHS {
            let full_path = if path.starts_with('/') {
                PathBuf::from(path)
            } else {
                home.join(path)
            };
            if full_path.exists() && full_path.is_file() {
                return Some(full_path);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_claude_cli() {
        // Just verify the function runs without panic
        // The result depends on whether claude CLI is installed
        let _ = has_claude_cli();
    }

    #[test]
    fn test_get_claude_path() {
        // If has_claude_cli returns true, get_claude_path should return Some
        if has_claude_cli() {
            assert!(get_claude_path().is_some());
        }
    }

    #[test]
    fn test_get_claude_path_exists() {
        // If has_claude_cli returns true, the returned path should exist
        if has_claude_cli() {
            let path = get_claude_path().expect("get_claude_path should return Some when has_claude_cli is true");
            assert!(path.exists(), "Claude CLI path {:?} should exist", path);
        }
    }
}
