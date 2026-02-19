//! NIP-26: Delegated Event Signing
//!
//! **WARNING**: This NIP is unrecommended as it adds unnecessary burden for little gain.
//! Consider using NIP-46 (Nostr Connect) for most delegation use cases instead.
//!
//! Implements event delegation allowing one keypair to authorize another to sign events
//! on its behalf with specific conditions:
//! - Restricted event kinds (kind=N)
//! - Time boundaries (created_at>T, created_at<T)
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/26.md>

use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during NIP-26 operations
#[derive(Debug, Error)]
pub enum Nip26Error {
    #[error("invalid delegation format: {0}")]
    InvalidFormat(String),

    #[error("invalid condition: {0}")]
    InvalidCondition(String),

    #[error("invalid signature: {0}")]
    InvalidSignature(String),

    #[error("delegation validation failed: {0}")]
    ValidationFailed(String),

    #[error("hex decode error: {0}")]
    HexDecode(String),

    #[error("invalid key: {0}")]
    InvalidKey(String),

    #[error("condition not satisfied: {0}")]
    ConditionNotSatisfied(String),
}

/// Represents a delegation condition
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// Event kind must equal specified value
    Kind(u16),
    /// Event created_at must be greater than specified timestamp
    CreatedAtAfter(u64),
    /// Event created_at must be less than specified timestamp
    CreatedAtBefore(u64),
}

impl Condition {
    /// Parse a single condition from string
    ///
    /// # Examples
    /// - "kind=1" -> Kind(1)
    /// - "created_at>1674834236" -> CreatedAtAfter(1674834236)
    /// - "created_at<1677426236" -> CreatedAtBefore(1677426236)
    pub fn parse(s: &str) -> Result<Self, Nip26Error> {
        if let Some(value) = s.strip_prefix("kind=") {
            let kind = value
                .parse::<u16>()
                .map_err(|e| Nip26Error::InvalidCondition(format!("invalid kind: {}", e)))?;
            Ok(Condition::Kind(kind))
        } else if let Some(value) = s.strip_prefix("created_at>") {
            let timestamp = value.parse::<u64>().map_err(|e| {
                Nip26Error::InvalidCondition(format!("invalid created_at timestamp: {}", e))
            })?;
            Ok(Condition::CreatedAtAfter(timestamp))
        } else if let Some(value) = s.strip_prefix("created_at<") {
            let timestamp = value.parse::<u64>().map_err(|e| {
                Nip26Error::InvalidCondition(format!("invalid created_at timestamp: {}", e))
            })?;
            Ok(Condition::CreatedAtBefore(timestamp))
        } else {
            Err(Nip26Error::InvalidCondition(format!(
                "unknown condition format: {}",
                s
            )))
        }
    }

    /// Check if an event satisfies this condition
    pub fn is_satisfied(&self, kind: u16, created_at: u64) -> bool {
        match self {
            Condition::Kind(k) => kind == *k,
            Condition::CreatedAtAfter(t) => created_at > *t,
            Condition::CreatedAtBefore(t) => created_at < *t,
        }
    }
}

/// Parse conditions query string
///
/// # Example
/// ```ignore
/// let conditions = parse_conditions("kind=1&created_at>1674834236&created_at<1677426236")?;
/// ```
pub fn parse_conditions(query: &str) -> Result<Vec<Condition>, Nip26Error> {
    query.split('&').map(Condition::parse).collect()
}

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Condition::Kind(k) => write!(f, "kind={}", k),
            Condition::CreatedAtAfter(t) => write!(f, "created_at>{}", t),
            Condition::CreatedAtBefore(t) => write!(f, "created_at<{}", t),
        }
    }
}

/// Convert conditions to query string
pub fn conditions_to_string(conditions: &[Condition]) -> String {
    conditions
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("&")
}

/// Create delegation string for signing
///
/// Format: `nostr:delegation:<delegatee_pubkey>:<conditions>`
///
/// # Arguments
/// * `delegatee_pubkey` - Public key of the delegatee (hex string)
/// * `conditions` - Condition query string (e.g., "kind=1&created_at>1674834236")
pub fn create_delegation_string(delegatee_pubkey: &str, conditions: &str) -> String {
    format!("nostr:delegation:{}:{}", delegatee_pubkey, conditions)
}

