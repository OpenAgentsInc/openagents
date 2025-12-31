//! NIP-57: Lightning Zaps
//!
//! This module implements Lightning Zaps, which enable tipping notes and profiles
//! with Lightning payments via LNURL.
//!
//! Key components:
//! - Zap Request (kind 9734): Request to recipient's Lightning wallet for invoice
//! - Zap Receipt (kind 9735): Confirmation that invoice has been paid
//!
//! ## Protocol Flow
//!
//! 1. Client fetches recipient's LNURL pay endpoint from their profile
//! 2. Client creates and signs a zap request (kind 9734)
//! 3. Zap request is sent to recipient's LNURL callback (not published to relays)
//! 4. Recipient's wallet returns a Lightning invoice
//! 5. Sender pays the invoice
//! 6. Recipient's wallet publishes zap receipt (kind 9735) to specified relays
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/57.md>

use crate::nip01::{Event, Nip01Error};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for zap requests
pub const ZAP_REQUEST_KIND: u16 = 9734;

/// Event kind for zap receipts
pub const ZAP_RECEIPT_KIND: u16 = 9735;

/// Errors that can occur during NIP-57 operations
#[derive(Debug, Error)]
pub enum Nip57Error {
    #[error("invalid zap request: {0}")]
    InvalidZapRequest(String),

    #[error("invalid zap receipt: {0}")]
    InvalidZapReceipt(String),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag value: {0}")]
    InvalidTagValue(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("nip01 error: {0}")]
    Nip01(#[from] Nip01Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("amount mismatch: expected {expected}, got {actual}")]
    AmountMismatch { expected: u64, actual: u64 },
}

/// A zap request (kind 9734)
///
/// Sent to recipient's LNURL callback, not published to relays.
/// Contains payment info and optional message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZapRequest {
    /// The underlying event
    pub event: Event,
    /// Recipient's public key (hex)
    pub recipient_pubkey: String,
    /// Optional event being zapped (hex id)
    pub zapped_event: Option<String>,
    /// Optional addressable event being zapped (NIP-33 coordinate)
    pub zapped_address: Option<String>,
    /// Relays to publish zap receipt to
    pub relays: Vec<String>,
    /// Amount in millisats (optional but recommended)
    pub amount_msats: Option<u64>,
    /// Recipient's LNURL (bech32 encoded, optional but recommended)
    pub lnurl: Option<String>,
    /// Optional message/comment
    pub content: String,
}

impl ZapRequest {
    /// Create a new zap request from an event
    pub fn from_event(event: Event) -> Result<Self, Nip57Error> {
        // Validate kind
        if event.kind != ZAP_REQUEST_KIND {
            return Err(Nip57Error::InvalidZapRequest(format!(
                "expected kind {}, got {}",
                ZAP_REQUEST_KIND, event.kind
            )));
        }

        // Extract required tags
        let mut recipient_pubkey = None;
        let mut zapped_event = None;
        let mut zapped_address = None;
        let mut relays = Vec::new();
        let mut amount_msats = None;
        let mut lnurl = None;

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "p" => {
                    if tag.len() > 1 {
                        if recipient_pubkey.is_some() {
                            return Err(Nip57Error::InvalidZapRequest(
                                "multiple p tags found".to_string(),
                            ));
                        }
                        recipient_pubkey = Some(tag[1].clone());
                    }
                }
                "e" => {
                    if tag.len() > 1 {
                        if zapped_event.is_some() {
                            return Err(Nip57Error::InvalidZapRequest(
                                "multiple e tags found".to_string(),
                            ));
                        }
                        zapped_event = Some(tag[1].clone());
                    }
                }
                "a" => {
                    if tag.len() > 1 {
                        if zapped_address.is_some() {
                            return Err(Nip57Error::InvalidZapRequest(
                                "multiple a tags found".to_string(),
                            ));
                        }
                        zapped_address = Some(tag[1].clone());
                    }
                }
                "relays" => {
                    // Relays are all elements after the first
                    relays.extend(tag.iter().skip(1).cloned());
                }
                "amount" => {
                    if tag.len() > 1 {
                        amount_msats = Some(tag[1].parse().map_err(|_| {
                            Nip57Error::InvalidTagValue(format!("invalid amount: {}", tag[1]))
                        })?);
                    }
                }
                "lnurl" => {
                    if tag.len() > 1 {
                        lnurl = Some(tag[1].clone());
                    }
                }
                _ => {}
            }
        }

        // Recipient pubkey is required
        let recipient_pubkey = recipient_pubkey
            .ok_or_else(|| Nip57Error::MissingTag("p tag (recipient pubkey)".to_string()))?;

        Ok(Self {
            content: event.content.clone(),
            event,
            recipient_pubkey,
            zapped_event,
            zapped_address,
            relays,
            amount_msats,
            lnurl,
        })
    }

    /// Validate the zap request according to NIP-57 Appendix D
    pub fn validate(&self) -> Result<(), Nip57Error> {
        // Must have valid signature (checked during event creation)

        // Must have tags
        if self.event.tags.is_empty() {
            return Err(Nip57Error::Validation("event must have tags".to_string()));
        }

        // Must have exactly one p tag (already validated in from_event)

        // Must have 0 or 1 e tags (already validated in from_event)

        // Should have relays tag
        if self.relays.is_empty() {
            return Err(Nip57Error::Validation(
                "relays tag should be present".to_string(),
            ));
        }

        Ok(())
    }
}

