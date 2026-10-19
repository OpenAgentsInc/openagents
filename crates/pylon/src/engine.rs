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
    /// The name the server knows the model by, read once from its
    /// `/v1/models` (see [`served_name`]).
    served: tokio::sync::OnceCell<String>,
}

/// The id in a `/v1/models` body to request `model` by. Psionic lists a
/// model by its file name (`Qwen3.5-0.8B-Q8_0.gguf`) and matches requests
/// case-sensitively, while a beacon names it `qwen3.5-0.8b-q8_0`; so pick
/// the listed id that equals `model` ignoring case and a `.gguf` ending,
/// else the only model listed, else `model` itself.
#[must_use]
pub fn served_name(model: &str, models: &Value) -> Option<String> {
    let plain = |name: &str| {
        let lower = name.to_lowercase();
        lower.strip_suffix(".gguf").unwrap_or(&lower).to_string()
    };
    let ids: Vec<&str> = models["data"]
        .as_array()?
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();
    ids.iter()
        .find(|id| plain(id) == plain(model))
        .or_else(|| (ids.len() == 1).then(|| &ids[0]))
        .map(|id| (*id).to_string())
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
            served: tokio::sync::OnceCell::new(),
        })
    }

    /// The name to request the model by: [`served_name`] over the
    /// server's list, remembered once read; `model` while it cannot be read.
    async fn request_name(&self) -> String {
        let read = self
            .served
            .get_or_try_init(|| async {
                let body: Value = self
                    .client
                    .get(format!("{}/v1/models", self.base))
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                    .map_err(|_| ())?
                    .json()
                    .await
                    .map_err(|_| ())?;
                served_name(&self.model, &body).ok_or(())
            })
            .await;
        read.map_or_else(|()| self.model.clone(), Clone::clone)
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
            let request = self.request_name().await;
            let response = self
                .client
                .post(format!("{}/v1/chat/completions", self.base))
                .json(&json!({
                    "model": request,
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
                model: self.model.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_served_name_matches_the_beacon_name_by_file_name() {
        let listed = json!({"data": [{"id": "Qwen3.5-0.8B-Q8_0.gguf"}]});
        assert_eq!(
            served_name("qwen3.5-0.8b-q8_0", &listed).as_deref(),
            Some("Qwen3.5-0.8B-Q8_0.gguf")
        );
        let two = json!({"data": [{"id": "a.gguf"}, {"id": "Qwen3.5-0.8B-Q8_0"}]});
        assert_eq!(
            served_name("qwen3.5-0.8b-q8_0", &two).as_deref(),
            Some("Qwen3.5-0.8B-Q8_0")
        );
        // One model listed under another name: that one.
        assert_eq!(
            served_name("m", &listed).as_deref(),
            Some("Qwen3.5-0.8B-Q8_0.gguf")
        );
        // Several and none matching: ask by the given name.
        assert_eq!(served_name("m", &two), None);
        assert_eq!(served_name("m", &json!({})), None);
    }
}
