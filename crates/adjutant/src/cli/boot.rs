//! Fast boot helpers for CLI commands.

use std::path::PathBuf;
use std::time::Duration;

use oanix::{boot, boot_with_config, BootConfig, OanixManifest};

use crate::auth::get_claude_path;

#[derive(Debug, Clone)]
pub struct ToolCheck {
    pub name: &'static str,
    pub path: Option<PathBuf>,
}

/// Run a fast OANIX boot for CLI commands (skip slow discovery).
pub async fn boot_fast() -> anyhow::Result<OanixManifest> {
    let config = BootConfig {
        skip_hardware: false,
        skip_compute: true,
        skip_network: true,
        skip_identity: false,
        skip_workspace: false,
        timeout: Duration::from_secs(1),
        retries: 0,
    };
    boot_with_config(config).await
}

/// Run the full OANIX boot.
pub async fn boot_full() -> anyhow::Result<OanixManifest> {
    boot().await
}

/// Quick checks for common agent binaries.
pub fn quick_tool_checks() -> Vec<ToolCheck> {
    vec![
        ToolCheck {
            name: "claude",
            path: get_claude_path(),
        },
        ToolCheck {
            name: "codex",
            path: find_codex_cli(),
        },
    ]
}

/// Print quick tool checks in a compact format.
pub fn print_quick_checks() {
    println!("Quick checks:");
    for check in quick_tool_checks() {
        match check.path {
            Some(path) => println!("  [OK] {}: {}", check.name, path.display()),
            None => println!("  [--] {}: not found", check.name),
        }
    }
    println!();
}

fn find_codex_cli() -> Option<PathBuf> {
    if let Ok(path) = which::which("codex") {
        return Some(path);
    }

    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".npm-global/bin/codex"));
        candidates.push(home.join(".local/bin/codex"));
        candidates.push(home.join("node_modules/.bin/codex"));
    }

    candidates.push(PathBuf::from("/usr/local/bin/codex"));
    candidates.push(PathBuf::from("/opt/homebrew/bin/codex"));

    candidates.into_iter().find(|path| path.exists())
}