/// Sign a delegation string to create a delegation token
///
/// # Arguments
/// * `delegator_privkey` - Delegator's private key (32 bytes)
/// * `delegatee_pubkey` - Delegatee's public key (hex string)
/// * `conditions` - Condition query string
///
/// # Returns
/// 64-byte Schnorr signature as hex string
pub fn create_delegation_token(
    delegator_privkey: &[u8; 32],
    delegatee_pubkey: &str,
    conditions: &str,
) -> Result<String, Nip26Error> {
    // Validate conditions by parsing them
    parse_conditions(conditions)?;

    // Create delegation string
    let delegation_string = create_delegation_string(delegatee_pubkey, conditions);

    // Hash the delegation string
    let mut hasher = Sha256::new();
    hasher.update(delegation_string.as_bytes());
    let hash = hasher.finalize();

    // Sign the hash
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(delegator_privkey)
        .map_err(|e| Nip26Error::InvalidKey(e.to_string()))?;

    let message = Message::from_digest_slice(&hash)
        .map_err(|e| Nip26Error::InvalidSignature(e.to_string()))?;

    let keypair = bitcoin::secp256k1::Keypair::from_secret_key(&secp, &secret_key);
    let signature = secp.sign_schnorr_no_aux_rand(&message, &keypair);

    Ok(hex::encode(signature.as_ref()))
}

/// Verify a delegation token
///
/// # Arguments
/// * `delegator_pubkey` - Delegator's public key (hex string)
/// * `delegatee_pubkey` - Delegatee's public key (hex string)
/// * `conditions` - Condition query string
/// * `delegation_token` - 64-byte signature (hex string)
pub fn verify_delegation_token(
    delegator_pubkey: &str,
    delegatee_pubkey: &str,
    conditions: &str,
    delegation_token: &str,
) -> Result<(), Nip26Error> {
    // Parse and validate conditions
    parse_conditions(conditions)?;

    // Create delegation string
    let delegation_string = create_delegation_string(delegatee_pubkey, conditions);

    // Hash the delegation string
    let mut hasher = Sha256::new();
    hasher.update(delegation_string.as_bytes());
    let hash = hasher.finalize();

    // Decode signature
    let sig_bytes = hex::decode(delegation_token)
        .map_err(|e| Nip26Error::HexDecode(format!("delegation token: {}", e)))?;

    if sig_bytes.len() != 64 {
        return Err(Nip26Error::InvalidSignature(format!(
            "expected 64 bytes, got {}",
            sig_bytes.len()
        )));
    }

    let signature = bitcoin::secp256k1::schnorr::Signature::from_slice(&sig_bytes)
        .map_err(|e| Nip26Error::InvalidSignature(e.to_string()))?;

    // Decode delegator public key
    let pubkey_bytes = hex::decode(delegator_pubkey)
        .map_err(|e| Nip26Error::HexDecode(format!("delegator pubkey: {}", e)))?;

    let pubkey = bitcoin::secp256k1::XOnlyPublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| Nip26Error::InvalidKey(e.to_string()))?;

    // Verify signature
    let secp = Secp256k1::new();
    let message = Message::from_digest_slice(&hash)
        .map_err(|e| Nip26Error::InvalidSignature(e.to_string()))?;

    secp.verify_schnorr(&signature, &message, &pubkey)
        .map_err(|e| Nip26Error::InvalidSignature(format!("verification failed: {}", e)))?;

    Ok(())
}

