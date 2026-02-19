//! NIP-13: Proof of Work
//!
//! Defines how to use proof-of-work to combat spam by requiring computational
//! effort for event creation. Difficulty is measured by leading zero bits in event ID.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/13.md>

use crate::Event;
use thiserror::Error;

/// Errors that can occur during NIP-13 operations
#[derive(Debug, Error)]
pub enum Nip13Error {
    #[error("invalid event ID format: {0}")]
    InvalidEventId(String),

    #[error("invalid nonce tag format: {0}")]
    InvalidNonceTag(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Calculate the difficulty (number of leading zero bits) from an event ID
///
/// The event ID is a 64-character hex string representing 32 bytes.
/// This counts the total number of leading zero bits.
pub fn calculate_difficulty(event_id: &str) -> Result<u32, Nip13Error> {
    if event_id.len() != 64 {
        return Err(Nip13Error::InvalidEventId(format!(
            "event ID must be 64 hex characters, got {}",
            event_id.len()
        )));
    }

    // Validate hex
    if !event_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Nip13Error::InvalidEventId(
            "event ID must be valid hex".to_string(),
        ));
    }

    let mut leading_zeros = 0u32;

    // Iterate through each hex character (nibble = 4 bits)
    for c in event_id.chars() {
        let nibble = c.to_digit(16).unwrap(); // Safe because we validated hex above

        if nibble == 0 {
            // All 4 bits are zero
            leading_zeros += 4;
        } else {
            // Count leading zeros in this nibble and stop
            leading_zeros += nibble.leading_zeros() - 28; // 28 because we only care about low 4 bits
            break;
        }
    }

    Ok(leading_zeros)
}

/// Parse the nonce tag from an event
///
/// Returns (nonce_value, target_difficulty) if the tag exists and is valid
pub fn parse_nonce_tag(event: &Event) -> Result<Option<(String, Option<u32>)>, Nip13Error> {
    for tag in &event.tags {
        if !tag.is_empty() && tag[0] == "nonce" {
            if tag.len() < 2 {
                return Err(Nip13Error::InvalidNonceTag(
                    "nonce tag must have at least 2 elements".to_string(),
                ));
            }

            let nonce_value = tag[1].clone();

            // Third element is optional target difficulty
            let target_difficulty = if tag.len() > 2 {
                tag[2].parse::<u32>().ok()
            } else {
                None
            };

            return Ok(Some((nonce_value, target_difficulty)));
        }
    }

    Ok(None)
}

/// Check if an event meets the minimum difficulty requirement
///
/// Returns true if the event's actual difficulty is >= min_difficulty
pub fn validate_pow(event: &Event, min_difficulty: u32) -> Result<bool, Nip13Error> {
    let actual_difficulty = calculate_difficulty(&event.id)?;
    Ok(actual_difficulty >= min_difficulty)
}

/// Check if an event has a valid proof-of-work
///
/// This checks:
/// 1. If the event has a nonce tag with target difficulty
/// 2. If the actual difficulty meets the target
///
/// Returns (has_pow, meets_target) where:
/// - has_pow: true if nonce tag exists
/// - meets_target: true if actual difficulty >= target (or None if no target specified)
pub fn check_pow(event: &Event) -> Result<(bool, Option<bool>), Nip13Error> {
    let nonce = parse_nonce_tag(event)?;

    if let Some((_, target_difficulty)) = nonce {
        let actual_difficulty = calculate_difficulty(&event.id)?;

        if let Some(target) = target_difficulty {
            Ok((true, Some(actual_difficulty >= target)))
        } else {
            Ok((true, None))
        }
    } else {
        Ok((false, None))
    }
}

