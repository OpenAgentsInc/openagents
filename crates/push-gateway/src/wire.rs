//! Closed request and response bodies.
//!
//! Every request is a closed JSON object: unknown members, duplicate
//! members, missing members, wrong types, trailing data, and any `v` other
//! than `1` are `400 {"error":"invalid_request"}`. The delivery request is
//! exactly the four members the relay sends. Registration requests carry no
//! owner field: the NIP-98 signer is the owner.

use serde::{Deserialize, Serialize};

/// The only wire version.
pub const WIRE_VERSION: u64 = 1;
/// Largest accepted request body, in bytes.
pub const MAX_BODY_BYTES: usize = 8_192;
/// Largest accepted `endpoint_grant`, in bytes.
pub const MAX_GRANT_BYTES: usize = 4_096;
/// Largest APNs device token, in bytes before hex encoding.
pub const MAX_APNS_TOKEN_BYTES: usize = 512;
/// Largest FCM registration token, in bytes.
pub const MAX_FCM_TOKEN_BYTES: usize = 4_096;

/// Registration routes. Serve these on the registration listener.
pub const INSTALLATIONS_PATH: &str = "/v1/installations";
/// Rotate the native token of an installation.
pub const ROTATE_PATH: &str = "/v1/installations/endpoint";
/// Revoke an installation and every delegation.
pub const INSTALLATION_REVOKE_PATH: &str = "/v1/installations/revoke";
/// Issue a delivery capability for one relay.
pub const DELEGATIONS_PATH: &str = "/v1/delegations";
/// Revoke the delivery capability for one relay.
pub const DELEGATION_REVOKE_PATH: &str = "/v1/delegations/revoke";
/// Delivery routes. Serve these on the private delivery listener only.
pub const APNS_DELIVERY_PATH: &str = "/v1/deliveries/apns";
/// FCM delivery route.
pub const FCM_DELIVERY_PATH: &str = "/v1/deliveries/fcm";

/// A platform transport with a registered wake constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// Apple Push Notification service.
    Apns,
    /// Firebase Cloud Messaging HTTP v1.
    Fcm,
}

impl Transport {
    /// The lease `transport` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Apns => "apns",
            Self::Fcm => "fcm",
        }
    }

    /// Parse a lease `transport` value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "apns" => Some(Self::Apns),
            "fcm" => Some(Self::Fcm),
            _ => None,
        }
    }

    /// Check a native token's shape for this transport. APNs tokens are
    /// even-length lowercase hexadecimal of at most 512 bytes; FCM tokens are
    /// printable ASCII without whitespace, at most 4,096 bytes.
    #[must_use]
    pub fn valid_token(self, token: &str) -> bool {
        match self {
            Self::Apns => {
                !token.is_empty()
                    && token.len().is_multiple_of(2)
                    && token.len() <= MAX_APNS_TOKEN_BYTES * 2
                    && token
                        .bytes()
                        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            }
            Self::Fcm => {
                !token.is_empty()
                    && token.len() <= MAX_FCM_TOKEN_BYTES
                    && token.bytes().all(|b| b.is_ascii_graphic())
            }
        }
    }
}

/// `POST /v1/installations`: hold a native token for the signer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationRequest {
    /// Always `1`.
    pub v: u64,
    /// A profile the gateway serves; it selects the transport.
    pub app_profile: String,
    /// The native token: APNs device token hex or FCM registration token.
    pub endpoint: String,
    /// Unix seconds when the gateway may forget the installation.
    pub expires_at: u64,
}

/// `201` answer to an installation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationResponse {
    /// Canonical lowercase UUID naming the installation.
    pub installation_handle: String,
    /// Current endpoint epoch; rotation increments it.
    pub endpoint_epoch: u64,
    /// When the installation lapses unless a delegation extends it.
    pub expires_at: u64,
}

/// `POST /v1/installations/endpoint`: replace the native token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotateRequest {
    /// Always `1`.
    pub v: u64,
    /// The installation.
    pub installation_handle: String,
    /// The current epoch.
    pub endpoint_epoch: u64,
    /// Exactly `endpoint_epoch + 1`.
    pub new_endpoint_epoch: u64,
    /// The new native token.
    pub endpoint: String,
}

/// `POST /v1/installations/revoke`: forget the token and every grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationRevokeRequest {
    /// Always `1`.
    pub v: u64,
    /// The installation.
    pub installation_handle: String,
    /// The current epoch.
    pub endpoint_epoch: u64,
}

