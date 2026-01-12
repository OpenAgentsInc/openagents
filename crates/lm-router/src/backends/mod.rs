//! LM backend implementations.

pub mod fm_bridge;
pub mod mock;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod swarm_sim;

pub use fm_bridge::FmBridgeBackend;
pub use mock::MockBackend;
pub use ollama::OllamaBackend;
pub use openai::OpenAiBackend;
pub use openrouter::OpenRouterBackend;
pub use swarm_sim::SwarmSimulator;

use crate::backend::LmBackend;
use crate::error::{Error, Result};
use crate::router::LmRouter;
use tracing::{info, warn};

/// Result of auto-detecting available LM backends.
#[derive(Debug, Clone)]
pub struct AutoRouter {
    /// Router configured with detected backends.
    pub router: LmRouter,
    /// Default model suggested by detection.
    pub default_model: Option<String>,
    /// Names of detected backends.
    pub backends: Vec<String>,
}

impl AutoRouter {
    /// Return true if at least one backend was detected.
    pub fn has_backends(&self) -> bool {
        !self.backends.is_empty()
    }
}

/// Auto-detect available backends using environment and local probes.
pub async fn auto_detect_router() -> Result<AutoRouter> {
    let mut builder = LmRouter::builder();
    let mut backends = Vec::new();
    let mut default_model = None;

    // Ollama (localhost:11434)
    let mut ollama = OllamaBackend::new();
    if ollama.is_available().await {
        if ollama.detect_models().await.is_ok() && default_model.is_none() {
            default_model = ollama.supported_models().first().cloned();
        }
        builder = builder.add_backend(ollama);
        backends.push("ollama".to_string());
    } else {
        info!("Ollama not detected at localhost:11434");
    }

    // FM Bridge (Apple Foundation Models)
    match FmBridgeBackend::new() {
        Ok(fm) => {
            if fm.health_check().await {
                builder = builder.add_backend(fm);
                backends.push("fm-bridge".to_string());
                if default_model.is_none() {
                    default_model = Some("apple-fm".to_string());
                }
            } else {
                info!("FM Bridge detected but not healthy");
            }
        }
        Err(err) => {
            info!("FM Bridge unavailable: {}", err);
        }
    }

    // OpenAI
    if std::env::var("OPENAI_API_KEY").is_ok() {
        match OpenAiBackend::new() {
            Ok(openai) => {
                builder = builder.add_backend(openai);
                backends.push("openai".to_string());
                if default_model.is_none() {
                    default_model = Some("gpt-4o-mini".to_string());
                }
            }
            Err(err) => warn!("OpenAI backend unavailable: {}", err),
        }
    }

    // OpenRouter
    if std::env::var("OPENROUTER_API_KEY").is_ok() {
        match OpenRouterBackend::new() {
            Ok(openrouter) => {
                builder = builder.add_backend(openrouter);
                backends.push("openrouter".to_string());
                if default_model.is_none() {
                    default_model = Some("openai/gpt-4o-mini".to_string());
                }
            }
            Err(err) => warn!("OpenRouter backend unavailable: {}", err),
        }
    }

    if backends.is_empty() {
        return Err(Error::BackendNotFound(
            "auto-detect found no backends".to_string(),
        ));
    }

    let router = builder.build();
    if default_model.is_none() {
        default_model = router.available_models().first().cloned();
    }

    Ok(AutoRouter {
        router,
        default_model,
        backends,
    })
}
