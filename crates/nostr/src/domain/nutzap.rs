//! NIP-61 nutzaps.
//!
//! Kind `10019` is a replaceable notice of where to send ecash. `relay`
//! tags name the relays that should receive nutzaps. `mint` tags name
//! the mints the user will accept, with optional unit markers. `pubkey`
//! is the P2PK key from the NIP-60 wallet. It is not the user's Nostr
//! key. Kind `9321` carries one or more P2PK proofs for one of those
//! mints.
//!
//! The relay does not talk to a mint, verify a DLEQ proof, or swap a
//! token. The pinned NIP-65 text does not define URL normalization, so
//! a mint URL matches only as written. NIP-61 is a draft, so these
//! kinds stay off the NIP-11 list.

use std::collections::BTreeMap;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, Filter};

const INFO_KIND: u16 = 10_019;
const NUTZAP_KIND: u16 = 9_321;

/// A mint a recipient accepts, with the units it listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NutzapMint {
    pub url: String,
    pub units: Vec<String>,
}

/// A kind `10019` receiving policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NutzapInfo {
    pub owner: String,
    pub relays: Vec<String>,
    pub mints: Vec<NutzapMint>,
    /// 32-byte x-only key. The P2PK lock prefixes it with `02`.
    pub p2pk: String,
}

/// One proof inside a kind `9321` nutzap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NutzapProof {
    pub id: String,
    pub amount: u64,
    pub commitment: String,
    pub lock: String,
}

/// A kind `9321` nutzap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nutzap {
    pub sender: String,
    pub recipient: String,
    pub mint: String,
    pub unit: String,
    pub comment: String,
    pub event_id: Option<String>,
    pub event_kind: Option<u16>,
    pub proofs: Vec<NutzapProof>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn relay_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("wss://")
        .or_else(|| value.strip_prefix("ws://"))
    else {
        return false;
    };
    !rest.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn unit_name(value: &str) -> Result<String, DomainError> {
    if !value.is_empty() && value.len() <= 8 && value.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        Ok(value.to_owned())
    } else {
        Err(invalid("a nutzap unit is a short lowercase name"))
    }
}

