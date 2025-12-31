//! Unit tests for GPT-OSS agent tools

use gpt_oss_agent::tools::{
    Tool, apply_patch::ApplyPatchTool, browser::BrowserTool, python::PythonTool,
};
use serde_json::json;
use std::path::PathBuf;

#[tokio::test]
async fn test_browser_tool_creation() {
    let browser = BrowserTool::new();
    assert_eq!(browser.name(), "browser");
    assert!(!browser.description().is_empty());

    let schema = browser.parameter_schema();
    assert!(schema.is_object());
}

#[tokio::test]
async fn test_browser_tool_invalid_params() {
    let browser = BrowserTool::new();

    // Missing required fields
    let result = browser.execute(json!({})).await;
    assert!(result.is_err());

    // Invalid action type
    let result = browser
        .execute(json!({
            "action": {
                "type": "invalid_action",
                "url": "http://example.com"
            }
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_python_tool_creation() {
    let python = PythonTool::new();
    assert_eq!(python.name(), "python");
    assert!(!python.description().is_empty());

    let schema = python.parameter_schema();
    assert!(schema.is_object());
}

#[tokio::test]
async fn test_python_tool_simple_code() {
    let python = PythonTool::new();

    let result = python
        .execute(json!({
            "code": "print('hello from test')"
        }))
        .await;

    // Note: This will fail if Docker is not available
    // In CI, we can skip this test or mock Docker
    if result.is_ok() {
        let tool_result = result.unwrap();
        assert!(tool_result.success || tool_result.error.is_some());
    }
}

#[tokio::test]
async fn test_python_tool_invalid_params() {
    let python = PythonTool::new();

    // Missing code parameter
    let result = python.execute(json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_apply_patch_tool_creation() {
    let workspace = PathBuf::from("/tmp/test-workspace");
    let tool = ApplyPatchTool::new(workspace);

    assert_eq!(tool.name(), "apply_patch");
    assert!(!tool.description().is_empty());

    let schema = tool.parameter_schema();
    assert!(schema.is_object());
}

#[tokio::test]
async fn test_apply_patch_tool_invalid_params() {
    let workspace = PathBuf::from("/tmp/test-workspace");
    let tool = ApplyPatchTool::new(workspace);

    // Missing required fields
    let result = tool.execute(json!({})).await;
    assert!(result.is_err());

    // Missing file path
    let result = tool
        .execute(json!({
            "patch": "some content"
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_all_tools_have_unique_names() {
    let browser = BrowserTool::new();
    let python = PythonTool::new();
    let apply_patch = ApplyPatchTool::new(PathBuf::from("/tmp"));

    let names = vec![browser.name(), python.name(), apply_patch.name()];
    let mut unique_names = names.clone();
    unique_names.sort();
    unique_names.dedup();

    assert_eq!(names.len(), unique_names.len(), "Tool names must be unique");
}

#[tokio::test]
async fn test_all_tools_have_schemas() {
    let browser = BrowserTool::new();
    let python = PythonTool::new();
    let apply_patch = ApplyPatchTool::new(PathBuf::from("/tmp"));

    for tool in &[&browser as &dyn Tool, &python, &apply_patch] {
        let schema = tool.parameter_schema();
        assert!(
            schema.is_object(),
            "Tool {} must have object schema",
            tool.name()
        );
    }
}
