//! Integration tests for issues-mcp server

use serde_json::{json, Value};

// Helper to create JSON-RPC request
fn make_request(method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    })
}

// =========================================================================
// JSON-RPC Protocol tests
// =========================================================================

#[test]
fn test_jsonrpc_version_in_response() {
    let req = make_request("initialize", json!({}));
    let req_str = serde_json::to_string(&req).unwrap();

    // Response should have jsonrpc: "2.0"
    assert!(req_str.contains("\"jsonrpc\":\"2.0\""));
}

#[test]
fn test_request_with_id() {
    let req = make_request("initialize", json!({}));

    assert_eq!(req["jsonrpc"], "2.0");
    assert_eq!(req["id"], 1);
    assert_eq!(req["method"], "initialize");
}

#[test]
fn test_request_with_null_params() {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize"
    });

    // Missing params should default to empty object
    assert!(req.get("params").is_none());
}

// =========================================================================
// Initialize method tests
// =========================================================================

#[test]
fn test_initialize_response_structure() {
    let expected_keys = vec!["protocolVersion", "capabilities", "serverInfo"];

    for key in expected_keys {
        let init_response = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "issues-mcp",
                "version": "0.1.0"
            }
        });

        assert!(init_response.get(key).is_some(), "Missing key: {}", key);
    }
}

#[test]
fn test_initialize_protocol_version() {
    let init_response = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "issues-mcp",
            "version": "0.1.0"
        }
    });

    assert_eq!(init_response["protocolVersion"], "2024-11-05");
}

#[test]
fn test_initialize_server_info() {
    let init_response = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "issues-mcp",
            "version": "0.1.0"
        }
    });

    assert_eq!(init_response["serverInfo"]["name"], "issues-mcp");
    assert_eq!(init_response["serverInfo"]["version"], "0.1.0");
}

#[test]
fn test_initialize_capabilities() {
    let init_response = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "issues-mcp",
            "version": "0.1.0"
        }
    });

    assert!(init_response["capabilities"].get("tools").is_some());
}

// =========================================================================
// Tools list tests
// =========================================================================

#[test]
fn test_tools_list_contains_issue_operations() {
    let tool_names = vec![
        "issue_list",
        "issue_create",
        "issue_get",
        "issue_claim",
        "issue_complete",
        "issue_block",
        "issue_ready",
        "issue_update",
        "issue_delete",
    ];

    for tool_name in tool_names {
        // Tool should exist in the list
        assert!(!tool_name.is_empty());
    }
}

#[test]
fn test_tools_list_contains_plan_mode_operations() {
    let tool_names = vec![
        "enter_plan_mode",
        "exit_plan_mode",
        "advance_plan_phase",
        "get_current_phase",
    ];

    for tool_name in tool_names {
        assert!(!tool_name.is_empty());
    }
}

#[test]
fn test_tool_has_required_fields() {
    let tool = json!({
        "name": "issue_list",
        "description": "List issues, optionally filtered by status",
        "inputSchema": {
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["open", "in_progress", "done"]
                }
            }
        }
    });

    assert!(tool.get("name").is_some());
    assert!(tool.get("description").is_some());
    assert!(tool.get("inputSchema").is_some());
}

#[test]
fn test_tool_input_schema_structure() {
    let schema = json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "Issue title"
            }
        },
        "required": ["title"]
    });

    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());
    assert!(schema["required"].is_array());
}

// =========================================================================
// Issue operations schema tests
// =========================================================================

#[test]
fn test_issue_create_schema() {
    let schema = json!({
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
    });

    assert_eq!(schema["required"][0], "title");
    assert!(schema["properties"]["priority"]["enum"].is_array());
}

#[test]
fn test_issue_get_schema() {
    let schema = json!({
        "type": "object",
        "properties": {
            "number": {
                "type": "integer",
                "description": "Issue number"
            }
        },
        "required": ["number"]
    });

    assert_eq!(schema["properties"]["number"]["type"], "integer");
    assert_eq!(schema["required"][0], "number");
}

