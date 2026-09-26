//! Generation through Vertex AI's OpenAI-compatible endpoint, for the open
//! models Google serves as managed APIs (`--provider vertex`).
//!
//! The endpoint speaks chat completions, so the request is
//! [`openrouter::Client::structured`]'s: the action comes back as JSON under
//! a strict `json_schema` response format. That format works for every
//! model listed in [`PRICES`]; forced tool calls don't (gpt-oss-120b refuses
//! them). The client's OpenRouter-only fields are accepted and ignored.
//!
//! Vertex reports tokens and no cost, so a step's cost is its tokens at the
//! model's list price ([`Basis::ListPrice`]). A model without a price here
//! leaves the cost unknown, and so does a failed call that may have been
//! billed. Cached input tokens are priced as uncached, since the client
//! doesn't read Vertex's cache count, so a cost can only be overstated.
//!
//! The bearer is an OAuth access token, which expires after about an hour,
//! so it is read from a file before every call: `VERTEX_TOKEN_FILE`, or
//! `~/.openagents/vertex-token` (mode 600), kept fresh by the operator.

use std::path::PathBuf;
use std::time::Instant;

use crate::models::{Basis, Generate, Generated, NextAction, next_action_schema};

/// The OpenAI-compatible endpoint at location `global`, where these models
/// are served (us-central1 doesn't host them). `VERTEX_BASE_URL`
/// overrides it.
pub const BASE_URL: &str = "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/endpoints/openapi";

/// The variable that names another endpoint.
pub const URL_VAR: &str = "VERTEX_BASE_URL";

/// The variable that names the token file.
pub const TOKEN_VAR: &str = "VERTEX_TOKEN_FILE";

/// Tokens a reply may use, reasoning included: gpt-oss reasons before it
/// writes the reply, and a reply cut short has no JSON.
pub const MAX_TOKENS: u32 = 32_000;

/// List prices in dollars per million input and output tokens, from Google
/// Cloud's Vertex AI pricing page
/// (<https://cloud.google.com/vertex-ai/generative-ai/pricing>, the
/// partner and open models tables, retrieved 2026-09-26).
pub const PRICES: [(&str, f64, f64); 5] = [
    ("openai/gpt-oss-120b-maas", 0.09, 0.36),
    ("qwen/qwen3-coder-480b-a35b-instruct-maas", 0.22, 1.80),
    ("moonshotai/kimi-k2-thinking-maas", 0.60, 2.50),
    ("zai-org/glm-5-maas", 1.00, 3.20),
    ("deepseek-ai/deepseek-v3.2-maas", 0.56, 1.68),
];

/// The list price of `input` and `output` tokens on `model`, or `None` when
/// the model has no price here.
#[must_use]
pub fn cost(model: &str, input: u64, output: u64) -> Option<f64> {
    PRICES
        .iter()
        .find(|(name, _, _)| *name == model)
        .map(|(_, i, o)| (input as f64 * i + output as f64 * o) / 1_000_000.0)
}

/// Generation on Vertex.
pub struct VertexGenerator {
    /// The Vertex model ID, such as `qwen/qwen3-coder-480b-a35b-instruct-maas`.
    pub model: String,
    /// `low`, `medium`, or `high`, or `None` for the model's default.
    pub effort: Option<String>,
    pub base_url: String,
    pub token_file: PathBuf,
}

impl VertexGenerator {
    /// A generator for `model` on `VERTEX_BASE_URL` (default [`BASE_URL`])
    /// with the token in `VERTEX_TOKEN_FILE` (default
    /// `~/.openagents/vertex-token`).
    ///
    /// # Errors
    ///
    /// When the token file can't be read now.
    pub fn from_env(model: &str, effort: Option<String>) -> Result<Self, String> {
        let token_file = std::env::var_os(TOKEN_VAR)
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".openagents/vertex-token"))
            })
            .ok_or("no home directory for the Vertex token")?;
        read_token(&token_file)?;
        Ok(VertexGenerator {
            model: model.to_string(),
            effort,
            base_url: std::env::var(URL_VAR)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| BASE_URL.to_string()),
            token_file,
        })
    }
}