/// A zap receipt (kind 9735)
///
/// Created by recipient's Lightning wallet after invoice is paid.
/// Published to relays specified in the zap request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZapReceipt {
    /// The underlying event
    pub event: Event,
    /// Recipient's public key (from original zap request)
    pub recipient_pubkey: String,
    /// Sender's public key (from original zap request)
    pub sender_pubkey: Option<String>,
    /// Optional event that was zapped
    pub zapped_event: Option<String>,
    /// Optional addressable event that was zapped
    pub zapped_address: Option<String>,
    /// Kind of the zapped event
    pub zapped_kind: Option<u16>,
    /// BOLT-11 invoice that was paid
    pub bolt11: String,
    /// JSON-encoded zap request
    pub description: String,
    /// Optional payment preimage
    pub preimage: Option<String>,
}

impl ZapReceipt {
    /// Create a new zap receipt from an event
    pub fn from_event(event: Event) -> Result<Self, Nip57Error> {
        // Validate kind
        if event.kind != ZAP_RECEIPT_KIND {
            return Err(Nip57Error::InvalidZapReceipt(format!(
                "expected kind {}, got {}",
                ZAP_RECEIPT_KIND, event.kind
            )));
        }

        // Extract required tags
        let mut recipient_pubkey = None;
        let mut sender_pubkey = None;
        let mut zapped_event = None;
        let mut zapped_address = None;
        let mut zapped_kind = None;
        let mut bolt11 = None;
        let mut description = None;
        let mut preimage = None;

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "p" => {
                    if tag.len() > 1 {
                        recipient_pubkey = Some(tag[1].clone());
                    }
                }
                "P" => {
                    if tag.len() > 1 {
                        sender_pubkey = Some(tag[1].clone());
                    }
                }
                "e" => {
                    if tag.len() > 1 {
                        zapped_event = Some(tag[1].clone());
                    }
                }
                "a" => {
                    if tag.len() > 1 {
                        zapped_address = Some(tag[1].clone());
                    }
                }
                "k" => {
                    if tag.len() > 1 {
                        zapped_kind = Some(tag[1].parse().map_err(|_| {
                            Nip57Error::InvalidTagValue(format!("invalid kind: {}", tag[1]))
                        })?);
                    }
                }
                "bolt11" => {
                    if tag.len() > 1 {
                        bolt11 = Some(tag[1].clone());
                    }
                }
                "description" => {
                    if tag.len() > 1 {
                        description = Some(tag[1].clone());
                    }
                }
                "preimage" => {
                    if tag.len() > 1 {
                        preimage = Some(tag[1].clone());
                    }
                }
                _ => {}
            }
        }

        // Validate required fields
        let recipient_pubkey = recipient_pubkey
            .ok_or_else(|| Nip57Error::MissingTag("p tag (recipient pubkey)".to_string()))?;

        let bolt11 = bolt11.ok_or_else(|| Nip57Error::MissingTag("bolt11 tag".to_string()))?;

        let description =
            description.ok_or_else(|| Nip57Error::MissingTag("description tag".to_string()))?;

        Ok(Self {
            event,
            recipient_pubkey,
            sender_pubkey,
            zapped_event,
            zapped_address,
            zapped_kind,
            bolt11,
            description,
            preimage,
        })
    }

    /// Get the zap request from the description field
    pub fn get_zap_request(&self) -> Result<ZapRequest, Nip57Error> {
        let event: Event = serde_json::from_str(&self.description)?;
        ZapRequest::from_event(event)
    }

    /// Validate the zap receipt according to NIP-57 Appendix F
    ///
    /// # Arguments
    /// * `lnurl_pubkey` - The recipient's LNURL provider's nostr pubkey
    /// * `expected_amount_msats` - Optional expected amount in millisats
    /// * `expected_lnurl` - Optional expected LNURL
    pub fn validate(
        &self,
        lnurl_pubkey: &str,
        expected_amount_msats: Option<u64>,
        expected_lnurl: Option<&str>,
    ) -> Result<(), Nip57Error> {
        // The zap receipt event's pubkey must match the LNURL provider's pubkey
        if self.event.pubkey != lnurl_pubkey {
            return Err(Nip57Error::Validation(format!(
                "zap receipt pubkey {} doesn't match lnurl provider pubkey {}",
                self.event.pubkey, lnurl_pubkey
            )));
        }

        // Parse and validate the zap request from description
        let zap_request = self.get_zap_request()?;
        zap_request.validate()?;

        // If amount is expected, validate it matches
        if let Some(expected) = expected_amount_msats {
            if let Some(actual) = zap_request.amount_msats {
                if expected != actual {
                    return Err(Nip57Error::AmountMismatch { expected, actual });
                }
            }
        }

        // If lnurl is expected, validate it matches
        if let Some(expected) = expected_lnurl {
            if let Some(actual) = &zap_request.lnurl {
                if expected != actual {
                    return Err(Nip57Error::Validation(format!(
                        "lnurl mismatch: expected {}, got {}",
                        expected, actual
                    )));
                }
            }
        }

        Ok(())
    }
}

