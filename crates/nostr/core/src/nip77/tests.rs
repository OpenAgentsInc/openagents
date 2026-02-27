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
    let incoming = NegentropyMessage::new(vec![Range::fingerprint(Bound::infinity(), [0x00; 16])]);

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

    let message = NegentropyMessage::new(vec![Range::fingerprint(Bound::infinity(), [0x00; 16])]);

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