/// `POST /v1/delegations`: issue a delivery capability for one relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationRequest {
    /// Always `1`.
    pub v: u64,
    /// The installation.
    pub installation_handle: String,
    /// Must equal the installation's current epoch.
    pub endpoint_epoch: u64,
    /// Strictly increasing per installation and relay. Clients use the
    /// lease generation.
    pub generation: u64,
    /// The relay signing key that will post deliveries; one the gateway
    /// is configured to accept.
    pub relay_pubkey: String,
    /// Unix seconds before which the capability is not valid.
    pub not_before: u64,
    /// Unix seconds after which the capability is not valid.
    pub expires_at: u64,
}

/// `201` answer to a delegation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationResponse {
    /// The opaque capability the lease carries as its `endpoint`.
    pub endpoint_grant: String,
}

/// `POST /v1/delegations/revoke`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationRevokeRequest {
    /// Always `1`.
    pub v: u64,
    /// The installation.
    pub installation_handle: String,
    /// The relay whose capability ends.
    pub relay_pubkey: String,
    /// The current delegation generation.
    pub generation: u64,
}

/// The relay-delivery request, exactly as the relay's adapters send it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRequest {
    /// Always `1`.
    pub v: u64,
    /// The lease endpoint: a capability this gateway issued.
    pub endpoint_grant: String,
    /// The relay's durable job UUID. Stable across retries.
    pub request_id: String,
    /// Unix seconds after which the wake is useless.
    pub expires_at: u64,
}

/// Parse a closed request body.
///
/// # Errors
///
/// Returns `invalid_request` for anything but one closed object of `T` with
/// `v` equal to `1`.
pub fn parse_closed<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, &'static str> {
    if body.len() > MAX_BODY_BYTES {
        return Err("invalid_request");
    }
    let value: T = serde_json::from_slice(body).map_err(|_| "invalid_request")?;
    let version: serde_json::Value = serde_json::from_slice(body).map_err(|_| "invalid_request")?;
    if version.get("v").and_then(serde_json::Value::as_u64) != Some(WIRE_VERSION) {
        return Err("invalid_request");
    }
    Ok(value)
}

/// A canonical lowercase hyphenated UUID.
#[must_use]
pub fn valid_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => matches!(byte, b'0'..=b'9' | b'a'..=b'f'),
        })
}

/// A 64-character lowercase hexadecimal public key.
#[must_use]
pub fn valid_pubkey(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A random version 4 UUID.
#[must_use]
pub fn random_uuid() -> String {
    use secp256k1::rand::RngCore as _;
    let mut bytes = [0_u8; 16];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = crate::hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_bodies_refuse_extra_duplicate_and_versioned_members() {
        let good = br#"{"v":1,"endpoint_grant":"g","request_id":"0f0e0d0c-0b0a-4908-8706-050403020100","expires_at":5}"#;
        assert!(parse_closed::<DeliveryRequest>(good).is_ok());
        for bad in [
            &br#"{"v":1,"endpoint_grant":"g","request_id":"r","expires_at":5,"title":"x"}"#[..],
            br#"{"v":1,"v":1,"endpoint_grant":"g","request_id":"r","expires_at":5}"#,
            br#"{"v":2,"endpoint_grant":"g","request_id":"r","expires_at":5}"#,
            br#"{"v":1.0,"endpoint_grant":"g","request_id":"r","expires_at":5}"#,
            br#"{"v":1,"endpoint_grant":"g","request_id":"r"}"#,
            br#"{"v":1,"endpoint_grant":"g","request_id":"r","expires_at":5} x"#,
            br#"{"v":1,"endpoint_grant":"g","request_id":"r","expires_at":-5}"#,
        ] {
            assert_eq!(
                parse_closed::<DeliveryRequest>(bad),
                Err("invalid_request"),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        assert!(parse_closed::<DeliveryRequest>(&vec![b' '; MAX_BODY_BYTES + 1]).is_err());
    }

    #[test]
    fn identifiers_and_tokens_have_exact_shapes() {
        let uuid = random_uuid();
        assert!(valid_uuid(&uuid), "{uuid}");
        assert_eq!(&uuid[14..15], "4");
        assert!(!valid_uuid(&uuid.to_uppercase()));
        assert!(!valid_uuid("0f0e0d0c0b0a490887060504030201000000"));
        assert!(Transport::Apns.valid_token("abcd01"));
        assert!(!Transport::Apns.valid_token("ABCD01"));
        assert!(!Transport::Apns.valid_token("abc"));
        assert!(!Transport::Apns.valid_token(&"ab".repeat(513)));
        assert!(Transport::Fcm.valid_token("fcm:token-1_A"));
        assert!(!Transport::Fcm.valid_token("has space"));
        assert!(!Transport::Fcm.valid_token(""));
    }
}
