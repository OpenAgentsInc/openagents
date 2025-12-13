//! Tests for protocol message serialization.

#[cfg(test)]
mod tests {
    use crate::protocol::*;
    use serde_json::json;

    #[test]
    fn test_parse_system_init_message() {
        let json = json!({
            "type": "system",
            "subtype": "init",
            "apiKeySource": "user",
            "claude_code_version": "1.0.0",
            "cwd": "/home/user/project",
            "tools": ["Read", "Write", "Bash"],
            "mcp_servers": [{"name": "test", "status": "connected"}],
            "model": "claude-sonnet-4-5-20250929",
            "permissionMode": "default",
            "slash_commands": ["/help", "/clear"],
            "output_style": "minimal",
            "skills": [],
            "plugins": [],
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::System(SdkSystemMessage::Init(init)) => {
                assert_eq!(init.claude_code_version, "1.0.0");
                assert_eq!(init.model, "claude-sonnet-4-5-20250929");
                assert_eq!(init.tools.len(), 3);
            }
            _ => panic!("Expected system init message"),
        }
    }

    #[test]
    fn test_parse_assistant_message() {
        let json = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "Hello!"}]
            },
            "parent_tool_use_id": null,
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::Assistant(assistant) => {
                assert!(assistant.message.is_object());
            }
            _ => panic!("Expected assistant message"),
        }
    }

    #[test]
    fn test_parse_result_success() {
        let json = json!({
            "type": "result",
            "subtype": "success",
            "duration_ms": 1000,
            "duration_api_ms": 800,
            "is_error": false,
            "num_turns": 3,
            "result": "Task completed successfully",
            "total_cost_usd": 0.05,
            "usage": {
                "input_tokens": 100,
                "output_tokens": 200
            },
            "modelUsage": {
                "claude-sonnet-4-5-20250929": {
                    "inputTokens": 100,
                    "outputTokens": 200,
                    "cacheReadInputTokens": 0,
                    "cacheCreationInputTokens": 0,
                    "webSearchRequests": 0,
                    "costUSD": 0.05,
                    "contextWindow": 200000
                }
            },
            "permission_denials": [],
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::Result(SdkResultMessage::Success(success)) => {
                assert_eq!(success.num_turns, 3);
                assert_eq!(success.result, "Task completed successfully");
                assert!((success.total_cost_usd - 0.05).abs() < 0.001);
            }
            _ => panic!("Expected result success message"),
        }
    }

    #[test]
    fn test_parse_control_request_can_use_tool() {
        let json = json!({
            "type": "control_request",
            "request_id": "req-123",
            "request": {
                "subtype": "can_use_tool",
                "tool_name": "Bash",
                "input": {"command": "ls -la"},
                "tool_use_id": "tool-123"
            }
        });

        let req: SdkControlRequest = serde_json::from_value(json).unwrap();
        match req.request {
            ControlRequestData::CanUseTool(tool_req) => {
                assert_eq!(tool_req.tool_name, "Bash");
                assert_eq!(tool_req.tool_use_id, "tool-123");
            }
            _ => panic!("Expected can_use_tool request"),
        }
    }

    #[test]
    fn test_serialize_control_response_allow() {
        let response = SdkControlResponse {
            msg_type: ControlResponseType::ControlResponse,
            response: ControlResponseData::Success {
                request_id: "req-123".to_string(),
                response: Some(
                    serde_json::to_value(PermissionResult::allow(json!({
                        "command": "ls -la"
                    })))
                    .unwrap(),
                ),
            },
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["type"], "control_response");
        assert_eq!(json["response"]["subtype"], "success");
        assert_eq!(json["response"]["request_id"], "req-123");
    }

    #[test]
    fn test_serialize_control_response_deny() {
        let response = SdkControlResponse {
            msg_type: ControlResponseType::ControlResponse,
            response: ControlResponseData::Success {
                request_id: "req-123".to_string(),
                response: Some(
                    serde_json::to_value(PermissionResult::deny("Not allowed")).unwrap(),
                ),
            },
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["type"], "control_response");
        assert_eq!(json["response"]["subtype"], "success");
    }

    #[test]
    fn test_serialize_user_message_outgoing() {
        let msg = SdkUserMessageOutgoing {
            msg_type: UserMessageType::User,
            message: json!({
                "role": "user",
                "content": "Hello, Claude!"
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: None,
            session_id: "session-123".to_string(),
            is_replay: None,
        };

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "user");
        assert_eq!(json["message"]["role"], "user");
        assert_eq!(json["message"]["content"], "Hello, Claude!");
    }

    #[test]
    fn test_parse_stream_event() {
        let json = json!({
            "type": "stream_event",
            "event": {
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "text_delta",
                    "text": "Hello"
                }
            },
            "parent_tool_use_id": null,
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::StreamEvent(event) => {
                assert!(event.event["type"] == "content_block_delta");
            }
            _ => panic!("Expected stream event"),
        }
    }

    #[test]
    fn test_parse_tool_progress() {
        let json = json!({
            "type": "tool_progress",
            "tool_use_id": "tool-123",
            "tool_name": "Bash",
            "parent_tool_use_id": null,
            "elapsed_time_seconds": 5.5,
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::ToolProgress(progress) => {
                assert_eq!(progress.tool_name, "Bash");
                assert!((progress.elapsed_time_seconds - 5.5).abs() < 0.001);
            }
            _ => panic!("Expected tool progress"),
        }
    }

    #[test]
    fn test_permission_mode_serialization() {
        // Test roundtrip
        for mode in [
            PermissionMode::Default,
            PermissionMode::AcceptEdits,
            PermissionMode::BypassPermissions,
            PermissionMode::Plan,
            PermissionMode::DontAsk,
        ] {
            let json = serde_json::to_value(&mode).unwrap();
            let parsed: PermissionMode = serde_json::from_value(json).unwrap();
            assert_eq!(format!("{:?}", mode), format!("{:?}", parsed));
        }
    }

    #[test]
    fn test_stdout_message_parsing() {
        // Test that StdoutMessage can parse different message types

        // SDK message
        let sdk_json = json!({
            "type": "assistant",
            "message": {},
            "parent_tool_use_id": null,
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });
        let _: StdoutMessage = serde_json::from_value(sdk_json).unwrap();

        // Control request
        let control_json = json!({
            "type": "control_request",
            "request_id": "req-123",
            "request": {
                "subtype": "interrupt"
            }
        });
        let _: StdoutMessage = serde_json::from_value(control_json).unwrap();

        // Keep alive
        let keepalive_json = json!({
            "type": "keep_alive"
        });
        let _: StdoutMessage = serde_json::from_value(keepalive_json).unwrap();
    }

    #[test]
    fn test_parse_user_message_with_tool_result() {
        // This is a critical test for the bug fix: user messages containing tool results
        // must parse correctly. Previously, the "type" field was consumed by the enum tag
        // but also expected by SdkUserMessage, causing parse failures.
        let json = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "tool_use_id": "toolu_013t7RgaiZRk2TL5uMVXuvAC",
                    "type": "tool_result",
                    "content": ".\n├── assets\n├── crates\n├── docs\n└── target\n\n4 directories",
                    "is_error": false
                }]
            },
            "parent_tool_use_id": null,
            "session_id": "5a7cf025-e061-47ee-be41-fed629978594",
            "uuid": "fe309e99-d8cd-48cb-bd65-1da67e36ff25",
            "tool_use_result": {
                "stdout": ".\n├── assets\n├── crates\n├── docs\n└── target\n\n4 directories",
                "stderr": "",
                "interrupted": false,
                "isImage": false
            }
        });

        let msg: SdkMessage =
            serde_json::from_value(json).expect("Failed to parse user message with tool result");
        match msg {
            SdkMessage::User(user_msg) => {
                assert_eq!(user_msg.session_id, "5a7cf025-e061-47ee-be41-fed629978594");
                // Check the content contains tool_result
                let content = user_msg.message.get("content").unwrap().as_array().unwrap();
                assert_eq!(content.len(), 1);
                assert_eq!(content[0]["type"], "tool_result");
                assert_eq!(content[0]["tool_use_id"], "toolu_013t7RgaiZRk2TL5uMVXuvAC");
            }
            _ => panic!("Expected user message, got {:?}", msg),
        }
    }

    #[test]
    fn test_parse_user_message_via_stdout_message() {
        // Test that StdoutMessage can also parse user messages correctly
        let json = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "tool_use_id": "tool-123",
                    "type": "tool_result",
                    "content": "Success",
                    "is_error": false
                }]
            },
            "parent_tool_use_id": null,
            "session_id": "session-123",
            "uuid": "uuid-123"
        });

        let stdout_msg: StdoutMessage =
            serde_json::from_value(json).expect("Failed to parse user message via StdoutMessage");
        match stdout_msg {
            StdoutMessage::Message(SdkMessage::User(_)) => {
                // Success!
            }
            _ => panic!(
                "Expected StdoutMessage::Message(SdkMessage::User), got {:?}",
                stdout_msg
            ),
        }
    }

    #[test]
    fn test_parse_stream_event_tool_use_start() {
        // Test parsing stream event for tool_use content_block_start
        let json = json!({
            "type": "stream_event",
            "event": {
                "type": "content_block_start",
                "index": 1,
                "content_block": {
                    "type": "tool_use",
                    "id": "toolu_01ABC123",
                    "name": "Bash",
                    "input": {}
                }
            },
            "parent_tool_use_id": null,
            "uuid": "uuid-123",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::StreamEvent(event) => {
                let content_block = event.event.get("content_block").unwrap();
                assert_eq!(content_block["type"], "tool_use");
                assert_eq!(content_block["id"], "toolu_01ABC123");
                assert_eq!(content_block["name"], "Bash");
            }
            _ => panic!("Expected stream event"),
        }
    }

    #[test]
    fn test_parse_stream_event_input_json_delta() {
        // Test parsing stream event for tool input streaming
        let json = json!({
            "type": "stream_event",
            "event": {
                "type": "content_block_delta",
                "index": 1,
                "delta": {
                    "type": "input_json_delta",
                    "partial_json": "{\"command\": \"ls"
                }
            },
            "parent_tool_use_id": null,
            "uuid": "uuid-123",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::StreamEvent(event) => {
                let delta = event.event.get("delta").unwrap();
                assert_eq!(delta["type"], "input_json_delta");
                assert_eq!(delta["partial_json"], "{\"command\": \"ls");
            }
            _ => panic!("Expected stream event"),
        }
    }

    #[test]
    fn test_parse_auth_status_message() {
        let json = json!({
            "type": "auth_status",
            "isAuthenticating": true,
            "output": ["Authenticating...", "Please wait"],
            "error": null,
            "uuid": "uuid-123",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::AuthStatus(auth) => {
                assert!(auth.is_authenticating);
                assert_eq!(auth.output.len(), 2);
                assert!(auth.error.is_none());
            }
            _ => panic!("Expected auth status message"),
        }
    }

    #[test]
    fn test_parse_hook_response() {
        let json = json!({
            "type": "system",
            "subtype": "hook_response",
            "hook_name": "session-start",
            "hook_event": "start",
            "stdout": "Hook output",
            "stderr": "",
            "exit_code": 0,
            "uuid": "uuid-123",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::System(SdkSystemMessage::HookResponse(hook)) => {
                assert_eq!(hook.hook_name, "session-start");
                assert_eq!(hook.stdout, "Hook output");
                assert_eq!(hook.exit_code, Some(0));
            }
            _ => panic!("Expected hook response message"),
        }
    }
}
