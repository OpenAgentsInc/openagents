//! MCP server exposing issue tracking tools
//!
//! This server implements the Model Context Protocol (MCP) over stdio,
//! providing tools for managing issues in the local SQLite database.

use anyhow::Result;
use issues::{db, issue, Priority, IssueType, Status};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Mutex;

/// JSON-RPC request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Tool definition for MCP
#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

/// MCP server state
struct McpServer {
    conn: Mutex<Connection>,
}

impl McpServer {
    fn new(db_path: PathBuf) -> Result<Self> {
        let conn = db::init_db(&db_path)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone().unwrap_or(Value::Null);

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&request.params),
            "notifications/initialized" => {
                // Notification, no response needed
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(Value::Null),
                    error: None,
                };
            }
            _ => Err(format!("Unknown method: {}", request.method)),
        };

        match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(value),
                error: None,
            },
            Err(msg) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: msg,
                }),
            },
        }
    }

    fn handle_initialize(&self, _params: &Value) -> Result<Value, String> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "issues-mcp",
                "version": "0.1.0"
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, String> {
        let tools = vec![
            Tool {
                name: "issue_list".to_string(),
                description: "List issues, optionally filtered by status".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "status": {
                            "type": "string",
                            "enum": ["open", "in_progress", "done"],
                            "description": "Filter by status"
                        }
                    }
                }),
            },
            Tool {
                name: "issue_create".to_string(),
                description: "Create a new issue".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Issue title"
                        },
                        "description": {
                            "type": "string",
                            "description": "Issue description"
                        },
                        "priority": {
                            "type": "string",
                            "enum": ["urgent", "high", "medium", "low"],
                            "description": "Priority level"
                        },
                        "issue_type": {
                            "type": "string",
                            "enum": ["task", "bug", "feature"],
                            "description": "Issue type"
                        },
                        "agent": {
                            "type": "string",
                            "enum": ["claude", "codex"],
                            "description": "Agent to assign (default: claude)"
                        }
                    },
                    "required": ["title"]
                }),
            },
            Tool {
                name: "issue_get".to_string(),
                description: "Get an issue by its number".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "number": {
                            "type": "integer",
                            "description": "Issue number"
                        }
                    },
                    "required": ["number"]
                }),
            },
            Tool {
                name: "issue_claim".to_string(),
                description: "Claim an issue for the current run".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "number": {
                            "type": "integer",
                            "description": "Issue number"
                        },
                        "run_id": {
                            "type": "string",
                            "description": "Run ID claiming this issue"
                        }
                    },
                    "required": ["number", "run_id"]
                }),
            },
            Tool {
                name: "issue_complete".to_string(),
                description: "Mark an issue as complete".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "number": {
                            "type": "integer",
                            "description": "Issue number"
                        }
                    },
                    "required": ["number"]
                }),
            },
            Tool {
                name: "issue_block".to_string(),
                description: "Block an issue with a reason".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "number": {
                            "type": "integer",
                            "description": "Issue number"
                        },
                        "reason": {
                            "type": "string",
                            "description": "Reason for blocking"
                        }
                    },
                    "required": ["number", "reason"]
                }),
            },
            Tool {
                name: "issue_ready".to_string(),
                description: "Get the next ready issue (highest priority, not blocked, not claimed)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "agent": {
                            "type": "string",
                            "enum": ["claude", "codex"],
                            "description": "Filter by agent (optional)"
                        }
                    }
                }),
            },
            Tool {
                name: "issue_update".to_string(),
                description: "Update an issue's title, description, priority, or type".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "number": {
                            "type": "integer",
                            "description": "Issue number"
                        },
                        "title": {
                            "type": "string",
                            "description": "New title"
                        },
                        "description": {
                            "type": "string",
                            "description": "New description"
                        },
                        "priority": {
                            "type": "string",
                            "enum": ["urgent", "high", "medium", "low"],
                            "description": "New priority level"
                        },
                        "issue_type": {
                            "type": "string",
                            "enum": ["task", "bug", "feature"],
                            "description": "New issue type"
                        }
                    },
                    "required": ["number"]
                }),
            },
            Tool {
                name: "enter_plan_mode".to_string(),
                description: "Enter planning mode to explore and design before implementing. Creates a plan file and enables restrictions.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "slug": {
                            "type": "string",
                            "description": "Short identifier for the plan (used in filename)"
                        },
                        "goal": {
                            "type": "string",
                            "description": "The goal or objective to plan for"
                        }
                    },
                    "required": ["slug", "goal"]
                }),
            },
            Tool {
                name: "exit_plan_mode".to_string(),
                description: "Exit planning mode after completing the plan. Verifies plan has content and lifts restrictions.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "advance_plan_phase".to_string(),
                description: "Advance to the next plan mode phase (Explore -> Design -> Review -> Final -> Exit). Returns phase-specific guidance.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "get_current_phase".to_string(),
                description: "Get the current plan mode phase and its guidance prompt.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "issue_delete".to_string(),
                description: "Delete an issue (hard delete). Use for cleanup and testing only.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "number": {
                            "type": "integer",
                            "description": "Issue number"
                        }
                    },
                    "required": ["number"]
                }),
            },
        ];

        Ok(json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, params: &Value) -> Result<Value, String> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing tool name")?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let result = match name {
            "issue_list" => self.tool_issue_list(&conn, &arguments),
            "issue_create" => self.tool_issue_create(&conn, &arguments),
            "issue_get" => self.tool_issue_get(&conn, &arguments),
            "issue_claim" => self.tool_issue_claim(&conn, &arguments),
            "issue_complete" => self.tool_issue_complete(&conn, &arguments),
            "issue_block" => self.tool_issue_block(&conn, &arguments),
            "issue_ready" => self.tool_issue_ready(&conn, &arguments),
            "issue_update" => self.tool_issue_update(&conn, &arguments),
            "issue_delete" => self.tool_issue_delete(&conn, &arguments),
            "enter_plan_mode" => self.tool_enter_plan_mode(&arguments),
            "exit_plan_mode" => self.tool_exit_plan_mode(),
            "advance_plan_phase" => self.tool_advance_plan_phase(),
            "get_current_phase" => self.tool_get_current_phase(),
            _ => Err(format!("Unknown tool: {}", name)),
        };

        match result {
            Ok(content) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": content
                }]
            })),
            Err(e) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: {}", e)
                }],
                "isError": true
            })),
        }
    }

    fn tool_issue_list(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "open" => Status::Open,
                "in_progress" => Status::InProgress,
                "done" => Status::Done,
                _ => Status::Open,
            });

        let issues = issue::list_issues(conn, status).map_err(|e| e.to_string())?;

        let output: Vec<Value> = issues
            .iter()
            .map(|i| {
                json!({
                    "number": i.number,
                    "title": i.title,
                    "status": i.status.as_str(),
                    "priority": i.priority.as_str(),
                    "agent": i.agent,
                    "is_blocked": i.is_blocked
                })
            })
            .collect();

        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
    }

    fn tool_issue_create(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("Missing title")?;

        let description = args.get("description").and_then(|v| v.as_str());

        let priority = args
            .get("priority")
            .and_then(|v| v.as_str())
            .map(Priority::from_str)
            .unwrap_or(Priority::Medium);

        let issue_type = args
            .get("issue_type")
            .and_then(|v| v.as_str())
            .map(IssueType::from_str)
            .unwrap_or(IssueType::Task);

        let agent = args.get("agent").and_then(|v| v.as_str());

        let created =
            issue::create_issue(conn, title, description, priority, issue_type, agent)
                .map_err(|e| e.to_string())?;

        Ok(format!(
            "Created issue #{}: {} (agent: {})",
            created.number, created.title, created.agent
        ))
    }

    fn tool_issue_get(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or("Missing number")? as i32;

        match issue::get_issue_by_number(conn, number).map_err(|e| e.to_string())? {
            Some(i) => {
                let output = json!({
                    "number": i.number,
                    "title": i.title,
                    "description": i.description,
                    "status": i.status.as_str(),
                    "priority": i.priority.as_str(),
                    "issue_type": i.issue_type.as_str(),
                    "agent": i.agent,
                    "is_blocked": i.is_blocked,
                    "blocked_reason": i.blocked_reason,
                    "claimed_by": i.claimed_by,
                    "created_at": i.created_at.to_rfc3339(),
                    "updated_at": i.updated_at.to_rfc3339()
                });
                serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
            }
            None => Ok(format!("Issue #{} not found", number)),
        }
    }

    fn tool_issue_claim(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or("Missing number")? as i32;

        let run_id = args
            .get("run_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing run_id")?;

        let i = issue::get_issue_by_number(conn, number)
            .map_err(|e| e.to_string())?
            .ok_or(format!("Issue #{} not found", number))?;

        if issue::claim_issue(conn, &i.id, run_id).map_err(|e| e.to_string())? {
            Ok(format!("Claimed issue #{}", number))
        } else {
            Ok(format!(
                "Could not claim issue #{} (already claimed or blocked)",
                number
            ))
        }
    }

    fn tool_issue_complete(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or("Missing number")? as i32;

        let i = issue::get_issue_by_number(conn, number)
            .map_err(|e| e.to_string())?
            .ok_or(format!("Issue #{} not found", number))?;

        if issue::complete_issue(conn, &i.id).map_err(|e| e.to_string())? {
            Ok(format!("Completed issue #{}", number))
        } else {
            Ok(format!("Could not complete issue #{}", number))
        }
    }

    fn tool_issue_block(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or("Missing number")? as i32;

        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or("Missing reason")?;

        let i = issue::get_issue_by_number(conn, number)
            .map_err(|e| e.to_string())?
            .ok_or(format!("Issue #{} not found", number))?;

        if issue::block_issue(conn, &i.id, reason).map_err(|e| e.to_string())? {
            Ok(format!("Blocked issue #{}: {}", number, reason))
        } else {
            Ok(format!("Could not block issue #{}", number))
        }
    }

    fn tool_issue_ready(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let agent = args.get("agent").and_then(|v| v.as_str());

        match issue::get_next_ready_issue(conn, agent).map_err(|e| e.to_string())? {
            Some(i) => {
                let output = json!({
                    "number": i.number,
                    "title": i.title,
                    "description": i.description,
                    "priority": i.priority.as_str(),
                    "issue_type": i.issue_type.as_str(),
                    "agent": i.agent
                });
                serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
            }
            None => Ok("No ready issues available".to_string()),
        }
    }

    fn tool_issue_update(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or("Missing number")? as i32;

        let title = args.get("title").and_then(|v| v.as_str());
        let description = args.get("description").and_then(|v| v.as_str());
        let priority = args
            .get("priority")
            .and_then(|v| v.as_str())
            .map(Priority::from_str);
        let issue_type = args
            .get("issue_type")
            .and_then(|v| v.as_str())
            .map(IssueType::from_str);

        let i = issue::get_issue_by_number(conn, number)
            .map_err(|e| e.to_string())?
            .ok_or(format!("Issue #{} not found", number))?;

        if issue::update_issue(conn, &i.id, title, description, priority, issue_type)
            .map_err(|e| e.to_string())?
        {
            Ok(format!("Updated issue #{}", number))
        } else {
            Ok(format!("No changes made to issue #{}", number))
        }
    }

    fn tool_issue_delete(&self, conn: &Connection, args: &Value) -> Result<String, String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or("Missing number")? as i32;

        let i = issue::get_issue_by_number(conn, number)
            .map_err(|e| e.to_string())?
            .ok_or(format!("Issue #{} not found", number))?;

        if issue::delete_issue(conn, &i.id).map_err(|e| e.to_string())? {
            Ok(format!("Deleted issue #{}", number))
        } else {
            Ok(format!("Could not delete issue #{}", number))
        }
    }

    fn tool_enter_plan_mode(&self, args: &Value) -> Result<String, String> {
        let slug = args
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or("Missing slug")?;

        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or("Missing goal")?;

        // Call autopilot's plan mode module
        let config = autopilot::planmode::PlanModeConfig::new(slug, goal);
        autopilot::planmode::enter_plan_mode(config)
    }

    fn tool_exit_plan_mode(&self) -> Result<String, String> {
        autopilot::planmode::exit_plan_mode()
    }

    fn tool_advance_plan_phase(&self) -> Result<String, String> {
        autopilot::planmode::advance_phase()
    }

    fn tool_get_current_phase(&self) -> Result<String, String> {
        let phase = autopilot::planmode::get_current_phase();
        let phase_prompt = autopilot::planmode::get_phase_prompt(phase);
        Ok(format!(
            "Current phase: {}\n\n{}",
            phase.as_str().to_uppercase(),
            phase_prompt
        ))
    }
}

fn main() -> Result<()> {
    // Get database path from environment or use default
    let db_path = std::env::var("ISSUES_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("autopilot.db"));

    let server = McpServer::new(db_path)?;

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Failed to parse request: {}", e);
                continue;
            }
        };

        let response = server.handle_request(&request);

        // Don't send response for notifications (no id)
        if request.id.is_none() {
            continue;
        }

        let response_json = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", response_json)?;
        stdout.flush()?;
    }

    Ok(())
}
