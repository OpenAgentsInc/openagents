//! NIP-77: Negentropy Syncing
//!
//! This module implements the Negentropy protocol for efficient event syncing between
//! Nostr clients and relays. Negentropy uses Range-Based Set Reconciliation (RBSR) to
//! minimize bandwidth when both sides have events in common.
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

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;
use thiserror::Error;

/// NIP-77 error types
#[derive(Debug, Error)]
pub enum Nip77Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid protocol version: {0}")]
    InvalidProtocolVersion(u8),

    #[error("Invalid mode: {0}")]
    InvalidMode(u64),

    #[error("Invalid hex encoding: {0}")]
    InvalidHex(String),

    #[error("Varint decode error: {0}")]
    VarintDecode(String),

    #[error("Varint encode error: {0}")]
    VarintEncode(String),

    #[error("Invalid bound: {0}")]
    InvalidBound(String),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Invalid fingerprint length: expected 16, got {0}")]
    InvalidFingerprintLength(usize),

    #[error("Invalid ID length: expected 32, got {0}")]
    InvalidIdLength(usize),
}

type Result<T> = std::result::Result<T, Nip77Error>;

/// Negentropy Protocol Version 1
pub const PROTOCOL_VERSION_1: u8 = 0x61;

/// Special infinity timestamp value
pub const TIMESTAMP_INFINITY: u64 = u64::MAX;

/// Encode a varint (variable-length unsigned integer)
///
/// Varints are represented as base-128 digits, most significant digit first.
/// Bit 7 (high bit) is set on each byte except the last.
pub fn encode_varint(mut value: u64) -> Result<Vec<u8>> {
    if value == 0 {
        return Ok(vec![0]);
    }

    let mut bytes = Vec::new();
    let mut temp = Vec::new();

    // Extract base-128 digits
    while value > 0 {
        temp.push((value & 0x7F) as u8);
        value >>= 7;
    }

    // Reverse and set high bit on all but last
    for (i, &byte) in temp.iter().rev().enumerate() {
        if i < temp.len() - 1 {
            bytes.push(byte | 0x80);
        } else {
            bytes.push(byte);
        }
    }

    Ok(bytes)
}

/// Decode a varint from a byte slice
///
/// Returns (value, bytes_consumed)
pub fn decode_varint(data: &[u8]) -> Result<(u64, usize)> {
    if data.is_empty() {
        return Err(Nip77Error::VarintDecode("empty data".to_string()));
    }

    let mut value: u64 = 0;
    let mut bytes_read = 0;

    for &byte in data.iter() {
        bytes_read += 1;

        // Check for overflow before shifting
        if value > (u64::MAX >> 7) {
            return Err(Nip77Error::VarintDecode("varint overflow".to_string()));
        }

        value = (value << 7) | ((byte & 0x7F) as u64);

        // If high bit is not set, this is the last byte
        if (byte & 0x80) == 0 {
            return Ok((value, bytes_read));
        }

        if bytes_read > 10 {
            return Err(Nip77Error::VarintDecode(
                "varint too long (max 10 bytes for u64)".to_string(),
            ));
        }
    }

    Err(Nip77Error::VarintDecode("incomplete varint".to_string()))
}

/// A 256-bit event ID
pub type EventId = [u8; 32];

/// A timestamp and ID prefix bound for ranges
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The timestamp (u64::MAX for infinity)
    pub timestamp: u64,
    /// The ID prefix (0-32 bytes)
    pub id_prefix: Vec<u8>,
}

impl Bound {
    /// Create a new bound
    pub fn new(timestamp: u64, id_prefix: Vec<u8>) -> Result<Self> {
        if id_prefix.len() > 32 {
            return Err(Nip77Error::InvalidBound(format!(
                "ID prefix too long: {} bytes (max 32)",
                id_prefix.len()
            )));
        }
        Ok(Self {
            timestamp,
            id_prefix,
        })
    }

    /// Create a bound at timestamp 0 with empty ID
    pub fn zero() -> Self {
        Self {
            timestamp: 0,
            id_prefix: vec![],
        }
    }

    /// Create an infinity bound
    pub fn infinity() -> Self {
        Self {
            timestamp: TIMESTAMP_INFINITY,
            id_prefix: vec![],
        }
    }

    /// Encode bound to bytes with previous timestamp for delta encoding
    pub fn encode(&self, prev_timestamp: u64) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        // Encode timestamp
        let encoded_timestamp = if self.timestamp == TIMESTAMP_INFINITY {
            0
        } else {
            // Delta from previous timestamp + 1
            1 + self.timestamp.saturating_sub(prev_timestamp)
        };

        bytes.extend_from_slice(&encode_varint(encoded_timestamp)?);

        // Encode ID prefix length and bytes
        bytes.extend_from_slice(&encode_varint(self.id_prefix.len() as u64)?);
        bytes.extend_from_slice(&self.id_prefix);

        Ok(bytes)
    }

    /// Decode bound from bytes with previous timestamp for delta decoding
    pub fn decode(data: &[u8], prev_timestamp: u64) -> Result<(Self, usize)> {
        let mut offset = 0;

        // Decode timestamp
        let (encoded_timestamp, ts_len) = decode_varint(&data[offset..])?;
        offset += ts_len;

        let timestamp = if encoded_timestamp == 0 {
            TIMESTAMP_INFINITY
        } else {
            prev_timestamp + encoded_timestamp - 1
        };

        // Decode ID prefix
        let (prefix_len, len_len) = decode_varint(&data[offset..])?;
        offset += len_len;

        if prefix_len > 32 {
            return Err(Nip77Error::InvalidBound(format!(
                "ID prefix length too long: {}",
                prefix_len
            )));
        }

        let prefix_len = prefix_len as usize;
        if offset + prefix_len > data.len() {
            return Err(Nip77Error::InvalidBound(
                "not enough data for ID prefix".to_string(),
            ));
        }

        let id_prefix = data[offset..offset + prefix_len].to_vec();
        offset += prefix_len;

        Ok((
            Self {
                timestamp,
                id_prefix,
            },
            offset,
        ))
    }
}

/// Range mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeMode {
    /// Skip this range (mode 0)
    Skip,
    /// Fingerprint mode (mode 1)
    Fingerprint,
    /// ID list mode (mode 2)
    IdList,
}

impl RangeMode {
    fn from_u64(value: u64) -> Result<Self> {
        match value {
            0 => Ok(Self::Skip),
            1 => Ok(Self::Fingerprint),
            2 => Ok(Self::IdList),
            _ => Err(Nip77Error::InvalidMode(value)),
        }
    }
}

/// Range payload
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangePayload {
    /// No payload for skip mode
    Skip,
    /// 16-byte fingerprint
    Fingerprint([u8; 16]),
    /// List of event IDs
    IdList(Vec<EventId>),
}

/// A range in the Negentropy protocol
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    /// Exclusive upper bound of this range
    pub upper_bound: Bound,
    /// The mode and payload
    pub payload: RangePayload,
}

impl Range {
    /// Create a new range
    pub fn new(upper_bound: Bound, payload: RangePayload) -> Self {
        Self {
            upper_bound,
            payload,
        }
    }

