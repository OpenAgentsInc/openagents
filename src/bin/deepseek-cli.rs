use anyhow::Result;
use clap::{Parser, Subcommand};
use openagents::server::services::deepseek::DeepSeekService;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Regular chat mode
    Chat {
        /// The message to send
        message: String,
    },
    /// Reasoning mode with Chain of Thought
    Reason {
        /// The message to reason about
        message: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY environment variable must be set");
    
    let service = DeepSeekService::new(api_key);

    match cli.command {
        Commands::Chat { message } => {
            let (response, _) = service.chat(message, false).await?;
            println!("Response: {}", response);
        }
        Commands::Reason { message } => {
            let (response, reasoning) = service.chat(message, true).await?;
            if let Some(reasoning) = reasoning {
                println!("Reasoning:\n{}\n", reasoning);
            }
            println!("Response: {}", response);
        }
    }

    Ok(())
}