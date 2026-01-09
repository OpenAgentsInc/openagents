use anyhow::Result;
use enum_dispatch::enum_dispatch;
use reqwest;
use rig::{
    client::Nothing,
    completion::{CompletionError, CompletionRequest, CompletionResponse},
    providers::*,
};
use std::borrow::Cow;

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
            _ => {
                anyhow::bail!(
                    "Unsupported provider: {}. Supported providers are: openai, anthropic, gemini, groq, openrouter, ollama",
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
}
