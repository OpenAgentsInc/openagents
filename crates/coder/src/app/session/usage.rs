/// Session-level usage stats for Claude API.
#[derive(Default, Clone)]
pub(crate) struct SessionUsageStats {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) total_cost_usd: f64,
    pub(crate) duration_ms: u64,
    pub(crate) num_turns: u32,
}

/// Rate limit window info.
#[derive(Clone, Default)]
pub(crate) struct RateLimitInfo {
    pub(crate) name: String,
    pub(crate) percent_used: f64,
    pub(crate) resets_at: String,
}

/// Cached rate limits from API.
#[derive(Default, Clone)]
pub(crate) struct RateLimits {
    pub(crate) primary: Option<RateLimitInfo>,
    pub(crate) secondary: Option<RateLimitInfo>,
}
