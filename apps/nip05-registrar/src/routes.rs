use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::error::RegistrarError;
use crate::store::Store;
use crate::validation::{decode_npub_to_hex, is_valid_hex_pubkey, validate_handle};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub admin_token: Arc<String>,
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

    let admin = Router::new()
        .route("/admin/claim", post(post_claim))
        .route("/admin/claim/{name}", delete(delete_claim));

    public.merge(admin).with_state(state)
}

#[derive(Debug, Deserialize)]
struct NameQuery {
    name: Option<String>,
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn get_nostr_json(
    State(state): State<AppState>,
    Query(query): Query<NameQuery>,
) -> Result<impl IntoResponse, RegistrarError> {
    let snapshot = state.store.snapshot()?;
    let body = if let Some(name) = query.name {
        let mut filtered = snapshot.clone();
        filtered.names.retain(|key, _| key == &name);
        // Also filter relays mapping to only keep entries whose pubkey survived.
        let kept_pubkeys: std::collections::BTreeSet<String> =
            filtered.names.values().cloned().collect();
        filtered.relays.retain(|pk, _| kept_pubkeys.contains(pk));
        filtered
    } else {
        snapshot
    };
    Ok(Json(body))
}

#[derive(Debug, Deserialize)]
struct ClaimRequest {
    name: String,
    /// Either an npub1... bech32 string or a 64-char lowercase hex pubkey.
    #[serde(default)]
    npub: Option<String>,
    #[serde(default)]
    pubkey: Option<String>,
}

#[derive(Debug, Serialize)]
struct ClaimResponse {
    name: String,
    pubkey: String,
}

async fn post_claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<ClaimRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, RegistrarError> {
    require_admin(&headers, &state.admin_token)?;
    let Json(payload) = body.map_err(|_| RegistrarError::BadRequest)?;

    let name = validate_handle(&payload.name)?;
    let pubkey_hex = match (payload.npub.as_deref(), payload.pubkey.as_deref()) {
        (Some(npub), _) if !npub.trim().is_empty() => decode_npub_to_hex(npub)?,
        (_, Some(hex)) if !hex.trim().is_empty() => {
            let lowered = hex.trim().to_ascii_lowercase();
            if is_valid_hex_pubkey(&lowered) {
                lowered
            } else {
                return Err(RegistrarError::InvalidNpub);
            }
        }
        _ => return Err(RegistrarError::InvalidNpub),
    };

    state.store.claim(&name, &pubkey_hex)?;

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
    if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
        return Err(RegistrarError::Unauthorized);
    }
    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
