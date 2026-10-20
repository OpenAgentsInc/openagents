//! Public profiles for Nostr keys (#12254): `/@{name}` and `/@{npub}`, the
//! picture they show, and `/.well-known/nostr.json` for
//! `name@openagents.com` (NIP-05).
//!
//! A person's key is their public identity. What the page shows is what
//! the key signed: its newest kind 0 (name, picture, about, website) and
//! kind 3 (who it follows), read from the configured relays
//! (`Config::profile_relays`, `wss://relay.openagents.com` in production)
//! and from the write relays its kind 10002 lists. This server never signs
//! a profile; the apps and Settings sign with the person's own key.
//!
//! Names live in the account service (`tenancy::accounts::names`, through
//! `GET /v1/names/{name}` and `GET /v1/names?pubkey=`). A name shows on a
//! page only when the account service says it points at that key.
//!
//! - `GET /@{name}` and `GET /@{npub}`: the page, server HTML with no
//!   script. `/@{npub}` of a key that has a name moves to `/@{name}`.
//! - `GET /@{handle}/picture`: the profile's picture, fetched by this
//!   server and served from this origin (the site's policy loads images
//!   from its own origin only). Only `https://` pictures on public
//!   addresses, at most [`picture::MAX_BYTES`], in PNG, JPEG, GIF, or WebP.
//! - `GET /.well-known/nostr.json?name={name}`: NIP-05, with
//!   `Access-Control-Allow-Origin: *` as NIP-05 asks.
//!
//! Reads are cached for [`FRESH`] per key, so a busy page reads the
//! relays about once a minute.

mod edit;
pub(crate) mod evidence;
mod explore;
pub(crate) mod gym;
pub(crate) use edit::PAGE as SETTINGS_PAGE;
mod picture;
pub(crate) mod relays;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use futures_util::future::join_all;
use maud::{Markup, html};
use nostr::domain::Event;
use nostr::metadata::{self, Metadata};
use openagents_ui::actions::{Avatar, AvatarSize};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::ui_page::{UiPage, action_link, problem};

/// The agents and pylons Verse draws in the Grid (#12313).
pub(crate) const PLACES: &str = "/verse/places";
/// The names Verse labels avatars with (#12261).
pub(crate) const NAMES: &str = "/verse/names";
/// The most keys one names question asks about.
const NAMES_MAX: usize = 50;

/// NIP-05's document.
pub(crate) const WELL_KNOWN: &str = "/.well-known/nostr.json";
/// The profile page.
pub(crate) const PAGE: &str = "/@{handle}";
/// Its picture.
pub(crate) const PICTURE: &str = "/@{handle}/picture";
/// How long a read stands before the relays are read again.
pub(crate) const FRESH: Duration = Duration::from_secs(60);
/// The most follows named on a page.
const FOLLOWS_SHOWN: usize = 50;
/// The most relays read at once: ours, then a person's own.
const RELAYS_MAX: usize = 3;
/// The NIP-05 domain.
pub(crate) const DOMAIN: &str = "openagents.com";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(WELL_KNOWN, get(nostr_json))
        .route(PAGE, get(page))
        .route(PICTURE, get(picture::serve))
        .route("/event/{id}", get(evidence::event_page))
        .route(explore::PAGE, get(explore::page))
        .route(NAMES, get(names))
        .route(PLACES, get(explore::places))
        .merge(edit::routes())
}

/// Whether this site answers `path` itself.
#[must_use]
pub(crate) fn owns(path: &str) -> bool {
    path == WELL_KNOWN
        || path.starts_with("/@")
        || path.starts_with("/event/")
        || path == explore::PAGE
        || path == NAMES
        || path == PLACES
        || edit::owns(path)
}

/// Who a page is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Who {
    /// 64 lowercase hex characters.
    pub(crate) pubkey: String,
    /// The `name@openagents.com` name pointing at it, if any.
    pub(crate) name: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn npub(pubkey: &str) -> String {
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        match pubkey
            .get(2 * i..2 * i + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        {
            Some(value) => *byte = value,
            None => return pubkey.to_string(),
        }
    }
    nostr::nip19::encode_npub(&key)
}

/// `npub1qqq…xyz`, for headings.
fn short_npub(pubkey: &str) -> String {
    let full = npub(pubkey);
    if full.len() > 20 {
        format!("{}…{}", &full[..12], &full[full.len() - 6..])
    } else {
        full
    }
}

