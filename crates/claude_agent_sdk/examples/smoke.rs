//! Live smoke against the installed `claude` CLI: the `initialize`
//! handshake and one short turn.
//!
//! ```sh
//! cargo run -p claude_agent_sdk --example smoke
//! ```
//!
//! The session is not saved (`--no-session-persistence`), so nothing lands
//! in the CLI's history. It loads no settings files, offers no tools, and
//! asks the cheapest model alias for one turn.

use claude_agent_sdk::{QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig, query};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = QueryOptions::new()
        .model(std::env::var("SMOKE_MODEL").unwrap_or_else(|_| "haiku".into()))
        .max_turns(1)
        .no_session_persistence();
    options.tools = Some(ToolsConfig::Names(Vec::new()));
    options.setting_sources = Some(Vec::new());
    // Run on the Claude Code login, never an API key.
    options.env_remove = vec!["ANTHROPIC_API_KEY".into()];

    let mut stream = query("Reply with the single word: pong", options).await?;
    let init = stream
        .initialization_result()
        .ok_or("initialize returned no payload")?;
    let models = init["models"].as_array().map_or(0, Vec::len);
    let commands = init["commands"].as_array().map_or(0, Vec::len);
    println!(
        "initialize: ok ({models} models, {commands} commands, claude_code_version={})",
        stream.claude_code_version().unwrap_or("absent")
    );

    let mut unknown = Vec::new();
    while let Some(message) = stream.next().await {
        match message? {
            SdkMessage::System(claude_agent_sdk::SdkSystemMessage::Init(init)) => {
                println!(
                    "system init: claude_code_version={} model={}",
                    init.claude_code_version, init.model
                );
            }
            SdkMessage::Result(SdkResultMessage::Success(result)) => {
                println!(
                    "result: success turns={} cost_usd={:.6} terminal_reason={:?} text={:?}",
                    result.num_turns, result.total_cost_usd, result.terminal_reason, result.result
                );
            }
            SdkMessage::Result(other) => {
                println!("result: {other:?}");
                return Err("turn did not succeed".into());
            }
            SdkMessage::Unknown { type_name, raw } => {
                unknown.push(format!("{type_name}/{}", raw["subtype"]));
            }
            other => println!("message: {}", other.type_name()),
        }
    }
    if unknown.is_empty() {
        println!("unknown messages: none");
    } else {
        println!("unknown messages: {unknown:?}");
    }
    Ok(())
}
