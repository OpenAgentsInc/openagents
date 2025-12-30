use std::path::Path;
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

use crate::logger::SessionLogger;
use crate::startup::ClaudeModel;

/// Check if verbose mode is enabled
fn is_verbose() -> bool {
    std::env::var("AUTOPILOT_VERBOSE").is_ok()
}

/// Print only if verbose mode is enabled
macro_rules! verbose_println {
    ($($arg:tt)*) => {
        if is_verbose() {
            eprintln!($($arg)*);
        }
    };
}

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

fn extract_session_id(message: &claude_agent_sdk::SdkMessage) -> Option<&str> {
    use claude_agent_sdk::{SdkMessage, SdkResultMessage, SdkSystemMessage};

    match message {
        SdkMessage::Assistant(msg) => Some(msg.session_id.as_str()),
        SdkMessage::User(msg) => Some(msg.session_id.as_str()),
        SdkMessage::Result(result) => match result {
            SdkResultMessage::Success(msg) => Some(msg.session_id.as_str()),
            SdkResultMessage::ErrorDuringExecution(msg)
            | SdkResultMessage::ErrorMaxTurns(msg)
            | SdkResultMessage::ErrorMaxBudget(msg)
            | SdkResultMessage::ErrorMaxStructuredOutputRetries(msg) => {
                Some(msg.session_id.as_str())
            }
        },
        SdkMessage::System(system) => Some(match system {
            SdkSystemMessage::Init(msg) => msg.session_id.as_str(),
            SdkSystemMessage::CompactBoundary(msg) => msg.session_id.as_str(),
            SdkSystemMessage::Status(msg) => msg.session_id.as_str(),
            SdkSystemMessage::HookResponse(msg) => msg.session_id.as_str(),
            SdkSystemMessage::ApiError(msg) => msg.session_id.as_str(),
            SdkSystemMessage::StopHookSummary(msg) => msg.session_id.as_str(),
            SdkSystemMessage::Informational(msg) => msg.session_id.as_str(),
            SdkSystemMessage::LocalCommand(msg) => msg.session_id.as_str(),
        }),
        SdkMessage::StreamEvent(event) => Some(event.session_id.as_str()),
        SdkMessage::ToolProgress(progress) => Some(progress.session_id.as_str()),
        SdkMessage::AuthStatus(auth) => Some(auth.session_id.as_str()),
    }
}

