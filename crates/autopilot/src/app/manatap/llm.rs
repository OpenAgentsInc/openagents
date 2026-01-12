//! LLM initialization and configuration.

use crate::app::manatap::chain::{ChainEventSender, ChainedCallback, VisualizerCallback};
use anyhow::{Context, Result};
use dsrs::{ChatAdapter, LM};
use gpt_oss::{GptOssClient, LlamaServerManager};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for LLM initialization.
pub struct LlmConfig {
    /// Server URL (default: http://localhost:8000)
    pub server_url: String,
    /// Whether to auto-start the server if not running
    pub auto_start: bool,
    /// Timeout for server startup (seconds)
    pub startup_timeout: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:8000".to_string(),
            auto_start: true,
            startup_timeout: 60,
        }
    }
}

/// Result from LLM initialization.
pub struct LlmInitResult {
    /// The server manager (if auto-started, keeps server alive)
    pub server_manager: Option<LlamaServerManager>,
    /// Whether the server is ready
    pub server_ready: bool,
    /// Status message
    pub status_message: String,
}

/// Initialize the LLM backend.
///
/// This function:
/// 1. Checks if llama-server is already running
/// 2. Auto-starts it if not (and auto_start is enabled)
/// 3. Configures dsrs with the GPT-OSS backend and visualizer callback
pub async fn init_llm(
    config: LlmConfig,
    event_sender: ChainEventSender,
) -> Result<LlmInitResult> {
    let has_settings = dsrs::GLOBAL_SETTINGS.read().unwrap().is_some();
    if has_settings {
        let callback = Arc::new(VisualizerCallback::new(event_sender));
        let chained = ChainedCallback::new(dsrs::get_callback(), callback);
        dsrs::set_callback(chained);
        return Ok(LlmInitResult {
            server_manager: None,
            server_ready: true,
            status_message: "Using existing DSPy configuration".to_string(),
        });
    }

    let client = GptOssClient::with_base_url(&config.server_url)?;

    // Check if server is already running
    let server_available = client.health().await.unwrap_or(false);

    let mut server_manager = None;
    let server_ready;
    let status_message;

    if server_available {
        server_ready = true;
        status_message = format!("Connected to existing server at {}", config.server_url);
        eprintln!("[manatap] {}", status_message);
    } else if config.auto_start {
        // Try to auto-start the server
        eprintln!("[manatap] Server not running, attempting auto-start...");

        // Check if binary is available
        if !LlamaServerManager::is_available() {
            return Ok(LlmInitResult {
                server_manager: None,
                server_ready: false,
                status_message: "llama-server binary not found. Please install llama.cpp or set LLAMA_MODEL_PATH.".to_string(),
            });
        }

        // Check if model is available
        let model_path = match LlamaServerManager::discover_model() {
            Some(path) => path,
            None => {
                return Ok(LlmInitResult {
                    server_manager: None,
                    server_ready: false,
                    status_message: "No GGUF model found. Set LLAMA_MODEL_PATH or place model in ~/models/gpt-oss/".to_string(),
                });
            }
        };

        eprintln!("[manatap] Using model: {}", model_path.display());

        // Start the server
        let mut manager = LlamaServerManager::new()
            .with_port(8000)
            .with_model(model_path);

        match manager.start() {
            Ok(_) => {
                eprintln!("[manatap] Server started, waiting for readiness...");

                // Wait for server to be ready
                match manager
                    .wait_ready_timeout(Duration::from_secs(config.startup_timeout))
                    .await
                {
                    Ok(_) => {
                        server_ready = true;
                        status_message = "Server auto-started successfully".to_string();
                        server_manager = Some(manager);
                        eprintln!("[manatap] Server ready!");
                    }
                    Err(e) => {
                        server_ready = false;
                        status_message = format!("Server startup timeout: {}", e);
                        eprintln!("[manatap] {}", status_message);
                    }
                }
            }
            Err(e) => {
                server_ready = false;
                status_message = format!("Failed to start server: {}", e);
                eprintln!("[manatap] {}", status_message);
            }
        }
    } else {
        server_ready = false;
        status_message = format!(
            "Server not available at {} and auto-start is disabled",
            config.server_url
        );
        eprintln!("[manatap] {}", status_message);
    }

    // Configure dsrs if server is ready
    if server_ready {
        let lm = LM::builder()
            .model("gptoss:gpt-oss-20b".to_string())
            .temperature(0.3)
            .max_tokens(2048)
            .build()
            .await
            .context("Failed to build LM")?;

        // Create and register the callback
        let callback = VisualizerCallback::new(event_sender);
        dsrs::configure_with_callback(lm, ChatAdapter, callback);

        eprintln!("[manatap] dsrs configured with GPT-OSS backend");
    }

    Ok(LlmInitResult {
        server_manager,
        server_ready,
        status_message,
    })
}
