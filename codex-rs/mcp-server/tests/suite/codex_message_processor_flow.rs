use std::path::Path;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_core::protocol_config_types::ReasoningSummary;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::mcp_protocol::AddConversationListenerParams;
use codex_protocol::mcp_protocol::AddConversationSubscriptionResponse;
use codex_protocol::mcp_protocol::EXEC_COMMAND_APPROVAL_METHOD;
use codex_protocol::mcp_protocol::NewConversationParams;
use codex_protocol::mcp_protocol::NewConversationResponse;
use codex_protocol::mcp_protocol::RemoveConversationListenerParams;
use codex_protocol::mcp_protocol::RemoveConversationSubscriptionResponse;
use codex_protocol::mcp_protocol::SendUserMessageParams;
use codex_protocol::mcp_protocol::SendUserMessageResponse;
use codex_protocol::mcp_protocol::SendUserTurnParams;
use codex_protocol::mcp_protocol::SendUserTurnResponse;
use mcp_test_support::McpProcess;
use mcp_test_support::create_final_assistant_message_sse_response;
use mcp_test_support::create_mock_chat_completions_server;
use mcp_test_support::create_shell_sse_response;
use mcp_test_support::to_response;
use mcp_types::JSONRPCNotification;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
use std::env;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_codex_jsonrpc_conversation_flow() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let tmp = TempDir::new().expect("tmp dir");
    // Temporary Codex home with config pointing at the mock server.
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home).expect("create codex home dir");
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory).expect("create working directory");

    // Create a mock model server that immediately ends each turn.
    // Two turns are expected: initial session configure + one user message.
    let responses = vec![
        create_shell_sse_response(
            vec!["ls".to_string()],
            Some(&working_directory),
            Some(5000),
            "call1234",
        )
        .expect("create shell sse response"),
        create_final_assistant_message_sse_response("Enjoy your new git repo!")
            .expect("create final assistant message"),
    ];
    let server = create_mock_chat_completions_server(responses).await;
    create_config_toml(&codex_home, &server.uri()).expect("write config");

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(&codex_home).await.expect("spawn mcp");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init error");

    // 1) newConversation
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            cwd: Some(working_directory.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await
        .expect("send newConversation");
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await
    .expect("newConversation timeout")
    .expect("newConversation resp");
    let new_conv_resp = to_response::<NewConversationResponse>(new_conv_resp)
        .expect("deserialize newConversation response");
    let NewConversationResponse {
        conversation_id,
        model,
        reasoning_effort: _,
        rollout_path: _,
    } = new_conv_resp;
    assert_eq!(model, "mock-model");

    // 2) addConversationListener
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams { conversation_id })
        .await
        .expect("send addConversationListener");
    let add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await
    .expect("addConversationListener timeout")
    .expect("addConversationListener resp");
    let AddConversationSubscriptionResponse { subscription_id } =
        to_response::<AddConversationSubscriptionResponse>(add_listener_resp)
            .expect("deserialize addConversationListener response");

    // 3) sendUserMessage (should trigger notifications; we only validate an OK response)
    let send_user_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![codex_protocol::mcp_protocol::InputItem::Text {
                text: "text".to_string(),
            }],
        })
        .await
        .expect("send sendUserMessage");
    let send_user_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_user_id)),
    )
    .await
    .expect("sendUserMessage timeout")
    .expect("sendUserMessage resp");
    let SendUserMessageResponse {} = to_response::<SendUserMessageResponse>(send_user_resp)
        .expect("deserialize sendUserMessage response");

    // Verify the task_finished notification is received.
    // Note this also ensures that the final request to the server was made.
    let task_finished_notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await
    .expect("task_finished_notification timeout")
    .expect("task_finished_notification resp");
    let serde_json::Value::Object(map) = task_finished_notification
        .params
        .expect("notification should have params")
    else {
        panic!("task_finished_notification should have params");
    };
    assert_eq!(
        map.get("conversationId")
            .expect("should have conversationId"),
        &serde_json::Value::String(conversation_id.to_string())
    );

    // 4) removeConversationListener
    let remove_listener_id = mcp
        .send_remove_conversation_listener_request(RemoveConversationListenerParams {
            subscription_id,
        })
        .await
        .expect("send removeConversationListener");
    let remove_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(remove_listener_id)),
    )
    .await
    .expect("removeConversationListener timeout")
    .expect("removeConversationListener resp");
    let RemoveConversationSubscriptionResponse {} =
        to_response(remove_listener_resp).expect("deserialize removeConversationListener response");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_send_user_turn_changes_approval_policy_behavior() {
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let tmp = TempDir::new().expect("tmp dir");
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home).expect("create codex home dir");
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory).expect("create working directory");

    // Mock server will request a python shell call for the first and second turn, then finish.
    let responses = vec![
        create_shell_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            Some(&working_directory),
            Some(5000),
            "call1",
        )
        .expect("create first shell sse response"),
        create_final_assistant_message_sse_response("done 1")
            .expect("create final assistant message 1"),
        create_shell_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            Some(&working_directory),
            Some(5000),
            "call2",
        )
        .expect("create second shell sse response"),
        create_final_assistant_message_sse_response("done 2")
            .expect("create final assistant message 2"),
    ];
    let server = create_mock_chat_completions_server(responses).await;
    create_config_toml(&codex_home, &server.uri()).expect("write config");

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(&codex_home).await.expect("spawn mcp");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init error");

    // 1) Start conversation with approval_policy=untrusted
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            cwd: Some(working_directory.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await
        .expect("send newConversation");
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await
    .expect("newConversation timeout")
    .expect("newConversation resp");
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<NewConversationResponse>(new_conv_resp)
        .expect("deserialize newConversation response");

    // 2) addConversationListener
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams { conversation_id })
        .await
        .expect("send addConversationListener");
    let _: AddConversationSubscriptionResponse =
        to_response::<AddConversationSubscriptionResponse>(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
            )
            .await
            .expect("addConversationListener timeout")
            .expect("addConversationListener resp"),
        )
        .expect("deserialize addConversationListener response");

    // 3) sendUserMessage triggers a shell call; approval policy is Untrusted so we should get an elicitation
    let send_user_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![codex_protocol::mcp_protocol::InputItem::Text {
                text: "run python".to_string(),
            }],
        })
        .await
        .expect("send sendUserMessage");
    let _send_user_resp: SendUserMessageResponse = to_response::<SendUserMessageResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_user_id)),
        )
        .await
        .expect("sendUserMessage timeout")
        .expect("sendUserMessage resp"),
    )
    .expect("deserialize sendUserMessage response");

    // Expect an ExecCommandApproval request (elicitation)
    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await
    .expect("waiting for exec approval request timeout")
    .expect("exec approval request");
    assert_eq!(request.method, EXEC_COMMAND_APPROVAL_METHOD);

    // Approve so the first turn can complete
    mcp.send_response(
        request.id,
        serde_json::json!({ "decision": codex_core::protocol::ReviewDecision::Approved }),
    )
    .await
    .expect("send approval response");

    // Wait for first TaskComplete
    let _ = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await
    .expect("task_complete 1 timeout")
    .expect("task_complete 1 notification");

    // 4) sendUserTurn with approval_policy=never should run without elicitation
    let send_turn_id = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![codex_protocol::mcp_protocol::InputItem::Text {
                text: "run python again".to_string(),
            }],
            cwd: working_directory.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "mock-model".to_string(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
        })
        .await
        .expect("send sendUserTurn");
    // Acknowledge sendUserTurn
    let _send_turn_resp: SendUserTurnResponse = to_response::<SendUserTurnResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_turn_id)),
        )
        .await
        .expect("sendUserTurn timeout")
        .expect("sendUserTurn resp"),
    )
    .expect("deserialize sendUserTurn response");

    // Ensure we do NOT receive an ExecCommandApproval request before the task completes.
    // If any Request is seen while waiting for task_complete, the helper will error and the test fails.
    let _ = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await
    .expect("task_complete 2 timeout")
    .expect("task_complete 2 notification");
}

// Helper: minimal config.toml pointing at mock provider.
fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
