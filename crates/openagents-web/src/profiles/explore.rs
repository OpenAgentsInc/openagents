//! `/explore` (#12259): agents, plugins, and pylons, read from relay events
//! so any client can build the same directory.
//!
//! - **Agents**: live NIP-AGENT public definitions (kind 30187) with a valid
//!   owner tag, ranked by the receipts and reviews signed about each agent
//!   (U4's evidence), never by payment. Each links to its page and its
//!   owner's profile.
//! - **Plugins**: NIP-EXT listings read through
//!   `plugin_registry::listings::published`, the same reader the
//!   `openagents` CLI's `search` uses, without the hosted runner's samples.
//!   Each links to its publisher's profile and its release event.
//! - **Pylons**: the same live snapshot `/pylons` draws.
//!
//! Server HTML with no script. Cached for [`super::FRESH`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use maud::{Markup, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde_json::json;

use super::{evidence, read_all, short_npub};
use crate::App;
use crate::ui_page::UiPage;

pub(crate) const PAGE: &str = "/explore";
/// The most of each kind shown.
const SHOWN: usize = 50;

#[derive(Clone, Debug)]
pub(crate) struct AgentRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) owner: String,
    /// Receipts and reviews signed about the agent.
    pub(crate) evidence: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Directory {
    pub(crate) agents: Vec<AgentRow>,
    pub(crate) plugins: Vec<plugin_registry::listings::Listing>,
    pub(crate) skills: Vec<SkillRow>,
}

/// A published skill (NIP-SKILL), with the reviews signed about it.
#[derive(Clone, Debug)]
pub(crate) struct SkillRow {
    pub(crate) listing: nostr::skill_listing::Listing,
    pub(crate) reviews: usize,
}

type Cache = Mutex<HashMap<String, (Instant, Arc<Directory>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Cache::default)
}

/// Rank `agents` by the evidence among `about`, most first, then by name.
pub(crate) fn ranked(
    mut agents: Vec<AgentRow>,
    about: &[nostr::domain::Event],
    keys: &HashMap<String, String>,
) -> Vec<AgentRow> {
    for row in &mut agents {
        if let Some(key) = keys.get(&row.id) {
            let (receipts, labels) = evidence::receipts_and_labels(key, about);
            row.evidence = receipts.len() + labels.len();
        }
    }
    agents.sort_by(|a, b| b.evidence.cmp(&a.evidence).then(a.name.cmp(&b.name)));
    agents.truncate(SHOWN);
    agents
}

pub(crate) async fn load(app: &App) -> Arc<Directory> {
    // Keep the relays' skill listings in line with the directory (#12303).
    let syncing = app.clone();
    tokio::spawn(async move { super::skills::sync(&syncing).await });
    let directories: Vec<String> = super::skills::directory(app)
        .map(|s| vec![s.pubkey().to_string()])
        .unwrap_or_default();
    let relays = &app.config.profile_relays;
    let key = format!("{}|{}", relays.join(","), directories.join(","));
    if let Some((at, found)) = cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        && at.elapsed() < super::FRESH
    {
        return found.clone();
    }
    let ours: Vec<String> = relays.iter().take(3).cloned().collect();
    let (definitions, _) = read_all(
        &ours,
        &json!({"kinds": [nostr::agent_public::DEFINITION_KIND], "limit": 500}),
    )
    .await;
    let mut keys = HashMap::new();
    let agents: Vec<AgentRow> = nostr::agent_public::newest_definitions(&definitions)
        .into_iter()
        .filter(|d| !d.revoked)
        .map(|d| {
            keys.insert(d.id.clone(), d.agent.clone());
            AgentRow {
                name: d
                    .document
                    .as_ref()
                    .and_then(|doc| doc["experiment"]["title"].as_str())
                    .unwrap_or("Agent")
                    .to_string(),
                id: d.id,
                owner: d.owner,
                evidence: 0,
            }
        })
        .collect();
    let agent_keys: Vec<&String> = keys.values().take(200).collect();
    let about = if agent_keys.is_empty() {
        Vec::new()
    } else {
        read_all(
            &ours,
            &json!({"kinds": [nostr::pylon::RECEIPT_KIND, 1985], "#p": agent_keys, "limit": 2000}),
        )
        .await
        .0
    };
    let agents = ranked(agents, &about, &keys);
    let (listings, _) = read_all(&ours, &plugin_registry::listings::filter(None, None, 500)).await;
    let mut plugins =
        plugin_registry::listings::without_samples(plugin_registry::listings::published(listings));
    plugins.truncate(SHOWN);
    let skills = if directories.is_empty() {
        Vec::new()
    } else {
        let (listings, _) = read_all(
            &ours,
            &json!({"kinds": [nostr::skill_listing::KIND], "authors": directories, "limit": 500}),
        )
        .await;
        let current = nostr::skill_listing::current(&listings, &directories);
        let addresses: Vec<String> = current
            .iter()
            .map(|l| nostr::skill_listing::address(&l.directory, &l.skill.name))
            .take(200)
            .collect();
        let reviews = if addresses.is_empty() {
            Vec::new()
        } else {
            read_all(
                &ours,
                &json!({"kinds": [1985], "#a": addresses, "limit": 2000}),
            )
            .await
            .0
        };
        let mut rows: Vec<SkillRow> = current
            .into_iter()
            .map(|listing| {
                let address =
                    nostr::skill_listing::address(&listing.directory, &listing.skill.name);
                let reviews = reviews
                    .iter()
                    .filter(|e| {
                        e.validate_crypto().is_ok() && e.tag_values("a").any(|a| a == address)
                    })
                    .map(|e| e.pubkey.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len();
                SkillRow { listing, reviews }
            })
            .collect();
        rows.sort_by(|a, b| {
            b.reviews
                .cmp(&a.reviews)
                .then(a.listing.skill.name.cmp(&b.listing.skill.name))
        });
        rows.truncate(SHOWN);
        rows
    };
    let found = Arc::new(Directory {
        agents,
        plugins,
        skills,
    });
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, (Instant::now(), found.clone()));
    found
}

