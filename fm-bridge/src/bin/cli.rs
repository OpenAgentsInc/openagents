use clap::{Parser, Subcommand};
use fm_bridge::{FMClient, StreamingClient, CompletionOptions, ChatMessage};
use futures::StreamExt;

#[derive(Parser)]
#[command(name = "fm")]
#[command(about = "Foundation Models HTTP Bridge CLI", long_about = None)]
struct Cli {
    /// Base URL of the bridge server
    #[arg(short, long, default_value = "http://localhost:11435")]
    url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check health status
    Health,

    /// List available models
    Models,

    /// Complete a prompt
    Complete {
        /// The prompt to complete
        prompt: String,

        /// Temperature (0.0-2.0)
        #[arg(short, long)]
        temperature: Option<f64>,

        /// Maximum tokens to generate
        #[arg(short, long)]
        max_tokens: Option<i32>,

        /// Use streaming
        #[arg(short, long)]
        stream: bool,
    },

    /// Chat with messages
    Chat {
        /// Messages in format "role:content", e.g., "user:Hello"
        messages: Vec<String>,

        /// Use streaming
        #[arg(short, long)]
        stream: bool,
    },

    /// Session management commands
    #[command(subcommand)]
    Session(SessionCommands),

    /// Adapter management commands
    #[command(subcommand)]
    Adapter(AdapterCommands),
}

#[derive(Subcommand)]
enum SessionCommands {
    /// Create a new session
    Create,

    /// List all sessions
    List,

    /// Get session info
    Get {
        /// Session ID
        session_id: String,
    },

    /// Get session transcript
    Transcript {
        /// Session ID
        session_id: String,
    },

    /// Delete a session
    Delete {
        /// Session ID
        session_id: String,
    },

    /// Complete using a session
    Complete {
        /// Session ID
        session_id: String,

        /// Prompt to complete
        prompt: String,
    },
}

#[derive(Subcommand)]
enum AdapterCommands {
    /// Load adapter from file
    Load {
        /// File path to .mlpackage
        file_path: String,

        /// Optional adapter ID
        #[arg(short, long)]
        id: Option<String>,
    },

    /// Load adapter by name
    LoadName {
        /// Adapter name
        name: String,

        /// Optional adapter ID
        #[arg(short, long)]
        id: Option<String>,
    },

    /// List all loaded adapters
    List,

    /// Get adapter info
    Get {
        /// Adapter ID
        adapter_id: String,
    },

    /// Unload adapter
    Unload {
        /// Adapter ID
        adapter_id: String,
    },

    /// Recompile adapter
    Compile {
        /// Adapter ID
        adapter_id: String,
    },

    /// Get compatible adapter identifiers
    Compatible {
        /// Adapter name
        name: String,
    },

    /// Cleanup obsolete adapters
    Cleanup,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = FMClient::builder().base_url(&cli.url).build();

