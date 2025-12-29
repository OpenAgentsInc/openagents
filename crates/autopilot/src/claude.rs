use std::path::Path;
use std::sync::mpsc;

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

#[derive(Clone)]
pub enum ClaudeToken {
    Chunk(String),
    ToolUse { name: String, params: String },
    ToolDone { name: String },
    Done(String),
    Error(String),
}

#[derive(Clone)]
pub enum ClaudeEvent {
    Text(String),
    Tool { name: String, params: String, done: bool },
}

pub fn run_claude_planning(
    _cwd: &Path,
    _issue_summary: &str,
    _assessment: &str,
    model: ClaudeModel,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    use futures_util::StreamExt;
    
    if let Some(ref log) = logger {
        log.log_phase_start("planning");
    }
    
    let prompt = r#"You are an expert software architect and project planner for the OpenAgents project.

## Your Task

1. First, gather context about this repository:
   - Run `git log --oneline -10` to see recent commits
   - Run `git branch --show-current` to see current branch
   - Run `git status --short` to see any uncommitted changes

2. Read the key project files:
   - Read README.md in the root directory
   - Read SYNTHESIS.md in the root directory (this is the comprehensive vision document)
   - Read all files in the .openagents/ directory (use Glob to find them, then Read each one)
     - This includes directives, issues, TODO.md, etc.

3. Based on what you learn, create a comprehensive plan in Markdown format:
   - **Current State** - What's the project about, what has been built
   - **Recent Activity** - What was worked on recently based on commits
   - **Open Work** - What issues/directives are pending
   - **Recommended Next Steps** - Prioritized list of what to work on next
   - **Technical Considerations** - Any blockers, dependencies, or architectural notes

Focus on being actionable and specific. This plan will guide what an autonomous agent works on next."#.to_string();

    let cwd_clone = std::env::current_dir().unwrap_or_default();
    
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(ClaudeToken::Error(format!("Failed to create runtime: {}", e)));
            return;
        }
    };

    rt.block_on(async {
        use claude_agent_sdk::{query, QueryOptions, SdkMessage, PermissionMode};
        
        verbose_println!("[CLAUDE] Starting query with prompt length: {} chars", prompt.len());
        verbose_println!("[CLAUDE] Options: cwd={:?}, model={}, permission_mode=Plan, max_turns=50", cwd_clone, model.as_str());
        
        let options = QueryOptions::new()
            .cwd(cwd_clone)
            .model(model.as_str())
            .permission_mode(PermissionMode::Plan)
            .max_turns(50);
        
        let mut stream = match query(&prompt, options).await {
            Ok(s) => {
                verbose_println!("[CLAUDE] Stream started successfully");
                s
            }
            Err(e) => {
                eprintln!("[CLAUDE] Failed to start: {}", e);
                let _ = tx.send(ClaudeToken::Error(format!("Failed to start Claude: {}", e)));
                return;
            }
        };

        let mut full_response = String::new();
        let mut last_tool_name = String::new();
        
        while let Some(msg) = stream.next().await {
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
                        let _ = tx.send(ClaudeToken::ToolDone { name: tool_name });
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
                }
                Ok(SdkMessage::AuthStatus(auth)) => {
                    verbose_println!("[CLAUDE][AUTH] is_authenticating={}", auth.is_authenticating);
                }
                Ok(SdkMessage::Result(result)) => {
                    verbose_println!("[CLAUDE][RESULT] {:?}", result);
                    break;
                }
                Err(e) => {
                    eprintln!("[CLAUDE][ERROR] Stream error: {}", e);
                    let _ = tx.send(ClaudeToken::Error(format!("Stream error: {}", e)));
                    return;
                }
            }
        }

        verbose_println!("[CLAUDE] Done. Total response length: {} chars, {} lines",
            full_response.len(),
            full_response.lines().count());
        if let Some(ref log) = logger {
            log.log_phase_end("planning", &format!("{} chars, {} lines", full_response.len(), full_response.lines().count()));
        }
        let _ = tx.send(ClaudeToken::Done(full_response));
    });
}

