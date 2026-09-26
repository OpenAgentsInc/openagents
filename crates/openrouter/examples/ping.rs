//! One live structured call: `cargo run -p openrouter --example ping [MODEL]`.
//! Reads the key as `Config::from_env` does and prints the reply and its cost.

use openrouter::{ChatRequest, Client, Config, Message};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Next {
    rationale: String,
    commands: Vec<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "openai/gpt-6-luna".to_string());
    let client = Client::new(Config::from_env()?)?;
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "rationale": {"type": "string"},
            "commands": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["rationale", "commands"],
        "additionalProperties": false
    });
    let request = ChatRequest::new(
        &model,
        vec![Message::user(
            "List the files in the current directory. Reply with one shell command.",
        )],
    )
    .effort("low");
    let reply = client
        .structured::<Next>(request, "next_action", schema)
        .await?;
    println!(
        "{} · {:?} · {:?}",
        reply.model, reply.value.commands, reply.value.rationale
    );
    println!(
        "{} in, {} out, ${:.6}, {} ms",
        reply.usage.prompt_tokens,
        reply.usage.completion_tokens,
        reply.usage.cost.unwrap_or(0.0),
        reply.milliseconds
    );
    Ok(())
}
