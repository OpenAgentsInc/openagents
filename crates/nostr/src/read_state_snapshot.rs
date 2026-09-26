//! NIP-RS atomic snapshot wire contract.
//!
//! These checks authenticate the events and identify a supplied cut. Only the
//! storage adapter can establish that the cut contains every retained current
//! coordinate from one writer-database statement. A digest proves neither that
//! completeness nor freshness after the statement.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::domain::Event;

/// Maximum number of current own-author coordinates in a snapshot.
pub const MAX_EVENTS: usize = 4_096;
/// Separate ceiling for stored payload bytes and the compact events array.
pub const MAX_EVENT_ARRAY_BYTES: usize = 8_388_608;
/// Separate room for the envelope around the compact events-array budget.
/// One KiB also leaves room for insignificant envelope JSON whitespace.
pub const MAX_ENVELOPE_BYTES: usize = MAX_EVENT_ARRAY_BYTES + 1_024;

/// A snapshot failure never establishes a complete view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    /// The extension filter was malformed or mixed with ordinary filters.
    InvalidRequest,
    /// The envelope, descriptor, digest, or coordinates were invalid.
    InvalidEnvelope,
    /// A retained event could not be authenticated.
    InvalidEvent,
    /// The community or signing identity did not match the trusted session.
    IdentityMismatch,
    /// One of the hard count or byte ceilings was exceeded.
    LimitExceeded,
}

/// Origin-bound NIP-11 discovery, resolved from the server's community map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDescriptor {
    pub version: u8,
    pub community_id: String,
    pub max_events: usize,
    pub max_event_array_bytes: usize,
}

impl SnapshotDescriptor {
    /// Construct the exact version-one descriptor for a resolved community.
    pub fn new(community_id: &str) -> Result<Self, SnapshotError> {
        uuid_bytes(community_id)?;
        Ok(Self {
            version: 1,
            community_id: community_id.into(),
            max_events: MAX_EVENTS,
            max_event_array_bytes: MAX_EVENT_ARRAY_BYTES,
        })
    }

    fn validate(&self) -> Result<(), SnapshotError> {
        if self != &Self::new(&self.community_id)? {
            return Err(SnapshotError::InvalidEnvelope);
        }
        Ok(())
    }
}

/// The authenticated signing identity, never its delegated owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub pubkey: String,
}

/// Parse a raw query filter array before ordinary-filter decoding discards keys.
///
/// `None` means no filter requested this extension. Any occurrence of its key
/// activates strict validation, including malformed or false values.
pub fn parse_request(
    raw_filters: &Value,
    authenticated_pubkey: &str,
) -> Result<Option<SnapshotRequest>, SnapshotError> {
    let filters = raw_filters
        .as_array()
        .ok_or(SnapshotError::InvalidRequest)?;
    if !filters
        .iter()
        .any(|filter| filter.get("read_state_snapshot").is_some())
    {
        return Ok(None);
    }
    hex_bytes::<32>(authenticated_pubkey).map_err(|_| SnapshotError::InvalidRequest)?;
    let [filter] = filters.as_slice() else {
        return Err(SnapshotError::InvalidRequest);
    };
    let object = filter.as_object().ok_or(SnapshotError::InvalidRequest)?;
    if object.len() != 3
        || object.get("read_state_snapshot").and_then(Value::as_u64) != Some(1)
        || object.get("kinds") != Some(&serde_json::json!([30078]))
        || object.get("authors") != Some(&serde_json::json!([authenticated_pubkey]))
    {
        return Err(SnapshotError::InvalidRequest);
    }
    Ok(Some(SnapshotRequest {
        pubkey: authenticated_pubkey.into(),
    }))
}

/// A point-in-time envelope. This is not a CAS token or a live-delivery cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadStateSnapshot {
    pub read_state_snapshot: u8,
    pub complete: bool,
    pub community_id: String,
    pub pubkey: String,
    pub snapshot_id: String,
    pub events: Vec<Event>,
}

/// Decode a bounded raw response without losing duplicate keys or extra event
/// fields, then authenticate it against origin-bound discovery and identity.
pub fn parse_snapshot(
    bytes: &[u8],
    descriptor: &SnapshotDescriptor,
    expected_pubkey: &str,
) -> Result<ReadStateSnapshot, SnapshotError> {
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(SnapshotError::LimitExceeded);
    }
    let value = crate::contracts::parse_strict_bounded(bytes, MAX_ENVELOPE_BYTES)
        .map_err(|_| SnapshotError::InvalidEnvelope)?;
    for event in value
        .get("events")
        .and_then(Value::as_array)
        .ok_or(SnapshotError::InvalidEnvelope)?
    {
        let fields = event.as_object().ok_or(SnapshotError::InvalidEnvelope)?;
        if fields.len() != 7
            || fields.keys().any(|key| {
                ![
                    "id",
                    "pubkey",
                    "created_at",
                    "kind",
                    "tags",
                    "content",
                    "sig",
                ]
                .contains(&key.as_str())
            })
        {
            return Err(SnapshotError::InvalidEnvelope);
        }
    }
    let snapshot: ReadStateSnapshot =
        serde_json::from_value(value).map_err(|_| SnapshotError::InvalidEnvelope)?;
    validate_snapshot(&snapshot, descriptor, expected_pubkey)?;
    Ok(snapshot)
}

