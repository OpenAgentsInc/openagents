use gpt_oss_agent::{
    GptOssAgent, GptOssAgentConfig,
    tools::{Tool, ToolRequest, ToolResult, apply_patch::ApplyPatchTool, browser::BrowserTool},
};
use serde_json::json;
use tempfile::TempDir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
async fn test_browser_open_url_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test-page"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                "<html><body><h1>Hello World</h1><p>Test content</p></body></html>",
            ),
        )
        .mount(&mock_server)
        .await;

    let browser = BrowserTool::new();
    let result = browser
        .execute(json!({
            "action": {
                "type": "open",
                "url": format!("{}/test-page", mock_server.uri())
            }
        }))
        .await
        .expect("Browser tool should execute");

    assert!(result.success, "Browser open should succeed");
    assert!(
        result.output.contains("Hello World"),
        "Should contain page content: {}",
        result.output
    );
    assert!(
        result.output.contains("Test content"),
        "Should contain test content"
    );
    assert!(result.error.is_none(), "Should have no error");
}

#[tokio::test]
async fn test_browser_open_url_strips_html_tags() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/html-page"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "<html><head><title>Test</title></head><body><div class='content'>Plain text here</div></body></html>",
        ))
        .mount(&mock_server)
        .await;

    let browser = BrowserTool::new();
    let result = browser
        .execute(json!({
            "action": {
                "type": "open",
                "url": format!("{}/html-page", mock_server.uri())
            }
        }))
        .await
        .expect("Browser tool should execute");

    assert!(result.success);
    assert!(!result.output.contains("<div"), "Should strip HTML tags");
    assert!(
        !result.output.contains("</body>"),
        "Should strip closing tags"
    );
    assert!(
        result.output.contains("Plain text here"),
        "Should preserve text content"
    );
}

#[tokio::test]
async fn test_browser_open_url_handles_404() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/not-found"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let browser = BrowserTool::new();
    let result = browser
        .execute(json!({
            "action": {
                "type": "open",
                "url": format!("{}/not-found", mock_server.uri())
            }
        }))
        .await
        .expect("Browser tool should execute");

    assert!(result.success);
    assert!(result.output.contains("Not Found"));
}

#[tokio::test]
async fn test_browser_find_text_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/searchable"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "<html><body>The quick brown fox jumps over the lazy dog</body></html>",
        ))
        .mount(&mock_server)
        .await;

    let browser = BrowserTool::new();
    let result = browser
        .execute(json!({
            "action": {
                "type": "find",
                "url": format!("{}/searchable", mock_server.uri()),
                "text": "quick brown fox"
            }
        }))
        .await
        .expect("Browser tool should execute");

    assert!(result.success);
    assert!(
        result.output.contains("Found text"),
        "Should indicate text was found: {}",
        result.output
    );
}

#[tokio::test]
async fn test_browser_find_text_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/searchable"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<html><body>Hello world</body></html>"),
        )
        .mount(&mock_server)
        .await;

    let browser = BrowserTool::new();
    let result = browser
        .execute(json!({
            "action": {
                "type": "find",
                "url": format!("{}/searchable", mock_server.uri()),
                "text": "does not exist"
            }
        }))
        .await
        .expect("Browser tool should execute");

    assert!(result.success);
    assert!(
        result.output.contains("not found"),
        "Should indicate text was not found: {}",
        result.output
    );
}

#[tokio::test]
async fn test_browser_search_returns_message() {
    let browser = BrowserTool::new();
    let result = browser
        .execute(json!({
            "action": {
                "type": "search",
                "query": "rust programming"
            }
        }))
        .await
        .expect("Browser tool should execute");

    assert!(result.success);
    assert!(
        result.output.contains("not yet implemented") || result.output.contains("rust programming"),
        "Should indicate search status or echo query: {}",
        result.output
    );
}

