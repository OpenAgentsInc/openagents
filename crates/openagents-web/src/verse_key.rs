//! A browser's Verse key, owned by the person's own key (#12313).
//!
//! The browser Grid signs its presence about ten times a second with a key
//! the page keeps; a browser extension can't sign that often. So the
//! person's phone or computer, which hold their key, sign a NIP-OA `auth`
//! tag for the browser's key once, and the browser carries it on its
//! presence states (NIP-MV `33301`). Readers then show the avatar as the
//! person's.
//!
//! - `POST /v1/verse/key` `{pubkey}`, from `/verse`: this account's browser
//!   Verse key, waiting for its owner tag. A new key replaces the old.
//! - `GET /v1/verse/key`: the key, the conditions and exact preimage to
//!   sign, and the tag once signed.
//! - `PUT /v1/verse/key` `{auth}`, from the app: the tag its key signed.
//!   The owner must be one of the account's keys.
//!
//! Every route acts as the signed-in account (the page reaches it with the
//! sign-in cookie).

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use nostr::agent_public;
use nostr::domain::Tag;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::App;
use crate::chat_store::{Error, Store};
use crate::coder_sync::{answer, refused, stored};

/// How long an owner tag for a Verse key stands: thirty days.
pub(crate) const LIFETIME: u64 = 30 * 24 * 60 * 60;

pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/verse/key"
}

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/v1/verse/key", get(read).post(offer).put(own))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Held {
    pubkey: String,
    #[serde(default)]
    conditions: String,
    #[serde(default)]
    auth: Option<Vec<String>>,
}

fn key(owner: &str) -> Result<String, Error> {
    Store::owner_key(owner, "verse-key.json")
}

async fn load(store: &Store, owner: &str) -> Result<(Option<Held>, Option<String>), Error> {
    match store.read_key(&key(owner)?).await? {
        Some((bytes, generation)) => Ok((serde_json::from_slice(&bytes).ok(), Some(generation))),
        None => Ok((None, None)),
    }
}

async fn save(
    store: &Store,
    owner: &str,
    held: &Held,
    generation: Option<&str>,
) -> Result<(), Error> {
    let bytes =
        serde_json::to_vec(held).map_err(|_| Error::Invalid("The key could not be saved."))?;
    store
        .write_key(&key(owner)?, bytes, generation)
        .await
        .map(|_| ())
}

fn now() -> u64 {
    crate::cloud::session::now()
}

fn view(held: &Held) -> serde_json::Value {
    json!({
        "pubkey": held.pubkey,
        "conditions": held.conditions,
        "preimage": format!("nostr:agent-auth:{}:{}", held.pubkey, held.conditions),
        "auth": held.auth,
    })
}

async fn who(app: &App, headers: &HeaderMap) -> Result<(String, String), Response> {
    let account = crate::coder_sync::account(app, headers).await?;
    Ok((account.clone(), crate::chat_store::account_owner(&account)))
}

async fn read(State(app): State<App>, headers: HeaderMap) -> Response {
    let (_, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((Some(held), _)) => answer(StatusCode::OK, view(&held)),
        Ok((None, _)) => refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No browser has asked yet.",
        ),
        Err(e) => stored(&e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Offer {
    pubkey: String,
}

async fn offer(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Offer>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (_, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    let Ok(Json(offer)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send the browser's key.",
        );
    };
    if !agent_public::valid_id(&offer.pubkey) {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "That isn't a Nostr key.",
        );
    }
    let store = &app.config.chat_store;
    let (held, generation) = match load(store, &owner).await {
        Ok(found) => found,
        Err(e) => return stored(&e),
    };
    // The same key keeps its tag; a new one starts over.
    if let Some(held) = held.filter(|h| h.pubkey == offer.pubkey) {
        return answer(StatusCode::OK, view(&held));
    }
    let held = Held {
        pubkey: offer.pubkey,
        conditions: agent_public::conditions_until(now() + LIFETIME),
        auth: None,
    };
    match save(store, &owner, &held, generation.as_deref()).await {
        Ok(()) => answer(StatusCode::OK, view(&held)),
        Err(e) => stored(&e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Own {
    auth: Vec<String>,
}

/// The account's Nostr keys, from the account service, under the caller's
/// own bearer.
async fn account_keys(app: &App, headers: &HeaderMap) -> Result<Vec<String>, Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        ));
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    match service
        .nostr_call(reqwest::Method::GET, "/v1/account/keys", Some(bearer), None)
        .await
    {
        Ok(found) if found.status == 200 => Ok(found.body["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| k["pubkey"].as_str().map(str::to_string))
            .collect()),
        _ => Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Try again later.",
        )),
    }
}

async fn own(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Own>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (_, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    let Ok(Json(body)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send the tag your key signed.",
        );
    };
    let store = &app.config.chat_store;
    let (held, generation) = match load(store, &owner).await {
        Ok(found) => found,
        Err(e) => return stored(&e),
    };
    let Some(mut held) = held else {
        return refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No browser has asked yet.",
        );
    };
    let tag = Tag::new(body.auth.clone());
    let Ok(signer) = agent_public::check_owner_tag_for(&held.pubkey, &tag, now()) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "That tag doesn't check out for the browser's key.",
        );
    };
    let keys = match account_keys(&app, &headers).await {
        Ok(keys) => keys,
        Err(r) => return r,
    };
    if !keys.contains(&signer) {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Sign with a key that's on your account.",
        );
    }
    held.auth = Some(body.auth);
    match save(store, &owner, &held, generation.as_deref()).await {
        Ok(()) => answer(StatusCode::OK, view(&held)),
        Err(e) => stored(&e),
    }
}
