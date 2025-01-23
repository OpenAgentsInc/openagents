use anyhow::Result;
use clap::{Parser, Subcommand};
use openagents::server::services::{
    deepseek::{AssistantMessage, ChatMessage, DeepSeekService},
    StreamUpdate,
};
use serde_json::json;
use std::io::{stdout, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

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
        /// Enable debug output
        #[arg(long)]
        debug: bool,
        /// Disable streaming output
        #[arg(long)]
        no_stream: bool,
    },
    /// Reasoning mode with Chain of Thought
    Reason {
        /// The message to reason about
        message: String,
        /// Enable debug output
        #[arg(long)]
        debug: bool,
        /// Disable streaming output
        #[arg(long)]
        no_stream: bool,
    },
    /// Weather tool example
    Weather {
        /// The location to check weather for
        location: String,
        /// Enable debug output
        #[arg(long)]
        debug: bool,
        /// Disable streaming output
        #[arg(long)]
        no_stream: bool,
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
        Commands::Chat {
            message,
            debug: _,
            no_stream,
        } => {
            if no_stream {
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
        Commands::Reason {
            message,
            debug: _,
            no_stream,
        } => {
            if no_stream {
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
                        _ => {}
                    }
                }
                println!();
            }
        }
        Commands::Weather {
            location,
            debug,
            no_stream: _,
        } => {
            // Create weather tool
            let get_weather_tool = DeepSeekService::create_tool(
                "get_weather".to_string(),
                Some("Get weather for a location".to_string()),
                json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city name"
                        }
                    },
                    "required": ["location"]
                }),
            );

            // Make initial request with tool
            print_colored("Asking about weather...\n", Color::Blue)?;

            if debug {
                println!("\nSending initial request with tool definition:");
                println!("{}", serde_json::to_string_pretty(&get_weather_tool)?);
            }

            let (content, _, tool_calls) = service
                .chat_with_tools(
                    format!("What's the weather in {}?", location),
                    vec![get_weather_tool.clone()],
                    None,
                    false,
                )
                .await?;

            println!("Initial response: {}", content);

            // If there's a tool call, handle it
            if let Some(tool_calls) = tool_calls {
                let tool_calls = tool_calls.clone(); // Clone here to avoid move
                for tool_call in &tool_calls {
                    if tool_call.function.name == "get_weather" {
                        print_colored("\nTool called: get_weather\n", Color::Yellow)?;
                        println!("Arguments: {}", tool_call.function.arguments);

                        // Initial user message
                        let user_message = ChatMessage {
                            role: "user".to_string(),
                            content: format!("What's the weather in {}?", location),
                            tool_call_id: None,
                            tool_calls: None,
                        };

                        // Assistant message with tool call
                        let assistant_message = ChatMessage {
                            role: "assistant".to_string(),
                            content: content.clone(),
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                        };

                        // Tool response message
                        let weather_message = ChatMessage {
                            role: "tool".to_string(),
                            content: "20°C and cloudy".to_string(),
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_calls: None,
                        };

                        if debug {
                            println!("\nSending messages:");
                            println!("1. User message:");
                            println!("   Role: {}", user_message.role);
                            println!("   Content: {}", user_message.content);
                            println!("2. Assistant message:");
                            println!("   Role: {}", assistant_message.role);
                            println!("   Content: {}", assistant_message.content);
                            println!("   Tool calls: {:?}", tool_calls);
                            println!("3. Tool response:");
                            println!("   Role: {}", weather_message.role);
                            println!("   Content: {}", weather_message.content);
                            println!("   Tool call ID: {}", weather_message.tool_call_id.as_ref().unwrap());
                        }

                        // Get final response
                        print_colored("\nGetting final response...\n", Color::Blue)?;

                        let messages = vec![
                            user_message,
                            assistant_message,
                            weather_message.clone(),
                        ];

                        let result = service
                            .chat_with_tool_response(
                                messages,
                                weather_message,
                                vec![get_weather_tool.clone()],
                                false,
                            )
                            .await;

                        match result {
                            Ok((final_content, _, _)) => {
                                print_colored("Final response: ", Color::Green)?;
                                println!("{}", final_content);
                            }
                            Err(e) => {
                                print_colored("Error getting final response: ", Color::Red)?;
                                println!("{}", e);
                                if debug {
                                    println!("\nError details: {:#?}", e);
                                }
                            }
                        }
                    }
                }
            } else {
                println!("No tool calls received");
            }
        }
    }

    Ok(())
}