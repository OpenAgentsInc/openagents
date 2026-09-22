//! NIP-77 Negentropy protocol version 1.
//!
//! The pinned specification defines the frame, the varint, the bound, the
//! three range modes, and the fingerprint. This module speaks that frame.
//! When a fingerprint differs and the range holds more than
//! [`ID_LIST_THRESHOLD`] records, the responder splits the range in half.
//! The split point is this implementation's choice. The bytes on the wire
//! stay inside the specified modes.

use sha2::{Digest, Sha256};

/// Protocol version 1.
pub const PROTOCOL_VERSION: u8 = 0x61;
/// A range this small is answered with every id it contains.
pub const ID_LIST_THRESHOLD: usize = 16;
/// Timestamp reserved as infinity. A record must not use it.
pub const INFINITY: u64 = u64::MAX;

/// One stored record: a timestamp and a 32-byte id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// Unix seconds, or another 64-bit clock the two sides share.
    pub timestamp: u64,
    /// Event id.
    pub id: [u8; 32],
}

/// Why a sync frame was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum SyncError {
    /// The bytes are not a version-1 message.
    Malformed,
    /// The other side spoke a version this side does not implement.
    UnsupportedVersion {
        /// The highest version this side can answer with.
        supported: u8,
    },
}

/// Inclusive lower bound and exclusive upper bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bound {
    timestamp: u64,
    id: [u8; 32],
}

