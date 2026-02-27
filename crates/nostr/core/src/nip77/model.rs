use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::codec::{decode_varint, encode_varint};
use super::error::{Nip77Error, Result};
/// Negentropy Protocol Version 1
pub const PROTOCOL_VERSION_1: u8 = 0x61;

/// Special infinity timestamp value
pub const TIMESTAMP_INFINITY: u64 = u64::MAX;
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
