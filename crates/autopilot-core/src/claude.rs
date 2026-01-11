use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::logger::SessionLogger;
use crate::startup::ClaudeModel;

use openagents_runtime::{
    ClaudeChunk, ClaudeLocalProvider, ClaudeProvider, ClaudeRequest, ClaudeSessionAutonomy,
    ClaudeSessionState, ClaudeUsage, ChunkType,
};

/// Retry configuration
const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 2000;
const MAX_RETRY_DELAY_MS: u64 = 30000;

/// Calculate retry delay with exponential backoff
fn retry_delay(attempt: u32) -> std::time::Duration {
    let delay_ms = INITIAL_RETRY_DELAY_MS * 2u64.pow(attempt);
    std::time::Duration::from_millis(delay_ms.min(MAX_RETRY_DELAY_MS))
}

/// Extract output from tool_result JSON (stdout, stderr, error)
fn extract_tool_output(tool_result: &serde_json::Value) -> Option<String> {
    // Check for error first
    if let Some(error) = tool_result.get("error").and_then(|e| e.as_str()) {
        if !error.is_empty() {
            return Some(error.to_string());
        }
    }

    // Combine stdout and stderr
    let stdout = tool_result
        .get("stdout")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    let stderr = tool_result
        .get("stderr")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    // Also check for content field (some tools use this)
    let content = tool_result
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    // Build combined output
    let combined = if !stdout.is_empty() && !stderr.is_empty() {
        format!("{}\n\nstderr:\n{}", stdout, stderr)
    } else if !stdout.is_empty() {
        stdout.to_string()
    } else if !stderr.is_empty() {
        stderr.to_string()
    } else if !content.is_empty() {
        content.to_string()
    } else {
        return None;
    };

    // Truncate very large outputs
    if combined.len() > 5000 {
        Some(format!(
            "{}...\n\n({} bytes total)",
            &combined[..5000],
            combined.len()
        ))
    } else {
        Some(combined)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ClaudeToken {
    Chunk(String),
    ToolUse {
        name: String,
        params: String,
    },
    ToolDone {
        name: String,
        output: Option<String>,
        is_error: bool,
    },
    Progress {
        tool_name: String,
        elapsed_secs: f64,
    },
    SessionId(String),
    Done(String),
    Error(String),
    /// Usage update from SDK Result
    Usage(ClaudeUsageData),
}

/// Usage data from Claude SDK Result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClaudeUsageData {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_cost_usd: f64,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
    pub num_turns: Option<u32>,
    pub context_window: Option<u64>,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClaudeEvent {
    Text(String),
    Tool {
        name: String,
        params: String,
        done: bool,
        output: Option<String>,
        is_error: bool,
    },
    ToolProgress {
        tool_name: String,
        elapsed_secs: f64,
    },
}

/// Timeout between stream messages (if no message received in this time, abort)
const STREAM_IDLE_TIMEOUT_SECS: u64 = 300;

pub fn run_claude_planning(
    _cwd: &Path,
    _issue_summary: &str,
    _assessment: &str,
    model: ClaudeModel,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    if let Some(ref log) = logger {
        log.log_phase_start("planning");
    }

    let prompt = r#"You are an expert software architect creating a comprehensive plan for the OpenAgents project.

Plan mode is active. You MUST NOT make any edits to files other than the plan output. You are only allowed to take READ-ONLY actions (Read, Glob, Grep, Bash for git commands, Task with Explore agents).

## Plan Workflow

### Phase 1: Initial Understanding
Goal: Gain comprehensive understanding of the project state and what needs to be done.

1. **Gather repository context:**
   - Run `git log --oneline -10` to see recent commits
   - Run `git branch --show-current` to see current branch
   - Run `git status --short` to see any uncommitted changes

2. **Launch up to 3 Explore agents IN PARALLEL** (single message, multiple Task tool calls) to efficiently explore the codebase:
   - Agent 1: Read README.md and SYNTHESIS.md to understand project vision and architecture
   - Agent 2: Explore .openagents/ directory (directives, issues, TODO.md) to find open work
   - Agent 3: Search for recent changes, blocked issues, or areas needing attention

   Use fewer agents if the scope is clear. Quality over quantity - use minimum agents necessary.

3. After exploration, synthesize what you learned about the project state.

### Phase 2: Design
Goal: Design an implementation approach based on Phase 1 findings.

1. Map the problem to specific files/modules.
2. Identify dependencies or constraints (tests, build system, API contracts).
3. Propose a step-by-step implementation plan.
4. Note any potential risks or tradeoffs.

### Phase 3: Final Output
Goal: Provide the actionable plan.

- Output a numbered list of steps.
- Each step should be specific (file paths, changes).
- Include test/verification steps.
- Keep it concise but complete.

Begin now."#;

    run_claude_session(
        "planning",
        prompt,
        model,
        resume_session_id,
        tx,
        logger,
    );
}

pub fn run_claude_execution(
    plan: &str,
    model: ClaudeModel,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    if let Some(ref log) = logger {
        log.log_phase_start("execution");
    }

    let prompt = format!(
        r#"You are an autonomous software engineer executing a plan for the OpenAgents project.

## The Plan

{}

## Your Task

Execute this plan step by step. For each recommended action:
1. Implement the changes using the available tools (Read, Edit, Write, Bash, etc.)
2. Verify your changes work (run tests, check compilation)
3. Commit completed work with clear commit messages

Work autonomously. Make real changes to the codebase. If you encounter blockers, note them and move to the next actionable item.

Start now."#,
        plan
    );

    run_claude_session(
        "execution",
        &prompt,
        model,
        resume_session_id,
        tx,
        logger,
    );
}

pub fn run_claude_review(
    plan: &str,
    model: ClaudeModel,
    repo_summary: &str,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    if let Some(ref log) = logger {
        log.log_phase_start("review");
    }

    let prompt = format!(
        r#"You are an expert code reviewer for the OpenAgents project.

## Plan Summary
{}

## Repository Summary
{}

## Your Task

Review the executed changes for correctness and completeness. Focus on:
- Behavior regressions
- Missing tests
- Edge cases
- Architecture consistency

Provide a concise review with any issues and recommendations.

Start now."#,
        plan, repo_summary
    );

    run_claude_session(
        "review",
        &prompt,
        model,
        resume_session_id,
        tx,
        logger,
    );
}

fn run_claude_session(
    phase: &str,
    prompt: &str,
    model: ClaudeModel,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(ClaudeToken::Error(format!(
                "Failed to create runtime: {}",
                e
            )));
            return;
        }
    };

    rt.block_on(async {
        let mut attempt = 0;
        let mut last_error = String::new();

        loop {
            if attempt > 0 {
                let delay = retry_delay(attempt - 1);
                eprintln!(
                    "[CLAUDE-{}] Retry attempt {} after {:?}...",
                    phase.to_uppercase(),
                    attempt,
                    delay
                );
                tokio::time::sleep(delay).await;
            }

            if attempt >= MAX_RETRIES {
                eprintln!(
                    "[CLAUDE-{}] All {} retries exhausted. Last error: {}",
                    phase.to_uppercase(),
                    MAX_RETRIES,
                    last_error
                );
                let _ = tx.send(ClaudeToken::Error(format!(
                    "Failed after {} retries: {}",
                    MAX_RETRIES, last_error
                )));
                return;
            }

            attempt += 1;

            let provider = match build_provider() {
                Ok(provider) => provider,
                Err(err) => {
                    last_error = err;
                    continue;
                }
            };

            let resume_session_id = resume_session_id.clone().filter(|id| !id.is_empty());
            let request = build_request(prompt, model, resume_session_id.clone());
            let session_id = match provider.create_session(request) {
                Ok(id) => id,
                Err(err) => {
                    last_error = err.to_string();
                    continue;
                }
            };

            let _ = tx.send(ClaudeToken::SessionId(session_id.clone()));

            let started_at = Instant::now();
            let mut full_response = String::new();
            let mut last_output = Instant::now();
            let mut tool_timings: std::collections::HashMap<String, Instant> =
                std::collections::HashMap::new();

            loop {
                match provider.poll_output(&session_id) {
                    Ok(Some(chunk)) => {
                        last_output = Instant::now();
                        if let Some(success) = handle_chunk(
                            phase,
                            &chunk,
                            &mut full_response,
                            &mut tool_timings,
                            &tx,
                            logger.as_ref(),
                        ) {
                            if let Some(usage) = usage_from_state(
                                provider.get_session(&session_id),
                                started_at,
                                model.as_str(),
                            ) {
                                let _ = tx.send(ClaudeToken::Usage(usage));
                            }
                            if success {
                                let _ = tx.send(ClaudeToken::Done(full_response));
                            }
                            return;
                        }
                    }
                    Ok(None) => {
                        if last_output.elapsed().as_secs() > STREAM_IDLE_TIMEOUT_SECS {
                            last_error = format!(
                                "Stream idle timeout after {}s",
                                STREAM_IDLE_TIMEOUT_SECS
                            );
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    Err(err) => {
                        last_error = err.to_string();
                        break;
                    }
                }
            }
        }
    });
}

fn build_provider() -> Result<Arc<dyn ClaudeProvider>, String> {
    ClaudeLocalProvider::new()
        .map(|provider| Arc::new(provider) as Arc<dyn ClaudeProvider>)
        .map_err(|err| err.to_string())
}

fn build_request(
    prompt: &str,
    model: ClaudeModel,
    resume_session_id: Option<String>,
) -> ClaudeRequest {
    let mut request = ClaudeRequest::new(model.as_str());
    request.autonomy = Some(ClaudeSessionAutonomy::Full);
    if let Some(session_id) = resume_session_id {
        request.resume_session_id = Some(session_id);
        request.initial_prompt = Some(String::new());
    } else {
        request.initial_prompt = Some(prompt.to_string());
    }
    request
}

fn handle_chunk(
    phase: &str,
    chunk: &ClaudeChunk,
    full_response: &mut String,
    tool_timings: &mut std::collections::HashMap<String, Instant>,
    tx: &mpsc::Sender<ClaudeToken>,
    logger: Option<&SessionLogger>,
) -> Option<bool> {
    match chunk.chunk_type {
        ChunkType::Text => {
            if let Some(delta) = chunk.delta.as_deref() {
                full_response.push_str(delta);
                if let Some(log) = logger {
                    let message = serde_json::Value::String(delta.to_string());
                    log.log_assistant(phase, &message);
                }
                let _ = tx.send(ClaudeToken::Chunk(delta.to_string()));
            }
            None
        }
        ChunkType::ToolStart => {
            if let Some(tool) = chunk.tool.as_ref() {
                let params = extract_tool_params(&tool.name, tool.params.as_ref());
                if let Some(log) = logger {
                    log.log_tool_use(phase, &tool.name, tool.params.as_ref().unwrap_or(&serde_json::Value::Null));
                }
                tool_timings.insert(tool.name.clone(), Instant::now());
                let _ = tx.send(ClaudeToken::ToolUse {
                    name: tool.name.clone(),
                    params,
                });
            }
            None
        }
        ChunkType::ToolOutput | ChunkType::ToolDone => {
            if let Some(tool) = chunk.tool.as_ref() {
                let output = tool
                    .result
                    .as_ref()
                    .and_then(extract_tool_output)
                    .or_else(|| tool.error.clone());
                let is_error = tool.error.is_some()
                    || tool
                        .result
                        .as_ref()
                        .and_then(|value| value.get("error"))
                        .is_some();
                if let Some(log) = logger {
                    if let Some(result) = tool.result.as_ref() {
                        log.log_tool_result(phase, &tool.name, result);
                    }
                }
                if let Some(started_at) = tool_timings.get(&tool.name) {
                    let _ = tx.send(ClaudeToken::Progress {
                        tool_name: tool.name.clone(),
                        elapsed_secs: started_at.elapsed().as_secs_f64(),
                    });
                }
                tool_timings.remove(&tool.name);
                let _ = tx.send(ClaudeToken::ToolDone {
                    name: tool.name.clone(),
                    output,
                    is_error,
                });
            }
            None
        }
        ChunkType::Done => Some(true),
        ChunkType::Error => {
            let message = chunk
                .delta
                .clone()
                .unwrap_or_else(|| "Claude error".to_string());
            let _ = tx.send(ClaudeToken::Error(message));
            Some(false)
        }
    }
}

fn usage_from_state(
    state: Option<ClaudeSessionState>,
    started_at: Instant,
    model: &str,
) -> Option<ClaudeUsageData> {
    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    match state {
        Some(ClaudeSessionState::Complete(response)) => {
            let usage = response.usage.unwrap_or(ClaudeUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: 0,
            });
            Some(ClaudeUsageData {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                total_cost_usd: response.cost_usd as f64 / 1_000_000.0,
                duration_ms: Some(elapsed_ms),
                duration_api_ms: None,
                num_turns: None,
                context_window: None,
                model: response.model,
            })
        }
        Some(ClaudeSessionState::Idle { usage, cost_usd, .. }) => {
            let usage = usage.unwrap_or(ClaudeUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: 0,
            });
            Some(ClaudeUsageData {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                total_cost_usd: cost_usd as f64 / 1_000_000.0,
                duration_ms: Some(elapsed_ms),
                duration_api_ms: None,
                num_turns: None,
                context_window: None,
                model: model.to_string(),
            })
        }
        _ => None,
    }
}

fn extract_tool_params(tool_name: &str, input: Option<&serde_json::Value>) -> String {
    match tool_name {
        "Read" | "read" => input
            .and_then(|i| i.get("file_path").or_else(|| i.get("filePath")))
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
        "Bash" | "bash" => input
            .and_then(|i| i.get("command"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string(),
        "Glob" | "glob" => input
            .and_then(|i| i.get("pattern"))
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
        "Grep" | "grep" => input
            .and_then(|i| i.get("pattern"))
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
        "Edit" | "edit" | "Write" | "write" => input
            .and_then(|i| i.get("file_path").or_else(|| i.get("filePath")))
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
        "Task" | "task" => input
            .and_then(|i| i.get("description"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string(),
        _ => input
            .and_then(|i| i.get("description"))
            .and_then(|d| d.as_str())
            .or_else(|| input.and_then(|i| i.get("prompt")).and_then(|p| p.as_str()))
            .unwrap_or("")
            .to_string(),
    }
}
