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
                assert!(success.api_error_status.is_none());
                assert!(success.terminal_reason.is_none());
                assert!(
                    success.model_usage["claude-sonnet-4-5-20250929"]
                        .max_output_tokens
                        .is_none()
                );
            }
            _ => panic!("Expected result success message"),
        }
    }

    #[test]
    fn test_parse_result_success_fidelity_fields() {
        let json = json!({
            "type": "result",
            "subtype": "success",
            "duration_ms": 1000,
            "duration_api_ms": 800,
            "is_error": true,
            "api_error_status": 429,
            "num_turns": 1,
            "result": "rate limited",
            "total_cost_usd": 0.01,
            "usage": { "input_tokens": 10, "output_tokens": 4 },
            "modelUsage": {
                "claude-sonnet-4-5-20250929": {
                    "inputTokens": 10,
                    "outputTokens": 4,
                    "cacheReadInputTokens": 0,
                    "cacheCreationInputTokens": 0,
                    "webSearchRequests": 0,
                    "costUSD": 0.01,
                    "contextWindow": 200000,
                    "maxOutputTokens": 16384
                }
            },
            "permission_denials": [],
            "terminal_reason": "completed",
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::Result(SdkResultMessage::Success(success)) => {
                assert_eq!(success.api_error_status, Some(429));
                assert_eq!(success.terminal_reason, Some(TerminalReason::Completed));
                assert_eq!(
                    success.model_usage["claude-sonnet-4-5-20250929"].max_output_tokens,
                    Some(16384)
                );
            }
            other => panic!("expected result success, got {other:?}"),
        }
    }

    #[test]
    fn test_unknown_terminal_reason_keeps_typed_result() {
        let json = json!({
            "type": "result",
            "subtype": "success",
            "duration_ms": 1,
            "duration_api_ms": 1,
            "is_error": false,
            "num_turns": 1,
            "result": "ok",
            "total_cost_usd": 0.0,
            "usage": { "input_tokens": 1, "output_tokens": 1 },
            "modelUsage": {},
            "permission_denials": [],
            "terminal_reason": "api_error",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(json).unwrap() {
            SdkMessage::Result(SdkResultMessage::Success(success)) => {
                assert_eq!(success.terminal_reason, Some(TerminalReason::Unknown));
            }
            other => panic!("later terminal_reason must not become SdkMessage::Unknown: {other:?}"),
        }
    }

    #[test]
    fn test_parse_result_error_terminal_reason() {
        let json = json!({
            "type": "result",
            "subtype": "error_max_budget_usd",
            "duration_ms": 10,
            "duration_api_ms": 8,
            "is_error": true,
            "num_turns": 2,
            "total_cost_usd": 1.5,
            "usage": { "input_tokens": 1, "output_tokens": 1 },
            "modelUsage": {},
            "permission_denials": [],
            "errors": ["budget"],
            "terminal_reason": "blocking_limit",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(json).unwrap() {
            SdkMessage::Result(SdkResultMessage::ErrorMaxBudget(err)) => {
                assert_eq!(err.terminal_reason, Some(TerminalReason::BlockingLimit));
            }
            other => panic!("expected error_max_budget_usd, got {other:?}"),
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
    fn test_serialize_user_message() {
        let msg = SdkUserMessage {
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
            PermissionMode::Auto,
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
    fn test_parse_api_retry_system_subtype() {
        let json = json!({
            "type": "system",
            "subtype": "api_retry",
            "attempt": 2,
            "max_retries": 5,
            "retry_delay_ms": 400,
            "error_status": 429,
            "error": "rate_limit",
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::System(SdkSystemMessage::ApiRetry(retry)) => {
                assert_eq!(retry.attempt, 2);
                assert_eq!(retry.max_retries, 5);
                assert_eq!(retry.retry_delay_ms, 400);
                assert_eq!(retry.error_status, Some(429));
            }
            other => panic!("expected api_retry, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_rate_limit_event() {
        let json = json!({
            "type": "rate_limit_event",
            "rate_limit_info": {
                "status": "allowed_warning",
                "resetsAt": 1_700_000_000,
                "rateLimitType": "five_hour",
                "utilization": 0.82
            },
            "uuid": "12345678-1234-1234-1234-123456789012",
            "session_id": "session-123"
        });

        let msg: SdkMessage = serde_json::from_value(json).unwrap();
        match msg {
            SdkMessage::RateLimitEvent(event) => {
                assert_eq!(event.rate_limit_info.status, "allowed_warning");
                assert_eq!(event.rate_limit_info.resets_at, Some(1_700_000_000));
            }
            other => panic!("expected rate_limit_event, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_prompt_suggestion_and_tool_use_summary() {
        let suggestion = json!({
            "type": "prompt_suggestion",
            "suggestion": "run the tests",
            "uuid": "u1",
            "session_id": "s1"
        });
        match serde_json::from_value::<SdkMessage>(suggestion).unwrap() {
            SdkMessage::PromptSuggestion(msg) => assert_eq!(msg.suggestion, "run the tests"),
            other => panic!("expected prompt_suggestion, got {other:?}"),
        }

        let summary = json!({
            "type": "tool_use_summary",
            "summary": "edited two files",
            "preceding_tool_use_ids": ["tool-1", "tool-2"],
            "uuid": "u2",
            "session_id": "s1"
        });
        match serde_json::from_value::<SdkMessage>(summary).unwrap() {
            SdkMessage::ToolUseSummary(msg) => {
                assert_eq!(msg.preceding_tool_use_ids.len(), 2);
            }
            other => panic!("expected tool_use_summary, got {other:?}"),
        }
    }

    #[test]
    fn test_unknown_type_is_sdk_unknown_and_round_trips() {
        let json = json!({
            "type": "future_widget",
            "foo": 1,
            "nested": { "ok": true }
        });

        let msg: SdkMessage = serde_json::from_value(json.clone()).unwrap();
        match &msg {
            SdkMessage::Unknown { type_name, raw } => {
                assert_eq!(type_name, "future_widget");
                assert_eq!(raw["foo"], 1);
                assert_eq!(raw["nested"]["ok"], true);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }

        let back = serde_json::to_value(&msg).unwrap();
        assert_eq!(back["type"], "future_widget");
        assert_eq!(back["foo"], 1);

        let stdout: StdoutMessage = serde_json::from_value(json).unwrap();
        match stdout {
            StdoutMessage::Message(SdkMessage::Unknown { type_name, .. }) => {
                assert_eq!(type_name, "future_widget");
            }
            other => panic!("control frames must not swallow unknown SDK types: {other:?}"),
        }
    }

    #[test]
    fn test_unknown_system_subtype_is_sdk_unknown() {
        let json = json!({
            "type": "system",
            "subtype": "not_a_real_subtype",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(json).unwrap() {
            SdkMessage::Unknown { type_name, raw } => {
                assert_eq!(type_name, "system");
                assert_eq!(raw["subtype"], "not_a_real_subtype");
            }
            other => panic!("expected Unknown system subtype, got {other:?}"),
        }
    }

    #[test]
    fn test_invalid_jsonl_is_unrecognized_message() {
        let err = parse_stdout_line("not-json {").unwrap_err();
        match err {
            crate::Error::UnrecognizedMessage { type_name, raw } => {
                assert!(type_name.is_none());
                assert!(raw.contains("not-json"));
            }
            other => panic!("expected UnrecognizedMessage, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_stdout_line_known_new_variant_and_unknown_type() {
        let retry = parse_stdout_line(
            r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":3,"retry_delay_ms":200,"error_status":null,"error":"overloaded","uuid":"u","session_id":"s"}"#,
        )
        .unwrap();
        match retry {
            StdoutMessage::Message(SdkMessage::System(SdkSystemMessage::ApiRetry(msg))) => {
                assert_eq!(msg.attempt, 1);
                assert!(matches!(msg.error, AssistantMessageError::Overloaded));
            }
            other => panic!("expected api_retry, got {other:?}"),
        }

        let unknown = parse_stdout_line(r#"{"type":"not_a_real_type","x":true}"#).unwrap();
        match unknown {
            StdoutMessage::Message(SdkMessage::Unknown { type_name, raw }) => {
                assert_eq!(type_name, "not_a_real_type");
                assert_eq!(raw["x"], true);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn test_control_and_keepalive_not_swallowed_as_unknown() {
        let control = parse_stdout_line(
            r#"{"type":"control_request","request_id":"req-1","request":{"subtype":"interrupt"}}"#,
        )
        .unwrap();
        assert!(matches!(control, StdoutMessage::ControlRequest(_)));

        let keepalive = parse_stdout_line(r#"{"type":"keep_alive"}"#).unwrap();
        assert!(matches!(keepalive, StdoutMessage::KeepAlive(_)));
    }

    #[test]
    fn test_hook_callback_stub_continues_without_running() {
        let json = json!({
            "type": "control_request",
            "request_id": "cli-hook-1",
            "request": {
                "subtype": "hook_callback",
                "callback_id": "cb-pre-1",
                "tool_use_id": "tu-1",
                "input": {
                    "hook_event_name": "PreToolUse",
                    "tool_name": "Bash",
                    "tool_input": { "command": "ls" },
                    "tool_use_id": "tu-1"
                }
            }
        });

        let stdout: StdoutMessage = serde_json::from_value(json).unwrap();
        let request = match stdout {
            StdoutMessage::ControlRequest(req) => req,
            other => panic!("expected control_request, got {other:?}"),
        };
        let hook_req = match request.request {
            ControlRequestData::HookCallback(hook_req) => hook_req,
            other => panic!("expected hook_callback, got {other:?}"),
        };
        assert_eq!(hook_req.callback_id, "cb-pre-1");
        assert_eq!(hook_req.tool_use_id.as_deref(), Some("tu-1"));

        let stub = HookCallbackStub::from_request(&hook_req);
        assert_eq!(stub.callback_id, "cb-pre-1");
        assert_eq!(stub.hook_event_name.as_deref(), Some("PreToolUse"));
        assert_eq!(stub.tool_use_id.as_deref(), Some("tu-1"));
        assert!(!stub.hook_ran, "stub must not claim a hook ran");
        assert_eq!(stub.output, SyncHookJSONOutput::continue_without_running());

        let value = stub.response_value();
        assert_eq!(value, json!({ "continue": true }));
        assert!(value.get("hookSpecificOutput").is_none());
        assert!(value.get("decision").is_none());
        assert!(value.get("permissionDecision").is_none());
    }

    #[test]
    fn test_hook_started_progress_and_response() {
        let started = json!({
            "type": "system",
            "subtype": "hook_started",
            "hook_id": "h1",
            "hook_name": "PreToolUse",
            "hook_event": "PreToolUse",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(started).unwrap() {
            SdkMessage::System(SdkSystemMessage::HookStarted(msg)) => {
                assert_eq!(msg.hook_id, "h1");
            }
            other => panic!("expected hook_started, got {other:?}"),
        }

        let response = json!({
            "type": "system",
            "subtype": "hook_response",
            "hook_id": "h1",
            "hook_name": "PreToolUse",
            "hook_event": "PreToolUse",
            "output": "ok",
            "stdout": "ok",
            "stderr": "",
            "exit_code": 0,
            "outcome": "success",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(response).unwrap() {
            SdkMessage::System(SdkSystemMessage::HookResponse(msg)) => {
                assert_eq!(msg.hook_id.as_deref(), Some("h1"));
                assert_eq!(msg.outcome.as_deref(), Some("success"));
            }
            other => panic!("expected hook_response, got {other:?}"),
        }
    }
}