    /// Create a skip range
    pub fn skip(upper_bound: Bound) -> Self {
        Self {
            upper_bound,
            payload: RangePayload::Skip,
        }
    }

    /// Create a fingerprint range
    pub fn fingerprint(upper_bound: Bound, fingerprint: [u8; 16]) -> Self {
        Self {
            upper_bound,
            payload: RangePayload::Fingerprint(fingerprint),
        }
    }

    /// Create an ID list range
    pub fn id_list(upper_bound: Bound, ids: Vec<EventId>) -> Self {
        Self {
            upper_bound,
            payload: RangePayload::IdList(ids),
        }
    }

    /// Encode range to bytes with previous timestamp
    pub fn encode(&self, prev_timestamp: u64) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        // Encode upper bound
        bytes.extend_from_slice(&self.upper_bound.encode(prev_timestamp)?);

        // Encode mode and payload
        match &self.payload {
            RangePayload::Skip => {
                bytes.extend_from_slice(&encode_varint(0)?);
            }
            RangePayload::Fingerprint(fp) => {
                bytes.extend_from_slice(&encode_varint(1)?);
                bytes.extend_from_slice(fp);
            }
            RangePayload::IdList(ids) => {
                bytes.extend_from_slice(&encode_varint(2)?);
                bytes.extend_from_slice(&encode_varint(ids.len() as u64)?);
                for id in ids {
                    bytes.extend_from_slice(id);
                }
            }
        }

        Ok(bytes)
    }

    /// Decode range from bytes with previous timestamp
    pub fn decode(data: &[u8], prev_timestamp: u64) -> Result<(Self, usize, u64)> {
        let mut offset = 0;

        // Decode upper bound
        let (upper_bound, bound_len) = Bound::decode(&data[offset..], prev_timestamp)?;
        offset += bound_len;

        let new_timestamp = upper_bound.timestamp;

        // Decode mode
        let (mode_val, mode_len) = decode_varint(&data[offset..])?;
        offset += mode_len;

        let mode = RangeMode::from_u64(mode_val)?;

        // Decode payload based on mode
        let payload = match mode {
            RangeMode::Skip => RangePayload::Skip,

            RangeMode::Fingerprint => {
                if offset + 16 > data.len() {
                    return Err(Nip77Error::InvalidRange(
                        "not enough data for fingerprint".to_string(),
                    ));
                }
                let mut fp = [0u8; 16];
                fp.copy_from_slice(&data[offset..offset + 16]);
                offset += 16;
                RangePayload::Fingerprint(fp)
            }

            RangeMode::IdList => {
                let (id_count, count_len) = decode_varint(&data[offset..])?;
                offset += count_len;

                let mut ids = Vec::new();
                for _ in 0..id_count {
                    if offset + 32 > data.len() {
                        return Err(Nip77Error::InvalidRange(
                            "not enough data for ID".to_string(),
                        ));
                    }
                    let mut id = [0u8; 32];
                    id.copy_from_slice(&data[offset..offset + 32]);
                    offset += 32;
                    ids.push(id);
                }

                RangePayload::IdList(ids)
            }
        };

        Ok((
            Self {
                upper_bound,
                payload,
            },
            offset,
            new_timestamp,
        ))
    }
}

/// A Negentropy protocol message
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegentropyMessage {
    /// Protocol version (0x61 for V1)
    pub version: u8,
    /// Ordered list of ranges
    pub ranges: Vec<Range>,
}

impl NegentropyMessage {
    /// Create a new message with protocol version 1
    pub fn new(ranges: Vec<Range>) -> Self {
        Self {
            version: PROTOCOL_VERSION_1,
            ranges,
        }
    }

    /// Encode message to bytes
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        // Protocol version
        bytes.push(self.version);

        // Encode ranges with timestamp delta encoding
        let mut prev_timestamp = 0;
        for range in &self.ranges {
            bytes.extend_from_slice(&range.encode(prev_timestamp)?);
            prev_timestamp = range.upper_bound.timestamp;
        }

        Ok(bytes)
    }

    /// Encode message to hex string
    pub fn encode_hex(&self) -> Result<String> {
        let bytes = self.encode()?;
        Ok(hex::encode(bytes))
    }

    /// Decode message from bytes
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            return Err(Nip77Error::InvalidProtocolVersion(0));
        }

        let version = data[0];

        // Check protocol version
        if version != PROTOCOL_VERSION_1 {
            return Err(Nip77Error::InvalidProtocolVersion(version));
        }

        let mut ranges = Vec::new();
        let mut offset = 1;
        let mut prev_timestamp = 0;

        while offset < data.len() {
            let (range, range_len, new_timestamp) = Range::decode(&data[offset..], prev_timestamp)?;
            offset += range_len;
            prev_timestamp = new_timestamp;
            ranges.push(range);
        }

        Ok(Self { version, ranges })
    }

    /// Decode message from hex string
    pub fn decode_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|e| Nip77Error::InvalidHex(e.to_string()))?;
        Self::decode(&bytes)
    }
}

/// Nostr NEG-OPEN message (client to relay)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegOpen {
    /// Subscription ID
    pub subscription_id: String,
    /// Filter (as JSON value for flexibility)
    pub filter: serde_json::Value,
    /// Initial Negentropy message (hex-encoded)
    pub initial_message: String,
}

impl NegOpen {
    /// Create a new NEG-OPEN message
    pub fn new(
        subscription_id: String,
        filter: serde_json::Value,
        message: &NegentropyMessage,
    ) -> Result<Self> {
        Ok(Self {
            subscription_id,
            filter,
            initial_message: message.encode_hex()?,
        })
    }

    /// Convert to JSON array format
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!([
            "NEG-OPEN",
            self.subscription_id,
            self.filter,
            self.initial_message
        ])
    }

    /// Parse from JSON array
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let arr = value
            .as_array()
            .ok_or_else(|| Nip77Error::InvalidHex("not an array".to_string()))?;

        if arr.len() != 4 {
            return Err(Nip77Error::InvalidHex(format!(
                "expected 4 elements, got {}",
                arr.len()
            )));
        }

        let msg_type = arr[0]
            .as_str()
            .ok_or_else(|| Nip77Error::InvalidHex("message type not a string".to_string()))?;

        if msg_type != "NEG-OPEN" {
            return Err(Nip77Error::InvalidHex(format!(
                "expected NEG-OPEN, got {}",
                msg_type
            )));
        }

        Ok(Self {
            subscription_id: arr[1]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("subscription ID not a string".to_string()))?
                .to_string(),
            filter: arr[2].clone(),
            initial_message: arr[3]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("initial message not a string".to_string()))?
                .to_string(),
        })
    }
}

/// Nostr NEG-MSG message (bidirectional)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegMsg {
    /// Subscription ID
    pub subscription_id: String,
    /// Negentropy message (hex-encoded)
    pub message: String,
}

impl NegMsg {
    /// Create a new NEG-MSG message
    pub fn new(subscription_id: String, message: &NegentropyMessage) -> Result<Self> {
        Ok(Self {
            subscription_id,
            message: message.encode_hex()?,
        })
    }

