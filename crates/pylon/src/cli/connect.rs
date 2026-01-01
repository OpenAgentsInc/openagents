//! pylon connect - Connect a local Claude tunnel client.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use openagents_relay::ClaudeSessionAutonomy;

use crate::claude_tunnel::{run_tunnel_client, ClaudeTunnelConfig};
use crate::config::PylonConfig;

/// Claude autonomy options for the tunnel client.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AutonomyArg {
    Full,
    Supervised,
    Restricted,
    ReadOnly,
}

impl From<AutonomyArg> for ClaudeSessionAutonomy {
    fn from(value: AutonomyArg) -> Self {
        match value {
            AutonomyArg::Full => ClaudeSessionAutonomy::Full,
            AutonomyArg::Supervised => ClaudeSessionAutonomy::Supervised,
            AutonomyArg::Restricted => ClaudeSessionAutonomy::Restricted,
            AutonomyArg::ReadOnly => ClaudeSessionAutonomy::ReadOnly,
        }
    }
}

/// Arguments for the connect command.
#[derive(Args)]
pub struct ConnectArgs {
    /// Tunnel WebSocket URL from /api/tunnel/register.
    #[arg(long)]
    pub tunnel_url: String,

    /// Override the default Claude model.
    #[arg(long)]
    pub model: Option<String>,

    /// Override the autonomy policy.
    #[arg(long, value_enum)]
    pub autonomy: Option<AutonomyArg>,

    /// Maximum cost per session (micro-USD).
    #[arg(long)]
    pub max_cost_usd: Option<u64>,

    /// Working directory for Claude sessions.
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Path to the Claude executable.
    #[arg(long)]
    pub executable_path: Option<PathBuf>,

    /// Custom config file path.
    #[arg(long, short)]
    pub config: Option<String>,
}

/// Run the connect command.
pub async fn run(args: ConnectArgs) -> anyhow::Result<()> {
    let config = if let Some(ref path) = args.config {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content)?
    } else {
        PylonConfig::load()?
    };

    if !config.claude.enabled {
        println!("Claude tunnel support is disabled in config.");
        println!("Set [claude].enabled = true in ~/.config/pylon/config.toml");
        return Err(anyhow::anyhow!("Claude tunnel disabled"));
    }

    let claude_cfg = config.claude.clone();
    let tunnel_config = ClaudeTunnelConfig {
        model: args.model.unwrap_or_else(|| claude_cfg.model.clone()),
        autonomy: args
            .autonomy
            .map(ClaudeSessionAutonomy::from)
            .unwrap_or_else(|| claude_cfg.autonomy.clone()),
        approval_required_tools: claude_cfg.approval_required_tools.clone(),
        allowed_tools: claude_cfg.allowed_tools.clone(),
        blocked_tools: claude_cfg.blocked_tools.clone(),
        max_cost_usd: args.max_cost_usd.or(claude_cfg.max_cost_usd),
        cwd: args.cwd.or(claude_cfg.cwd.clone()),
        executable_path: args.executable_path.or(claude_cfg.executable_path.clone()),
    };

    run_tunnel_client(args.tunnel_url, tunnel_config).await
}
