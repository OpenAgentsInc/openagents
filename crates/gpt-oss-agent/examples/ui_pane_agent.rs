use gpt_oss_agent::tools::Tool;
use gpt_oss_agent::tools::ui_pane::{PaneManager, UiPaneTool};
use std::sync::{Arc, RwLock};
use std::time::Duration;

fn log_tool_call(tool_name: &str, action: &str, params: &serde_json::Value) {
    println!("\n{}", "=".repeat(70));
    println!("TOOL CALL: {}", tool_name);
    println!("ACTION: {}", action);
    println!("PARAMS: {}", serde_json::to_string_pretty(params).unwrap());
    println!("{}", "-".repeat(70));
}

fn log_result(result: &gpt_oss_agent::tools::ToolResult) {
    if result.success {
        println!("SUCCESS: {}", result.output);
    } else {
        println!(
            "FAILED: {}",
            result
                .error
                .as_ref()
                .unwrap_or(&"Unknown error".to_string())
        );
    }
    println!("{}", "=".repeat(70));
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

#[tokio::main]
async fn main() {
    println!("\n");
    println!("{}", "#".repeat(70));
    println!("# UI PANE AGENT TOOL CALL DEMONSTRATION");
    println!("# Simulating an agent manipulating UI panes");
    println!("{}", "#".repeat(70));

    let manager = Arc::new(RwLock::new(PaneManager::new()));
    let tool = UiPaneTool::new(manager.clone());

    println!("\n[SCENARIO: Agent sets up a development workspace]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "CreatePane",
        "id": "editor",
        "title": "Code Editor",
        "position": { "x": 50.0, "y": 50.0 },
        "size": { "width": 800.0, "height": 600.0 },
        "content_type": "code"
    });
    log_tool_call("ui_pane", "CreatePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(300);

    let params = serde_json::json!({
        "action": "CreatePane",
        "id": "terminal",
        "title": "Terminal",
        "position": { "x": 50.0, "y": 670.0 },
        "size": { "width": 800.0, "height": 200.0 },
        "content_type": "terminal"
    });
    log_tool_call("ui_pane", "CreatePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(300);

    let params = serde_json::json!({
        "action": "CreatePane",
        "id": "chat",
        "title": "AI Assistant",
        "position": { "x": 870.0, "y": 50.0 },
        "size": { "width": 400.0, "height": 500.0 },
        "content_type": "chat"
    });
    log_tool_call("ui_pane", "CreatePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(300);

    let params = serde_json::json!({
        "action": "CreatePane",
        "id": "diagnostics",
        "title": "Diagnostics",
        "position": { "x": 870.0, "y": 570.0 },
        "size": { "width": 400.0, "height": 300.0 },
        "content_type": "diagnostics"
    });
    log_tool_call("ui_pane", "CreatePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(500);

    println!("\n[SCENARIO: Agent found an error, needs user attention on diagnostics]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "SetPriority",
        "id": "diagnostics",
        "priority": "Urgent"
    });
    log_tool_call("ui_pane", "SetPriority", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(300);

    let params = serde_json::json!({
        "action": "Focus",
        "id": "diagnostics"
    });
    log_tool_call("ui_pane", "Focus", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(300);

    let params = serde_json::json!({
        "action": "Animate",
        "id": "diagnostics",
        "animation": {
            "Pulse": { "count": 3, "duration_ms": 500 }
        }
    });
    log_tool_call("ui_pane", "Animate", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(300);

    let params = serde_json::json!({
        "action": "SetFrameStyle",
        "id": "diagnostics",
        "style": "Kranox"
    });
    log_tool_call("ui_pane", "SetFrameStyle", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(500);

    println!("\n[SCENARIO: User acknowledged error, agent shows fix in editor]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "SetPriority",
        "id": "diagnostics",
        "priority": "Normal"
    });
    log_tool_call("ui_pane", "SetPriority", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "SetGlow",
        "id": "diagnostics",
        "color": null
    });
    log_tool_call("ui_pane", "SetGlow", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "Focus",
        "id": "editor"
    });
    log_tool_call("ui_pane", "Focus", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "SetGlow",
        "id": "editor",
        "color": "#00ff88"
    });
    log_tool_call("ui_pane", "SetGlow", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(500);

    println!("\n[SCENARIO: Agent needs to run tests, focuses terminal]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "MovePane",
        "id": "terminal",
        "position": { "x": 50.0, "y": 400.0 },
        "animate": true
    });
    log_tool_call("ui_pane", "MovePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "ResizePane",
        "id": "terminal",
        "size": { "width": 800.0, "height": 400.0 },
        "animate": true
    });
    log_tool_call("ui_pane", "ResizePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "Focus",
        "id": "terminal"
    });
    log_tool_call("ui_pane", "Focus", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "SetPriority",
        "id": "terminal",
        "priority": "Elevated"
    });
    log_tool_call("ui_pane", "SetPriority", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(500);

    println!("\n[SCENARIO: Tests passed! Agent minimizes terminal, shows success in chat]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "SetState",
        "id": "terminal",
        "state": "Minimized"
    });
    log_tool_call("ui_pane", "SetState", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(200);

    let params = serde_json::json!({
        "action": "RequestAttention",
        "id": "chat",
        "message": "All tests passed! Ready for review."
    });
    log_tool_call("ui_pane", "RequestAttention", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(500);

    println!("\n[SCENARIO: Agent lists all panes and their states]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "ListPanes"
    });
    log_tool_call("ui_pane", "ListPanes", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);
    sleep_ms(500);

    println!("\n[SCENARIO: Cleanup - agent closes diagnostics pane]");
    sleep_ms(500);

    let params = serde_json::json!({
        "action": "ClosePane",
        "id": "diagnostics"
    });
    log_tool_call("ui_pane", "ClosePane", &params);
    let result = tool.execute(params).await.unwrap();
    log_result(&result);

    println!("\n");
    println!("{}", "#".repeat(70));
    println!("# DEMONSTRATION COMPLETE");
    println!("# Total tool calls executed: 18");
    println!("{}", "#".repeat(70));
    println!("\n");
}
