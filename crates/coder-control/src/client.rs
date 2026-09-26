//! Exact signed messages. Constructing one grants no host authority.
use crate::*;
use nostr::domain::{RelaySigner, Tag};
use secp256k1::rand::RngCore;
use secp256k1::{SecretKey, XOnlyPublicKey};
use std::str::FromStr;

mod text;
pub use text::{TextDelivery, fetch_text, text};

pub fn random_id() -> String {
    random_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub(crate) fn random_bytes() -> [u8; 32] {
    let mut bytes = [0; 32];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    bytes
}
pub fn pubkey(secret: &SecretKey) -> String {
    secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}

/// The explicit trusted host configuration a thin client needs. Local task
/// directories and executable grants are deliberately absent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema: String,
    pub owner: String,
    pub authority: String,
    pub scope: Value,
    pub policy: Value,
    pub operations: BTreeMap<String, Operation>,
    pub retain_until: u64,
}
impl Configuration {
    pub fn verify_grant(&self, event: &Event, secret: &SecretKey, now: u64) -> Result<Value> {
        let opened = nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
        let grant = nostr::control::parse(opened.inline_bytes().ok_or("grant bytes unavailable")?)
            .map_err(|e| e.to_string())?;
        if self.schema != "openagents.control-client.v1"
            || opened.signer() != self.authority
            || grant["v"] != nostr::control::GRANT
            || grant["owner"] != self.owner
            || grant["authority"] != self.authority
            || grant["client"] != pubkey(secret)
            || grant["scope"] != self.scope
            || grant["policy"] != self.policy
            || grant["issued_at"]
                .as_u64()
                .is_none_or(|issued| issued > now)
            || grant["expires_at"]
                .as_u64()
                .is_none_or(|expires| expires <= now)
        {
            return Err(
                "grant differs from the client's admitted authority, scope, policy, or time".into(),
            );
        }
        Ok(grant)
    }
}
pub fn envelope(
    value: &Value,
    secret: &SecretKey,
    recipient: &str,
    now: u64,
    retain_until: u64,
) -> Result<Event> {
    if retain_until <= now {
        return Err("artifact retention must follow issue time".into());
    }
    let schema = value["v"].as_str().ok_or("artifact version")?;
    let bytes = jcs(value).map_err(|e| e.to_string())?;
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference(&bytes,"application/json",schema),"inline":value,"issued_at":now,"retain_until":retain_until});
    nostr::private_artifact::seal(
        &body,
        secret,
        &XOnlyPublicKey::from_str(recipient).map_err(|e| e.to_string())?,
        &random_id(),
        now,
        random_bytes(),
    )
    .map_err(|e| e.to_string())
}
#[allow(
    clippy::too_many_arguments,
    reason = "Keep the signed identity, exact input, and each explicit lifetime visible at the call site."
)]
pub fn request(
    secret: &SecretKey,
    authority: &str,
    operation: &Operation,
    input: &Event,
    input_ref: &Value,
    now: u64,
    deadline: u64,
    retain_until: u64,
    request: &str,
) -> Result<Event> {
    if input.pubkey != pubkey(secret) || !input.tag_values("p").any(|key| key == authority) {
        return Err("control input signer or recipient differs".into());
    }
    let input_ref = event_reference(input, input_ref)?;
    let body = json!({"v":nostr::execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":random_id(),"target":operation.target,"lock":operation.lock,"input":{"artifact":input_ref},"context":operation.context,"requirements":operation.requirements,"bounds":{"output_bytes":1048576},"deadline":deadline,"retain_until":retain_until});
    seal(
        secret,
        authority,
        nostr::execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), authority.into()]),
            Tag::new(vec!["expiration".into(), deadline.to_string()]),
        ],
        &body,
        now,
    )
}
pub fn event_reference(event: &Event, artifact: &Value) -> Result<Value> {
    nostr::private_artifact::admit(event).map_err(|e| e.to_string())?;
    let mut reference = artifact.clone();
    reference["event"] = json!({"id":event.id,"pubkey":event.pubkey,"kind":event.kind});
    parse_artifact(&reference).map_err(|e| e.to_string())?;
    Ok(reference)
}
/// Verify the exact worker, recipient, execute event, request, and attempt.
pub fn result(
    event: &Event,
    request: &Event,
    authority: &str,
    request_id: &str,
    secret: &SecretKey,
) -> Result<Value> {
    nostr::execution::bind_worker_event(
        event,
        &nostr::execution::Pending {
            execute_event: &request.id,
            worker: authority,
            customer: &pubkey(secret),
            request: request_id,
            attempt: 1,
        },
        secret,
    )
    .map_err(|e| format!("{e:?}"))
}
pub(crate) fn seal(
    secret: &SecretKey,
    recipient: &str,
    kind: u16,
    tags: Vec<Tag>,
    value: &Value,
    now: u64,
) -> Result<Event> {
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|e| e.to_string())?;
    let peer = XOnlyPublicKey::from_str(recipient).map_err(|e| e.to_string())?;
    nostr::execution::Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(secret, &peer),
        nonce: random_bytes(),
        created_at: now,
    }
    .event(kind, tags, value)
    .map_err(|e| format!("{e:?}"))
}

/// Open only the original authority's addressed artifact with the expected pin.
pub fn open_artifact(
    event: &Event,
    expected_authority: &str,
    expected: &Value,
    secret: &SecretKey,
) -> Result<Value> {
    let opened = nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
    let mut reference = parse_artifact(expected).map_err(|e| e.to_string())?;
    if let Some(declared) = reference.event.take()
        && (declared.id != event.id
            || declared.pubkey != event.pubkey
            || declared.kind != event.kind)
    {
        return Err("control artifact declaring event mismatch".into());
    }
    if opened.signer() != expected_authority || opened.artifact() != &reference {
        return Err("control artifact identity mismatch".into());
    }
    let bytes = opened
        .inline_bytes()
        .ok_or("control inline artifact unavailable")?;
    nostr::contracts::parse_strict(bytes).map_err(|e| e.to_string())
}