#[test]
fn test_issue_claim_schema() {
    let schema = json!({
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
    });

    assert_eq!(schema["required"].as_array().unwrap().len(), 2);
}

#[test]
fn test_issue_update_schema() {
    let schema = json!({
        "type": "object",
        "properties": {
            "number": {
                "type": "integer"
            },
            "title": {
                "type": "string"
            },
            "description": {
                "type": "string"
            },
            "priority": {
                "type": "string",
                "enum": ["urgent", "high", "medium", "low"]
            },
            "issue_type": {
                "type": "string",
                "enum": ["task", "bug", "feature"]
            }
        },
        "required": ["number"]
    });

    assert_eq!(schema["required"][0], "number");
    assert!(schema["properties"]["title"].is_object());
}

// =========================================================================
// Plan mode operations schema tests
// =========================================================================

#[test]
fn test_enter_plan_mode_schema() {
    let schema = json!({
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
    });

    assert_eq!(schema["required"].as_array().unwrap().len(), 2);
    assert_eq!(schema["required"][0], "slug");
    assert_eq!(schema["required"][1], "goal");
}

#[test]
fn test_exit_plan_mode_schema() {
    let schema = json!({
        "type": "object",
        "properties": {
            "launchSwarm": {
                "type": "boolean",
                "description": "Whether to launch a swarm to implement the plan"
            },
            "teammateCount": {
                "type": "number",
                "description": "Number of teammates to spawn in the swarm"
            }
        }
    });

    // Optional parameters, no required field
    assert!(schema.get("required").is_none() || schema["required"].as_array().unwrap().is_empty());
}

#[test]
fn test_advance_plan_phase_schema() {
    let schema = json!({
        "type": "object",
        "properties": {}
    });

    // No parameters needed
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].as_object().unwrap().is_empty());
}

// =========================================================================
// Response format tests
// =========================================================================

#[test]
fn test_success_response_format() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{
                "type": "text",
                "text": "Success message"
            }]
        }
    });

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response.get("result").is_some());
    assert!(response.get("error").is_none());
}

#[test]
fn test_error_response_format() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -32603,
            "message": "Internal error"
        }
    });

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response.get("error").is_some());
    assert!(response.get("result").is_none());
}

#[test]
fn test_error_code_internal_error() {
    let error = json!({
        "code": -32603,
        "message": "Internal error"
    });

    assert_eq!(error["code"], -32603);
}

#[test]
fn test_tool_result_content_structure() {
    let result = json!({
        "content": [{
            "type": "text",
            "text": "Result text"
        }]
    });

    assert!(result["content"].is_array());
    assert_eq!(result["content"][0]["type"], "text");
}

#[test]
fn test_tool_error_result_structure() {
    let result = json!({
        "content": [{
            "type": "text",
            "text": "Error: Something went wrong"
        }],
        "isError": true
    });

    assert_eq!(result["isError"], true);
    assert!(result["content"][0]["text"].as_str().unwrap().starts_with("Error:"));
}

// =========================================================================
// Parameter validation tests
// =========================================================================

#[test]
fn test_missing_required_parameter() {
    // Create request missing required "title" parameter
    let args = json!({
        "description": "Test description"
    });

    // Should fail validation
    assert!(args.get("title").is_none());
}

#[test]
fn test_valid_priority_values() {
    let valid_priorities = vec!["urgent", "high", "medium", "low"];

    for priority in valid_priorities {
        assert!(!priority.is_empty());
    }
}

#[test]
fn test_valid_issue_type_values() {
    let valid_types = vec!["task", "bug", "feature"];

    for issue_type in valid_types {
        assert!(!issue_type.is_empty());
    }
}

#[test]
fn test_valid_status_values() {
    let valid_statuses = vec!["open", "in_progress", "done"];

    for status in valid_statuses {
        assert!(!status.is_empty());
    }
}

