use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
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

/// A NIP-OA credential an owner minted for an agent key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintedOwnerAttestation {
    /// The owner's x-only public key, 64 lowercase hex characters.
    pub owner_pubkey: String,
    /// The exact conditions string the owner signed.
    pub conditions: String,
    /// The BIP-340 signature, 128 lowercase hex characters.
    pub signature: String,
}

impl MintedOwnerAttestation {
    /// Returns the `["auth", owner, conditions, signature]` tag.
    #[must_use]
    pub fn tag(&self) -> Tag {
        Tag::new(vec![
            "auth".to_owned(),
            self.owner_pubkey.clone(),
            self.conditions.clone(),
            self.signature.clone(),
        ])
    }
}

/// Mint the NIP-OA credential by which `owner_secret` authorizes
/// `agent_pubkey` under `conditions`.
///
/// The conditions follow the grammar the verifiers enforce, and the signed
/// message is `SHA256("nostr:agent-auth:" || agent_pubkey || ":" ||
/// conditions)`. The signature uses 32 zero bytes as BIP-340 auxiliary
/// data, so the same inputs always mint the same credential.
///
/// # Errors
///
/// Returns an error when `agent_pubkey` is not a lowercase hex BIP-340 key,
/// when it equals the owner's key, or when `conditions` breaks the grammar.
pub fn mint_owner_attestation(
    owner_secret: &SecretKey,
    agent_pubkey: &str,
    conditions: &str,
) -> Result<MintedOwnerAttestation, String> {
    let agent_bytes = decode_lower_hex::<32>(agent_pubkey, "agent pubkey")
        .map_err(|_| "owner attestation agent must be 64 lowercase hex characters".to_owned())?;
    XOnlyPublicKey::from_byte_array(agent_bytes)
        .map_err(|_| "owner attestation agent is not a valid BIP-340 key".to_owned())?;
    parse_conditions(conditions)?;
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, owner_secret);
    let owner_pubkey = keypair.x_only_public_key().0.to_string();
    if owner_pubkey == agent_pubkey {
        return Err("owner attestation must not be self-signed".to_owned());
    }
    let digest: [u8; 32] =
        Sha256::digest(format!("{OWNER_ATTESTATION_DOMAIN}{agent_pubkey}:{conditions}").as_bytes())
            .into();
    let signature = secp
        .sign_schnorr_with_aux_rand(&digest, &keypair, &[0; 32])
        .to_string();
    Ok(MintedOwnerAttestation {
        owner_pubkey,
        conditions: conditions.to_owned(),
        signature,
    })
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

#[cfg(test)]
mod mint_tests {
    use super::*;
    use crate::domain::hex::encode_lower_hex;

    // TEST KEYS from NIP-OA's vectors. Never use them in production.
    const OWNER_SECRET: [u8; 32] = one(1);
    const AGENT_SECRET: [u8; 32] = one(2);
    const OWNER: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const AGENT: &str = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    const CONDITIONS: &str = "kind=1&created_at<1713957000";
    const AUTH_SIG: &str = "8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369";
    const TAG_BYTES_HEX: &str = "5b2261757468222c2237396265363637656639646362626163353561303632393563653837306230373032396266636462326463653238643935396632383135623136663831373938222c226b696e643d3126637265617465645f61743c31373133393537303030222c223862376466323537356361663061313038333734663834373137323262323333633533663966663832376138623066393138363139363663336239646435636232653138396561653966343964373231383736373463326635626432343431343565313066663836633966323537666665363561316565356631303862333639225d";
    const EVENT_ID: &str = "d892a65e7677e0554ebb70ee16deeb6a0727dba46450fb4bc001291d7bff971b";
    const EVENT_SIG: &str = "7fd38992b70b5e9e113644e51b4c8ee2227f3bdd402b1855f8786c0600394ab3ec2621742a7bad0b0000b93d4d1ae6e39525f286a3c1029f43f46c3359a6c76f";

