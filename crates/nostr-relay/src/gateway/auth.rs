use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    net::IpAddr,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use crate::domain::Event;

use super::GatewayError;

const AUTH_KIND: u16 = 22_242;
const AUTH_WINDOW_SECONDS: u64 = 600;

pub struct AuthState {
    challenge: String,
    relay_url: String,
    authenticated: HashMap<String, Option<String>>,
}

impl AuthState {
    pub fn new(challenge: String, relay_url: String) -> Self {
        Self {
            challenge,
            relay_url,
            authenticated: HashMap::new(),
        }
    }

    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    pub fn is_authenticated(&self) -> bool {
        !self.authenticated.is_empty()
    }

    pub fn is_authenticated_as(&self, pubkey: &str) -> bool {
        self.authenticated.contains_key(pubkey)
    }

    pub fn is_directly_authenticated_as(&self, pubkey: &str) -> bool {
        matches!(self.authenticated.get(pubkey), Some(None))
    }

    pub fn authenticated_pubkeys(&self) -> Vec<String> {
        self.authenticated.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn authenticate(&mut self, event: &Event, now: u64) -> Result<(), String> {
        self.verify(event, now)?;
        self.accept_direct(event.pubkey.clone());
        Ok(())
    }

    pub fn verify(&self, event: &Event, now: u64) -> Result<(), String> {
        event
            .validate_structure()
            .map_err(|error| format!("invalid: {error}"))?;
        event
            .validate_crypto()
            .map_err(|error| format!("invalid: {error}"))?;
        if event.kind != AUTH_KIND {
            return Err("invalid: authentication event must have kind 22242".to_owned());
        }
        if event.created_at.abs_diff(now) > AUTH_WINDOW_SECONDS {
            return Err("invalid: authentication event timestamp is outside 10 minutes".to_owned());
        }
        if !event
            .tag_values("challenge")
            .any(|value| value == self.challenge)
        {
            return Err("invalid: authentication challenge does not match".to_owned());
        }
        let expected_relay = normalize_relay_url(&self.relay_url);
        if !event
            .tag_values("relay")
            .any(|value| normalize_relay_url(value) == expected_relay)
        {
            return Err("invalid: authentication relay URL does not match".to_owned());
        }
        Ok(())
    }

    pub fn accept_direct(&mut self, pubkey: String) {
        self.authenticated.insert(pubkey, None);
    }

    pub fn accept_virtual(&mut self, pubkey: String, owner_pubkey: String) {
        self.authenticated.insert(pubkey, Some(owner_pubkey));
    }

    pub fn virtual_owner_for(&self, pubkey: &str) -> Option<&str> {
        self.authenticated.get(pubkey).and_then(Option::as_deref)
    }
}

pub fn read_process_secret() -> Result<[u8; 32], GatewayError> {
    let mut secret = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut secret))
        .map_err(|error| {
            GatewayError::Internal(format!("cannot obtain challenge entropy: {error}"))
        })?;
    Ok(secret)
}

pub fn make_challenge(secret: &[u8; 32], connection_id: u64, peer: IpAddr) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut digest = Sha256::new();
    digest.update(secret);
    digest.update(connection_id.to_be_bytes());
    digest.update(now.to_be_bytes());
    digest.update(peer.to_string().as_bytes());
    encode_hex(&digest.finalize())
}

fn normalize_relay_url(value: &str) -> &str {
    value.trim_end_matches('/')
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use secp256k1::{Keypair, Secp256k1, SecretKey};
    use serde::Deserialize;

    use crate::domain::{Event, Tag};

    use super::AuthState;

    #[derive(Deserialize)]
    struct Fixture {
        relay_url: String,
        challenge: String,
        now: u64,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        kind: u16,
        offset: i64,
        challenge: String,
        relay: String,
        valid: bool,
    }

    #[test]
    fn nip42_fixture_corpus() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../../../tests/fixtures/nip42/auth.json"))
                .unwrap();
        for case in fixture.cases {
            let created_at = fixture.now.checked_add_signed(case.offset).unwrap();
            let event = signed_event(
                created_at,
                case.kind,
                vec![
                    Tag::new(vec!["relay".into(), case.relay]),
                    Tag::new(vec!["challenge".into(), case.challenge]),
                ],
            );
            let mut auth = AuthState::new(fixture.challenge.clone(), fixture.relay_url.clone());
            assert_eq!(
                auth.authenticate(&event, fixture.now).is_ok(),
                case.valid,
                "NIP-42 fixture case: {}",
                case.name
            );
            assert_eq!(auth.is_authenticated(), case.valid, "case: {}", case.name);
        }

        let mut virtual_auth = AuthState::new("virtual".into(), fixture.relay_url);
        virtual_auth.accept_virtual("agent".into(), "owner".into());
        assert!(virtual_auth.is_authenticated_as("agent"));
        assert!(!virtual_auth.is_directly_authenticated_as("agent"));
        virtual_auth.accept_direct("agent".into());
        assert!(virtual_auth.is_directly_authenticated_as("agent"));
    }

    fn signed_event(created_at: u64, kind: u16, tags: Vec<Tag>) -> Event {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_byte_array([42; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let mut event = Event {
            id: "0".repeat(64),
            pubkey: keypair.x_only_public_key().0.to_string(),
            created_at,
            kind,
            tags,
            content: String::new(),
            sig: "0".repeat(128),
        };
        let id = event.computed_id_bytes().unwrap();
        event.id = event.computed_id().unwrap();
        event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
        event
    }
}
