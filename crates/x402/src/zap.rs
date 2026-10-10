//! NIP-57 zaps to our agents and plugins (#11147): tips, never payment
//! for a request.
//!
//! The receiving side of a `lud16` that allows Nostr: the LNURL-pay
//! answer that says so ([`pay_request`]), the checks NIP-57 (Appendix D)
//! puts on a zap request before an invoice is made ([`check_request`]),
//! the invoice's description hash ([`description_hash`], the SHA-256 of
//! the request's JSON), and the kind `9735` receipt published once the
//! invoice is paid ([`receipt_event`]).
//!
//! **A zap never pays a request.** A zap invoice commits to the zap
//! request; an x402 or `Payment` invoice commits to the `http:1` binding
//! of one HTTP request, and the facilitator accepts only invoices the
//! router issued for that binding. The two are never mixed, and a zap
//! receipt is never written as an `openagents.payment-receipt.v1` for a
//! served request (its protocol is `zap`).
//!
//! Serving the LNURL endpoint and watching for the payment are the
//! receiver host's; nothing here touches the network or a wallet.

use nostr::domain::{Event, RelaySigner, Tag, ZapRequest, open_zap_receipt, open_zap_request};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The kind NIP-57 zap receipts use.
pub const RECEIPT_KIND: u16 = 9_735;
/// The largest zap request JSON accepted in a callback.
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// The LNURL-pay (LUD-06) answer for a `lud16` that accepts zaps:
/// `allowsNostr` and the key that signs receipts (`nostrPubkey`), which
/// is the receiver's key, not the recipient's.
#[must_use]
pub fn pay_request(
    callback: &str,
    receipt_pubkey: &str,
    min_msat: u64,
    max_msat: u64,
    text: &str,
) -> Value {
    json!({
        "tag": "payRequest",
        "callback": callback,
        "minSendable": min_msat,
        "maxSendable": max_msat,
        "metadata": json!([["text/plain", text]]).to_string(),
        "allowsNostr": true,
        "nostrPubkey": receipt_pubkey,
        "commentAllowed": 0,
    })
}

/// A zap request a callback accepted: the parsed request and its exact
/// JSON (the receipt's `description`).
#[derive(Debug, Clone)]
pub struct Accepted {
    pub request: ZapRequest,
    pub json: String,
}

/// Check the `nostr` query value of an LNURL callback for `amount_msat`
/// against NIP-57: a valid signature, kind `9734`, tags, exactly one `p`
/// (one of `recipients`, the keys this receiver takes zaps for), zero or
/// one `e`, a `relays` tag, and an `amount` tag, when present, equal to
/// the callback's amount. An `lnurl` tag, when present, must be ours.
///
/// # Errors
///
/// A sentence for the LNURL error answer.
pub fn check_request(
    json: &str,
    amount_msat: u64,
    recipients: &[&str],
    lnurl: Option<&str>,
) -> Result<Accepted, String> {
    if json.len() > MAX_REQUEST_BYTES {
        return Err("The zap request is too large.".into());
    }
    let event: Event =
        serde_json::from_str(json).map_err(|_| "The zap request isn't an event.".to_owned())?;
    event
        .validate_structure()
        .map_err(|_| "The zap request isn't a well-formed event.".to_owned())?;
    event
        .validate_crypto()
        .map_err(|_| "The zap request's signature doesn't verify.".to_owned())?;
    let request = open_zap_request(&event).map_err(|error| error.to_string())?;
    if !recipients.contains(&request.recipient.as_str()) {
        return Err("The zap request is for someone this address doesn't take zaps for.".into());
    }
    if let Some(amount) = request.amount_msat
        && amount != amount_msat
    {
        return Err("The zap request's amount doesn't match the amount asked for.".into());
    }
    if let (Some(theirs), Some(ours)) = (request.lnurl.as_deref(), lnurl)
        && !theirs.eq_ignore_ascii_case(ours)
    {
        return Err("The zap request names another LNURL.".into());
    }
    Ok(Accepted {
        request,
        json: json.to_owned(),
    })
}

/// The invoice's description hash for an accepted zap request.
#[must_use]
pub fn description_hash(accepted: &Accepted) -> [u8; 32] {
    Sha256::digest(accepted.json.as_bytes()).into()
}

