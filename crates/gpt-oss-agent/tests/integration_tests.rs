//! Integration tests for GPT-OSS agent
//!
//! These tests verify the agent workflow end-to-end, including
//! session management, tool execution, and trajectory recording.

use gpt_oss_agent::{GptOssAgent, GptOssAgentConfig};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_agent_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        base_url: "http://localhost:8000".to_string(),
        model: "gpt-oss-20b".to_string(),
        workspace_root: temp_dir.path().to_path_buf(),
        record_trajectory: false,
    };

    let agent = GptOssAgent::new(config).await;
    assert!(agent.is_ok(), "Agent initialization should succeed");
}

#[tokio::test]
async fn test_agent_lists_all_tools() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let tools = agent.list_tools().await;

    assert_eq!(tools.len(), 4, "Should have exactly 4 tools");
    assert!(
        tools.contains(&"browser".to_string()),
        "Should have browser tool"
    );
    assert!(
        tools.contains(&"python".to_string()),
        "Should have python tool"
    );
    assert!(
        tools.contains(&"apply_patch".to_string()),
        "Should have apply_patch tool"
    );
    assert!(
        tools.contains(&"ui_pane".to_string()),
        "Should have ui_pane tool"
    );
}

#[tokio::test]
async fn test_agent_get_tool_schemas() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();

    for tool_name in &["browser", "python", "apply_patch", "ui_pane"] {
        let schema = agent.get_tool_schema(tool_name).await;
        assert!(schema.is_some(), "Tool {} should have a schema", tool_name);
        assert!(
            schema.unwrap().is_object(),
            "Tool {} schema should be an object",
            tool_name
        );
    }
}

#[tokio::test]
async fn test_agent_get_nonexistent_tool_schema() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let schema = agent.get_tool_schema("nonexistent_tool").await;

    assert!(schema.is_none(), "Nonexistent tool should return None");
}

#[tokio::test]
async fn test_session_creation() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        record_trajectory: false,
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let session = agent.create_session().await;

    assert!(session.session_id().starts_with("gpt-oss-"));
    assert!(!session.is_recording(), "Recording should be disabled");

    let history = session.history().await;
    assert!(history.is_empty(), "New session should have empty history");

    let state = session.state().await;
    assert_eq!(state.turn, 0, "New session should start at turn 0");
    assert_eq!(state.tokens_in, 0, "New session should have 0 tokens in");
    assert_eq!(state.tokens_out, 0, "New session should have 0 tokens out");
}

#[tokio::test]
async fn test_session_with_trajectory_recording() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        record_trajectory: true,
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let session = agent.create_session().await;

    assert!(
        session.is_recording(),
        "Recording should be enabled when configured"
    );
}

#[tokio::test]
async fn test_session_clear_history() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();
    let session = agent.create_session().await;

    // Clear should work even on empty session
    session.clear().await;

    let history = session.history().await;
    assert!(history.is_empty());

    let state = session.state().await;
    assert_eq!(state.turn, 0);
}

#[tokio::test]
async fn test_multiple_sessions_have_unique_ids() {
    let temp_dir = TempDir::new().unwrap();
    let config = GptOssAgentConfig {
        workspace_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.unwrap();

    let session1 = agent.create_session().await;
    let session2 = agent.create_session().await;
    let session3 = agent.create_session().await;

    let id1 = session1.session_id();
    let id2 = session2.session_id();
    let id3 = session3.session_id();

    assert_ne!(id1, id2, "Session IDs must be unique");
    assert_ne!(id2, id3, "Session IDs must be unique");
    assert_ne!(id1, id3, "Session IDs must be unique");
}

#[tokio::test]
async fn test_agent_config_defaults() {
    let config = GptOssAgentConfig::default();

    assert_eq!(config.base_url, "http://localhost:8000");
    assert_eq!(config.model, "gpt-oss-20b");
    assert!(!config.record_trajectory);
    assert!(config.workspace_root.exists() || config.workspace_root == PathBuf::from("."));
}
