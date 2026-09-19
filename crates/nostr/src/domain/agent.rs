use secp256k1::{Secp256k1, XOnlyPublicKey, schnorr::Signature};
use sha2::{Digest, Sha256};

use super::{Event, Tag, hex::decode_lower_hex};

pub const AGENT_OBSERVER_KIND: u16 = 24_200;
pub const AGENT_TURN_METRIC_KIND: u16 = 44_200;

const OWNER_ATTESTATION_DOMAIN: &str = "nostr:agent-auth:";
const NIP44_MIN_CONTENT_LEN: usize = 132;
const NIP44_MAX_CONTENT_LEN: usize = 87_472;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerAttestation {
    pub owner_pubkey: String,
    pub conditions: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentObserverDirection {
    Telemetry,
    Control,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentObserverRoute {
    pub agent_pubkey: String,
    pub owner_pubkey: String,
    pub direction: AgentObserverDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Condition {
    Kind(u16),
    CreatedBefore(u64),
    CreatedAfter(u64),
}

/// Verify the sole NIP-OA owner attestation on an ordinary event.
///
/// The event remains authored only by `event.pubkey`. This helper validates
/// the event itself before treating the tag as provenance and evaluates every
/// condition against that event.
pub fn verify_owner_attestation(event: &Event) -> Result<Option<OwnerAttestation>, String> {
    event
        .validate_structure()
        .map_err(|error| format!("invalid event carrying owner attestation: {error}"))?;
    event
        .validate_crypto()
        .map_err(|error| format!("invalid event carrying owner attestation: {error}"))?;
    verify_attestation(event, true)
}

/// Verify the NIP-OA credential carried by a NIP-AA authentication event.
///
/// NIP-AA evaluates timestamp conditions at connection admission but treats
/// `kind=` clauses as owner intent rather than an admission restriction.
pub fn verify_agent_auth_attestation(event: &Event) -> Result<Option<OwnerAttestation>, String> {
    event
        .validate_structure()
        .map_err(|error| format!("invalid authentication event: {error}"))?;
    event
        .validate_crypto()
        .map_err(|error| format!("invalid authentication event: {error}"))?;
    verify_attestation(event, false)
}

/// Verify a NIP-OA tag as an owner binding for an explicitly named agent.
///
/// NIP-IA owner requests are authored by the owner rather than by the agent,
/// so their signing preimage names `agent_pubkey` and self-attestation is not
/// an error. Timestamp clauses are evaluated against the request while
/// `kind=` remains identity intent, matching NIP-IA's request-borne path.
pub fn verify_owner_binding(
    event: &Event,
    agent_pubkey: &str,
) -> Result<Option<OwnerAttestation>, String> {
    event
        .validate_structure()
        .map_err(|error| format!("invalid owner request: {error}"))?;
    event
        .validate_crypto()
        .map_err(|error| format!("invalid owner request: {error}"))?;
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("auth"))
        .collect::<Vec<_>>();
    let Some(tag) = tags.first() else {
        return Ok(None);
    };
    if tags.len() != 1 || tag.as_slice().len() != 4 {
        return Err("owner request must contain exactly one four-element auth tag".to_owned());
    }
    let owner_pubkey = &tag.as_slice()[1];
    if owner_pubkey != &event.pubkey {
        return Err("owner request auth pubkey must equal the request author".to_owned());
    }
    let owner_bytes = decode_lower_hex::<32>(owner_pubkey, "owner pubkey")
        .map_err(|_| "owner request pubkey must be 64 lowercase hex characters".to_owned())?;
    let owner_key = XOnlyPublicKey::from_byte_array(owner_bytes)
        .map_err(|_| "owner request pubkey is not a valid BIP-340 key".to_owned())?;
    decode_lower_hex::<32>(agent_pubkey, "agent pubkey")
        .map_err(|_| "owner request target must be a lowercase hex pubkey".to_owned())?;
    let conditions = &tag.as_slice()[2];
    let signature = Signature::from_byte_array(
        decode_lower_hex::<64>(&tag.as_slice()[3], "owner signature").map_err(|_| {
            "owner request signature must be 128 lowercase hex characters".to_owned()
        })?,
    );
    let parsed_conditions = parse_conditions(conditions)?;
    let digest: [u8; 32] =
        Sha256::digest(format!("{OWNER_ATTESTATION_DOMAIN}{agent_pubkey}:{conditions}").as_bytes())
            .into();
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &digest, &owner_key)
        .map_err(|_| "owner request attestation signature verification failed".to_owned())?;
    for condition in parsed_conditions {
        let satisfied = match condition {
            Condition::Kind(_) => true,
            Condition::CreatedBefore(bound) => event.created_at < bound,
            Condition::CreatedAfter(bound) => event.created_at > bound,
        };
        if !satisfied {
            return Err("owner request attestation time conditions are not satisfied".to_owned());
        }
    }
    Ok(Some(OwnerAttestation {
        owner_pubkey: owner_pubkey.clone(),
        conditions: conditions.clone(),
    }))
}

fn verify_attestation(
    event: &Event,
    evaluate_kind: bool,
) -> Result<Option<OwnerAttestation>, String> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("auth"))
        .collect::<Vec<_>>();
    let Some(tag) = tags.first() else {
        return Ok(None);
    };
    if tags.len() != 1 {
        return Err("owner attestation must contain exactly one auth tag".to_owned());
    }
    if tag.as_slice().len() != 4 {
        return Err("owner attestation auth tag must contain exactly four elements".to_owned());
    }

    let owner_pubkey = &tag.as_slice()[1];
    let conditions = &tag.as_slice()[2];
    let signature = &tag.as_slice()[3];
    let owner_bytes = decode_lower_hex::<32>(owner_pubkey, "owner pubkey")
        .map_err(|_| "owner attestation pubkey must be 64 lowercase hex characters".to_owned())?;
    let owner_key = XOnlyPublicKey::from_byte_array(owner_bytes)
        .map_err(|_| "owner attestation pubkey is not a valid BIP-340 key".to_owned())?;
    if owner_pubkey == &event.pubkey {
        return Err("owner attestation must not be self-signed".to_owned());
    }
    let signature = Signature::from_byte_array(
        decode_lower_hex::<64>(signature, "owner signature").map_err(|_| {
            "owner attestation signature must be 128 lowercase hex characters".to_owned()
        })?,
    );
    let parsed_conditions = parse_conditions(conditions)?;
    let digest: [u8; 32] = Sha256::digest(
        format!("{OWNER_ATTESTATION_DOMAIN}{}:{conditions}", event.pubkey).as_bytes(),
    )
    .into();
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &digest, &owner_key)
        .map_err(|_| "owner attestation signature verification failed".to_owned())?;

    for condition in parsed_conditions {
        let satisfied = match condition {
            Condition::Kind(kind) => !evaluate_kind || event.kind == kind,
            Condition::CreatedBefore(bound) => event.created_at < bound,
            Condition::CreatedAfter(bound) => event.created_at > bound,
        };
        if !satisfied {
            return Err("owner attestation conditions do not authorize this event".to_owned());
        }
    }

    Ok(Some(OwnerAttestation {
        owner_pubkey: owner_pubkey.clone(),
        conditions: conditions.clone(),
    }))
}

