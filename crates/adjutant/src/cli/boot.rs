//! Fast boot helpers for CLI commands.

use std::path::PathBuf;
use std::time::Duration;

use oanix::{boot, boot_with_config, BootConfig, OanixManifest};

use crate::auth::get_codex_path;

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
            name: "codex",
            path: get_codex_path(),
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