fn hex32(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "nutzap event")
        .map_err(|_| invalid("a nutzap event id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn receiving_key(value: &str, nostr: &str) -> Result<String, DomainError> {
    let reason = "a nutzap receiving key is 32 bytes, or 33 bytes starting with 02, and it is not the nostr key";
    let p2pk = if value.len() == 64 {
        decode_lower_hex::<32>(value, "nutzap receiving key").map_err(|_| invalid(reason))?;
        value.to_owned()
    } else if value.len() == 66 && value.starts_with("02") {
        decode_lower_hex::<33>(value, "nutzap receiving key").map_err(|_| invalid(reason))?;
        value[2..].to_owned()
    } else {
        return Err(invalid(reason));
    };
    if p2pk == nostr {
        return Err(invalid(reason));
    }
    Ok(p2pk)
}

fn lock_key(secret: &str) -> Result<String, DomainError> {
    let value: serde_json::Value =
        serde_json::from_str(secret).map_err(|_| invalid("a nutzap proof is P2PK-locked"))?;
    let Some(fields) = value.as_array() else {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    };
    if fields.len() != 2 || fields[0].as_str() != Some("P2PK") {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    }
    let Some(body) = fields[1].as_object() else {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    };
    if body.len() != 2 {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    }
    let Some(nonce) = body.get("nonce").and_then(serde_json::Value::as_str) else {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    };
    decode_lower_hex::<32>(nonce, "p2pk nonce")
        .map_err(|_| invalid("a nutzap proof is P2PK-locked"))?;
    let Some(data) = body.get("data").and_then(serde_json::Value::as_str) else {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    };
    let bytes = decode_lower_hex::<33>(data, "p2pk data")
        .map_err(|_| invalid("a nutzap proof is P2PK-locked"))?;
    if bytes[0] != 0x02 {
        return Err(invalid("a nutzap lock key starts with 02"));
    }
    Ok(data.to_owned())
}

fn proof_json(value: &str) -> Result<NutzapProof, DomainError> {
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|_| invalid("a nutzap proof is JSON"))?;
    let Some(fields) = parsed.as_object() else {
        return Err(invalid("a nutzap proof is JSON"));
    };
    for key in fields.keys() {
        if !matches!(key.as_str(), "id" | "amount" | "secret" | "C" | "dleq") {
            return Err(invalid(
                "a nutzap proof has an amount, an id, a secret, and a commitment",
            ));
        }
    }
    let Some(id) = fields.get("id").and_then(serde_json::Value::as_str) else {
        return Err(invalid("a nutzap proof has an id"));
    };
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_whitespace) {
        return Err(invalid("a nutzap proof has an id"));
    }
    let Some(amount) = fields.get("amount").and_then(serde_json::Value::as_u64) else {
        return Err(invalid("a nutzap proof amount is a positive integer"));
    };
    if amount == 0 {
        return Err(invalid("a nutzap proof amount is a positive integer"));
    }
    let Some(secret) = fields.get("secret").and_then(serde_json::Value::as_str) else {
        return Err(invalid("a nutzap proof is P2PK-locked"));
    };
    let Some(commitment) = fields.get("C").and_then(serde_json::Value::as_str) else {
        return Err(invalid(
            "a nutzap proof commitment is 33 lowercase hex bytes",
        ));
    };
    decode_lower_hex::<33>(commitment, "proof commitment")
        .map_err(|_| invalid("a nutzap proof commitment is 33 lowercase hex bytes"))?;
    if let Some(dleq) = fields.get("dleq") {
        let Some(dleq) = dleq.as_object() else {
            return Err(invalid("a nutzap DLEQ field names e, s, and r"));
        };
        for name in ["e", "s", "r"] {
            let Some(part) = dleq.get(name).and_then(serde_json::Value::as_str) else {
                return Err(invalid("a nutzap DLEQ field names e, s, and r"));
            };
            if part.is_empty()
                || part.len() > 128
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(invalid("a nutzap DLEQ field names e, s, and r"));
            }
        }
    }
    Ok(NutzapProof {
        id: id.to_owned(),
        amount,
        commitment: commitment.to_owned(),
        lock: lock_key(secret)?,
    })
}

/// Read a kind `10019` receiving policy.
pub fn open_nutzap_info(event: &Event) -> Result<NutzapInfo, DomainError> {
    if event.kind != INFO_KIND {
        return Err(invalid("a nutzap policy has kind 10019"));
    }
    let mut relays = Vec::new();
    let mut mints: Vec<NutzapMint> = Vec::new();
    let mut p2pk = None;
    for tag in &event.tags {
        match tag.name() {
            Some("relay") => {
                if tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzap relay is ws:// or wss://"));
                }
                let Some(value) = tag.value().filter(|value| relay_url(value)) else {
                    return Err(invalid("a nutzap relay is ws:// or wss://"));
                };
                if relays.iter().any(|seen| seen == value) {
                    return Err(invalid("a nutzap relay is listed once"));
                }
                relays.push(value.to_owned());
            }
            Some("mint") => {
                let parts = tag.as_slice();
                if parts.len() < 2 || !http_url(&parts[1]) {
                    return Err(invalid("a nutzap mint is an http:// or https:// URL"));
                }
                if mints.iter().any(|mint| mint.url == parts[1]) {
                    return Err(invalid("a nutzap mint is listed once"));
                }
                let mut units = Vec::new();
                for unit in parts.iter().skip(2) {
                    let unit = unit_name(unit)?;
                    if units.iter().any(|seen| seen == &unit) {
                        return Err(invalid("a nutzap unit is listed once"));
                    }
                    units.push(unit);
                }
                mints.push(NutzapMint {
                    url: parts[1].clone(),
                    units,
                });
            }
            Some("pubkey") => {
                if p2pk.is_some() || tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzap policy has one receiving key"));
                }
                let Some(value) = tag.value() else {
                    return Err(invalid("a nutzap policy has one receiving key"));
                };
                p2pk = Some(receiving_key(value, &event.pubkey)?);
            }
            _ => {}
        }
    }
    if relays.is_empty() {
        return Err(invalid("a nutzap policy lists a relay"));
    }
    if mints.is_empty() {
        return Err(invalid("a nutzap policy lists a mint"));
    }
    let Some(p2pk) = p2pk else {
        return Err(invalid("a nutzap policy has one receiving key"));
    };
    Ok(NutzapInfo {
        owner: event.pubkey.clone(),
        relays,
        mints,
        p2pk,
    })
}