fn parse_conditions(input: &str) -> Result<Vec<Condition>, String> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if !input.is_ascii() || input.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err("owner attestation conditions must be ASCII without whitespace".to_owned());
    }

    input
        .split('&')
        .map(|clause| {
            if clause.is_empty() {
                return Err("owner attestation conditions contain an empty clause".to_owned());
            }
            if let Some(value) = clause.strip_prefix("kind=") {
                return parse_canonical_decimal(value, u64::from(u16::MAX), "kind").and_then(
                    |value| {
                        u16::try_from(value)
                            .map(Condition::Kind)
                            .map_err(|_| "owner attestation kind is out of range".to_owned())
                    },
                );
            }
            if let Some(value) = clause.strip_prefix("created_at<") {
                return parse_canonical_decimal(value, u64::from(u32::MAX), "created_at<")
                    .map(Condition::CreatedBefore);
            }
            if let Some(value) = clause.strip_prefix("created_at>") {
                return parse_canonical_decimal(value, u64::from(u32::MAX), "created_at>")
                    .map(Condition::CreatedAfter);
            }
            Err("owner attestation contains an unsupported condition".to_owned())
        })
        .collect()
}

fn parse_canonical_decimal(input: &str, maximum: u64, name: &str) -> Result<u64, String> {
    if input.is_empty()
        || !input.bytes().all(|byte| byte.is_ascii_digit())
        || (input.len() > 1 && input.starts_with('0'))
    {
        return Err(format!(
            "owner attestation {name} must be a canonical decimal"
        ));
    }
    let value = input
        .parse::<u64>()
        .map_err(|_| format!("owner attestation {name} is out of range"))?;
    if value > maximum {
        return Err(format!("owner attestation {name} is out of range"));
    }
    Ok(value)
}

