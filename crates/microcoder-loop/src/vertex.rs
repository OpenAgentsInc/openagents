//! Generation through Vertex AI's OpenAI-compatible endpoint, for the open
//! models Google serves as managed APIs: the `microcoder --provider vertex`
//! research runs of an operator who holds a Vertex token.
//!
//! Coder never generates through this module and never reads its token:
//! Coder's Vertex provider is the OpenAgents cloud fallback
//! ([`crate::capacity::Provider::Vertex`]), which holds the credential on
//! the cloud worker. INVARIANTS.md records that line.
//!
//! The endpoint speaks chat completions, so the generator is
//! [`crate::openai_compat::ChatGenerator`], and the request is
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

pub use crate::openai_compat::read_token;
use crate::openai_compat::{ChatGenerator, Format, Pricing};

/// The OpenAI-compatible endpoint at location `global`, where these models
/// are served (us-central1 doesn't host them). `VERTEX_BASE_URL`
/// overrides it.
pub const BASE_URL: &str = "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/endpoints/openapi";

/// The variable that names another endpoint.
pub const URL_VAR: &str = "VERTEX_BASE_URL";

/// The variable that names the token file.
pub const TOKEN_VAR: &str = "VERTEX_TOKEN_FILE";

/// The token file: `VERTEX_TOKEN_FILE`, else `~/.openagents/vertex-token`.
#[must_use]
pub fn token_path() -> Option<PathBuf> {
    std::env::var_os(TOKEN_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            user_home::home_os().map(|home| PathBuf::from(home).join(".openagents/vertex-token"))
        })
}

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

/// A generator for `model` on `VERTEX_BASE_URL` (default [`BASE_URL`])
/// with the token in `VERTEX_TOKEN_FILE` (default
/// `~/.openagents/vertex-token`), asking for the action under the strict
/// response format at Vertex's list prices.
///
/// # Errors
///
/// When the token file can't be read now.
pub fn generator(model: &str, effort: Option<String>) -> Result<VertexGenerator, String> {
    let token_file = token_path().ok_or("no home directory for the Vertex token")?;
    read_token(&token_file)?;
    let base_url = std::env::var(URL_VAR)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| BASE_URL.to_string());
    let mut generator = ChatGenerator::new(
        "vertex",
        model,
        &base_url,
        Some(token_file),
        Pricing::Vertex,
        Format::Schema,
    );
    generator.effort = effort;
    // Vertex accepts OpenRouter's fields and ignores them.
    generator.dialect = openrouter::Dialect::OpenRouter;
    Ok(generator)
}

/// Generation on Vertex: the OpenAI-compatible generator, configured by
/// [`generator`].
pub type VertexGenerator = ChatGenerator;

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
