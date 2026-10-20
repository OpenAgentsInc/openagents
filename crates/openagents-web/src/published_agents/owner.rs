//! A published agent's own Nostr key and its owner (#12255, NIP-AGENT
//! "Public agents").
//!
//! Each published agent gets a key of its own. This site holds it: the
//! secret is derived from a seed only this site holds
//! (`Config::agent_key_seed`) and the published ID, so nothing is stored
//! and nothing published reveals it. The owner proves ownership with a
//! NIP-OA `auth` tag their own client signs; this site never holds the
//! owner's secret.
//!
//! - `GET /v1/published-agents/{id}/owner`: the agent's key, what the owner
//!   signs (`nostr:agent-auth:<agent>:created_at<T>`), and the owner once
//!   set. Anyone may read it.
//! - `PUT /v1/published-agents/{id}/owner` `{"auth": ["auth", owner,
//!   conditions, sig]}`, as the publisher: checks the tag for this agent
//!   key, that its owner is one of the publisher's account keys, then
//!   publishes the agent's kind 0 and its public definition (kind 30187).
//! - `DELETE /v1/published-agents/{id}/owner`, as the publisher: retires the
//!   agent, publishing the revoked definition.
//!
//! A new revision of an owned agent republishes its definition.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use hmac::{Hmac, Mac};
use nostr::agent_public::{self, Profile};
use nostr::domain::{RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;

use super::{Record, Refusal, key, load};
use crate::App;
use crate::chat_store::Error;
use crate::coder_sync::{answer, refused, stored};

/// The owner a published agent carries.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Owned {
    /// The owner's `auth` tag as signed.
    pub(crate) tag: Vec<String>,
    /// The owner's key.
    pub(crate) pubkey: String,
    /// Whether the agent was retired.
    #[serde(default)]
    pub(crate) revoked: bool,
    /// When its events were last signed, so a retirement always comes after.
    #[serde(default)]
    pub(crate) signed_at: u64,
    /// Who may start a private conversation with it (#12256); the owner
    /// when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) messages: Option<super::social::MessagesFrom>,
    #[serde(default, skip_serializing_if = "agent_public::Runtime::is_empty")]
    pub(crate) runtime: agent_public::Runtime,
}

/// Who may message the agent.
pub(crate) fn messages_from(owned: &Owned) -> super::social::MessagesFrom {
    owned.messages.unwrap_or_default()
}

/// The agent key for published agent `id`, as a signer and a secret.
pub(crate) fn keys(app: &App, id: &str) -> Option<(RelaySigner, secp256k1::SecretKey)> {
    let hex = secret_hex(app, id)?;
    let secret: secp256k1::SecretKey = hex.parse().ok()?;
    Some((RelaySigner::from_secret_hex(&hex).ok()?, secret))
}

/// The agent key for published agent `id`, when this site has a seed.
pub(crate) fn signer(app: &App, id: &str) -> Option<RelaySigner> {
    RelaySigner::from_secret_hex(&secret_hex(app, id)?).ok()
}

fn secret_hex(app: &App, id: &str) -> Option<String> {
    let seed = app.config.agent_key_seed.as_ref()?;
    for round in 0u8..8 {
        let mut mac = Hmac::<Sha256>::new_from_slice(seed).ok()?;
        mac.update(b"openagents.agent-key.v1\0");
        mac.update(id.as_bytes());
        mac.update(&[round]);
        let bytes = mac.finalize().into_bytes();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        if RelaySigner::from_secret_hex(&hex).is_ok() {
            return Some(hex);
        }
    }
    None
}

/// Where a published agent's definition is served.
pub(crate) fn definition_url(app: &App, id: &str) -> String {
    format!(
        "{}/agent/{id}/definition.json",
        crate::wellknown::origin(app)
    )
}

/// The index from an agent key to its published ID, so `/agent/{npub}`
/// finds it.
fn index_key(pubkey: &str) -> String {
    format!("published-agents/keys/{pubkey}.json")
}

/// The published ID an agent key belongs to.
pub(crate) async fn id_of_key(app: &App, pubkey: &str) -> Option<String> {
    let (bytes, _) = app
        .config
        .chat_store
        .read_key(&index_key(pubkey))
        .await
        .ok()??;
    let id: String = serde_json::from_slice(&bytes).ok()?;
    agent_public::valid_id(&id).then_some(id)
}

fn now() -> u64 {
    crate::cloud::session::now()
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Agent keys aren't set up here.",
    )
}

/// The definition document for a record's newest revision.
pub(super) fn document(record: &Record) -> Option<agent_graph::definition::Definition> {
    let newest = record.revisions.last()?;
    Some(agent_graph::definition::of(
        &newest.template,
        &newest.id,
        &newest.id,
        &agent_graph::lab::Notes::default(),
    ))
}