/// Validate and route a NIP-AO observer envelope.
///
/// `Ok(None)` is the forward-compatible silent-drop result for an unknown
/// frame value. Known frames return the exact agent/owner pair that the relay
/// must confirm through authenticated ownership state.
pub fn agent_observer_route(event: &Event) -> Result<Option<AgentObserverRoute>, String> {
    if event.kind != AGENT_OBSERVER_KIND {
        return Err("agent observer event must have kind 24200".to_owned());
    }
    validate_nip44_v2_content(&event.content, "agent observer")?;
    let recipient = single_pubkey_tag(event, "p", "agent observer")?;
    let agent = single_pubkey_tag(event, "agent", "agent observer")?;
    let frame = single_tag_value(event, "frame", "agent observer")?;

    let (owner_pubkey, direction, expected_frame) = if event.pubkey == agent && recipient != agent {
        (recipient, AgentObserverDirection::Telemetry, "telemetry")
    } else if recipient == agent && event.pubkey != agent {
        (
            event.pubkey.clone(),
            AgentObserverDirection::Control,
            "control",
        )
    } else {
        return Err(
            "agent observer frame must be agent-to-owner telemetry or owner-to-agent control"
                .to_owned(),
        );
    };

    if frame != expected_frame {
        return Ok(None);
    }
    Ok(Some(AgentObserverRoute {
        agent_pubkey: agent,
        owner_pubkey,
        direction,
    }))
}

/// Validate the public NIP-AM envelope and return its owner pubkey.
pub fn agent_turn_metric_owner(event: &Event) -> Result<String, String> {
    if event.kind != AGENT_TURN_METRIC_KIND {
        return Err("agent turn metric must have kind 44200".to_owned());
    }
    if event.tags.iter().any(|tag| tag.name() == Some("h")) {
        return Err("agent turn metric must not have an h tag".to_owned());
    }
    let owner = single_pubkey_tag(event, "p", "agent turn metric")?;
    let agent = single_pubkey_tag(event, "agent", "agent turn metric")?;
    if agent != event.pubkey {
        return Err("agent turn metric agent tag must equal event pubkey".to_owned());
    }
    validate_nip44_v2_content(&event.content, "agent turn metric")?;
    Ok(owner)
}

fn single_pubkey_tag(event: &Event, name: &str, subject: &str) -> Result<String, String> {
    let value = single_tag_value(event, name, subject)?;
    let bytes = decode_lower_hex::<32>(value, "agent pubkey")
        .map_err(|_| format!("{subject} {name} tag must be a lowercase hex pubkey"))?;
    XOnlyPublicKey::from_byte_array(bytes)
        .map_err(|_| format!("{subject} {name} tag is not a valid BIP-340 pubkey"))?;
    Ok(value.to_owned())
}

fn single_tag_value<'a>(event: &'a Event, name: &str, subject: &str) -> Result<&'a str, String> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<&Tag>>();
    if tags.len() != 1 {
        return Err(format!("{subject} must contain exactly one {name} tag"));
    }
    tags[0]
        .value()
        .ok_or_else(|| format!("{subject} {name} tag is missing its value"))
}

/// Perform a bounded NIP-44 v2 ciphertext envelope check without decrypting.
pub fn validate_nip44_v2_content(content: &str, subject: &str) -> Result<(), String> {
    let bytes = content.as_bytes();
    if !(NIP44_MIN_CONTENT_LEN..=NIP44_MAX_CONTENT_LEN).contains(&bytes.len())
        || !bytes.len().is_multiple_of(4)
    {
        return Err(format!(
            "{subject} content is not a bounded NIP-44 v2 ciphertext"
        ));
    }

    let mut padding = 0_usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' if padding == 0 => {}
            b'=' if index >= bytes.len().saturating_sub(2) && padding < 2 => padding += 1,
            _ => return Err(format!("{subject} content is not canonical base64")),
        }
    }
    let decoded_len = (bytes.len() / 4) * 3 - padding;
    if decoded_len < 99 {
        return Err(format!("{subject} content is too short for NIP-44 v2"));
    }
    let first = base64_value(bytes[0])
        .zip(base64_value(bytes[1]))
        .map(|(high, low)| (high << 2) | (low >> 4));
    if first != Some(0x02) {
        return Err(format!("{subject} content does not carry NIP-44 version 2"));
    }
    Ok(())
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