fn maybe_send_session_id(
    session_id: &str,
    last_sent: &mut Option<String>,
    tx: &mpsc::Sender<ClaudeToken>,
) {
    if last_sent.as_deref() != Some(session_id) {
        let _ = tx.send(ClaudeToken::SessionId(session_id.to_string()));
        *last_sent = Some(session_id.to_string());
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ClaudeToken {
    Chunk(String),
    ToolUse { name: String, params: String },
    ToolDone {
        name: String,
        output: Option<String>,
        is_error: bool,
    },
    Progress { tool_name: String, elapsed_secs: f64 },
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
    pub cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: u32,
    pub context_window: u64,
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
    ToolProgress { tool_name: String, elapsed_secs: f64 },
}

pub fn run_claude_planning(
    _cwd: &Path,
    _issue_summary: &str,
    _assessment: &str,
    model: ClaudeModel,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    use futures_util::StreamExt;
    use tokio::time::{timeout, Duration};

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

Launch 1-3 Plan agent(s) to design the implementation:
- **Default**: Launch at least 1 Plan agent to validate understanding and consider alternatives
- **Multiple agents**: Use for complex tasks benefiting from different perspectives:
  - Perspective 1: Focus on highest business value / revenue impact
  - Perspective 2: Focus on technical debt / infrastructure improvements
  - Perspective 3: Focus on quick wins / unblocking other work

In the agent prompt, provide:
- Comprehensive context from Phase 1 exploration
- Current blockers and constraints
- Request a prioritized implementation plan

### Phase 3: Review
Goal: Review and consolidate the plan(s) from Phase 2.

1. Read any critical files identified by the Plan agents
2. Ensure recommendations align with project priorities (check SYNTHESIS.md, directives)
3. Resolve conflicts between different perspectives

### Phase 4: Final Plan
Goal: Output your final consolidated plan.

Create a comprehensive plan in Markdown format with these sections:
- **Current State** - Project status, what's been built, recent activity
- **Open Work** - Pending issues, directives, their priorities and blockers
- **Recommended Next Steps** - Prioritized list of what to work on next (be specific!)
  - Include file paths that need to be modified
  - Include specific implementation steps
  - Estimate complexity (small/medium/large)
- **Technical Considerations** - Blockers, dependencies, architectural notes

The plan must be:
- Actionable and specific (not vague suggestions)
- Prioritized by business value and feasibility
- Executable by an autonomous agent

### Phase 5: Exit
Call ExitPlanMode when your plan is complete.

Your turn should only end with calling ExitPlanMode. Do not stop early."#.to_string();

    let cwd_clone = std::env::current_dir().unwrap_or_default();

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(ClaudeToken::Error(format!("Failed to create runtime: {}", e)));
            return;
        }
    };

    rt.block_on(async {
        use claude_agent_sdk::{query, QueryOptions, SdkMessage, PermissionMode, SettingSource};

        let mut attempt = 0;
        let mut last_error = String::new();

        'retry: loop {
            if attempt > 0 {
                let delay = retry_delay(attempt - 1);
                eprintln!("[CLAUDE] Retry attempt {} after {:?}...", attempt, delay);
                tokio::time::sleep(delay).await;
            }

            if attempt >= MAX_RETRIES {
                eprintln!("[CLAUDE] All {} retries exhausted. Last error: {}", MAX_RETRIES, last_error);
                let _ = tx.send(ClaudeToken::Error(format!("Failed after {} retries: {}", MAX_RETRIES, last_error)));
                return;
            }

            attempt += 1;
            let resume_session_id = resume_session_id.clone().filter(|id| !id.is_empty());
            if let Some(ref session_id) = resume_session_id {
                verbose_println!("[CLAUDE] Resuming session {}", session_id);
            }
            verbose_println!("[CLAUDE] Starting query (attempt {}/{}) with prompt length: {} chars", attempt, MAX_RETRIES, prompt.len());
            verbose_println!("[CLAUDE] Options: cwd={:?}, model={}, permission_mode=Plan, max_turns=50", cwd_clone, model.as_str());

            let mut options = QueryOptions::new()
                .cwd(cwd_clone.clone())
                .model(model.as_str())
                .permission_mode(PermissionMode::Plan)
                .max_turns(50)
                .include_partial_messages(true)
                .setting_sources(vec![SettingSource::Project, SettingSource::User])
                .disallowed_tools(vec![
                    "Edit".to_string(),
                    "Write".to_string(),
                    "NotebookEdit".to_string(),
                ]);

            if let Some(ref session_id) = resume_session_id {
                options = options.resume(session_id.clone());
            }

            let prompt_to_send = if resume_session_id.is_some() { "" } else { &prompt };

            // Start query with timeout
            let query_result = timeout(
                Duration::from_secs(QUERY_START_TIMEOUT_SECS),
                query(prompt_to_send, options)
            ).await;

            let mut stream = match query_result {
                Ok(Ok(s)) => {
                    verbose_println!("[CLAUDE] Stream started successfully");
                    if resume_session_id.is_none() {
                        // Send initial usage estimate based on prompt size (rough: ~4 chars per token)
                        let estimated_input = (prompt.len() / 4) as u64;
                        let _ = tx.send(ClaudeToken::Usage(ClaudeUsageData {
                            input_tokens: estimated_input,
                            output_tokens: 0,
                            cache_read_tokens: 0,
                            cache_creation_tokens: 0,
                            total_cost_usd: 0.0,
                            duration_ms: 0,
                            duration_api_ms: 0,
                            num_turns: 1,
                            context_window: 200_000,
                            model: model.as_str().to_string(),
                        }));
                    }
                    s
                }
                Ok(Err(e)) => {
                    last_error = format!("Failed to start Claude: {}", e);
                    eprintln!("[CLAUDE] {}", last_error);
                    continue 'retry;
                }
                Err(_) => {
                    last_error = format!("Timeout starting query after {}s", QUERY_START_TIMEOUT_SECS);
                    eprintln!("[CLAUDE] {}", last_error);
                    continue 'retry;
                }
            };

            let mut session_id: Option<String> = None;
            let mut full_response = String::new();
            let mut last_tool_name = String::new();

            loop {
                let next_result = timeout(
                    Duration::from_secs(STREAM_IDLE_TIMEOUT_SECS),
                    stream.next()
                ).await;

                let msg = match next_result {
                    Ok(Some(msg)) => msg,
                    Ok(None) => {
                        // Stream ended - this is normal when Result was already received
                        if full_response.is_empty() {
                            last_error = "Stream ended unexpectedly with no data".to_string();
                            eprintln!("[CLAUDE] {}", last_error);
                            let _ = stream.abort().await;
                            continue 'retry;
                        }
                        break;
                    }
                    Err(_) => {
                        last_error = format!("Stream idle timeout after {}s", STREAM_IDLE_TIMEOUT_SECS);
                        eprintln!("[CLAUDE] {}", last_error);
                        let _ = stream.abort().await;
                        continue 'retry;
                    }
                };

                if let Ok(ref sdk_msg) = msg {
                    if let Some(id) = extract_session_id(sdk_msg) {
                        maybe_send_session_id(id, &mut session_id, &tx);
                    }
                }

                match msg {
                    Ok(SdkMessage::Assistant(assistant_msg)) => {
                        verbose_println!("[CLAUDE][ASSISTANT] uuid={}", assistant_msg.uuid);
                        verbose_println!("[CLAUDE][ASSISTANT] message={}", serde_json::to_string_pretty(&assistant_msg.message).unwrap_or_default());
                        if let Some(ref log) = logger {
                            log.log_assistant("planning", &assistant_msg.message);
                        }
                        if let Some(content) = assistant_msg.message.get("content") {
                            if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    verbose_println!("[CLAUDE][BLOCK] type={}", block.get("type").and_then(|t| t.as_str()).unwrap_or("unknown"));
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        verbose_println!("[CLAUDE][TEXT] {}", text);
                                        full_response.push_str(text);
                                        let _ = tx.send(ClaudeToken::Chunk(text.to_string()));
                                    }
                                    if let Some(tool_name) = block.get("name").and_then(|n| n.as_str()) {
                                        verbose_println!("[CLAUDE][TOOL_USE] name={}", tool_name);
                                        let input = block.get("input");
                                        let params = extract_tool_params(tool_name, input);
                                        last_tool_name = tool_name.to_string();
                                        if let Some(ref log) = logger {
                                            log.log_tool_use("planning", tool_name, input.unwrap_or(&serde_json::Value::Null));
                                        }
                                        let _ = tx.send(ClaudeToken::ToolUse {
                                            name: tool_name.to_string(),
                                            params
                                        });
                                        verbose_println!("[CLAUDE][TOOL_USE] input={}", serde_json::to_string_pretty(input.unwrap_or(&serde_json::Value::Null)).unwrap_or_default());
                                    }
                                }
                            }
                        }
                    }
                    Ok(SdkMessage::User(user_msg)) => {
                        verbose_println!("[CLAUDE][USER] session_id={}", user_msg.session_id);
                        verbose_println!("[CLAUDE][USER] message={}", serde_json::to_string_pretty(&user_msg.message).unwrap_or_default());
                        if let Some(ref log) = logger {
                            log.log_user("planning", &user_msg.message);
                        }
                        if let Some(tool_result) = &user_msg.tool_use_result {
                            let tool_name = tool_result.get("name")
                                .and_then(|n| n.as_str())
                                .or_else(|| tool_result.get("tool_name").and_then(|n| n.as_str()))
                                .unwrap_or(&last_tool_name)
                                .to_string();
                            if let Some(ref log) = logger {
                                log.log_tool_result("planning", &tool_name, tool_result);
                            }
                            // Extract output and error status from tool_result
                            let output = extract_tool_output(tool_result);
                            let is_error = tool_result
                                .get("is_error")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let _ = tx.send(ClaudeToken::ToolDone {
                                name: tool_name,
                                output,
                                is_error,
                            });
                            verbose_println!("[CLAUDE][TOOL_RESULT] {}", serde_json::to_string_pretty(tool_result).unwrap_or_default());
                        }
                    }
                    Ok(SdkMessage::System(sys_msg)) => {
                        verbose_println!("[CLAUDE][SYSTEM] {:?}", sys_msg);
                    }
                    Ok(SdkMessage::StreamEvent(event)) => {
                        verbose_println!("[CLAUDE][STREAM] {:?}", event);
                    }
                    Ok(SdkMessage::ToolProgress(progress)) => {
                        verbose_println!("[CLAUDE][TOOL_PROGRESS] tool={} elapsed={}s", progress.tool_name, progress.elapsed_time_seconds);
                        let _ = tx.send(ClaudeToken::Progress {
                            tool_name: progress.tool_name.clone(),
                            elapsed_secs: progress.elapsed_time_seconds,
                        });
                    }
                    Ok(SdkMessage::AuthStatus(auth)) => {
                        verbose_println!("[CLAUDE][AUTH] is_authenticating={}", auth.is_authenticating);
                    }
                    Ok(SdkMessage::Result(result)) => {
                        verbose_println!("[CLAUDE][RESULT] {:?}", result);
                        // Log result to session logger
                        if let Some(ref log) = logger {
                            log.log_result("planning", &serde_json::to_value(&result).unwrap_or_default());
                        }
                        // Extract final usage data and send it (replaces initial estimate)
                        let usage = extract_usage_from_result(&result, model.as_str());
                        let _ = tx.send(ClaudeToken::Usage(usage));
                        break;
                    }
                    Err(e) => {
                        last_error = format!("Stream error: {}", e);
                        eprintln!("[CLAUDE][ERROR] {}", last_error);
                        let _ = stream.abort().await;
                        continue 'retry;
                    }
                }
            }

            // Success - cleanup and return
            verbose_println!("[CLAUDE] Aborting stream for cleanup...");
            if let Err(e) = stream.abort().await {
                verbose_println!("[CLAUDE] Abort returned error (expected): {}", e);
            }

            verbose_println!("[CLAUDE] Done. Total response length: {} chars, {} lines",
                full_response.len(),
                full_response.lines().count());
            if let Some(ref log) = logger {
                log.log_phase_end("planning", &format!("{} chars, {} lines", full_response.len(), full_response.lines().count()));
            }
            let _ = tx.send(ClaudeToken::Done(full_response));

            // Small delay to let process cleanup complete before next query
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }
    });
}

