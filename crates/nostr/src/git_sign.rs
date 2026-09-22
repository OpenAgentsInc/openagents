//! NIP-GS git object signing.
//!
//! The client signs the bytes git would write to a program's stdin and
//! verifies the armored envelope. This module does not talk to a relay:
//! the pinned specification defines no event kind.

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
use sha2::{Digest, Sha256};

/// Domain separator mixed into every git signing hash.
pub const DOMAIN: &str = "nostr:git:v1:";
/// Git refuses a payload larger than this.
pub const MAX_PAYLOAD_BYTES: usize = 100 * 1024 * 1024;
/// Decoded envelope JSON must stay within this bound.
pub const MAX_ENVELOPE_BYTES: usize = 2_048;
const MAX_BASE64_LINE: usize = 4_096;
const BEGIN: &str = "-----BEGIN SIGNED MESSAGE-----";
const END: &str = "-----END SIGNED MESSAGE-----";

/// How a verified signature relates to the configured signing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitTrust {
    /// `pk` matches the configured signing key.
    Fully,
    /// The signature is valid and the key is not the configured one.
    Undefined,
}

/// A verified git signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitVerification {
    /// Signer pubkey, 64 lowercase hex characters.
    pub pubkey: String,
    /// Signing timestamp from the envelope.
    pub timestamp: u64,
    /// Local trust, which is not a global assertion.
    pub trust: GitTrust,
    /// `Some` when `oa` was present. False leaves the git signature valid.
    pub owner_authorized: Option<bool>,
    /// GnuPG status text a verifier writes after success.
    pub status: String,
}

/// Why a git signature was refused, with the status text to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    /// `ERRSIG` or `BADSIG` status text.
    pub status: String,
    /// Short cause.
    pub reason: &'static str,
}

/// Armor and signing status for one payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitArmored {
    /// The signature block git stores.
    pub armor: String,
    /// GnuPG status text a signer writes after success.
    pub status: String,
}

/// Sign `payload` at `timestamp`. `owner` is `(owner pubkey, conditions, owner sig)`.
///
/// # Errors
///
/// Returns an error when the payload or envelope is outside the specification
/// limits, or when `owner` is not three well-formed strings.
pub fn sign_git_object(
    secret: &SecretKey,
    payload: &[u8],
    timestamp: u64,
    owner: Option<(&str, &str, &str)>,
) -> Result<GitArmored, GitError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(errsig_unknown("payload"));
    }
    if timestamp > u64::from(u32::MAX) {
        return Err(errsig_unknown("timestamp"));
    }
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, secret);
    let pubkey = keypair.x_only_public_key().0.to_string();
    let owner = match owner {
        Some((owner_pubkey, conditions, owner_sig)) => {
            validate_owner_parts(owner_pubkey, conditions, owner_sig)?;
            Some((
                owner_pubkey.to_owned(),
                conditions.to_owned(),
                owner_sig.to_owned(),
            ))
        }
        None => None,
    };
    let hash = signing_hash(timestamp, owner.as_ref(), payload);
    let signature = secp.sign_schnorr_no_aux_rand(&hash, &keypair).to_string();
    let json = canonical_json(timestamp, &pubkey, &signature, owner.as_ref());
    if json.len() > MAX_ENVELOPE_BYTES {
        return Err(errsig_unknown("envelope"));
    }
    let encoded = base64_encode(json.as_bytes());
    if encoded.len() > MAX_BASE64_LINE {
        return Err(errsig_unknown("armor"));
    }
    Ok(GitArmored {
        armor: format!("{BEGIN}\n{encoded}\n{END}\n"),
        status: format!(
            "[GNUPG:] BEGIN_SIGNING\n[GNUPG:] SIG_CREATED D 8 1 00 {timestamp} {pubkey}\n"
        ),
    })
}

