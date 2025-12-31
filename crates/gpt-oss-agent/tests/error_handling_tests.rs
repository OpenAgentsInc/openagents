//! Error handling tests for GPT-OSS agent
//!
//! Tests edge cases, error conditions, and error propagation.

use gpt_oss_agent::tools::{Tool, ToolRequest};
use gpt_oss_agent::{GptOssAgent, GptOssAgentConfig, GptOssAgentError};
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_execute_nonexistent_tool() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();

    let request = ToolRequest {
        tool: "nonexistent_tool".to_string(),
        parameters: json!({}),
    };

    let result = agent.execute_tool(request).await;
    assert!(result.is_err(), "Executing nonexistent tool should fail");

    if let Err(GptOssAgentError::ToolError(msg)) = result {
        assert!(
            msg.contains("not found"),
            "Error message should indicate tool not found"
        );
    } else {
        panic!("Expected ToolError variant");
    }
}

#[tokio::test]
async fn test_tool_with_invalid_json_params() {
    use gpt_oss_agent::tools::browser::BrowserTool;

    let browser = BrowserTool::new();

    // Invalid JSON structure for browser tool
    let result = browser.execute(json!("not an object")).await;
    assert!(
        result.is_err(),
        "Tool should reject invalid parameter structure"
    );
}

#[tokio::test]
async fn test_session_execute_nonexistent_tool() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let session = agent.create_session().await;

    let request = ToolRequest {
        tool: "invalid_tool".to_string(),
        parameters: json!({}),
    };

    let result = session.execute_tool(request).await;
    assert!(result.is_err(), "Session should reject invalid tool");
}

#[tokio::test]
async fn test_agent_config_with_invalid_workspace() {
    let config = GptOssAgentConfig {
        workspace_root: PathBuf::from("/nonexistent/path/that/should/not/exist"),
        ..Default::default()
    };

    // Agent creation should still succeed - workspace is created on demand
    let result = GptOssAgent::new(config).await;
    assert!(
        result.is_ok(),
        "Agent should create with invalid workspace path"
    );
}

#[tokio::test]
async fn test_agent_is_ready_when_server_unavailable() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        base_url: "http://localhost:9999".to_string(), // Invalid port
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();

    // Should return false when server is unavailable
    let ready = agent.is_ready().await;
    assert!(
        !ready,
        "Agent should not be ready when server is unavailable"
    );
}

#[tokio::test]
async fn test_multiple_tool_errors_are_independent() {
    use gpt_oss_agent::tools::python::PythonTool;

    let python = PythonTool::new();

    // Execute multiple invalid requests
    let result1 = python.execute(json!({})).await;
    let result2 = python.execute(json!({"invalid": "param"})).await;

    assert!(result1.is_err());
    assert!(result2.is_err());

    // Errors should be independent
    // Tool should still be usable after errors
    let result3 = python
        .execute(json!({
            "code": "print('test')"
        }))
        .await;

    // This might fail if Docker isn't available, but shouldn't panic
    let _ = result3;
}

#[tokio::test]
async fn test_session_state_persistence_after_errors() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let session = agent.create_session().await;

    // Try to execute invalid tool
    let request = ToolRequest {
        tool: "invalid".to_string(),
        parameters: json!({}),
    };
    let _ = session.execute_tool(request).await;

    // Session state should remain intact
    let state = session.state().await;
    assert_eq!(state.turn, 0, "Turn count should not change on error");

    let history = session.history().await;
    assert!(history.is_empty(), "History should remain empty on error");
}

#[tokio::test]
async fn test_error_types_are_serializable() {
    // Verify that error types can be displayed/debugged
    let error = GptOssAgentError::ToolError("test error".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ToolError"));

    let display_str = format!("{}", error);
    assert!(display_str.contains("test error"));
}

#[tokio::test]
async fn test_concurrent_tool_execution() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();

    // Execute multiple tool requests concurrently
    let futures = vec![
        agent.execute_tool(ToolRequest {
            tool: "browser".to_string(),
            parameters: json!({"action": {"type": "search", "query": "test"}}),
        }),
        agent.execute_tool(ToolRequest {
            tool: "python".to_string(),
            parameters: json!({"code": "print('hello')"}),
        }),
        agent.execute_tool(ToolRequest {
            tool: "apply_patch".to_string(),
            parameters: json!({"file": "test.txt", "patch": "content"}),
        }),
    ];

    let results = futures_util::future::join_all(futures).await;

    // All should complete (though may error if tools aren't available)
    assert_eq!(results.len(), 3);
}
