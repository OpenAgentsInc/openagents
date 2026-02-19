//! pylon gateway - Interact with external AI gateways (Cerebras, etc.)

use clap::{Args, Subcommand};
use gateway::{CerebrasGateway, ChatRequest, InferenceGateway, Message};
use std::io::{self, Write};

/// Arguments for the gateway command
#[derive(Args)]
pub struct GatewayArgs {
    #[command(subcommand)]
    pub command: GatewayCommands,
}

/// Gateway subcommands
#[derive(Subcommand)]
pub enum GatewayCommands {
    /// Send a chat message to a gateway
    Chat(ChatArgs),
    /// List available models
    Models(ModelsArgs),
    /// Check gateway health
    Health(HealthArgs),
}

/// Arguments for chat command
#[derive(Args)]
pub struct ChatArgs {
    /// Message to send
    pub message: String,

    /// Model to use (default: zai-glm-4.7)
    #[arg(short, long, default_value = "zai-glm-4.7")]
    pub model: String,

    /// System prompt
    #[arg(short, long)]
    pub system: Option<String>,

    /// Temperature (0.0 - 2.0)
    #[arg(short, long)]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[arg(long)]
    pub max_tokens: Option<u32>,

    /// Gateway provider (default: cerebras)
    #[arg(short, long, default_value = "cerebras")]
    pub provider: String,
}

/// Arguments for models command
#[derive(Args)]
pub struct ModelsArgs {
    /// Gateway provider (default: cerebras)
    #[arg(short, long, default_value = "cerebras")]
    pub provider: String,
}

/// Arguments for health command
#[derive(Args)]
pub struct HealthArgs {
    /// Gateway provider (default: cerebras)
    #[arg(short, long, default_value = "cerebras")]
    pub provider: String,
}

/// Load environment from .env.local if it exists
fn load_env() {
    // Try .env.local first, then .env
    if dotenvy::from_filename(".env.local").is_err() {
        let _ = dotenvy::dotenv();
    }
}

/// Create a gateway based on provider name
fn create_gateway(provider: &str) -> anyhow::Result<Box<dyn InferenceGateway>> {
    load_env();

    match provider {
        "cerebras" => {
            let gw = CerebrasGateway::from_env()
                .map_err(|e| anyhow::anyhow!("Failed to create Cerebras gateway: {}", e))?;
            Ok(Box::new(gw))
        }
        _ => anyhow::bail!("Unknown provider: {}. Supported: cerebras", provider),
    }
}

/// Run the gateway command
pub async fn run(args: GatewayArgs) -> anyhow::Result<()> {
    match args.command {
        GatewayCommands::Chat(chat_args) => run_chat(chat_args).await,
        GatewayCommands::Models(models_args) => run_models(models_args).await,
        GatewayCommands::Health(health_args) => run_health(health_args).await,
    }
}

/// Run the chat command
async fn run_chat(args: ChatArgs) -> anyhow::Result<()> {
    let gateway = create_gateway(&args.provider)?;

    eprintln!(
        "[gateway] Using provider={} model={}",
        args.provider, args.model
    );

    let mut messages = Vec::new();

    // Add system message if provided
    if let Some(system) = args.system {
        messages.push(Message::system(system));
    }

    // Add user message
    messages.push(Message::user(&args.message));

    // Build request
    let mut request = ChatRequest::new(&args.model, messages);

    if let Some(temp) = args.temperature {
        request = request.with_temperature(temp);
    }

    if let Some(max_tokens) = args.max_tokens {
        request = request.with_max_tokens(max_tokens);
    }

    // Send request
    let response = gateway
        .chat(request)
        .await
        .map_err(|e| anyhow::anyhow!("Chat request failed: {}", e))?;

    // Print response
    if let Some(content) = response.content() {
        println!("{}", content);
    }

    // Print usage info
    eprintln!(
        "\n[gateway] tokens: prompt={} completion={} total={}",
        response.usage.prompt_tokens, response.usage.completion_tokens, response.usage.total_tokens
    );

    Ok(())
}

/// Run the models command
async fn run_models(args: ModelsArgs) -> anyhow::Result<()> {
    let gateway = create_gateway(&args.provider)?;

    let models = gateway
        .models()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list models: {}", e))?;

    println!("Available models for {}:", args.provider);
    println!();

    for model in models {
        println!("  {} ({})", model.id, model.name);
        println!("    Context: {} tokens", model.context_length);
        if let Some(pricing) = &model.pricing {
            println!(
                "    Pricing: ${:.2}/M input, ${:.2}/M output",
                pricing.input_per_million, pricing.output_per_million
            );
        }
        println!("    Capabilities: {:?}", model.capabilities);
        println!();
    }

    Ok(())
}

/// Run the health command
async fn run_health(args: HealthArgs) -> anyhow::Result<()> {
    let gateway = create_gateway(&args.provider)?;

    eprint!("[gateway] Checking {} health... ", args.provider);
    io::stderr().flush()?;

    let health = gateway.health().await;

    if health.available {
        eprintln!("OK");
        println!("Gateway: {} ({})", gateway.name(), gateway.provider());
        println!("Status: Available");
        if let Some(latency) = health.latency_ms {
            println!("Latency: {}ms", latency);
        }
    } else {
        eprintln!("FAILED");
        println!("Gateway: {} ({})", gateway.name(), gateway.provider());
        println!("Status: Unavailable");
        if let Some(error) = health.error {
            println!("Error: {}", error);
        }
    }

    Ok(())
}
