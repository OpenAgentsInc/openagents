//! Buying an x402 call from the person's Spark wallet (`openagents wallet`),
//! the default payer for `x402 fetch`, `call`, and `buy` (#10324): the
//! same wallet as the phone's, so "one balance on every device" holds when
//! an agent buys. This computer's Lightning node (`openagents x402 node`)
//! pays only with `--pay-with node`.

use openagents_spark::computer;
use openagents_spark::model::{AgentPayFailure, InvoicePayment, Node};
use openagents_wallet::Proof;
use sha2::{Digest, Sha256};

/// Pay `bolt11` from the Spark wallet for at most `max_fee_msat` in fees,
/// once. The proof carries the invoice's own payment hash and the preimage
/// the payee released, checked against that hash.
pub(crate) fn pay(bolt11: &str, network: &str, max_fee_msat: u64) -> Result<Proof, String> {
    if network != nostr::x402::MAINNET {
        return Err(format!(
            "the invoice is on {network}, and your wallet pays Bitcoin invoices only; nothing was paid. Pay from this computer's Lightning node instead with --pay-with node"
        ));
    }
    let invoice = nostr::x402::decode_invoice(bolt11)
        .map_err(|_| "the invoice can't be read; nothing was paid".to_string())?;
    let wallet = computer::open(&computer::home())?;
    wallet
        .sync()
        .map_err(|_| "Your wallet could not be read right now; nothing was paid.".to_string())?;
    let max_fee_sats = max_fee_msat / 1000;
    let key = invoice_key(bolt11);
    let paid = wallet
        .pay_invoice(bolt11, max_fee_sats, &key)
        .map_err(|failure| refused(&failure, max_fee_sats))?;
    proof(invoice.payment_hash(), invoice.amount_msat(), bolt11, paid)
}

/// Why the wallet did not pay, in plain words.
fn refused(failure: &AgentPayFailure, max_fee_sats: u64) -> String {
    match failure {
        AgentPayFailure::InsufficientFunds => {
            "Your wallet doesn't hold enough to pay this and its fee; nothing was paid. `openagents wallet receive` gives a payment request to add to it.".into()
        }
        AgentPayFailure::FeeTooHigh(fee) => format!(
            "The fee is ₿{fee}, above the fee cap of ₿{max_fee_sats}; nothing was paid. Raise it with --max-fee-msat."
        ),
        AgentPayFailure::Failed(message) => format!("{message} Nothing was paid."),
        AgentPayFailure::Unknown(message) => format!(
            "{message} `openagents wallet history` shows whether it went through. Running the call again with this same invoice is safe: the wallet returns that payment instead of paying twice. A new invoice would be paid again."
        ),
    }
}

