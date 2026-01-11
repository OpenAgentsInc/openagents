//! Authentication detection for Codex CLI.
//!
//! Uses the CLI's own auth - we just check if it's installed.
//! The Codex CLI handles its own authentication state.

use std::path::PathBuf;

/// Known locations where Codex CLI might be installed.
const CODEX_PATHS: &[&str] = &[
    ".npm-global/bin/codex",
    ".local/bin/codex",
    "node_modules/.bin/codex",
    "/usr/local/bin/codex",
    "/opt/homebrew/bin/codex",
];

/// Check if Codex CLI is available.
pub fn has_codex_cli() -> bool {
    if which::which("codex").is_ok() {
        return true;
    }

    if let Some(home) = dirs::home_dir() {
        for path in CODEX_PATHS {
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

/// Get the path to the Codex CLI executable.
pub fn get_codex_path() -> Option<PathBuf> {
    if let Ok(path) = which::which("codex") {
        return Some(path);
    }

    if let Some(home) = dirs::home_dir() {
        for path in CODEX_PATHS {
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
    fn test_has_codex_cli() {
        let _ = has_codex_cli();
    }

    #[test]
    fn test_get_codex_path() {
        if has_codex_cli() {
            assert!(get_codex_path().is_some());
        }
    }

    #[test]
    fn test_get_codex_path_exists() {
        if has_codex_cli() {
            let path = get_codex_path().expect("get_codex_path should return Some when has_codex_cli is true");
            assert!(path.exists(), "Codex CLI path {:?} should exist", path);
        }
    }
}