/// Publish `events` to the profile relays; how many relays took them all.
pub(super) async fn send(app: &App, events: &[nostr::domain::Event]) -> usize {
    let relays = &app.config.profile_relays;
    let mut took = 0;
    for relay in relays {
        let mut all = true;
        for event in events {
            if crate::profiles::relays::publish(relay, event)
                .await
                .is_err()
            {
                all = false;
                break;
            }
        }
        took += usize::from(all);
    }
    for event in events {
        crate::profiles::forget(relays, &event.pubkey);
        if let Some(owner) = event.tag_values("p").next() {
            crate::profiles::forget(relays, owner);
        }
    }
    took
}

/// The agent's kind 0 and definition for `record`, signed now.
pub(super) fn events(
    app: &App,
    id: &str,
    record: &Record,
    owned: &Owned,
) -> Option<Vec<nostr::domain::Event>> {
    let agent = signer(app, id)?;
    let tag = Tag::new(owned.tag.clone());
    let url = definition_url(app, id);
    let at = now().max(owned.signed_at);
    if owned.revoked {
        let gone = agent_public::revoked_event(&agent, &tag, &owned.pubkey, id, &url, at).ok()?;
        return Some(vec![gone]);
    }
    let newest = record.revisions.last()?;
    let spec = &newest.definition.spec;
    let profile = agent_public::profile_event(
        &agent,
        &tag,
        &Profile {
            name: spec.name.clone(),
            about: spec.description.clone(),
            website: url.clone(),
            picture: None,
        },
        at,
    );
    let content = serde_json::to_string(&document(record)?).ok()?;
    let definition = agent_public::definition_with_runtime(
        &agent,
        &tag,
        &owned.pubkey,
        id,
        &url,
        &content,
        &owned.runtime,
        at,
    )
    .ok()?;
    Some(vec![profile, definition])
}

/// After a new revision: republish an owned, live agent's definition.
pub(super) async fn republish(app: &App, id: &str) {
    if let Ok(Some((record, _))) = load(&app.config.chat_store, id).await
        && let Some(owned) = record.owner.as_ref().filter(|o| !o.revoked)
        && let Some(events) = events(app, id, &record, owned)
    {
        send(app, &events).await;
    }
}

