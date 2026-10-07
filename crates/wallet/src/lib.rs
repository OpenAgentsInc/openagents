//! The openagents Lightning wallet: a node key this process holds alone, so
//! it can take both x402 `exact`/`lnbtc` roles. As the receiver it mints an
//! exact-amount BOLT11 whose description hash is the caller's request hash
//! and whose signer is `payTo`. As the payer it enforces a routing-fee cap
//! before dispatch and returns the preimage that x402 proof requires.
//!
//! [`LightningWallet`] is the contract; the `ldk` feature provides the
//! [`ldk::LdkWallet`] implementation on `ldk-node`; [`resident`] lets one
//! long-running node answer other processes over a local socket. Everything here is
//! wallet mechanics: no facilitator, replay store, or Nostr record lives in
//! this crate.

pub mod backup;
pub mod config;
pub mod custody;
#[cfg(feature = "ldk")]
pub mod ldk;
pub mod model;
#[cfg(all(feature = "ldk", unix))]
pub mod open;
#[cfg(unix)]
pub mod resident;

pub use config::{Network, WalletConfig};
pub use model::{
    Balance, Channel, IssuedInvoice, PaymentDirection, PaymentRecord, PaymentStatus, Proof,
    WalletError,
};

use std::time::Duration;

/// A Lightning wallet on an exclusively held node key.
pub trait LightningWallet {
    /// The node's public key: 33 compressed bytes as 66 lowercase hex digits.
    /// This is the x402 `payTo` for invoices this wallet issues.
    fn node_id(&self) -> String;

    /// Issue a fresh invoice for exactly `amount_msat` whose BOLT11 `h` field
    /// is `request_hash`, with no inline description.
    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError>;

    /// Issue an invoice only on the expected node. Remote adapters carry
    /// this identity to the resident that creates the invoice.
    fn receive_exact_from_node(
        &self,
        expected_node: &str,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        let actual_node = self.node_id();
        if actual_node != expected_node {
            return Err(WalletError::NodeMismatch {
                expected_node: expected_node.into(),
                actual_node,
            });
        }
        self.receive_exact(amount_msat, request_hash, expiry_secs)
    }

    /// Pay `invoice`, refusing any route whose total routing fee exceeds
    /// `max_fee_msat`, and wait up to `wait` for the outcome. Paying an
    /// invoice this wallet already paid returns the recorded proof instead of
    /// paying again.
    fn pay(&self, invoice: &str, max_fee_msat: u64, wait: Duration) -> Result<Proof, WalletError>;

    /// Pay only from the explicitly approved node. Remote adapters must
    /// carry this identity to the resident that dispatches the payment.
    fn pay_from_node(
        &self,
        expected_node: &str,
        invoice: &str,
        max_fee_msat: u64,
        wait: Duration,
    ) -> Result<Proof, WalletError> {
        let actual_node = self.node_id();
        if actual_node != expected_node {
            return Err(WalletError::NodeMismatch {
                expected_node: expected_node.into(),
                actual_node,
            });
        }
        self.pay(invoice, max_fee_msat, wait)
    }

    /// The recorded state of the payment with `payment_hash`, in either
    /// direction, or `None` when this wallet never saw it.
    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError>;

    /// Look up a payment only on the expected node. Remote adapters carry
    /// this identity to the resident that reads its payment record.
    fn lookup_from_node(
        &self,
        expected_node: &str,
        payment_hash: [u8; 32],
    ) -> Result<Option<PaymentRecord>, WalletError> {
        let actual_node = self.node_id();
        if actual_node != expected_node {
            return Err(WalletError::NodeMismatch {
                expected_node: expected_node.into(),
                actual_node,
            });
        }
        self.lookup(payment_hash)
    }

    /// Every Lightning payment the node recorded, in either direction,
    /// without preimages or invoices (on-chain payments are left out). For
    /// reconciliation (`crates/pay-ledger/src/reconcile.rs`).
    fn payments(&self) -> Result<Vec<PaymentRecord>, WalletError> {
        Err(WalletError::Node(
            "this wallet cannot list its payments".into(),
        ))
    }

    /// On-chain and Lightning balances.
    fn balance(&self) -> Result<Balance, WalletError>;

    /// Every channel the node has, open or pending.
    fn channels(&self) -> Result<Vec<Channel>, WalletError>;

    /// A fresh on-chain address for funding the node.
    fn funding_address(&self) -> Result<String, WalletError>;

    /// Open a channel of `amount_sats` to `node_id` at `address`
    /// (`host:port`). Returns the local channel identifier.
    fn open_channel(
        &self,
        node_id: &str,
        address: &str,
        amount_sats: u64,
        announce: bool,
    ) -> Result<String, WalletError>;

    /// Close the channel `user_channel_id` with `counterparty`. A
    /// cooperative close needs the peer online; `force` broadcasts the
    /// latest commitment instead, which is the only way out after a
    /// restore without the channel's store.
    fn close_channel(
        &self,
        user_channel_id: &str,
        counterparty: &str,
        force: bool,
    ) -> Result<(), WalletError>;
}

/// Decode 64 lowercase hex digits into a 32-byte hash.
pub fn parse_hash32(text: &str) -> Result<[u8; 32], WalletError> {
    let bytes = hex::decode(text).map_err(|_| WalletError::Invalid("hash is not hex".into()))?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| WalletError::Invalid("hash must be 32 bytes".into()))?;
    if text.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(WalletError::Invalid("hash must be lowercase hex".into()));
    }
    Ok(array)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lowercase_hash() {
        let text = "00".repeat(32);
        assert_eq!(parse_hash32(&text).unwrap(), [0; 32]);
        assert!(parse_hash32(&"0".repeat(63)).is_err());
        assert!(parse_hash32(&"AB".repeat(32)).is_err());
        assert!(parse_hash32(&"zz".repeat(32)).is_err());
    }
}
