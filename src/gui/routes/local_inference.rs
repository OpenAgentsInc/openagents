//! Local inference configuration routes
//!
//! Provides endpoints for configuring local model backends (GPT-OSS/llama.cpp).

use actix_web::{web, HttpResponse};
use anyhow;
use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use toml;

use crate::gui::state::AppState;

/// Configure local inference API routes
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get_config))
        .route("", web::post().to(save_config))
        .route("/status", web::get().to(get_status))
        .route("/models", web::get().to(list_models));
}

/// Configure local inference page routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(settings_page));
}

/// Local inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalInferenceConfig {
    /// Base URL for the inference server (e.g., http://localhost:8080)
    pub server_url: String,
    /// Default model to use (e.g., gpt-oss-20b)
    pub default_model: String,
    /// Reasoning effort level (low, medium, high)
    pub reasoning_effort: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Whether to enable streaming responses
    pub streaming_enabled: bool,
}

impl Default for LocalInferenceConfig {
    fn default() -> Self {
        Self {
            server_url: std::env::var("GPT_OSS_URL")
                .or_else(|_| std::env::var("LLAMACPP_URL"))
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            default_model: std::env::var("GPT_OSS_MODEL")
                .unwrap_or_else(|_| "gpt-oss-20b".to_string()),
            reasoning_effort: std::env::var("GPT_OSS_REASONING_EFFORT")
                .unwrap_or_else(|_| "medium".to_string()),
            timeout_seconds: 120,
            streaming_enabled: true,
        }
    }
}

impl LocalInferenceConfig {
    /// Get the default config file path
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(&home)
            .join(".openagents")
            .join("local-inference.toml")
    }

    /// Load configuration from file, or return default if file doesn't exist
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match Self::load_from_file(&path) {
                Ok(config) => return config,
                Err(e) => {
                    eprintln!("Warning: Failed to load local inference config: {}", e);
                }
            }
        }
        Self::default()
    }

    /// Load configuration from a TOML file
    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: LocalInferenceConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to the default config file
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        self.save_to_file(&path)
    }

    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Server status response
#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub available: bool,
    pub server_url: String,
    pub loaded_model: Option<String>,
    pub error: Option<String>,
}

/// Model info response
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size: Option<String>,
    pub loaded: bool,
}

/// Get current configuration
async fn get_config(_state: web::Data<AppState>) -> HttpResponse {
    let config = LocalInferenceConfig::load();
    HttpResponse::Ok().json(config)
}

/// Save configuration
async fn save_config(
    _state: web::Data<AppState>,
    body: web::Json<LocalInferenceConfig>,
) -> HttpResponse {
    let config = body.into_inner();

    // Validate URL format
    if !config.server_url.starts_with("http://") && !config.server_url.starts_with("https://") {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid server URL - must start with http:// or https://"
        }));
    }

    // Validate reasoning effort
    if !["low", "medium", "high"].contains(&config.reasoning_effort.as_str()) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid reasoning effort - must be low, medium, or high"
        }));
    }

    // Persist configuration to file
    match config.save() {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": format!("Configuration saved to {}", LocalInferenceConfig::config_path().display()),
                "config": config
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to save configuration: {}", e)
            }))
        }
    }
}

/// Get server status
async fn get_status(_state: web::Data<AppState>) -> HttpResponse {
    let config = LocalInferenceConfig::load();
    let status = check_server_status(&config.server_url).await;
    HttpResponse::Ok().json(status)
}

/// List available models
async fn list_models(_state: web::Data<AppState>) -> HttpResponse {
    let config = LocalInferenceConfig::load();
    let models = fetch_available_models(&config.server_url).await;
    HttpResponse::Ok().json(models)
}

/// Check if the local inference server is available
async fn check_server_status(server_url: &str) -> ServerStatus {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ServerStatus {
                available: false,
                server_url: server_url.to_string(),
                loaded_model: None,
                error: Some(format!("Failed to create HTTP client: {}", e)),
            };
        }
    };

    // Try the health endpoint first
    match client.get(format!("{}/health", server_url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            // Try to get model info from /props or /v1/models
            let loaded_model = get_loaded_model(&client, server_url).await;

            ServerStatus {
                available: true,
                server_url: server_url.to_string(),
                loaded_model,
                error: None,
            }
        }
        Ok(resp) => ServerStatus {
            available: false,
            server_url: server_url.to_string(),
            loaded_model: None,
            error: Some(format!("Server returned status: {}", resp.status())),
        },
        Err(e) => ServerStatus {
            available: false,
            server_url: server_url.to_string(),
            loaded_model: None,
            error: Some(format!("Connection failed: {}", e)),
        },
    }
}

