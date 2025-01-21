use anyhow::Result;
use clap::{Parser, Subcommand};
use openagents::server::services::{deepseek::DeepSeekService, StreamUpdate};
use std::io::{stdout, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Disable streaming output
    #[arg(long)]
    no_stream: bool,
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

fn print_colored(text: &str, color: Color) -> Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(ColorSpec::new().set_fg(Some(color)))?;
    write!(stdout, "{}", text)?;
    stdout.reset()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY environment variable must be set");

    let service = DeepSeekService::new(api_key);

    match cli.command {
        Commands::Chat { message } => {
            if cli.no_stream {
                let (response, _) = service.chat(message, false).await?;
                println!("{}", response);
            } else {
                let mut stream = service.chat_stream(message, false).await;
                while let Some(update) = stream.recv().await {
                    match update {
                        StreamUpdate::Content(text) => {
                            print!("{}", text);
                            stdout().flush()?;
                        }
                        StreamUpdate::Done => break,
                        _ => {}
                    }
                }
                println!();
            }
        }
        Commands::Reason { message } => {
            if cli.no_stream {
                let (response, reasoning) = service.chat(message, true).await?;
                if let Some(reasoning) = reasoning {
                    print_colored("Reasoning:\n", Color::Yellow)?;
                    println!("{}\n", reasoning);
                }
                print_colored("Response: ", Color::Green)?;
                println!("{}", response);
            } else {
                print_colored("Reasoning:\n", Color::Yellow)?;
                let mut in_reasoning = true;
                let mut stream = service.chat_stream(message, true).await;
                while let Some(update) = stream.recv().await {
                    match update {
                        StreamUpdate::Reasoning(r) => {
                            print_colored(&r, Color::Yellow)?;
                        }
                        StreamUpdate::Content(c) => {
                            if in_reasoning {
                                println!();
                                print_colored("Response: ", Color::Green)?;
                                in_reasoning = false;
                            }
                            print!("{}", c);
                            stdout().flush()?;
                        }
                        StreamUpdate::Done => break,
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}