/// Verify an armored signature over `payload`.
///
/// `trusted` is `user.signingkey`: 64 hex characters or an `npub`.
///
/// # Errors
///
/// Returns [`GitError`] with `ERRSIG` or `BADSIG` status text.
pub fn verify_git_object(
    armor: &str,
    payload: &[u8],
    trusted: Option<&str>,
) -> Result<GitVerification, GitError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(errsig_unknown("payload"));
    }
    let encoded = armor_payload(armor)?;
    let decoded = base64_decode(encoded).map_err(|_| errsig_unknown("base64"))?;
    if decoded.len() > MAX_ENVELOPE_BYTES {
        return Err(errsig_unknown("envelope"));
    }
    let text = std::str::from_utf8(&decoded).map_err(|_| errsig_unknown("utf8"))?;
    reject_duplicate_keys(text).map_err(|_| errsig_unknown("duplicate"))?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| errsig_unknown("json"))?;
    let object = value.as_object().ok_or_else(|| errsig_unknown("json"))?;
    let allowed = ["v", "pk", "sig", "t", "oa"];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(errsig_unknown("field"));
    }
    if object.get("v").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(errsig_unknown("version"));
    }
    let pubkey = object
        .get("pk")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| errsig_unknown("pk"))?;
    let signature = object
        .get("sig")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| errsig_unknown("sig"))?;
    let timestamp = object
        .get("t")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value <= u64::from(u32::MAX))
        .ok_or_else(|| errsig_unknown("timestamp"))?;
    let owner = match object.get("oa") {
        None => None,
        Some(value) => Some(owner_from_value(value)?),
    };
    let canonical = canonical_json(timestamp, pubkey, signature, owner.as_ref());
    if canonical.as_bytes() != decoded {
        return Err(errsig(pubkey, "canonical"));
    }
    let public_key = xonly(pubkey).map_err(|_| errsig(pubkey, "pk"))?;
    let parsed =
        Signature::from_byte_array(decode_hex::<64>(signature).map_err(|_| errsig(pubkey, "sig"))?);
    let hash = signing_hash(timestamp, owner.as_ref(), payload);
    if Secp256k1::verification_only()
        .verify_schnorr(&parsed, &hash, &public_key)
        .is_err()
    {
        return Err(GitError {
            status: format!("[GNUPG:] NEWSIG\n[GNUPG:] BADSIG {pubkey} {pubkey}\n"),
            reason: "signature",
        });
    }
    let owner_authorized = owner.as_ref().map(|(owner_pubkey, conditions, owner_sig)| {
        owner_signature_ok(pubkey, owner_pubkey, conditions, owner_sig)
    });
    let trust = match trusted {
        Some(configured) if trusted_matches(configured, pubkey) => GitTrust::Fully,
        _ => GitTrust::Undefined,
    };
    let trust_word = match trust {
        GitTrust::Fully => "TRUST_FULLY",
        GitTrust::Undefined => "TRUST_UNDEFINED",
    };
    let date = utc_date(timestamp);
    Ok(GitVerification {
        pubkey: pubkey.to_owned(),
        timestamp,
        trust,
        owner_authorized,
        status: format!(
            "[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG {pubkey} {pubkey}\n[GNUPG:] VALIDSIG {pubkey} {date} {timestamp} 0 - - - - - {pubkey}\n[GNUPG:] {trust_word} 0 shell\n"
        ),
    })
}

/// UTC `YYYY-MM-DD` for a unix timestamp.
#[must_use]
pub fn utc_date(timestamp: u64) -> String {
    let (year, month, day) = civil_date(timestamp);
    format!("{year:04}-{month:02}-{day:02}")
}

