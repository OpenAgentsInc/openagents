use std::path::PathBuf;

use agent_client_protocol as acp;
use anyhow::{anyhow, Result};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command};
use tracing::{debug, info, warn, error};

pub struct CodexExecOptions {
    pub bin: PathBuf,
    pub cwd: PathBuf,
    pub extra_args: Vec<String>,
}

pub async fn run_codex_exec_once(
    opts: &CodexExecOptions,
    prompt: &str,
    mut on_update: impl FnMut(acp::SessionUpdate) + Send,
) -> Result<()> {
    let mut args = Vec::new();
    if opts.extra_args.is_empty() {
        args.extend(["exec".to_string(), "--json".to_string()]);
    } else {
        args.extend(opts.extra_args.clone());
        if !args.iter().any(|a| a == "exec") {
            args.insert(0, "exec".to_string());
        }
        if !args.iter().any(|a| a == "--json") {
            args.insert(1, "--json".to_string());
        }
    }
    // Positional prompt (avoid stdin ambiguity)
    args.push(prompt.to_string());

    let mut cmd = Command::new(&opts.bin);
    cmd.current_dir(&opts.cwd)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());

    info!(bin=%opts.bin.display(), args=?args, cwd=%opts.cwd.display(), "spawning codex exec --json");
    let mut child = cmd.spawn().map_err(|e| anyhow!("spawn failed: {e} (bin='{}' args={args:?})", opts.bin.display()))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("missing stdout"))?;
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() { continue; }
        debug!(codex_json_line=%line, "codex stdout line");
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(update) = map_codex_event_to_acp(&v) {
                debug!(mapped=?update, "mapped codex event to ACP");
                on_update(update);
            } else {
                debug!(event=?v, "unmapped codex event");
            }
        }
    }

    Ok(())
}

fn map_codex_event_to_acp(v: &serde_json::Value) -> Option<acp::SessionUpdate> {
    // Try both keys commonly used for event typing
    let kind = v.get("type").or_else(|| v.get("event")).and_then(|s| s.as_str())?.to_ascii_lowercase();
    match kind.as_str() {
        // Agent visible text delta
        "agent_message_content_delta" | "agentmessagecontentdelta" | "agent_message" => {
            let delta = v.get("delta").and_then(|d| d.as_str()).unwrap_or_default().to_string();
            Some(acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk {
                content: acp::ContentBlock::Text(acp::TextContent { annotations: None, text: delta, meta: None }),
                meta: None,
            }))
        }
        // Reasoning/thought text delta
        "reasoning_content_delta" | "reasoning_raw_content_delta" | "agent_reasoning_delta" => {
            let delta = v.get("delta").and_then(|d| d.as_str()).unwrap_or_default().to_string();
            Some(acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk {
                content: acp::ContentBlock::Text(acp::TextContent { annotations: None, text: delta, meta: None }),
                meta: None,
            }))
        }
        // Plan update
        "plan_update" | "plan" => {
            if let Some(arr) = v.get("plan").and_then(|p| p.as_array()) {
                let entries = arr.iter().filter_map(|item| {
                    let content = item.get("step").or_else(|| item.get("content")).and_then(|s| s.as_str())?.to_string();
                    let status = item.get("status").and_then(|s| s.as_str()).unwrap_or("pending").to_ascii_lowercase();
                    let status = match status.as_str() {
                        "in_progress" | "in-progress" => acp::PlanEntryStatus::InProgress,
                        "completed" | "complete" => acp::PlanEntryStatus::Completed,
                        _ => acp::PlanEntryStatus::Pending,
                    };
                    Some(acp::PlanEntry { content, priority: acp::PlanEntryPriority::Medium, status, meta: None })
                }).collect::<Vec<_>>();
                return Some(acp::SessionUpdate::Plan(acp::Plan { entries, meta: None }));
            }
            None
        }
        // Other events not yet mapped (exec/mcp/toolcalls/patches)
        _ => None,
    }
}
