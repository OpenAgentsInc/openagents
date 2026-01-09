use rig::completion::Usage;
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LmUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl From<Usage> for LmUsage {
    fn from(usage: Usage) -> Self {
        LmUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
        }
    }
}

impl Add for LmUsage {
    type Output = LmUsage;

    fn add(self, other: LmUsage) -> Self {
        LmUsage {
            prompt_tokens: self.prompt_tokens + other.prompt_tokens,
            completion_tokens: self.completion_tokens + other.completion_tokens,
            total_tokens: self.total_tokens + other.total_tokens,
        }
    }
}
