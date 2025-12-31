use clap::{Parser, Subcommand};
use futures::StreamExt;
use reqwest::Client;
use std::error::Error;

#[derive(Parser)]
#[command(name = "agentctl")]
#[command(about = "Control plane CLI for OpenAgents runtime")]
struct Cli {
    /// Runtime endpoint base URL.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    endpoint: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List agents.
    List,
    /// Fetch agent status.
    Status { id: String },
    /// Send a message to an agent.
    Send { id: String, message: String },
    /// Trigger a manual tick.
    Tick { id: String },
    /// Show logs (follow for stream).
    Logs {
        id: String,
        #[arg(long)]
        follow: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let client = Client::new();
    match cli.command {
        Command::List => list_agents(&client, &cli.endpoint).await?,
        Command::Status { id } => status_agent(&client, &cli.endpoint, &id).await?,
        Command::Send { id, message } => send_agent(&client, &cli.endpoint, &id, &message).await?,
        Command::Tick { id } => tick_agent(&client, &cli.endpoint, &id).await?,
        Command::Logs { id, follow } => logs_agent(&client, &cli.endpoint, &id, follow).await?,
    }
    Ok(())
}

async fn list_agents(client: &Client, endpoint: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/agents", endpoint.trim_end_matches('/'));
    let resp = client.get(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    if let Ok(ids) = serde_json::from_str::<Vec<String>>(&body) {
        for id in ids {
            println!("{id}");
        }
    } else {
        println!("{body}");
    }
    Ok(())
}

async fn status_agent(client: &Client, endpoint: &str, id: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/agents/{}/status", endpoint.trim_end_matches('/'), id);
    let resp = client.get(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    println!("{body}");
    Ok(())
}

async fn send_agent(
    client: &Client,
    endpoint: &str,
    id: &str,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/agents/{}/send", endpoint.trim_end_matches('/'), id);
    client
        .post(url)
        .body(message.to_string())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn tick_agent(client: &Client, endpoint: &str, id: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/agents/{}/tick", endpoint.trim_end_matches('/'), id);
    let resp = client.post(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    println!("{body}");
    Ok(())
}

async fn logs_agent(
    client: &Client,
    endpoint: &str,
    id: &str,
    follow: bool,
) -> Result<(), Box<dyn Error>> {
    if follow {
        let url = format!(
            "{}/agents/{}/logs/trace?watch=1",
            endpoint.trim_end_matches('/'),
            id
        );
        let resp = client.get(url).send().await?.error_for_status()?;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end();
                if let Some(data) = line.strip_prefix("data:") {
                    println!("{}", data.trim());
                }
                buffer = buffer[pos + 1..].to_string();
            }
        }
    } else {
        let url = format!(
            "{}/agents/{}/logs/recent",
            endpoint.trim_end_matches('/'),
            id
        );
        let resp = client.get(url).send().await?.error_for_status()?;
        let body = resp.text().await?;
        println!("{body}");
    }
    Ok(())
}