/// Build an envelope from one complete, authorized writer-database cut.
///
/// The caller must enumerate all non-deleted own-author kind-30078 coordinates
/// in that statement, without pagination, ordinary query limits, or replicas.
/// `stored_content_and_tag_bytes` is the database's sum of content bytes and
/// PostgreSQL JSON tag text bytes, measured before decoding or filtering rows.
pub fn build_snapshot(
    community_id: &str,
    pubkey: &str,
    mut events: Vec<Event>,
    stored_content_and_tag_bytes: usize,
) -> Result<ReadStateSnapshot, SnapshotError> {
    if stored_content_and_tag_bytes > MAX_EVENT_ARRAY_BYTES {
        return Err(SnapshotError::LimitExceeded);
    }
    validate_events(&events, pubkey)?;
    events.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    let snapshot_id = snapshot_id(community_id, pubkey, &events)?;
    Ok(ReadStateSnapshot {
        read_state_snapshot: 1,
        complete: true,
        community_id: community_id.into(),
        pubkey: pubkey.into(),
        snapshot_id,
        events,
    })
}

/// Check a response against discovery and the authenticated signing identity.
///
/// The caller must bind discovery and this response to the same trusted origin
/// and community session, fence identity changes, and decrypt recognized
/// coordinates before relying on the view for a destructive operation.
pub fn validate_snapshot(
    snapshot: &ReadStateSnapshot,
    descriptor: &SnapshotDescriptor,
    expected_pubkey: &str,
) -> Result<(), SnapshotError> {
    descriptor.validate()?;
    if snapshot.community_id != descriptor.community_id || snapshot.pubkey != expected_pubkey {
        return Err(SnapshotError::IdentityMismatch);
    }
    if snapshot.read_state_snapshot != 1 || !snapshot.complete {
        return Err(SnapshotError::InvalidEnvelope);
    }
    validate_events(&snapshot.events, expected_pubkey)?;
    if snapshot.snapshot_id
        != snapshot_id(&snapshot.community_id, expected_pubkey, &snapshot.events)?
    {
        return Err(SnapshotError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_events(events: &[Event], pubkey: &str) -> Result<(), SnapshotError> {
    hex_bytes::<32>(pubkey)?;
    if events.len() > MAX_EVENTS {
        return Err(SnapshotError::LimitExceeded);
    }
    if serde_json::to_vec(events)
        .map_err(|_| SnapshotError::InvalidEnvelope)?
        .len()
        > MAX_EVENT_ARRAY_BYTES
    {
        return Err(SnapshotError::LimitExceeded);
    }
    let mut coordinates = HashSet::new();
    for event in events {
        if event.kind != 30078 || event.pubkey != pubkey {
            return Err(SnapshotError::IdentityMismatch);
        }
        event
            .validate_nip01_structure()
            .map_err(|_| SnapshotError::InvalidEvent)?;
        event
            .validate_crypto()
            .map_err(|_| SnapshotError::InvalidEvent)?;
        if !coordinates.insert(event.distinct_parameter().unwrap_or_default()) {
            return Err(SnapshotError::InvalidEnvelope);
        }
    }
    Ok(())
}

fn snapshot_id(community: &str, pubkey: &str, events: &[Event]) -> Result<String, SnapshotError> {
    let mut hasher = Sha256::new();
    hasher.update(b"buzz-read-state-snapshot-v1\0");
    hasher.update(uuid_bytes(community)?);
    hasher.update(hex_bytes::<32>(pubkey)?);
    let mut ordered = events.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    for event in ordered {
        hasher.update(hex_bytes::<32>(&event.id)?);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn uuid_bytes(value: &str) -> Result<[u8; 16], SnapshotError> {
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte != b'-'
            } else {
                !matches!(byte, b'0'..=b'9' | b'a'..=b'f')
            }
        })
    {
        return Err(SnapshotError::InvalidEnvelope);
    }
    hex_bytes::<16>(&value.replace('-', ""))
}

pub(crate) fn hex_bytes<const N: usize>(value: &str) -> Result<[u8; N], SnapshotError> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(SnapshotError::InvalidEnvelope);
    }
    let mut bytes = [0; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| SnapshotError::InvalidEnvelope)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    const COMMUNITY: &str = "00000000-0000-0000-0000-000000000001";

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{:064x}", 1)).unwrap()
    }

    fn event(d: &str, time: u64) -> Event {
        signer().sign(
            time,
            30078,
            vec![Tag::new(vec!["d".into(), d.into()])],
            "opaque".into(),
        )
    }

    #[test]
    fn request_requires_exact_self_and_no_other_filters() {
        let key = signer().pubkey().to_owned();
        let valid = serde_json::json!([{"kinds":[30078],"authors":[key],"read_state_snapshot":1}]);
        assert_eq!(parse_request(&valid, &key).unwrap().unwrap().pubkey, key);
        for invalid in [
            serde_json::json!([{"kinds":[30078],"authors":[key],"read_state_snapshot":0}]),
            serde_json::json!([{"kinds":[30078],"authors":[key],"read_state_snapshot":1,"limit":1}]),
            serde_json::json!([valid[0], {}]),
        ] {
            assert_eq!(
                parse_request(&invalid, &key),
                Err(SnapshotError::InvalidRequest)
            );
        }
        assert!(
            parse_request(&serde_json::json!([{"kinds":[9]}]), &key)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cut_keeps_unrelated_coordinates_and_binds_identity_and_events() {
        let key = signer().pubkey().to_owned();
        let descriptor = SnapshotDescriptor::new(COMMUNITY).unwrap();
        let cut = build_snapshot(
            COMMUNITY,
            &key,
            vec![event("other-app", 1), event("read-state", 2)],
            100,
        )
        .unwrap();
        assert_eq!(cut.events[0].created_at, 2);
        validate_snapshot(&cut, &descriptor, &key).unwrap();
        let mut altered = cut.clone();
        altered.events[0].content.push('x');
        assert_eq!(
            validate_snapshot(&altered, &descriptor, &key),
            Err(SnapshotError::InvalidEvent)
        );
        altered = cut.clone();
        altered.events.pop();
        assert_eq!(
            validate_snapshot(&altered, &descriptor, &key),
            Err(SnapshotError::InvalidEnvelope)
        );
        altered = cut;
        altered.complete = false;
        assert_eq!(
            validate_snapshot(&altered, &descriptor, &key),
            Err(SnapshotError::InvalidEnvelope)
        );
    }

    #[test]
    fn duplicate_coordinate_and_all_hard_bounds_fail_without_partial_envelope() {
        let key = signer().pubkey().to_owned();
        assert_eq!(
            build_snapshot(COMMUNITY, &key, vec![event("same", 1), event("same", 2)], 0),
            Err(SnapshotError::InvalidEnvelope)
        );
        assert_eq!(
            build_snapshot(COMMUNITY, &key, vec![], MAX_EVENT_ARRAY_BYTES + 1),
            Err(SnapshotError::LimitExceeded)
        );
        assert_eq!(
            build_snapshot(COMMUNITY, &key, vec![event("x", 1); MAX_EVENTS + 1], 0),
            Err(SnapshotError::LimitExceeded)
        );
        let mut huge = event("x", 1);
        huge.content = "x".repeat(MAX_EVENT_ARRAY_BYTES);
        assert_eq!(
            build_snapshot(COMMUNITY, &key, vec![huge], 0),
            Err(SnapshotError::LimitExceeded)
        );
    }

    #[test]
    fn empty_cut_has_a_stable_published_preimage() {
        let key = signer().pubkey().to_owned();
        let snapshot = build_snapshot(COMMUNITY, &key, vec![], 0).unwrap();
        let mut preimage = b"buzz-read-state-snapshot-v1\0".to_vec();
        preimage.extend([0_u8; 15]);
        preimage.push(1);
        preimage.extend(hex_bytes::<32>(&key).unwrap());
        let expected: String = Sha256::digest(preimage)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(snapshot.snapshot_id, expected);
    }

    #[test]
    fn raw_responses_refuse_ordinary_arrays_duplicate_keys_and_extra_event_fields() {
        let key = signer().pubkey().to_owned();
        let descriptor = SnapshotDescriptor::new(COMMUNITY).unwrap();
        let snapshot = build_snapshot(COMMUNITY, &key, vec![event("read-state", 1)], 0).unwrap();
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        assert_eq!(parse_snapshot(&bytes, &descriptor, &key).unwrap(), snapshot);
        assert_eq!(
            parse_snapshot(b"[]", &descriptor, &key),
            Err(SnapshotError::InvalidEnvelope)
        );
        let duplicate = String::from_utf8(bytes)
            .unwrap()
            .replacen("{", "{\"complete\":true,", 1);
        assert_eq!(
            parse_snapshot(duplicate.as_bytes(), &descriptor, &key),
            Err(SnapshotError::InvalidEnvelope)
        );
        let mut value = serde_json::to_value(&snapshot).unwrap();
        value["events"][0]["extra"] = true.into();
        assert_eq!(
            parse_snapshot(&serde_json::to_vec(&value).unwrap(), &descriptor, &key),
            Err(SnapshotError::InvalidEnvelope)
        );
    }
}
