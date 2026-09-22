use std::{collections::BTreeSet, str::FromStr};

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

/// A validated view of a NIP-09 kind 5 event.
///
/// `k` tags name the kinds the author wants removed. They are optional, and
/// a tombstone does not consult them. A request with no well-formed
/// same-author `e` or `a` reference still parses: it creates no tombstone,
/// and the relay keeps the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionRequest {
    pub request_id: String,
    pub author: String,
    pub created_at: u64,
    pub event_ids: BTreeSet<String>,
    pub addresses: BTreeSet<ReplacementAddress>,
}

impl DeletionRequest {
    /// Extract actionable references. Malformed references and addresses for
    /// another author are ignored; they must never create tombstones.
    pub fn from_event(event: &Event) -> Result<Self, DomainError> {
        if event.kind != 5 {
            return Err(DomainError::NotDeletionRequest);
        }
        let mut event_ids = BTreeSet::new();
        let mut addresses = BTreeSet::new();
        for tag in &event.tags {
            match (tag.name(), tag.value()) {
                (Some("e"), Some(id)) if decode_lower_hex::<32>(id, "deleted event id").is_ok() => {
                    event_ids.insert(id.to_owned());
                }
                (Some("a"), Some(value)) => {
                    if let Ok(address) = ReplacementAddress::from_str(value)
                        && address.pubkey == event.pubkey
                    {
                        addresses.insert(address);
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            request_id: event.id.clone(),
            author: event.pubkey.clone(),
            created_at: event.created_at,
            event_ids,
            addresses,
        })
    }

    pub fn tombstones(&self) -> impl Iterator<Item = DeletionTombstone> + '_ {
        let event_tombstones =
            self.event_ids
                .iter()
                .cloned()
                .map(|event_id| DeletionTombstone::Event {
                    event_id,
                    author: self.author.clone(),
                    request_id: self.request_id.clone(),
                });
        let address_tombstones =
            self.addresses
                .iter()
                .cloned()
                .map(|address| DeletionTombstone::Address {
                    address,
                    through: self.created_at,
                    request_id: self.request_id.clone(),
                });
        event_tombstones.chain(address_tombstones)
    }

    pub fn deletes(&self, event: &Event) -> bool {
        // NIP-59: a request from the reader removes wraps addressed to that
        // reader. The wrap is signed by a one-time key, so the same-author
        // tombstone does not name it.
        super::gift_wrap::recipient_removed_wrap(event, &self.author)
            || self.tombstones().any(|tombstone| tombstone.deletes(event))
    }
}

/// Durable deletion state. Storing this even before the target arrives makes
/// deletion-before-event ordering safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionTombstone {
    Event {
        event_id: String,
        author: String,
        request_id: String,
    },
    Address {
        address: ReplacementAddress,
        through: u64,
        request_id: String,
    },
}

impl DeletionTombstone {
    pub fn deletes(&self, event: &Event) -> bool {
        // NIP-09 explicitly gives deletion requests permanence: a request to
        // delete another request has no effect.
        if event.kind == 5 {
            return false;
        }
        match self {
            Self::Event {
                event_id, author, ..
            } => event.id == *event_id && event.pubkey == *author,
            Self::Address {
                address, through, ..
            } => {
                event.created_at <= *through
                    && event.replacement_address().as_ref() == Some(address)
            }
        }
    }
}