    match cli.command {
        Commands::Health => {
            let health = client.health().await?;
            println!("Status: {}", health.status);
            println!("Model Available: {}", health.model_available);
            println!("Version: {}", health.version);
            println!("Platform: {}", health.platform);
        }

        Commands::Models => {
            let models = client.models().await?;
            println!("Available models:");
            for model in models {
                println!("  - {}", model.id);
                println!("    Owner: {}", model.owned_by);
            }
        }

        Commands::Complete {
            prompt,
            temperature,
            max_tokens,
            stream,
        } => {
            let options = CompletionOptions {
                model: None,
                temperature,
                max_tokens,
                stream: Some(stream),
            };

            if stream {
                // Streaming mode
                let streaming_client = StreamingClient::new(client);
                let request = fm_bridge::CompletionRequest {
                    model: Some("apple-foundation-model".to_string()),
                    messages: vec![ChatMessage {
                        role: "user".to_string(),
                        content: prompt,
                    }],
                    temperature,
                    max_tokens,
                    stream: Some(true),
                    response_format: None,
                };

                let mut stream = streaming_client.stream(request).await?;

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => {
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    print!("{}", content);
                                    use std::io::{self, Write};
                                    io::stdout().flush()?;
                                }

                                if choice.finish_reason.is_some() {
                                    println!(); // New line at end
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            break;
                        }
                    }
                }
            } else {
                // Non-streaming mode
                let response = client.complete(prompt, Some(options)).await?;

                println!("Response:");
                println!("---");
                if let Some(choice) = response.choices.first() {
                    println!("{}", choice.message.content);
                }
                println!("---");

                if let Some(usage) = response.usage {
                    if let (Some(prompt_tokens), Some(completion_tokens), Some(total_tokens)) =
                        (usage.prompt_tokens, usage.completion_tokens, usage.total_tokens)
                    {
                        println!(
                            "Usage: {} prompt + {} completion = {} total tokens",
                            prompt_tokens, completion_tokens, total_tokens
                        );
                    }
                }
            }
        }

        Commands::Chat { messages, stream } => {
            let chat_messages: Vec<ChatMessage> = messages
                .iter()
                .filter_map(|msg| {
                    let parts: Vec<&str> = msg.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        Some(ChatMessage {
                            role: parts[0].to_string(),
                            content: parts[1].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            if chat_messages.is_empty() {
                eprintln!("No valid messages. Format: role:content");
                return Ok(());
            }

            if stream {
                // Streaming mode
                let streaming_client = StreamingClient::new(client);
                let request = fm_bridge::CompletionRequest {
                    model: Some("apple-foundation-model".to_string()),
                    messages: chat_messages,
                    temperature: None,
                    max_tokens: None,
                    stream: Some(true),
                    response_format: None,
                };

                let mut stream = streaming_client.stream(request).await?;

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => {
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    print!("{}", content);
                                    use std::io::{self, Write};
                                    io::stdout().flush()?;
                                }

                                if choice.finish_reason.is_some() {
                                    println!();
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            break;
                        }
                    }
                }
            } else {
                // Non-streaming mode
                let response = client.chat(chat_messages, None).await?;

                if let Some(choice) = response.choices.first() {
                    println!("{}", choice.message.content.as_ref().unwrap_or(&"".to_string()));
                }
            }
        }

        Commands::Session(session_cmd) => {
            let session_client = client.sessions();

            match session_cmd {
                SessionCommands::Create => {
                    let session = session_client.create_empty_session().await?;
                    println!("Created session:");
                    println!("  ID: {}", session.id);
                    println!("  Created: {}", session.created);
                }

                SessionCommands::List => {
                    let sessions = session_client.list_sessions().await?;
                    println!("Sessions ({}):", sessions.count);
                    for session in sessions.sessions {
                        println!("  - {}", session.id);
                        println!("    Created: {}", session.created);
                        println!("    Last used: {}", session.last_used);
                        println!("    Messages: {}", session.message_count);
                    }
                }

                SessionCommands::Get { session_id } => {
                    let session = session_client.get_session(&session_id).await?;
                    println!("Session {}:", session.id);
                    println!("  Created: {}", session.created);
                    println!("  Last used: {}", session.last_used);
                    println!("  Messages: {}", session.message_count);
                }

                SessionCommands::Transcript { session_id } => {
                    let transcript = session_client.get_transcript(&session_id).await?;
                    println!("Transcript for session {}:", transcript.session_id);
                    for msg in transcript.messages {
                        println!("  [{}]: {}", msg.role, msg.content);
                    }
                }

                SessionCommands::Delete { session_id } => {
                    let result = session_client.delete_session(&session_id).await?;
                    if result.deleted {
                        println!("Deleted session: {}", result.id);
                    } else {
                        println!("Failed to delete session: {}", result.id);
                    }
                }

                SessionCommands::Complete {
                    session_id,
                    prompt,
                } => {
                    let response = session_client
                        .complete_prompt_with_session(&session_id, prompt)
                        .await?;

                    if let Some(choice) = response.choices.first() {
                        println!("{}", choice.message.content.as_ref().unwrap_or(&"".to_string()));
                    }
                }
            }
        }

        Commands::Adapter(adapter_cmd) => {
            let adapter_client = client.adapters();

            match adapter_cmd {
                AdapterCommands::Load { file_path, id } => {
                    let result = adapter_client.load_from_file(&file_path, id).await?;
                    println!("Loaded adapter:");
                    println!("  ID: {}", result.id);
                    println!("  Name: {}", result.adapter.name.as_ref().unwrap_or(&"N/A".to_string()));
                    println!("  File: {}", result.adapter.file_url.as_ref().unwrap_or(&"N/A".to_string()));
                    println!("  Loaded at: {}", result.adapter.loaded_at);
                }

                AdapterCommands::LoadName { name, id } => {
                    let result = adapter_client.load_by_name(&name, id).await?;
                    println!("Loaded adapter:");
                    println!("  ID: {}", result.id);
                    println!("  Name: {}", result.adapter.name.as_ref().unwrap_or(&"N/A".to_string()));
                    println!("  Loaded at: {}", result.adapter.loaded_at);
                }

                AdapterCommands::List => {
                    let adapters = adapter_client.list_adapters().await?;
                    println!("Loaded adapters ({}):", adapters.count);
                    for adapter in adapters.adapters {
                        println!("  - {}", adapter.id);
                        if let Some(name) = adapter.name {
                            println!("    Name: {}", name);
                        }
                        if let Some(file) = adapter.file_url {
                            println!("    File: {}", file);
                        }
                        println!("    Loaded at: {}", adapter.loaded_at);
                        println!("    Last used: {}", adapter.last_used);
                    }
                }

                AdapterCommands::Get { adapter_id } => {
                    let adapter = adapter_client.get_adapter(&adapter_id).await?;
                    println!("Adapter {}:", adapter.id);
                    if let Some(name) = adapter.name {
                        println!("  Name: {}", name);
                    }
                    if let Some(file) = adapter.file_url {
                        println!("  File: {}", file);
                    }
                    println!("  Loaded at: {}", adapter.loaded_at);
                    println!("  Last used: {}", adapter.last_used);
                    if !adapter.metadata.is_empty() {
                        println!("  Metadata:");
                        for (key, value) in adapter.metadata {
                            println!("    {}: {:?}", key, value);
                        }
                    }
                }

                AdapterCommands::Unload { adapter_id } => {
                    let result = adapter_client.unload_adapter(&adapter_id).await?;
                    if result.unloaded {
                        println!("Unloaded adapter: {}", result.id);
                    } else {
                        println!("Failed to unload adapter: {}", result.id);
                    }
                }

                AdapterCommands::Compile { adapter_id } => {
                    let result = adapter_client.recompile_adapter(&adapter_id).await?;
                    if result.compiled {
                        println!("Recompiled adapter: {}", result.id);
                        println!("  Compiled at: {}", result.compiled_at);
                    }
                }

                AdapterCommands::Compatible { name } => {
                    let result = adapter_client.get_compatible_identifiers(&name).await?;
                    println!("Compatible adapters for '{}' ({}):", result.name, result.count);
                    for id in result.compatible_identifiers {
                        println!("  - {}", id);
                    }
                }

                AdapterCommands::Cleanup => {
                    let result = adapter_client.cleanup_obsolete_adapters().await?;
                    if result.cleaned {
                        println!("{}", result.message);
                    }
                }
            }
        }
    }

    Ok(())
}
