//! x402 v2 `exact` Lightning over HTTP (`http:1`) and MCP (`mcp:1`) for
//! openagents.
//!
//! `crates/nostr::x402` verifies terms, invoices, and preimages without state.
//! This crate adds the two things a paid service needs around it: a
//! restart-durable replay store whose insert is atomic across processes, and
//! the embedded facilitator that maps every refusal to the upstream
//! `errorReason` vocabulary. `wire` holds the `PaymentRequired`,
//! `PaymentPayload`, and `SettlementResponse` shapes and the base64 header
//! codecs pinned to x402 commit `4fcf836cc393174130e1358577ce5d37356da1c3`.
//! `server` binds one paid resource: challenge, reconstruct, settle, execute,
//! over the smallest HTTP/1.1 loop; `front` is the multi-route version, one
//! receiver and one replay store behind many priced routes, with a settlement
//! hook before execution and the HTTP `Payment` scheme (`payment_scheme`)
//! beside x402 on the same invoice; `mcp` is the same toll on one MCP
//! server's `tools/call`, over the upstream MCP transport's `_meta` names.
//! `hosted` is an author's HTTP service sold through the front: signed
//! registrations, the upstream address rule, and the signed paid header.
//! `policy` is the buyer's standing ceilings, allowlist, and daily cap, and
//! the ledger of what it paid. `router` is the API's front door: one `402`
//! with every live method's challenge on one invoice, one settle path, and
//! the live method list discovery prints; `receipt` is the one
//! `openagents.payment-receipt.v1` record per settled payment. Nothing here
//! pays.

pub mod execution;
pub mod facilitator;
pub mod front;
pub mod hosted;
pub mod mcp;
pub mod native;
pub mod outcome;
pub mod payment_scheme;
pub mod policy;
pub mod receipt;
pub mod replay;
pub mod router;
pub mod server;
pub mod wire;

pub use facilitator::{Facilitator, settle, verify};
pub use replay::{FileReplayStore, ReplayError, ReplayStore};
pub use wire::{
    PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentPayload, PaymentRequired,
    ResourceInfo, SettlementResponse, WireError,
};

/// The x402 network identifier for a wallet network name. Only mainnet and
/// testnet exist in the `lnbtc` method; signet and regtest return `None`.
pub fn network_id(wallet_network: &str) -> Option<&'static str> {
    match wallet_network {
        "bitcoin" => Some(nostr::x402::MAINNET),
        "testnet" => Some(nostr::x402::TESTNET),
        _ => None,
    }
}

/// Seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}