/// The account service's answer to a name question, if it has one.
async fn ask(app: &App, path: &str) -> Result<Option<Value>, ()> {
    let Some(service) = app.config.cloud.as_ref() else {
        return Err(());
    };
    match service
        .nostr_call(reqwest::Method::GET, path, None, None)
        .await
    {
        Ok(answer) if answer.status == 200 => Ok(Some(answer.body)),
        Ok(answer) if answer.status == 404 => Ok(None),
        _ => Err(()),
    }
}

/// The key a name points at.
pub(crate) async fn key_of_name(app: &App, name: &str) -> Result<Option<String>, ()> {
    let name = tenancy::accounts::names::normalize(name);
    if !tenancy::accounts::names::valid_name(&name) {
        return Ok(None);
    }
    Ok(ask(app, &format!("/v1/names/{name}"))
        .await?
        .and_then(|body| body["pubkey"].as_str().map(str::to_string)))
}

/// The name pointing at a key.
pub(crate) async fn name_of_key(app: &App, pubkey: &str) -> Option<String> {
    ask(app, &format!("/v1/names?pubkey={pubkey}"))
        .await
        .ok()
        .flatten()
        .and_then(|body| body["name"].as_str().map(str::to_string))
}

/// What a `/@{handle}` names: a name, or a key as `npub1…` or hex.
pub(crate) enum Handle {
    Name(String),
    Key(String),
}

pub(crate) fn handle(text: &str) -> Option<Handle> {
    if text.starts_with("npub1") {
        return nostr::nip19::decode_npub(text)
            .ok()
            .map(|key| Handle::Key(hex(&key)));
    }
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some(Handle::Key(text.to_ascii_lowercase()));
    }
    let name = tenancy::accounts::names::normalize(text);
    tenancy::accounts::names::valid_name(&name).then_some(Handle::Name(name))
}

/// Who `/@{handle}` is, or why there's no page.
pub(crate) async fn who(app: &App, text: &str) -> Result<Who, StatusCode> {
    match handle(text).ok_or(StatusCode::NOT_FOUND)? {
        Handle::Name(name) => match key_of_name(app, &name).await {
            Ok(Some(pubkey)) => Ok(Who {
                pubkey,
                name: Some(name),
            }),
            Ok(None) => Err(StatusCode::NOT_FOUND),
            Err(()) => Err(StatusCode::BAD_GATEWAY),
        },
        Handle::Key(pubkey) => {
            let name = name_of_key(app, &pubkey).await;
            Ok(Who { pubkey, name })
        }
    }
}

/// A short note someone published (kind 1).
#[derive(Clone, Debug)]
pub(crate) struct Note {
    pub(crate) id: String,
    pub(crate) author: String,
    pub(crate) content: String,
    pub(crate) created_at: u64,
}

/// The most notes the feed shows.
const FEED_SHOWN: usize = 20;

/// What the relays hold for one key.
#[derive(Clone, Debug, Default)]
pub(crate) struct Loaded {
    pub(crate) metadata: Option<Metadata>,
    /// The kind 0 content as signed, so an edit keeps fields it doesn't
    /// touch.
    pub(crate) content: Option<String>,
    pub(crate) follows: Vec<String>,
    /// The follow list's tags as signed, so a new follow keeps petnames
    /// and relay hints (#12304).
    pub(crate) follow_tags: Vec<Vec<String>>,
    /// Recent notes by the first follows, newest first (#12304).
    pub(crate) feed: Vec<Note>,
    /// Shown names of the first follows, by key.
    pub(crate) follow_names: HashMap<String, String>,
    /// Agents this key owns (NIP-AGENT kind 30187 naming it), live ones
    /// only: published ID and name.
    pub(crate) agents: Vec<(String, String)>,
    /// Whether any relay answered.
    pub(crate) reached: bool,
}

type Cache = Mutex<HashMap<String, (Instant, Arc<Loaded>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Cache::default)
}

fn cache_key(relays: &[String], pubkey: &str) -> String {
    format!("{}|{pubkey}", relays.join(","))
}

/// Forget what was read for `pubkey`, after a publish.
pub(crate) fn forget(relays: &[String], pubkey: &str) {
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&cache_key(relays, pubkey));
}