/// Check if an event satisfies delegation conditions
///
/// Multiple conditions on the same field (e.g., kind=0&kind=1) are OR'd together.
/// Conditions on different fields (e.g., kind&created_at) are AND'd together.
///
/// # Arguments
/// * `conditions` - Condition query string
/// * `event_kind` - Event kind to check
/// * `event_created_at` - Event created_at timestamp to check
pub fn check_delegation_conditions(
    conditions: &str,
    event_kind: u16,
    event_created_at: u64,
) -> Result<(), Nip26Error> {
    let parsed_conditions = parse_conditions(conditions)?;

    // Separate conditions by type
    let kind_conditions: Vec<_> = parsed_conditions
        .iter()
        .filter_map(|c| match c {
            Condition::Kind(k) => Some(*k),
            _ => None,
        })
        .collect();

    let time_conditions: Vec<_> = parsed_conditions
        .iter()
        .filter(|c| {
            matches!(
                c,
                Condition::CreatedAtAfter(_) | Condition::CreatedAtBefore(_)
            )
        })
        .collect();

    // Check kind conditions (OR - at least one must match if any exist)
    if !kind_conditions.is_empty() && !kind_conditions.contains(&event_kind) {
        return Err(Nip26Error::ConditionNotSatisfied(format!(
            "event kind {} does not match any of the allowed kinds: {:?}",
            event_kind, kind_conditions
        )));
    }

    // Check time conditions (AND - all must be satisfied)
    for condition in &time_conditions {
        if !condition.is_satisfied(event_kind, event_created_at) {
            return Err(Nip26Error::ConditionNotSatisfied(format!(
                "condition '{}' not satisfied for kind={}, created_at={}",
                condition, event_kind, event_created_at
            )));
        }
    }

    Ok(())
}

