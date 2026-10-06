//! Lightning top-ups of the shared compute balance (#10707), on the
//! existing receiver wallet ([`LightningWallet`]) and the central ledger
//! ([`pay_ledger::compute::purchase`]).
//!
//! The wallet keeps its keys; this module only asks it for an exact invoice
//! and for what became of it. An invoice reaches the customer only after the
//! ledger records it, so an invoice issued before a crash and never recorded
//! is never shown and cannot be paid. Only the wallet's report that an
//! invoice was paid in full credits the balance, and the ledger credits each
//! payment hash once.

use openagents_wallet::{LightningWallet, PaymentStatus, parse_hash32};
use pay_ledger::{
    Ledger,
    compute::{Need, Purchase, PurchaseState, Receipt, TopUp},
};

use crate::{Error, Result, sha256_hex};

/// How long a top-up invoice stays payable.
pub const INVOICE_EXPIRY_SECS: u32 = 900;

/// A customer's request to buy credits.
#[derive(Debug, Clone)]
pub struct TopUpRequest {
    /// The client's principal and credential digest; it needs the spend
    /// right.
    pub principal: String,
    pub credential: String,
    /// The purchase identity the client chose; a retry reuses it.
    pub purchase: String,
    pub amount_sats: u64,
    pub now: i64,
}

/// The BOLT11 description hash a top-up's invoice commits to.
#[must_use]
pub fn request_hash(account: &str, purchase: &str, amount_sats: u64) -> [u8; 32] {
    let text = format!("openagents.cloud.top-up.v1\n{account}\n{purchase}\n{amount_sats}");
    let mut out = [0u8; 32];
    out.copy_from_slice(&hex::decode(sha256_hex(text.as_bytes())).unwrap_or_else(|_| vec![0; 32]));
    out
}

/// Create a top-up invoice, or return the recorded one for a retried
/// purchase. The wallet is asked for an invoice only when the purchase is
/// new.
///
/// # Errors
///
/// A principal without the spend right, an amount out of bounds, a
/// purchase identity reused with other terms, or a wallet refusal.
pub fn request_top_up(
    ledger: &mut Ledger,
    wallet: &impl LightningWallet,
    request: &TopUpRequest,
) -> Result<Purchase> {
    let principal =
        ledger.resolve_principal(&request.principal, &request.credential, Need::Spend)?;
    let amount_msat = request
        .amount_sats
        .checked_mul(1000)
        .and_then(|msat| i64::try_from(msat).ok())
        .ok_or(Error::Invalid("top-up amount"))?;
    if let Some(existing) = ledger.top_up(&request.purchase)? {
        if existing.top_up.account != principal.account
            || existing.top_up.amount_msat != amount_msat
        {
            return Err(Error::Conflict("the purchase exists with other terms"));
        }
        return Ok(existing);
    }
    if amount_msat <= 0 || amount_msat > pay_ledger::compute::purchase::TOP_UP_MAX_MSAT {
        return Err(Error::Invalid("top-up amount"));
    }
    let invoice = wallet.receive_exact(
        amount_msat as u64,
        request_hash(&principal.account, &request.purchase, request.amount_sats),
        INVOICE_EXPIRY_SECS,
    )?;
    if invoice.amount_msat != amount_msat as u64 {
        return Err(Error::Invalid("the wallet issued another amount"));
    }
    Ok(ledger.open_top_up(&TopUp {
        id: request.purchase.clone(),
        account: principal.account,
        amount_msat,
        payment_hash: invoice.payment_hash,
        invoice: invoice.bolt11,
        created_at: request.now,
        expires_at: request.now + i64::from(INVOICE_EXPIRY_SECS),
    })?)
}

/// The wallet's callback that `payment_hash` was paid. A duplicate callback
/// credits nothing more.
///
/// # Errors
///
/// An unknown payment hash or a ledger failure.
pub fn on_paid(
    ledger: &mut Ledger,
    payment_hash: &str,
    received_msat: u64,
    at: i64,
) -> Result<Purchase> {
    let received_msat = i64::try_from(received_msat).map_err(|_| Error::Invalid("amount"))?;
    Ok(ledger.observe_top_up(payment_hash, &Receipt::Paid { received_msat, at })?)
}

/// What one reconciliation pass did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Reconciled {
    pub paid: u32,
    pub expired: u32,
    pub pending: u32,
    pub unknown: u32,
    /// Lookups that failed; the purchase is left as it was.
    pub unreachable: u32,
}

/// Ask the wallet about every pending or unknown top-up and record what it
/// says. A failed lookup changes nothing; a payment hash the wallet does not
/// know is unknown, never expired or paid.
///
/// # Errors
///
/// A ledger failure.
pub fn reconcile(
    ledger: &mut Ledger,
    wallet: &impl LightningWallet,
    now: i64,
) -> Result<Reconciled> {
    let mut out = Reconciled::default();
    for purchase in ledger.open_top_ups()? {
        let hash = &purchase.top_up.payment_hash;
        let Ok(bytes) = parse_hash32(hash) else {
            out.unreachable += 1;
            continue;
        };
        let receipt = match wallet.lookup(bytes) {
            Err(_) => {
                out.unreachable += 1;
                continue;
            }
            Ok(None) => Receipt::Unknown {
                at: now,
                detail: "the wallet has no record of this invoice".into(),
            },
            Ok(Some(record)) => match (record.status, record.amount_msat) {
                (PaymentStatus::Succeeded, Some(received)) => Receipt::Paid {
                    received_msat: i64::try_from(received).unwrap_or(i64::MAX),
                    at: now,
                },
                (PaymentStatus::Succeeded, None) => Receipt::Unknown {
                    at: now,
                    detail: "paid without a received amount".into(),
                },
                (PaymentStatus::Failed, _) => Receipt::Failed { at: now },
                (PaymentStatus::Pending, _) if now >= purchase.top_up.expires_at => {
                    Receipt::Failed { at: now }
                }
                (PaymentStatus::Pending, _) => Receipt::Pending,
            },
        };
        let after = ledger.observe_top_up(hash, &receipt)?;
        match after.state {
            PurchaseState::Paid => out.paid += 1,
            PurchaseState::Expired => out.expired += 1,
            PurchaseState::Pending => out.pending += 1,
            PurchaseState::Unknown => out.unknown += 1,
        }
    }
    Ok(out)
}
