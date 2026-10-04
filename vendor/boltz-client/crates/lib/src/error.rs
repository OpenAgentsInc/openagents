use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum BoltzError {
    #[error("API error: {reason} (code: {code:?})")]
    Api { reason: String, code: Option<u16> },

    #[error("EVM error: {reason} (tx: {tx_hash:?})")]
    Evm {
        reason: String,
        tx_hash: Option<String>,
    },

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// The client was configured in a way it cannot honour, so no operation
    /// would succeed. Not retryable until the caller changes the config.
    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Signing error: {0}")]
    Signing(String),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Swap expired: {swap_id}")]
    SwapExpired { swap_id: String },

    #[error("Swap failed: {swap_id}: {reason}")]
    SwapFailed { swap_id: String, reason: String },

    #[error("Quote expired")]
    QuoteExpired,

    #[error("Amount out of range: {amount} sats (min: {min}, max: {max})")]
    AmountOutOfRange { amount: u64, min: u64, max: u64 },

    #[error("Invalid quote: {0}")]
    InvalidQuote(String),

    #[error(
        "DEX quote degraded beyond slippage tolerance: expected {expected_usd}, got {quoted_usd}"
    )]
    QuoteDegradedBeyondSlippage { expected_usd: u64, quoted_usd: u64 },

    /// The claim `UserOp` was broadcast (`wallet_sendPreparedCalls` returned a
    /// `call_id`, so the preimage is already on-chain) but the confirming poll
    /// failed transiently. The claim must NOT be re-submitted — doing so would
    /// broadcast a second, reverting claim. The swap stays in `Claiming` with
    /// the `call_id` persisted; when Boltz sends the `invoice.settled` WS event
    /// the manager re-resolves it via `resume_pending_call` (and, if that still
    /// can't reach the sponsor, the on-chain lock-state check in
    /// `check_on_chain_and_retry`).
    #[error("Claim broadcast but unconfirmed (call_id {call_id}); awaiting recovery")]
    ClaimBroadcastUnconfirmed { call_id: String },

    /// Boltz rejected swap creation because the preimage hash was already used
    /// (HTTP 409 Conflict). This indicates a serious local state issue: the key
    /// index counter has regressed, causing preimage reuse. Callers must NOT
    /// retry with the next index — that would trust Boltz to tell us which
    /// indices are used.
    #[error("Duplicate preimage hash: key index counter may have regressed")]
    DuplicatePreimage,

    #[error("{0}")]
    Generic(String),
}

impl From<platform_utils::HttpError> for BoltzError {
    fn from(err: platform_utils::HttpError) -> Self {
        Self::Api {
            code: err.status(),
            reason: err.to_string(),
        }
    }
}