pub(super) async fn read(State(app): State<App>, Path(id): Path<String>) -> Response {
    if !agent_public::valid_id(&id) {
        return Refusal::Missing.response();
    }
    let Some(agent) = signer(&app, &id) else {
        return unavailable();
    };
    let record = match load(&app.config.chat_store, &id).await {
        Ok(Some((record, _))) => record,
        Ok(None) => return Refusal::Missing.response(),
        Err(e) => return stored(&e),
    };
    let until = now() + 365 * 24 * 60 * 60;
    let conditions = agent_public::conditions_until(until);
    answer(
        StatusCode::OK,
        json!({
            "agent": agent.pubkey(),
            "npub": crate::profiles::npub(agent.pubkey()),
            "conditions": conditions,
            "preimage": format!("nostr:agent-auth:{}:{conditions}", agent.pubkey()),
            "owner": record.owner.as_ref().map(|o| o.pubkey.clone()),
            "revoked": record.owner.as_ref().is_some_and(|o| o.revoked),
        }),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SetOwner {
    auth: Vec<String>,
}

/// The publisher's Nostr keys, from the account service, under the
/// caller's own bearer.
async fn publisher_keys(app: &App, headers: &HeaderMap) -> Result<Vec<String>, Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(unavailable());
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

/// Change the record's owner under `change`, as its publisher.
async fn change_owner(
    app: &App,
    publisher: &str,
    id: &str,
    change: impl Fn(&mut Record) -> Result<Owned, Refusal>,
) -> Result<Result<(Record, Owned), Refusal>, Error> {
    let store = &app.config.chat_store;
    for _ in 0..16 {
        let Some((mut record, generation)) = load(store, id).await? else {
            return Ok(Err(Refusal::Missing));
        };
        if record.publisher != publisher {
            return Ok(Err(Refusal::Denied));
        }
        let owned = match change(&mut record) {
            Ok(owned) => owned,
            Err(refusal) => return Ok(Err(refusal)),
        };
        record.owner = Some(owned.clone());
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| Error::Invalid("The agent could not be saved."))?;
        match store.write_key(&key(id), bytes, Some(&generation)).await {
            Ok(_) => return Ok(Ok((record, owned))),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

async fn write_index(app: &App, pubkey: &str, id: &str) {
    let store = &app.config.chat_store;
    let key = index_key(pubkey);
    let generation = store.read_key(&key).await.ok().flatten().map(|(_, g)| g);
    let bytes = serde_json::to_vec(id).unwrap_or_default();
    let _ = store.write_key(&key, bytes, generation.as_deref()).await;
    // And the list the answering loop listens for.
    for _ in 0..8 {
        let held = store.read_key(KEYS).await.ok().flatten();
        let generation = held.as_ref().map(|(_, g)| g.clone());
        let mut keys: Vec<String> = held
            .and_then(|(bytes, _)| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if keys.iter().any(|k| k == pubkey) {
            return;
        }
        keys.push(pubkey.to_string());
        let bytes = serde_json::to_vec(&keys).unwrap_or_default();
        match store.write_key(KEYS, bytes, generation.as_deref()).await {
            Err(Error::Conflict) => continue,
            _ => return,
        }
    }
}

/// Every agent key this site holds an owner for.
const KEYS: &str = "published-agents/agent-keys.json";

/// The agent keys with an owner.
pub(crate) async fn agent_keys(app: &App) -> Vec<String> {
    app.config
        .chat_store
        .read_key(KEYS)
        .await
        .ok()
        .flatten()
        .and_then(|(bytes, _)| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Set who may message published agent `id`, as its publisher.
pub(super) async fn set_messages(
    app: &App,
    publisher: &str,
    id: &str,
    from: super::social::MessagesFrom,
) -> Result<Result<(), Refusal>, Error> {
    if !agent_public::valid_id(id) {
        return Ok(Err(Refusal::Missing));
    }
    Ok(change_owner(app, publisher, id, |record| {
        let mut owned = record.owner.clone().ok_or(Refusal::Missing)?;
        owned.messages = Some(from);
        Ok(owned)
    })
    .await?
    .map(|_| ()))
}

pub(super) async fn set(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<SetOwner>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let publisher = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Json(body)) = body else {
        return Refusal::Invalid("Send the owner tag your key signed.".into()).response();
    };
    if !agent_public::valid_id(&id) {
        return Refusal::Missing.response();
    }
    let Some(agent) = signer(&app, &id) else {
        return unavailable();
    };
    let tag = Tag::new(body.auth);
    let owner = match agent_public::check_owner_tag(&agent, &tag, now()) {
        Ok(owner) => owner,
        Err(_) => {
            return Refusal::Invalid(
                "That owner tag doesn't check out for this agent. Sign a new one.".into(),
            )
            .response();
        }
    };
    let keys = match publisher_keys(&app, &headers).await {
        Ok(keys) => keys,
        Err(response) => return response,
    };
    if !keys.contains(&owner) {
        return Refusal::Invalid("Sign with a key that's on your account.".into()).response();
    }
    let tag_values = tag.as_slice().to_vec();
    let changed = change_owner(&app, &publisher, &id, |_| {
        Ok(Owned {
            tag: tag_values.clone(),
            pubkey: owner.clone(),
            revoked: false,
            signed_at: now(),
            messages: None,
            runtime: agent_public::Runtime::default(),
        })
    })
    .await;
    let (record, owned) = match changed {
        Ok(Ok(found)) => found,
        Ok(Err(r)) => return r.response(),
        Err(e) => return stored(&e),
    };
    write_index(&app, agent.pubkey(), &id).await;
    let published = match events(&app, &id, &record, &owned) {
        Some(events) => send(&app, &events).await,
        None => 0,
    };
    answer(
        StatusCode::OK,
        json!({
            "agent": agent.pubkey(),
            "npub": crate::profiles::npub(agent.pubkey()),
            "owner": owned.pubkey,
            "published": published,
        }),
    )
}

pub(super) async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let publisher = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !agent_public::valid_id(&id) {
        return Refusal::Missing.response();
    }
    if signer(&app, &id).is_none() {
        return unavailable();
    }
    let changed = change_owner(&app, &publisher, &id, |record| {
        let mut owned = record.owner.clone().ok_or(Refusal::Missing)?;
        owned.revoked = true;
        owned.signed_at = now().max(owned.signed_at + 1);
        Ok(owned)
    })
    .await;
    let (record, owned) = match changed {
        Ok(Ok(found)) => found,
        Ok(Err(r)) => return r.response(),
        Err(e) => return stored(&e),
    };
    let published = match events(&app, &id, &record, &owned) {
        Some(events) => send(&app, &events).await,
        None => 0,
    };
    answer(
        StatusCode::OK,
        json!({"revoked": true, "published": published}),
    )
}

/// A published agent as its public page draws it.
pub(crate) struct Found {
    pub(crate) newest: openagents_sdk::agent_lab::PublishedAgent,
    pub(crate) owner: Option<Owned>,
    /// The agent's own key, when this site has agent keys.
    pub(crate) agent: Option<String>,
    pub(crate) definition: Option<agent_graph::definition::Definition>,
}

/// Published agent `id`, if there is one.
pub(crate) async fn found(app: &App, id: &str) -> Option<Found> {
    if !agent_public::valid_id(id) {
        return None;
    }
    let (record, _) = load(&app.config.chat_store, id).await.ok()??;
    Some(Found {
        newest: record.revisions.last()?.clone(),
        owner: record.owner.clone(),
        agent: signer(app, id).map(|s| s.pubkey().to_string()),
        definition: document(&record),
    })
}