/// Validate a complete delegation
///
/// Verifies both the delegation token signature and that event satisfies conditions
///
/// # Arguments
/// * `delegator_pubkey` - Delegator's public key (hex string)
/// * `delegatee_pubkey` - Delegatee's public key (hex string)
/// * `conditions` - Condition query string
/// * `delegation_token` - 64-byte signature (hex string)
/// * `event_kind` - Event kind to validate
/// * `event_created_at` - Event created_at to validate
pub fn validate_delegation(
    delegator_pubkey: &str,
    delegatee_pubkey: &str,
    conditions: &str,
    delegation_token: &str,
    event_kind: u16,
    event_created_at: u64,
) -> Result<(), Nip26Error> {
    // Verify the delegation token
    verify_delegation_token(
        delegator_pubkey,
        delegatee_pubkey,
        conditions,
        delegation_token,
    )?;

    // Check if event satisfies conditions
    check_delegation_conditions(conditions, event_kind, event_created_at)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};

    #[test]
    fn test_parse_condition_kind() {
        let cond = Condition::parse("kind=1").unwrap();
        assert_eq!(cond, Condition::Kind(1));
        assert_eq!(cond.to_string(), "kind=1");
    }

    #[test]
    fn test_parse_condition_created_at_after() {
        let cond = Condition::parse("created_at>1674834236").unwrap();
        assert_eq!(cond, Condition::CreatedAtAfter(1674834236));
        assert_eq!(cond.to_string(), "created_at>1674834236");
    }

    #[test]
    fn test_parse_condition_created_at_before() {
        let cond = Condition::parse("created_at<1677426236").unwrap();
        assert_eq!(cond, Condition::CreatedAtBefore(1677426236));
        assert_eq!(cond.to_string(), "created_at<1677426236");
    }

    #[test]
    fn test_parse_conditions() {
        let conditions =
            parse_conditions("kind=1&created_at>1674834236&created_at<1677426236").unwrap();
        assert_eq!(conditions.len(), 3);
        assert_eq!(conditions[0], Condition::Kind(1));
        assert_eq!(conditions[1], Condition::CreatedAtAfter(1674834236));
        assert_eq!(conditions[2], Condition::CreatedAtBefore(1677426236));
    }

    #[test]
    fn test_conditions_to_string() {
        let conditions = vec![
            Condition::Kind(1),
            Condition::CreatedAtAfter(1674834236),
            Condition::CreatedAtBefore(1677426236),
        ];
        let query = conditions_to_string(&conditions);
        assert_eq!(query, "kind=1&created_at>1674834236&created_at<1677426236");
    }

    #[test]
    fn test_condition_is_satisfied() {
        let cond1 = Condition::Kind(1);
        assert!(cond1.is_satisfied(1, 1000));
        assert!(!cond1.is_satisfied(2, 1000));

        let cond2 = Condition::CreatedAtAfter(1000);
        assert!(cond2.is_satisfied(1, 1001));
        assert!(!cond2.is_satisfied(1, 999));

        let cond3 = Condition::CreatedAtBefore(2000);
        assert!(cond3.is_satisfied(1, 1999));
        assert!(!cond3.is_satisfied(1, 2001));
    }

    #[test]
    fn test_create_delegation_string() {
        let delegatee = "477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396";
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";
        let result = create_delegation_string(delegatee, conditions);
        assert_eq!(
            result,
            "nostr:delegation:477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396:kind=1&created_at>1674834236&created_at<1677426236"
        );
    }

    #[test]
    fn test_create_and_verify_delegation_token() {
        let secp = Secp256k1::new();

        // Delegator keys
        let delegator_sk = SecretKey::from_slice(&[0xee; 32]).unwrap();
        let delegator_pk = bitcoin::secp256k1::XOnlyPublicKey::from_keypair(
            &bitcoin::secp256k1::Keypair::from_secret_key(&secp, &delegator_sk),
        )
        .0;

        // Delegatee keys
        let delegatee_sk = SecretKey::from_slice(&[0x77; 32]).unwrap();
        let delegatee_pk = bitcoin::secp256k1::XOnlyPublicKey::from_keypair(
            &bitcoin::secp256k1::Keypair::from_secret_key(&secp, &delegatee_sk),
        )
        .0;

        let delegator_pubkey = hex::encode(delegator_pk.serialize());
        let delegatee_pubkey = hex::encode(delegatee_pk.serialize());
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";

        // Create delegation token
        let token =
            create_delegation_token(&delegator_sk.secret_bytes(), &delegatee_pubkey, conditions)
                .unwrap();

        // Verify delegation token
        verify_delegation_token(&delegator_pubkey, &delegatee_pubkey, conditions, &token).unwrap();
    }

    #[test]
    fn test_verify_delegation_token_invalid_signature() {
        let delegator = "8e0d3d3eb2881ec137a11debe736a9086715a8c8beeeda615780064d68bc25dd";
        let delegatee = "477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396";
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";
        let bad_token = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

        let result = verify_delegation_token(delegator, delegatee, conditions, bad_token);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_delegation_conditions_satisfied() {
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";
        let result = check_delegation_conditions(conditions, 1, 1675000000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_delegation_conditions_not_satisfied_kind() {
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";
        let result = check_delegation_conditions(conditions, 2, 1675000000);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_delegation_conditions_not_satisfied_time() {
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";
        let result = check_delegation_conditions(conditions, 1, 1674000000);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_delegation_complete() {
        let secp = Secp256k1::new();

        let delegator_sk = SecretKey::from_slice(&[0xee; 32]).unwrap();
        let delegator_pk = bitcoin::secp256k1::XOnlyPublicKey::from_keypair(
            &bitcoin::secp256k1::Keypair::from_secret_key(&secp, &delegator_sk),
        )
        .0;

        let delegatee_sk = SecretKey::from_slice(&[0x77; 32]).unwrap();
        let delegatee_pk = bitcoin::secp256k1::XOnlyPublicKey::from_keypair(
            &bitcoin::secp256k1::Keypair::from_secret_key(&secp, &delegatee_sk),
        )
        .0;

        let delegator_pubkey = hex::encode(delegator_pk.serialize());
        let delegatee_pubkey = hex::encode(delegatee_pk.serialize());
        let conditions = "kind=1&created_at>1674834236&created_at<1677426236";

        let token =
            create_delegation_token(&delegator_sk.secret_bytes(), &delegatee_pubkey, conditions)
                .unwrap();

        // Valid event
        let result = validate_delegation(
            &delegator_pubkey,
            &delegatee_pubkey,
            conditions,
            &token,
            1,
            1675000000,
        );
        assert!(result.is_ok());

        // Invalid event (wrong kind)
        let result = validate_delegation(
            &delegator_pubkey,
            &delegatee_pubkey,
            conditions,
            &token,
            2,
            1675000000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_kind_conditions() {
        let conditions = "kind=0&kind=1&created_at>1674834236";
        let parsed = parse_conditions(conditions).unwrap();
        assert_eq!(parsed.len(), 3);

        // Should satisfy if kind is 0
        assert!(check_delegation_conditions(conditions, 0, 1675000000).is_ok());
        // Should satisfy if kind is 1
        assert!(check_delegation_conditions(conditions, 1, 1675000000).is_ok());
        // Should not satisfy if kind is 2
        assert!(check_delegation_conditions(conditions, 2, 1675000000).is_err());
    }
}