fn signing_hash(
    timestamp: u64,
    owner: Option<&(String, String, String)>,
    payload: &[u8],
) -> [u8; 32] {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(DOMAIN.as_bytes());
    preimage.extend_from_slice(timestamp.to_string().as_bytes());
    preimage.push(b':');
    if let Some((owner_pubkey, conditions, owner_sig)) = owner {
        preimage.extend_from_slice(owner_pubkey.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(conditions.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(owner_sig.as_bytes());
        preimage.push(b':');
    }
    preimage.extend_from_slice(payload);
    Sha256::digest(&preimage).into()
}

fn canonical_json(
    timestamp: u64,
    pubkey: &str,
    signature: &str,
    owner: Option<&(String, String, String)>,
) -> String {
    let mut json =
        format!("{{\"v\":1,\"pk\":\"{pubkey}\",\"sig\":\"{signature}\",\"t\":{timestamp}");
    if let Some((owner_pubkey, conditions, owner_sig)) = owner {
        json.push_str(",\"oa\":[");
        json.push_str(&json_string(owner_pubkey));
        json.push(',');
        json.push_str(&json_string(conditions));
        json.push(',');
        json.push_str(&json_string(owner_sig));
        json.push(']');
    }
    json.push('}');
    json
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{code:04x}", code = character as u32));
            }
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

fn owner_from_value(value: &serde_json::Value) -> Result<(String, String, String), GitError> {
    let items = value.as_array().ok_or_else(|| errsig_unknown("oa"))?;
    if items.len() != 3 {
        return Err(errsig_unknown("oa"));
    }
    let parts = items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| errsig_unknown("oa"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_owner_parts(&parts[0], &parts[1], &parts[2])?;
    Ok((parts[0].clone(), parts[1].clone(), parts[2].clone()))
}

fn validate_owner_parts(pubkey: &str, conditions: &str, signature: &str) -> Result<(), GitError> {
    xonly(pubkey).map_err(|_| errsig_unknown("oa"))?;
    if decode_hex::<64>(signature).is_err() {
        return Err(errsig_unknown("oa"));
    }
    if conditions.chars().any(|character| character.is_control()) {
        return Err(errsig_unknown("oa"));
    }
    Ok(())
}

fn owner_signature_ok(agent: &str, owner: &str, conditions: &str, signature: &str) -> bool {
    if owner == agent {
        return false;
    }
    let Ok(owner_key) = xonly(owner) else {
        return false;
    };
    let Ok(bytes) = decode_hex::<64>(signature) else {
        return false;
    };
    let signature = Signature::from_byte_array(bytes);
    let digest: [u8; 32] =
        Sha256::digest(format!("nostr:agent-auth:{agent}:{conditions}").as_bytes()).into();
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &digest, &owner_key)
        .is_ok()
}

fn trusted_matches(configured: &str, pubkey: &str) -> bool {
    let normalized = configured
        .strip_prefix("npub1")
        .and_then(|_| crate::nip19::decode_npub(configured).ok())
        .map(|bytes| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .unwrap_or_else(|| configured.to_ascii_lowercase());
    normalized == pubkey
}

fn armor_payload(armor: &str) -> Result<&str, GitError> {
    if armor.contains('\r') {
        return Err(errsig_unknown("armor"));
    }
    let mut lines: Vec<&str> = armor.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if lines.last() == Some(&"") || lines.len() != 3 || lines[0] != BEGIN || lines[2] != END {
        return Err(errsig_unknown("armor"));
    }
    let payload = lines[1];
    if payload.is_empty()
        || payload.len() > MAX_BASE64_LINE
        || payload.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(errsig_unknown("armor"));
    }
    Ok(payload)
}

fn errsig_unknown(reason: &'static str) -> GitError {
    errsig("0000000000000000", reason)
}

fn errsig(key_id: &str, reason: &'static str) -> GitError {
    let key_id = if key_id.len() == 64 {
        key_id
    } else {
        "0000000000000000"
    };
    GitError {
        status: format!("[GNUPG:] ERRSIG {key_id} 0 0 00 0 9\n"),
        reason,
    }
}

fn xonly(value: &str) -> Result<XOnlyPublicKey, ()> {
    XOnlyPublicKey::from_byte_array(decode_hex::<32>(value)?).map_err(|_| ())
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], ()> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(());
    }
    let mut out = [0_u8; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| ())?;
    }
    Ok(out)
}

fn reject_duplicate_keys(input: &str) -> Result<(), ()> {
    let mut scan = Scan {
        bytes: input.as_bytes(),
        index: 0,
    };
    scan.value()?;
    scan.skip_ws();
    if scan.index != scan.bytes.len() {
        return Err(());
    }
    Ok(())
}

struct Scan<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Scan<'_> {
    fn value(&mut self) -> Result<(), ()> {
        self.skip_ws();
        match self.peek().ok_or(())? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(|_| ()),
            b't' => self.literal(b"true"),
            b'f' => self.literal(b"false"),
            b'n' => self.literal(b"null"),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(()),
        }
    }

    fn object(&mut self) -> Result<(), ()> {
        self.bump(b'{')?;
        let mut keys = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.index += 1;
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            if keys.iter().any(|existing: &String| existing == &key) {
                return Err(());
            }
            keys.push(key);
            self.skip_ws();
            self.bump(b':')?;
            self.value()?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.index += 1,
                Some(b'}') => {
                    self.index += 1;
                    return Ok(());
                }
                _ => return Err(()),
            }
        }
    }

    fn array(&mut self) -> Result<(), ()> {
        self.bump(b'[')?;
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.index += 1;
            return Ok(());
        }
        loop {
            self.value()?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.index += 1,
                Some(b']') => {
                    self.index += 1;
                    return Ok(());
                }
                _ => return Err(()),
            }
        }
    }

    fn string(&mut self) -> Result<String, ()> {
        self.bump(b'"')?;
        let mut out = String::new();
        while let Some(byte) = self.peek() {
            self.index += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escaped = self.peek().ok_or(())?;
                    self.index += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' => out.push(char::from(escaped)),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut hex = [0_u8; 4];
                            for slot in &mut hex {
                                *slot = self.peek().ok_or(())?;
                                self.index += 1;
                            }
                            let digits = std::str::from_utf8(&hex).map_err(|_| ())?;
                            let code = u32::from_str_radix(digits, 16).map_err(|_| ())?;
                            out.push(char::from_u32(code).ok_or(())?);
                        }
                        _ => return Err(()),
                    }
                }
                byte if byte < 0x20 => return Err(()),
                byte => out.push(char::from(byte)),
            }
        }
        Err(())
    }

    fn literal(&mut self, expected: &[u8]) -> Result<(), ()> {
        for byte in expected {
            self.bump(*byte)?;
        }
        Ok(())
    }

    fn number(&mut self) -> Result<(), ()> {
        if self.peek() == Some(b'-') {
            self.index += 1;
        }
        let start = self.index;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.index += 1;
        }
        if self.index == start {
            return Err(());
        }
        if self.peek() == Some(b'.') {
            self.index += 1;
            let fraction = self.index;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.index += 1;
            }
            if self.index == fraction {
                return Err(());
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            let exponent = self.index;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.index += 1;
            }
            if self.index == exponent {
                return Err(());
            }
        }
        Ok(())
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn bump(&mut self, expected: u8) -> Result<(), ()> {
        if self.peek() == Some(expected) {
            self.index += 1;
            Ok(())
        } else {
            Err(())
        }
    }
}

