use secp256k1::{Secp256k1, XOnlyPublicKey, schnorr::Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::expanded::validate_expanded_event;
use super::hex::{decode_lower_hex, encode_lower_hex};
use super::{DomainError, EventClass, ReplacementAddress, TimestampPolicy};

/// Tag names beyond single ASCII letters that this relay indexes for
/// filtering. NIP-WK Work Events (kind 32171) are enumerated through the
/// `work` tag per the pinned `nips/openagents/WK.md` rendering contract.
pub const EXTENDED_INDEXED_TAG_NAMES: &[&str] = &["work"];

/// True when a tag name participates in the relay's tag index: one ASCII
/// letter per NIP-01, or an extended indexed name.
pub fn is_indexed_tag_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (bytes.len() == 1 && bytes[0].is_ascii_alphabetic())
        || EXTENDED_INDEXED_TAG_NAMES.contains(&name)
}

/// A NIP-01 tag. The first element is its name and the second, when present,
/// is its first value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tag(pub Vec<String>);

impl Tag {
    pub fn new(values: Vec<String>) -> Self {
        Self(values)
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn name(&self) -> Option<&str> {
        self.0.first().map(String::as_str)
    }

    pub fn value(&self) -> Option<&str> {
        self.0.get(1).map(String::as_str)
    }

    /// Return the index key and first value for an ASCII-letter tag or an
    /// extended indexed tag name.
    pub fn indexed_pair(&self) -> Option<(&str, &str)> {
        let name = self.name()?;
        if is_indexed_tag_name(name) {
            self.value().map(|value| (name, value))
        } else {
            None
        }
    }
}

/// The event object transmitted by Nostr clients and relays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Tag>,
    pub content: String,
    pub sig: String,
}

impl Event {
    /// Serialize the exact NIP-01 preimage used to calculate an event ID.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DomainError> {
        serde_json::to_vec(&(
            0_u8,
            &self.pubkey,
            self.created_at,
            self.kind,
            &self.tags,
            &self.content,
        ))
        .map_err(|error| DomainError::Serialization(error.to_string()))
    }

    pub fn canonical_json(&self) -> Result<String, DomainError> {
        String::from_utf8(self.canonical_bytes()?)
            .map_err(|error| DomainError::Serialization(error.to_string()))
    }

    pub fn computed_id_bytes(&self) -> Result<[u8; 32], DomainError> {
        Ok(Sha256::digest(self.canonical_bytes()?).into())
    }

    pub fn computed_id(&self) -> Result<String, DomainError> {
        Ok(encode_lower_hex(&self.computed_id_bytes()?))
    }

    /// Validate wire shape and lowercase fixed-width hexadecimal fields.
    pub fn validate_nip01_structure(&self) -> Result<(), DomainError> {
        decode_lower_hex::<32>(&self.id, "id")?;
        decode_lower_hex::<32>(&self.pubkey, "pubkey")?;
        decode_lower_hex::<64>(&self.sig, "sig")?;
        if self.tags.iter().any(|tag| tag.0.is_empty()) {
            return Err(DomainError::EmptyTag);
        }
        Ok(())
    }

    /// Validate NIP-01 structure plus every adopted extension shape.
    pub fn validate_structure(&self) -> Result<(), DomainError> {
        self.validate_nip01_structure()?;
        validate_expanded_event(self)?;
        Ok(())
    }

    pub fn validate_id(&self) -> Result<(), DomainError> {
        let expected = self.computed_id()?;
        if expected == self.id {
            Ok(())
        } else {
            Err(DomainError::EventIdMismatch {
                expected,
                actual: self.id.clone(),
            })
        }
    }

    /// Verify the canonical ID and BIP-340 Schnorr signature.
    pub fn validate_crypto(&self) -> Result<(), DomainError> {
        self.validate_id()?;
        let public_key =
            XOnlyPublicKey::from_byte_array(decode_lower_hex::<32>(&self.pubkey, "pubkey")?)
                .map_err(|_| DomainError::InvalidPublicKey)?;
        let signature = Signature::from_byte_array(decode_lower_hex::<64>(&self.sig, "sig")?);
        let message = decode_lower_hex::<32>(&self.id, "id")?;
        Secp256k1::verification_only()
            .verify_schnorr(&signature, &message, &public_key)
            .map_err(|_| DomainError::InvalidSignature)
    }

    /// Perform all admission-time domain checks that do not require policy
    /// or storage state.
    pub fn validate_at(
        &self,
        now: u64,
        timestamp_policy: TimestampPolicy,
    ) -> Result<(), DomainError> {
        self.validate_structure()?;
        timestamp_policy.validate(self.created_at, now)?;
        if let Some(expiration) = self.expiration()
            && expiration <= now
        {
            return Err(DomainError::ExpiredEvent { expiration, now });
        }
        self.validate_crypto()
    }

    pub fn class(&self) -> EventClass {
        EventClass::from_kind(self.kind)
    }

    pub fn replacement_address(&self) -> Option<ReplacementAddress> {
        ReplacementAddress::from_event(self)
    }

    /// Return the first `d` tag's value. Missing values are the empty string,
    /// as required for addressable events by NIP-01.
    pub fn distinct_parameter(&self) -> Option<&str> {
        if self.class() != EventClass::Addressable {
            return None;
        }
        Some(
            self.tags
                .iter()
                .find(|tag| tag.name() == Some("d"))
                .and_then(Tag::value)
                .unwrap_or(""),
        )
    }

    pub fn indexed_tags(&self) -> impl Iterator<Item = (&str, &str)> {
        self.tags.iter().filter_map(Tag::indexed_pair)
    }

    pub fn tag_values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.tags
            .iter()
            .filter(move |tag| tag.name() == Some(name))
            .filter_map(Tag::value)
    }

    /// Parse the first NIP-40 expiration tag.
    pub fn expiration(&self) -> Option<u64> {
        let tag = self
            .tags
            .iter()
            .find(|tag| tag.name() == Some("expiration"))?;
        tag.value()?.parse::<u64>().ok()
    }

    pub fn is_expired(&self, now: u64) -> bool {
        self.expiration()
            .is_some_and(|expiration| expiration <= now)
    }

    pub fn has_exact_tag(&self, name: &str) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.as_slice().len() == 1 && tag.name() == Some(name))
    }

    pub fn is_protected(&self) -> bool {
        self.has_exact_tag("-")
    }

    pub fn gift_wrap_recipient(&self) -> Option<&str> {
        (self.kind == 1_059)
            .then(|| self.tag_values("p").next())
            .flatten()
    }

    pub fn group_id(&self) -> Option<&str> {
        self.tag_values("h").next()
    }

    pub fn embeds_protected_repost(&self) -> bool {
        matches!(self.kind, 6 | 16)
            && serde_json::from_str::<Self>(&self.content)
                .ok()
                .is_some_and(|event| event.is_protected())
    }
}
