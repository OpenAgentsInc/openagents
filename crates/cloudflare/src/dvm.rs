//! NIP-90 Data Vending Machine processor.
//!
//! Handles job requests and executes them using Cloudflare Workers AI.

use nostr::{JobRequest, KIND_JOB_SUMMARIZATION, KIND_JOB_TEXT_GENERATION};
use serde::{Deserialize, Serialize};
use worker::Env;

/// DVM processor using Cloudflare Workers AI.
pub struct DvmProcessor<'a> {
    env: &'a Env,
}

/// Request format for Cloudflare AI chat models.
#[derive(Serialize)]
struct AiChatRequest {
    messages: Vec<AiMessage>,
}

/// A message in the chat format.
#[derive(Serialize)]
struct AiMessage {
    role: String,
    content: String,
}

/// Response from Cloudflare AI chat models.
#[derive(Deserialize)]
struct AiChatResponse {
    response: String,
}

impl<'a> DvmProcessor<'a> {
    /// Create a new DVM processor with the given Cloudflare environment.
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Process a job request and return the result content.
    pub async fn process(&self, request: &JobRequest) -> Result<String, String> {
        match request.kind {
            KIND_JOB_TEXT_GENERATION => self.text_generation(request).await,
            KIND_JOB_SUMMARIZATION => self.summarization(request).await,
            _ => Err(format!("unsupported job kind: {}", request.kind)),
        }
    }

    /// Kind 5050: Text generation using Llama 3.
    async fn text_generation(&self, request: &JobRequest) -> Result<String, String> {
        let input = request.inputs.first().ok_or("no input provided")?;

        // Get optional system prompt from params
        let system = request
            .params
            .iter()
            .find(|p| p.key == "system")
            .map(|p| p.value.clone())
            .unwrap_or_else(|| "You are a helpful assistant.".to_string());

        self.call_ai(&system, &input.data).await
    }

    /// Kind 5001: Summarization.
    async fn summarization(&self, request: &JobRequest) -> Result<String, String> {
        let input = request.inputs.first().ok_or("no input provided")?;

        let system =
            "You are a summarization assistant. Provide a concise summary of the given text.";
        self.call_ai(system, &input.data).await
    }

    /// Call Cloudflare Workers AI with a chat completion request.
    async fn call_ai(&self, system: &str, user: &str) -> Result<String, String> {
        // Get the AI binding
        let ai = self.env.ai("AI").map_err(|e| {
            format!(
                "AI binding not found: {}. Make sure [ai] is configured in wrangler.toml",
                e
            )
        })?;

        let request = AiChatRequest {
            messages: vec![
                AiMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                AiMessage {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
        };

        // Use Llama 3 8B Instruct model
        let response: AiChatResponse = ai
            .run("@cf/meta/llama-3-8b-instruct", &request)
            .await
            .map_err(|e| format!("AI call failed: {}", e))?;

        Ok(response.response)
    }
}

/// Check if a job kind is supported by this DVM.
#[allow(dead_code)]
pub fn is_supported_kind(kind: u16) -> bool {
    kind == KIND_JOB_TEXT_GENERATION || kind == KIND_JOB_SUMMARIZATION
}

/// Get a human-readable name for a job kind.
#[allow(dead_code)]
pub fn kind_name(kind: u16) -> &'static str {
    if kind == KIND_JOB_TEXT_GENERATION {
        "text-generation"
    } else if kind == KIND_JOB_SUMMARIZATION {
        "summarization"
    } else {
        "unknown"
    }
}
