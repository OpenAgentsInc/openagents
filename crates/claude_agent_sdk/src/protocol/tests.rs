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
            "terminal_reason": "some_future_reason",
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
        let response = SdkControlResponse::success(
            "req-123",
            Some(
                serde_json::to_value(PermissionResult::allow(json!({
                    "command": "ls -la"
                })))
                .unwrap(),
            ),
        );

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["type"], "control_response");
        assert_eq!(json["response"]["subtype"], "success");
        assert_eq!(json["response"]["request_id"], "req-123");
    }

    #[test]
    fn test_serialize_control_response_deny() {
        let response = SdkControlResponse::success(
            "req-123",
            Some(serde_json::to_value(PermissionResult::deny("Not allowed")).unwrap()),
        );

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
            client_composed: Some(true),
            agent_id: None,
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
        assert_eq!(hook_req.hook_event_name(), Some("PreToolUse"));
    }

    #[test]
    fn test_hook_json_output_sync_and_async_forms() {
        let sync = HookJSONOutput::from(SyncHookJSONOutput {
            hook_specific_output: Some(json!({
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow"
            })),
            terminal_sequence: Some("\u{7}".into()),
            ..SyncHookJSONOutput::continue_execution()
        });
        let value = serde_json::to_value(&sync).unwrap();
        assert_eq!(value["continue"], true);
        assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "allow");
        assert!(value.get("terminalSequence").is_some());

        let parsed: HookJSONOutput =
            serde_json::from_value(json!({"async": true, "asyncTimeout": 30})).unwrap();
        assert_eq!(
            parsed,
            HookJSONOutput::Async(AsyncHookJSONOutput {
                is_async: true,
                async_timeout: Some(30),
            })
        );
        let parsed: HookJSONOutput = serde_json::from_value(json!({"decision": "block"})).unwrap();
        assert!(matches!(parsed, HookJSONOutput::Sync(_)));
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

    fn system(subtype: &str, extra: serde_json::Value) -> SdkMessage {
        let mut value =
            json!({"type": "system", "subtype": subtype, "uuid": "u", "session_id": "s"});
        for (key, field) in extra.as_object().unwrap() {
            value[key] = field.clone();
        }
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn test_0_3_289_system_subtypes_parse_to_typed_variants() {
        match system(
            "control_request_progress",
            json!({"request_id": "sdk-1", "status": "api_retry", "attempt": 2, "max_retries": 5, "retry_delay_ms": 400, "error_status": 529}),
        ) {
            SdkMessage::System(SdkSystemMessage::ControlRequestProgress(m)) => {
                assert_eq!(m.request_id, "sdk-1");
                assert_eq!(m.attempt, Some(2));
            }
            other => panic!("expected control_request_progress, got {other:?}"),
        }
        match system(
            "model_refusal_no_fallback",
            json!({"original_model": "opus", "request_id": null, "content": "refused"}),
        ) {
            SdkMessage::System(SdkSystemMessage::ModelRefusalNoFallback(m)) => {
                assert_eq!(m.original_model, "opus");
            }
            other => panic!("expected model_refusal_no_fallback, got {other:?}"),
        }
        match system(
            "background_tasks_changed",
            json!({"tasks": [{"task_id": "t1", "task_type": "local_bash", "description": "build", "ambient": false}]}),
        ) {
            SdkMessage::System(SdkSystemMessage::BackgroundTasksChanged(m)) => {
                assert_eq!(m.tasks[0].task_id, "t1");
            }
            other => panic!("expected background_tasks_changed, got {other:?}"),
        }
        match system("worker_shutting_down", json!({"reason": "deploy"})) {
            SdkMessage::System(SdkSystemMessage::WorkerShuttingDown(m)) => {
                assert_eq!(m.reason, "deploy");
            }
            other => panic!("expected worker_shutting_down, got {other:?}"),
        }
        match system(
            "informational",
            json!({"content": "heads up", "level": "warning", "prevent_continuation": true}),
        ) {
            SdkMessage::System(SdkSystemMessage::Informational(m)) => {
                assert_eq!(m.level, "warning");
                assert_eq!(m.prevent_continuation, Some(true));
            }
            other => panic!("expected informational, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_289_top_level_types_parse_to_typed_variants() {
        let goal = parse_stdout_line(
            r#"{"type":"active_goal","value":{"condition":"tests pass","iterations":3,"set_at":1700000000000,"tokens_at_start":100},"uuid":"u","session_id":"s"}"#,
        )
        .unwrap();
        match goal {
            StdoutMessage::Message(SdkMessage::ActiveGoal(m)) => {
                assert_eq!(m.value.unwrap().condition, "tests pass");
            }
            other => panic!("expected active_goal, got {other:?}"),
        }
        let cleared =
            parse_stdout_line(r#"{"type":"active_goal","value":null,"uuid":"u","session_id":"s"}"#)
                .unwrap();
        assert!(matches!(
            cleared,
            StdoutMessage::Message(SdkMessage::ActiveGoal(ref m)) if m.value.is_none()
        ));

        let reset = parse_stdout_line(
            r#"{"type":"conversation_reset","new_conversation_id":"c2","trigger":"clear","uuid":"u","session_id":"s"}"#,
        )
        .unwrap();
        match reset {
            StdoutMessage::Message(msg @ SdkMessage::ConversationReset(_)) => {
                assert_eq!(msg.type_name(), "conversation_reset");
                let round_trip = serde_json::to_value(&msg).unwrap();
                assert_eq!(round_trip["type"], "conversation_reset");
                assert_eq!(round_trip["new_conversation_id"], "c2");
            }
            other => panic!("expected conversation_reset, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_289_result_fields_and_new_terminal_reasons() {
        let json = json!({
            "type": "result",
            "subtype": "success",
            "duration_ms": 10,
            "duration_api_ms": 8,
            "is_error": false,
            "num_turns": 1,
            "result": "ok",
            "total_cost_usd": 0.001,
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "modelUsage": {"claude-haiku": {
                "inputTokens": 1, "outputTokens": 1, "cacheReadInputTokens": 0,
                "cacheCreationInputTokens": 0, "webSearchRequests": 0, "costUSD": 0.001,
                "contextWindow": 200000, "maxOutputTokens": 64000, "thinkingTokens": 0,
                "canonicalModel": "claude-haiku-4-5", "provider": "firstParty", "costBasis": "list"
            }},
            "permission_denials": [],
            "structured_output": null,
            "terminal_reason": "budget_exhausted",
            "queued_turn_count": 0,
            "result_index": 0,
            "user_message_uuid": "um-1",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(json).unwrap() {
            SdkMessage::Result(SdkResultMessage::Success(r)) => {
                assert_eq!(r.terminal_reason, Some(TerminalReason::BudgetExhausted));
                assert_eq!(r.turn.queued_turn_count, Some(0));
                assert_eq!(r.turn.user_message_uuid.as_deref(), Some("um-1"));
                let usage = &r.model_usage["claude-haiku"];
                assert_eq!(usage.canonical_model.as_deref(), Some("claude-haiku-4-5"));
                assert_eq!(usage.cost_basis.as_deref(), Some("list"));
            }
            other => panic!("expected typed success result, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_289_assistant_error_values() {
        let json = json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": []},
            "parent_tool_use_id": null,
            "error": "cloud_credential_error",
            "timestamp": "2026-10-04T00:00:00Z",
            "uuid": "u",
            "session_id": "s"
        });
        match serde_json::from_value::<SdkMessage>(json).unwrap() {
            SdkMessage::Assistant(m) => {
                assert!(matches!(
                    m.error,
                    Some(AssistantMessageError::CloudCredentialError)
                ));
                assert_eq!(m.timestamp.as_deref(), Some("2026-10-04T00:00:00Z"));
            }
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn test_inbound_control_frames_at_0_3_289() {
        let elicitation = parse_stdout_line(
            r#"{"type":"control_request","request_id":"r1","request":{"subtype":"elicitation","mcp_server_name":"docs","message":"Pick","mode":"url","url":"https://example.com","elicitation_id":"e1"}}"#,
        )
        .unwrap();
        match elicitation {
            StdoutMessage::ControlRequest(SdkControlRequest {
                request: ControlRequestData::Elicitation(e),
                ..
            }) => {
                assert_eq!(e.mode.as_deref(), Some("url"));
                assert_eq!(e.elicitation_id.as_deref(), Some("e1"));
            }
            other => panic!("expected elicitation, got {other:?}"),
        }

        let dialog = parse_stdout_line(
            r#"{"type":"control_request","request_id":"r2","request":{"subtype":"request_user_dialog","dialog_kind":"confirm","payload":{"text":"ok?"},"tool_use_id":"tu"}}"#,
        )
        .unwrap();
        assert!(matches!(
            dialog,
            StdoutMessage::ControlRequest(SdkControlRequest {
                request: ControlRequestData::RequestUserDialog(_),
                ..
            })
        ));

        let cancel =
            parse_stdout_line(r#"{"type":"control_cancel_request","request_id":"r1"}"#).unwrap();
        match cancel {
            StdoutMessage::ControlCancelRequest(c) => assert_eq!(c.request_id, "r1"),
            other => panic!("expected control_cancel_request, got {other:?}"),
        }

        let future = parse_stdout_line(
            r#"{"type":"control_request","request_id":"r3","request":{"subtype":"brand_new","x":1}}"#,
        )
        .unwrap();
        match future {
            StdoutMessage::UnsupportedControlRequest(u) => {
                assert_eq!(u.request_id, "r3");
                assert_eq!(u.subtype, "brand_new");
                assert_eq!(serde_json::to_value(&u).unwrap()["request"]["x"], 1);
            }
            other => panic!("expected unsupported control request, got {other:?}"),
        }

        let permission = parse_stdout_line(
            r#"{"type":"control_request","request_id":"r4","request":{"subtype":"can_use_tool","tool_name":"mcp__docs__search","input":{},"tool_use_id":"tu","decision_reason_type":"classifier","classifier_approvable":true,"mcp_server":{"name":"docs"},"display_name":"Search docs"}}"#,
        )
        .unwrap();
        match permission {
            StdoutMessage::ControlRequest(SdkControlRequest {
                request: ControlRequestData::CanUseTool(c),
                ..
            }) => {
                assert_eq!(c.decision_reason_type.as_deref(), Some("classifier"));
                assert_eq!(c.display_name.as_deref(), Some("Search docs"));
            }
            other => panic!("expected can_use_tool, got {other:?}"),
        }
    }

    #[test]
    fn test_control_response_carries_pending_requests() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"sdk-3","response":{},"pending_permission_requests":[{"type":"control_request","request_id":"p1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{},"tool_use_id":"tu"}}]}}"#;
        match parse_stdout_line(line).unwrap() {
            StdoutMessage::ControlResponse(SdkControlResponse {
                response:
                    ControlResponseData::Success {
                        pending_permission_requests: Some(pending),
                        ..
                    },
                ..
            }) => assert_eq!(pending[0].request_id, "p1"),
            other => panic!("expected success with pending requests, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_292_subagent_messages_carry_agent_id() {
        let assistant: SdkMessage = serde_json::from_value(json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": []},
            "parent_tool_use_id": "tu-1",
            "agent_id": "task-7",
            "subagent_type": "Explore",
            "task_description": "Find the config",
            "uuid": "a1",
            "session_id": "s"
        }))
        .unwrap();
        match assistant {
            SdkMessage::Assistant(a) => {
                assert_eq!(a.agent_id.as_deref(), Some("task-7"));
                assert_eq!(a.subagent_type.as_deref(), Some("Explore"));
                assert_eq!(a.task_description.as_deref(), Some("Find the config"));
            }
            other => panic!("expected assistant, got {other:?}"),
        }
        let user: SdkMessage = serde_json::from_value(json!({
            "type": "user",
            "message": {"role": "user", "content": "hi"},
            "parent_tool_use_id": "tu-1",
            "agent_id": "task-7",
            "session_id": "s"
        }))
        .unwrap();
        match user {
            SdkMessage::User(u) => assert_eq!(u.agent_id.as_deref(), Some("task-7")),
            other => panic!("expected user, got {other:?}"),
        }
    }

    #[test]
    fn test_inbound_user_messages_parse_typed_and_round_trip() {
        // The CLI's user frames (tool results, replays) used to fall to
        // `Unknown` because the enum tag consumed `type`.
        let wire = json!({
            "type": "user",
            "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "tu", "content": "ok"}
            ]},
            "parent_tool_use_id": null,
            "isSynthetic": true,
            "isReplay": true,
            "tool_use_result": {"stdout": "ok"},
            "uuid": "u1",
            "session_id": "s"
        });
        let msg: SdkMessage = serde_json::from_value(wire.clone()).unwrap();
        match &msg {
            SdkMessage::User(u) => {
                assert_eq!(u.is_synthetic, Some(true));
                assert_eq!(u.is_replay, Some(true));
                assert_eq!(u.uuid.as_deref(), Some("u1"));
            }
            other => panic!("expected user, got {other:?}"),
        }
        let text = serde_json::to_string(&msg).unwrap();
        assert_eq!(text.matches("\"type\":\"user\"").count(), 1, "{text}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            wire
        );
    }

    #[test]
    fn test_0_3_292_task_events_carry_run_and_parent_ids() {
        match system(
            "task_started",
            json!({"task_id": "t2", "run_id": "r-01", "description": "d",
                   "parent_task_id": "t1", "spawn_depth": 2}),
        ) {
            SdkMessage::System(SdkSystemMessage::TaskStarted(m)) => {
                assert_eq!(m.run_id.as_deref(), Some("r-01"));
                assert_eq!(m.parent_task_id.as_deref(), Some("t1"));
            }
            other => panic!("expected task_started, got {other:?}"),
        }
        match system(
            "task_updated",
            json!({"task_id": "t2", "run_id": "r-01", "patch": {"status": "running"}}),
        ) {
            SdkMessage::System(SdkSystemMessage::TaskUpdated(m)) => {
                assert_eq!(m.run_id.as_deref(), Some("r-01"))
            }
            other => panic!("expected task_updated, got {other:?}"),
        }
        match system(
            "task_progress",
            json!({"task_id": "t2", "run_id": "r-01", "description": "d",
                   "usage": {"total_tokens": 1, "tool_uses": 0, "duration_ms": 5}}),
        ) {
            SdkMessage::System(SdkSystemMessage::TaskProgress(m)) => {
                assert_eq!(m.run_id.as_deref(), Some("r-01"))
            }
            other => panic!("expected task_progress, got {other:?}"),
        }
        match system(
            "task_notification",
            json!({"task_id": "t2", "run_id": "r-01", "status": "completed",
                   "output_file": "/o", "summary": "done"}),
        ) {
            SdkMessage::System(SdkSystemMessage::TaskNotification(m)) => {
                assert_eq!(m.run_id.as_deref(), Some("r-01"))
            }
            other => panic!("expected task_notification, got {other:?}"),
        }
        match system(
            "background_tasks_changed",
            json!({"tasks": [{"task_id": "t2", "run_id": "r-01", "task_type": "local_agent",
                              "subagent_type": "general-purpose", "description": "d",
                              "parent_task_id": "t1"}]}),
        ) {
            SdkMessage::System(SdkSystemMessage::BackgroundTasksChanged(m)) => {
                let task = &m.tasks[0];
                assert_eq!(task.run_id.as_deref(), Some("r-01"));
                assert_eq!(task.subagent_type.as_deref(), Some("general-purpose"));
                assert_eq!(task.parent_task_id.as_deref(), Some("t1"));
                let wire = serde_json::to_value(task).unwrap();
                assert_eq!(wire["parent_task_id"], "t1");
            }
            other => panic!("expected background_tasks_changed, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_295_rate_limit_overage_fields() {
        let msg: SdkMessage = serde_json::from_value(json!({
            "type": "rate_limit_event",
            "rate_limit_info": {
                "status": "allowed_warning",
                "overageStatus": "allowed",
                "overageResetsAt": 1760000000,
                "overageDisabledReason": "out_of_credits",
                "overageEnabled": true
            },
            "uuid": "u",
            "session_id": "s"
        }))
        .unwrap();
        match msg {
            SdkMessage::RateLimitEvent(e) => {
                let info = &e.rate_limit_info;
                assert_eq!(info.status, "allowed_warning");
                assert_eq!(info.overage_enabled, Some(true));
                assert_eq!(info.overage_status.as_deref(), Some("allowed"));
                assert_eq!(info.overage_resets_at, Some(1760000000));
                assert_eq!(
                    info.overage_disabled_reason.as_deref(),
                    Some("out_of_credits")
                );
                let wire = serde_json::to_value(info).unwrap();
                assert_eq!(wire["overageEnabled"], true);
            }
            other => panic!("expected rate_limit_event, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_295_text_block_citations_survive() {
        let citation = json!({"type": "web_search_result_location", "url": "https://x",
                              "title": "X", "cited_text": "c", "encrypted_index": "e"});
        let assistant: SdkMessage = serde_json::from_value(json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": [
                {"type": "text", "text": "cited", "citations": [citation.clone()]}
            ]},
            "parent_tool_use_id": null,
            "uuid": "a",
            "session_id": "s"
        }))
        .unwrap();
        let wire = serde_json::to_value(&assistant).unwrap();
        assert_eq!(wire["message"]["content"][0]["citations"][0], citation);
        let delta: SdkMessage = serde_json::from_value(json!({
            "type": "stream_event",
            "event": {"type": "content_block_delta", "index": 0,
                      "delta": {"type": "citations_delta", "citation": citation.clone()}},
            "parent_tool_use_id": null,
            "uuid": "e",
            "session_id": "s"
        }))
        .unwrap();
        match delta {
            SdkMessage::StreamEvent(e) => {
                assert_eq!(e.event["delta"]["citation"], citation)
            }
            other => panic!("expected stream_event, got {other:?}"),
        }
    }

    #[test]
    fn test_0_3_296_startup_failure_reasons_stay_typed() {
        for reason in ["org_config_required_unavailable", "org_config_refused"] {
            let msg: SdkMessage = serde_json::from_value(json!({
                "type": "result",
                "subtype": "error_during_execution",
                "duration_ms": 1, "duration_api_ms": 0, "is_error": true,
                "num_turns": 0, "total_cost_usd": 0.0,
                "usage": {"input_tokens": 0, "output_tokens": 0},
                "modelUsage": {}, "permission_denials": [], "errors": ["refused"],
                "startup_failure_reason": reason,
                "uuid": "r", "session_id": "s"
            }))
            .unwrap();
            match msg {
                SdkMessage::Result(SdkResultMessage::ErrorDuringExecution(e)) => {
                    assert_eq!(e.turn.startup_failure_reason.as_deref(), Some(reason))
                }
                other => panic!("expected error result, got {other:?}"),
            }
        }
    }

    fn rules(n: usize) -> Vec<PermissionRule> {
        (0..n)
            .map(|i| PermissionRule {
                tool_name: format!("Tool{i}"),
                rule_content: None,
            })
            .collect()
    }

    #[test]
    fn test_0_3_295_oversized_updated_permissions_become_a_deny() {
        let allow = |updates: Vec<PermissionUpdate>| PermissionResult::Allow {
            updated_input: json!({}),
            updated_permissions: Some(updates),
            tool_use_id: Some("tu".into()),
            decision_classification: None,
        };
        // One update holding 4,095 rules is 4,096 entries: at the limit.
        let at_limit = allow(vec![PermissionUpdate::AddRules {
            rules: rules(MAX_UPDATED_PERMISSIONS - 1),
            behavior: PermissionBehavior::Allow,
            destination: "session".into(),
        }]);
        assert_eq!(
            at_limit.updated_permission_entries(),
            MAX_UPDATED_PERMISSIONS
        );
        assert!(matches!(
            at_limit.within_limits(),
            PermissionResult::Allow { .. }
        ));
        // Rules and directories across updates count together.
        let over = allow(vec![
            PermissionUpdate::AddRules {
                rules: rules(3000),
                behavior: PermissionBehavior::Allow,
                destination: "session".into(),
            },
            PermissionUpdate::AddDirectories {
                directories: (0..1095).map(|i| format!("/d{i}")).collect(),
                destination: "session".into(),
            },
        ]);
        assert_eq!(over.updated_permission_entries(), 4097);
        match over.within_limits() {
            PermissionResult::Deny {
                message,
                tool_use_id,
                ..
            } => {
                assert!(message.contains("4097"), "{message}");
                assert_eq!(tool_use_id.as_deref(), Some("tu"));
            }
            other => panic!("expected deny, got {other:?}"),
        }
        assert_eq!(PermissionResult::deny("no").updated_permission_entries(), 0);
    }
}
