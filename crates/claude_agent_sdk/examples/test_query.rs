use claude_agent_sdk::{query, QueryOptions, SettingSource};
use claude_agent_sdk::transport::ExecutableConfig;
use futures::StreamExt;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    println!("Testing Claude Agent SDK...");

    let home = std::env::var("HOME").unwrap_or_default();
    let claude_path = format!("{}/.claude/local/claude", home);
    println!("Using claude at: {}", claude_path);

    let mut exec_config = ExecutableConfig::default();
    exec_config.path = Some(PathBuf::from(&claude_path));

    let cwd = std::env::current_dir().expect("Failed to get cwd");
    println!("CWD: {:?}", cwd);

    let mut options = QueryOptions::new()
        .model("claude-sonnet-4-20250514")
        .max_turns(1)
        .cwd(cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);
    options.executable = exec_config;

    println!("Calling query()...");
    let mut stream = match query("say hi", options).await {
        Ok(s) => {
            println!("Query started successfully!");
            s
        }
        Err(e) => {
            println!("Query failed: {}", e);
            return;
        }
    };

    println!("Streaming messages...");
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(m) => println!("Message: {:?}", m),
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }
    println!("Done!");
}
