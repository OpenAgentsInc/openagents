use core::str::FromStr;

use secp256k1::{Message, Secp256k1, XOnlyPublicKey, schnorr::Signature};
use sha2::{Digest, Sha256};
use spacetimedb::{Identity, ReducerContext, Table};

const NOSTR_CLAIM_TTL_SECONDS: u64 = 300;
const NOSTR_CHALLENGE_DOMAIN: &str = "openagents:nostr-presence-bind:v1";

#[spacetimedb::table(name = active_connection, public)]
pub struct ActiveConnection {
    #[primary_key]
    pub identity: Identity,
    pub identity_hex: String,
    pub connected_at_unix_ms: u64,
    pub last_seen_unix_ms: u64,
    pub nostr_pubkey_hex: Option<String>,
    pub nostr_pubkey_npub: Option<String>,
    pub nostr_challenge_proof_sig: Option<String>,
    pub nostr_challenge_bound_at_unix_ms: Option<u64>,
}

#[spacetimedb::table(name = nostr_presence_claim, public)]
pub struct NostrPresenceClaim {
    #[primary_key]
    pub identity: Identity,
    pub challenge: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub consumed: bool,
}

#[spacetimedb::table(name = stream_head, public)]
pub struct StreamHead {
    #[primary_key]
    pub stream_id: String,
    pub head_seq: u64,
    pub updated_at_unix_ms: u64,
}

#[spacetimedb::table(name = sync_event, public)]
pub struct SyncEvent {
    #[primary_key]
    pub idempotency_key: String,
    pub stream_id: String,
    pub seq: u64,
    pub payload_hash: String,
    pub payload_json: String,
    pub committed_at_unix_ms: u64,
    pub durable_offset: u64,
    pub confirmed_read: bool,
}

#[spacetimedb::table(name = stream_checkpoint, public)]
pub struct StreamCheckpoint {
    #[primary_key]
    pub checkpoint_key: String,
    pub client_id: String,
    pub stream_id: String,
    pub last_applied_seq: u64,
    pub durable_offset: u64,
    pub updated_at_unix_ms: u64,
}

#[spacetimedb::reducer(init)]
pub fn init(_ctx: &ReducerContext) {
    log::info!("autopilot-sync module initialized");
}

#[spacetimedb::reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    let now = now_unix_ms(ctx);
    let identity_hex = ctx.sender.to_hex();
    let row = ActiveConnection {
        identity: ctx.sender,
        identity_hex,
        connected_at_unix_ms: now,
        last_seen_unix_ms: now,
        nostr_pubkey_hex: None,
        nostr_pubkey_npub: None,
        nostr_challenge_proof_sig: None,
        nostr_challenge_bound_at_unix_ms: None,
    };

    if ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender)
        .is_some()
    {
        ctx.db.active_connection().identity().update(row);
    } else {
        ctx.db.active_connection().insert(row);
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    ctx.db.active_connection().identity().delete(ctx.sender);
    ctx.db.nostr_presence_claim().identity().delete(ctx.sender);
}

#[spacetimedb::reducer]
pub fn heartbeat(ctx: &ReducerContext) {
    let Some(mut row) = ctx.db.active_connection().identity().find(ctx.sender) else {
        return;
    };

    row.last_seen_unix_ms = now_unix_ms(ctx);
    ctx.db.active_connection().identity().update(row);
}

#[spacetimedb::reducer]
pub fn request_nostr_presence_challenge(ctx: &ReducerContext) -> Result<String, String> {
    ensure_connected(ctx)?;

    let now = now_unix_ms(ctx);
    let challenge = format!(
        "{}:{}:{}",
        NOSTR_CHALLENGE_DOMAIN,
        ctx.sender.to_hex(),
        now
    );
    let claim = NostrPresenceClaim {
        identity: ctx.sender,
        challenge: challenge.clone(),
        issued_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(NOSTR_CLAIM_TTL_SECONDS.saturating_mul(1000)),
        consumed: false,
    };

    if ctx
        .db
        .nostr_presence_claim()
        .identity()
        .find(ctx.sender)
        .is_some()
    {
        ctx.db.nostr_presence_claim().identity().update(claim);
    } else {
        ctx.db.nostr_presence_claim().insert(claim);
    }

    Ok(challenge)
}

#[spacetimedb::reducer]
pub fn bind_nostr_presence_identity(
    ctx: &ReducerContext,
    nostr_pubkey_hex: String,
    nostr_pubkey_npub: String,
    challenge: String,
    challenge_signature_hex: String,
) -> Result<(), String> {
    ensure_connected(ctx)?;

    let mut connection = ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender)
        .ok_or_else(|| "active connection missing".to_string())?;

    let mut claim = ctx
        .db
        .nostr_presence_claim()
        .identity()
        .find(ctx.sender)
        .ok_or_else(|| "nostr claim challenge missing".to_string())?;

    let now = now_unix_ms(ctx);
    if claim.consumed {
        return Err("nostr claim challenge already consumed".to_string());
    }
    if now > claim.expires_at_unix_ms {
        return Err("nostr claim challenge expired".to_string());
    }

    let normalized_challenge = challenge.trim();
    if normalized_challenge.is_empty() || normalized_challenge != claim.challenge {
        return Err("nostr claim challenge mismatch".to_string());
    }

    verify_nostr_signature(
        &ctx.sender,
        nostr_pubkey_hex.as_str(),
        normalized_challenge,
        challenge_signature_hex.as_str(),
    )?;

    connection.nostr_pubkey_hex = Some(nostr_pubkey_hex.trim().to_string());
    connection.nostr_pubkey_npub = Some(nostr_pubkey_npub.trim().to_string());
    connection.nostr_challenge_proof_sig = Some(challenge_signature_hex.trim().to_string());
    connection.nostr_challenge_bound_at_unix_ms = Some(now);
    connection.last_seen_unix_ms = now;

    claim.consumed = true;

    ctx.db.active_connection().identity().update(connection);
    ctx.db.nostr_presence_claim().identity().update(claim);

    Ok(())
}

