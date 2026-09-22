//! NIP-46 remote signing.
//!
//! Kind `24133` carries a NIP-44 ciphertext and one `p` tag. The kind is
//! ephemeral, so a relay fans the event out and does not store it. The
//! ciphertext is a request (`id`, `method`, `params`) or a response
//! (`id`, `result`, `error`). `bunker://` and `nostrconnect://` name the
//! relays and the secret for a connection.
//!
//! The relay does not decrypt. A remote signer here answers `connect`,
//! `ping`, `get_public_key`, `sign_event`, the NIP-04 and NIP-44 helpers,
//! `switch_relays`, and `logout`. It does not fetch a NIP-89 announcement
//! or a `nostr.json` document.

use std::collections::BTreeMap;
use std::str::FromStr;

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use crate::nip44;

use super::{DomainError, Event, Tag};

const KIND: u16 = 24_133;

/// A `bunker://` token from the remote signer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BunkerUrl {
    pub remote_signer: String,
    pub relays: Vec<String>,
    pub secret: Option<String>,
}

/// A `nostrconnect://` token from the client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NostrConnectUrl {
    pub client: String,
    pub relays: Vec<String>,
    pub secret: String,
    pub perms: Vec<Permission>,
    pub name: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
}

/// One requested permission. `sign_event` may name a kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permission {
    pub method: String,
    pub kind: Option<u16>,
}

/// A decrypted request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRequest {
    pub id: String,
    pub method: String,
    pub params: Vec<String>,
}

/// A decrypted response. `error` is set when the call failed.
/// `result == "auth_url"` uses `error` as the URL to show the user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteResponse {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
struct Session {
    /// `None` means the connection did not name a permission list.
    perms: Option<Vec<Permission>>,
}

/// The bunker side of one NIP-46 session.
pub struct RemoteSigner {
    remote: SecretKey,
    user: SecretKey,
    bunker_secret: Option<String>,
    spent_bunker_secret: Option<String>,
    relays: Vec<String>,
    sessions: BTreeMap<String, Session>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn xonly_of(secret: &SecretKey) -> XOnlyPublicKey {
    let secp = Secp256k1::signing_only();
    Keypair::from_secret_key(&secp, secret)
        .x_only_public_key()
        .0
}

fn pubkey_hex(secret: &SecretKey) -> String {
    xonly_of(secret).to_string()
}

fn is_hex_pubkey(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn hex_pubkey(value: &str) -> Result<XOnlyPublicKey, DomainError> {
    if !is_hex_pubkey(value) {
        return Err(invalid("a remote-signing pubkey is 32 lowercase hex bytes"));
    }
    XOnlyPublicKey::from_str(value)
        .map_err(|_| invalid("a remote-signing pubkey is not on the curve"))
}

fn sign(secret: &SecretKey, created_at: u64, tags: Vec<Tag>, content: String) -> Event {
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind: KIND,
        tags,
        content,
        sig: "0".repeat(128),
    };
    let id = event
        .computed_id_bytes()
        .expect("serializing an owned event cannot fail");
    event.id = event
        .computed_id()
        .expect("serializing an owned event cannot fail");
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'+' {
            out.push(b' ');
            index += 1;
        } else if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &value[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
            } else {
                out.push(bytes[index]);
                index += 1;
            }
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query(url: &str, scheme: &str) -> Result<(String, Vec<(String, String)>), DomainError> {
    let Some(rest) = url.strip_prefix(scheme) else {
        return Err(invalid(
            "a connection token uses bunker:// or nostrconnect://",
        ));
    };
    let (pubkey, query) = rest
        .split_once('?')
        .ok_or_else(|| invalid("a connection token names its relays"))?;
    if !is_hex_pubkey(pubkey) {
        return Err(invalid("a remote-signing pubkey is 32 lowercase hex bytes"));
    }
    let pairs = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(name), percent_decode(value))
        })
        .collect();
    Ok((pubkey.to_owned(), pairs))
}

/// Parse a `bunker://` token.
pub fn parse_bunker(url: &str) -> Result<BunkerUrl, DomainError> {
    let (remote_signer, pairs) = query(url, "bunker://")?;
    let relays: Vec<String> = pairs
        .iter()
        .filter(|(name, _)| name == "relay")
        .map(|(_, value)| value.clone())
        .collect();
    if relays.is_empty() || relays.iter().any(|relay| !is_relay(relay)) {
        return Err(invalid("a bunker token names ws:// or wss:// relays"));
    }
    let secret = pairs
        .iter()
        .find(|(name, _)| name == "secret")
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty());
    Ok(BunkerUrl {
        remote_signer,
        relays,
        secret,
    })
}