/// One range from a decoded message.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    Skip,
    Fingerprint([u8; 16]),
    IdList(Vec<[u8; 32]>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Range {
    upper: Bound,
    mode: Mode,
}

const ZERO: Bound = Bound {
    timestamp: 0,
    id: [0; 32],
};

const END: Bound = Bound {
    timestamp: INFINITY,
    id: [0; 32],
};

/// Sort `items` the way the protocol compares them.
pub fn prepare(items: &mut Vec<Item>) {
    items.sort_by(order);
    items.retain(|item| item.timestamp != INFINITY);
    items.dedup_by(|left, right| left.timestamp == right.timestamp && left.id == right.id);
}

/// Fingerprint of `ids` in their current order. Order does not change the sum.
#[must_use]
pub fn fingerprint(ids: &[[u8; 32]]) -> [u8; 16] {
    let mut sum = [0u8; 32];
    for id in ids {
        add_le(&mut sum, id);
    }
    let mut body = Vec::with_capacity(40);
    body.extend(sum);
    encode_varint(ids.len() as u64, &mut body);
    let digest = Sha256::digest(body);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// The server's next message for `local` given the initiator's `message`.
///
/// # Errors
///
/// Returns [`SyncError::UnsupportedVersion`] when the version byte is not
/// [`PROTOCOL_VERSION`]. The `supported` byte is what the reply frame carries.
pub fn respond(local: &[Item], message: &[u8]) -> Result<Vec<u8>, SyncError> {
    if message.first() != Some(&PROTOCOL_VERSION) {
        return Err(SyncError::UnsupportedVersion {
            supported: PROTOCOL_VERSION,
        });
    }
    let ranges = decode_ranges(&message[1..])?;
    let mut out = Vec::new();
    for range in ranges {
        respond_range(local, &range, &mut out)?;
    }
    encode_message(&out)
}

/// What the initiator still needs to ask, and the ids each side is missing.
#[derive(Debug, PartialEq, Eq)]
pub struct Step {
    /// Hex is not applied here. These are the next protocol bytes, or empty
    /// when the initiator has nothing left to send.
    pub next: Vec<u8>,
    /// Ids the initiator has and the responder does not.
    pub have: Vec<[u8; 32]>,
    /// Ids the responder has and the initiator does not.
    pub need: Vec<[u8; 32]>,
}

/// Read a responder message and build the initiator's next request.
///
/// # Errors
///
/// Returns [`SyncError`] when `message` is not a version-1 frame.
pub fn continue_sync(local: &[Item], message: &[u8]) -> Result<Step, SyncError> {
    if message.first() != Some(&PROTOCOL_VERSION) {
        return Err(SyncError::UnsupportedVersion {
            supported: PROTOCOL_VERSION,
        });
    }
    let ranges = decode_ranges(&message[1..])?;
    let mut next = Vec::new();
    let mut have = Vec::new();
    let mut need = Vec::new();
    let mut lower = ZERO;
    for range in ranges {
        let slice = slice_of(local, lower, range.upper);
        match range.mode {
            Mode::Skip => {}
            Mode::Fingerprint(_) => {
                next.push(Range {
                    upper: range.upper,
                    mode: mode_for(slice),
                });
            }
            Mode::IdList(ids) => {
                let theirs: std::collections::BTreeSet<[u8; 32]> = ids.into_iter().collect();
                for item in slice {
                    if !theirs.contains(&item.id) {
                        have.push(item.id);
                    }
                }
                for id in &theirs {
                    if !slice.iter().any(|item| item.id == *id) {
                        need.push(*id);
                    }
                }
            }
        }
        lower = range.upper;
    }
    let bytes = if next.is_empty() {
        Vec::new()
    } else {
        encode_message(&next)?
    };
    Ok(Step {
        next: bytes,
        have,
        need,
    })
}

/// The initiator's first message: one fingerprint over the whole space.
#[must_use]
pub fn open(local: &[Item]) -> Vec<u8> {
    encode_message(&[Range {
        upper: END,
        mode: mode_for(local),
    }])
    .expect("a fingerprint frame encodes")
}

fn mode_for(slice: &[Item]) -> Mode {
    if slice.len() <= ID_LIST_THRESHOLD {
        Mode::IdList(slice.iter().map(|item| item.id).collect())
    } else {
        Mode::Fingerprint(fingerprint(&ids_of(slice)))
    }
}

fn respond_range(local: &[Item], range: &Range, out: &mut Vec<Range>) -> Result<(), SyncError> {
    match &range.mode {
        Mode::Skip => out.push(Range {
            upper: range.upper,
            mode: Mode::Skip,
        }),
        Mode::Fingerprint(theirs) => {
            // The lower bound is recovered by the caller through `out`'s
            // previous upper bound. `respond` walks ranges in order, so the
            // slice is taken against the bound this range starts at.
            let start = out.last().map(|item| item.upper).unwrap_or(ZERO);
            split_fingerprint(local, start, range.upper, *theirs, out);
        }
        Mode::IdList(_) => {
            let start = out.last().map(|item| item.upper).unwrap_or(ZERO);
            let slice = slice_of(local, start, range.upper);
            out.push(Range {
                upper: range.upper,
                mode: Mode::IdList(slice.iter().map(|item| item.id).collect()),
            });
        }
    }
    Ok(())
}

fn split_fingerprint(
    local: &[Item],
    lower: Bound,
    upper: Bound,
    theirs: [u8; 16],
    out: &mut Vec<Range>,
) {
    let slice = slice_of(local, lower, upper);
    let mine = fingerprint(&ids_of(slice));
    if mine == theirs || slice.len() <= ID_LIST_THRESHOLD {
        let mode = if mine == theirs {
            Mode::Skip
        } else {
            Mode::IdList(slice.iter().map(|item| item.id).collect())
        };
        out.push(Range { upper, mode });
        return;
    }
    let mid = slice[slice.len() / 2];
    let middle = Bound {
        timestamp: mid.timestamp,
        id: mid.id,
    };
    let left = slice_of(local, lower, middle);
    let right = slice_of(local, middle, upper);
    out.push(Range {
        upper: middle,
        mode: Mode::Fingerprint(fingerprint(&ids_of(left))),
    });
    out.push(Range {
        upper,
        mode: Mode::Fingerprint(fingerprint(&ids_of(right))),
    });
}

fn slice_of(local: &[Item], lower: Bound, upper: Bound) -> &[Item] {
    let start = local.partition_point(|item| order_bound(item, lower).is_lt());
    let end = local.partition_point(|item| order_bound(item, upper).is_lt());
    &local[start..end]
}

fn ids_of(items: &[Item]) -> Vec<[u8; 32]> {
    items.iter().map(|item| item.id).collect()
}

fn order(left: &Item, right: &Item) -> std::cmp::Ordering {
    left.timestamp
        .cmp(&right.timestamp)
        .then_with(|| left.id.cmp(&right.id))
}

fn order_bound(item: &Item, bound: Bound) -> std::cmp::Ordering {
    item.timestamp
        .cmp(&bound.timestamp)
        .then_with(|| item.id.cmp(&bound.id))
}

fn encode_message(ranges: &[Range]) -> Result<Vec<u8>, SyncError> {
    let mut out = vec![PROTOCOL_VERSION];
    let mut previous = 0u64;
    for range in ranges {
        encode_bound(range.upper, &mut previous, &mut out)?;
        match &range.mode {
            Mode::Skip => encode_varint(0, &mut out),
            Mode::Fingerprint(bytes) => {
                encode_varint(1, &mut out);
                out.extend(bytes);
            }
            Mode::IdList(ids) => {
                encode_varint(2, &mut out);
                encode_varint(ids.len() as u64, &mut out);
                for id in ids {
                    out.extend(id);
                }
            }
        }
    }
    if ranges
        .last()
        .is_none_or(|range| range.upper.timestamp != INFINITY)
    {
        encode_bound(END, &mut previous, &mut out)?;
        encode_varint(0, &mut out);
    }
    Ok(out)
}

fn encode_bound(bound: Bound, previous: &mut u64, out: &mut Vec<u8>) -> Result<(), SyncError> {
    if bound.timestamp == INFINITY {
        encode_varint(0, out);
    } else {
        let offset = bound
            .timestamp
            .checked_sub(*previous)
            .ok_or(SyncError::Malformed)?;
        encode_varint(offset.saturating_add(1), out);
        *previous = bound.timestamp;
    }
    let prefix = significant_prefix(&bound.id);
    encode_varint(prefix.len() as u64, out);
    out.extend(prefix);
    Ok(())
}

fn significant_prefix(id: &[u8; 32]) -> &[u8] {
    let end = id
        .iter()
        .rposition(|byte| *byte != 0)
        .map(|index| index + 1);
    match end {
        Some(end) => &id[..end],
        None => &[],
    }
}

fn decode_ranges(bytes: &[u8]) -> Result<Vec<Range>, SyncError> {
    let mut cursor = 0;
    let mut previous = 0u64;
    let mut lower_seen_infinity = false;
    let mut ranges = Vec::new();
    while cursor < bytes.len() {
        if lower_seen_infinity {
            return Err(SyncError::Malformed);
        }
        let (upper, next) = decode_bound(bytes, cursor, &mut previous)?;
        cursor = next;
        if upper.timestamp == INFINITY {
            lower_seen_infinity = true;
        }
        let (mode_byte, next) = decode_varint(bytes, cursor)?;
        cursor = next;
        let mode = match mode_byte {
            0 => Mode::Skip,
            1 => {
                let end = cursor + 16;
                let bytes = bytes.get(cursor..end).ok_or(SyncError::Malformed)?;
                cursor = end;
                let mut fingerprint = [0u8; 16];
                fingerprint.copy_from_slice(bytes);
                Mode::Fingerprint(fingerprint)
            }
            2 => {
                let (len, next) = decode_varint(bytes, cursor)?;
                cursor = next;
                let count = usize::try_from(len).map_err(|_| SyncError::Malformed)?;
                let mut ids = Vec::with_capacity(count);
                for _ in 0..count {
                    let end = cursor + 32;
                    let raw = bytes.get(cursor..end).ok_or(SyncError::Malformed)?;
                    cursor = end;
                    let mut id = [0u8; 32];
                    id.copy_from_slice(raw);
                    ids.push(id);
                }
                Mode::IdList(ids)
            }
            _ => return Err(SyncError::Malformed),
        };
        ranges.push(Range { upper, mode });
    }
    if !lower_seen_infinity {
        ranges.push(Range {
            upper: END,
            mode: Mode::Skip,
        });
    }
    Ok(ranges)
}

fn decode_bound(
    bytes: &[u8],
    cursor: usize,
    previous: &mut u64,
) -> Result<(Bound, usize), SyncError> {
    let (encoded, cursor) = decode_varint(bytes, cursor)?;
    let timestamp = if encoded == 0 {
        INFINITY
    } else {
        let timestamp = previous
            .checked_add(encoded - 1)
            .ok_or(SyncError::Malformed)?;
        *previous = timestamp;
        timestamp
    };
    let (length, cursor) = decode_varint(bytes, cursor)?;
    let length = usize::try_from(length).map_err(|_| SyncError::Malformed)?;
    if length > 32 {
        return Err(SyncError::Malformed);
    }
    let end = cursor + length;
    let prefix = bytes.get(cursor..end).ok_or(SyncError::Malformed)?;
    let mut id = [0u8; 32];
    id[..length].copy_from_slice(prefix);
    Ok((Bound { timestamp, id }, end))
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    if value == 0 {
        out.push(0);
        return;
    }
    let mut digits = Vec::new();
    while value > 0 {
        digits.push((value % 128) as u8);
        value /= 128;
    }
    digits.reverse();
    let last = digits.len() - 1;
    for (index, digit) in digits.into_iter().enumerate() {
        if index == last {
            out.push(digit);
        } else {
            out.push(digit | 0x80);
        }
    }
}

fn decode_varint(bytes: &[u8], mut cursor: usize) -> Result<(u64, usize), SyncError> {
    let mut value = 0u64;
    for _ in 0..10 {
        let digit = *bytes.get(cursor).ok_or(SyncError::Malformed)?;
        cursor += 1;
        let piece = u64::from(digit & 0x7f);
        value = value.checked_mul(128).ok_or(SyncError::Malformed)?;
        value = value.checked_add(piece).ok_or(SyncError::Malformed)?;
        if digit & 0x80 == 0 {
            return Ok((value, cursor));
        }
    }
    Err(SyncError::Malformed)
}

fn add_le(sum: &mut [u8; 32], id: &[u8; 32]) {
    let mut carry = 0u16;
    for index in 0..32 {
        let total = u16::from(sum[index]) + u16::from(id[index]) + carry;
        sum[index] = total as u8;
        carry = total >> 8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(timestamp: u64, byte: u8) -> Item {
        let mut id = [0u8; 32];
        id[0] = byte;
        id[31] = byte;
        Item { timestamp, id }
    }

    #[test]
    fn the_fingerprint_is_a_little_endian_sum_then_sha256() {
        let ids = [item(1, 1).id, item(1, 2).id];
        let again = fingerprint(&[ids[1], ids[0]]);
        assert_eq!(fingerprint(&ids), again);
        assert_ne!(fingerprint(&ids), fingerprint(&[ids[0]]));
    }

    #[test]
    fn an_unknown_version_names_version_one() {
        let error = respond(&[], &[0x62]).unwrap_err();
        assert_eq!(
            error,
            SyncError::UnsupportedVersion {
                supported: PROTOCOL_VERSION
            }
        );
    }

    #[test]
    fn two_stores_learn_the_symmetric_difference() {
        let mut server = vec![item(10, 1), item(10, 2), item(20, 3), item(30, 4)];
        // Pad past the id-list threshold so the first answer is a split.
        for byte in 10..40 {
            server.push(item(40, byte));
        }
        prepare(&mut server);
        let mut client = vec![item(10, 1), item(20, 9), item(30, 4)];
        for byte in 10..40 {
            client.push(item(40, byte));
        }
        prepare(&mut client);

        let mut message = open(&client);
        let mut have = Vec::new();
        let mut need = Vec::new();
        for _ in 0..8 {
            let reply = respond(&server, &message).unwrap();
            let step = continue_sync(&client, &reply).unwrap();
            have.extend(step.have);
            need.extend(step.need);
            if step.next.is_empty() {
                message = Vec::new();
                break;
            }
            message = step.next;
        }
        assert!(message.is_empty(), "rounds left");
        have.sort();
        need.sort();
        assert_eq!(have.iter().map(|id| id[0]).collect::<Vec<_>>(), vec![9]);
        assert_eq!(need.iter().map(|id| id[0]).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn a_truncated_frame_is_malformed() {
        assert_eq!(
            respond(&[], &[PROTOCOL_VERSION, 0x80]),
            Err(SyncError::Malformed)
        );
    }
}
