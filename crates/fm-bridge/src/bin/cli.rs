/// FM Bridge CLI
///
/// Simple CLI for testing Foundation Model API

use clap::{Parser, Subcommand};
use fm_bridge::{FMClient, CompletionOptions};
use tokio_stream::StreamExt;

#[derive(Parser)]
#[command(name = "fm")]
#[command(about = "Foundation Model API CLI", long_about = None)]
struct Cli {
    /// Base URL of the FM API server
    #[arg(short, long, default_value = "http://localhost:3030")]
    url: String,

    /// Default model to use
    #[arg(short, long, default_value = "gpt-4o-mini-2024-07-18")]
    model: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Complete a prompt
    Complete {
        /// The prompt to complete
        prompt: String,

        /// Temperature (0.0 - 2.0)
        #[arg(short, long)]
        temperature: Option<f32>,

        /// Maximum tokens to generate
        #[arg(short = 'n', long)]
        max_tokens: Option<u32>,

        /// Enable streaming
        #[arg(short, long)]
        stream: bool,
    },

    /// List available models
    Models,

    /// Health check
    Health,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let client = FMClient::builder()
        .base_url(cli.url)
        .default_model(cli.model)
        .build();

    match cli.command {
        Commands::Complete {
            prompt,
            temperature,
            max_tokens,
            stream,
        } => {
            let options = CompletionOptions {
                model: None, // Use default from client
                temperature,
                max_tokens,
                top_p: None,
                stop: None,
            };

            if stream {
                println!("Streaming response:");
                println!("---");

                let mut stream = client.stream(&prompt, Some(options)).await?;

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            print!("{}", chunk.text);
                            if chunk.finish_reason.is_some() {
                                println!();
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("\nError: {}", e);
                            break;
                        }
                    }
                }

                println!("---");
            } else {
                println!("Sending request...");

                let response = client.complete(&prompt, Some(options)).await?;

                println!("\nResponse:");
                println!("---");
                println!("{}", response.choices[0].message.content);
                println!("---");
                if let Some(usage) = &response.usage {
                    println!(
                        "\nUsage: {} prompt + {} completion = {} total tokens",
                        usage.prompt_tokens.unwrap_or(0),
                        usage.completion_tokens.unwrap_or(0),
                        usage.total_tokens.unwrap_or(0)
                    );
                }
            }
        }

        Commands::Models => {
            println!("Fetching available models...\n");

            let models = client.models().await?;

            println!("Available models:");
            for model in models {
                println!("  - {}", model.id);
                println!("    Owner: {}", model.owned_by);
            }
        }

        Commands::Health => {
            println!("Checking health...");

            match client.health().await {
                Ok(true) => {
                    println!("✓ API is healthy");
                    std::process::exit(0);
                }
                Ok(false) => {
                    println!("✗ API is not healthy");
                    std::process::exit(1);
                }
                Err(e) => {
                    println!("✗ Health check failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
