use k256::schnorr::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use spacetimedb::{Identity, ReducerContext, Table};

const NOSTR_CLAIM_TTL_SECONDS: u64 = 300;
const NOSTR_CHALLENGE_DOMAIN: &str = "openagents:nostr-presence-bind:v1";
const DEFAULT_REGION: &str = "unknown";
const UNBOUND_SESSION_ID: &str = "unbound";

#[spacetimedb::table(name = "active_connection", public, accessor = active_connection)]
pub struct ActiveConnection {
    #[primary_key]
    pub identity: Identity,
    pub identity_hex: String,
    #[unique]
    pub node_id: String,
    pub session_id: String,
    pub worker_id: Option<String>,
    pub region: String,
    pub connected_at_unix_ms: u64,
    pub last_seen_unix_ms: u64,
    pub nostr_pubkey_hex: Option<String>,
    pub nostr_pubkey_npub: Option<String>,
    pub nostr_challenge_proof_sig: Option<String>,
    pub nostr_challenge_bound_at_unix_ms: Option<u64>,
}

#[spacetimedb::table(name = "nostr_presence_claim", public, accessor = nostr_presence_claim)]
pub struct NostrPresenceClaim {
    #[primary_key]
    pub node_id: String,
    pub identity: Identity,
    pub identity_hex: String,
    pub challenge: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub consumed: bool,
}

#[spacetimedb::table(name = "stream_head", public, accessor = stream_head)]
pub struct StreamHead {
    #[primary_key]
    pub stream_id: String,
    pub head_seq: u64,
    pub updated_at_unix_ms: u64,
}

#[spacetimedb::table(name = "sync_event", public, accessor = sync_event)]
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

#[spacetimedb::table(name = "stream_checkpoint", public, accessor = stream_checkpoint)]
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
    let identity_hex = ctx.sender().to_hex().to_string();
    if let Some(mut row) = ctx.db.active_connection().identity().find(ctx.sender()) {
        row.last_seen_unix_ms = now;
        if row.session_id == UNBOUND_SESSION_ID {
            row.identity_hex = identity_hex.clone();
            row.node_id = default_node_id(identity_hex.as_str());
            row.worker_id = None;
            row.region = DEFAULT_REGION.to_string();
        }
        ctx.db.active_connection().identity().update(row);
    } else {
        let row = ActiveConnection {
            identity: ctx.sender(),
            identity_hex: identity_hex.clone(),
            node_id: default_node_id(identity_hex.as_str()),
            session_id: UNBOUND_SESSION_ID.to_string(),
            worker_id: None,
            region: DEFAULT_REGION.to_string(),
            connected_at_unix_ms: now,
            last_seen_unix_ms: now,
            nostr_pubkey_hex: None,
            nostr_pubkey_npub: None,
            nostr_challenge_proof_sig: None,
            nostr_challenge_bound_at_unix_ms: None,
        };
        ctx.db.active_connection().insert(row);
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    if let Some(row) = ctx.db.active_connection().identity().find(ctx.sender()) {
        if row.session_id == UNBOUND_SESSION_ID {
            ctx.db.nostr_presence_claim().node_id().delete(row.node_id);
            ctx.db.active_connection().identity().delete(ctx.sender());
        }
    }
}

#[spacetimedb::reducer]
pub fn heartbeat(ctx: &ReducerContext, node_id: String) -> Result<(), String> {
    let node_id = normalize_required(node_id, "node_id")?;
    let mut row = ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender())
        .ok_or_else(|| "identity is not connected".to_string())?;
    if row.node_id != node_id {
        return Err("active connection node_id mismatch".to_string());
    }

    row.last_seen_unix_ms = now_unix_ms(ctx);
    ctx.db.active_connection().identity().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn request_nostr_presence_challenge(
    ctx: &ReducerContext,
    node_id: String,
    session_id: String,
    worker_id: String,
    region: String,
) -> Result<(), String> {
    let node_id = normalize_required(node_id, "node_id")?;
    let session_id = normalize_required(session_id, "session_id")?;
    let region = normalize_required(region, "region")?;
    let worker_id = normalize_optional(worker_id);

    let now = now_unix_ms(ctx);
    upsert_connection(
        ctx,
        node_id.clone(),
        session_id,
        worker_id,
        region,
        now,
    );

    let identity_hex = ctx.sender().to_hex().to_string();
    let challenge = format!(
        "{}:{}:{}:{}",
        NOSTR_CHALLENGE_DOMAIN, node_id, identity_hex, now
    );
    let claim = NostrPresenceClaim {
        node_id: node_id.clone(),
        identity: ctx.sender(),
        identity_hex,
        challenge,
        issued_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(NOSTR_CLAIM_TTL_SECONDS.saturating_mul(1000)),
        consumed: false,
    };

    if ctx
        .db
        .nostr_presence_claim()
        .node_id()
        .find(node_id.clone())
        .is_some()
    {
        ctx.db.nostr_presence_claim().node_id().update(claim);
    } else {
        ctx.db.nostr_presence_claim().insert(claim);
    }

    Ok(())
}