/// The kind `9735` receipt for a paid zap invoice, signed by the
/// receiver's key (the `nostrPubkey` above), `created_at` the time it was
/// paid. Checked by the NIP-57 reader before it is returned.
///
/// # Errors
///
/// A receipt the reader refuses: the invoice doesn't commit to this
/// request, its amount differs, or the preimage doesn't match.
pub fn receipt_event(
    signer: &RelaySigner,
    accepted: &Accepted,
    bolt11: &str,
    preimage: Option<&str>,
    paid_at: u64,
) -> Result<Event, String> {
    let request = &accepted.request;
    let mut tags = vec![Tag::new(vec!["p".into(), request.recipient.clone()])];
    if let Some(event_id) = &request.event_id {
        tags.push(Tag::new(vec!["e".into(), event_id.clone()]));
    }
    if let Some(address) = &request.address {
        tags.push(Tag::new(vec!["a".into(), address.to_string()]));
    }
    if let Some(kind) = request.target_kind {
        tags.push(Tag::new(vec!["k".into(), kind.to_string()]));
    }
    tags.push(Tag::new(vec!["P".into(), request.sender.clone()]));
    tags.push(Tag::new(vec!["bolt11".into(), bolt11.to_owned()]));
    tags.push(Tag::new(vec!["description".into(), accepted.json.clone()]));
    if let Some(preimage) = preimage {
        tags.push(Tag::new(vec!["preimage".into(), preimage.to_owned()]));
    }
    let event = signer.sign(paid_at, RECEIPT_KIND, tags, String::new());
    open_zap_receipt(&event).map_err(|error| error.to_string())?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::x402::test_invoice::{number, signed_by, tag, words};

    const NOW: u64 = 1_791_590_400;

    fn key(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn zap_request(sender: &RelaySigner, recipient: &str, amount: Option<u64>) -> String {
        let mut tags = vec![
            Tag::new(vec!["p".into(), recipient.to_owned()]),
            Tag::new(vec!["relays".into(), "wss://relay.openagents.com".into()]),
        ];
        if let Some(amount) = amount {
            tags.push(Tag::new(vec!["amount".into(), amount.to_string()]));
        }
        serde_json::to_string(&sender.sign(NOW, 9_734, tags, "thanks".into())).unwrap()
    }

    /// A 21 sat invoice committing to `hash`, paid by `preimage`.
    fn invoice(hash: [u8; 32], preimage: [u8; 32]) -> String {
        let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&payment_hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&hash)));
        fields.extend(tag(6, &number(600)));
        signed_by([0x31; 32], "lnbc210n", fields, false, false, NOW)
    }

    #[test]
    fn a_zap_is_checked_invoiced_on_its_request_and_receipted() {
        let agent = key("21");
        let receiver = key("22");
        let tipper = key("23");
        let answer = pay_request(
            "https://openagents.com/lnurlp/agent/callback",
            receiver.pubkey(),
            1_000,
            1_000_000,
            "Tip this agent",
        );
        assert_eq!(answer["allowsNostr"], true);
        assert_eq!(answer["nostrPubkey"], receiver.pubkey());

        let json = zap_request(&tipper, agent.pubkey(), Some(21_000));
        let accepted = check_request(&json, 21_000, &[agent.pubkey()], None).unwrap();
        assert_eq!(accepted.request.sender, tipper.pubkey());
        let hash = description_hash(&accepted);
        let preimage = [0x44; 32];
        let bolt11 = invoice(hash, preimage);
        let receipt = receipt_event(
            &receiver,
            &accepted,
            &bolt11,
            Some(&hex::encode(preimage)),
            NOW + 5,
        )
        .unwrap();
        let opened = open_zap_receipt(&receipt).unwrap();
        assert_eq!(opened.recipient, agent.pubkey());
        assert_eq!(opened.sender, tipper.pubkey());
        assert_eq!(opened.amount_msat, Some(21_000));

        // An invoice that commits to anything else can't carry the receipt.
        let other = invoice([9; 32], preimage);
        assert!(receipt_event(&receiver, &accepted, &other, None, NOW + 5).is_err());
        // Nor can a preimage for another payment.
        assert!(
            receipt_event(
                &receiver,
                &accepted,
                &bolt11,
                Some(&"55".repeat(32)),
                NOW + 5
            )
            .is_err()
        );
    }

    #[test]
    fn a_zap_request_is_refused_before_an_invoice() {
        let agent = key("21");
        let tipper = key("23");
        let json = zap_request(&tipper, agent.pubkey(), Some(21_000));
        // Another amount, another recipient.
        assert!(check_request(&json, 1_000, &[agent.pubkey()], None).is_err());
        assert!(check_request(&json, 21_000, &[key("24").pubkey()], None).is_err());
        // A tampered request.
        let tampered = json.replace("thanks", "thank you");
        assert!(check_request(&tampered, 21_000, &[agent.pubkey()], None).is_err());
        // No relays tag.
        let bare = serde_json::to_string(&tipper.sign(
            NOW,
            9_734,
            vec![Tag::new(vec!["p".into(), agent.pubkey().to_owned()])],
            String::new(),
        ))
        .unwrap();
        assert!(check_request(&bare, 21_000, &[agent.pubkey()], None).is_err());
        // No amount tag: any amount the callback asks for.
        let open = zap_request(&tipper, agent.pubkey(), None);
        assert!(check_request(&open, 5_000, &[agent.pubkey()], None).is_ok());
    }
}
