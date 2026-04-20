#![allow(
    clippy::print_stdout,
    reason = "headless runtime intentionally prints operator-facing startup information."
)]

use std::path::PathBuf;

use anyhow::Result;
use autopilot_desktop::desktop_control::{
    DESKTOP_CONTROL_BIND_ENV, DESKTOP_CONTROL_MANIFEST_ENV, control_manifest_path,
};
use autopilot_desktop::{DesktopAppOptions, run_desktop_app_with_options};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "autopilot-headless-data-market")]
#[command(about = "No-window Data Market desktop-control host")]
struct Cli {
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    bind: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(manifest_path) = cli.manifest_path.as_ref() {
        // SAFETY: this process sets the override before any worker threads start.
        unsafe {
            std::env::set_var(DESKTOP_CONTROL_MANIFEST_ENV, manifest_path);
        }
    }
    if let Some(bind) = cli.bind.as_ref() {
        // SAFETY: this process sets the override before any worker threads start.
        unsafe {
            std::env::set_var(DESKTOP_CONTROL_BIND_ENV, bind);
        }
    }
    let manifest_path = control_manifest_path();
    println!(
        "starting autopilot-headless-data-market; manifest will be written to {}",
        manifest_path.display()
    );
    let disable_codex = std::env::var("OPENAGENTS_DISABLE_CODEX")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false);
    run_desktop_app_with_options(DesktopAppOptions {
        window_visible: false,
        disable_codex,
    })
}