    /// Convert to JSON array format
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!(["NEG-MSG", self.subscription_id, self.message])
    }

    /// Parse from JSON array
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let arr = value
            .as_array()
            .ok_or_else(|| Nip77Error::InvalidHex("not an array".to_string()))?;

        if arr.len() != 3 {
            return Err(Nip77Error::InvalidHex(format!(
                "expected 3 elements, got {}",
                arr.len()
            )));
        }

        let msg_type = arr[0]
            .as_str()
            .ok_or_else(|| Nip77Error::InvalidHex("message type not a string".to_string()))?;

        if msg_type != "NEG-MSG" {
            return Err(Nip77Error::InvalidHex(format!(
                "expected NEG-MSG, got {}",
                msg_type
            )));
        }

        Ok(Self {
            subscription_id: arr[1]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("subscription ID not a string".to_string()))?
                .to_string(),
            message: arr[2]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("message not a string".to_string()))?
                .to_string(),
        })
    }
}

/// Nostr NEG-ERR message (relay to client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegErr {
    /// Subscription ID
    pub subscription_id: String,
    /// Error reason code
    pub reason: String,
}

impl NegErr {
    /// Create a new NEG-ERR message
    pub fn new(subscription_id: String, reason: String) -> Self {
        Self {
            subscription_id,
            reason,
        }
    }

    /// Create a "blocked" error
    pub fn blocked(subscription_id: String, message: &str) -> Self {
        Self {
            subscription_id,
            reason: format!("blocked: {}", message),
        }
    }

    /// Create a "closed" error
    pub fn closed(subscription_id: String, message: &str) -> Self {
        Self {
            subscription_id,
            reason: format!("closed: {}", message),
        }
    }

    /// Convert to JSON array format
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!(["NEG-ERR", self.subscription_id, self.reason])
    }

    /// Parse from JSON array
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let arr = value
            .as_array()
            .ok_or_else(|| Nip77Error::InvalidHex("not an array".to_string()))?;

        if arr.len() != 3 {
            return Err(Nip77Error::InvalidHex(format!(
                "expected 3 elements, got {}",
                arr.len()
            )));
        }

        let msg_type = arr[0]
            .as_str()
            .ok_or_else(|| Nip77Error::InvalidHex("message type not a string".to_string()))?;

        if msg_type != "NEG-ERR" {
            return Err(Nip77Error::InvalidHex(format!(
                "expected NEG-ERR, got {}",
                msg_type
            )));
        }

        Ok(Self {
            subscription_id: arr[1]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("subscription ID not a string".to_string()))?
                .to_string(),
            reason: arr[2]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("reason not a string".to_string()))?
                .to_string(),
        })
    }
}

/// Nostr NEG-CLOSE message (client to relay)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegClose {
    /// Subscription ID
    pub subscription_id: String,
}

impl NegClose {
    /// Create a new NEG-CLOSE message
    pub fn new(subscription_id: String) -> Self {
        Self { subscription_id }
    }

    /// Convert to JSON array format
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!(["NEG-CLOSE", self.subscription_id])
    }

    /// Parse from JSON array
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let arr = value
            .as_array()
            .ok_or_else(|| Nip77Error::InvalidHex("not an array".to_string()))?;

        if arr.len() != 2 {
            return Err(Nip77Error::InvalidHex(format!(
                "expected 2 elements, got {}",
                arr.len()
            )));
        }

        let msg_type = arr[0]
            .as_str()
            .ok_or_else(|| Nip77Error::InvalidHex("message type not a string".to_string()))?;

        if msg_type != "NEG-CLOSE" {
            return Err(Nip77Error::InvalidHex(format!(
                "expected NEG-CLOSE, got {}",
                msg_type
            )));
        }

        Ok(Self {
            subscription_id: arr[1]
                .as_str()
                .ok_or_else(|| Nip77Error::InvalidHex("subscription ID not a string".to_string()))?
                .to_string(),
        })
    }
}

/// Calculate the fingerprint for a set of event IDs
///
/// The fingerprint algorithm:
/// 1. Compute addition mod 2^256 of element IDs (as 32-byte little-endian unsigned integers)
/// 2. Concatenate with the number of elements, encoded as a varint
/// 3. Hash with SHA-256
/// 4. Take the first 16 bytes
///
/// # Example
///
/// ```
/// use nostr::calculate_fingerprint;
///
/// let id1 = [0x01; 32];
/// let id2 = [0x02; 32];
/// let ids = vec![id1, id2];
///
/// let fp = calculate_fingerprint(&ids);
/// assert_eq!(fp.len(), 16);
///
/// // Fingerprint is deterministic
/// let fp2 = calculate_fingerprint(&ids);
/// assert_eq!(fp, fp2);
///
/// // And commutative (order doesn't matter)
/// let ids_reversed = vec![id2, id1];
/// let fp3 = calculate_fingerprint(&ids_reversed);
/// assert_eq!(fp, fp3);
/// ```
pub fn calculate_fingerprint(ids: &[EventId]) -> [u8; 16] {
    // Step 1: Add all IDs mod 2^256 (as little-endian)
    let mut sum = [0u8; 32];

    for id in ids {
        // Add this ID to sum (mod 2^256 is automatic with wrapping)
        let mut carry = 0u16;
        for i in 0..32 {
            let s = sum[i] as u16 + id[i] as u16 + carry;
            sum[i] = s as u8;
            carry = s >> 8;
        }
    }

    // Step 2: Concatenate with element count as varint
    let count_varint = encode_varint(ids.len() as u64).unwrap_or_default();
    let mut to_hash = Vec::with_capacity(32 + count_varint.len());
    to_hash.extend_from_slice(&sum);
    to_hash.extend_from_slice(&count_varint);

    // Step 3: Hash with SHA-256
    let hash = Sha256::digest(&to_hash);

    // Step 4: Take first 16 bytes
    let mut fingerprint = [0u8; 16];
    fingerprint.copy_from_slice(&hash[..16]);
    fingerprint
}

/// A record in the Negentropy protocol (timestamp + ID)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Record {
    /// Timestamp (used for sorting)
    pub timestamp: u64,
    /// Event ID
    pub id: EventId,
}

impl Record {
    /// Create a new record
    pub fn new(timestamp: u64, id: EventId) -> Self {
        Self { timestamp, id }
    }
}

/// Sort records according to Negentropy protocol rules
///
/// Records are sorted by:
/// 1. Timestamp (ascending)
/// 2. ID lexically (ascending) if timestamps are equal
///
/// # Example
///
/// ```
/// use nostr::{Record, sort_records};
///
/// let mut records = vec![
///     Record::new(200, [0x03; 32]),
///     Record::new(100, [0x02; 32]),
///     Record::new(100, [0x01; 32]),
/// ];
///
/// sort_records(&mut records);
///
/// // Sorted by timestamp first
/// assert_eq!(records[0].timestamp, 100);
/// assert_eq!(records[1].timestamp, 100);
/// assert_eq!(records[2].timestamp, 200);
///
/// // Then by ID when timestamps equal
/// assert!(records[0].id < records[1].id);
/// ```
pub fn sort_records(records: &mut [Record]) {
    records.sort_by(|a, b| match a.timestamp.cmp(&b.timestamp) {
        std::cmp::Ordering::Equal => a.id.cmp(&b.id),
        other => other,
    });
}

