//! Public `3201` receipts for sealed jobs: Pylon v2 receipts whose digests
//! are over ciphertext, with the attested block, and payment only as Pylon
//! v2's opaque settlement reference and signed acknowledgment.
use crate::Result;
use nostr::domain::{Event, RelaySigner};
use nostr::pylon::{Receipt, sha256_hex};
use serde_json::{Value, json};

/// The attested facts a sealed receipt names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attested {
    pub endpoint: String,
    pub release: String,
    pub level: String,
    pub measurement: String,
}

impl Attested {
    fn meta(&self) -> Value {
        json!({"attested": {"endpoint": self.endpoint, "release": self.release, "level": self.level, "measurement": self.measurement}})
    }
}

/// Turn a receipt for a sealed job into the sealed form: the attested
/// feature and block, and digests of the request and result **ciphertext**
/// (the event contents as sent), never of the plaintext.
///
/// # Errors
///
/// When the result body is not valid afterwards (any payment is checked
/// once acknowledged).
pub fn seal(
    mut receipt: Receipt,
    attested: &Attested,
    request: &Event,
    result: &Event,
) -> Result<Receipt> {
    receipt.requires = vec![nostr::att::ATTESTED_FEATURE.into()];
    receipt.meta = Some(attested.meta());
    receipt.request = request.id.clone();
    receipt.request_digest = sha256_hex(request.content.as_bytes());
    receipt.result_digest = Some(sha256_hex(result.content.as_bytes()));
    // A payment is acknowledged after this, by the provider, over the
    // complete sealed receipt.
    let mut unpaid = receipt.clone();
    unpaid.payment = None;
    unpaid.validate()?;
    Ok(receipt)
}

/// Sign the sealed receipt as the buyer, after the provider acknowledged
/// any payment with [`nostr::pylon::acknowledge_payment`].
///
/// # Errors
///
/// When the body is invalid or the signer is not the buyer.
pub fn publish(buyer: &RelaySigner, receipt: &Receipt, now: u64) -> Result<Event> {
    nostr::pylon::receipt_event(buyer, receipt, now)
}

/// Verify a public sealed-job receipt against the ciphertext events it
/// references and the endpoint the reader verified.
///
/// # Errors
///
/// When it is not a sealed receipt, names another endpoint or release, or
/// its digests are not those of the referenced ciphertext.
pub fn verify(
    event: &Event,
    attested: &Attested,
    endpoint_key: &str,
    request: &Event,
    result: &Event,
) -> Result<Receipt> {
    let receipt = nostr::pylon::parse_receipt(event, None)?;
    if receipt.requires != [nostr::att::ATTESTED_FEATURE] {
        return Err("not a sealed-job receipt".into());
    }
    if receipt.meta.as_ref() != Some(&attested.meta()) {
        return Err("the receipt names another endpoint, release, level or measurement".into());
    }
    if request.tag_values("p").next() != Some(endpoint_key) && request.kind != 1_059 {
        return Err("the request was not sent to the attested endpoint".into());
    }
    if result.pubkey != endpoint_key {
        return Err("the result is not signed by the attested endpoint".into());
    }
    if receipt.request != request.id
        || receipt.request_digest != sha256_hex(request.content.as_bytes())
    {
        return Err("the receipt does not reference this request ciphertext".into());
    }
    if receipt.result_digest.as_deref() != Some(sha256_hex(result.content.as_bytes()).as_str()) {
        return Err("the receipt does not reference this result ciphertext".into());
    }
    Ok(receipt)
}

/// Refuse a receipt that would reveal private data: a digest equal to the
/// digest of any of the given plaintexts, or any of them verbatim.
///
/// # Errors
///
/// When the receipt event leaks one.
pub fn check_private(event: &Event, plaintexts: &[&[u8]]) -> Result<()> {
    let wire = serde_json::to_string(event).map_err(|e| e.to_string())?;
    for plaintext in plaintexts {
        if wire.contains(&sha256_hex(plaintext)) {
            return Err("the receipt carries a plaintext digest".into());
        }
        if let Ok(text) = std::str::from_utf8(plaintext) {
            if !text.is_empty() && wire.contains(text) {
                return Err("the receipt carries plaintext".into());
            }
        }
    }
    Ok(())
}