/// Read a kind `9321` nutzap.
pub fn open_nutzap(event: &Event) -> Result<Nutzap, DomainError> {
    if event.kind != NUTZAP_KIND {
        return Err(invalid("a nutzap has kind 9321"));
    }
    if event.content.len() > 2_048 || event.content.chars().any(char::is_control) {
        return Err(invalid("a nutzap comment is plain text"));
    }
    let mut proofs = Vec::new();
    let mut unit = None;
    let mut mint = None;
    let mut recipient = None;
    let mut event_id = None;
    let mut event_kind = None;
    for tag in &event.tags {
        match tag.name() {
            Some("proof") => {
                if tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzap proof is JSON"));
                }
                let Some(value) = tag.value() else {
                    return Err(invalid("a nutzap proof is JSON"));
                };
                let proof = proof_json(value)?;
                if proofs.iter().any(|seen: &NutzapProof| seen.id == proof.id) {
                    return Err(invalid("a nutzap proof is listed once"));
                }
                proofs.push(proof);
            }
            Some("unit") => {
                if unit.is_some() || tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzap has one unit"));
                }
                let Some(value) = tag.value() else {
                    return Err(invalid("a nutzap has one unit"));
                };
                unit = Some(unit_name(value)?);
            }
            Some("u") => {
                if mint.is_some() || tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzap names one mint"));
                }
                let Some(value) = tag.value().filter(|value| http_url(value)) else {
                    return Err(invalid("a nutzap names one mint"));
                };
                mint = Some(value.to_owned());
            }
            Some("p") => {
                if recipient.is_some() || tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzap names one recipient"));
                }
                let Some(value) = tag.value() else {
                    return Err(invalid("a nutzap names one recipient"));
                };
                recipient =
                    Some(hex32(value).map_err(|_| invalid("a nutzap names one recipient"))?);
            }
            Some("e") => {
                if event_id.is_some() || !(2..=3).contains(&tag.as_slice().len()) {
                    return Err(invalid("a nutzap names one event"));
                }
                let parts = tag.as_slice();
                if parts.len() == 3 && !parts[2].is_empty() && !relay_url(&parts[2]) {
                    return Err(invalid("a nutzap event relay is ws:// or wss://"));
                }
                event_id = Some(hex32(&parts[1])?);
            }
            Some("k") => {
                if event_kind.is_some() || tag.as_slice().len() != 2 {
                    return Err(invalid("a nutzapped kind is a number"));
                }
                let Some(value) = tag.value() else {
                    return Err(invalid("a nutzapped kind is a number"));
                };
                if value.is_empty()
                    || (value.len() > 1 && value.starts_with('0'))
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(invalid("a nutzapped kind is a number"));
                }
                event_kind = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| invalid("a nutzapped kind is a number"))?,
                );
            }
            _ => {}
        }
    }
    if proofs.is_empty() {
        return Err(invalid("a nutzap lists one proof"));
    }
    let Some(mint) = mint else {
        return Err(invalid("a nutzap names one mint"));
    };
    let Some(recipient) = recipient else {
        return Err(invalid("a nutzap names one recipient"));
    };
    Ok(Nutzap {
        sender: event.pubkey.clone(),
        recipient,
        mint,
        unit: unit.unwrap_or_else(|| "sat".to_owned()),
        comment: event.content.clone(),
        event_id,
        event_kind,
        proofs,
    })
}

