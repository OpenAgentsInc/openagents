//! Arrow - Happy path testing for Autopilot
//!
//! This binary provides quick environment detection and basic inference tests
//! to verify that Autopilot can run correctly.

use clap::Parser;
use gpt_oss::{GptOssClient, LlamaServerManager};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    verbose: bool,

    #[arg(long, default_value = "http://localhost:8000")]
    server_url: String,

    #[arg(long)]
    skip_inference: bool,

    /// Disable auto-starting llama-server if not running
    #[arg(long)]
    no_auto_start: bool,

    /// Path to model file (overrides LLAMA_MODEL_PATH and auto-discovery)
    #[arg(long, env = "LLAMA_MODEL_PATH")]
    model_path: Option<PathBuf>,

    /// Timeout for server startup health check (seconds)
    #[arg(long, default_value = "30")]
    startup_timeout: u64,
}

async fn check_gptoss_server(url: &str) -> anyhow::Result<bool> {
    let client = GptOssClient::with_base_url(url)?;

    match client.health().await {
        Ok(healthy) => {
            if healthy {
                info!("  GPT-OSS Server: ✓ (healthy)");
                Ok(true)
            } else {
                warn!("  GPT-OSS Server: ✗ (unhealthy)");
                Ok(false)
            }
        }
        Err(e) => {
            warn!("  GPT-OSS Server: ✗ (unreachable: {})", e);
            Ok(false)
        }
    }
}

async fn test_inference(client: &GptOssClient) -> anyhow::Result<()> {
    use gpt_oss::{GptOssRequest, HarmonyRenderer, HarmonyRole, HarmonyTurn};
    use std::io::Write;
    use tokio_stream::StreamExt;

    info!("\nTesting Inference:");

    let model = "gpt-oss-20b";
    let user_message = "What is Rust? Answer in one sentence.";

    info!("  Model: {}", model);
    info!("  Prompt: {}", user_message);

    // Use Harmony to render the prompt with proper GPT-OSS formatting
    let renderer = HarmonyRenderer::gpt_oss()?;
    let turns = vec![HarmonyTurn::new(HarmonyRole::User, user_message)];
    let prompt = renderer.render_prompt(&turns, &[])?;

    print!("  Response: ");
    std::io::stdout().flush()?;

    let request = GptOssRequest {
        model: model.to_string(),
        prompt,
        max_tokens: Some(200),
        temperature: None,
        top_p: None,
        stop: None, // Harmony handles stop tokens
        stream: true,
    };

    let mut full_response = String::new();

    match client.stream(request).await {
        Ok(mut stream) => {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(c) => {
                        let text = c.delta();
                        if !text.is_empty() {
                            full_response.push_str(text);
                            print!("{}", text);
                            std::io::stdout().flush()?;
                        }
                    }
                    Err(e) => {
                        error!("\n  ✗ Stream error: {}", e);
                        return Err(e.into());
                    }
                }
            }

            // Parse the response to extract clean assistant text
            if let Ok(clean_text) = renderer.extract_assistant_text(&full_response) {
                println!("\n  Parsed: {}", clean_text);
            }

            info!("  ✓ Inference successful");
            Ok(())
        }
        Err(e) => {
            error!("\n  ✗ Inference failed: {}", e);
            Err(e.into())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    info!("=== Arrow - Autopilot Happy Path Test ===\n");

    info!("Environment:");
    info!("  OS: {}", std::env::consts::OS);
    info!("  Arch: {}", std::env::consts::ARCH);
    info!("  Family: {}", std::env::consts::FAMILY);

    info!("\nEnvironment Variables:");
    if std::env::var("OPENAI_API_KEY").is_ok() {
        info!("  OPENAI_API_KEY: ✓ (present)");
    } else {
        warn!("  OPENAI_API_KEY: ✗ (not set)");
    }

    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        info!("  CEREBRAS_API_KEY: ✓ (present)");
    } else {
        warn!("  CEREBRAS_API_KEY: ✗ (not set)");
    }

    info!("\nBackend Detection:");
    let mut server_available = check_gptoss_server(&args.server_url).await?;

    // Track server manager for auto-started server (keeps it alive until main exits)
    let mut _server_manager: Option<LlamaServerManager> = None;

    // Auto-start logic
    if !server_available && !args.no_auto_start {
        info!("\nServer not running, attempting auto-start...");

        // Check binary availability
        if !LlamaServerManager::is_available() {
            warn!("  llama-server binary not found in PATH");
            warn!("  Install llama.cpp or add llama-server to PATH");
        } else {
            // Show discovered binary
            if let Some(binary) = LlamaServerManager::find_binary() {
                info!("  Binary: {:?}", binary);
            }

            // Discover or use provided model
            let model_path = args
                .model_path
                .clone()
                .or_else(LlamaServerManager::discover_model);

            match model_path {
                Some(path) => {
                    info!("  Model: {:?}", path);

                    let mut manager = LlamaServerManager::new().with_model(path);

                    match manager.start() {
                        Ok(()) => {
                            let timeout = Duration::from_secs(args.startup_timeout);
                            match manager.wait_ready_timeout(timeout).await {
                                Ok(()) => {
                                    info!("  llama-server started successfully");
                                    server_available = true;
                                    _server_manager = Some(manager);
                                }
                                Err(e) => {
                                    error!("  Failed to start server: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("  Failed to spawn server: {}", e);
                        }
                    }
                }
                None => {
                    warn!("  No model files found");
                    warn!("  Set LLAMA_MODEL_PATH or place .gguf files in ~/models/gpt-oss/");
                }
            }
        }
    }

    if server_available && !args.skip_inference {
        let client = GptOssClient::with_base_url(&args.server_url)?;

        match client.models().await {
            Ok(models) => {
                info!("  Available models:");
                for model in models {
                    info!("    - {}", model.display_name());
                }
            }
            Err(e) => {
                warn!("  Failed to list models: {}", e);
            }
        }

        test_inference(&client).await?;
    } else if !server_available {
        info!("\nTo enable GPT-OSS mode:");
        info!("  1. Install llama.cpp and ensure llama-server is in PATH");
        info!("  2. Place a .gguf model in ~/models/gpt-oss/ or set LLAMA_MODEL_PATH");
        info!("  3. Re-run: cargo arrow");
        info!("\nAlternatively, start manually:");
        info!("  llama-server --model <model.gguf> --port 8000 --chat-template chatml");
    }

    info!("\n=== Test Complete ===");
    Ok(())
}
