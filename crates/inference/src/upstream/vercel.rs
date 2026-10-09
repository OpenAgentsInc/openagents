//! The Vercel AI Gateway, through its Open Responses route
//! (`https://ai-gateway.vercel.sh/v1/responses`): a second broad route and
//! one adapter among many, not the router.
//!
//! Privacy: `strict` sends `providerOptions.gateway.zeroDataRetention:
//! true`, which limits the request to zero-retention providers; `standard`
//! sends nothing extra. Every request is `store: false`.
//!
//! Key: `AI_GATEWAY_API_KEY`, then `CODER_AI_GATEWAY_KEY` (each also as a
//! `_FILE`), then Secret Manager `openagents-vercel-gateway-api-key`.

use serde_json::json;

use super::google::TokenSource;
use super::responses::{ResponsesConfig, ResponsesUpstream, fields, no_fields};
use super::secret::{KeyRef, Secret};
use super::{Account, CostBasis, ModelRow, Price, PrivacyTerms};
use crate::openagents::Privacy;

/// The gateway's root.
pub const BASE_URL: &str = "https://ai-gateway.vercel.sh";

/// The Secret Manager secret holding the key.
pub const SECRET: &str = "openagents-vercel-gateway-api-key";

/// Where the key is looked up.
#[must_use]
pub fn key_ref() -> KeyRef {
    KeyRef::new(
        &["AI_GATEWAY_API_KEY", "CODER_AI_GATEWAY_KEY"],
        Some((super::vertex::DEFAULT_PROJECT, SECRET)),
    )
}

/// The privacy fields for `level`.
#[must_use]
pub fn privacy_fields(level: &Privacy) -> serde_json::Map<String, serde_json::Value> {
    match level {
        Privacy::Standard => no_fields(),
        _ => fields(json!({"providerOptions": {"gateway": {"zeroDataRetention": true}}})),
    }
}

/// The models we route to Vercel by default: the two the chat worker uses
/// today.
#[must_use]
pub fn default_models() -> Vec<ModelRow> {
    vec![
        ModelRow {
            id: "google/gemini-3.8-flash".into(),
            upstream_model: "google/gemini-3.8-flash".into(),
            capabilities: super::vertex::default_models()[0].0.capabilities,
            price: Price::micro(750_000, 75_000, 3_750_000),
            price_source: "Vercel's marketCost in crates/coder/fixtures/gateway/google-gemini-3.8-flash.sse",
        },
        ModelRow {
            id: "zai/glm-5.3-flash".into(),
            upstream_model: "zai/glm-5.3-flash".into(),
            capabilities: super::zai::default_models()[0].capabilities,
            price: Price::micro(150_000, 30_000, 500_000),
            price_source: "Vercel's marketCost in crates/coder/fixtures/gateway/zai-glm-5.3-flash.sse",
        },
    ]
}

/// Settings with `key`.
#[must_use]
pub fn config(key: Option<Secret>) -> ResponsesConfig {
    let base = std::env::var("VERCEL_AI_GATEWAY_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| BASE_URL.to_owned());
    ResponsesConfig {
        name: "vercel",
        url: format!("{}/v1/responses", base.trim_end_matches('/')),
        key,
        headers: Vec::new(),
        account: Account {
            id: "vercel".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention(
            "Vercel AI Gateway: providerOptions.gateway.zeroDataRetention routes only to \
             zero-retention providers",
        ),
        privacy_fields,
        models: default_models(),
    }
}

/// The adapter with the key from the environment or a mounted file.
#[must_use]
pub fn from_env() -> ResponsesUpstream {
    ResponsesUpstream::new(config(key_ref().local()))
}

/// The adapter with the key from the environment, a mounted file, or
/// Secret Manager.
///
/// # Errors
///
/// A sentence when Secret Manager could not be read.
pub async fn resolve(google: &TokenSource) -> Result<ResponsesUpstream, String> {
    Ok(ResponsesUpstream::new(config(
        key_ref().resolve(google).await?,
    )))
}

/// The Vercel adapter type.
pub type Vercel = ResponsesUpstream;