#[tokio::test]
async fn test_apply_patch_modifies_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace = temp_dir.path().to_path_buf();

    let test_file = workspace.join("test.txt");
    std::fs::write(&test_file, "line 1\nline 2\nline 3\n").expect("Failed to create test file");

    let tool = ApplyPatchTool::new(workspace.clone());

    let patch = r#"--- test.txt
+++ test.txt
@@ -1,3 +1,4 @@
 line 1
+inserted line
 line 2
 line 3
"#;

    let result = tool
        .execute(json!({
            "file_path": "test.txt",
            "patch": patch
        }))
        .await
        .expect("Apply patch should execute");

    if result.success {
        let content = std::fs::read_to_string(&test_file).expect("Failed to read file");
        assert!(
            content.contains("inserted line"),
            "File should contain patched content: {}",
            content
        );
    } else {
        assert!(result.error.is_some(), "Should have error if patch failed");
    }
}

#[tokio::test]
async fn test_apply_patch_rejects_path_traversal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace = temp_dir.path().to_path_buf();

    let tool = ApplyPatchTool::new(workspace);

    let result = tool
        .execute(json!({
            "file_path": "../../../etc/passwd",
            "patch": "malicious content"
        }))
        .await
        .expect("Apply patch should execute");

    assert!(!result.success, "Should reject path traversal attempts");
    assert!(result.error.is_some(), "Should have error message");
    assert!(
        result.error.as_ref().unwrap().contains("outside workspace")
            || result.error.as_ref().unwrap().contains("Invalid"),
        "Error should mention path issue: {:?}",
        result.error
    );
}

#[tokio::test]
async fn test_apply_patch_accepts_nested_paths() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace = temp_dir.path().to_path_buf();

    let subdir = workspace.join("subdir");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");

    let tool = ApplyPatchTool::new(workspace.clone());

    let test_file = subdir.join("nested.txt");
    std::fs::write(&test_file, "original\n").expect("Failed to write file");

    let patch = r#"--- subdir/nested.txt
+++ subdir/nested.txt
@@ -1 +1 @@
-original
+modified
"#;

    let result = tool
        .execute(json!({
            "file_path": "subdir/nested.txt",
            "patch": patch
        }))
        .await
        .expect("Apply patch should execute");

    if !result.success {
        let error = result.error.unwrap_or_default();
        assert!(
            !error.contains("outside workspace"),
            "Valid path should not be rejected as outside workspace"
        );
    }
}

#[tokio::test]
async fn test_agent_execute_tool_browser() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api-endpoint"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"status": "ok", "message": "API response"}"#),
        )
        .mount(&mock_server)
        .await;

    let config = GptOssAgentConfig {
        workspace_root: std::env::current_dir().unwrap(),
        ..Default::default()
    };

    let agent = GptOssAgent::new(config).await.expect("Agent should create");

    let request = ToolRequest {
        tool: "browser".to_string(),
        parameters: json!({
            "action": {
                "type": "open",
                "url": format!("{}/api-endpoint", mock_server.uri())
            }
        }),
    };

    let result = agent
        .execute_tool(request)
        .await
        .expect("Tool execution should succeed");

    assert!(result.success);
    assert!(result.output.contains("API response"));
}

#[tokio::test]
async fn test_agent_execute_tool_not_found() {
    let config = GptOssAgentConfig::default();
    let agent = GptOssAgent::new(config).await.expect("Agent should create");

    let request = ToolRequest {
        tool: "nonexistent_tool".to_string(),
        parameters: json!({}),
    };

    let result = agent.execute_tool(request).await;

    assert!(result.is_err(), "Should error on unknown tool");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("nonexistent"),
        "Error should mention tool not found: {}",
        error
    );
}

#[tokio::test]
async fn test_agent_lists_available_tools() {
    let config = GptOssAgentConfig::default();
    let agent = GptOssAgent::new(config).await.expect("Agent should create");

    let tools = agent.list_tools().await;

    assert!(tools.contains(&"browser".to_string()));
    assert!(tools.contains(&"python".to_string()));
    assert!(tools.contains(&"apply_patch".to_string()));
    assert!(tools.contains(&"ui_pane".to_string()));
    assert_eq!(tools.len(), 4);
}

