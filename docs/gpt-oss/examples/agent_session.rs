use gpt_oss_agent::{GptOssAgent, GptOssAgentConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = GptOssAgentConfig {
        base_url: "http://localhost:8000".to_string(),
        model: "gpt-oss-20b".to_string(),
        workspace_root: std::env::current_dir()?,
        record_trajectory: false,
    };

    let agent = GptOssAgent::new(config).await?;
    let session = agent.create_session().await;

    let reply = session.send("Summarize the repo in one sentence.").await?;
    println!("{}", reply.trim());

    Ok(())
}
