//! NIP-77: Negentropy Syncing
//!
//! This module implements the Negentropy protocol for efficient event syncing between
//! Nostr clients and relays. Negentropy uses Range-Based Set Reconciliation (RBSR) to
//! minimize bandwidth when both sides have events in common.
//!
//! Internal module boundaries:
//! - `error`: shared protocol error and result types
//! - `codec`: varint codec primitives
//! - `model`: protocol wire/model structures and message encoding
//! - `reconciliation`: stateful range reconciliation algorithm
//! - `tests`: codec/model/reconciliation coverage
//!
//! # Overview
//!
//! Traditional Nostr syncing requires transferring all event IDs to determine differences.
//! Negentropy is more efficient when:
//! - Both sides have most events in common (typical for active users)
//! - Event sets are large (thousands of events)
//! - Bandwidth is constrained
//!
//! The protocol works by:
//! 1. Dividing the timestamp/ID space into ranges
//! 2. Computing fingerprints (hashes) for each range
//! 3. Exchanging only ranges with different fingerprints
//! 4. Recursively subdividing ranges until individual IDs are identified
//!
//! # Protocol Flow
//!
//! ```text
//! Client                                  Relay
//!   |                                       |
//!   |  NEG-OPEN (filter, initial msg) ---→ |
//!   |                                       | (compute fingerprints)
//!   | ←--- NEG-MSG (ranges with fps)       |
//!   |                                       |
//!   | (compare fingerprints)                |
//!   |  NEG-MSG (refined ranges) ---------→ |
//!   |                                       |
//!   | ←--- NEG-MSG (more refined)          |
//!   |  ...continues until complete...      |
//!   |                                       |
//!   |  NEG-CLOSE ------------------------→ |
//! ```
//!
//! After sync completes, client knows:
//! - IDs it has that relay needs (upload with EVENT)
//! - IDs relay has that it needs (download with REQ)
//!
//! # Usage Example
//!
//! ```
//! use nostr::{Record, NegentropyMessage, Range, Bound, calculate_fingerprint, sort_records};
//!
//! // Prepare local event set
//! let mut records = vec![
//!     Record::new(1000, [0x01; 32]),
//!     Record::new(2000, [0x02; 32]),
//!     Record::new(3000, [0x03; 32]),
//! ];
//! sort_records(&mut records);
//!
//! // Create initial message covering all events
//! let ids: Vec<_> = records.iter().map(|r| r.id).collect();
//! let fp = calculate_fingerprint(&ids);
//!
//! let initial_range = Range::fingerprint(
//!     Bound::infinity(),
//!     fp
//! );
//! let message = NegentropyMessage::new(vec![initial_range]);
//!
//! // Encode for transmission
//! let hex_message = message.encode_hex().unwrap();
//! println!("Initial message: {}", hex_message);
//!
//! // Server would decode and compare fingerprints
//! let decoded = NegentropyMessage::decode_hex(&hex_message).unwrap();
//! assert_eq!(decoded.ranges.len(), 1);
//! ```
//!
//! # Performance Characteristics
//!
//! - **Time Complexity**: O(log N) round trips for N events
//! - **Bandwidth**: O(d log N) where d is the number of differences
//! - **Best Case**: Both sides identical = 1 round trip
//! - **Worst Case**: Completely different = O(N) (falls back to ID list)
//!
//! # References
//!
//! - NIP-77: <https://github.com/nostr-protocol/nips/blob/master/77.md>
//! - Negentropy Protocol: <https://github.com/hoytech/negentropy>
//! - RBSR Paper: <https://logperiodic.com/rbsr.html>

mod codec;
mod error;
mod model;
mod reconciliation;

pub use codec::{decode_varint, encode_varint};
pub use error::{Nip77Error, Result};
pub use model::{
    Bound, EventId, NegClose, NegErr, NegMsg, NegOpen, NegentropyMessage, PROTOCOL_VERSION_1,
    Range, RangeMode, RangePayload, Record, TIMESTAMP_INFINITY, calculate_fingerprint,
    sort_records,
};
pub use reconciliation::ReconciliationState;

#[cfg(test)]
mod tests;
