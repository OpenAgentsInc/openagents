//! The model a pylon serves. The provider asks an [`Engine`] for one chat
//! completion; [`Psionic`] is the engine it uses in production, a
//! `psionic-openai-server` on loopback, and [`Echo`] is the fake for tests.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde_json::{Value, json};

/// One message of a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// `system`, `user`, or `assistant`.
    pub role: String,
    pub content: String,
}

/// A finished completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub text: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub model: String,
}

/// A boxed future, so the trait stays object safe.
pub type Pending<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A model a pylon can run jobs on.
pub trait Engine: Send + Sync {
    /// The model identifier the beacon advertises.
    fn model(&self) -> &str;
    /// Whether the engine answers right now.
    fn healthy(&self) -> Pending<'_, bool>;
    /// Complete `turns` with at most `max_tokens` new tokens.
    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>>;
}

/// A Psionic model server's OpenAI-compatible `/v1/chat/completions`, built
/// from `crates/psionic` and listening on loopback.
pub struct Psionic {
    base: String,
    model: String,
    client: reqwest::Client,
}

impl Psionic {
    /// The server at `base` (such as `http://127.0.0.1:18080`) serving
    /// `model`.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn new(base: &str, model: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client,
        })
    }
}

impl Engine for Psionic {
    fn model(&self) -> &str {
        &self.model
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async move {
            self.client
                .get(format!("{}/v1/models", self.base))
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
        })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let messages: Vec<Value> = turns
                .iter()
                .map(|t| json!({"role": t.role, "content": t.content}))
                .collect();
            let response = self
                .client
                .post(format!("{}/v1/chat/completions", self.base))
                .json(&json!({
                    "model": self.model,
                    "messages": messages,
                    "max_tokens": max_tokens,
                }))
                .send()
                .await
                .map_err(|e| format!("model server unreachable: {e}"))?;
            let status = response.status();
            let body: Value = response
                .json()
                .await
                .map_err(|e| format!("model server answered with no JSON: {e}"))?;
            if !status.is_success() {
                return Err(format!("model server refused with {status}"));
            }
            let text = body["choices"][0]["message"]["content"]
                .as_str()
                .ok_or("model server answer has no text")?
                .trim()
                .to_string();
            if text.is_empty() {
                return Err("model server returned empty text".into());
            }
            Ok(Generation {
                text,
                input_tokens: body["usage"]["prompt_tokens"].as_u64(),
                output_tokens: body["usage"]["completion_tokens"].as_u64(),
                model: body["model"].as_str().unwrap_or(&self.model).to_string(),
            })
        })
    }
}

/// A fake engine that answers with the last user message reversed, for
/// tests that need no model.
pub struct Echo;

impl Engine for Echo {
    fn model(&self) -> &str {
        "echo"
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        _max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let last = turns
                .iter()
                .rev()
                .find(|t| t.role == "user")
                .ok_or("no user message")?;
            Ok(Generation {
                text: format!("echo: {}", last.content.chars().rev().collect::<String>()),
                input_tokens: Some(last.content.len() as u64),
                output_tokens: Some(last.content.len() as u64),
                model: "echo".into(),
            })
        })
    }
}
