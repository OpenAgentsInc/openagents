//! OpenRouter, through its Open Responses route
//! (`https://openrouter.ai/api/v1/responses`, stateless), for the long
//! tail of models we hold no direct account for.
//!
//! Privacy goes in `provider`: `strict` sends `data_collection: "deny"` and
//! `zdr: true`; `standard` sends `data_collection: "deny"`. OpenRouter
//! refuses a request no provider can serve under them rather than sending
//! it elsewhere, as the chat worker relies on today (#11040).
//!
//! The key is found the way `crates/openrouter` finds it
//! (`OPENROUTER_API_KEY`, then `~/.openagents/openrouter.json`), then
//! Secret Manager `openagents-openrouter-api-key`.

use serde_json::json;

use super::google::TokenSource;
use super::responses::{ResponsesConfig, ResponsesUpstream, fields};
use super::secret::{KeyRef, Secret};
use super::{Account, Capabilities, CostBasis, ModelRow, Price, PrivacyTerms};
use crate::openagents::Privacy;

/// The Secret Manager secret holding the key.
pub const SECRET: &str = "openagents-openrouter-api-key";

/// The privacy fields for `level`.
#[must_use]
pub fn privacy_fields(level: &Privacy) -> serde_json::Map<String, serde_json::Value> {
    match level {
        Privacy::Standard => fields(json!({"provider": {"data_collection": "deny"}})),
        _ => fields(json!({"provider": {"data_collection": "deny", "zdr": true}})),
    }
}

/// The models we route to OpenRouter by default. Others are added with
/// [`ResponsesUpstream::with_model`].
#[must_use]
pub fn default_models() -> Vec<ModelRow> {
    vec![
        ModelRow {
            id: "stealth/space-bunny-alpha".into(),
            upstream_model: "stealth/space-bunny-alpha".into(),
            capabilities: Capabilities {
                tools: true,
                reasoning: true,
                reasoning_always_on: false,
                json_schema: true,
                images: false,
                context: 256_000,
                max_output: 64_000,
            },
            price: Price::micro(0, 0, 0),
            price_source: "free while in stealth: usage.cost 0 in \
                           crates/coder/fixtures/gateway/stealth-space-bunny-alpha.sse (2026-10-01)",
        },
        ModelRow {
            id: "google/gemini-3.8-flash".into(),
            upstream_model: "google/gemini-3.8-flash".into(),
            capabilities: super::vertex::default_models()[0].0.capabilities,
            price: Price::micro(750_000, 75_000, 3_750_000),
            price_source: "Google's list price, as on the Vertex row",
        },
        ModelRow {
            id: "zai/glm-5.3-flash".into(),
            upstream_model: "z-ai/glm-5.3-flash".into(),
            capabilities: super::zai::default_models()[0].capabilities,
            price: Price::micro(150_000, 30_000, 500_000),
            price_source: "Z.ai's list price, as on the Z.ai row",
        },
    ]
}

/// The OpenRouter key from the environment or `~/.openagents/openrouter.json`.
#[must_use]
pub fn local_key() -> Option<Secret> {
    openrouter::Config::from_env()
        .ok()
        .and_then(|config| Secret::new(config.api_key.expose()))
        .or_else(|| KeyRef::new(&[openrouter::KEY_VAR], None).local())
}

/// Settings with `key`.
#[must_use]
pub fn config(key: Option<Secret>) -> ResponsesConfig {
    let base = std::env::var("OPENROUTER_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| openrouter::BASE_URL.to_owned());
    ResponsesConfig {
        name: "openrouter",
        url: format!("{}/responses", base.trim_end_matches('/')),
        key,
        headers: vec![
            ("X-Title".to_owned(), "OpenAgents".to_owned()),
            (
                "HTTP-Referer".to_owned(),
                "https://openagents.com".to_owned(),
            ),
        ],
        account: Account {
            id: "openrouter".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention(
            "OpenRouter provider routing: `zdr` and `data_collection: deny` restrict a request \
             to zero-retention, no-training endpoints",
        ),
        privacy_fields,
        models: default_models(),
    }
}

/// The adapter with the key from the environment or the key file.
#[must_use]
pub fn from_env() -> ResponsesUpstream {
    ResponsesUpstream::new(config(local_key()))
}

/// The adapter with the key from the environment, the key file, or Secret
/// Manager.
///
/// # Errors
///
/// A sentence when Secret Manager could not be read.
pub async fn resolve(google: &TokenSource) -> Result<ResponsesUpstream, String> {
    let key = match local_key() {
        Some(key) => Some(key),
        None => {
            KeyRef::new(&[], Some((super::vertex::DEFAULT_PROJECT, SECRET)))
                .resolve(google)
                .await?
        }
    };
    Ok(ResponsesUpstream::new(config(key)))
}

/// The OpenRouter adapter type.
pub type OpenRouter = ResponsesUpstream;
