//! Public payment acknowledgments. Private wallet evidence stays with its owner.

use super::{Payment, RECEIPT_KIND, Receipt, canonical, hex64, sha256_hex};
use crate::domain::{RelaySigner, Tag};

const MARKER: &str = "oa:pylon-payment-ack:v1";

/// An opaque, stable reference to a private network/payment identity.
#[must_use]
pub fn settlement_reference(network: &str, payment_hash: &str) -> String {
    sha256_hex(format!("openagents.pylon-settlement.v1:{network}:{payment_hash}").as_bytes())
}

fn unsigned(receipt: &Receipt) -> Result<String, String> {
    let mut body = receipt.clone();
    body.payment
        .as_mut()
        .ok_or("a payment is required")?
        .acknowledgment = None;
    String::from_utf8(canonical(&body)?).map_err(|e| e.to_string())
}

fn terms(payment: &Payment) -> Result<(), String> {
    if !matches!(payment.profile.as_str(), "lightning-bolt11" | "x402-exact") {
        return Err("unknown payment profile".into());
    }
    if !matches!(
        payment.network.as_str(),
        "bitcoin" | "testnet" | "signet" | "regtest"
    ) || (payment.profile == "x402-exact"
        && !matches!(payment.network.as_str(), "bitcoin" | "testnet"))
    {
        return Err("unknown payment network".into());
    }
    if payment.amount_msat == 0 || payment.amount_msat > 9_007_199_254_740_991 {
        return Err("payment amount must be a positive safe integer".into());
    }
    hex64(&payment.settlement, "settlement")
}

/// Sign an acknowledgment over the complete receipt, excluding the acknowledgment.
/// This attributes a payment claim; it supplies no financial authority.
///
/// # Errors
/// Invalid receipt terms or a signer other than the seller or broker.
pub fn acknowledge_payment(signer: &RelaySigner, receipt: &mut Receipt) -> Result<(), String> {
    let mut free = receipt.clone();
    free.payment = None;
    free.validate()?;
    let payment = receipt.payment.as_ref().ok_or("a payment is required")?;
    terms(payment)?;
    if signer.pubkey() != receipt.provider && signer.pubkey() != receipt.buyer {
        return Err("the seller or broker must acknowledge the payment".into());
    }
    let content = unsigned(receipt)?;
    let event = signer.sign(
        receipt.finished_at,
        RECEIPT_KIND,
        vec![
            Tag::new(vec!["t".into(), MARKER.into()]),
            Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
        ],
        content,
    );
    receipt
        .payment
        .as_mut()
        .ok_or("a payment is required")?
        .acknowledgment = Some(event);
    receipt.validate()
}

pub(super) fn validate(receipt: &Receipt, payment: &Payment) -> Result<(), String> {
    terms(payment)?;
    let ack = payment
        .acknowledgment
        .as_ref()
        .ok_or("payment acknowledgment is missing")?;
    ack.validate_crypto().map_err(|e| e.to_string())?;
    if ack.kind != RECEIPT_KIND
        || ack.created_at != receipt.finished_at
        || (ack.pubkey != receipt.provider && ack.pubkey != receipt.buyer)
        || ack.content != unsigned(receipt)?
        || ack.tags
            != vec![
                Tag::new(vec!["t".into(), MARKER.into()]),
                Tag::new(vec!["x".into(), sha256_hex(ack.content.as_bytes())]),
            ]
    {
        return Err("payment acknowledgment does not match the receipt".into());
    }
    Ok(())
}
