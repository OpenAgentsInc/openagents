use std::path::Path;
use std::sync::mpsc;

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

pub fn run_claude_planning(_cwd: &Path, _issue_summary: &str, _assessment: &str, tx: mpsc::Sender<ClaudeToken>) {
    use futures_util::StreamExt;
    
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
        
        eprintln!("[CLAUDE] Starting query with prompt length: {} chars", prompt.len());
        eprintln!("[CLAUDE] Options: cwd={:?}, permission_mode=Plan, max_turns=50", cwd_clone);
        
        let options = QueryOptions::new()
            .cwd(cwd_clone)
            .permission_mode(PermissionMode::Plan)
            .max_turns(50);
        
        let mut stream = match query(&prompt, options).await {
            Ok(s) => {
                eprintln!("[CLAUDE] Stream started successfully");
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
                    eprintln!("[CLAUDE][ASSISTANT] uuid={}", assistant_msg.uuid);
                    eprintln!("[CLAUDE][ASSISTANT] message={}", serde_json::to_string_pretty(&assistant_msg.message).unwrap_or_default());
                    if let Some(content) = assistant_msg.message.get("content") {
                        if let Some(blocks) = content.as_array() {
                            for block in blocks {
                                if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                                    eprintln!("[CLAUDE][BLOCK] type={}", block_type);
                                }
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    eprintln!("[CLAUDE][TEXT] {}", text);
                                    full_response.push_str(text);
                                    let _ = tx.send(ClaudeToken::Chunk(text.to_string()));
                                }
                                if let Some(tool_name) = block.get("name").and_then(|n| n.as_str()) {
                                    eprintln!("[CLAUDE][TOOL_USE] name={}", tool_name);
                                    let input = block.get("input");
                                    let params = extract_tool_params(tool_name, input);
                                    last_tool_name = tool_name.to_string();
                                    let _ = tx.send(ClaudeToken::ToolUse { 
                                        name: tool_name.to_string(), 
                                        params 
                                    });
                                    if let Some(inp) = input {
                                        eprintln!("[CLAUDE][TOOL_USE] input={}", serde_json::to_string_pretty(inp).unwrap_or_default());
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(SdkMessage::User(user_msg)) => {
                    eprintln!("[CLAUDE][USER] session_id={}", user_msg.session_id);
                    eprintln!("[CLAUDE][USER] message={}", serde_json::to_string_pretty(&user_msg.message).unwrap_or_default());
                    if let Some(tool_result) = &user_msg.tool_use_result {
                        let tool_name = tool_result.get("name")
                            .and_then(|n| n.as_str())
                            .or_else(|| tool_result.get("tool_name").and_then(|n| n.as_str()))
                            .unwrap_or(&last_tool_name)
                            .to_string();
                        let _ = tx.send(ClaudeToken::ToolDone { name: tool_name });
                        eprintln!("[CLAUDE][TOOL_RESULT] {}", serde_json::to_string_pretty(tool_result).unwrap_or_default());
                    }
                }
                Ok(SdkMessage::System(sys_msg)) => {
                    eprintln!("[CLAUDE][SYSTEM] {:?}", sys_msg);
                }
                Ok(SdkMessage::StreamEvent(event)) => {
                    eprintln!("[CLAUDE][STREAM] {:?}", event);
                }
                Ok(SdkMessage::ToolProgress(progress)) => {
                    eprintln!("[CLAUDE][TOOL_PROGRESS] tool={} elapsed={}s", progress.tool_name, progress.elapsed_time_seconds);
                }
                Ok(SdkMessage::AuthStatus(auth)) => {
                    eprintln!("[CLAUDE][AUTH] is_authenticating={}", auth.is_authenticating);
                }
                Ok(SdkMessage::Result(result)) => {
                    eprintln!("[CLAUDE][RESULT] {:?}", result);
                    break;
                }
                Err(e) => {
                    eprintln!("[CLAUDE][ERROR] Stream error: {}", e);
                    let _ = tx.send(ClaudeToken::Error(format!("Stream error: {}", e)));
                    return;
                }
            }
        }
        
        eprintln!("[CLAUDE] Done. Total response length: {} chars, {} lines", 
            full_response.len(), 
            full_response.lines().count());
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