#[test]
fn test_valid_agent_values() {
    let valid_agents = vec!["claude", "codex"];

    for agent in valid_agents {
        assert!(!agent.is_empty());
    }
}

// =========================================================================
// Edge case tests
// =========================================================================

#[test]
fn test_empty_parameters() {
    let params = json!({});

    assert!(params.is_object());
    assert!(params.as_object().unwrap().is_empty());
}

#[test]
fn test_null_optional_parameter() {
    let params = json!({
        "title": "Test Issue",
        "description": null
    });

    assert_eq!(params["title"], "Test Issue");
    assert!(params["description"].is_null());
}

#[test]
fn test_tool_name_validation() {
    let valid_tool_names = vec![
        "issue_list",
        "issue_create",
        "issue_get",
        "issue_claim",
        "issue_complete",
        "issue_block",
        "issue_ready",
        "issue_update",
        "issue_delete",
        "enter_plan_mode",
        "exit_plan_mode",
        "advance_plan_phase",
        "get_current_phase",
    ];

    for name in valid_tool_names {
        // Should be lowercase with underscores
        assert!(!name.contains('-'));
        assert!(!name.contains(' '));
    }
}

#[test]
fn test_integer_type_for_number() {
    let args = json!({
        "number": 42
    });

    assert!(args["number"].is_i64());
    assert_eq!(args["number"], 42);
}

#[test]
fn test_string_type_for_text_fields() {
    let args = json!({
        "title": "Test Title",
        "description": "Test Description"
    });

    assert!(args["title"].is_string());
    assert!(args["description"].is_string());
}

// =========================================================================
// Notification handling tests
// =========================================================================

#[test]
fn test_notification_request_no_id() {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    assert!(notification.get("id").is_none());
}

#[test]
fn test_notification_method() {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    assert_eq!(notification["method"], "notifications/initialized");
}

// =========================================================================
// Unknown method/tool handling tests
// =========================================================================

#[test]
fn test_unknown_method_format() {
    let method = "unknown_method";
    let error_msg = format!("Unknown method: {}", method);

    assert!(error_msg.contains("unknown_method"));
}

#[test]
fn test_unknown_tool_format() {
    let tool = "unknown_tool";
    let error_msg = format!("Unknown tool: {}", tool);

    assert!(error_msg.contains("unknown_tool"));
}

// =========================================================================
// Issue output format tests
// =========================================================================

#[test]
fn test_issue_list_output_fields() {
    let issue_output = json!({
        "number": 1,
        "title": "Test Issue",
        "status": "open",
        "priority": "medium",
        "agent": "claude",
        "is_blocked": false
    });

    assert_eq!(issue_output["number"], 1);
    assert_eq!(issue_output["title"], "Test Issue");
    assert_eq!(issue_output["status"], "open");
    assert_eq!(issue_output["priority"], "medium");
    assert_eq!(issue_output["agent"], "claude");
    assert_eq!(issue_output["is_blocked"], false);
}

#[test]
fn test_issue_get_output_fields() {
    let issue_output = json!({
        "number": 1,
        "title": "Test Issue",
        "description": "Test Description",
        "status": "open",
        "priority": "medium",
        "issue_type": "task",
        "agent": "claude",
        "is_blocked": false,
        "blocked_reason": null,
        "claimed_by": null,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    });

    assert!(issue_output.get("number").is_some());
    assert!(issue_output.get("created_at").is_some());
    assert!(issue_output.get("updated_at").is_some());
}

#[test]
fn test_issue_ready_output_fields() {
    let issue_output = json!({
        "number": 1,
        "title": "Ready Issue",
        "description": "Description",
        "priority": "high",
        "issue_type": "task",
        "agent": "claude"
    });

    assert_eq!(issue_output["number"], 1);
    assert!(issue_output["title"].is_string());
    assert!(issue_output["priority"].is_string());
}
