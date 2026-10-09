//! OpenAI's native Responses API on the caller's API key.

use super::responses::{ResponsesConfig, ResponsesUpstream, no_fields};
use super::secret::Secret;
use super::{Account, CostBasis, ModelRow, PrivacyTerms};

/// Native model ids and capabilities, using the existing OpenAI catalog.
pub fn default_models() -> Vec<ModelRow> {
    super::pro::default_models()
        .into_iter()
        .map(|mut row| {
            row.upstream_model = row.id.trim_start_matches("openai/").to_owned();
            row.capabilities.tools = true;
            row.capabilities.images = true;
            row.capabilities.json_schema = true;
            row.price.cache_write = None;
            row
        })
        .collect()
}

/// Settings for a caller's key. No ambient credentials are read.
pub fn config(key: Option<Secret>) -> ResponsesConfig {
    ResponsesConfig {
        name: "openai",
        url: "https://api.openai.com/v1/responses".into(),
        key,
        headers: Vec::new(),
        account: Account {
            id: crate::run::CALLER_KEY.into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::unverified(
            "OpenAI: the caller's retention agreement is not verified",
        ),
        privacy_fields: |_| no_fields(),
        models: default_models(),
    }
}

pub type OpenAi = ResponsesUpstream;
