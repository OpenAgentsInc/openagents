//! Agent-linked NIP-ATT evidence and explicitly published gateway receipts.
//! No model-name or operator-name matching: the signed definition names
//! exact ATT events, and a receipt names the agent's definition address.

use std::collections::BTreeSet;
use std::time::Duration;

use maud::{Markup, html};
use nostr::{agent_public, att, domain::Event};
use openagents_ui::content::MarkdownRoot;
use serde_json::json;

use super::read_all;
use crate::App;

const SHOWN: usize = 20;

#[derive(Default)]
struct Evidence {
    release: Option<(String, String)>,
    rebuilds: Vec<String>,
    endpoint: Option<String>,
    receipts: Vec<(String, receipts::ExecutionReceipt)>,
}

/// Bound the entire extra read so an unavailable relay cannot hold the page.
pub(crate) async fn section(app: &App, agent: &str, id: &str, owner: &str) -> Markup {
    tokio::time::timeout(Duration::from_secs(8), load(app, agent, id, owner))
        .await
        .map(|e| render(&e))
        .unwrap_or_default()
}

async fn load(app: &App, agent: &str, id: &str, owner: &str) -> Evidence {
    let relays: Vec<String> = app.config.profile_relays.iter().take(3).cloned().collect();
    let (definitions, _) = read_all(
        &relays,
        &json!({
            "kinds": [agent_public::DEFINITION_KIND], "authors": [agent], "#d": [id], "limit": 20,
        }),
    )
    .await;
    let at = crate::cloud::session::now();
    let Some(definition) = agent_public::newest_definitions(&definitions)
        .into_iter()
        .find(|d| {
            d.agent == agent
                && d.id == id
                && d.owner == owner
                && !d.revoked
                && d.created_at <= at + 60
        })
    else {
        return Evidence::default();
    };
    let (mut found, (events, _)) = tokio::join!(
        runtime(&relays, &definition.runtime, at),
        read_all(
            &relays,
            &json!({
                "kinds": [receipts_nostr::KIND], "authors": [owner], "#p": [agent],
                "#a": [format!("{}:{agent}:{id}", agent_public::DEFINITION_KIND)], "limit": SHOWN * 4,
            })
        ),
    );
    found.receipts = selected_receipts(&definition, &events, at);
    found
}

fn selected_receipts(
    definition: &agent_public::Definition,
    events: &[Event],
    at: u64,
) -> Vec<(String, receipts::ExecutionReceipt)> {
    let mut sorted: Vec<_> = events.iter().collect();
    sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    let mut seen = BTreeSet::new();
    sorted
        .into_iter()
        .filter(|event| event.created_at <= at + 60)
        .filter_map(|event| {
            receipts_nostr::parse(event, definition)
                .ok()
                .map(|p| (event.id.clone(), p.receipt))
        })
        // Republishing the same receipt does not count it twice.
        .filter(|(_, receipt)| seen.insert(receipt.digest.clone()))
        .take(SHOWN)
        .collect()
}