/// Get the currently loaded model from the server
async fn get_loaded_model(client: &reqwest::Client, server_url: &str) -> Option<String> {
    // Try /props endpoint (llama.cpp)
    if let Ok(resp) = client.get(format!("{}/props", server_url)).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(model) = json.get("default_generation_settings")
                .and_then(|s| s.get("model"))
                .and_then(|m| m.as_str())
            {
                return Some(model.to_string());
            }
        }
    }

    // Try /v1/models endpoint (OpenAI-compatible)
    if let Ok(resp) = client.get(format!("{}/v1/models", server_url)).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(models) = json.get("data").and_then(|d| d.as_array()) {
                if let Some(first) = models.first() {
                    if let Some(id) = first.get("id").and_then(|i| i.as_str()) {
                        return Some(id.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Fetch available models from the server
async fn fetch_available_models(server_url: &str) -> Vec<ModelInfo> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Try /v1/models endpoint
    if let Ok(resp) = client.get(format!("{}/v1/models", server_url)).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                return data
                    .iter()
                    .filter_map(|m| {
                        let id = m.get("id").and_then(|i| i.as_str())?;
                        Some(ModelInfo {
                            id: id.to_string(),
                            name: id.to_string(),
                            size: None,
                            loaded: true,
                        })
                    })
                    .collect();
            }
        }
    }

    // Return common model options if server doesn't respond
    vec![
        ModelInfo {
            id: "gpt-oss-20b".to_string(),
            name: "GPT-OSS 20B".to_string(),
            size: Some("21B params (3.6B active)".to_string()),
            loaded: false,
        },
        ModelInfo {
            id: "gpt-oss-120b".to_string(),
            name: "GPT-OSS 120B".to_string(),
            size: Some("117B params (5.1B active)".to_string()),
            loaded: false,
        },
    ]
}