/// Parse a `nostrconnect://` token. `relay` and `secret` are required.
pub fn parse_nostrconnect(url: &str) -> Result<NostrConnectUrl, DomainError> {
    let (client, pairs) = query(url, "nostrconnect://")?;
    let relays: Vec<String> = pairs
        .iter()
        .filter(|(name, _)| name == "relay")
        .map(|(_, value)| value.clone())
        .collect();
    if relays.is_empty() || relays.iter().any(|relay| !is_relay(relay)) {
        return Err(invalid("a nostrconnect token names ws:// or wss:// relays"));
    }
    let Some(secret) = pairs
        .iter()
        .find(|(name, _)| name == "secret")
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty())
    else {
        return Err(invalid("a nostrconnect token includes a secret"));
    };
    let perms = pairs
        .iter()
        .find(|(name, _)| name == "perms")
        .map(|(_, value)| parse_perms(value))
        .transpose()?
        .unwrap_or_default();
    let field = |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
    };
    Ok(NostrConnectUrl {
        client,
        relays,
        secret,
        perms,
        name: field("name"),
        url: field("url"),
        image: field("image"),
    })
}

fn parse_perms(value: &str) -> Result<Vec<Permission>, DomainError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|item| {
            let (method, kind) = item
                .split_once(':')
                .map(|(method, kind)| (method, Some(kind)))
                .unwrap_or((item, None));
            if method.is_empty() {
                return Err(invalid("a permission names a method"));
            }
            let kind = match kind {
                None => None,
                Some(kind) => Some(
                    kind.parse::<u16>()
                        .map_err(|_| invalid("a sign_event permission names a kind"))?,
                ),
            };
            if kind.is_some() && method != "sign_event" {
                return Err(invalid("only sign_event takes a kind permission"));
            }
            Ok(Permission {
                method: method.to_owned(),
                kind,
            })
        })
        .collect()
}

/// Kind `24133` admission. One `p` tag and NIP-44 framing. No decryption.
pub fn validate_remote_signing(event: &Event) -> Result<(), DomainError> {
    if event.kind != KIND {
        return Err(invalid("a remote-signing event has kind 24133"));
    }
    let recipients = event.tag_values("p").collect::<Vec<_>>();
    if recipients.len() != 1 || hex_pubkey(recipients[0]).is_err() {
        return Err(invalid("a remote-signing event has one p-tagged peer"));
    }
    nip44::payload_shape(&event.content)
        .map_err(|_| invalid("remote-signing content must be NIP-44 ciphertext"))?;
    Ok(())
}

/// Encrypt a request from `client` to `remote`.
pub fn seal_request(
    client: &SecretKey,
    remote: &XOnlyPublicKey,
    request: &RemoteRequest,
    created_at: u64,
    nonce: [u8; 32],
) -> Result<Event, DomainError> {
    if request.id.is_empty() || request.method.is_empty() {
        return Err(invalid("a remote-signing request has an id and a method"));
    }
    let plaintext = serde_json::to_string(request)
        .map_err(|error| DomainError::Serialization(error.to_string()))?;
    let content = nip44::encrypt(&plaintext, &nip44::conversation_key(client, remote), nonce)
        .map_err(|_| invalid("remote-signing content must be NIP-44 ciphertext"))?;
    Ok(sign(
        client,
        created_at,
        vec![Tag::new(vec!["p".into(), remote.to_string()])],
        content,
    ))
}

/// Decrypt a response addressed to `client`.
pub fn open_response(event: &Event, client: &SecretKey) -> Result<RemoteResponse, DomainError> {
    validate_remote_signing(event)?;
    let client_hex = pubkey_hex(client);
    if event.tag_values("p").next() != Some(client_hex.as_str()) {
        return Err(invalid("the response is addressed to another client"));
    }
    let signer = hex_pubkey(&event.pubkey)?;
    let plaintext = nip44::decrypt(&event.content, &nip44::conversation_key(client, &signer))
        .map_err(|_| invalid("a remote-signing response cannot be decrypted"))?;
    serde_json::from_str(&plaintext)
        .map_err(|_| invalid("a remote-signing response is not the JSON object"))
}

