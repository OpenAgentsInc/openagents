//! The Pro door's upstream: the LLM proxy behind `pro.openagents.com`
//! (see `docs/gateway/README.md`), as an adapter, so the separate service
//! can retire after P1.
//!
//! Reimplemented here from that page's description of the door, not copied
//! from the private `pro` repository. What the door did, this adapter does:
//! the model goes upstream as `openai/<id>`; the output cap is sent as
//! `max_completion_tokens`; `temperature: 0` is dropped; streams ask for
//! usage; an optional customer id header is sent when
//! `PRO_UPSTREAM_CUSTOMER` is set. The upstream's name is scrubbed from
//! every error message, as the door did, so no caller sees it.
//!
//! The proxy takes no tools, so every model row says `tools: false` and a
//! request with tools is refused before sending. JSON schema output and
//! images are not confirmed through the proxy either, and stay off.
//!
//! Privacy: the proxy's data terms are not confirmed (spec section 14), so
//! this endpoint takes only `standard` requests until
//! `PRO_TERMS_VERIFIED=zero-retention` records the owner's confirmation.
//!
//! Key: `PRO_UPSTREAM_KEY` (or `PRO_UPSTREAM_KEY_FILE`), else Secret
//! Manager `pro-upstream-key`.

use super::chat::{ChatConfig, ChatUpstream, Dialect, ReasoningStyle};
use super::google::TokenSource;
use super::secret::KeyRef;
use super::{Account, Capabilities, CostBasis, ModelRow, Price, PrivacyTerms};

/// The proxy's chat completions URL. `PRO_UPSTREAM_URL` replaces it.
pub const UPSTREAM_URL: &str = "https://llm.stripe.com/chat/completions";

/// The Secret Manager secret holding the proxy key.
pub const SECRET: &str = "pro-upstream-key";

/// Words never shown to a caller.
pub const SCRUB: &[&str] = &["llm.stripe.com", "Stripe", "stripe"];

/// Where the key is looked up.
#[must_use]
pub fn key_ref() -> KeyRef {
    KeyRef::new(
        &["PRO_UPSTREAM_KEY"],
        Some((super::vertex::DEFAULT_PROJECT, SECRET)),
    )
}

fn row(name: &str, input: u64, output: u64, cached: u64, cache_write: u64) -> ModelRow {
    ModelRow {
        id: format!("openai/{name}"),
        upstream_model: format!("openai/{name}"),
        capabilities: Capabilities {
            tools: false,
            reasoning: true,
            reasoning_always_on: false,
            json_schema: false,
            images: false,
            context: 400_000,
            max_output: 128_000,
        },
        price: Price {
            input,
            cached_input: cached,
            cache_write: Some(cache_write),
            output,
        },
        price_source: "the proxy's OpenAI per-token rates, read 2026-10-08 (docs/gateway/README.md)",
    }
}

/// The three models the door served.
#[must_use]
pub fn default_models() -> Vec<ModelRow> {
    vec![
        row("gpt-5.6-sol", 4_000_000, 20_000_000, 400_000, 5_000_000),
        row("gpt-5.6-terra", 2_000_000, 12_000_000, 200_000, 2_500_000),
        row("gpt-5.6-luna", 200_000, 1_200_000, 20_000, 250_000),
    ]
}

/// The Chat Completions dialect the proxy takes.
pub const DIALECT: Dialect = Dialect {
    legacy_max_tokens: false,
    reasoning: ReasoningStyle::Effort,
    drop_zero_temperature: true,
    openai_extensions: true,
};

fn var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn terms() -> PrivacyTerms {
    if var("PRO_TERMS_VERIFIED").is_some_and(|value| value == "zero-retention") {
        PrivacyTerms::zero_retention("PRO_TERMS_VERIFIED: the owner confirmed the proxy's terms")
    } else {
        PrivacyTerms::unverified("the proxy's data terms are not confirmed yet (spec section 14)")
    }
}

/// Settings with `key`.
#[must_use]
pub fn config(key: Option<super::secret::Secret>) -> ChatConfig {
    let mut headers = Vec::new();
    if let Some(customer) = var("PRO_UPSTREAM_CUSTOMER") {
        headers.push(("X-Stripe-Customer-ID".to_owned(), customer));
    }
    ChatConfig {
        name: "pro",
        url: var("PRO_UPSTREAM_URL").unwrap_or_else(|| UPSTREAM_URL.to_owned()),
        key,
        headers,
        dialect: DIALECT,
        account: Account {
            id: "pro-free-capacity".into(),
            basis: CostBasis::FreeCapacity,
        },
        privacy: terms(),
        models: default_models(),
        scrub: SCRUB,
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

/// The Pro adapter type.
pub type Pro = ChatUpstream;