#[spacetimedb::reducer]
pub fn bind_nostr_presence_identity(
    ctx: &ReducerContext,
    node_id: String,
    nostr_pubkey_hex: String,
    nostr_pubkey_npub: String,
    challenge_signature_hex: String,
) -> Result<(), String> {
    let node_id = normalize_required(node_id, "node_id")?;
    let mut connection = ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender())
        .ok_or_else(|| "active connection missing".to_string())?;
    if connection.node_id != node_id {
        return Err("active connection node_id mismatch".to_string());
    }

    let mut claim = ctx
        .db
        .nostr_presence_claim()
        .node_id()
        .find(node_id.clone())
        .ok_or_else(|| "nostr claim challenge missing".to_string())?;
    if claim.identity != ctx.sender() {
        return Err("nostr claim identity mismatch".to_string());
    }

    let now = now_unix_ms(ctx);
    if claim.consumed {
        return Err("nostr claim challenge already consumed".to_string());
    }
    if now > claim.expires_at_unix_ms {
        return Err("nostr claim challenge expired".to_string());
    }

    let nostr_pubkey_hex = normalize_required(nostr_pubkey_hex, "nostr_pubkey_hex")?;
    let challenge_signature_hex =
        normalize_required(challenge_signature_hex, "challenge_signature_hex")?;
    verify_nostr_signature(
        nostr_pubkey_hex.as_str(),
        node_id.as_str(),
        claim.challenge.as_str(),
        challenge_signature_hex.as_str(),
    )?;

    connection.nostr_pubkey_hex = Some(nostr_pubkey_hex);
    connection.nostr_pubkey_npub = normalize_optional(nostr_pubkey_npub);
    connection.nostr_challenge_proof_sig = Some(challenge_signature_hex);
    connection.nostr_challenge_bound_at_unix_ms = Some(now);
    connection.last_seen_unix_ms = now;

    claim.consumed = true;

    ctx.db.active_connection().identity().update(connection);
    ctx.db.nostr_presence_claim().node_id().update(claim);

    Ok(())
}

#[spacetimedb::reducer]
pub fn register_offline(ctx: &ReducerContext, node_id: String) -> Result<(), String> {
    let node_id = normalize_required(node_id, "node_id")?;
    let connection = ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender())
        .ok_or_else(|| "active connection missing".to_string())?;
    if connection.node_id != node_id {
        return Err("active connection node_id mismatch".to_string());
    }

    ctx.db.active_connection().identity().delete(ctx.sender());
    ctx.db.nostr_presence_claim().node_id().delete(node_id);
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
) -> Result<(), String> {
    let stream_id = normalize_required(stream_id, "stream_id")?;
    let idempotency_key = normalize_required(idempotency_key, "idempotency_key")?;
    let payload_hash = normalize_required(payload_hash, "payload_hash")?;

    if let Some(existing) = ctx
        .db
        .sync_event()
        .idempotency_key()
        .find(idempotency_key.clone())
    {
        let _ = existing;
        return Ok(());
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

    Ok(())
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

fn upsert_connection(
    ctx: &ReducerContext,
    node_id: String,
    session_id: String,
    worker_id: Option<String>,
    region: String,
    now_unix_ms: u64,
) {
    let identity_hex = ctx.sender().to_hex().to_string();
    let row = if let Some(existing) = ctx.db.active_connection().identity().find(ctx.sender()) {
        ActiveConnection {
            identity: ctx.sender(),
            identity_hex,
            node_id,
            session_id,
            worker_id,
            region,
            connected_at_unix_ms: existing.connected_at_unix_ms,
            last_seen_unix_ms: now_unix_ms,
            nostr_pubkey_hex: existing.nostr_pubkey_hex,
            nostr_pubkey_npub: existing.nostr_pubkey_npub,
            nostr_challenge_proof_sig: existing.nostr_challenge_proof_sig,
            nostr_challenge_bound_at_unix_ms: existing.nostr_challenge_bound_at_unix_ms,
        }
    } else {
        ActiveConnection {
            identity: ctx.sender(),
            identity_hex,
            node_id,
            session_id,
            worker_id,
            region,
            connected_at_unix_ms: now_unix_ms,
            last_seen_unix_ms: now_unix_ms,
            nostr_pubkey_hex: None,
            nostr_pubkey_npub: None,
            nostr_challenge_proof_sig: None,
            nostr_challenge_bound_at_unix_ms: None,
        }
    };

    if ctx
        .db
        .active_connection()
        .identity()
        .find(ctx.sender())
        .is_some()
    {
        ctx.db.active_connection().identity().update(row);
    } else {
        ctx.db.active_connection().insert(row);
    }
}

fn normalize_required(value: String, field: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(format!("{} is required", field));
    }
    Ok(normalized)
}

fn normalize_optional(value: String) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn now_unix_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() as u64) / 1_000
}

fn default_node_id(identity_hex: &str) -> String {
    format!("spacetime:{identity_hex}")
}

fn verify_nostr_signature(
    pubkey_hex: &str,
    node_id: &str,
    challenge: &str,
    signature_hex: &str,
) -> Result<(), String> {
    let pubkey_bytes = hex::decode(pubkey_hex.trim())
        .map_err(|error| format!("invalid nostr pubkey hex: {error}"))?;
    let signature_bytes = hex::decode(signature_hex.trim())
        .map_err(|error| format!("invalid signature hex: {error}"))?;
    let signature = Signature::try_from(signature_bytes.as_slice())
        .map_err(|error| format!("invalid schnorr signature: {error}"))?;
    let verifying_key = VerifyingKey::from_bytes(pubkey_bytes.as_slice())
        .map_err(|error| format!("invalid nostr pubkey: {error}"))?;
    let digest = challenge_digest(node_id, challenge);
    verifying_key
        .verify_raw(&digest, &signature)
        .map_err(|error| format!("nostr challenge signature verification failed: {error}"))
}

fn challenge_digest(node_id: &str, challenge: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NOSTR_CHALLENGE_DOMAIN.as_bytes());
    hasher.update(b":");
    hasher.update(node_id.as_bytes());
    hasher.update(b":");
    hasher.update(challenge.as_bytes());
    hasher.finalize().into()
}