/// The URL in an auth challenge, when `result` is `auth_url`.
pub fn auth_challenge_url(response: &RemoteResponse) -> Option<&str> {
    match response.result.as_deref() {
        Some("auth_url") => response.error.as_deref(),
        _ => None,
    }
}

impl RemoteSigner {
    /// `bunker_secret` is consumed by the first successful `connect`.
    pub fn new(
        remote: SecretKey,
        user: SecretKey,
        bunker_secret: Option<String>,
        relays: Vec<String>,
    ) -> Self {
        Self {
            remote,
            user,
            bunker_secret,
            spent_bunker_secret: None,
            relays,
            sessions: BTreeMap::new(),
        }
    }

    pub fn remote_pubkey(&self) -> String {
        pubkey_hex(&self.remote)
    }

    pub fn user_pubkey(&self) -> String {
        pubkey_hex(&self.user)
    }

    /// Answer one kind `24133` request with a kind `24133` response.
    pub fn handle(
        &mut self,
        event: &Event,
        created_at: u64,
        nonce: [u8; 32],
    ) -> Result<Event, DomainError> {
        validate_remote_signing(event)?;
        let remote_hex = self.remote_pubkey();
        if event.tag_values("p").next() != Some(remote_hex.as_str()) {
            return Err(invalid("the request is addressed to another signer"));
        }
        let client = hex_pubkey(&event.pubkey)?;
        let plaintext = nip44::decrypt(
            &event.content,
            &nip44::conversation_key(&self.remote, &client),
        )
        .map_err(|_| invalid("a remote-signing request cannot be decrypted"))?;
        let request: RemoteRequest = serde_json::from_str(&plaintext)
            .map_err(|_| invalid("a remote-signing request is not the JSON object"))?;
        let response = self.dispatch(&event.pubkey, &request);
        self.seal_response(&event.pubkey, &response, created_at, nonce)
    }

    /// A response that asks the user to open `url` before the real result.
    pub fn auth_challenge(
        &self,
        client: &str,
        request_id: &str,
        url: &str,
        created_at: u64,
        nonce: [u8; 32],
    ) -> Result<Event, DomainError> {
        let response = RemoteResponse {
            id: request_id.to_owned(),
            result: Some("auth_url".into()),
            error: Some(url.to_owned()),
        };
        self.seal_response(client, &response, created_at, nonce)
    }

    fn seal_response(
        &self,
        client: &str,
        response: &RemoteResponse,
        created_at: u64,
        nonce: [u8; 32],
    ) -> Result<Event, DomainError> {
        let peer = hex_pubkey(client)?;
        let plaintext = serde_json::to_string(response)
            .map_err(|error| DomainError::Serialization(error.to_string()))?;
        let content = nip44::encrypt(
            &plaintext,
            &nip44::conversation_key(&self.remote, &peer),
            nonce,
        )
        .map_err(|_| invalid("remote-signing content must be NIP-44 ciphertext"))?;
        Ok(sign(
            &self.remote,
            created_at,
            vec![Tag::new(vec!["p".into(), client.to_owned()])],
            content,
        ))
    }

    fn dispatch(&mut self, client: &str, request: &RemoteRequest) -> RemoteResponse {
        let outcome = self.dispatch_method(client, request);
        match outcome {
            Ok(result) => RemoteResponse {
                id: request.id.clone(),
                result: Some(result),
                error: None,
            },
            Err(reason) => RemoteResponse {
                id: request.id.clone(),
                result: None,
                error: Some(reason),
            },
        }
    }

    fn dispatch_method(&mut self, client: &str, request: &RemoteRequest) -> Result<String, String> {
        if request.method == "connect" {
            return self.connect(client, request);
        }
        let Some(session) = self.sessions.get(client).cloned() else {
            return Err("connect before calling this method".into());
        };
        match request.method.as_str() {
            "ping" => Ok("pong".into()),
            "get_public_key" => Ok(self.user_pubkey()),
            "logout" => {
                self.sessions.remove(client);
                Ok("ack".into())
            }
            "switch_relays" => {
                if self.relays.is_empty() {
                    Ok("null".into())
                } else {
                    serde_json::to_string(&self.relays).map_err(|error| error.to_string())
                }
            }
            "sign_event" => self.sign_event(request, &session),
            "nip04_encrypt" | "nip04_decrypt" | "nip44_encrypt" | "nip44_decrypt" => {
                self.cipher(request, &session)
            }
            _ => Err("unsupported method".into()),
        }
    }

