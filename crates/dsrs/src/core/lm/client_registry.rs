use anyhow::Result;
use enum_dispatch::enum_dispatch;
use reqwest;
use rig::{
    client::Nothing,
    completion::{CompletionError, CompletionRequest, CompletionResponse},
    providers::*,
};
use std::borrow::Cow;
use std::sync::Arc;

use super::claude_sdk::{self, ClaudeSdkModel};
use super::lm_router::LmRouterLM;
use super::pylon::{PylonCompletionModel, PylonConfig};

#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait CompletionProvider {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError>;
}

#[enum_dispatch(CompletionProvider)]
#[derive(Clone)]
pub enum LMClient {
    OpenAI(openai::completion::CompletionModel),
    Gemini(gemini::completion::CompletionModel),
    Anthropic(anthropic::completion::CompletionModel),
    Groq(groq::CompletionModel<reqwest::Client>),
    OpenRouter(openrouter::completion::CompletionModel),
    Ollama(ollama::CompletionModel<reqwest::Client>),
    Azure(azure::CompletionModel<reqwest::Client>),
    XAI(xai::completion::CompletionModel),
    Cohere(cohere::completion::CompletionModel),
    Mistral(mistral::completion::CompletionModel),
    Together(together::completion::CompletionModel),
    Deepseek(deepseek::CompletionModel<reqwest::Client>),
    Pylon(PylonCompletionModel),
    ClaudeSdk(ClaudeSdkModel),
    LmRouter(LmRouterLM),
}