fn pylons(app: &App) -> Markup {
    let Some(relay) = app.config.pylons_relay.as_deref() else {
        return html! {};
    };
    let snapshot = crate::pages::pylons_field(relay).current();
    html! {
        section.oa-card aria-labelledby="explore-pylons" {
            (MarkdownRoot::new(html! {
                h2 id="explore-pylons" { "Pylons" }
                @if snapshot.pylons.is_empty() {
                    p { "No pylons are online right now." }
                } @else {
                    ul {
                        @for pylon in snapshot.pylons.iter().take(SHOWN) {
                            li { strong { (pylon.name) } " " (pylon.summary()) }
                        }
                    }
                }
                p { a href="/pylons" { "See them on the map" } }
            }))
        }
    }
}

pub(super) async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    let directory = load(&app).await;
    let content = html! {
        (MarkdownRoot::new(html! {
            h1 { "Explore" }
            p { "Agents, skills, plugins, and pylons on the OpenAgents network. Everything here comes from signed Nostr events, so any app can show the same list." }
        }))
        section.oa-card aria-labelledby="explore-agents" {
            (MarkdownRoot::new(html! {
                h2 id="explore-agents" { "Agents" }
                @if directory.agents.is_empty() {
                    p { "No agents are published yet." }
                } @else {
                    ul {
                        @for agent in &directory.agents {
                            li {
                                a href=(format!("/agent/{}", agent.id)) { (agent.name) }
                                " · by "
                                a href=(super::href(&agent.owner, None)) { (short_npub(&agent.owner)) }
                                @if agent.evidence > 0 {
                                    " · " (agent.evidence) " signed reviews and jobs"
                                }
                            }
                        }
                    }
                }
            }))
        }
        section.oa-card aria-labelledby="explore-skills" {
            (MarkdownRoot::new(html! {
                h2 id="explore-skills" { "Skills" }
                @if directory.skills.is_empty() {
                    p { "No skills are published yet." }
                } @else {
                    ul {
                        @for row in &directory.skills {
                            li {
                                a href=(row.listing.skill.url) { (row.listing.skill.name) }
                                " " (row.listing.skill.version)
                                @if let Some(author) = &row.listing.author {
                                    " · by "
                                    a href=(super::href(author, None)) { (short_npub(author)) }
                                }
                                @if !row.listing.skill.category.is_empty() {
                                    " · " (row.listing.skill.category)
                                }
                                @if row.reviews > 0 {
                                    " · " (row.reviews) " signed reviews"
                                }
                            }
                        }
                    }
                }
            }))
        }
        section.oa-card aria-labelledby="explore-plugins" {
            (MarkdownRoot::new(html! {
                h2 id="explore-plugins" { "Plugins" }
                @if directory.plugins.is_empty() {
                    p { "No plugins are published yet." }
                } @else {
                    ul {
                        @for plugin in &directory.plugins {
                            li {
                                @if let Some(release) = plugin.release["id"].as_str().filter(|id| nostr::agent_public::valid_id(id)) {
                                    a href=(format!("/event/{release}")) { (if plugin.title.is_empty() { &plugin.slug } else { &plugin.title }) }
                                } @else {
                                    (if plugin.title.is_empty() { &plugin.slug } else { &plugin.title })
                                }
                                " · by "
                                a href=(super::href(&plugin.publisher, None)) { (short_npub(&plugin.publisher)) }
                                @if !plugin.description.is_empty() {
                                    " · " (plugin.description)
                                }
                            }
                        }
                    }
                }
            }))
        }
        (pylons(&app))
    };
    UiPage::new("Explore")
        .path(PAGE)
        .description("Agents, skills, plugins, and pylons on the OpenAgents network.")
        .scriptless()
        .content(PageColumn::new(content))
        .respond(&headers)
}

/// `GET /verse/places`: the agents and pylons the Verse Grid draws
/// (#12313), from the same reads as this page.
pub(super) async fn places(State(app): State<App>) -> Response {
    use axum::response::IntoResponse;
    let directory = load(&app).await;
    let pylons: Vec<serde_json::Value> = match app.config.pylons_relay.as_deref() {
        Some(relay) => crate::pages::pylons_field(relay)
            .current()
            .pylons
            .iter()
            .take(SHOWN)
            .map(|p| json!({"id": p.id, "name": p.name}))
            .collect(),
        None => Vec::new(),
    };
    let agents: Vec<serde_json::Value> = directory
        .agents
        .iter()
        .map(|a| json!({"id": a.id, "name": a.name}))
        .collect();
    let mut response = axum::Json(json!({"agents": agents, "pylons": pylons})).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("public, max-age=60"),
    );
    response
}
