//! TBench Runner - spawns tbench CLI with streaming output
//!
//! Executes Terminal-Bench tasks using the `tbench` CLI with `--stream` flag,
//! parsing JSON events from stdout and sending them to GPUI for display.

use anyhow::{Context, Result};
use gpui_tokio::Tokio;
use harbor::StreamEvent;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use terminalbench::TBTask;

/// Options for running a TB2 task
#[derive(Clone, Debug)]
pub struct TBRunOptions {
    /// The task to run
    pub task: TBTask,
    /// Model to use (optional override)
    pub model: Option<String>,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Maximum turns
    pub max_turns: u32,
}

impl Default for TBRunOptions {
    fn default() -> Self {
        Self {
            task: TBTask::default(),
            model: None,
            timeout_secs: 3600,
            max_turns: 300,
        }
    }
}

/// Events from the TBench runner
#[derive(Clone, Debug)]
pub enum TBenchRunnerEvent {
    /// Run is starting
    RunStart {
        run_id: String,
        task_id: String,
        task_name: String,
    },
    /// Received a stream event from tbench
    StreamEvent(StreamEvent),
    /// Run completed
    RunComplete {
        run_id: String,
        success: bool,
        turns: u32,
        cost: Option<f64>,
        error: Option<String>,
    },
    /// Error occurred
    Error(String),
}

/// Runner for Terminal-Bench tasks
pub struct TBenchRunner {
    /// Project root / working directory for tbench
    project_root: PathBuf,
    /// Output directory for tbench artifacts
    output_dir: PathBuf,
}

impl TBenchRunner {
    /// Create a new TBench runner
    pub fn new(project_root: PathBuf) -> Self {
        // Use temp dir for tbench outputs
        let output_dir = std::env::temp_dir().join("mechacoder-tbench");
        Self {
            project_root,
            output_dir,
        }
    }

    /// Create with custom output directory
    pub fn with_output_dir(project_root: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            project_root,
            output_dir,
        }
    }

    /// Get the project root
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    /// Get the output directory
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }

    /// Start a TB2 run with streaming output
    ///
    /// Returns a receiver that will emit TBenchRunnerEvents as the run progresses.
    /// Spawns the tbench process on the Tokio runtime.
    pub fn start_run<V: 'static>(
        &self,
        options: TBRunOptions,
        cx: &mut gpui::Context<V>,
    ) -> (String, mpsc::UnboundedReceiver<TBenchRunnerEvent>) {
        let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let (tx, rx) = mpsc::unbounded_channel::<TBenchRunnerEvent>();

        // Ensure output directory exists
        let run_output_dir = self.output_dir.join(&run_id);
        if let Err(e) = std::fs::create_dir_all(&run_output_dir) {
            let _ = tx.send(TBenchRunnerEvent::Error(format!(
                "Failed to create output dir: {}",
                e
            )));
            return (run_id, rx);
        }

        let task = options.task.clone();
        let task_id = task.id.clone();
        let task_name = task.name.clone();
        let instruction = build_instruction(&task);
        let project_root = self.project_root.clone();
        let timeout = options.timeout_secs;
        let max_turns = options.max_turns;
        let model = options.model;
        let run_id_clone = run_id.clone();

        // Send start event
        let _ = tx.send(TBenchRunnerEvent::RunStart {
            run_id: run_id.clone(),
            task_id: task_id.clone(),
            task_name: task_name.clone(),
        });

        // Spawn the tbench process on Tokio runtime
        Tokio::spawn(cx, async move {
            let result = run_tbench_streaming(
                &instruction,
                &project_root,
                &run_output_dir,
                timeout,
                max_turns,
                model.as_deref(),
                tx.clone(),
            )
            .await;

            match result {
                Ok((success, turns, cost, error)) => {
                    let _ = tx.send(TBenchRunnerEvent::RunComplete {
                        run_id: run_id_clone,
                        success,
                        turns,
                        cost,
                        error,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TBenchRunnerEvent::Error(format!("TBench run failed: {}", e)));
                }
            }
        })
        .detach();

        (run_id, rx)
    }
}

/// Build the instruction string for a task
fn build_instruction(task: &TBTask) -> String {
    // Use the task description as the instruction
    // The task runner environment (tb2) provides the actual test harness
    task.description.clone()
}

/// Run tbench with streaming output
async fn run_tbench_streaming(
    instruction: &str,
    cwd: &PathBuf,
    output_dir: &PathBuf,
    timeout_secs: u64,
    max_turns: u32,
    model: Option<&str>,
    tx: mpsc::UnboundedSender<TBenchRunnerEvent>,
) -> Result<(bool, u32, Option<f64>, Option<String>)> {
    let mut args = vec![
        "--instruction".to_string(),
        instruction.to_string(),
        "--output-dir".to_string(),
        output_dir.display().to_string(),
        "--timeout".to_string(),
        timeout_secs.to_string(),
        "--max-turns".to_string(),
        max_turns.to_string(),
        "--stream".to_string(),
    ];

    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }

    info!("Starting tbench with args: {:?}", args);
    info!("Working dir: {}", cwd.display());

    let mut child = Command::new("tbench")
        .args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn tbench CLI. Is it installed and in PATH?")?;

    let stdout = child.stdout.take().expect("stdout was captured");
    let mut stdout_reader = BufReader::new(stdout).lines();

    let mut final_success = false;
    let mut final_turns = 0u32;
    let mut final_cost: Option<f64> = None;
    let mut final_error: Option<String> = None;

    // Process stdout lines as StreamEvent JSON
    while let Ok(Some(line)) = stdout_reader.next_line().await {
        debug!("tbench stdout: {}", line);

        // Try to parse as StreamEvent
        match serde_json::from_str::<StreamEvent>(&line) {
            Ok(event) => {
                // Track completion state
                if let StreamEvent::Complete {
                    success,
                    turns,
                    cost,
                    ref error,
                } = event
                {
                    final_success = success;
                    final_turns = turns;
                    final_cost = cost;
                    final_error = error.clone();
                }

                // Forward the event
                let _ = tx.send(TBenchRunnerEvent::StreamEvent(event));
            }
            Err(e) => {
                // Not a JSON line, might be stderr or debug output
                warn!("Non-JSON line from tbench: {} ({})", line, e);
            }
        }
    }

    // Wait for process to complete
    let status = child.wait().await?;
    if !status.success() && final_error.is_none() {
        final_error = Some(format!(
            "tbench exited with code {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok((final_success, final_turns, final_cost, final_error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_instruction() {
        let task = TBTask {
            id: "test-task".to_string(),
            name: "Test Task".to_string(),
            description: "Do something".to_string(),
            ..Default::default()
        };

        let instruction = build_instruction(&task);
        assert_eq!(instruction, "Do something");
    }

    #[test]
    fn test_run_options_default() {
        let opts = TBRunOptions::default();
        assert_eq!(opts.timeout_secs, 3600);
        assert_eq!(opts.max_turns, 300);
        assert!(opts.model.is_none());
    }
}