async fn read_all(relays: &[String], filter: &Value) -> (Vec<Event>, bool) {
    let reads = relays
        .iter()
        .map(|relay| relays::read(relay, filter.clone(), relays::READ_WAIT));
    let answers = join_all(reads).await;
    let reached = answers.iter().any(|events| !events.is_empty());
    (answers.into_iter().flatten().collect(), reached)
}

/// Read `pubkey`'s profile, follows, and the names of the first follows.
pub(crate) async fn load(relays: &[String], pubkey: &str) -> Arc<Loaded> {
    let key = cache_key(relays, pubkey);
    if let Some((at, loaded)) = cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        && at.elapsed() < FRESH
    {
        return loaded.clone();
    }
    let ours: Vec<String> = relays.iter().take(RELAYS_MAX).cloned().collect();
    let (mut events, mut reached) = read_all(
        &ours,
        &json!({"authors": [pubkey], "kinds": [0, 3, 10002], "limit": 20}),
    )
    .await;
    // The person's own write relays, when they list any.
    if let Some(list) = metadata::newest(pubkey, 10_002, &events)
        .and_then(|e| nostr::domain::open_relay_list(e).ok())
    {
        let mut theirs = Vec::new();
        for relay in nostr::domain::write_relays(&list) {
            if theirs.len() >= RELAYS_MAX {
                break;
            }
            let relay = relay.trim_end_matches('/').to_string();
            if ours.iter().any(|o| o.trim_end_matches('/') == relay) {
                continue;
            }
            if relays::listed_relay_allowed(&relay).await {
                theirs.push(relay);
            }
        }
        let (more, any) = read_all(
            &theirs,
            &json!({"authors": [pubkey], "kinds": [0, 3], "limit": 10}),
        )
        .await;
        events.extend(more);
        reached |= any;
    }
    let profile = metadata::newest(pubkey, metadata::METADATA_KIND, &events);
    let follow_event = metadata::newest(pubkey, metadata::FOLLOWS_KIND, &events);
    let follows = follow_event.map(metadata::follows).unwrap_or_default();
    let follow_tags: Vec<Vec<String>> = follow_event
        .map(|e| e.tags.iter().map(|t| t.as_slice().to_vec()).collect())
        .unwrap_or_default();
    let mut follow_names = HashMap::new();
    if !follows.is_empty() {
        let first: Vec<&String> = follows.iter().take(FOLLOWS_SHOWN).collect();
        let (theirs, _) = read_all(
            &ours,
            &json!({"authors": first, "kinds": [0], "limit": FOLLOWS_SHOWN * 2}),
        )
        .await;
        for key in first {
            if let Some((found, _)) = metadata::profile_of(key, &theirs)
                && let Some(name) = found.shown_name()
            {
                follow_names.insert(key.clone(), name.to_string());
            }
        }
    }
    let mut feed = Vec::new();
    if !follows.is_empty() {
        let first: Vec<&String> = follows.iter().take(FOLLOWS_SHOWN).collect();
        let (notes, _) = read_all(
            &ours,
            &json!({"authors": first, "kinds": [1], "limit": FEED_SHOWN * 2}),
        )
        .await;
        let mut seen = std::collections::BTreeSet::new();
        let mut notes: Vec<Note> = notes
            .into_iter()
            .filter(|e| e.kind == 1 && follows.contains(&e.pubkey) && seen.insert(e.id.clone()))
            .filter(|e| !e.content.trim().is_empty())
            .map(|e| Note {
                id: e.id,
                author: e.pubkey,
                content: e.content.chars().take(280).collect(),
                created_at: e.created_at,
            })
            .collect();
        notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        notes.truncate(FEED_SHOWN);
        feed = notes;
    }
    let (owned, _) = read_all(
        &ours,
        &json!({"kinds": [nostr::agent_public::DEFINITION_KIND], "#p": [pubkey], "limit": 100}),
    )
    .await;
    let agents = nostr::agent_public::newest_definitions(&owned)
        .into_iter()
        .filter(|d| d.owner == pubkey && !d.revoked)
        .map(|d| {
            let name = d
                .document
                .as_ref()
                .and_then(|doc| doc["experiment"]["title"].as_str())
                .unwrap_or("Agent")
                .to_string();
            (d.id, name)
        })
        .collect();
    let loaded = Arc::new(Loaded {
        agents,
        metadata: profile.map(|e| Metadata::read(&e.content)),
        content: profile.map(|e| e.content.clone()),
        follows,
        follow_tags,
        feed,
        follow_names,
        reached: reached || profile.is_some(),
    });
    let mut held = cache().lock().unwrap_or_else(PoisonError::into_inner);
    if held.len() > 10_000 {
        held.retain(|_, (at, _)| at.elapsed() < FRESH);
    }
    held.insert(key, (Instant::now(), loaded.clone()));
    loaded
}

