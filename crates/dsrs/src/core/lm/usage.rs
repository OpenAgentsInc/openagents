use rig::completion::Usage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::Add;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LmUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    /// Cost in millisatoshis (for Pylon swarm inference, 0 for local/API providers)
    #[serde(default)]
    pub cost_msats: u64,
    /// Provider-specific usage payload (e.g., OpenAI Responses usage object)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_usage: Option<Value>,
}

impl LmUsage {
    /// Get cost in satoshis (rounded up from millisatoshis)
    pub fn cost_sats(&self) -> u64 {
        self.cost_msats.div_ceil(1000)
    }

    /// Create usage from Pylon result (no token counts, just cost)
    pub fn from_pylon_cost(cost_msats: u64) -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cost_msats,
            provider_usage: None,
        }
    }

    pub fn with_provider_usage(mut self, provider_usage: Option<Value>) -> Self {
        self.provider_usage = provider_usage;
        self
    }
}

impl From<Usage> for LmUsage {
    fn from(usage: Usage) -> Self {
        LmUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
            cost_msats: 0,
            provider_usage: None,
        }
    }
}

impl Add for LmUsage {
    type Output = LmUsage;

    fn add(self, other: LmUsage) -> Self {
        let provider_usage = match (self.provider_usage, other.provider_usage) {
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (Some(_), Some(_)) => None,
            (None, None) => None,
        };
        LmUsage {
            prompt_tokens: self.prompt_tokens + other.prompt_tokens,
            completion_tokens: self.completion_tokens + other.completion_tokens,
            total_tokens: self.total_tokens + other.total_tokens,
            cost_msats: self.cost_msats + other.cost_msats,
            provider_usage,
        }
    }
}
