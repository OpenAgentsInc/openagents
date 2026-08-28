//! Shared JSON envelopes for Coder inference.
//!
//! Phoenix production and the local Rust coder API both speak these shapes.
//! The CLI parses them. A field that is not in this crate is not part of
//! the Coder inference door.

use serde::{Deserialize, Serialize};

pub mod classify;

/// Built-in `delegate` agents and the catalog ids they open on when the
/// parent names no `model` (#257).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuiltinAgent {
    pub id: &'static str,
    pub pool: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<&'static str>,
}

pub const BUILTIN_AGENTS: &[BuiltinAgent] = &[
    BuiltinAgent {
        id: "coder-mini",
        pool: "read-only",
        default_model: None,
    },
    BuiltinAgent {
        id: "explore",
        pool: "read-only",
        default_model: Some("gemini-3.7-flash"),
    },
    BuiltinAgent {
        id: "plan",
        pool: "read-only",
        default_model: Some("glm-5.3-flash"),
    },
    BuiltinAgent {
        id: "coder",
        pool: "read-write",
        default_model: None,
    },
];

/// `GET /api/v1/models`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogResponse {
    pub models: Vec<CatalogModel>,
    pub default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogModel {
    pub id: String,
    pub provider: String,
    pub context_window: u64,
    pub max_output: u64,
    pub availability: String,
    pub pricing_basis: String,
    pub default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<CatalogPricing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogPricing {
    pub id: String,
    pub basis: String,
    pub input_per_million_tokens: u64,
    pub output_per_million_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_per_million_tokens: Option<u64>,
}

/// What the CLI keeps from the catalog: served and configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedModel {
    pub id: String,
    pub available: bool,
    pub default: bool,
}

impl CatalogModel {
    pub fn served(&self) -> ServedModel {
        ServedModel {
            id: self.id.clone(),
            available: self.availability == "available",
            default: self.default,
        }
    }
}

/// Grant returned at mint. The plaintext `token` exists only here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MintedGrant {
    pub token: String,
    pub url: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// What the CLI holds after `POST /api/v1/threads`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InferenceGrant {
    pub thread_id: String,
    pub token: String,
    pub proxy_url: String,
    pub model: String,
}

/// `GET /api/v1/credit`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credit {
    pub allowance_microusd: i64,
    pub spent_microusd: i64,
    pub remaining_microusd: i64,
    pub unpriced_calls: u64,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreditEnvelope {
    pub credit: Credit,
}

/// Token usage one turn or grant recorded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// One transcript event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Typed refusal class. The CLI still reads `message`; `code` is how a
/// client tells drop-only from retry-safe without parsing prose.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<serde_json::Value>,
}

pub const MODEL_UNAVAILABLE: &str = "model_unavailable";
pub const MODEL_NOT_SERVED: &str = "model_not_served";
pub const MODEL_MISMATCH: &str = "model_mismatch";
pub const THREAD_QUOTA_REACHED: &str = "thread_quota_reached";
pub const THREAD_LANE_LOCAL: &str = "thread_lane_local";
pub const THREAD_TERMINAL: &str = "thread_terminal";
pub const CREDIT_EXHAUSTED: &str = "credit_exhausted";
pub const GRANT_REVOKED: &str = "grant_revoked";
pub const PROVIDER_FAILED: &str = "provider_failed";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explore_pins_gemini_in_the_shared_table() {
        let explore = BUILTIN_AGENTS
            .iter()
            .find(|agent| agent.id == "explore")
            .unwrap();
        assert_eq!(explore.default_model, Some("gemini-3.7-flash"));
        let plan = BUILTIN_AGENTS
            .iter()
            .find(|agent| agent.id == "plan")
            .unwrap();
        assert_eq!(plan.default_model, Some("glm-5.3-flash"));
    }

    #[test]
    fn credit_round_trips_the_phoenix_envelope() {
        let body = r#"{"credit":{"allowance_microusd":20000000,"spent_microusd":1600000,"remaining_microusd":18400000,"unpriced_calls":0,"complete":true}}"#;
        let parsed: CreditEnvelope = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.credit.remaining_microusd, 18_400_000);
        assert!(parsed.credit.complete);
    }

    #[test]
    fn catalog_availability_maps_to_served() {
        let model = CatalogModel {
            id: "gemini-3.7-flash".into(),
            provider: "vercel_gateway".into(),
            context_window: 1,
            max_output: 1,
            availability: "unavailable".into(),
            pricing_basis: "placeholder".into(),
            default: false,
            pricing: None,
        };
        assert!(!model.served().available);
    }
}