/// Where a key's page lives: its name when it has one.
pub(crate) fn href(pubkey: &str, name: Option<&str>) -> String {
    match name {
        Some(name) => format!("/@{name}"),
        None => format!("/@{}", npub(pubkey)),
    }
}

fn missing(headers: &HeaderMap) -> Response {
    problem(
        headers,
        StatusCode::NOT_FOUND,
        "Profile not found",
        "Nobody has that name.",
        ("/", "Home"),
    )
}

fn unreachable(headers: &HeaderMap) -> Response {
    problem(
        headers,
        StatusCode::BAD_GATEWAY,
        "Profile unavailable",
        "We can't look that name up right now. Try again in a minute.",
        ("/", "Home"),
    )
}

async fn page(State(app): State<App>, Path(text): Path<String>, headers: HeaderMap) -> Response {
    let who = match who(&app, &text).await {
        Ok(who) => who,
        Err(StatusCode::NOT_FOUND) => return missing(&headers),
        Err(_) => return unreachable(&headers),
    };
    if let Some(name) = &who.name
        && *name != text
    {
        return Redirect::permanent(&format!("/@{name}")).into_response();
    }
    let loaded = load(&app.config.profile_relays, &who.pubkey).await;
    let evidence = evidence::load(
        &app.config.profile_relays,
        &app.config.xp_referees,
        &who.pubkey,
    )
    .await;
    render(&headers, &who, &loaded, &evidence)
}

fn render(
    headers: &HeaderMap,
    who: &Who,
    loaded: &Loaded,
    evidence: &evidence::Evidence,
) -> Response {
    let metadata = loaded.metadata.clone().unwrap_or_default();
    let npub_full = npub(&who.pubkey);
    let handle_text = who
        .name
        .as_ref()
        .map_or_else(|| short_npub(&who.pubkey), |name| format!("@{name}"));
    let shown = metadata
        .shown_name()
        .map_or_else(|| handle_text.clone(), str::to_string);
    let path = href(&who.pubkey, who.name.as_deref());
    let mut avatar = Avatar::new().name(shown.clone()).size(AvatarSize::Px96);
    if metadata.picture.is_some() {
        avatar = avatar.image_url(format!("{path}/picture"));
    }
    let description = metadata
        .about
        .as_deref()
        .map(|about| about.chars().take(160).collect::<String>())
        .unwrap_or_else(|| format!("{shown} on OpenAgents."));
    let content = html! {
        section.oa-card aria-labelledby="profile-name" {
            (avatar)
            (MarkdownRoot::new(html! {
                h1 id="profile-name" { (shown) }
                p { (handle_text) }
                @if let Some(name) = &who.name {
                    p.oa-page-meta { (name) "@" (DOMAIN) }
                }
                @if let Some(about) = &metadata.about {
                    @for line in about.lines().filter(|l| !l.trim().is_empty()) {
                        p { (line) }
                    }
                }
                @if !loaded.reached {
                    p.oa-page-meta { "We couldn't reach the profile's relays, so some of it may be missing." }
                } @else if loaded.metadata.is_none() {
                    p.oa-page-meta { "No profile yet." }
                }
            }))
            div.oa-page-actions {
                (action_link("Follow", &format!("{path}/follow")))
                @if let Some(website) = &metadata.website {
                    (action_link("Website", website))
                }
            }
        }
        @if !loaded.agents.is_empty() {
            section.oa-card aria-labelledby="profile-agents" {
                (MarkdownRoot::new(html! {
                    h2 id="profile-agents" { "Agents" }
                    ul {
                        @for (id, name) in &loaded.agents {
                            li { a href=(format!("/agent/{id}")) { (name) } }
                        }
                    }
                }))
            }
        }
        (evidence::sections(evidence))
        (feed_section(loaded))
        (follows_section(loaded))
        section.oa-card aria-labelledby="profile-key" {
            (MarkdownRoot::new(html! {
                h2 id="profile-key" { "Nostr key" }
                p { code { (npub_full) } }
            }))
        }
    };
    UiPage::new(shown.clone())
        .path(path)
        .description(description)
        .scriptless()
        .content(PageColumn::new(content))
        .respond(headers)
}