    fn connect(&mut self, client: &str, request: &RemoteRequest) -> Result<String, String> {
        let remote = request
            .params
            .first()
            .ok_or_else(|| "connect names the remote signer".to_owned())?;
        if remote != &self.remote_pubkey() {
            return Err("connect names the remote signer".into());
        }
        let secret = request.params.get(1).map(String::as_str).unwrap_or("");
        if self.spent_bunker_secret.as_deref() == Some(secret) && !secret.is_empty() {
            return Err("the bunker secret was already used".into());
        }
        let result = if self.bunker_secret.is_some() {
            if self.bunker_secret.as_deref() != Some(secret) {
                return Err("the bunker secret does not match".into());
            }
            self.spent_bunker_secret = self.bunker_secret.take();
            "ack".into()
        } else if secret.is_empty() {
            "ack".into()
        } else {
            secret.to_owned()
        };
        let perms = match request.params.get(2).map(String::as_str) {
            None | Some("") => None,
            Some(value) => Some(parse_perms(value).map_err(|error| error.to_string())?),
        };
        if let Some(metadata) = request.params.get(3).filter(|value| !value.is_empty()) {
            let _: serde_json::Value = serde_json::from_str(metadata)
                .map_err(|_| "client metadata is a JSON object".to_owned())?;
        }
        self.sessions.insert(client.to_owned(), Session { perms });
        Ok(result)
    }

    fn allowed(session: &Session, method: &str, kind: Option<u16>) -> bool {
        let Some(perms) = &session.perms else {
            return true;
        };
        perms.iter().any(|perm| {
            perm.method == method && (kind.is_none() || perm.kind.is_none() || perm.kind == kind)
        })
    }

    fn sign_event(&self, request: &RemoteRequest, session: &Session) -> Result<String, String> {
        let Some(body) = request.params.first() else {
            return Err("sign_event takes the event JSON".into());
        };
        let value: serde_json::Value =
            serde_json::from_str(body).map_err(|_| "sign_event takes the event JSON".to_owned())?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_u64)
            .and_then(|kind| u16::try_from(kind).ok())
            .ok_or_else(|| "sign_event takes the event JSON".to_owned())?;
        if !Self::allowed(session, "sign_event", Some(kind)) {
            return Err("sign_event is not permitted for this kind".into());
        }
        let content = value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "sign_event takes the event JSON".to_owned())?;
        let created_at = value
            .get("created_at")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "sign_event takes the event JSON".to_owned())?;
        let tags = serde_json::from_value::<Vec<Tag>>(
            value
                .get("tags")
                .cloned()
                .unwrap_or(serde_json::Value::Array(Vec::new())),
        )
        .map_err(|_| "sign_event takes the event JSON".to_owned())?;
        let event = sign_as(&self.user, created_at, kind, tags, content.to_owned());
        serde_json::to_string(&event).map_err(|error| error.to_string())
    }

    fn cipher(&self, request: &RemoteRequest, session: &Session) -> Result<String, String> {
        if !Self::allowed(session, &request.method, None) {
            return Err(format!("{} is not permitted", request.method));
        }
        let peer = request
            .params
            .first()
            .ok_or_else(|| "the cipher methods take a pubkey and a text".to_owned())?;
        let text = request
            .params
            .get(1)
            .ok_or_else(|| "the cipher methods take a pubkey and a text".to_owned())?;
        let peer = hex_pubkey(peer).map_err(|error| error.to_string())?;
        match request.method.as_str() {
            "nip44_encrypt" => nip44::encrypt(
                text,
                &nip44::conversation_key(&self.user, &peer),
                [4_u8; 32],
            ),
            "nip44_decrypt" => nip44::decrypt(text, &nip44::conversation_key(&self.user, &peer)),
            "nip04_encrypt" => crate::nip04::encrypt(text, &self.user, &peer, [5_u8; 16]),
            "nip04_decrypt" => crate::nip04::decrypt(text, &self.user, &peer),
            _ => Err("unsupported method".into()),
        }
    }
}