async fn runtime(relays: &[String], runtime: &agent_public::Runtime, at: u64) -> Evidence {
    let mut found = Evidence::default();
    let ids: Vec<_> = [&runtime.release, &runtime.endpoint]
        .into_iter()
        .flatten()
        .collect();
    if ids.is_empty() {
        return found;
    }
    let (mut events, _) = read_all(relays, &json!({"ids": ids})).await;
    let endpoint = runtime
        .endpoint
        .as_ref()
        .and_then(|id| events.iter().find(|e| &e.id == id))
        .and_then(|event| {
            att::parse_endpoint(event)
                .ok()
                .map(|parsed| (event.clone(), parsed))
        });
    let release_id = runtime
        .release
        .as_deref()
        .or_else(|| endpoint.as_ref().map(|(_, e)| e.body.release.as_str()));
    let Some(release_id) = release_id else {
        return found;
    };
    if !events.iter().any(|e| e.id == release_id) {
        let (more, _) = read_all(relays, &json!({"ids": [release_id]})).await;
        events.extend(more);
    }
    let Some((event, release)) = events
        .iter()
        .find(|e| e.id == release_id)
        .and_then(|e| att::parse_release(e).ok().map(|r| (e, r)))
        .filter(|(e, _)| e.created_at <= at + 60)
    else {
        return found;
    };
    // A direct release pin is an owner declaration and a publisher signature.
    // Only the full /att check below can establish a hardware-backed endpoint.
    if runtime.release.is_some() {
        found.release = Some((event.id.clone(), release.publisher.clone()));
    }
    let endpoint_check = async {
        let (endpoint_event, endpoint) = endpoint.as_ref()?;
        if endpoint.body.release != event.id {
            return None;
        }
        let (heads, _) = read_all(relays, &json!({
            "kinds": [att::HEAD_KIND], "authors": [release.publisher], "#d": [release.workload], "limit": 20,
        })).await;
        let head = heads
            .into_iter()
            .filter(|e| e.created_at <= at + 60 && att::parse_head(e).is_ok())
            .max_by(|a, b| a.created_at.cmp(&b.created_at).then(b.id.cmp(&a.id)))?;
        let records = oa_att::Records {
            release: event.clone(),
            head,
            endpoint: endpoint_event.clone(),
            beacon: None,
        };
        (crate::pages::att::gateway_check(&records, &release.workload, at)["ok"] == true)
            .then(|| endpoint_event.id.clone())
    };
    let rebuild_check = async {
        if release.rebuilds.is_empty() {
            return vec![];
        }
        let builders: Vec<_> = release
            .rebuilds
            .iter()
            .map(|r| &r.builder)
            .take(SHOWN)
            .collect();
        let (statements, _) = read_all(relays, &json!({
            "kinds": [att_release::rebuild::KIND], "authors": builders, "#w": [release.workload], "limit": SHOWN * 4,
        })).await;
        matching_rebuilds(&release, &statements, at)
    };
    let (endpoint, rebuilds) = tokio::join!(endpoint_check, rebuild_check);
    if endpoint.is_some() {
        found.release = Some((event.id.clone(), release.publisher));
    }
    found.endpoint = endpoint;
    if found.release.is_some() {
        found.rebuilds = rebuilds;
    }
    found
}

fn matching_rebuilds(release: &att::Release, statements: &[Event], at: u64) -> Vec<String> {
    let mut seen = BTreeSet::new();
    statements
        .iter()
        .filter(|e| e.created_at <= at + 60)
        .filter(|e| att_release::rebuild::check(release, std::slice::from_ref(e), 1).is_ok())
        .filter(|e| seen.insert(e.pubkey.clone()))
        .take(SHOWN)
        .map(|e| e.id.clone())
        .collect()
}

fn render(evidence: &Evidence) -> Markup {
    html! {
        @if let Some((release, publisher)) = &evidence.release {
            section.oa-card aria-labelledby="agent-attestations" {
                (MarkdownRoot::new(html! {
                    h2 id="agent-attestations" { "Attestations" }
                    ul {
                        li {
                            a href=(format!("/event/{release}")) { "Release" }
                            " · signed by "
                            a href=(super::href(publisher, None)) { (super::short_npub(publisher)) }
                        }
                        @for rebuild in &evidence.rebuilds {
                            li { a href=(format!("/event/{rebuild}")) { "Matching independent rebuild" } }
                        }
                        @if let Some(endpoint) = &evidence.endpoint {
                            li { a href=(format!("/event/{endpoint}")) { "Verified sealed endpoint" } }
                        }
                    }
                }))
            }
        }
        @if !evidence.receipts.is_empty() {
            section.oa-card aria-labelledby="agent-gateway-receipts" {
                (MarkdownRoot::new(html! {
                    h2 id="agent-gateway-receipts" { "Gateway receipts" }
                    p.oa-page-meta { "Chosen and published by the owner." }
                    ul {
                        @for (id, receipt) in &evidence.receipts {
                            li {
                                a href=(format!("/event/{id}")) { (receipt.served.model) }
                                " · " (match receipt.outcome {
                                    receipts::Outcome::Answered => "Answered",
                                    receipts::Outcome::Refused => "Refused",
                                    receipts::Outcome::Unavailable => "Unavailable",
                                    receipts::Outcome::Unattempted => "Not attempted",
                                    receipts::Outcome::Unknown => "Unknown outcome",
                                })
                                @if let Some(ms) = receipt.timing.latency_ms { " · " (ms) " ms" }
                            }
                        }
                    }
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "agent_evidence_tests.rs"]
mod tests;
