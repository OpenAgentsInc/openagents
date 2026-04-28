use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, header};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tower_http::cors::{Any, CorsLayer};

use crate::challenge::{
    CHALLENGE_DOMAIN, CHALLENGE_TTL_SECS, ChallengeStore, ChallengeView, PROOF_EVENT_KIND,
    verify_proof_event,
};
use crate::error::RegistrarError;
use crate::store::Store;
use crate::validation::{resolve_pubkey, validate_handle};

const CLAIM_HTML: &str = include_str!("../web/claim.html");

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub admin_token: Arc<String>,
    pub challenges: Arc<ChallengeStore>,
}

impl AppState {
    pub fn new(store: Arc<Store>, admin_token: String) -> Self {
        Self {
            store,
            admin_token: Arc::new(admin_token),
            challenges: Arc::new(ChallengeStore::new()),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let public_cors = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_headers(Any)
        .allow_origin(Any);

    let public = Router::new()
        .route("/.well-known/nostr.json", get(get_nostr_json))
        .route("/healthz", get(healthz))
        .layer(public_cors);

    // Public proof-of-control endpoints.
    let claim_public = Router::new()
        .route("/claim", get(claim_page))
        .route("/claim/challenge", post(post_challenge))
        .route("/claim/complete", post(post_complete));

    let admin = Router::new()
        .route("/admin/claim", post(post_admin_claim))
        .route("/admin/claim/{name}", delete(delete_claim));

    public.merge(claim_public).merge(admin).with_state(state)
}

#[derive(Debug, Deserialize)]
struct NameQuery {
    name: Option<String>,
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn claim_page() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline'; \
             style-src 'self' 'unsafe-inline'; \
             connect-src 'self'; \
             img-src 'self' data:; \
             frame-ancestors 'none'; \
             base-uri 'none'; \
             form-action 'self'",
        ),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    (StatusCode::OK, headers, CLAIM_HTML)
}

async fn get_nostr_json(
    State(state): State<AppState>,
    Query(query): Query<NameQuery>,
) -> Result<impl IntoResponse, RegistrarError> {
    let snapshot = state.store.snapshot()?;
    let body = if let Some(name) = query.name {
        let mut filtered = snapshot.clone();
        filtered.names.retain(|key, _| key == &name);
        let kept_pubkeys: std::collections::BTreeSet<String> =
            filtered.names.values().cloned().collect();
        filtered.relays.retain(|pk, _| kept_pubkeys.contains(pk));
        filtered
    } else {
        snapshot
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    // 60s public cache: tolerable freshness for verifiers, low load on
    // the registrar. `must-revalidate` keeps stale snapshots from
    // sticking after a delete.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, must-revalidate"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    Ok((StatusCode::OK, headers, Json(body)))
}

#[derive(Debug, Deserialize)]
struct ChallengeRequest {
    name: String,
    #[serde(default)]
    npub: Option<String>,
    #[serde(default)]
    pubkey: Option<String>,
}

async fn post_challenge(
    State(state): State<AppState>,
    body: Result<Json<ChallengeRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, RegistrarError> {
    let Json(payload) = body.map_err(|_| RegistrarError::BadRequest)?;
    let name = validate_handle(&payload.name)?;
    let pubkey_hex = resolve_pubkey(payload.npub.as_deref(), payload.pubkey.as_deref())?;

    if state.store.is_reserved(&name) {
        tracing::info!(
            event = "claim_challenge_rejected",
            reason = "reserved_handle",
            handle = %name,
            pubkey = %pubkey_hex,
            "challenge refused"
        );
        return Err(RegistrarError::ReservedHandle);
    }
    if state.store.has_handle(&name)? {
        tracing::info!(
            event = "claim_challenge_rejected",
            reason = "handle_taken",
            handle = %name,
            pubkey = %pubkey_hex,
            "challenge refused"
        );
        return Err(RegistrarError::HandleTaken);
    }
    if state.store.has_pubkey_other_than(&pubkey_hex, &name)? {
        tracing::info!(
            event = "claim_challenge_rejected",
            reason = "pubkey_taken",
            handle = %name,
            pubkey = %pubkey_hex,
            "challenge refused"
        );
        return Err(RegistrarError::PubkeyTaken);
    }

    let record = state
        .challenges
        .issue(&name, &pubkey_hex, CHALLENGE_TTL_SECS)?;
    tracing::info!(
        event = "claim_challenge_issued",
        challenge_id = %record.id,
        handle = %record.name,
        pubkey = %record.pubkey_hex,
        expires_at = record.expires_at,
        "challenge issued"
    );

    let view = ChallengeView {
        challenge_id: record.id.clone(),
        otp: record.otp.clone(),
        nonce: record.nonce.clone(),
        message: record.canonical_message(),
        expires_at: record.expires_at,
        domain: CHALLENGE_DOMAIN,
        kind: PROOF_EVENT_KIND,
    };
    Ok((StatusCode::CREATED, Json(view)))
}

#[derive(Debug, Deserialize)]
struct CompleteRequest {
    challenge_id: String,
    event: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ClaimResponse {
    name: String,
    pubkey: String,
}

async fn post_complete(
    State(state): State<AppState>,
    body: Result<Json<CompleteRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, RegistrarError> {
    let Json(payload) = body.map_err(|_| RegistrarError::BadRequest)?;
    let record = state.challenges.take(&payload.challenge_id)?;
    if let Err(err) = verify_proof_event(&record, &payload.event) {
        tracing::warn!(
            event = "claim_complete_rejected",
            challenge_id = %record.id,
            handle = %record.name,
            pubkey = %record.pubkey_hex,
            reason = %err,
            "proof verification failed"
        );
        return Err(err);
    }
    state.store.claim(&record.name, &record.pubkey_hex)?;
    tracing::info!(
        event = "claim_complete_succeeded",
        challenge_id = %record.id,
        handle = %record.name,
        pubkey = %record.pubkey_hex,
        "claim recorded"
    );
    let response = ClaimResponse {
        name: record.name,
        pubkey: record.pubkey_hex,
    };
    Ok((StatusCode::CREATED, Json(response)))
}

#[derive(Debug, Deserialize)]
struct AdminClaimRequest {
    name: String,
    #[serde(default)]
    npub: Option<String>,
    #[serde(default)]
    pubkey: Option<String>,
    /// Operator must opt in explicitly to bypass the proof-of-control
    /// flow. Default is `false`. Intended for one-shot bootstrap of
    /// officially-managed reserved handles (e.g. seeding the `agent`
    /// handle to the OpenAgents-controlled key on a fresh deploy).
    #[serde(default)]
    operator_override: bool,
}

async fn post_admin_claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<AdminClaimRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, RegistrarError> {
    require_admin(&headers, &state.admin_token)?;
    let Json(payload) = body.map_err(|_| RegistrarError::BadRequest)?;

    let name = validate_handle(&payload.name)?;
    let pubkey_hex = resolve_pubkey(payload.npub.as_deref(), payload.pubkey.as_deref())?;

    if !payload.operator_override {
        // Without `operator_override`, an admin claim is just an admin
        // bypass of the proof flow — exactly the thing review flagged.
        // Force the caller to either go through /claim/challenge or to
        // explicitly mark this as a documented operator override.
        tracing::warn!(
            event = "claim_admin_rejected",
            handle = %name,
            pubkey = %pubkey_hex,
            reason = "missing_operator_override",
            "admin claim refused; use /claim/challenge or pass operator_override=true"
        );
        return Err(RegistrarError::ChallengeInvalid(
            "operator_override required to skip proof flow",
        ));
    }

    state.store.claim(&name, &pubkey_hex)?;
    tracing::warn!(
        event = "claim_admin_override",
        handle = %name,
        pubkey = %pubkey_hex,
        "admin override claim recorded — emergency seed path"
    );
    let response = ClaimResponse {
        name,
        pubkey: pubkey_hex,
    };
    Ok((StatusCode::CREATED, Json(response)))
}

async fn delete_claim(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, RegistrarError> {
    require_admin(&headers, &state.admin_token)?;
    let name = validate_handle(&name)?;
    state.store.delete(&name)?;
    tracing::info!(event = "claim_deleted", handle = %name, "delete recorded");
    Ok(StatusCode::NO_CONTENT)
}

fn require_admin(headers: &HeaderMap, expected: &str) -> Result<(), RegistrarError> {
    let value: &HeaderValue = headers
        .get(header::AUTHORIZATION)
        .ok_or(RegistrarError::Unauthorized)?;
    let raw = value.to_str().map_err(|_| RegistrarError::Unauthorized)?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .ok_or(RegistrarError::Unauthorized)?
        .trim();
    if token.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1 {
        return Err(RegistrarError::Unauthorized);
    }
    Ok(())
}