    const fn one(last: u8) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[31] = last;
        bytes
    }

    fn secret(bytes: [u8; 32]) -> SecretKey {
        SecretKey::from_byte_array(bytes).unwrap()
    }

    fn minted() -> MintedOwnerAttestation {
        mint_owner_attestation(&secret(OWNER_SECRET), AGENT, CONDITIONS).unwrap()
    }

    /// Signs a kind-1 event with `signer` and zero auxiliary bytes.
    fn signed(signer: [u8; 32], tags: Vec<Tag>) -> Event {
        let secp = Secp256k1::signing_only();
        let keypair = Keypair::from_secret_key(&secp, &secret(signer));
        let mut event = Event {
            id: String::new(),
            pubkey: keypair.x_only_public_key().0.to_string(),
            created_at: 1_713_956_400,
            kind: 1,
            tags,
            content: "owner-attested agent event".to_owned(),
            sig: String::new(),
        };
        let id = event.computed_id_bytes().unwrap();
        event.id = encode_lower_hex(&id);
        event.sig = secp
            .sign_schnorr_with_aux_rand(&id, &keypair, &[0; 32])
            .to_string();
        event
    }

    fn with_tag(tag: Vec<&str>) -> Event {
        signed(
            AGENT_SECRET,
            vec![Tag::new(tag.into_iter().map(str::to_owned).collect())],
        )
    }

    /// NIP-OA's vector signatures used random auxiliary bytes, so a
    /// deterministic minter cannot match their bytes. The test reproduces
    /// everything else and checks that both signatures verify over the same
    /// digest.
    #[test]
    fn nip_oa_vector_reproduces() {
        let digest: [u8; 32] =
            Sha256::digest(format!("nostr:agent-auth:{AGENT}:{CONDITIONS}").as_bytes()).into();
        assert_eq!(
            encode_lower_hex(&digest),
            "08cdecd55af4c28d3801fd69615dcf5cc04fab3bc134b38a840bf157197069a6"
        );
        let owner_key =
            XOnlyPublicKey::from_byte_array(decode_lower_hex(OWNER, "o").unwrap()).unwrap();
        let minted = minted();
        assert_eq!(minted.owner_pubkey, OWNER);
        assert_eq!(minted.conditions, CONDITIONS);
        for signature in [AUTH_SIG, minted.signature.as_str()] {
            let signature = Signature::from_byte_array(decode_lower_hex(signature, "s").unwrap());
            Secp256k1::verification_only()
                .verify_schnorr(&signature, &digest, &owner_key)
                .unwrap();
        }

        let spec = MintedOwnerAttestation {
            signature: AUTH_SIG.to_owned(),
            ..minted.clone()
        };
        let tag_bytes = serde_json::to_vec(&spec.tag()).unwrap();
        assert_eq!(encode_lower_hex(&tag_bytes), TAG_BYTES_HEX);

        let mut event = signed(AGENT_SECRET, vec![spec.tag()]);
        assert_eq!(event.pubkey, AGENT);
        assert_eq!(event.id, EVENT_ID);
        event.sig = EVENT_SIG.to_owned();
        let expected = Some(OwnerAttestation {
            owner_pubkey: OWNER.to_owned(),
            conditions: CONDITIONS.to_owned(),
        });
        assert_eq!(verify_owner_attestation(&event).unwrap(), expected);
        let ours = signed(AGENT_SECRET, vec![minted.tag()]);
        assert_eq!(verify_owner_attestation(&ours).unwrap(), expected);
    }

    #[test]
    fn minting_matches_the_no_aux_signer_coder_used() {
        let digest: [u8; 32] =
            Sha256::digest(format!("nostr:agent-auth:{AGENT}:{CONDITIONS}").as_bytes()).into();
        let secp = Secp256k1::signing_only();
        let keypair = Keypair::from_secret_key(&secp, &secret(OWNER_SECRET));
        assert_eq!(
            secp.sign_schnorr_no_aux_rand(&digest, &keypair).to_string(),
            minted().signature
        );
    }

    #[test]
    fn minting_refuses_what_verifiers_reject() {
        let owner = secret(OWNER_SECRET);
        for conditions in [
            "kind=1&",
            "&kind=1",
            "kind=1&&kind=2",
            "kind=01",
            "kind=65536",
            "kind =1",
            "size=1",
        ] {
            assert!(
                mint_owner_attestation(&owner, AGENT, conditions).is_err(),
                "{conditions}"
            );
        }
        assert!(mint_owner_attestation(&owner, OWNER, CONDITIONS).is_err());
        assert!(mint_owner_attestation(&owner, &AGENT.to_uppercase(), CONDITIONS).is_err());
        assert!(mint_owner_attestation(&owner, "00", CONDITIONS).is_err());
        let empty = mint_owner_attestation(&owner, AGENT, "").unwrap();
        assert!(verify_owner_attestation(&signed(AGENT_SECRET, vec![empty.tag()])).is_ok());
    }

    #[test]
    fn nip_oa_invalid_vectors_are_rejected() {
        let tag = minted().tag();
        let two = signed(AGENT_SECRET, vec![tag.clone(), tag.clone()]);
        assert!(verify_owner_attestation(&two).is_err());

        assert!(verify_owner_attestation(&with_tag(vec!["auth", OWNER, CONDITIONS])).is_err());
        assert!(
            verify_owner_attestation(&with_tag(vec!["auth", OWNER, CONDITIONS, AUTH_SIG, "x"]))
                .is_err()
        );

        assert!(
            verify_owner_attestation(&with_tag(vec!["auth", OWNER, "kind=1&", AUTH_SIG])).is_err()
        );
        assert!(
            verify_owner_attestation(&with_tag(vec!["auth", OWNER, "kind=01", AUTH_SIG])).is_err()
        );

        // Self-attestation: the auth tag names the event's own author.
        let mut self_tag = tag.clone();
        self_tag.0[1] = AGENT.to_owned();
        assert!(verify_owner_attestation(&signed(AGENT_SECRET, vec![self_tag])).is_err());

        let mut bad_sig = signed(AGENT_SECRET, vec![tag.clone()]);
        bad_sig.sig = format!("{}00", &bad_sig.sig[..126]);
        assert!(verify_owner_attestation(&bad_sig).is_err());
        let mut bad_id = signed(AGENT_SECRET, vec![tag]);
        bad_id.content.push('!');
        assert!(verify_owner_attestation(&bad_id).is_err());
    }
}