// Implement the trait for each concrete provider type using the CompletionModel trait from rig
impl CompletionProvider for openai::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        // Convert the typed response to unit type
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for anthropic::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for gemini::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for groq::CompletionModel<reqwest::Client> {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for openrouter::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for ollama::CompletionModel<reqwest::Client> {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for azure::CompletionModel<reqwest::Client> {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}
impl CompletionProvider for xai::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for cohere::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for mistral::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for together::completion::CompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl CompletionProvider for deepseek::CompletionModel<reqwest::Client> {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let response = rig::completion::CompletionModel::completion(self, request).await?;
        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }
}

impl LMClient {
    fn get_api_key<'a>(provided: Option<&'a str>, env_var: &str) -> Result<Cow<'a, str>> {
        match provided {
            Some(k) => Ok(Cow::Borrowed(k)),
            None => Ok(Cow::Owned(std::env::var(env_var).map_err(|_| {
                anyhow::anyhow!("{} environment variable not set", env_var)
            })?)),
        }
    }

    /// Build case 1: OpenAI-compatible API from base_url + api_key
    pub fn from_openai_compatible(base_url: &str, api_key: &str, model: &str) -> Result<Self> {
        println!(
            "Building OpenAI-compatible model from base_url: {} and api_key: {} and model: {}",
            base_url, api_key, model
        );
        let client = openai::CompletionsClient::builder()
            .api_key(api_key)
            .base_url(base_url)
            .build()?;
        Ok(LMClient::OpenAI(openai::completion::CompletionModel::new(
            client, model,
        )))
    }

    /// Build case 2: Local OpenAI-compatible model from base_url (vLLM, etc.)
    /// Uses a dummy API key since local servers don't require authentication
    pub fn from_local(base_url: &str, model: &str) -> Result<Self> {
        println!(
            "Building local OpenAI-compatible model from base_url: {} and model: {}",
            base_url, model
        );
        let client = openai::CompletionsClient::builder()
            .api_key("dummy-key-for-local-server")
            .base_url(base_url)
            .build()?;
        Ok(LMClient::OpenAI(openai::completion::CompletionModel::new(
            client, model,
        )))
    }

    /// Build case 3: From provider via model name (provider:model format)
    pub fn from_model_string(model_str: &str, api_key: Option<&str>) -> Result<Self> {
        let (provider, model_id) = model_str.split_once(':').ok_or(anyhow::anyhow!(
            "Model string must be in format 'provider:model_name'"
        ))?;

        match provider {
            "openai" => {
                let key = Self::get_api_key(api_key, "OPENAI_API_KEY")?;
                let client = openai::CompletionsClient::builder()
                    .api_key(key.as_ref())
                    .build()?;
                Ok(LMClient::OpenAI(openai::completion::CompletionModel::new(
                    client, model_id,
                )))
            }
            "anthropic" => {
                let key = Self::get_api_key(api_key, "ANTHROPIC_API_KEY")?;
                let client = anthropic::Client::builder().api_key(key.as_ref()).build()?;
                Ok(LMClient::Anthropic(
                    anthropic::completion::CompletionModel::new(client, model_id),
                ))
            }
            "gemini" => {
                let key = Self::get_api_key(api_key, "GEMINI_API_KEY")?;
                let client = gemini::Client::<reqwest::Client>::builder()
                    .api_key(key.as_ref())
                    .build()?;
                Ok(LMClient::Gemini(gemini::completion::CompletionModel::new(
                    client, model_id,
                )))
            }
            "ollama" => {
                let client = ollama::Client::builder().api_key(Nothing).build()?;
                Ok(LMClient::Ollama(ollama::CompletionModel::new(
                    client, model_id,
                )))
            }
            "openrouter" => {
                let key = Self::get_api_key(api_key, "OPENROUTER_API_KEY")?;
                let client = openrouter::Client::builder()
                    .api_key(key.as_ref())
                    .build()?;
                Ok(LMClient::OpenRouter(
                    openrouter::completion::CompletionModel::new(client, model_id),
                ))
            }
            "groq" => {
                let key = Self::get_api_key(api_key, "GROQ_API_KEY")?;
                let client = groq::Client::builder().api_key(key.as_ref()).build()?;
                Ok(LMClient::Groq(groq::CompletionModel::new(client, model_id)))
            }
            "lm-router" | "lmrouter" | "router" => {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let detected = lm_router::backends::auto_detect_router().await?;
                    let default_model = detected.default_model.clone().ok_or_else(|| {
                        anyhow::anyhow!("lm-router auto-detect found no default model")
                    })?;
                    let model = if model_id.is_empty() || model_id == "auto" {
                        default_model
                    } else {
                        model_id.to_string()
                    };

                    let mut lm = LmRouterLM::new(Arc::new(detected.router), model);
                    let cheap_model = std::env::var("LM_ROUTER_CHEAP_MODEL")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                        .or_else(|| select_cheap_model(lm.router().available_models().as_slice()));

                    if let Some(cheap_model) = cheap_model {
                        lm = lm.with_cheap_model(cheap_model);
                    }

                    Ok(LMClient::LmRouter(lm))
                })
            }
            "pylon" => {
                // model_id: "local", "swarm", "hybrid", or "local:ollama", etc.
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    if model_id == "swarm" {
                        let mnemonic = api_key.ok_or_else(|| {
                            anyhow::anyhow!("pylon:swarm requires mnemonic as api_key")
                        })?;
                        Self::pylon_swarm(mnemonic).await
                    } else if model_id == "hybrid" {
                        let mnemonic = api_key.ok_or_else(|| {
                            anyhow::anyhow!("pylon:hybrid requires mnemonic as api_key")
                        })?;
                        Self::pylon_hybrid(mnemonic).await
                    } else if model_id.starts_with("local:") {
                        let backend = model_id.strip_prefix("local:").unwrap();
                        Self::pylon_local_with(backend).await
                    } else {
                        // Default to local (model_id == "local" or anything else)
                        Self::pylon_local().await
                    }
                })
            }
            "claude-sdk" | "claude" => {
                // Uses Claude Code headless mode via claude-agent-sdk
                Self::claude_sdk()
            }
            _ => {
                anyhow::bail!(
                    "Unsupported provider: {}. Supported providers are: openai, anthropic, gemini, groq, openrouter, ollama, pylon, claude-sdk, lm-router",
                    provider
                );
            }
        }
    }

    /// Convert a concrete completion model to LMClient
    ///
    /// This function accepts concrete types that can be converted to LMClient.
    /// The enum_dispatch macro automatically generates From implementations for
    /// each variant type, so you can use this with any concrete completion model.
    pub fn from_custom<T: Into<LMClient>>(client: T) -> Self {
        client.into()
    }

    // ============ Pylon factory methods ============

    /// Create Pylon client with local-only mode using Ollama (no network cost)
    ///
    /// Uses llama3.2 as the default model
    pub async fn pylon_local() -> Result<Self> {
        let model = PylonCompletionModel::local(PylonConfig::local()).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Create Pylon client with specific Ollama model
    ///
    /// Model options: any model available in Ollama (e.g., "llama3.2", "mistral", "codellama")
    pub async fn pylon_local_with(model_name: &str) -> Result<Self> {
        let model = PylonCompletionModel::local(PylonConfig::local_with(model_name)).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Create Pylon client with swarm mode (distributed, paid via NIP-90)
    ///
    /// Requires mnemonic for signing NIP-90 jobs
    pub async fn pylon_swarm(mnemonic: &str) -> Result<Self> {
        let model = PylonCompletionModel::from_mnemonic(mnemonic, PylonConfig::swarm()).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Create Pylon client with hybrid mode (local first, swarm fallback)
    ///
    /// Requires mnemonic for signing NIP-90 jobs when falling back to swarm
    pub async fn pylon_hybrid(mnemonic: &str) -> Result<Self> {
        let model = PylonCompletionModel::from_mnemonic(mnemonic, PylonConfig::hybrid()).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Create Pylon client with custom configuration
    ///
    /// Pass None for mnemonic if using local-only mode
    pub async fn pylon(mnemonic: Option<&str>, config: PylonConfig) -> Result<Self> {
        let model = match mnemonic {
            Some(m) => PylonCompletionModel::from_mnemonic(m, config).await?,
            None => PylonCompletionModel::local(config).await?,
        };
        Ok(LMClient::Pylon(model))
    }

    // ============ Claude SDK factory methods ============

    /// Create Claude SDK client using Claude Code headless mode.
    ///
    /// Uses the user's existing Claude subscription (Pro/Max).
    /// Requires `claude` CLI to be installed and authenticated.
    pub fn claude_sdk() -> Result<Self> {
        if !claude_sdk::has_claude_cli() {
            anyhow::bail!(
                "Claude CLI not found. Install from https://claude.ai/download or \
                 ensure ~/.claude/local/claude exists."
            );
        }
        Ok(LMClient::ClaudeSdk(ClaudeSdkModel::new()))
    }
}

fn select_cheap_model(models: &[String]) -> Option<String> {
    let patterns = [
        "mini", "haiku", "small", "lite", "tiny", "nano", "7b", "8b", "3b", "1b",
    ];

    for pattern in patterns {
        if let Some(model) = models.iter().find(|m| m.to_lowercase().contains(pattern)) {
            return Some(model.clone());
        }
    }

    None
}
