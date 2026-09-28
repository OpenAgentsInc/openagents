//! Wallet records: what a command returns and what the store remembers.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WalletError {
    /// The caller's input is malformed.
    #[error("{0}")]
    Invalid(String),
    /// The wallet is not initialized, or its files are unreadable.
    #[error("{0}")]
    Setup(String),
    /// The node refused or failed the operation.
    #[error("{0}")]
    Node(String),
    /// The payment did not settle before the wait ended; its state is unknown
    /// until `lookup` reports it.
    #[error("payment {payment_hash} is still pending after {waited_secs}s")]
    Pending {
        payment_hash: String,
        waited_secs: u64,
    },
    /// The payment failed; `reason` is the node's account of why.
    #[error("payment {payment_hash} failed: {reason}")]
    Failed {
        payment_hash: String,
        reason: String,
    },
}

/// An invoice this wallet issued.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedInvoice {
    pub bolt11: String,
    pub payment_hash: String,
    pub amount_msat: u64,
    /// The BOLT11 `h` field: the caller's request hash.
    pub description_hash: String,
    pub expiry_secs: u32,
    /// The signer, which x402 requires to equal `payTo`.
    pub pay_to: String,
}

/// Evidence that this wallet paid an invoice.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Proof {
    pub payment_hash: String,
    /// 32 bytes as 64 lowercase hex digits.
    pub preimage: String,
    pub amount_msat: u64,
    pub fee_msat: u64,
    pub bolt11: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentDirection {
    Inbound,
    Outbound,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Succeeded,
    Failed,
}

/// The store's record of one payment.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentRecord {
    pub payment_hash: String,
    pub direction: PaymentDirection,
    pub status: PaymentStatus,
    pub amount_msat: Option<u64>,
    pub fee_msat: Option<u64>,
    pub preimage: Option<String>,
    pub bolt11: Option<String>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Balance {
    pub onchain_total_sats: u64,
    pub onchain_spendable_sats: u64,
    pub lightning_total_sats: u64,
    pub anchor_reserve_sats: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    pub channel_id: String,
    pub user_channel_id: String,
    pub counterparty: String,
    pub value_sats: u64,
    pub outbound_msat: u64,
    pub inbound_msat: u64,
    pub confirmations: Option<u32>,
    pub confirmations_required: Option<u32>,
    pub ready: bool,
    pub usable: bool,
    pub announced: bool,
}