/// Get the actual difficulty of an event
pub fn get_difficulty(event: &Event) -> Result<u32, Nip13Error> {
    calculate_difficulty(&event.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_difficulty_all_zeros() {
        // 64 zeros = 256 bits
        let id = "0".repeat(64);
        assert_eq!(calculate_difficulty(&id).unwrap(), 256);
    }

    #[test]
    fn test_calculate_difficulty_36_bits() {
        // Example from spec: 36 leading zero bits
        let id = "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d";
        assert_eq!(calculate_difficulty(id).unwrap(), 36);
    }

    #[test]
    fn test_calculate_difficulty_10_bits() {
        // "002f" = "0000 0000 0010 1111" = 10 leading zeros
        let id = format!("002f{}", "0".repeat(60));
        assert_eq!(calculate_difficulty(&id).unwrap(), 10);
    }

    #[test]
    fn test_calculate_difficulty_8_bits() {
        // "00ff" = "0000 0000 1111 1111" = 8 leading zeros
        let id = format!("00ff{}", "0".repeat(60));
        assert_eq!(calculate_difficulty(&id).unwrap(), 8);
    }

    #[test]
    fn test_calculate_difficulty_4_bits() {
        // "0f" = "0000 1111" = 4 leading zeros
        let id = format!("0f{}", "0".repeat(62));
        assert_eq!(calculate_difficulty(&id).unwrap(), 4);
    }

    #[test]
    fn test_calculate_difficulty_no_zeros() {
        // "f" = "1111" = 0 leading zeros
        let id = format!("f{}", "0".repeat(63));
        assert_eq!(calculate_difficulty(&id).unwrap(), 0);
    }

    #[test]
    fn test_calculate_difficulty_1_bit() {
        // "7" = "0111" = 1 leading zero
        let id = format!("7{}", "0".repeat(63));
        assert_eq!(calculate_difficulty(&id).unwrap(), 1);
    }

    #[test]
    fn test_calculate_difficulty_2_bits() {
        // "3" = "0011" = 2 leading zeros
        let id = format!("3{}", "0".repeat(63));
        assert_eq!(calculate_difficulty(&id).unwrap(), 2);
    }

    #[test]
    fn test_calculate_difficulty_3_bits() {
        // "1" = "0001" = 3 leading zeros
        let id = format!("1{}", "0".repeat(63));
        assert_eq!(calculate_difficulty(&id).unwrap(), 3);
    }

    #[test]
    fn test_calculate_difficulty_invalid_length() {
        let result = calculate_difficulty("abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_difficulty_invalid_hex() {
        let id = format!("g{}", "0".repeat(63));
        let result = calculate_difficulty(&id);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_nonce_tag_with_target() {
        let event = Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec![
                "nonce".to_string(),
                "12345".to_string(),
                "20".to_string(),
            ]],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        let nonce = parse_nonce_tag(&event).unwrap();
        assert!(nonce.is_some());
        let (nonce_value, target) = nonce.unwrap();
        assert_eq!(nonce_value, "12345");
        assert_eq!(target, Some(20));
    }

    #[test]
    fn test_parse_nonce_tag_without_target() {
        let event = Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["nonce".to_string(), "12345".to_string()]],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        let nonce = parse_nonce_tag(&event).unwrap();
        assert!(nonce.is_some());
        let (nonce_value, target) = nonce.unwrap();
        assert_eq!(nonce_value, "12345");
        assert_eq!(target, None);
    }

    #[test]
    fn test_parse_nonce_tag_none() {
        let event = Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        let nonce = parse_nonce_tag(&event).unwrap();
        assert!(nonce.is_none());
    }

    #[test]
    fn test_validate_pow_passes() {
        let event = Event {
            id: "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        // Event has 36 leading zeros, so it passes for min_difficulty <= 36
        assert!(validate_pow(&event, 20).unwrap());
        assert!(validate_pow(&event, 36).unwrap());
    }

    #[test]
    fn test_validate_pow_fails() {
        let event = Event {
            id: "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        // Event has 36 leading zeros, so it fails for min_difficulty > 36
        assert!(!validate_pow(&event, 40).unwrap());
    }

    #[test]
    fn test_check_pow_with_target() {
        let event = Event {
            id: "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec![
                "nonce".to_string(),
                "12345".to_string(),
                "30".to_string(),
            ]],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        let (has_pow, meets_target) = check_pow(&event).unwrap();
        assert!(has_pow);
        assert_eq!(meets_target, Some(true)); // 36 >= 30
    }

    #[test]
    fn test_check_pow_fails_target() {
        let event = Event {
            id: "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec![
                "nonce".to_string(),
                "12345".to_string(),
                "40".to_string(),
            ]],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        let (has_pow, meets_target) = check_pow(&event).unwrap();
        assert!(has_pow);
        assert_eq!(meets_target, Some(false)); // 36 < 40
    }

    #[test]
    fn test_get_difficulty() {
        let event = Event {
            id: "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
            sig: "test_sig".to_string(),
        };

        assert_eq!(get_difficulty(&event).unwrap(), 36);
    }
}
