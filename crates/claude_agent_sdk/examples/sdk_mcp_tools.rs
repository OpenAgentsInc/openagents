//! A custom tool written in Rust and run inside this process: an
//! SDK-hosted MCP server with one tool, `add(a, b)`, that Claude calls.
//!
//! ```sh
//! cargo run -p claude_agent_sdk --example sdk_mcp_tools
//! ```
//!
//! Like the `smoke` example it runs on the Claude Code login (no API key),
//! saves no session, loads no settings files, and offers no built-in tools;
//! the only tool is `mcp__calc__add`.

use claude_agent_sdk::{
    QueryOptions, SdkMcpServer, SdkMessage, SdkResultMessage, ToolResult, ToolsConfig, query,
};
use futures::StreamExt;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let calc = SdkMcpServer::new("calc", "1.0.0").tool(
        "add",
        "Add two numbers and return the sum.",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First addend"},
                "b": {"type": "number", "description": "Second addend"}
            },
            "required": ["a", "b"]
        }),
        move |args| {
            let seen = seen.clone();
            async move {
                seen.fetch_add(1, Ordering::SeqCst);
                let (Some(a), Some(b)) = (args["a"].as_f64(), args["b"].as_f64()) else {
                    return Ok(ToolResult::error("a and b must be numbers"));
                };
                println!("tool: add({a}, {b}) = {}", a + b);
                Ok(ToolResult::text((a + b).to_string()))
            }
        },
    );
    let add = calc.tool_name("add");

    let mut options = QueryOptions::new()
        .model(std::env::var("SMOKE_MODEL").unwrap_or_else(|_| "haiku".into()))
        .max_turns(3)
        .no_session_persistence()
        .sdk_mcp_server(calc);
    options.tools = Some(ToolsConfig::Names(Vec::new()));
    options.allowed_tools = Some(vec![add.clone()]);
    options.setting_sources = Some(Vec::new());
    options.env_remove = vec!["ANTHROPIC_API_KEY".into()];

    let prompt = format!("Use the {add} tool to add 1234 and 5678, then reply with only the sum.");
    let mut stream = query(prompt, options).await?;
    let mut result_text = None;
    while let Some(message) = stream.next().await {
        match message? {
            SdkMessage::System(claude_agent_sdk::SdkSystemMessage::Init(init)) => {
                println!(
                    "system init: claude_code_version={} mcp_servers={}",
                    init.claude_code_version,
                    serde_json::to_string(&init.mcp_servers)?
                );
            }
            SdkMessage::Result(SdkResultMessage::Success(result)) => {
                println!(
                    "result: success turns={} text={:?}",
                    result.num_turns, result.result
                );
                result_text = Some(result.result);
            }
            SdkMessage::Result(other) => {
                println!("result: {other:?}");
                return Err("turn did not succeed".into());
            }
            _ => {}
        }
    }
    let calls = calls.load(Ordering::SeqCst);
    println!("add calls: {calls}");
    if calls == 0 {
        return Err("Claude never called the add tool".into());
    }
    if !result_text.unwrap_or_default().contains("6912") {
        return Err("the reply does not carry the sum 6912".into());
    }
    Ok(())
}
