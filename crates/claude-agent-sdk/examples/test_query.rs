use claude_agent_sdk::{query, QueryOptions, SettingSource};
use futures::StreamExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    println!("Testing Claude Agent SDK...");

    let cwd = std::env::current_dir().expect("Failed to get cwd");
    println!("CWD: {:?}", cwd);

    // Let SDK find claude automatically via PATH or common locations
    let options = QueryOptions::new()
        .max_turns(1)
        .cwd(cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);

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