/// Whether `zap` uses a mint and the P2PK key from `info`.
pub fn nutzap_matches(info: &NutzapInfo, zap: &Nutzap) -> Result<(), DomainError> {
    if zap.recipient != info.owner {
        return Err(invalid("a nutzap pays the owner of the receiving policy"));
    }
    let Some(mint) = info.mints.iter().find(|mint| mint.url == zap.mint) else {
        return Err(invalid("a nutzap uses a mint from the receiving policy"));
    };
    if !mint.units.is_empty() && !mint.units.iter().any(|unit| unit == &zap.unit) {
        return Err(invalid("a nutzap uses a unit the mint listed"));
    }
    let lock = format!("02{}", info.p2pk);
    if zap.proofs.iter().any(|proof| proof.lock != lock) {
        return Err(invalid("a nutzap proof is locked to the receiving key"));
    }
    Ok(())
}

/// The inbox query from the pinned receiving example.
pub fn nutzap_inbox(recipient: &str, mints: &[String], since: u64) -> Result<Filter, DomainError> {
    hex32(recipient).map_err(|_| invalid("a nutzap inbox names a recipient"))?;
    if mints.is_empty() || mints.iter().any(|mint| !http_url(mint)) {
        return Err(invalid("a nutzap inbox names the recipient's mints"));
    }
    let mut tags = BTreeMap::new();
    tags.insert("p".to_owned(), vec![recipient.to_owned()]);
    tags.insert("u".to_owned(), mints.to_vec());
    let filter = Filter {
        kinds: Some(vec![NUTZAP_KIND]),
        tags,
        since: Some(since),
        ..Filter::default()
    };
    filter
        .validate()
        .map_err(|_| invalid("a nutzap inbox names a recipient"))?;
    Ok(filter)
}

#[cfg(test)]
mod tests {
    use secp256k1::SecretKey;

