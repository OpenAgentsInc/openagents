//! Child agent delegation engine with live CLI harnesses
//! Supports ox-alpha, opencode, devin (via ACP/CLI), claude, and codex

use crate::cli::CoderArgs;
use crate::runtime::{CoderRuntimeSession, Lane};
use crate::tools::HarnessToolRegistry;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildWorkerTask {
    pub id: usize,
    pub prompt: String,
    pub lane: String,
    pub worktree_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildWorkerResult {
    pub id: usize,
    pub success: bool,
    pub output: String,
    pub duration_ms: u128,
}

pub struct DelegationSupervisor {
    pub count: usize,
    pub lane: String,
    pub user_token: Option<String>,
}

impl DelegationSupervisor {
    pub fn new(count: usize, lane: &str, user_token: Option<String>) -> Self {
        Self {
            count,
            lane: lane.to_string(),
            user_token,
        }
    }

    pub async fn dispatch(&self, prompt: &str) -> Vec<ChildWorkerResult> {
        let mut handles = Vec::new();
        for id in 1..=self.count {
            let task = ChildWorkerTask {
                id,
                prompt: prompt.to_string(),
                lane: self.lane.clone(),
                worktree_path: None,
            };
            let token = self.user_token.clone();
            handles.push(tokio::spawn(async move {
                Self::execute_worker(task, token).await
            }));
        }

        let mut results = Vec::new();
        for handle in join_all(handles).await {
            if let Ok(res) = handle {
                results.push(res);
            }
        }
        results
    }

    async fn execute_worker(task: ChildWorkerTask, user_token: Option<String>) -> ChildWorkerResult {
        let start = Instant::now();
        let lane_str = task.lane.to_lowercase();

        let (success, output) = match lane_str.as_str() {
            "claude" => run_claude_cli(&task.prompt).await,
            "codex" => run_codex_cli(&task.prompt).await,
            "gemini" => run_opencode_cli(&task.prompt, "gemini-3.7-flash").await,
            "devin" => run_devin_cli(&task.prompt).await,
            _ => {
                // Default ox-alpha via live CoderRuntimeSession
                let tools = HarnessToolRegistry::new(None);
                let mut runtime = CoderRuntimeSession::new(Lane::OxAlpha, None, user_token, tools);
                match runtime.execute_turn(&task.prompt, |_| {}).await {
                    Ok(out) => (true, out),
                    Err(e) => (false, format!("Inference error: {}", e)),
                }
            }
        };

        ChildWorkerResult {
            id: task.id,
            success,
            output,
            duration_ms: start.elapsed().as_millis(),
        }
    }
}

async fn run_claude_cli(prompt: &str) -> (bool, String) {
    let mut cmd = Command::new("claude");
    cmd.args(["-p", prompt]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match cmd.output().await {
        Ok(res) => (res.status.success(), String::from_utf8_lossy(&res.stdout).trim().to_string()),
        Err(e) => (false, format!("Failed to spawn claude: {}", e)),
    }
}

async fn run_codex_cli(prompt: &str) -> (bool, String) {
    let mut cmd = Command::new("codex");
    cmd.args(["exec", prompt]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match cmd.output().await {
        Ok(res) => (res.status.success(), String::from_utf8_lossy(&res.stdout).trim().to_string()),
        Err(e) => (false, format!("Failed to spawn codex: {}", e)),
    }
}

async fn run_opencode_cli(prompt: &str, model: &str) -> (bool, String) {
    let mut cmd = Command::new("opencode");
    cmd.args(["run", "--model", model, prompt]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match cmd.output().await {
        Ok(res) => (res.status.success(), String::from_utf8_lossy(&res.stdout).trim().to_string()),
        Err(e) => (false, format!("Failed to spawn opencode: {}", e)),
    }
}

async fn run_devin_cli(prompt: &str) -> (bool, String) {
    let mut cmd = Command::new("devin");
    cmd.args(["--prompt", prompt]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match cmd.output().await {
        Ok(res) => (res.status.success(), String::from_utf8_lossy(&res.stdout).trim().to_string()),
        Err(e) => (false, format!("Failed to spawn devin: {}", e)),
    }
}

pub async fn run_delegation(args: CoderArgs, user_token: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let count = args.count.max(1);
    let lane = args.lane.unwrap_or_else(|| "ox-alpha".to_string());
    let prompt = args.prompt.unwrap_or_else(|| "Analyze workspace and run tests".to_string());

    println!("Starting parallel delegation across {} child workers on lane {}...", count, lane);
    let supervisor = DelegationSupervisor::new(count, &lane, user_token);
    let results = supervisor.dispatch(&prompt).await;

    for res in &results {
        println!("Child {}: status={}, duration={}ms, output={}", res.id, if res.success { "ok" } else { "err" }, res.duration_ms, res.output);
    }
    println!("Delegation fan-out complete. {}/{} children succeeded.", results.iter().filter(|r| r.success).count(), results.len());
    Ok(())
}
