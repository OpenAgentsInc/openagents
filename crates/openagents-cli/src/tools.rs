//! Real tool execution runtime for OpenAgents Coder
//! Implements `shell`, `skill`, `openagents`, `capability`, and delegation hooks

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub const OUTPUT_LIMIT: usize = 30_000;
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const MAXIMUM_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub body: String,
}

pub struct HarnessToolRegistry {
    pub cwd: PathBuf,
    pub skills: HashMap<String, SkillInfo>,
}

impl HarnessToolRegistry {
    pub fn new(cwd: Option<PathBuf>) -> Self {
        let root = cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let mut registry = Self {
            cwd: root,
            skills: HashMap::new(),
        };
        registry.load_local_skills();
        registry
    }

    pub fn load_local_skills(&mut self) {
        let mut search_dirs = Vec::new();
        search_dirs.push(self.cwd.join(".agents").join("skills"));
        search_dirs.push(self.cwd.join("packages").join("openagents-cli").join("skills"));
        if let Ok(home) = std::env::var("HOME") {
            search_dirs.push(PathBuf::from(home).join(".agents").join("skills"));
        }

        for dir in search_dirs {
            if !dir.exists() || !dir.is_dir() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let skill_md = if path.is_dir() {
                        path.join("SKILL.md")
                    } else if path.extension().map_or(false, |ext| ext == "md") {
                        path.clone()
                    } else {
                        continue;
                    };

                    if skill_md.exists() {
                        if let Ok(content) = fs::read_to_string(&skill_md) {
                            let (name, desc, body) = parse_skill_markdown(&skill_md, &content);
                            self.skills.insert(name.clone(), SkillInfo {
                                name,
                                description: desc,
                                body,
                            });
                        }
                    }
                }
            }
        }
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let mut skill_list = String::new();
        for (name, info) in &self.skills {
            skill_list.push_str(&format!("\n- `{}`: {}", name, info.description));
        }

        vec![
            ToolDefinition {
                name: "shell".to_string(),
                description: "Run a shell command on this machine. Returns combined stdout and stderr with exit code. Paths are relative to the working directory. Batch independent commands with &&.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "The command line to run through /bin/sh -c."},
                        "timeout_seconds": {"type": "integer", "description": "How long to wait. Defaults to 120; raise for a build or test run."}
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "skill".to_string(),
                description: format!("Read one of this repository skill procedures: a written procedure with conventions, commands, and rules. Call it before doing work a skill covers. Skills available:{}", skill_list),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "The skill to read."}
                    },
                    "required": ["name"]
                }),
            },
            ToolDefinition {
                name: "openagents".to_string(),
                description: "Run the OpenAgents CLI commands (issue, project, repo, auth, etc.). Pass the arguments as a list without openagents itself.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "args": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "The arguments after openagents as a list."
                        }
                    },
                    "required": ["args"]
                }),
            },
        ]
    }

    pub async fn execute_tool(&self, call: &ToolCall) -> ToolOutput {
        match call.name.as_str() {
            "shell" => {
                let cmd = call.arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
                let timeout_secs = call.arguments.get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(DEFAULT_TIMEOUT_SECS)
                    .min(MAXIMUM_TIMEOUT_SECS);

                if let Some(refusal) = check_shell_refusal(cmd) {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: refusal,
                        is_error: true,
                    };
                }

                let output_str = run_real_shell(cmd, &self.cwd, timeout_secs).await;
                ToolOutput {
                    call_id: call.id.clone(),
                    output: output_str,
                    is_error: false,
                }
            }
            "skill" => {
                let name = call.arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(skill_info) = self.skills.get(name) {
                    ToolOutput {
                        call_id: call.id.clone(),
                        output: skill_info.body.clone(),
                        is_error: false,
                    }
                } else {
                    ToolOutput {
                        call_id: call.id.clone(),
                        output: format!("Skill '{}' not found.", name),
                        is_error: true,
                    }
                }
            }
            "openagents" => {
                let args_array = call.arguments.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect::<Vec<_>>())
                    .unwrap_or_default();

                let output_str = run_openagents_cli(&args_array).await;
                ToolOutput {
                    call_id: call.id.clone(),
                    output: output_str,
                    is_error: false,
                }
            }
            _ => ToolOutput {
                call_id: call.id.clone(),
                output: format!("Unknown tool: {}", call.name),
                is_error: true,
            },
        }
    }
}

fn parse_skill_markdown(path: &Path, content: &str) -> (String, String, String) {
    let fallback_name = path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("skill");

    if let Some(after_front) = content.strip_prefix("---") {
        if let Some(end_front) = after_front.find("---") {
            let front_matter = &after_front[..end_front];
            let body = after_front[end_front + 3..].trim().to_string();

            let mut name = fallback_name.to_string();
            let mut desc = String::new();

            for line in front_matter.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("name:") {
                    name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = trimmed.strip_prefix("description:") {
                    desc = val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
            if desc.is_empty() {
                desc = format!("Procedure for {}", name);
            }
            return (name, desc, body);
        }
    }

    (fallback_name.to_string(), format!("Procedure for {}", fallback_name), content.to_string())
}

pub fn check_shell_refusal(cmd: &str) -> Option<String> {
    let lower = cmd.to_lowercase();
    let dangerous = ["rm -rf /", "rm -rf ~", "rm -rf $home"];
    for d in &dangerous {
        if lower.contains(d) {
            return Some("That would erase a root or a home directory. This session refuses it.".to_string());
        }
    }
    None
}

async fn run_real_shell(cmd: &str, cwd: &Path, timeout_secs: u64) -> String {
    let child = match Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return format!("Failed to spawn shell command: {}", e),
    };

    let execution = child.wait_with_output();
    match timeout(Duration::from_secs(timeout_secs), execution).await {
        Ok(Ok(output)) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));

            let total_len = combined.len();
            let bounded = if total_len > OUTPUT_LIMIT {
                format!("{}\n\n[Output truncated: printed {} characters, limit is {}]", &combined[..OUTPUT_LIMIT], total_len, OUTPUT_LIMIT)
            } else {
                combined
            };

            if output.status.success() {
                if bounded.trim().is_empty() {
                    "The command succeeded and printed nothing.".to_string()
                } else {
                    bounded.trim().to_string()
                }
            } else {
                let code = output.status.code().unwrap_or(1);
                format!("The command exited with code {}.\n\n{}", code, bounded.trim())
            }
        }
        Ok(Err(e)) => format!("Shell execution error: {}", e),
        Err(_) => format!("The command timed out after {} seconds and was stopped.", timeout_secs),
    }
}

async fn run_openagents_cli(args: &[String]) -> String {
    let mut cmd = Command::new("openagents");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match cmd.output().await {
        Ok(output) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            combined.trim().to_string()
        }
        Err(e) => format!("Failed to run openagents CLI: {}", e),
    }
}
