#![allow(clippy::print_stdout, reason = "This binary speaks MCP over stdio.")]

use std::path::PathBuf;

use anyhow::Result;
use autopilot_desktop::compute_mcp::{ResolvedDesktopControlTarget, run_stdio_server};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "autopilot-compute-mcp")]
#[command(about = "MCP server for OpenAgents compute control over desktop-control")]
struct Cli {
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long)]
    manifest: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let target = ResolvedDesktopControlTarget::resolve(
        cli.base_url.as_deref(),
        cli.auth_token.as_deref(),
        cli.manifest.as_deref(),
    )?;
    run_stdio_server(target)
}