fn civil_date(timestamp: u64) -> (i32, u32, u32) {
    let z = i64::try_from(timestamp / 86_400).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = u64::try_from(z - era * 146_097).unwrap_or(0);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = i64::try_from(year_of_era).unwrap_or(0) + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (
        i32::try_from(year).unwrap_or(1970),
        month as u32,
        day as u32,
    )
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(char::from(ALPHABET[usize::from(first >> 2)]));
        encoded.push(char::from(
            ALPHABET[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        if chunk.len() > 1 {
            encoded.push(char::from(
                ALPHABET[usize::from(((second & 0x0f) << 2) | (third >> 6))],
            ));
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(char::from(ALPHABET[usize::from(third & 0x3f)]));
        } else {
            encoded.push('=');
        }
    }
    encoded
}

fn base64_decode(value: &str) -> Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(4) {
        return Err(());
    }
    let mut out = Vec::new();
    for chunk in value.as_bytes().chunks(4) {
        let mut values = [0_u8; 4];
        let mut pad = 0;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if byte == b'=' {
                if index < 2 {
                    return Err(());
                }
                pad += 1;
                values[index] = 0;
            } else if pad > 0 {
                return Err(());
            } else {
                values[index] = base64_value(byte).ok_or(())?;
            }
        }
        out.push((values[0] << 2) | (values[1] >> 4));
        if pad < 2 {
            out.push((values[1] << 4) | (values[2] >> 2));
        }
        if pad < 1 {
            out.push((values[2] << 6) | values[3]);
        }
    }
    Ok(out)
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
mod tests {
    use super::*;

    fn secret(byte: u8) -> SecretKey {
        SecretKey::from_byte_array([byte; 32]).unwrap()
    }

    #[test]
    fn a_signed_payload_verifies_and_a_changed_byte_does_not() {
        let secret = secret(0x11);
        let payload = b"commit payload\n";
        let signed = sign_git_object(&secret, payload, 1_700_000_000, None).unwrap();
        assert!(signed.status.contains("SIG_CREATED D 8 1 00 1700000000"));
        let verified = verify_git_object(
            &signed.armor,
            payload,
            Some(&signed.status[signed.status.len() - 65..signed.status.len() - 1]),
        )
        .unwrap();
        assert_eq!(verified.timestamp, 1_700_000_000);
        assert_eq!(utc_date(1_700_000_000), "2023-11-14");
        assert!(verified.owner_authorized.is_none());
        assert!(verified.status.contains("GOODSIG"));
        let refused = verify_git_object(&signed.armor, b"commit payload\r", None).unwrap_err();
        assert!(refused.status.contains("BADSIG"));
    }

    #[test]
    fn owner_attestation_is_bound_into_the_git_hash() {
        let agent = secret(0x21);
        let owner = secret(0x31);
        let agent_key = Keypair::from_secret_key(&Secp256k1::signing_only(), &agent);
        let owner_key = Keypair::from_secret_key(&Secp256k1::signing_only(), &owner);
        let agent_pub = agent_key.x_only_public_key().0.to_string();
        let owner_pub = owner_key.x_only_public_key().0.to_string();
        let digest: [u8; 32] =
            Sha256::digest(format!("nostr:agent-auth:{agent_pub}:").as_bytes()).into();
        let owner_sig = Secp256k1::signing_only()
            .sign_schnorr_no_aux_rand(&digest, &owner_key)
            .to_string();
        let signed = sign_git_object(
            &agent,
            b"tree",
            1_700_000_000,
            Some((&owner_pub, "", &owner_sig)),
        )
        .unwrap();
        let verified = verify_git_object(&signed.armor, b"tree", None).unwrap();
        assert_eq!(verified.owner_authorized, Some(true));
        let unbound = sign_git_object(
            &agent,
            b"tree",
            1_700_000_000,
            Some((&owner_pub, "", &"0".repeat(128))),
        )
        .unwrap();
        let unverified_owner = verify_git_object(&unbound.armor, b"tree", None).unwrap();
        assert_eq!(unverified_owner.owner_authorized, Some(false));
        assert!(verify_git_object(&signed.armor, b"other", None).is_err());
    }
}