/// Timeout for starting a query (getting the stream)
const QUERY_START_TIMEOUT_SECS: u64 = 60;
/// Timeout between stream messages (if no message received in this time, abort)
const STREAM_IDLE_TIMEOUT_SECS: u64 = 300;

pub fn run_claude_execution(
    plan: &str,
    model: ClaudeModel,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    use futures_util::StreamExt;
    use tokio::time::{timeout, Duration};

    if let Some(ref log) = logger {
        log.log_phase_start("execution");
    }

    let prompt = format!(r#"You are an autonomous software engineer executing a plan for the OpenAgents project.

## The Plan

{}

## Your Task

Execute this plan step by step. For each recommended action:
1. Implement the changes using the available tools (Read, Edit, Write, Bash, etc.)
2. Verify your changes work (run tests, check compilation)
3. Commit completed work with clear commit messages

Work autonomously. Make real changes to the codebase. If you encounter blockers, note them and move to the next actionable item.

Start now."#, plan);

    let cwd_clone = std::env::current_dir().unwrap_or_default();

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(ClaudeToken::Error(format!("Failed to create runtime: {}", e)));
            return;
        }
    };

    rt.block_on(async {
        use claude_agent_sdk::{query, QueryOptions, SdkMessage, PermissionMode, SettingSource};

        let mut attempt = 0;
        let mut last_error = String::new();

        'retry: loop {
            if attempt > 0 {
                let delay = retry_delay(attempt - 1);
                eprintln!("[CLAUDE-EXEC] Retry attempt {} after {:?}...", attempt, delay);
                tokio::time::sleep(delay).await;
            }

            if attempt >= MAX_RETRIES {
                eprintln!("[CLAUDE-EXEC] All {} retries exhausted. Last error: {}", MAX_RETRIES, last_error);
                let _ = tx.send(ClaudeToken::Error(format!("Failed after {} retries: {}", MAX_RETRIES, last_error)));
                return;
            }

            attempt += 1;
            let resume_session_id = resume_session_id.clone().filter(|id| !id.is_empty());
            if let Some(ref session_id) = resume_session_id {
                verbose_println!("[CLAUDE-EXEC] Resuming session {}", session_id);
            }
            verbose_println!("[CLAUDE-EXEC] Starting execution (attempt {}/{}) with plan length: {} chars", attempt, MAX_RETRIES, plan.len());
            verbose_println!("[CLAUDE-EXEC] Options: cwd={:?}, model={}, permission_mode=BypassPermissions, max_turns=100", cwd_clone, model.as_str());

            let mut options = QueryOptions::new()
                .cwd(cwd_clone.clone())
                .model(model.as_str())
                .permission_mode(PermissionMode::BypassPermissions)
                .dangerously_skip_permissions(true)
                .max_turns(100)
                .include_partial_messages(true)
                .setting_sources(vec![SettingSource::Project, SettingSource::User]);

            if let Some(ref session_id) = resume_session_id {
                options = options.resume(session_id.clone());
            }

            let prompt_to_send = if resume_session_id.is_some() { "" } else { &prompt };

            verbose_println!("[CLAUDE-EXEC] Calling query() with {}s timeout...", QUERY_START_TIMEOUT_SECS);
            let query_result = timeout(
                Duration::from_secs(QUERY_START_TIMEOUT_SECS),
                query(prompt_to_send, options)
            ).await;

            let mut stream = match query_result {
                Ok(Ok(s)) => {
                    verbose_println!("[CLAUDE-EXEC] Stream started successfully");
                    s
                }
                Ok(Err(e)) => {
                    last_error = format!("Failed to start Claude: {}", e);
                    eprintln!("[CLAUDE-EXEC] {}", last_error);
                    continue 'retry;
                }
                Err(_) => {
                    last_error = format!("Timeout starting query after {}s", QUERY_START_TIMEOUT_SECS);
                    eprintln!("[CLAUDE-EXEC] {}", last_error);
                    continue 'retry;
                }
            };

            let mut session_id: Option<String> = None;
            let mut full_response = String::new();
            let mut last_tool_name = String::new();

            loop {
                let next_result = timeout(
                    Duration::from_secs(STREAM_IDLE_TIMEOUT_SECS),
                    stream.next()
                ).await;

                let msg = match next_result {
                    Ok(Some(msg)) => msg,
                    Ok(None) => {
                        if full_response.is_empty() {
                            last_error = "Stream ended unexpectedly with no data".to_string();
                            eprintln!("[CLAUDE-EXEC] {}", last_error);
                            let _ = stream.abort().await;
                            continue 'retry;
                        }
                        verbose_println!("[CLAUDE-EXEC] Stream ended (None)");
                        break;
                    }
                    Err(_) => {
                        last_error = format!("Stream idle timeout after {}s", STREAM_IDLE_TIMEOUT_SECS);
                        eprintln!("[CLAUDE-EXEC] {}", last_error);
                        let _ = stream.abort().await;
                        continue 'retry;
                    }
                };

                if let Ok(ref sdk_msg) = msg {
                    if let Some(id) = extract_session_id(sdk_msg) {
                        maybe_send_session_id(id, &mut session_id, &tx);
                    }
                }

                match msg {
                    Ok(SdkMessage::Assistant(assistant_msg)) => {
                        if let Some(ref log) = logger {
                            log.log_assistant("execution", &assistant_msg.message);
                        }
                        if let Some(content) = assistant_msg.message.get("content") {
                            if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        full_response.push_str(text);
                                        let _ = tx.send(ClaudeToken::Chunk(text.to_string()));
                                    }
                                    if let Some(tool_name) = block.get("name").and_then(|n| n.as_str()) {
                                        let input = block.get("input");
                                        let params = extract_tool_params(tool_name, input);
                                        last_tool_name = tool_name.to_string();
                                        if let Some(ref log) = logger {
                                            log.log_tool_use("execution", tool_name, input.unwrap_or(&serde_json::Value::Null));
                                        }
                                        let _ = tx.send(ClaudeToken::ToolUse {
                                            name: tool_name.to_string(),
                                            params
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Ok(SdkMessage::User(user_msg)) => {
                        if let Some(ref log) = logger {
                            log.log_user("execution", &user_msg.message);
                        }
                        if let Some(tool_result) = &user_msg.tool_use_result {
                            let tool_name = tool_result.get("name")
                                .and_then(|n| n.as_str())
                                .or_else(|| tool_result.get("tool_name").and_then(|n| n.as_str()))
                                .unwrap_or(&last_tool_name)
                                .to_string();
                            if let Some(ref log) = logger {
                                log.log_tool_result("execution", &tool_name, tool_result);
                            }
                            // Extract output and error status from tool_result
                            let output = extract_tool_output(tool_result);
                            let is_error = tool_result
                                .get("is_error")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let _ = tx.send(ClaudeToken::ToolDone {
                                name: tool_name,
                                output,
                                is_error,
                            });
                        }
                    }
                    Ok(SdkMessage::StreamEvent(event)) => {
                        verbose_println!("[CLAUDE-EXEC][STREAM] {:?}", event);
                    }
                    Ok(SdkMessage::ToolProgress(progress)) => {
                        verbose_println!(
                            "[CLAUDE-EXEC][TOOL_PROGRESS] tool={} elapsed={}s",
                            progress.tool_name,
                            progress.elapsed_time_seconds
                        );
                        let _ = tx.send(ClaudeToken::Progress {
                            tool_name: progress.tool_name.clone(),
                            elapsed_secs: progress.elapsed_time_seconds,
                        });
                    }
                    Ok(SdkMessage::Result(result)) => {
                        verbose_println!("[CLAUDE-EXEC] Got Result message");
                        let usage = extract_usage_from_result(&result, model.as_str());
                        let _ = tx.send(ClaudeToken::Usage(usage));
                        break;
                    }
                    Err(e) => {
                        last_error = format!("Stream error: {}", e);
                        eprintln!("[CLAUDE-EXEC][ERROR] {}", last_error);
                        let _ = stream.abort().await;
                        continue 'retry;
                    }
                    _ => {}
                }
            }

            // Success - cleanup and return
            verbose_println!("[CLAUDE-EXEC] Aborting stream for cleanup...");
            if let Err(e) = stream.abort().await {
                verbose_println!("[CLAUDE-EXEC] Abort returned error (expected): {}", e);
            }

            verbose_println!("[CLAUDE-EXEC] Done. Total response length: {} chars", full_response.len());
            if let Some(ref log) = logger {
                log.log_phase_end("execution", &format!("{} chars", full_response.len()));
            }
            let _ = tx.send(ClaudeToken::Done(full_response));

            // Small delay to let process cleanup complete before next query
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }
    });
}

/// Review the work done against the original plan and determine next steps
pub fn run_claude_review(
    plan: &str,
    execution_result: &str,
    iteration: u32,
    model: ClaudeModel,
    resume_session_id: Option<String>,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    use futures_util::StreamExt;
    use tokio::time::{timeout, Duration};

    if let Some(ref log) = logger {
        log.log_phase_start(&format!("review_{}", iteration));
    }

    let prompt = format!(r#"You are reviewing work done by an autonomous software engineer on the OpenAgents project.

## Iteration {iteration}

## Original Plan

{plan}

## Work Completed

{execution_result}

## Your Task

1. Review what was accomplished vs what was planned
2. Identify any remaining work from the original plan
3. Check for any new issues or improvements discovered during execution
4. Read the git log to see what commits were made
5. Check git status for any uncommitted changes

Based on your review, create a NEW plan for the next iteration that includes:
- Any uncompleted items from the original plan
- Any new issues discovered
- Any improvements or follow-up work identified

If ALL planned work is complete and no new issues were found, respond with:
"CYCLE COMPLETE - All planned work has been finished."

Otherwise, provide a detailed plan for the next iteration in the same format as the original plan."#);

    let cwd_clone = std::env::current_dir().unwrap_or_default();

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(ClaudeToken::Error(format!("Failed to create runtime: {}", e)));
            return;
        }
    };

    rt.block_on(async {
        use claude_agent_sdk::{query, QueryOptions, SdkMessage, PermissionMode, SettingSource};

        let mut attempt = 0;
        let mut last_error = String::new();

        'retry: loop {
            if attempt > 0 {
                let delay = retry_delay(attempt - 1);
                eprintln!("[CLAUDE-REVIEW] Retry attempt {} after {:?}...", attempt, delay);
                tokio::time::sleep(delay).await;
            }

            if attempt >= MAX_RETRIES {
                eprintln!("[CLAUDE-REVIEW] All {} retries exhausted. Last error: {}", MAX_RETRIES, last_error);
                let _ = tx.send(ClaudeToken::Error(format!("Failed after {} retries: {}", MAX_RETRIES, last_error)));
                return;
            }

            attempt += 1;
            let resume_session_id = resume_session_id.clone().filter(|id| !id.is_empty());
            if let Some(ref session_id) = resume_session_id {
                verbose_println!("[CLAUDE-REVIEW] Resuming session {}", session_id);
            }
            verbose_println!("[CLAUDE-REVIEW] Starting review iteration {} (attempt {}/{})", iteration, attempt, MAX_RETRIES);
            verbose_println!("[CLAUDE-REVIEW] Options: cwd={:?}, model={}, permission_mode=BypassPermissions, max_turns=30", cwd_clone, model.as_str());

            let mut options = QueryOptions::new()
                .cwd(cwd_clone.clone())
                .model(model.as_str())
                .permission_mode(PermissionMode::BypassPermissions)
                .dangerously_skip_permissions(true)
                .max_turns(30)
                .include_partial_messages(true)
                .setting_sources(vec![SettingSource::Project, SettingSource::User]);

            if let Some(ref session_id) = resume_session_id {
                options = options.resume(session_id.clone());
            }

            let prompt_to_send = if resume_session_id.is_some() { "" } else { &prompt };

            let query_result = timeout(
                Duration::from_secs(QUERY_START_TIMEOUT_SECS),
                query(prompt_to_send, options)
            ).await;

            let mut stream = match query_result {
                Ok(Ok(s)) => {
                    verbose_println!("[CLAUDE-REVIEW] Stream started successfully");
                    s
                }
                Ok(Err(e)) => {
                    last_error = format!("Failed to start Claude: {}", e);
                    eprintln!("[CLAUDE-REVIEW] {}", last_error);
                    continue 'retry;
                }
                Err(_) => {
                    last_error = format!("Timeout starting query after {}s", QUERY_START_TIMEOUT_SECS);
                    eprintln!("[CLAUDE-REVIEW] {}", last_error);
                    continue 'retry;
                }
            };

            let mut session_id: Option<String> = None;
            let mut full_response = String::new();
            let mut last_tool_name = String::new();

            loop {
                let next_result = timeout(
                    Duration::from_secs(STREAM_IDLE_TIMEOUT_SECS),
                    stream.next()
                ).await;

                let msg = match next_result {
                    Ok(Some(msg)) => msg,
                    Ok(None) => {
                        if full_response.is_empty() {
                            last_error = "Stream ended unexpectedly with no data".to_string();
                            eprintln!("[CLAUDE-REVIEW] {}", last_error);
                            let _ = stream.abort().await;
                            continue 'retry;
                        }
                        break;
                    }
                    Err(_) => {
                        last_error = format!("Stream idle timeout after {}s", STREAM_IDLE_TIMEOUT_SECS);
                        eprintln!("[CLAUDE-REVIEW] {}", last_error);
                        let _ = stream.abort().await;
                        continue 'retry;
                    }
                };

                if let Ok(ref sdk_msg) = msg {
                    if let Some(id) = extract_session_id(sdk_msg) {
                        maybe_send_session_id(id, &mut session_id, &tx);
                    }
                }

                match msg {
                    Ok(SdkMessage::Assistant(assistant_msg)) => {
                        if let Some(ref log) = logger {
                            log.log_assistant(&format!("review_{}", iteration), &assistant_msg.message);
                        }
                        if let Some(content) = assistant_msg.message.get("content") {
                            if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        full_response.push_str(text);
                                        let _ = tx.send(ClaudeToken::Chunk(text.to_string()));
                                    }
                                    if let Some(tool_name) = block.get("name").and_then(|n| n.as_str()) {
                                        let input = block.get("input");
                                        let params = extract_tool_params(tool_name, input);
                                        last_tool_name = tool_name.to_string();
                                        if let Some(ref log) = logger {
                                            log.log_tool_use(&format!("review_{}", iteration), tool_name, input.unwrap_or(&serde_json::Value::Null));
                                        }
                                        let _ = tx.send(ClaudeToken::ToolUse {
                                            name: tool_name.to_string(),
                                            params
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Ok(SdkMessage::User(user_msg)) => {
                        if let Some(ref log) = logger {
                            log.log_user(&format!("review_{}", iteration), &user_msg.message);
                        }
                        if let Some(tool_result) = &user_msg.tool_use_result {
                            let tool_name = tool_result.get("name")
                                .and_then(|n| n.as_str())
                                .or_else(|| tool_result.get("tool_name").and_then(|n| n.as_str()))
                                .unwrap_or(&last_tool_name)
                                .to_string();
                            if let Some(ref log) = logger {
                                log.log_tool_result(&format!("review_{}", iteration), &tool_name, tool_result);
                            }
                            // Extract output and error status from tool_result
                            let output = extract_tool_output(tool_result);
                            let is_error = tool_result
                                .get("is_error")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let _ = tx.send(ClaudeToken::ToolDone {
                                name: tool_name,
                                output,
                                is_error,
                            });
                        }
                    }
                    Ok(SdkMessage::StreamEvent(event)) => {
                        verbose_println!("[CLAUDE-REVIEW][STREAM] {:?}", event);
                    }
                    Ok(SdkMessage::ToolProgress(progress)) => {
                        verbose_println!(
                            "[CLAUDE-REVIEW][TOOL_PROGRESS] tool={} elapsed={}s",
                            progress.tool_name,
                            progress.elapsed_time_seconds
                        );
                        let _ = tx.send(ClaudeToken::Progress {
                            tool_name: progress.tool_name.clone(),
                            elapsed_secs: progress.elapsed_time_seconds,
                        });
                    }
                    Ok(SdkMessage::Result(result)) => {
                        // Extract usage data and send it
                        let usage = extract_usage_from_result(&result, model.as_str());
                        verbose_println!("[CLAUDE-REVIEW] Usage: input={}, output={}, cost=${:.4}, turns={}",
                            usage.input_tokens, usage.output_tokens, usage.total_cost_usd, usage.num_turns);
                        let _ = tx.send(ClaudeToken::Usage(usage));
                        break;
                    }
                    Err(e) => {
                        last_error = format!("Stream error: {}", e);
                        eprintln!("[CLAUDE-REVIEW][ERROR] {}", last_error);
                        let _ = stream.abort().await;
                        continue 'retry;
                    }
                    _ => {}
                }
            }

            // Success - cleanup and return
            verbose_println!("[CLAUDE-REVIEW] Aborting stream for cleanup...");
            if let Err(e) = stream.abort().await {
                verbose_println!("[CLAUDE-REVIEW] Abort returned error (expected): {}", e);
            }

            verbose_println!("[CLAUDE-REVIEW] Done. Total response length: {} chars", full_response.len());
            if let Some(ref log) = logger {
                log.log_phase_end(&format!("review_{}", iteration), &format!("{} chars", full_response.len()));
            }
            let _ = tx.send(ClaudeToken::Done(full_response));

            // Small delay to let process cleanup complete before next query
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }
    });
}

/// Extract usage data from SDK Result message into ClaudeUsageData.
fn extract_usage_from_result(result: &claude_agent_sdk::SdkResultMessage, model: &str) -> ClaudeUsageData {
    use claude_agent_sdk::SdkResultMessage;

    // Extract common fields from success or error variants
    let (duration_ms, duration_api_ms, num_turns, total_cost_usd, usage, model_usage) = match result {
        SdkResultMessage::Success(s) => (
            s.duration_ms,
            s.duration_api_ms,
            s.num_turns,
            s.total_cost_usd,
            &s.usage,
            &s.model_usage,
        ),
        SdkResultMessage::ErrorDuringExecution(e)
        | SdkResultMessage::ErrorMaxTurns(e)
        | SdkResultMessage::ErrorMaxBudget(e)
        | SdkResultMessage::ErrorMaxStructuredOutputRetries(e) => (
            e.duration_ms,
            e.duration_api_ms,
            e.num_turns,
            e.total_cost_usd,
            &e.usage,
            &e.model_usage,
        ),
    };

    // Get context window from model_usage if available
    let context_window = model_usage
        .values()
        .next()
        .map(|m| m.context_window)
        .unwrap_or(0);

    ClaudeUsageData {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_input_tokens.unwrap_or(0),
        cache_creation_tokens: usage.cache_creation_input_tokens.unwrap_or(0),
        total_cost_usd,
        duration_ms,
        duration_api_ms,
        num_turns,
        context_window,
        model: model.to_string(),
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