pub fn run_claude_execution(
    plan: &str,
    model: ClaudeModel,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    use futures_util::StreamExt;
    
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
        use claude_agent_sdk::{query, QueryOptions, SdkMessage, PermissionMode};
        
        verbose_println!("[CLAUDE-EXEC] Starting execution with plan length: {} chars", plan.len());
        verbose_println!("[CLAUDE-EXEC] Options: cwd={:?}, model={}, permission_mode=BypassPermissions, max_turns=100", cwd_clone, model.as_str());
        
        let options = QueryOptions::new()
            .cwd(cwd_clone)
            .model(model.as_str())
            .permission_mode(PermissionMode::BypassPermissions)
            .dangerously_skip_permissions(true)
            .max_turns(100);
        
        let mut stream = match query(&prompt, options).await {
            Ok(s) => {
                verbose_println!("[CLAUDE-EXEC] Stream started successfully");
                s
            }
            Err(e) => {
                eprintln!("[CLAUDE-EXEC] Failed to start: {}", e);
                let _ = tx.send(ClaudeToken::Error(format!("Failed to start Claude: {}", e)));
                return;
            }
        };

        let mut full_response = String::new();
        let mut last_tool_name = String::new();
        
        while let Some(msg) = stream.next().await {
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
                        let _ = tx.send(ClaudeToken::ToolDone { name: tool_name });
                    }
                }
                Ok(SdkMessage::Result(_)) => {
                    break;
                }
                Err(e) => {
                    let _ = tx.send(ClaudeToken::Error(format!("Stream error: {}", e)));
                    return;
                }
                _ => {}
            }
        }
        
        verbose_println!("[CLAUDE-EXEC] Done. Total response length: {} chars", full_response.len());
        if let Some(ref log) = logger {
            log.log_phase_end("execution", &format!("{} chars", full_response.len()));
        }
        let _ = tx.send(ClaudeToken::Done(full_response));
    });
}

/// Review the work done against the original plan and determine next steps
pub fn run_claude_review(
    plan: &str,
    execution_result: &str,
    iteration: u32,
    model: ClaudeModel,
    tx: mpsc::Sender<ClaudeToken>,
    logger: Option<SessionLogger>,
) {
    use futures_util::StreamExt;
    
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
        use claude_agent_sdk::{query, QueryOptions, SdkMessage, PermissionMode};
        
        verbose_println!("[CLAUDE-REVIEW] Starting review iteration {}", iteration);
        verbose_println!("[CLAUDE-REVIEW] Options: cwd={:?}, model={}, permission_mode=BypassPermissions, max_turns=30", cwd_clone, model.as_str());
        
        let options = QueryOptions::new()
            .cwd(cwd_clone)
            .model(model.as_str())
            .permission_mode(PermissionMode::BypassPermissions)
            .dangerously_skip_permissions(true)
            .max_turns(30);
        
        let mut stream = match query(&prompt, options).await {
            Ok(s) => {
                verbose_println!("[CLAUDE-REVIEW] Stream started successfully");
                s
            }
            Err(e) => {
                eprintln!("[CLAUDE-REVIEW] Failed to start: {}", e);
                let _ = tx.send(ClaudeToken::Error(format!("Failed to start Claude: {}", e)));
                return;
            }
        };

        let mut full_response = String::new();
        let mut last_tool_name = String::new();
        
        while let Some(msg) = stream.next().await {
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
                        let _ = tx.send(ClaudeToken::ToolDone { name: tool_name });
                    }
                }
                Ok(SdkMessage::Result(_)) => {
                    break;
                }
                Err(e) => {
                    let _ = tx.send(ClaudeToken::Error(format!("Stream error: {}", e)));
                    return;
                }
                _ => {}
            }
        }
        
        verbose_println!("[CLAUDE-REVIEW] Done. Total response length: {} chars", full_response.len());
        if let Some(ref log) = logger {
            log.log_phase_end(&format!("review_{}", iteration), &format!("{} chars", full_response.len()));
        }
        let _ = tx.send(ClaudeToken::Done(full_response));
    });
}

fn extract_tool_params(tool_name: &str, input: Option<&serde_json::Value>) -> String {
    match tool_name {
        "Read" | "read" => input
            .and_then(|i| i.get("filePath"))
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
            .and_then(|i| i.get("filePath"))
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