/// User-configurable zap defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZapSettings {
    /// Default zap amount in millisats.
    pub default_amount_msats: u64,
}

impl ZapSettings {
    /// Create new zap settings with a default amount.
    pub fn new(default_amount_msats: u64) -> Self {
        Self {
            default_amount_msats,
        }
    }

    /// Resolve the zap amount using an override when provided.
    pub fn resolve_amount_msats(&self, override_amount_msats: Option<u64>) -> u64 {
        override_amount_msats.unwrap_or(self.default_amount_msats)
    }
}

/// Count zap receipts for a specific event ID.
pub fn count_zaps_for_event(receipts: &[ZapReceipt], event_id: &str) -> usize {
    receipts
        .iter()
        .filter(|receipt| receipt.zapped_event.as_deref() == Some(event_id))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example zap request from NIP-57
    fn example_zap_request() -> Event {
        Event {
            kind: 9734,
            content: "Zap!".to_string(),
            tags: vec![
                vec![
                    "relays".to_string(),
                    "wss://nostr-pub.wellorder.com".to_string(),
                    "wss://anotherrelay.example.com".to_string(),
                ],
                vec!["amount".to_string(), "21000".to_string()],
                vec![
                    "lnurl".to_string(),
                    "lnurl1dp68gurn8ghj7um5v93kketj9ehx2amn9uh8wetvdskkkmn0wahz7mrww4excup0dajx2mrv92x9xp".to_string(),
                ],
                vec![
                    "p".to_string(),
                    "04c915daefee38317fa734444acee390a8269fe5810b2241e5e6dd343dfbecc9".to_string(),
                ],
                vec![
                    "e".to_string(),
                    "9ae37aa68f48645127299e9453eb5d908a0cbb6058ff340d528ed4d37c8994fb".to_string(),
                ],
                vec!["k".to_string(), "1".to_string()],
            ],
            pubkey: "97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad98e322".to_string(),
            created_at: 1679673265,
            id: "30efed56a035b2549fcaeec0bf2c1595f9a9b3bb4b1a38abaf8ee9041c4b7d93".to_string(),
            sig: "f2cb581a84ed10e4dc84937bd98e27acac71ab057255f6aa8dfa561808c981fe8870f4a03c1e3666784d82a9c802d3704e174371aa13d63e2aeaf24ff5374d9d".to_string(),
        }
    }

    fn zap_receipt_event(zapped_event_id: &str, receipt_id: &str) -> Event {
        let description = serde_json::to_string(&example_zap_request()).unwrap();

        Event {
            id: receipt_id.to_string(),
            pubkey: "9630f464cca6a5147aa8a35f0bcdd3ce485324e732fd39e09233b1d848238f31".to_string(),
            created_at: 1674164545,
            kind: 9735,
            tags: vec![
                vec![
                    "p".to_string(),
                    "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245".to_string(),
                ],
                vec![
                    "P".to_string(),
                    "97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad98e322".to_string(),
                ],
                vec!["e".to_string(), zapped_event_id.to_string()],
                vec!["k".to_string(), "1".to_string()],
                vec!["bolt11".to_string(), "lnbc10u1test".to_string()],
                vec!["description".to_string(), description],
            ],
            content: "".to_string(),
            sig: "...".to_string(),
        }
    }

    #[test]
    fn test_zap_request_from_event() {
        let event = example_zap_request();
        let zap_req = ZapRequest::from_event(event).expect("should parse zap request");

        assert_eq!(
            zap_req.recipient_pubkey,
            "04c915daefee38317fa734444acee390a8269fe5810b2241e5e6dd343dfbecc9"
        );
        assert_eq!(
            zap_req.zapped_event,
            Some("9ae37aa68f48645127299e9453eb5d908a0cbb6058ff340d528ed4d37c8994fb".to_string())
        );
        assert_eq!(zap_req.amount_msats, Some(21000));
        assert_eq!(zap_req.content, "Zap!");
        assert_eq!(zap_req.relays.len(), 2);
    }

    #[test]
    fn test_zap_request_profile_only() {
        let mut event = example_zap_request();
        event.tags.retain(|tag| tag[0] != "e");

        let zap_req = ZapRequest::from_event(event).expect("should parse zap request");
        assert_eq!(
            zap_req.recipient_pubkey,
            "04c915daefee38317fa734444acee390a8269fe5810b2241e5e6dd343dfbecc9"
        );
        assert!(zap_req.zapped_event.is_none());
        zap_req.validate().expect("should validate");
    }

    #[test]
    fn test_zap_request_validation() {
        let event = example_zap_request();
        let zap_req = ZapRequest::from_event(event).expect("should parse zap request");
        zap_req.validate().expect("should validate");
    }

    #[test]
    fn test_zap_request_missing_p_tag() {
        let mut event = example_zap_request();
        event.tags.retain(|tag| tag[0] != "p");

        let result = ZapRequest::from_event(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_zap_request_wrong_kind() {
        let mut event = example_zap_request();
        event.kind = 1;

        let result = ZapRequest::from_event(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_zap_settings_resolve_amount() {
        let settings = ZapSettings::new(21000);

        assert_eq!(settings.resolve_amount_msats(None), 21000);
        assert_eq!(settings.resolve_amount_msats(Some(5000)), 5000);
    }

    #[test]
    fn test_count_zaps_for_event() {
        let target_event = "9ae37aa68f48645127299e9453eb5d908a0cbb6058ff340d528ed4d37c8994fb";
        let other_event = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";

        let receipt_target = ZapReceipt::from_event(zap_receipt_event(target_event, "receipt1"))
            .expect("should parse receipt");
        let receipt_other = ZapReceipt::from_event(zap_receipt_event(other_event, "receipt2"))
            .expect("should parse receipt");

        let receipts = vec![receipt_target, receipt_other];

        assert_eq!(count_zaps_for_event(&receipts, target_event), 1);
        assert_eq!(count_zaps_for_event(&receipts, other_event), 1);
        assert_eq!(count_zaps_for_event(&receipts, "missing"), 0);
    }

    #[test]
    fn test_zap_receipt_from_event() {
        let description = serde_json::to_string(&example_zap_request()).unwrap();

        let event = Event {
            id: "67b48a14fb66c60c8f9070bdeb37afdfcc3d08ad01989460448e4081eddda446".to_string(),
            pubkey: "9630f464cca6a5147aa8a35f0bcdd3ce485324e732fd39e09233b1d848238f31".to_string(),
            created_at: 1674164545,
            kind: 9735,
            tags: vec![
                vec![
                    "p".to_string(),
                    "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245".to_string(),
                ],
                vec![
                    "P".to_string(),
                    "97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad98e322".to_string(),
                ],
                vec![
                    "e".to_string(),
                    "3624762a1274dd9636e0c552b53086d70bc88c165bc4dc0f9e836a1eaf86c3b8".to_string(),
                ],
                vec!["k".to_string(), "1".to_string()],
                vec![
                    "bolt11".to_string(),
                    "lnbc10u1p3unwfusp5t9r3yymhpfqculx78u027lxspgxcr2n2987mx2j55nnfs95nxnzqpp5jmrh92pfld78spqs78v9euf2385t83uvpwk9ldrlvf6ch7tpascqhp5zvkrmemgth3tufcvflmzjzfvjt023nazlhljz2n9hattj4f8jq8qxqyjw5qcqpjrzjqtc4fc44feggv7065fqe5m4ytjarg3repr5j9el35xhmtfexc42yczarjuqqfzqqqqqqqqlgqqqqqqgq9q9qxpqysgq079nkq507a5tw7xgttmj4u990j7wfggtrasah5gd4ywfr2pjcn29383tphp4t48gquelz9z78p4cq7ml3nrrphw5w6eckhjwmhezhnqpy6gyf0".to_string(),
                ],
                vec!["description".to_string(), description],
                vec![
                    "preimage".to_string(),
                    "5d006d2cf1e73c7148e7519a4c68adc81642ce0e25a432b2434c99f97344c15f".to_string(),
                ],
            ],
            content: "".to_string(),
            sig: "...".to_string(),
        };

        let zap_receipt = ZapReceipt::from_event(event).expect("should parse zap receipt");

        assert_eq!(
            zap_receipt.recipient_pubkey,
            "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245"
        );
        assert_eq!(
            zap_receipt.sender_pubkey,
            Some("97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad98e322".to_string())
        );
        assert_eq!(zap_receipt.zapped_kind, Some(1));
        assert!(zap_receipt.bolt11.starts_with("lnbc"));
        assert!(zap_receipt.preimage.is_some());
    }

    #[test]
    fn test_zap_receipt_get_request() {
        let description = serde_json::to_string(&example_zap_request()).unwrap();

        let event = Event {
            id: "67b48a14fb66c60c8f9070bdeb37afdfcc3d08ad01989460448e4081eddda446".to_string(),
            pubkey: "9630f464cca6a5147aa8a35f0bcdd3ce485324e732fd39e09233b1d848238f31".to_string(),
            created_at: 1674164545,
            kind: 9735,
            tags: vec![
                vec![
                    "p".to_string(),
                    "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245".to_string(),
                ],
                vec!["bolt11".to_string(), "lnbc...".to_string()],
                vec!["description".to_string(), description],
            ],
            content: "".to_string(),
            sig: "...".to_string(),
        };

        let zap_receipt = ZapReceipt::from_event(event).expect("should parse zap receipt");
        let zap_request = zap_receipt
            .get_zap_request()
            .expect("should get zap request");

        assert_eq!(zap_request.amount_msats, Some(21000));
        assert_eq!(zap_request.content, "Zap!");
    }

    #[test]
    fn test_zap_receipt_missing_bolt11() {
        let description = serde_json::to_string(&example_zap_request()).unwrap();

        let event = Event {
            id: "67b48a14fb66c60c8f9070bdeb37afdfcc3d08ad01989460448e4081eddda446".to_string(),
            pubkey: "9630f464cca6a5147aa8a35f0bcdd3ce485324e732fd39e09233b1d848238f31".to_string(),
            created_at: 1674164545,
            kind: 9735,
            tags: vec![
                vec![
                    "p".to_string(),
                    "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245".to_string(),
                ],
                vec!["description".to_string(), description],
            ],
            content: "".to_string(),
            sig: "...".to_string(),
        };

        let result = ZapReceipt::from_event(event);
        assert!(result.is_err());
    }
}