fn feed_section(loaded: &Loaded) -> Markup {
    if loaded.feed.is_empty() {
        return html! {};
    }
    html! {
        section.oa-card aria-labelledby="profile-feed" {
            (MarkdownRoot::new(html! {
                h2 id="profile-feed" { "From people they follow" }
                @for note in &loaded.feed {
                    p {
                        a href=(href(&note.author, None)) {
                            (loaded.follow_names.get(&note.author).cloned().unwrap_or_else(|| short_npub(&note.author)))
                        }
                        ": "
                        a href=(format!("/event/{}", note.id)) { (note.content) }
                    }
                }
            }))
        }
    }
}

fn follows_section(loaded: &Loaded) -> Markup {
    if loaded.follows.is_empty() {
        return html! {};
    }
    let count = loaded.follows.len();
    html! {
        section.oa-card aria-labelledby="profile-follows" {
            (MarkdownRoot::new(html! {
                h2 id="profile-follows" {
                    "Follows " (count)
                }
                ul {
                    @for key in loaded.follows.iter().take(FOLLOWS_SHOWN) {
                        li {
                            a href=(href(key, None)) {
                                (loaded.follow_names.get(key).cloned().unwrap_or_else(|| short_npub(key)))
                            }
                        }
                    }
                }
                @if count > FOLLOWS_SHOWN {
                    p.oa-page-meta { "And " (count - FOLLOWS_SHOWN) " more." }
                }
            }))
        }
    }
}

#[derive(Deserialize)]
struct NameQuery {
    name: Option<String>,
}

/// `GET /.well-known/nostr.json?name=`.
async fn nostr_json(State(app): State<App>, Query(query): Query<NameQuery>) -> Response {
    let mut names = serde_json::Map::new();
    let mut relays = serde_json::Map::new();
    let mut status = StatusCode::OK;
    if let Some(name) = query.name.as_deref() {
        let name = tenancy::accounts::names::normalize(name);
        match key_of_name(&app, &name).await {
            Ok(Some(pubkey)) => {
                let listed: Vec<Value> = app
                    .config
                    .profile_relays
                    .iter()
                    .filter(|r| r.starts_with("wss://"))
                    .map(|r| json!(r))
                    .collect();
                if !listed.is_empty() {
                    relays.insert(pubkey.clone(), Value::Array(listed));
                }
                names.insert(name, json!(pubkey));
            }
            Ok(None) => {}
            Err(()) => status = StatusCode::SERVICE_UNAVAILABLE,
        }
    }
    let mut body = json!({"names": names});
    if !relays.is_empty() {
        body["relays"] = Value::Object(relays);
    }
    let mut response = (status, axum::Json(body)).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if status == StatusCode::OK {
            "public, max-age=300"
        } else {
            "no-store"
        }),
    );
    response
}

#[derive(Deserialize)]
struct KeysQuery {
    keys: Option<String>,
}

/// `GET /verse/names?keys=HEX,HEX`: each key's shown name from its kind 0,
/// and its `name@openagents.com` name when it has one, for Verse to label
/// avatars. Keys without a profile are left out.
async fn names(State(app): State<App>, Query(query): Query<KeysQuery>) -> Response {
    let keys: Vec<String> = query
        .keys
        .unwrap_or_default()
        .split(',')
        .filter_map(|k| match handle(k.trim())? {
            Handle::Key(key) => Some(key),
            Handle::Name(_) => None,
        })
        .take(NAMES_MAX)
        .collect();
    let loaded = join_all(keys.iter().map(|k| load(&app.config.profile_relays, k))).await;
    let mut out = serde_json::Map::new();
    for (key, profile) in keys.iter().zip(loaded) {
        if let Some(shown) = profile.metadata.as_ref().and_then(Metadata::shown_name) {
            out.insert(key.clone(), json!({"name": shown}));
        }
    }
    let mut response = axum::Json(json!({"names": out})).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60"),
    );
    response
}