    use super::decode_lower_hex;
    use super::*;
    use crate::domain::{
        EventClass, HistoryDirection, RelaySigner, ReplacementDecision, Tag, TokenRole,
        compare_replacement, open_spend_history, read_wallet_content, seal_wallet_content,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn secret(byte: &str) -> SecretKey {
        SecretKey::from_byte_array(decode_lower_hex::<32>(&byte.repeat(32), "key").unwrap())
            .unwrap()
    }

    fn proof(lock: &str) -> String {
        let secret = serde_json::json!([
            "P2PK",
            {
                "nonce": "b0".repeat(32),
                "data": lock,
            }
        ]);
        serde_json::json!({
            "amount": 1,
            "C": format!("02{}", "c0".repeat(32)),
            "id": "000a93d6f8a1d2c4",
            "secret": secret.to_string(),
        })
        .to_string()
    }

    #[test]
    fn a_nutzap_uses_the_recipients_mint_and_lock_key() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/61.md"
        ))
        .unwrap();
        assert!(text.contains("10019"));
        assert!(text.contains("mint"));
        assert!(text.contains("9321"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "61.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "61.md")
        );

        let bob = signer("61");
        let alice = signer("62");
        let wallet = signer("63");
        let relay = "wss://nutzap.example";
        let mint = "https://mint1.example";
        let info = bob.sign(
            1_700_000_000,
            INFO_KIND,
            vec![
                Tag::new(vec!["relay".into(), relay.into()]),
                Tag::new(vec!["mint".into(), mint.into(), "sat".into()]),
                Tag::new(vec![
                    "mint".into(),
                    "https://mint2.example".into(),
                    "usd".into(),
                    "sat".into(),
                ]),
                Tag::new(vec!["pubkey".into(), wallet.pubkey().to_owned()]),
            ],
            String::new(),
        );
        info.validate_structure().unwrap();
        assert_eq!(info.class(), EventClass::Replaceable);
        let opened = open_nutzap_info(&info).unwrap();
        assert_eq!(opened.p2pk, wallet.pubkey());
        assert_eq!(opened.mints.len(), 2);
        assert_ne!(opened.p2pk, bob.pubkey());

        let reused = bob.sign(
            1_700_000_050,
            INFO_KIND,
            vec![
                Tag::new(vec!["relay".into(), relay.into()]),
                Tag::new(vec!["mint".into(), mint.into()]),
                Tag::new(vec!["pubkey".into(), bob.pubkey().to_owned()]),
            ],
            String::new(),
        );
        assert!(reused.validate_structure().is_err());

        let revised = bob.sign(
            1_700_000_100,
            INFO_KIND,
            vec![
                Tag::new(vec!["relay".into(), relay.into()]),
                Tag::new(vec!["mint".into(), mint.into(), "sat".into()]),
                Tag::new(vec!["pubkey".into(), wallet.pubkey().to_owned()]),
            ],
            String::new(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&info, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let target = format!("ab{}", "cd".repeat(31));
        let zap = alice.sign(
            1_700_000_200,
            NUTZAP_KIND,
            vec![
                Tag::new(vec![
                    "proof".into(),
                    proof(&format!("02{}", wallet.pubkey())),
                ]),
                Tag::new(vec!["unit".into(), "sat".into()]),
                Tag::new(vec!["u".into(), mint.into()]),
                Tag::new(vec!["e".into(), target.clone(), relay.into()]),
                Tag::new(vec!["k".into(), "1".into()]),
                Tag::new(vec!["p".into(), bob.pubkey().to_owned()]),
            ],
            "Thanks for this great idea.".into(),
        );
        zap.validate_structure().unwrap();
        assert_eq!(zap.class(), EventClass::Regular);
        let parsed = open_nutzap(&zap).unwrap();
        assert_eq!(parsed.recipient, bob.pubkey());
        assert_eq!(parsed.proofs[0].amount, 1);
        assert_eq!(parsed.event_kind, Some(1));
        nutzap_matches(&opened, &parsed).unwrap();
        let foreign = Nutzap {
            mint: "https://other.example".into(),
            ..parsed.clone()
        };
        assert!(nutzap_matches(&opened, &foreign).is_err());
        assert!(matches!(
            compare_replacement(&zap, &zap),
            Err(DomainError::NotReplaceable)
        ));

        let inbox = nutzap_inbox(bob.pubkey(), &[mint.to_owned()], 1_700_000_000).unwrap();
        assert_eq!(inbox.kinds.as_deref(), Some(&[NUTZAP_KIND][..]));
        assert!(inbox.matches(&zap));

        let plaintext = format!(
            r#"[["direction","in"],["amount","1"],["unit","sat"],["e","{}","","created"]]"#,
            "ab".repeat(32)
        );
        let sealed = seal_wallet_content(&secret("61"), &plaintext, [0x61; 32]).unwrap();
        let history = bob.sign(
            1_700_000_300,
            7_376,
            vec![
                Tag::new(vec![
                    "e".into(),
                    zap.id.clone(),
                    relay.into(),
                    "redeemed".into(),
                ]),
                Tag::new(vec!["p".into(), alice.pubkey().to_owned()]),
            ],
            sealed,
        );
        history.validate_structure().unwrap();
        let spent = open_spend_history(
            &read_wallet_content(&secret("61"), &history.content).unwrap(),
            &history,
        )
        .unwrap();
        assert_eq!(spent.direction, HistoryDirection::In);
        assert!(spent.refs.iter().any(|reference| {
            reference.event_id == zap.id && reference.role == TokenRole::Redeemed
        }));
    }
}