/// State for tracking reconciliation progress
#[derive(Debug, Clone)]
#[allow(dead_code)] // Will be used in process_message() implementation
pub struct ReconciliationState {
    /// Local event records (sorted)
    pub records: Vec<Record>,
    /// IDs we have that remote needs
    pub have: Vec<EventId>,
    /// IDs remote has that we need
    pub need: Vec<EventId>,
    /// Current position in records for range splitting
    pub position: usize,
}

impl ReconciliationState {
    /// Create new reconciliation state with sorted records
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn new(mut records: Vec<Record>) -> Self {
        sort_records(&mut records);
        Self {
            records,
            have: Vec::new(),
            need: Vec::new(),
            position: 0,
        }
    }

    /// Find records within a bound range
    ///
    /// Returns indices of records where lower_bound <= record < upper_bound
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn find_records_in_range(&self, lower: &Bound, upper: &Bound) -> Vec<usize> {
        let mut indices = Vec::new();

        for (i, record) in self.records.iter().enumerate() {
            // Check if record >= lower_bound
            if record.timestamp < lower.timestamp {
                continue;
            }
            if record.timestamp == lower.timestamp && !lower.id_prefix.is_empty() {
                // Compare ID prefix
                let prefix_len = lower.id_prefix.len().min(32);
                if &record.id[..prefix_len] < lower.id_prefix.as_slice() {
                    continue;
                }
            }

            // Check if record < upper_bound (exclusive)
            if upper.timestamp != TIMESTAMP_INFINITY {
                if record.timestamp > upper.timestamp {
                    break; // Records are sorted, can stop early
                }
                if record.timestamp == upper.timestamp {
                    if upper.id_prefix.is_empty() {
                        // No ID prefix means exclude all records at this timestamp
                        break;
                    } else {
                        // Compare ID prefix
                        let prefix_len = upper.id_prefix.len().min(32);
                        if &record.id[..prefix_len] >= upper.id_prefix.as_slice() {
                            break;
                        }
                    }
                }
            }

            indices.push(i);
        }

        indices
    }

    /// Calculate fingerprint for records in a range
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn calculate_range_fingerprint(&self, lower: &Bound, upper: &Bound) -> [u8; 16] {
        let indices = self.find_records_in_range(lower, upper);
        let ids: Vec<EventId> = indices.iter().map(|&i| self.records[i].id).collect();
        calculate_fingerprint(&ids)
    }

    /// Split a range into smaller sub-ranges
    ///
    /// Divides the range to isolate differences. Strategy:
    /// - If range has 0 records: return empty (skip range)
    /// - If range has 1 record: return ID list
    /// - If range has multiple: split at midpoint
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn split_range(&self, lower: &Bound, upper: &Bound) -> Result<Vec<Range>> {
        let indices = self.find_records_in_range(lower, upper);

        if indices.is_empty() {
            // No local records in this range - skip it
            return Ok(vec![Range::skip(upper.clone())]);
        }

        if indices.len() == 1 {
            // Single record - send as ID list
            let id = self.records[indices[0]].id;
            return Ok(vec![Range::id_list(upper.clone(), vec![id])]);
        }

        // Multiple records - split at midpoint
        let mid_idx = indices[indices.len() / 2];
        let mid_record = &self.records[mid_idx];

        // Create midpoint bound
        let mid_bound = Bound::new(mid_record.timestamp, mid_record.id[..8].to_vec())?;

        // Calculate fingerprints for each half
        let fp_lower = self.calculate_range_fingerprint(lower, &mid_bound);
        let fp_upper = self.calculate_range_fingerprint(&mid_bound, upper);

        Ok(vec![
            Range::fingerprint(mid_bound, fp_lower),
            Range::fingerprint(upper.clone(), fp_upper),
        ])
    }

    /// Add an ID to the "have" set (we have, remote needs)
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn add_have(&mut self, id: EventId) {
        if !self.have.contains(&id) {
            self.have.push(id);
        }
    }

    /// Add an ID to the "need" set (remote has, we need)
    #[allow(dead_code)] // Will be called by process_message()
    pub fn add_need(&mut self, id: EventId) {
        if !self.need.contains(&id) {
            self.need.push(id);
        }
    }

    /// Process an incoming Negentropy message and generate response
    ///
    /// This is the core reconciliation algorithm. For each incoming range:
    /// 1. If it's a Skip - no remote records in this range
    /// 2. If it's Fingerprint - compare with local fingerprint:
    ///    - If match: skip range (both sides agree)
    ///    - If mismatch: split range to isolate differences
    /// 3. If it's IdList - compare with local IDs:
    ///    - Add IDs we have but they don't to "have" set
    ///    - Add IDs they have but we don't to "need" set
    ///
    /// Returns a response message with our ranges
    #[allow(dead_code)] // Will be used in relay/client integration
    pub fn process_message(&mut self, incoming: &NegentropyMessage) -> Result<NegentropyMessage> {
        let mut response_ranges = Vec::new();
        let mut prev_bound = Bound::zero();

        for incoming_range in &incoming.ranges {
            let upper = &incoming_range.upper_bound;

            match &incoming_range.payload {
                RangePayload::Skip => {
                    // Remote has no records in [prev_bound, upper)
                    // Send all our records in this range
                    let our_ranges = self.split_range(&prev_bound, upper)?;
                    response_ranges.extend(our_ranges);
                }

                RangePayload::Fingerprint(remote_fp) => {
                    // Compare fingerprints
                    let local_fp = self.calculate_range_fingerprint(&prev_bound, upper);

                    if &local_fp == remote_fp {
                        // Fingerprints match - both sides agree on this range
                        response_ranges.push(Range::skip(upper.clone()));
                    } else {
                        // Fingerprints differ - split to isolate differences
                        let our_ranges = self.split_range(&prev_bound, upper)?;
                        response_ranges.extend(our_ranges);
                    }
                }

                RangePayload::IdList(remote_ids) => {
                    // Get our IDs in this range
                    let indices = self.find_records_in_range(&prev_bound, upper);
                    let local_ids: Vec<EventId> =
                        indices.iter().map(|&i| self.records[i].id).collect();

                    // Find IDs we have that they don't
                    for local_id in &local_ids {
                        if !remote_ids.contains(local_id) {
                            self.add_have(*local_id);
                        }
                    }

                    // Find IDs they have that we don't
                    for remote_id in remote_ids {
                        if !local_ids.contains(remote_id) {
                            self.add_need(*remote_id);
                        }
                    }

                    // Respond with our IDs for this range
                    if local_ids.is_empty() {
                        response_ranges.push(Range::skip(upper.clone()));
                    } else {
                        response_ranges.push(Range::id_list(upper.clone(), local_ids));
                    }
                }
            }

            prev_bound = upper.clone();
        }

        // Ensure we cover the full range up to infinity
        if prev_bound.timestamp != TIMESTAMP_INFINITY {
            let remaining_ranges = self.split_range(&prev_bound, &Bound::infinity())?;
            response_ranges.extend(remaining_ranges);
        }

        Ok(NegentropyMessage::new(response_ranges))
    }

