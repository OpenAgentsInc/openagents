//! openagents-mcp — a Model Context Protocol server over the running engine.
//!
//! `openagents mcp` speaks MCP (JSON-RPC 2.0, newline-delimited) on stdin/stdout
//! and proxies every tool into the engine's localhost IPC — the same
//! `openagents_rpc` WebSocket the headed app and `openagents sync` dial.
//! The engine supplies its signed-in account to the public SDK for `land_status`
//! and handles chats, devices, projects, and commands. Several tools work locally:
//! `changed_paths` runs `git diff` in a chat's folder, and `build_status`
//! reads this computer's agent builds and writes their limit (#11971).
//! `machine_action` prunes a build cache or stops a process under the
//! owner's grant to the calling chat, and logs it (#12033), and
//! `upgrade_host` starts an upgrade of this computer's app from `main`
//! under the same kind of grant (#12056).
//! `list_runs` reads this computer's terminal agent sessions and identifies
//! claims whose holder is dead or has stopped producing output (#12041).
//! `judge` asks a coordination question set of the Decisions API and
//! returns its receipt (#12042).
//!
//! The server is hand-rolled rather than an SDK: the MCP surface a stdio
//! tool server needs is five methods (`initialize`, `ping`, `tools/list`,
//! `tools/call`, and the `initialized` notification), and the repo already
//! owns JSON-RPC framing for the Codex and ACP drivers. Zero new crates.
//!
//! When the engine launches a harness it can inject this server into the
//! agent's MCP config with `OPENAGENTS_CHAT_ID` / `OPENAGENTS_DEVICE_ID` in the
//! environment. Every send from such an agent is then attributed to its
//! originating chat, and a chat can never message itself.

mod changed_paths;
#[cfg(unix)]
mod build_status;
mod judge;
#[cfg(unix)]
mod machine_action;
#[cfg(unix)]
pub mod machine_status;
mod inbox;
mod jsonrpc;
#[cfg(unix)]
mod runs;
mod tools;
mod transcript;
#[cfg(unix)]
mod upgrade_host;
pub mod video_lab;
mod openagents;
mod work;

pub use jsonrpc::serve_stdio;
pub use tools::{ToolDef, Tools};
pub use transcript::{RenderOptions, RenderedMessage, render_entries};
pub use openagents::{Origin, OpenAgents};

/// How `openagents mcp` finds the engine and who it speaks for.
#[derive(Debug, Clone)]
pub struct McpConfig {
    /// Loopback IPC port of the engine to proxy (`OPENAGENTS_IPC_PORT`, default `openagents_brand::DEFAULT_IPC_PORT`).
    pub ipc_port: u16,
    /// The chat whose agent spawned this server, when injected by the engine.
    pub origin: Origin,
}

impl McpConfig {
    /// Resolve from the process environment: `OPENAGENTS_IPC_PORT` for the engine,
    /// `OPENAGENTS_CHAT_ID` / `OPENAGENTS_DEVICE_ID` for the originating chat.
    pub fn from_env() -> Self {
        let ipc_port = openagents_brand::env_var("OPENAGENTS_IPC_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(openagents_brand::DEFAULT_IPC_PORT);
        Self {
            ipc_port,
            origin: Origin::from_env(),
        }
    }
}

/// Run the MCP server on this process's stdin/stdout until stdin closes.
pub async fn run(config: McpConfig) -> anyhow::Result<()> {
    let openagents = OpenAgents::new(format!("ws://127.0.0.1:{}", config.ipc_port), config.origin);
    video_lab::prime().await;
    let tools = Tools::new(std::sync::Arc::new(openagents));
    serve_stdio(std::sync::Arc::new(tools)).await
}