fn sign_as(
    secret: &SecretKey,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: String,
) -> Event {
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content,
        sig: "0".repeat(128),
    };
    let id = event
        .computed_id_bytes()
        .expect("serializing an owned event cannot fail");
    event.id = event
        .computed_id()
        .expect("serializing an owned event cannot fail");
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EventClass;

    fn secret(byte: u8) -> SecretKey {
        SecretKey::from_byte_array([byte; 32]).expect("secret")
    }

    #[test]
    fn a_client_connects_and_the_signer_returns_a_signed_event() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/46.md"
        ))
        .unwrap();
        assert!(text.contains("kind: 24133"));
        assert!(text.contains("bunker://"));
        assert!(text.contains("nostrconnect://"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "46.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "46.md")
        );

        let pinned = "nostrconnect://83f3b2ae6aa368e8275397b9c26cf550101d63ebaab900d19dd4a4429f5ad8f5?relay=wss%3A%2F%2Frelay1.example.com&perms=nip44_encrypt%2Cnip44_decrypt%2Csign_event%3A13%2Csign_event%3A14%2Csign_event%3A1059&name=My+Client&secret=0s8j2djs&relay=wss%3A%2F%2Frelay2.example2.com";
        let parsed = parse_nostrconnect(pinned).unwrap();
        assert_eq!(parsed.secret, "0s8j2djs");
        assert_eq!(parsed.name.as_deref(), Some("My Client"));
        assert_eq!(
            parsed.relays,
            vec![
                "wss://relay1.example.com".to_owned(),
                "wss://relay2.example2.com".to_owned()
            ]
        );
        assert!(
            parsed
                .perms
                .iter()
                .any(|perm| perm.method == "sign_event" && perm.kind == Some(13))
        );

        let client = secret(0x46);
        let remote = secret(0x47);
        let user = secret(0x48);
        let mut signer = RemoteSigner::new(
            remote,
            user,
            Some("one-time".into()),
            vec!["wss://relay.example".into()],
        );
        let bunker = format!(
            "bunker://{}?relay=wss%3A%2F%2Frelay.example&secret=one-time",
            signer.remote_pubkey()
        );
        assert_eq!(
            parse_bunker(&bunker).unwrap().secret.as_deref(),
            Some("one-time")
        );

        let connect = RemoteRequest {
            id: "c1".into(),
            method: "connect".into(),
            params: vec![
                signer.remote_pubkey(),
                "one-time".into(),
                "sign_event:1".into(),
                String::new(),
            ],
        };
        let request = seal_request(
            &client,
            &xonly_of(&remote),
            &connect,
            1_700_000_000,
            [7_u8; 32],
        )
        .unwrap();
        request.validate_structure().unwrap();
        assert_eq!(request.class(), EventClass::Ephemeral);
        let mut rejected = request.clone();
        rejected
            .tags
            .push(Tag::new(vec!["p".into(), pubkey_hex(&client)]));
        assert!(validate_remote_signing(&rejected).is_err());
        let response = signer.handle(&request, 1_700_000_001, [8_u8; 32]).unwrap();
        response.validate_structure().unwrap();
        let opened = open_response(&response, &client).unwrap();
        assert_eq!(opened.result.as_deref(), Some("ack"));
        assert!(opened.error.is_none());

        let again = RemoteRequest {
            id: "c2".into(),
            method: "connect".into(),
            params: vec![signer.remote_pubkey(), "one-time".into()],
        };
        let again = seal_request(
            &client,
            &xonly_of(&remote),
            &again,
            1_700_000_002,
            [9_u8; 32],
        )
        .unwrap();
        let again = open_response(
            &signer.handle(&again, 1_700_000_003, [10_u8; 32]).unwrap(),
            &client,
        )
        .unwrap();
        assert!(again.error.is_some());

        let public_key = RemoteRequest {
            id: "k".into(),
            method: "get_public_key".into(),
            params: Vec::new(),
        };
        let public_key = seal_request(
            &client,
            &xonly_of(&remote),
            &public_key,
            1_700_000_004,
            [11_u8; 32],
        )
        .unwrap();
        let public_key = open_response(
            &signer
                .handle(&public_key, 1_700_000_005, [12_u8; 32])
                .unwrap(),
            &client,
        )
        .unwrap();
        assert_eq!(
            public_key.result.as_deref(),
            Some(signer.user_pubkey().as_str())
        );
        assert_ne!(signer.user_pubkey(), signer.remote_pubkey());

        let body = serde_json::json!({
            "content": "Hello, I'm signing remotely",
            "kind": 1,
            "tags": [],
            "created_at": 1_714_078_911_u64
        })
        .to_string();
        let sign_request = RemoteRequest {
            id: "s".into(),
            method: "sign_event".into(),
            params: vec![body],
        };
        let sign_request = seal_request(
            &client,
            &xonly_of(&remote),
            &sign_request,
            1_700_000_006,
            [13_u8; 32],
        )
        .unwrap();
        let signed = open_response(
            &signer
                .handle(&sign_request, 1_700_000_007, [14_u8; 32])
                .unwrap(),
            &client,
        )
        .unwrap();
        let signed: Event = serde_json::from_str(signed.result.as_deref().unwrap()).unwrap();
        signed.validate_crypto().unwrap();
        assert_eq!(signed.pubkey, signer.user_pubkey());
        assert_eq!(signed.content, "Hello, I'm signing remotely");

        let other_kind = RemoteRequest {
            id: "s4".into(),
            method: "sign_event".into(),
            params: vec![
                serde_json::json!({"content":"no","kind":4,"tags":[],"created_at":1}).to_string(),
            ],
        };
        let other_kind = seal_request(
            &client,
            &xonly_of(&remote),
            &other_kind,
            1_700_000_008,
            [15_u8; 32],
        )
        .unwrap();
        let other_kind = open_response(
            &signer
                .handle(&other_kind, 1_700_000_009, [16_u8; 32])
                .unwrap(),
            &client,
        )
        .unwrap();
        assert!(other_kind.error.is_some());

        let ping = RemoteRequest {
            id: "p".into(),
            method: "ping".into(),
            params: Vec::new(),
        };
        let ping = seal_request(
            &client,
            &xonly_of(&remote),
            &ping,
            1_700_000_010,
            [17_u8; 32],
        )
        .unwrap();
        let ping = open_response(
            &signer.handle(&ping, 1_700_000_011, [18_u8; 32]).unwrap(),
            &client,
        )
        .unwrap();
        assert_eq!(ping.result.as_deref(), Some("pong"));

        let unknown = RemoteRequest {
            id: "u".into(),
            method: "create_account".into(),
            params: Vec::new(),
        };
        let unknown = seal_request(
            &client,
            &xonly_of(&remote),
            &unknown,
            1_700_000_012,
            [19_u8; 32],
        )
        .unwrap();
        let unknown = open_response(
            &signer.handle(&unknown, 1_700_000_013, [20_u8; 32]).unwrap(),
            &client,
        )
        .unwrap();
        assert_eq!(unknown.error.as_deref(), Some("unsupported method"));

        let challenge = signer
            .auth_challenge(
                &pubkey_hex(&client),
                "s",
                "https://signer.example/auth",
                1_700_000_014,
                [21_u8; 32],
            )
            .unwrap();
        let challenge = open_response(&challenge, &client).unwrap();
        assert_eq!(
            auth_challenge_url(&challenge),
            Some("https://signer.example/auth")
        );

        let logout = RemoteRequest {
            id: "out".into(),
            method: "logout".into(),
            params: Vec::new(),
        };
        let logout = seal_request(
            &client,
            &xonly_of(&remote),
            &logout,
            1_700_000_015,
            [22_u8; 32],
        )
        .unwrap();
        let logout = open_response(
            &signer.handle(&logout, 1_700_000_016, [23_u8; 32]).unwrap(),
            &client,
        )
        .unwrap();
        assert_eq!(logout.result.as_deref(), Some("ack"));
        let after = seal_request(
            &client,
            &xonly_of(&remote),
            &ping_request(),
            1_700_000_017,
            [24_u8; 32],
        )
        .unwrap();
        let after = open_response(
            &signer.handle(&after, 1_700_000_018, [25_u8; 32]).unwrap(),
            &client,
        )
        .unwrap();
        assert!(after.error.is_some());
    }

    fn ping_request() -> RemoteRequest {
        RemoteRequest {
            id: "p2".into(),
            method: "ping".into(),
            params: Vec::new(),
        }
    }
}
