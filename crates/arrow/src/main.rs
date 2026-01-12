//! Arrow - Happy path testing for Autopilot
//!
//! This binary provides quick environment detection and basic inference tests
//! to verify that Autopilot can run correctly.

use clap::Parser;
use dsrs::pipelines::CodeChangePipeline;
use dsrs::{ChatAdapter, LM};
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

    /// Run DSPy code change chain test
    #[arg(long)]
    test_dspy_chain: bool,
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
        json_schema: None,
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

async fn test_dspy_code_change_chain(server_url: &str) -> anyhow::Result<()> {
    info!("\n=== DSPy Code Change Chain Test ===\n");

    // Configure dsrs with GPT-OSS backend via local OpenAI-compatible endpoint
    info!("Configuring dsrs with GPT-OSS backend...");
    let lm = LM::builder()
        .base_url(server_url.to_string())
        .model("gpt-oss-20b".to_string())
        .temperature(0.3)
        .max_tokens(2048)
        .build()
        .await?;

    dsrs::configure(lm, ChatAdapter);
    info!("  dsrs configured with GPT-OSS LM");

    // Create the pipeline
    let pipeline = CodeChangePipeline::new();

    // Sample task: add a --version flag
    let user_task = "Add a --version flag that prints the version from Cargo.toml";
    let repo_context = r#"
This is a Rust CLI tool called 'arrow' that tests Autopilot functionality.
It uses clap for argument parsing and has a main.rs entry point.
"#;
    let repo_structure = r#"
crates/arrow/
  Cargo.toml
  src/
    main.rs
"#;

    info!("Task: {}", user_task);

    // Stage 1: Task Understanding
    info!("\n[Stage 1] Understanding task...");
    match pipeline.understand_task(user_task, repo_context).await {
        Ok(task) => {
            info!("  Task type: {:?}", task.task_type);
            info!("  Scope: {:?}", task.scope);
            info!("  Requirements:");
            for req in &task.requirements {
                info!("    - {}", req);
            }
            if !task.clarifying_questions.is_empty() {
                info!("  Questions:");
                for q in &task.clarifying_questions {
                    info!("    ? {}", q);
                }
            }
            info!("  Confidence: {:.1}%", task.confidence * 100.0);

            // Stage 2: Code Exploration
            info!("\n[Stage 2] Exploring code...");
            match pipeline.explore_code(&task.requirements, repo_structure).await {
                Ok(exploration) => {
                    info!("  Queries: {:?}", exploration.queries);
                    info!("  Lanes: {:?}", exploration.lanes);
                    info!("  Rationale: {}", exploration.rationale);

                    // Stage 3 & 4: Generate sample edit
                    info!("\n[Stage 3-4] Generating code edit...");
                    let sample_content = r#"
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();
    println!("Running...");
}
"#;
                    match pipeline
                        .generate_edit(
                            "src/main.rs",
                            sample_content,
                            &task.requirements.join("\n"),
                            "",
                        )
                        .await
                    {
                        Ok(edit) => {
                            info!("  File: {}", edit.file_path);
                            info!("  Summary: {}", edit.summary);
                            info!("  Affected lines: {}", edit.affected_lines);
                            info!("  Confidence: {:.1}%", edit.confidence * 100.0);
                            info!("  Diff:");
                            for line in edit.unified_diff.lines().take(20) {
                                info!("    {}", line);
                            }

                            // Stage 5: Verification
                            info!("\n[Stage 5] Verifying changes...");
                            match pipeline
                                .verify_changes(user_task, &edit.summary, "(no tests run)")
                                .await
                            {
                                Ok(verification) => {
                                    info!("  Status: {:?}", verification.status);
                                    if !verification.missing_requirements.is_empty() {
                                        info!("  Missing:");
                                        for m in &verification.missing_requirements {
                                            info!("    - {}", m);
                                        }
                                    }
                                    if !verification.issues_found.is_empty() {
                                        info!("  Issues:");
                                        for i in &verification.issues_found {
                                            info!("    - {}", i);
                                        }
                                    }
                                    info!("  Confidence: {:.1}%", verification.confidence * 100.0);
                                }
                                Err(e) => {
                                    error!("  Verification failed: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("  Edit generation failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("  Exploration failed: {}", e);
                }
            }
        }
        Err(e) => {
            error!("  Task understanding failed: {}", e);
        }
    }

    info!("\n=== DSPy Chain Test Complete ===");
    Ok(())
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

        // Run DSPy chain test if requested
        if args.test_dspy_chain {
            test_dspy_code_change_chain(&args.server_url).await?;
        } else {
            test_inference(&client).await?;
        }
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