fn read_token(path: &std::path::Path) -> Result<String, String> {
    let token = std::fs::read_to_string(path)
        .map_err(|e| format!("can't read the Vertex token at {}: {e}", path.display()))?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(format!("the Vertex token at {} is empty", path.display()));
    }
    Ok(token)
}

impl Generate for VertexGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let started = Instant::now();
        let milliseconds = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let failed = |why: String, usd: Option<f64>| Generated {
            action: Err(why.clone()),
            model: self.model.clone(),
            prompt_tokens: 0,
            completion_tokens: 0,
            usd,
            known_usd: usd.unwrap_or(0.0),
            cost_unknown: usd.is_none().then_some(why),
            cost_basis: Basis::ListPrice,
            milliseconds: milliseconds(),
        };
        let token = match read_token(&self.token_file) {
            Ok(token) => token,
            // Nothing was sent.
            Err(why) => return failed(why, Some(0.0)),
        };
        let mut config = openrouter::Config::new(openrouter::ApiKey::new(&token));
        config.base_url = self.base_url.clone();
        config.referer = None;
        config.title = None;
        let client = match openrouter::Client::new(config) {
            Ok(client) => client,
            Err(error) => return failed(error.to_string(), Some(0.0)),
        };
        let mut request = openrouter::ChatRequest::new(
            &self.model,
            vec![
                openrouter::Message::system(system),
                openrouter::Message::user(prompt),
            ],
        )
        .max_tokens(MAX_TOKENS);
        if let Some(effort) = &self.effort {
            request = request.effort(effort);
        }
        let priced = |usage: &openrouter::Usage| {
            cost(&self.model, usage.prompt_tokens, usage.completion_tokens)
        };
        let no_price = || format!("{} has no list price in microcoder::vertex", self.model);
        match client
            .structured::<NextAction>(request, "next_action", next_action_schema())
            .await
        {
            Ok(reply) => {
                let usd = priced(&reply.usage);
                Generated {
                    action: Ok(reply.value),
                    model: self.model.clone(),
                    prompt_tokens: reply.usage.prompt_tokens,
                    completion_tokens: reply.usage.completion_tokens,
                    usd,
                    known_usd: usd.unwrap_or(0.0),
                    cost_unknown: usd.is_none().then(no_price),
                    cost_basis: Basis::ListPrice,
                    milliseconds: reply.milliseconds,
                }
            }
            Err(openrouter::Error::Schema {
                detail,
                excerpt,
                usage,
            }) => {
                let usd = priced(&usage);
                Generated {
                    action: Err(format!(
                        "the reply didn't match the schema ({detail}): {excerpt}"
                    )),
                    model: self.model.clone(),
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    usd,
                    known_usd: usd.unwrap_or(0.0),
                    cost_unknown: usd.is_none().then(no_price),
                    cost_basis: Basis::ListPrice,
                    milliseconds: milliseconds(),
                }
            }
            Err(
                error @ (openrouter::Error::Connection(_)
                | openrouter::Error::Timeout
                | openrouter::Error::Decode { .. }),
            ) => failed(
                format!("the call failed after it was sent and may have been billed ({error})"),
                None,
            ),
            // An error status: refused, so not billed.
            Err(error) => failed(error.to_string(), Some(0.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_prices_come_from_the_table() {
        let usd = cost(
            "qwen/qwen3-coder-480b-a35b-instruct-maas",
            1_000_000,
            1_000_000,
        );
        assert!((usd.unwrap() - 2.02).abs() < 1e-9);
        let usd = cost("openai/gpt-oss-120b-maas", 10_000, 1_000).unwrap();
        assert!((usd - 0.001_26).abs() < 1e-12);
        assert_eq!(cost("some/unpriced-model", 1, 1), None);
    }
}