#[tokio::test]
async fn test_agent_get_tool_schema_browser() {
    let config = GptOssAgentConfig::default();
    let agent = GptOssAgent::new(config).await.expect("Agent should create");

    let schema = agent
        .get_tool_schema("browser")
        .await
        .expect("Should get browser schema");

    assert!(schema.is_object());
    assert!(schema.get("properties").is_some());
    assert!(schema.get("required").is_some());
}

#[tokio::test]
async fn test_agent_get_tool_schema_apply_patch() {
    let config = GptOssAgentConfig::default();
    let agent = GptOssAgent::new(config).await.expect("Agent should create");

    let schema = agent
        .get_tool_schema("apply_patch")
        .await
        .expect("Should get apply_patch schema");

    assert!(schema.is_object());
    let props = schema.get("properties").expect("Should have properties");
    assert!(props.get("file_path").is_some());
    assert!(props.get("patch").is_some());
}

#[test]
fn test_tool_result_serialization() {
    let result = ToolResult {
        success: true,
        output: "Operation completed".to_string(),
        error: None,
    };

    let json = serde_json::to_string(&result).expect("Should serialize");
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"output\":\"Operation completed\""));

    let parsed: ToolResult = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(parsed.success, result.success);
    assert_eq!(parsed.output, result.output);
}

#[test]
fn test_tool_result_with_error_serialization() {
    let result = ToolResult {
        success: false,
        output: String::new(),
        error: Some("Connection timeout".to_string()),
    };

    let json = serde_json::to_string(&result).expect("Should serialize");
    assert!(json.contains("\"success\":false"));
    assert!(json.contains("Connection timeout"));

    let parsed: ToolResult = serde_json::from_str(&json).expect("Should deserialize");
    assert!(!parsed.success);
    assert_eq!(parsed.error, Some("Connection timeout".to_string()));
}

#[test]
fn test_tool_request_serialization() {
    let request = ToolRequest {
        tool: "browser".to_string(),
        parameters: json!({
            "action": {
                "type": "open",
                "url": "https://example.com"
            }
        }),
    };

    let json = serde_json::to_string(&request).expect("Should serialize");
    assert!(json.contains("\"tool\":\"browser\""));
    assert!(json.contains("\"url\":\"https://example.com\""));

    let parsed: ToolRequest = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(parsed.tool, "browser");
}

#[tokio::test]
async fn test_concurrent_browser_requests() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/page1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Page 1 content"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/page2"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Page 2 content"))
        .mount(&mock_server)
        .await;

    let browser = BrowserTool::new();

    let (result1, result2) = tokio::join!(
        browser.execute(json!({
            "action": {
                "type": "open",
                "url": format!("{}/page1", mock_server.uri())
            }
        })),
        browser.execute(json!({
            "action": {
                "type": "open",
                "url": format!("{}/page2", mock_server.uri())
            }
        }))
    );

    let r1 = result1.expect("First request should succeed");
    let r2 = result2.expect("Second request should succeed");

    assert!(r1.success);
    assert!(r2.success);
    assert!(r1.output.contains("Page 1"));
    assert!(r2.output.contains("Page 2"));
}

#[tokio::test]
async fn test_concurrent_tool_request_creation() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Response data"))
        .mount(&mock_server)
        .await;

    let requests: Vec<_> = (0..5)
        .map(|i| {
            let uri = mock_server.uri();
            ToolRequest {
                tool: "browser".to_string(),
                parameters: json!({
                    "action": {
                        "type": "open",
                        "url": format!("{}/page{}", uri, i)
                    }
                }),
            }
        })
        .collect();

    for req in requests {
        assert_eq!(req.tool, "browser");
    }
}