#[spacetimedb::reducer]
pub fn append_sync_event(
    ctx: &ReducerContext,
    stream_id: String,
    idempotency_key: String,
    payload_hash: String,
    payload_json: String,
    committed_at_unix_ms: u64,
    durable_offset: u64,
    confirmed_read: bool,
    expected_next_seq: u64,
) -> Result<u64, String> {
    let stream_id = normalize_required(stream_id, "stream_id")?;
    let idempotency_key = normalize_required(idempotency_key, "idempotency_key")?;
    let payload_hash = normalize_required(payload_hash, "payload_hash")?;

    if let Some(existing) = ctx
        .db
        .sync_event()
        .idempotency_key()
        .find(idempotency_key.clone())
    {
        return Ok(existing.seq);
    }

    let now = now_unix_ms(ctx);
    let current_head = ctx
        .db
        .stream_head()
        .stream_id()
        .find(stream_id.clone())
        .map(|row| row.head_seq)
        .unwrap_or(0);
    let next_seq = current_head.saturating_add(1);

    if expected_next_seq > 0 && expected_next_seq != next_seq {
        return Err(format!(
            "sequence_conflict expected_next_seq={} actual_next_seq={}",
            expected_next_seq, next_seq
        ));
    }

    let event = SyncEvent {
        idempotency_key,
        stream_id: stream_id.clone(),
        seq: next_seq,
        payload_hash,
        payload_json,
        committed_at_unix_ms,
        durable_offset,
        confirmed_read,
    };

    ctx.db.sync_event().insert(event);

    let head = StreamHead {
        stream_id: stream_id.clone(),
        head_seq: next_seq,
        updated_at_unix_ms: now,
    };
    if ctx
        .db
        .stream_head()
        .stream_id()
        .find(stream_id.clone())
        .is_some()
    {
        ctx.db.stream_head().stream_id().update(head);
    } else {
        ctx.db.stream_head().insert(head);
    }

    Ok(next_seq)
}

#[spacetimedb::reducer]
pub fn ack_stream_checkpoint(
    ctx: &ReducerContext,
    client_id: String,
    stream_id: String,
    last_applied_seq: u64,
    durable_offset: u64,
) -> Result<(), String> {
    let client_id = normalize_required(client_id, "client_id")?;
    let stream_id = normalize_required(stream_id, "stream_id")?;
    let checkpoint_key = format!("{}:{}", client_id, stream_id);

    let row = StreamCheckpoint {
        checkpoint_key: checkpoint_key.clone(),
        client_id,
        stream_id,
        last_applied_seq,
        durable_offset,
        updated_at_unix_ms: now_unix_ms(ctx),
    };

    if ctx
        .db
        .stream_checkpoint()
        .checkpoint_key()
        .find(checkpoint_key.clone())
        .is_some()
    {
        ctx.db.stream_checkpoint().checkpoint_key().update(row);
    } else {
        ctx.db.stream_checkpoint().insert(row);
    }

    Ok(())
}

fn ensure_connected(ctx: &ReducerContext) -> Result<(), String> {
    if ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender)
        .is_none()
    {
        return Err("identity is not connected".to_string());
    }
    Ok(())
}

fn normalize_required(value: String, field: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(format!("{} is required", field));
    }
    Ok(normalized)
}

fn now_unix_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() as u64) / 1_000
}

fn verify_nostr_signature(
    identity: &Identity,
    pubkey_hex: &str,
    challenge: &str,
    signature_hex: &str,
) -> Result<(), String> {
    let normalized_pubkey = pubkey_hex.trim();
    if normalized_pubkey.is_empty() {
        return Err("nostr_pubkey_hex is required".to_string());
    }

    let normalized_signature = signature_hex.trim();
    if normalized_signature.is_empty() {
        return Err("challenge_signature_hex is required".to_string());
    }

    let pubkey = XOnlyPublicKey::from_str(normalized_pubkey)
        .map_err(|error| format!("invalid nostr pubkey: {error}"))?;

    let signature_bytes =
        hex::decode(normalized_signature).map_err(|error| format!("invalid signature hex: {error}"))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|error| format!("invalid schnorr signature bytes: {error}"))?;

    let digest = challenge_digest(identity, challenge);
    let message = Message::from_digest(digest);
    let secp = Secp256k1::verification_only();
    secp.verify_schnorr(&signature, &message, &pubkey)
        .map_err(|error| format!("nostr challenge signature verification failed: {error}"))
}

fn challenge_digest(identity: &Identity, challenge: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NOSTR_CHALLENGE_DOMAIN.as_bytes());
    hasher.update(b":");
    hasher.update(identity.to_hex().as_bytes());
    hasher.update(b":");
    hasher.update(challenge.as_bytes());
    hasher.finalize().into()
}
