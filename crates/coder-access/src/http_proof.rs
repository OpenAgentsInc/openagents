//! Exact-request NIP-98 proofs for the optional app device connection.
use crate::{Code, Error, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use nostr::domain::{Event, RelaySigner, Tag};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DEVICE_HEADER: &str = "x-openagents-device-proof";
pub const HOST_HEADER: &str = "x-openagents-host-proof";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub url: String,
    pub method: String,
    pub payload: String,
}
impl Intent {
    pub fn new(url: &str, method: &str, body: &[u8]) -> Result<Self> {
        let url = url
            .strip_prefix("wss://")
            .map(|s| format!("https://{s}"))
            .or_else(|| url.strip_prefix("ws://").map(|s| format!("http://{s}")))
            .unwrap_or_else(|| url.to_owned());
        let result = Self {
            url,
            method: method.into(),
            payload: format!("{:x}", Sha256::digest(body)),
        };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        let url = url::Url::parse(&self.url).map_err(|_| invalid())?;
        if self.url.len() > 8192
            || !matches!(url.scheme(), "http" | "https")
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url
                .query_pairs()
                .any(|(key, value)| key == "token" && !value.is_empty())
            || !matches!(self.method.as_str(), "GET" | "POST" | "PUT" | "DELETE")
            || self.payload.len() != 64
            || !self
                .payload
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn has_origin(&self, origin: &str) -> bool {
        let Ok(url) = url::Url::parse(&self.url) else {
            return false;
        };
        let Ok(origin) = url::Url::parse(origin) else {
            return false;
        };
        url.origin() == origin.origin()
    }
}
fn invalid() -> Error {
    Error::new(Code::Malformed, "Invalid device proof.")
}

pub fn sign(
    secret: &SecretKey,
    intent: &Intent,
    now: u64,
    extra: Vec<(&str, String)>,
) -> Result<Event> {
    intent.validate()?;
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|_| invalid())?;
    let nonce: [u8; 16] = secp256k1::rand::random();
    let nonce = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut tags = vec![
        Tag::new(vec!["u".into(), intent.url.clone()]),
        Tag::new(vec!["method".into(), intent.method.clone()]),
        Tag::new(vec!["payload".into(), intent.payload.clone()]),
        Tag::new(vec!["nonce".into(), nonce]),
    ];
    tags.extend(
        extra
            .into_iter()
            .map(|(name, value)| Tag::new(vec![name.into(), value])),
    );
    Ok(signer.sign(now, 27235, tags, String::new()))
}
pub fn header(event: &Event) -> Result<String> {
    Ok(format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(event).map_err(|_| invalid())?)
    ))
}
pub fn verify(header: &str, intent: &Intent, now: u64) -> Result<Event> {
    if header.len() > 24 * 1024 {
        return Err(invalid());
    }
    nostr::domain::parse_http_authorization_hash(
        header,
        &intent.method,
        &intent.url,
        Some(&intent.payload),
        now,
    )
    .map_err(|_| invalid())?;
    let bytes = STANDARD
        .decode(header.strip_prefix("Nostr ").ok_or_else(invalid)?)
        .map_err(|_| invalid())?;
    let event: Event = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if !event.content.is_empty() {
        return Err(invalid());
    }
    Ok(event)
}
pub fn tag<'a>(event: &'a Event, name: &str) -> Result<&'a str> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect();
    match tags.as_slice() {
        [tag] if tag.0.len() == 2 => tag.value().ok_or_else(invalid),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proofs_bind_body_method_url_query_key_and_time() {
        let key = SecretKey::new(&mut secp256k1::rand::rng());
        let intent = Intent::new("wss://sync.example/chat2/a/ws?device=phone", "GET", b"").unwrap();
        let signed = sign(&key, &intent, 100, vec![("account", "a".into())]).unwrap();
        let proof = header(&signed).unwrap();
        assert_eq!(verify(&proof, &intent, 100).unwrap().pubkey, signed.pubkey);
        for other in [
            Intent::new(&intent.url, "POST", b"").unwrap(),
            Intent::new(&intent.url, "GET", b"changed").unwrap(),
            Intent::new("https://sync.example/chat2/a/ws?device=other", "GET", b"").unwrap(),
        ] {
            assert!(verify(&proof, &other, 100).is_err());
        }
        assert!(verify(&proof, &intent, 161).is_err());
        assert!(
            Intent::new(
                "https://sync.example/chat2/a/ws?token=fixture-session",
                "GET",
                b""
            )
            .is_err()
        );
        let mut changed = signed.clone();
        changed.pubkey = "a".repeat(64);
        assert!(verify(&header(&changed).unwrap(), &intent, 100).is_err());
        assert_ne!(sign(&key, &intent, 100, vec![]).unwrap().id, signed.id);
        assert_eq!(tag(&signed, "account").unwrap(), "a");
    }
}
