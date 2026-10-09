//! Z.ai direct (GLM), billed to the prepaid Z.ai credit.
//!
//! Wire: Z.ai's OpenAI-compatible Chat Completions
//! (`https://api.z.ai/api/paas/v4/chat/completions`). `glm-5.3-flash` is
//! the cheap, fast tier the chat worker's `glm` lane already names (spec
//! section 4). Z.ai's docs say its thinking cannot be turned off, so every
//! request sends `thinking: {"type": "enabled"}` and the stream's
//! `reasoning_content` becomes a reasoning item, never answer text.
//!
//! Privacy: Z.ai's data terms (training, retention) are not confirmed yet
//! (spec section 14), so this endpoint takes only `standard` requests and
//! refuses `strict` ones until `ZAI_TERMS_VERIFIED=zero-retention` records
//! the owner's confirmation.
//!
//! Key: `ZAI_API_KEY` (or `ZAI_API_KEY_FILE`), else Secret Manager
//! `zai-api-key` in `openagentsgemini`. Without one the adapter stays
//! listed and refuses with `unconfigured`.

use super::chat::{ChatConfig, ChatUpstream, Dialect, ReasoningStyle};
use super::google::TokenSource;
use super::secret::KeyRef;
use super::{Account, Capabilities, CostBasis, ModelRow, Price, PrivacyTerms};

/// Z.ai's API root.
pub const BASE_URL: &str = "https://api.z.ai/api/paas/v4";

/// The Secret Manager secret holding the key (spec section 14).
pub const SECRET: &str = "zai-api-key";

/// Where the key is looked up.
#[must_use]
pub fn key_ref() -> KeyRef {
    KeyRef::new(
        &["ZAI_API_KEY"],
        Some((super::vertex::DEFAULT_PROJECT, SECRET)),
    )
}

/// The models served through Z.ai.
#[must_use]
pub fn default_models() -> Vec<ModelRow> {
    vec![ModelRow {
        id: "zai/glm-5.3-flash".into(),
        upstream_model: "glm-5.3-flash".into(),
        capabilities: Capabilities {
            tools: true,
            reasoning: true,
            reasoning_always_on: true,
            json_schema: false,
            images: false,
            context: 1_000_000,
            max_output: 128_000,
        },
        price: Price::micro(150_000, 30_000, 500_000),
        price_source: "Z.ai list price (spec section 8); matches Vercel's marketCost in \
                       crates/coder/fixtures/gateway/zai-glm-5.3-flash.sse (2026-09-19)",
    }]
}

/// The Chat Completions dialect Z.ai takes.
pub const DIALECT: Dialect = Dialect {
    legacy_max_tokens: true,
    reasoning: ReasoningStyle::AlwaysOn,
    drop_zero_temperature: false,
    openai_extensions: false,
};

fn terms() -> PrivacyTerms {
    let verified = std::env::var("ZAI_TERMS_VERIFIED").is_ok_and(|value| value == "zero-retention");
    if verified {
        PrivacyTerms::zero_retention("ZAI_TERMS_VERIFIED: the owner confirmed Z.ai's data terms")
    } else {
        PrivacyTerms::unverified("Z.ai's data terms are not confirmed yet (spec section 14)")
    }
}

/// Settings with `key`.
#[must_use]
pub fn config(key: Option<super::secret::Secret>) -> ChatConfig {
    let base = std::env::var("ZAI_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| BASE_URL.to_owned());
    ChatConfig {
        name: "zai",
        url: format!("{}/chat/completions", base.trim_end_matches('/')),
        key,
        headers: Vec::new(),
        dialect: DIALECT,
        account: Account {
            id: "zai-credit".into(),
            basis: CostBasis::PrepaidCredit,
        },
        privacy: terms(),
        models: default_models(),
        scrub: &[],
    }
}

/// The adapter with the key from the environment or a mounted file.
#[must_use]
pub fn from_env() -> ChatUpstream {
    ChatUpstream::new(config(key_ref().local()))
}

/// The adapter with the key from the environment, a mounted file, or
/// Secret Manager.
///
/// # Errors
///
/// A sentence when Secret Manager could not be read.
pub async fn resolve(google: &TokenSource) -> Result<ChatUpstream, String> {
    Ok(ChatUpstream::new(config(key_ref().resolve(google).await?)))
}

/// The Z.ai adapter type.
pub type Zai = ChatUpstream;