/// The idempotency key for paying `bolt11`: a UUID derived from the
/// invoice, so every attempt at the same invoice, including a retry after
/// an unknown outcome, carries the same key and the wallet pays it at most
/// once. An invoice can be paid only once anyway, so no legitimate second
/// payment shares it.
fn invoice_key(bolt11: &str) -> String {
    let digest = Sha256::digest(bolt11.trim().to_ascii_lowercase().as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string()
}

/// The proof for a payment the wallet made, or why there is none yet.
fn proof(
    payment_hash: [u8; 32],
    amount_msat: u64,
    bolt11: &str,
    paid: InvoicePayment,
) -> Result<Proof, String> {
    let hash = hex(&payment_hash);
    if paid.row.status == "failed" {
        return Err("The payment did not go through. Nothing was sent.".into());
    }
    let Some(preimage) = paid.preimage else {
        return Err(format!(
            "payment {hash} is on its way, but its proof hasn't arrived yet; `openagents wallet history` shows when it lands. Don't run the call again: it would pay a second time."
        ));
    };
    if !preimage_matches(&payment_hash, &preimage) {
        return Err(format!(
            "payment {hash} went through, but the payee's proof doesn't match the invoice; the call was not retried"
        ));
    }
    Ok(Proof {
        payment_hash: hash,
        preimage,
        amount_msat,
        fee_msat: paid.row.fee_sats.saturating_mul(1000),
        bolt11: bolt11.to_owned(),
    })
}

/// Whether `preimage` (hex) hashes to `payment_hash`.
fn preimage_matches(payment_hash: &[u8; 32], preimage: &str) -> bool {
    let Some(bytes) = unhex(preimage) else {
        return false;
    };
    bytes.len() == 32 && Sha256::digest(&bytes).as_slice() == payment_hash
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_spark::model::PaymentRow;

    fn paid(status: &str, preimage: Option<&str>) -> InvoicePayment {
        InvoicePayment {
            row: PaymentRow {
                id: "p1".into(),
                received: false,
                amount_sats: 5,
                fee_sats: 3,
                method: "Lightning".into(),
                status: status.into(),
                at: 0,
            },
            preimage: preimage.map(str::to_owned),
        }
    }

    #[test]
    fn a_paid_invoice_proves_with_its_own_hash_and_a_checked_preimage() {
        let preimage = [7_u8; 32];
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        let proved = proof(
            hash,
            5_000,
            "lnbc50n1x",
            paid("completed", Some(&hex(&preimage))),
        )
        .expect("proved");
        assert_eq!(proved.payment_hash, hex(&hash));
        assert_eq!(proved.preimage, hex(&preimage));
        assert_eq!((proved.amount_msat, proved.fee_msat), (5_000, 3_000));

        let wrong = proof_err(hash, Some(&hex(&[8_u8; 32])));
        assert!(wrong.contains("doesn't match"), "{wrong}");
        let pending = proof_err(hash, None);
        assert!(pending.contains("Don't run the call again"), "{pending}");
        let failed = proof(hash, 5_000, "lnbc", paid("failed", None)).unwrap_err();
        assert!(failed.contains("Nothing was sent"), "{failed}");
    }

    fn proof_err(hash: [u8; 32], preimage: Option<&str>) -> String {
        proof(hash, 5_000, "lnbc", paid("pending", preimage)).unwrap_err()
    }

    #[test]
    fn only_bitcoin_invoices_are_paid_from_the_wallet() {
        let refused = pay("lntb1x", nostr::x402::TESTNET, 1_000).unwrap_err();
        assert!(refused.contains("--pay-with node"), "{refused}");
        assert!(refused.contains("nothing was paid"), "{refused}");
    }

    #[test]
    fn an_unknown_outcome_never_says_nothing_was_paid() {
        let unknown = refused(
            &AgentPayFailure::Unknown(
                "The wallet lost track of this payment while sending it (Network error: reset), so it may have gone through.".into(),
            ),
            1,
        );
        assert!(
            !unknown.to_lowercase().contains("nothing was paid"),
            "{unknown}"
        );
        assert!(unknown.contains("may have gone through"), "{unknown}");
        assert!(unknown.contains("openagents wallet history"), "{unknown}");
        let failed = refused(&AgentPayFailure::Failed("Refused.".into()), 1);
        assert!(failed.contains("Nothing was paid"), "{failed}");
    }

    #[test]
    fn a_retry_of_the_same_invoice_reuses_its_key() {
        let key = invoice_key("lnbc50n1abc");
        assert_eq!(key, invoice_key("lnbc50n1abc"));
        assert_eq!(key, invoice_key(" LNBC50N1ABC\n"));
        assert_ne!(key, invoice_key("lnbc50n1abd"));
        let parsed = uuid::Uuid::parse_str(&key).expect("a UUID");
        assert_eq!(parsed.get_version_num(), 4);
    }

    #[test]
    fn refusals_say_nothing_was_paid_and_what_to_do() {
        let poor = refused(&AgentPayFailure::InsufficientFunds, 1);
        assert!(poor.contains("wallet receive"), "{poor}");
        let fee = refused(&AgentPayFailure::FeeTooHigh(3), 1);
        assert!(
            fee.contains("₿3") && fee.contains("--max-fee-msat"),
            "{fee}"
        );
    }
}