/// Settings page for local inference configuration
async fn settings_page(_state: web::Data<AppState>) -> HttpResponse {
    let config = LocalInferenceConfig::load();
    let status = check_server_status(&config.server_url).await;
    let models = fetch_available_models(&config.server_url).await;

    let content = render_settings_page(&config, &status, &models);
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

/// Render the settings page HTML
fn render_settings_page(
    config: &LocalInferenceConfig,
    status: &ServerStatus,
    models: &[ModelInfo],
) -> Markup {
    let status_color = if status.available { "#22c55e" } else { "#ef4444" };
    let status_text = if status.available { "Connected" } else { "Disconnected" };

    html! {
        div style="padding: 24px; max-width: 800px; margin: 0 auto;" {
            // Header
            div style="display: flex; align-items: center; gap: 12px; margin-bottom: 24px;" {
                h1 style="margin: 0; font-size: 24px; font-weight: 600;" {
                    "Local Inference Settings"
                }
                span style={"padding: 4px 12px; font-size: 12px; font-weight: 500; color: white; background: " (status_color) ";"} {
                    (status_text)
                }
            }

            // Status Card
            div style="background: #1a1a2e; padding: 20px; margin-bottom: 24px;" {
                h2 style="margin: 0 0 16px 0; font-size: 16px; font-weight: 600; color: #888;" {
                    "Server Status"
                }
                div style="display: grid; grid-template-columns: 1fr 1fr; gap: 16px;" {
                    div {
                        div style="font-size: 12px; color: #666; margin-bottom: 4px;" { "URL" }
                        div style="font-family: monospace; font-size: 14px;" { (status.server_url) }
                    }
                    div {
                        div style="font-size: 12px; color: #666; margin-bottom: 4px;" { "Loaded Model" }
                        div style="font-size: 14px;" {
                            @if let Some(model) = &status.loaded_model {
                                (model)
                            } @else {
                                span style="color: #666;" { "None" }
                            }
                        }
                    }
                }
                @if let Some(error) = &status.error {
                    div style="margin-top: 12px; padding: 12px; background: #2a1a1a; color: #ef4444; font-size: 13px;" {
                        (error)
                    }
                }
            }

            // Configuration Form
            form
                hx-post="/api/local-inference"
                hx-target="#save-result"
                hx-swap="innerHTML"
                style="background: #1a1a2e; padding: 20px; margin-bottom: 24px;"
            {
                h2 style="margin: 0 0 20px 0; font-size: 16px; font-weight: 600; color: #888;" {
                    "Configuration"
                }

                // Server URL
                div style="margin-bottom: 20px;" {
                    label style="display: block; font-size: 13px; color: #888; margin-bottom: 6px;" for="server_url" {
                        "Server URL"
                    }
                    input
                        type="text"
                        name="server_url"
                        id="server_url"
                        value=(config.server_url)
                        placeholder="http://localhost:8080"
                        style="width: 100%; padding: 10px 12px; background: #0d0d1a; border: 1px solid #333; color: white; font-family: monospace; font-size: 14px; box-sizing: border-box;";
                    div style="font-size: 11px; color: #666; margin-top: 4px;" {
                        "The URL of your llama-server or GPT-OSS Responses API server"
                    }
                }

                // Default Model
                div style="margin-bottom: 20px;" {
                    label style="display: block; font-size: 13px; color: #888; margin-bottom: 6px;" for="default_model" {
                        "Default Model"
                    }
                    select
                        name="default_model"
                        id="default_model"
                        style="width: 100%; padding: 10px 12px; background: #0d0d1a; border: 1px solid #333; color: white; font-size: 14px;"
                    {
                        @for model in models {
                            option value=(model.id) selected[model.id == config.default_model] {
                                (model.name)
                                @if let Some(size) = &model.size {
                                    " (" (size) ")"
                                }
                            }
                        }
                    }
                }

                // Reasoning Effort
                div style="margin-bottom: 20px;" {
                    label style="display: block; font-size: 13px; color: #888; margin-bottom: 6px;" for="reasoning_effort" {
                        "Reasoning Effort"
                    }
                    div style="display: flex; gap: 12px;" {
                        @for effort in ["low", "medium", "high"] {
                            label style="display: flex; align-items: center; gap: 6px; cursor: pointer;" {
                                input
                                    type="radio"
                                    name="reasoning_effort"
                                    value=(effort)
                                    checked[*effort == config.reasoning_effort]
                                    style="accent-color: #3b82f6;";
                                span style="font-size: 14px; text-transform: capitalize;" { (effort) }
                            }
                        }
                    }
                    div style="font-size: 11px; color: #666; margin-top: 4px;" {
                        "Higher effort = more tokens and longer thinking time"
                    }
                }

                // Timeout
                div style="margin-bottom: 20px;" {
                    label style="display: block; font-size: 13px; color: #888; margin-bottom: 6px;" for="timeout_seconds" {
                        "Request Timeout (seconds)"
                    }
                    input
                        type="number"
                        name="timeout_seconds"
                        id="timeout_seconds"
                        value=(config.timeout_seconds)
                        min="10"
                        max="600"
                        style="width: 120px; padding: 10px 12px; background: #0d0d1a; border: 1px solid #333; color: white; font-size: 14px;";
                }

                // Streaming
                div style="margin-bottom: 20px;" {
                    label style="display: flex; align-items: center; gap: 8px; cursor: pointer;" {
                        input
                            type="checkbox"
                            name="streaming_enabled"
                            checked[config.streaming_enabled]
                            style="accent-color: #3b82f6; width: 18px; height: 18px;";
                        span style="font-size: 14px;" { "Enable streaming responses" }
                    }
                }

                // Save Button
                div style="display: flex; gap: 12px; align-items: center;" {
                    button
                        type="submit"
                        style="padding: 10px 24px; background: #3b82f6; color: white; border: none; font-weight: 500; cursor: pointer;"
                    {
                        "Save Configuration"
                    }
                    button
                        type="button"
                        hx-get="/api/local-inference/status"
                        hx-target="#status-check"
                        hx-swap="innerHTML"
                        style="padding: 10px 24px; background: transparent; color: #888; border: 1px solid #333; cursor: pointer;"
                    {
                        "Test Connection"
                    }
                    div id="save-result" {}
                    div id="status-check" {}
                }
            }

            // Help Section
            div style="background: #1a1a2e; padding: 20px;" {
                h2 style="margin: 0 0 16px 0; font-size: 16px; font-weight: 600; color: #888;" {
                    "Getting Started"
                }
                div style="font-size: 14px; line-height: 1.6; color: #ccc;" {
                    p style="margin: 0 0 12px 0;" {
                        "To use local inference, you need a running llama-server with a compatible model loaded."
                    }
                    div style="background: #0d0d1a; padding: 16px; font-family: monospace; font-size: 13px; margin-bottom: 12px; overflow-x: auto;" {
                        (PreEscaped("# Start llama-server with GPT-OSS<br>"))
                        (PreEscaped("llama-server \\<br>"))
                        (PreEscaped("  --model gpt-oss-20b-Q4_K_M.gguf \\<br>"))
                        (PreEscaped("  --port 8080 \\<br>"))
                        (PreEscaped("  --ctx-size 8192"))
                    }
                    p style="margin: 0; color: #888;" {
                        "Environment variables: "
                        code style="background: #0d0d1a; padding: 2px 6px;" { "GPT_OSS_URL" }
                        ", "
                        code style="background: #0d0d1a; padding: 2px 6px;" { "GPT_OSS_MODEL" }
                        ", "
                        code style="background: #0d0d1a; padding: 2px 6px;" { "GPT_OSS_REASONING_EFFORT" }
                    }
                }
            }
        }
    }
}