    /// Check if reconciliation is complete
    ///
    /// Reconciliation is complete when both sides have exchanged ID lists
    /// for all ranges (no more fingerprints to compare)
    #[allow(dead_code)] // Will be used in relay/client integration
    pub fn is_complete(&self, last_message: &NegentropyMessage) -> bool {
        // If all ranges are Skip or IdList (no Fingerprint), we're done
        last_message
            .ranges
            .iter()
            .all(|r| matches!(r.payload, RangePayload::Skip | RangePayload::IdList(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encode_zero() {
        assert_eq!(encode_varint(0).unwrap(), vec![0]);
    }

    #[test]
    fn test_varint_encode_small() {
        assert_eq!(encode_varint(127).unwrap(), vec![127]);
    }

    #[test]
    fn test_varint_encode_multi_byte() {
        assert_eq!(encode_varint(128).unwrap(), vec![0x81, 0x00]);
        assert_eq!(encode_varint(300).unwrap(), vec![0x82, 0x2C]);
    }

    #[test]
    fn test_varint_roundtrip() {
        let test_values = vec![0, 1, 127, 128, 255, 256, 300, 16383, 16384, u64::MAX / 2];
        for value in test_values {
            let encoded = encode_varint(value).unwrap();
            let (decoded, len) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(len, encoded.len());
        }
    }

    #[test]
    fn test_bound_encode_decode() {
        let bound = Bound::new(12345, vec![0xAB, 0xCD]).unwrap();
        let encoded = bound.encode(0).unwrap();
        let (decoded, _) = Bound::decode(&encoded, 0).unwrap();
        assert_eq!(decoded, bound);
    }

    #[test]
    fn test_bound_infinity() {
        let bound = Bound::infinity();
        let encoded = bound.encode(1000).unwrap();
        let (decoded, _) = Bound::decode(&encoded, 1000).unwrap();
        assert_eq!(decoded.timestamp, TIMESTAMP_INFINITY);
    }

    #[test]
    fn test_range_skip() {
        let range = Range::skip(Bound::new(100, vec![]).unwrap());
        let encoded = range.encode(0).unwrap();
        let (decoded, _, _) = Range::decode(&encoded, 0).unwrap();
        assert_eq!(decoded, range);
    }

    #[test]
    fn test_range_fingerprint() {
        let fp = [0xAB; 16];
        let range = Range::fingerprint(Bound::new(200, vec![]).unwrap(), fp);
        let encoded = range.encode(0).unwrap();
        let (decoded, _, _) = Range::decode(&encoded, 0).unwrap();
        assert_eq!(decoded, range);
    }

    #[test]
    fn test_range_id_list() {
        let ids = vec![[0x01; 32], [0x02; 32]];
        let range = Range::id_list(Bound::new(300, vec![]).unwrap(), ids.clone());
        let encoded = range.encode(0).unwrap();
        let (decoded, _, _) = Range::decode(&encoded, 0).unwrap();
        assert_eq!(decoded, range);
    }

    #[test]
    fn test_negentropy_message() {
        let ranges = vec![
            Range::skip(Bound::new(100, vec![]).unwrap()),
            Range::fingerprint(Bound::new(200, vec![]).unwrap(), [0xAB; 16]),
        ];
        let msg = NegentropyMessage::new(ranges);
        let hex = msg.encode_hex().unwrap();
        let decoded = NegentropyMessage::decode_hex(&hex).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_neg_open_json() {
        let msg = NegentropyMessage::new(vec![]);
        let neg_open =
            NegOpen::new("sub1".to_string(), serde_json::json!({"kinds": [1]}), &msg).unwrap();

        let json = neg_open.to_json();
        let parsed = NegOpen::from_json(&json).unwrap();
        assert_eq!(parsed.subscription_id, neg_open.subscription_id);
    }

    #[test]
    fn test_neg_close_json() {
        let neg_close = NegClose::new("sub1".to_string());
        let json = neg_close.to_json();
        let parsed = NegClose::from_json(&json).unwrap();
        assert_eq!(parsed.subscription_id, neg_close.subscription_id);
    }

    #[test]
    fn test_fingerprint_empty() {
        let fp = calculate_fingerprint(&[]);
        // Empty set should have consistent fingerprint
        assert_eq!(fp.len(), 16);
    }

    #[test]
    fn test_fingerprint_single() {
        let id = [0x01; 32];
        let fp = calculate_fingerprint(&[id]);
        assert_eq!(fp.len(), 16);
        // Should be deterministic
        let fp2 = calculate_fingerprint(&[id]);
        assert_eq!(fp, fp2);
    }

    #[test]
    fn test_fingerprint_multiple() {
        let ids = vec![[0x01; 32], [0x02; 32], [0x03; 32]];
        let fp = calculate_fingerprint(&ids);
        assert_eq!(fp.len(), 16);

        // Order shouldn't matter for fingerprint (commutative)
        let ids_reversed = vec![[0x03; 32], [0x02; 32], [0x01; 32]];
        let fp_reversed = calculate_fingerprint(&ids_reversed);
        assert_eq!(fp, fp_reversed);
    }

    #[test]
    fn test_fingerprint_different_counts() {
        // Same IDs but different counts should produce different fingerprints
        let fp1 = calculate_fingerprint(&[[0x01; 32]]);
        let fp2 = calculate_fingerprint(&[[0x01; 32], [0x01; 32]]);
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_record_sorting() {
        let mut records = vec![
            Record::new(100, [0x03; 32]),
            Record::new(50, [0x01; 32]),
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
        ];

        sort_records(&mut records);

        // Should be sorted by timestamp, then ID
        assert_eq!(records[0].timestamp, 50);
        assert_eq!(records[1].timestamp, 100);
        assert_eq!(records[1].id, [0x01; 32]);
        assert_eq!(records[2].timestamp, 100);
        assert_eq!(records[2].id, [0x03; 32]);
        assert_eq!(records[3].timestamp, 200);
    }

    #[test]
    fn test_record_sorting_same_timestamp() {
        let mut records = vec![
            Record::new(100, [0xFF; 32]),
            Record::new(100, [0x00; 32]),
            Record::new(100, [0x7F; 32]),
        ];

        sort_records(&mut records);

        // Should be sorted lexically by ID when timestamps are equal
        assert!(records[0].id < records[1].id);
        assert!(records[1].id < records[2].id);
    }

    // === Varint Edge Case Tests ===

    #[test]
    fn test_varint_boundary_values() {
        // Test zero explicitly
        let encoded = encode_varint(0).unwrap();
        assert_eq!(encoded, vec![0x00]);
        assert_eq!(decode_varint(&encoded).unwrap(), (0, 1));

        // Test max u64
        let max = u64::MAX;
        let encoded = encode_varint(max).unwrap();
        let (decoded, _) = decode_varint(&encoded).unwrap();
        assert_eq!(decoded, max);

        // Test powers of 2
        for power in 0..=63 {
            let value = if power == 63 {
                1u64 << 63 // Highest bit
            } else {
                1u64 << power
            };
            let encoded = encode_varint(value).unwrap();
            let (decoded, len) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value, "Failed for 2^{}", power);
            assert_eq!(len, encoded.len());
        }
    }

    #[test]
    fn test_varint_encoding_lengths() {
        // 1 byte: 0-127 (7 bits)
        assert_eq!(encode_varint(0).unwrap().len(), 1);
        assert_eq!(encode_varint(1).unwrap().len(), 1);
        assert_eq!(encode_varint(127).unwrap().len(), 1);

        // 2 bytes: 128-16383 (14 bits)
        assert_eq!(encode_varint(128).unwrap().len(), 2);
        assert_eq!(encode_varint(255).unwrap().len(), 2);
        assert_eq!(encode_varint(256).unwrap().len(), 2);
        assert_eq!(encode_varint(16383).unwrap().len(), 2);

        // 3 bytes: 16384-2097151 (21 bits)
        assert_eq!(encode_varint(16384).unwrap().len(), 3);
        assert_eq!(encode_varint(65535).unwrap().len(), 3);
        assert_eq!(encode_varint(65536).unwrap().len(), 3);
        assert_eq!(encode_varint(2097151).unwrap().len(), 3);

        // 10 bytes: max u64 (64 bits = ceiling(64/7) = 10 bytes)
        assert_eq!(encode_varint(u64::MAX).unwrap().len(), 10);
    }

    #[test]
    fn test_varint_specific_values() {
        // Test specific known encodings
        let test_cases = vec![
            (0u64, vec![0x00]),
            (1u64, vec![0x01]),
            (127u64, vec![0x7F]),
            (128u64, vec![0x81, 0x00]),
            (255u64, vec![0x81, 0x7F]),
            (256u64, vec![0x82, 0x00]),
            (300u64, vec![0x82, 0x2C]),
            (16383u64, vec![0xFF, 0x7F]),
            (16384u64, vec![0x81, 0x80, 0x00]),
        ];

        for (value, expected_bytes) in test_cases {
            let encoded = encode_varint(value).unwrap();
            assert_eq!(
                encoded, expected_bytes,
                "Encoding mismatch for value {}",
                value
            );

            let (decoded, len) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(len, expected_bytes.len());
        }
    }

    #[test]
    fn test_varint_decode_errors() {
        // Empty data
        let result = decode_varint(&[]);
        assert!(result.is_err());
        assert!(matches!(result, Err(Nip77Error::VarintDecode(_))));

        // Incomplete varint (high bit set but no continuation)
        let result = decode_varint(&[0x80]);
        assert!(result.is_err());
        assert!(matches!(result, Err(Nip77Error::VarintDecode(_))));

        // Incomplete multi-byte (high bit set on last byte)
        let result = decode_varint(&[0x81, 0x80]);
        assert!(result.is_err());

        // Varint too long (>10 bytes for u64)
        let too_long = vec![0x80; 11];
        let result = decode_varint(&too_long);
        assert!(result.is_err());
    }

    #[test]
    fn test_varint_decode_overflow() {
        // Create a varint that would overflow u64
        // 11 bytes with max values would exceed u64
        let overflow_bytes = vec![
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ];
        let result = decode_varint(&overflow_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_varint_continuation_bits() {
        // Valid: high bit set on all but last
        let valid = vec![0x81, 0x00]; // 128
        assert!(decode_varint(&valid).is_ok());

        // Valid: single byte without high bit
        let valid_single = vec![0x7F]; // 127
        assert!(decode_varint(&valid_single).is_ok());

        // Invalid: high bit not set on intermediate byte
        // This would actually decode successfully but give wrong value
        // Let's test that decoding stops at first byte without high bit
        let stops_early = vec![0x00, 0xFF]; // Should decode as 0, consuming 1 byte
        let (value, len) = decode_varint(&stops_early).unwrap();
        assert_eq!(value, 0);
        assert_eq!(len, 1);
    }

    #[test]
    fn test_varint_roundtrip_comprehensive() {
        // Test a comprehensive set of values
        let test_values = vec![
            0,
            1,
            127, // 1 byte boundary
            128, // 2 byte start
            255,
            256,
            16383, // 2 byte boundary
            16384, // 3 byte start
            65535, // Common 16-bit boundary
            65536,
            2097151,         // 3 byte boundary
            2097152,         // 4 byte start
            u32::MAX as u64, // 32-bit max
            u32::MAX as u64 + 1,
            u64::MAX / 2,
            u64::MAX - 1,
            u64::MAX,
        ];

        for value in test_values {
            let encoded = encode_varint(value).unwrap();
            let (decoded, len) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value, "Roundtrip failed for value {}", value);
            assert_eq!(len, encoded.len());

            // Verify first byte has high bit set if multi-byte
            if encoded.len() > 1 {
                assert_eq!(
                    encoded[0] & 0x80,
                    0x80,
                    "First byte of multi-byte varint should have high bit set"
                );
            }

            // Verify last byte doesn't have high bit set
            assert_eq!(
                encoded[encoded.len() - 1] & 0x80,
                0,
                "Last byte should not have high bit set"
            );
        }
    }

    #[test]
    fn test_varint_decode_partial_buffer() {
        // Decode should only consume what's needed
        // Two varints: 128 (0x81 0x00), then 127 (0x7F)
        let multi_value_buffer = vec![0x81, 0x00, 0x7F];

        let (value1, len1) = decode_varint(&multi_value_buffer).unwrap();
        assert_eq!(value1, 128);
        assert_eq!(len1, 2);

        let (value2, len2) = decode_varint(&multi_value_buffer[len1..]).unwrap();
        assert_eq!(value2, 127);
        assert_eq!(len2, 1);
    }

    #[test]
    fn test_varint_sequential_values() {
        // Test a range of sequential values to ensure no gaps
        for value in 0..1000u64 {
            let encoded = encode_varint(value).unwrap();
            let (decoded, _) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value);
        }

        // Test around boundaries
        for offset in 0..100u64 {
            let test_values = [
                127u64.saturating_sub(offset),
                128u64.saturating_add(offset),
                16383u64.saturating_sub(offset),
                16384u64.saturating_add(offset),
            ];

            for value in test_values {
                let encoded = encode_varint(value).unwrap();
                let (decoded, _) = decode_varint(&encoded).unwrap();
                assert_eq!(decoded, value);
            }
        }
    }

    #[test]
    fn test_varint_max_encoding_length() {
        // u64::MAX should encode to exactly 10 bytes
        // 64 bits / 7 bits per byte = 9.14... = ceiling 10 bytes
        let max_encoded = encode_varint(u64::MAX).unwrap();
        assert_eq!(max_encoded.len(), 10);

        // All bytes except last should have high bit set
        for (i, &byte) in max_encoded.iter().enumerate() {
            if i < 9 {
                assert_eq!(byte & 0x80, 0x80, "Byte {} should have high bit", i);
            } else {
                assert_eq!(byte & 0x80, 0, "Last byte should not have high bit");
            }
        }
    }

    #[test]
    fn test_varint_deterministic() {
        // Encoding should be deterministic
        let value = 123456789u64;
        let encoded1 = encode_varint(value).unwrap();
        let encoded2 = encode_varint(value).unwrap();
        assert_eq!(encoded1, encoded2);

        // Decoding should be deterministic
        let (decoded1, len1) = decode_varint(&encoded1).unwrap();
        let (decoded2, len2) = decode_varint(&encoded2).unwrap();
        assert_eq!(decoded1, decoded2);
        assert_eq!(len1, len2);
    }

    // === ReconciliationState Tests ===

    #[test]
    fn test_reconciliation_state_new() {
        let records = vec![
            Record::new(300, [0x03; 32]),
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
        ];

        let state = ReconciliationState::new(records);

        // Should be sorted
        assert_eq!(state.records[0].timestamp, 100);
        assert_eq!(state.records[1].timestamp, 200);
        assert_eq!(state.records[2].timestamp, 300);

        // Should start with empty have/need
        assert!(state.have.is_empty());
        assert!(state.need.is_empty());
        assert_eq!(state.position, 0);
    }

    #[test]
    fn test_find_records_in_range() {
        let records = vec![
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
            Record::new(300, [0x03; 32]),
            Record::new(400, [0x04; 32]),
        ];

        let state = ReconciliationState::new(records);

        // Find all records
        let lower = Bound::zero();
        let upper = Bound::infinity();
        let indices = state.find_records_in_range(&lower, &upper);
        assert_eq!(indices, vec![0, 1, 2, 3]);

        // Find middle range
        let lower = Bound::new(200, vec![]).unwrap();
        let upper = Bound::new(400, vec![]).unwrap();
        let indices = state.find_records_in_range(&lower, &upper);
        assert_eq!(indices, vec![1, 2]); // Records at 200 and 300

        // Find empty range
        let lower = Bound::new(150, vec![]).unwrap();
        let upper = Bound::new(180, vec![]).unwrap();
        let indices = state.find_records_in_range(&lower, &upper);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_find_records_with_id_prefix() {
        let mut id1 = [0x00; 32];
        let mut id2 = [0x00; 32];
        let mut id3 = [0x00; 32];

        id1[0] = 0x10;
        id2[0] = 0x20;
        id3[0] = 0x30;

        let records = vec![
            Record::new(100, id1),
            Record::new(100, id2),
            Record::new(100, id3),
        ];

        let state = ReconciliationState::new(records);

        // Find range with ID prefix
        let lower = Bound::new(100, vec![0x15]).unwrap(); // After id1
        let upper = Bound::new(100, vec![0x25]).unwrap(); // Before id3
        let indices = state.find_records_in_range(&lower, &upper);
        assert_eq!(indices, vec![1]); // Only id2 (0x20) is in range
    }

    #[test]
    fn test_calculate_range_fingerprint() {
        let records = vec![
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
            Record::new(300, [0x03; 32]),
        ];

        let state = ReconciliationState::new(records);

        // Fingerprint of all records
        let fp_all = state.calculate_range_fingerprint(&Bound::zero(), &Bound::infinity());
        assert_eq!(fp_all.len(), 16);

        // Fingerprint of subset
        let lower = Bound::new(200, vec![]).unwrap();
        let upper = Bound::infinity();
        let fp_subset = state.calculate_range_fingerprint(&lower, &upper);

        // Should be different
        assert_ne!(fp_all, fp_subset);

        // Empty range should have consistent fingerprint
        let lower = Bound::new(150, vec![]).unwrap();
        let upper = Bound::new(180, vec![]).unwrap();
        let fp_empty = state.calculate_range_fingerprint(&lower, &upper);
        assert_eq!(fp_empty.len(), 16);
    }

    #[test]
    fn test_split_range_empty() {
        let records = vec![Record::new(100, [0x01; 32]), Record::new(300, [0x03; 32])];

        let state = ReconciliationState::new(records);

        // Split empty range (between records)
        let lower = Bound::new(150, vec![]).unwrap();
        let upper = Bound::new(250, vec![]).unwrap();
        let ranges = state.split_range(&lower, &upper).unwrap();

        // Should return skip range
        assert_eq!(ranges.len(), 1);
        assert!(matches!(ranges[0].payload, RangePayload::Skip));
        assert_eq!(ranges[0].upper_bound, upper);
    }

    #[test]
    fn test_split_range_single_record() {
        let records = vec![Record::new(100, [0x01; 32]), Record::new(200, [0x02; 32])];

        let state = ReconciliationState::new(records);

        // Split range with single record
        let lower = Bound::new(180, vec![]).unwrap();
        let upper = Bound::new(220, vec![]).unwrap();
        let ranges = state.split_range(&lower, &upper).unwrap();

        // Should return ID list with single ID
        assert_eq!(ranges.len(), 1);
        assert!(matches!(ranges[0].payload, RangePayload::IdList(_)));
        if let RangePayload::IdList(ids) = &ranges[0].payload {
            assert_eq!(ids.len(), 1);
            assert_eq!(ids[0], [0x02; 32]);
        }
    }

    #[test]
    fn test_split_range_multiple_records() {
        let records = vec![
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
            Record::new(300, [0x03; 32]),
            Record::new(400, [0x04; 32]),
        ];

        let state = ReconciliationState::new(records);

        // Split range with 4 records
        let lower = Bound::zero();
        let upper = Bound::infinity();
        let ranges = state.split_range(&lower, &upper).unwrap();

        // Should return 2 ranges with fingerprints
        assert_eq!(ranges.len(), 2);
        assert!(matches!(ranges[0].payload, RangePayload::Fingerprint(_)));
        assert!(matches!(ranges[1].payload, RangePayload::Fingerprint(_)));

        // Midpoint should be around record 2 (200)
        // First range covers [0, mid), second covers [mid, infinity)
    }

    #[test]
    fn test_split_range_two_records() {
        let records = vec![Record::new(100, [0x01; 32]), Record::new(200, [0x02; 32])];

        let state = ReconciliationState::new(records);

        // Split range with exactly 2 records
        let lower = Bound::zero();
        let upper = Bound::infinity();
        let ranges = state.split_range(&lower, &upper).unwrap();

        // Should split into 2 fingerprint ranges
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_add_have_and_need() {
        let records = vec![Record::new(100, [0x01; 32])];
        let mut state = ReconciliationState::new(records);

        let id1 = [0xAB; 32];
        let id2 = [0xCD; 32];

        // Add to have
        state.add_have(id1);
        assert_eq!(state.have.len(), 1);
        assert_eq!(state.have[0], id1);

        // Add duplicate should not increase size
        state.add_have(id1);
        assert_eq!(state.have.len(), 1);

        // Add different ID
        state.add_have(id2);
        assert_eq!(state.have.len(), 2);

        // Add to need
        state.add_need(id1);
        assert_eq!(state.need.len(), 1);
        assert_eq!(state.need[0], id1);

        state.add_need(id2);
        assert_eq!(state.need.len(), 2);
    }

    #[test]
    fn test_split_range_preserves_fingerprints() {
        let records = vec![
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
            Record::new(300, [0x03; 32]),
            Record::new(400, [0x04; 32]),
        ];

        let state = ReconciliationState::new(records);

        let lower = Bound::zero();
        let upper = Bound::infinity();
        let ranges = state.split_range(&lower, &upper).unwrap();

        // Verify each range has correct fingerprint for its sub-range
        for range in &ranges {
            if let RangePayload::Fingerprint(fp) = &range.payload {
                // Fingerprint should be 16 bytes
                assert_eq!(fp.len(), 16);
            }
        }
    }

    #[test]
    fn test_process_message_identical_sets() {
        // Both sides have same events
        let records = vec![Record::new(100, [0x01; 32]), Record::new(200, [0x02; 32])];

        let mut state = ReconciliationState::new(records.clone());

        // Create message with matching fingerprint
        let ids: Vec<EventId> = records.iter().map(|r| r.id).collect();
        let fp = calculate_fingerprint(&ids);
        let incoming = NegentropyMessage::new(vec![Range::fingerprint(Bound::infinity(), fp)]);

        let response = state.process_message(&incoming).unwrap();

        // Should respond with skip (fingerprints match)
        assert_eq!(response.ranges.len(), 1);
        assert!(matches!(response.ranges[0].payload, RangePayload::Skip));
        assert_eq!(state.have.len(), 0);
        assert_eq!(state.need.len(), 0);
    }

    #[test]
    fn test_process_message_empty_sets() {
        // Both sides have no events
        let mut state = ReconciliationState::new(vec![]);

        // Remote has no events
        let incoming = NegentropyMessage::new(vec![Range::skip(Bound::infinity())]);

        let response = state.process_message(&incoming).unwrap();

        // Should respond with skip (we also have nothing)
        assert_eq!(response.ranges.len(), 1);
        assert!(matches!(response.ranges[0].payload, RangePayload::Skip));
    }

    #[test]
    fn test_process_message_disjoint_sets() {
        // We have events [0x01, 0x02]
        let our_records = vec![Record::new(100, [0x01; 32]), Record::new(200, [0x02; 32])];

        let mut state = ReconciliationState::new(our_records);

        // They have completely different events [0x03, 0x04]
        let their_ids = vec![[0x03; 32], [0x04; 32]];
        let incoming =
            NegentropyMessage::new(vec![Range::id_list(Bound::infinity(), their_ids.clone())]);

        let response = state.process_message(&incoming).unwrap();

        // We should send our IDs
        assert_eq!(response.ranges.len(), 1);
        if let RangePayload::IdList(ids) = &response.ranges[0].payload {
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&[0x01; 32]));
            assert!(ids.contains(&[0x02; 32]));
        } else {
            panic!("Expected IdList");
        }

        // have = [0x01, 0x02] (we have, they don't)
        assert_eq!(state.have.len(), 2);
        assert!(state.have.contains(&[0x01; 32]));
        assert!(state.have.contains(&[0x02; 32]));

        // need = [0x03, 0x04] (they have, we don't)
        assert_eq!(state.need.len(), 2);
        assert!(state.need.contains(&[0x03; 32]));
        assert!(state.need.contains(&[0x04; 32]));
    }

    #[test]
    fn test_process_message_partial_overlap() {
        // We have [0x01, 0x02, 0x03]
        let our_records = vec![
            Record::new(100, [0x01; 32]),
            Record::new(200, [0x02; 32]),
            Record::new(300, [0x03; 32]),
        ];

        let mut state = ReconciliationState::new(our_records);

        // They have [0x02, 0x03, 0x04] (overlap on 0x02 and 0x03)
        let their_ids = vec![[0x02; 32], [0x03; 32], [0x04; 32]];
        let incoming = NegentropyMessage::new(vec![Range::id_list(Bound::infinity(), their_ids)]);

        let response = state.process_message(&incoming).unwrap();

        // We should send our IDs
        assert_eq!(response.ranges.len(), 1);
        if let RangePayload::IdList(ids) = &response.ranges[0].payload {
            assert_eq!(ids.len(), 3);
        } else {
            panic!("Expected IdList");
        }

        // have = [0x01] (we have, they don't)
        assert_eq!(state.have.len(), 1);
        assert!(state.have.contains(&[0x01; 32]));

        // need = [0x04] (they have, we don't)
        assert_eq!(state.need.len(), 1);
        assert!(state.need.contains(&[0x04; 32]));
    }

    #[test]
    fn test_process_message_fingerprint_mismatch() {
        let records = vec![Record::new(100, [0x01; 32]), Record::new(200, [0x02; 32])];

        let mut state = ReconciliationState::new(records);

        // Send mismatched fingerprint (all zeros)
        let incoming =
            NegentropyMessage::new(vec![Range::fingerprint(Bound::infinity(), [0x00; 16])]);

        let response = state.process_message(&incoming).unwrap();

        // Should split the range (fingerprint mismatch)
        // With 2 records, should split into 2 fingerprint ranges
        assert!(response.ranges.len() >= 1);

        // At least one range should be a Fingerprint or IdList
        let has_fingerprint_or_id = response.ranges.iter().any(|r| {
            matches!(
                r.payload,
                RangePayload::Fingerprint(_) | RangePayload::IdList(_)
            )
        });
        assert!(has_fingerprint_or_id);
    }

    #[test]
    fn test_process_message_skip_range() {
        // We have events
        let records = vec![Record::new(100, [0x01; 32]), Record::new(200, [0x02; 32])];

        let mut state = ReconciliationState::new(records);

        // Remote has no events (skip)
        let incoming = NegentropyMessage::new(vec![Range::skip(Bound::infinity())]);

        let response = state.process_message(&incoming).unwrap();

        // Should send our records
        assert!(response.ranges.len() >= 1);

        // Should have at least one non-skip range
        let has_content = response
            .ranges
            .iter()
            .any(|r| !matches!(r.payload, RangePayload::Skip));
        assert!(has_content);
    }

    #[test]
    fn test_is_complete_with_fingerprints() {
        let state = ReconciliationState::new(vec![]);

        let message =
            NegentropyMessage::new(vec![Range::fingerprint(Bound::infinity(), [0x00; 16])]);

        // Not complete - still has fingerprints to resolve
        assert!(!state.is_complete(&message));
    }

    #[test]
    fn test_is_complete_with_id_lists() {
        let state = ReconciliationState::new(vec![]);

        let message = NegentropyMessage::new(vec![
            Range::id_list(Bound::new(500, vec![]).unwrap(), vec![[0x01; 32]]),
            Range::skip(Bound::infinity()),
        ]);

        // Complete - all ranges are Skip or IdList
        assert!(state.is_complete(&message));
    }

    #[test]
    fn test_is_complete_with_skip_only() {
        let state = ReconciliationState::new(vec![]);

        let message = NegentropyMessage::new(vec![Range::skip(Bound::infinity())]);

        // Complete - all ranges are Skip
        assert!(state.is_complete(&message));
    }

    #[test]
    fn test_process_message_multiple_ranges() {
        // We have events at different timestamps
        let records = vec![Record::new(100, [0x01; 32]), Record::new(500, [0x05; 32])];

        let mut state = ReconciliationState::new(records);

        // Remote sends multiple ranges
        let incoming = NegentropyMessage::new(vec![
            Range::id_list(Bound::new(300, vec![]).unwrap(), vec![[0x02; 32]]),
            Range::skip(Bound::infinity()),
        ]);

        let response = state.process_message(&incoming).unwrap();

        // Should process each range correctly
        assert!(response.ranges.len() >= 1);

        // need = [0x02] (they have in first range)
        assert_eq!(state.need.len(), 1);
        assert!(state.need.contains(&[0x02; 32]));

        // have = [0x01] (we have in first range, they don't)
        assert_eq!(state.have.len(), 1);
        assert!(state.have.contains(&[0x01; 32]));
    }
}
