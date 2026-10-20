//! What signed events say about a key (#12257): XP, receipts, and labels,
//! on `/@{name}` and on an agent's page, each linking to the event behind
//! it at `/event/{id}`.
//!
//! - **XP** comes from NIP-XP awards (kind 3193) naming the key, derived
//!   with `xp-ledger` under the site's referee list
//!   (`Config::xp_referees`). Awards by anyone else don't count; revoked
//!   awards don't count; every award is re-checked against the quest and
//!   evidence it names, which are read from the relays too.
//! - **Receipts** are NIP-PYLON buyer receipts (kind 3201) for work the key
//!   did as a provider, each checked with `nostr::pylon::parse_receipt`.
//! - **Labels** are NIP-32 labels (kind 1985) naming the key, shown with who
//!   signed them.
//!
//! Reads are cached for [`super::FRESH`] per key.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use maud::{Markup, html};
use nostr::domain::{Event, LabelTarget, open_labeling};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde_json::json;

use super::read_all;
use crate::App;
use crate::ui_page::{UiPage, problem};

/// The most of each kind read and shown.
const SHOWN: usize = 20;

/// One counted award.
#[derive(Clone, Debug)]
pub(crate) struct XpLine {
    pub(crate) award: String,
    pub(crate) title: String,
    pub(crate) xp: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiptLine {
    pub(crate) id: String,
    pub(crate) buyer: String,
    pub(crate) capability: String,
    pub(crate) outcome: &'static str,
}

#[derive(Clone, Debug)]
pub(crate) struct LabelLine {
    pub(crate) id: String,
    pub(crate) value: String,
    pub(crate) signer: String,
}

/// What the relays say about one key.
#[derive(Clone, Debug, Default)]
pub(crate) struct Evidence {
    pub(crate) xp: u64,
    pub(crate) awards: Vec<XpLine>,
    pub(crate) receipts: Vec<ReceiptLine>,
    pub(crate) labels: Vec<LabelLine>,
}

impl Evidence {
    fn is_empty(&self) -> bool {
        self.awards.is_empty() && self.receipts.is_empty() && self.labels.is_empty()
    }
}

type Cache = Mutex<HashMap<String, (Instant, Arc<Evidence>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Cache::default)
}

fn ids_named(events: &[Event], seen: &BTreeSet<String>) -> Vec<String> {
    let mut out: Vec<String> = events
        .iter()
        .flat_map(|e| e.tag_values("e").map(str::to_string).collect::<Vec<_>>())
        .filter(|id| id.len() == 64 && !seen.contains(id))
        .collect();
    out.sort();
    out.dedup();
    out.truncate(200);
    out
}

/// XP for `pubkey` under `referees`: totals and the awards that counted.
pub(crate) fn xp_of(pubkey: &str, events: &[Event], referees: &[String]) -> (u64, Vec<XpLine>) {
    let trust = xp_ledger::XpTrust {
        referees: referees.iter().cloned().collect(),
        runners: BTreeSet::new(),
    };
    let ledger = xp_ledger::derive(events, &trust);
    let awards = ledger
        .credits
        .iter()
        .filter(|c| c.pubkey == pubkey)
        .map(|c| XpLine {
            award: c.award.clone(),
            title: c.title.clone(),
            xp: c.xp,
        })
        .collect();
    (ledger.totals.get(pubkey).copied().unwrap_or(0), awards)
}

/// The receipts and labels among `events` that are about `pubkey`.
pub(crate) fn receipts_and_labels(
    pubkey: &str,
    events: &[Event],
) -> (Vec<ReceiptLine>, Vec<LabelLine>) {
    let mut receipts = Vec::new();
    let mut labels = Vec::new();
    let mut seen = BTreeSet::new();
    for event in events {
        if !seen.insert(event.id.clone()) || event.validate_crypto().is_err() {
            continue;
        }
        if event.kind == nostr::pylon::RECEIPT_KIND {
            if let Ok(receipt) = nostr::pylon::parse_receipt(event, None)
                && receipt.provider == pubkey
                && receipts.len() < SHOWN
            {
                receipts.push(ReceiptLine {
                    id: event.id.clone(),
                    buyer: receipt.buyer,
                    capability: receipt.capability,
                    outcome: match receipt.outcome {
                        nostr::pylon::Outcome::Accepted => "Accepted",
                        nostr::pylon::Outcome::Rejected => "Rejected",
                        nostr::pylon::Outcome::Failed => "Failed",
                        nostr::pylon::Outcome::Timeout => "Timed out",
                    },
                });
            }
        } else if event.kind == 1_985
            && event.pubkey != pubkey
            && let Ok(labeling) = open_labeling(event)
            && labeling
                .targets
                .iter()
                .any(|t| matches!(t, LabelTarget::Pubkey { pubkey: p, .. } if p == pubkey))
        {
            for label in labeling.labels {
                if labels.len() < SHOWN && label.value.chars().count() <= 100 {
                    labels.push(LabelLine {
                        id: event.id.clone(),
                        value: label.value,
                        signer: event.pubkey.clone(),
                    });
                }
            }
        }
    }
    (receipts, labels)
}

/// Read what the relays say about `pubkey`.
pub(crate) async fn load(relays: &[String], referees: &[String], pubkey: &str) -> Arc<Evidence> {
    let key = format!("{}|{}|{pubkey}", relays.join(","), referees.join(","));
    if let Some((at, found)) = cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        && at.elapsed() < super::FRESH
    {
        return found.clone();
    }
    let ours: Vec<String> = relays.iter().take(3).cloned().collect();
    let (about, _) = read_all(
        &ours,
        &json!({"kinds": [nostr::pylon::RECEIPT_KIND, 1985], "#p": [pubkey], "limit": SHOWN * 4}),
    )
    .await;
    let (receipts, labels) = receipts_and_labels(pubkey, &about);
    let (mut xp, mut awards) = (0, Vec::new());
    if !referees.is_empty() {
        let (mut events, _) = read_all(
            &ours,
            &json!({"kinds": [nostr::xp::AWARD_KIND], "#p": [pubkey], "authors": referees, "limit": 200}),
        )
        .await;
        if !events.is_empty() {
            let (more, _) = read_all(
                &ours,
                &json!({"kinds": [nostr::xp::QUEST_KIND, nostr::xp::REVOCATION_KIND], "authors": referees, "limit": 500}),
            )
            .await;
            events.extend(more);
            // The evidence the awards name, and what that evidence names.
            let mut seen: BTreeSet<String> = events.iter().map(|e| e.id.clone()).collect();
            for _ in 0..2 {
                let wanted = ids_named(&events, &seen);
                if wanted.is_empty() {
                    break;
                }
                let (found, _) = read_all(&ours, &json!({"ids": wanted})).await;
                seen.extend(found.iter().map(|e| e.id.clone()));
                events.extend(found);
            }
            (xp, awards) = xp_of(pubkey, &events, referees);
        }
    }
    let found = Arc::new(Evidence {
        xp,
        awards,
        receipts,
        labels,
    });
    let mut held = cache().lock().unwrap_or_else(PoisonError::into_inner);
    if held.len() > 10_000 {
        held.clear();
    }
    held.insert(key, (Instant::now(), found.clone()));
    found
}

fn event_link(id: &str) -> String {
    format!("/event/{id}")
}

/// The sections a page shows, or nothing when there's no evidence.
pub(crate) fn sections(evidence: &Evidence) -> Markup {
    if evidence.is_empty() {
        return html! {};
    }
    html! {
        @if !evidence.awards.is_empty() {
            section.oa-card aria-labelledby="evidence-xp" {
                (MarkdownRoot::new(html! {
                    h2 id="evidence-xp" { "XP " (evidence.xp) }
                    ul {
                        @for award in &evidence.awards {
                            li {
                                a href=(event_link(&award.award)) { (award.title) }
                                " " (award.xp) " XP"
                            }
                        }
                    }
                }))
            }
        }
        @if !evidence.receipts.is_empty() {
            section.oa-card aria-labelledby="evidence-receipts" {
                (MarkdownRoot::new(html! {
                    h2 id="evidence-receipts" { "Jobs done" }
                    ul {
                        @for receipt in &evidence.receipts {
                            li {
                                a href=(event_link(&receipt.id)) { (receipt.capability) }
                                " · " (receipt.outcome) " · for "
                                a href=(super::href(&receipt.buyer, None)) { (super::short_npub(&receipt.buyer)) }
                            }
                        }
                    }
                }))
            }
        }
        @if !evidence.labels.is_empty() {
            section.oa-card aria-labelledby="evidence-labels" {
                (MarkdownRoot::new(html! {
                    h2 id="evidence-labels" { "Reviews" }
                    ul {
                        @for label in &evidence.labels {
                            li {
                                a href=(event_link(&label.id)) { (label.value) }
                                " · from "
                                a href=(super::href(&label.signer, None)) { (super::short_npub(&label.signer)) }
                            }
                        }
                    }
                }))
            }
        }
    }
}

/// `/event/{id}`: one signed event as the relays hold it, so a number on a
/// profile can be checked.
pub(super) async fn event_page(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let missing = || {
        problem(
            &headers,
            StatusCode::NOT_FOUND,
            "Event not found",
            "The relays we read don't have that event.",
            ("/", "Home"),
        )
    };
    if !nostr::agent_public::valid_id(&id) {
        return missing();
    }
    let ours: Vec<String> = app.config.profile_relays.iter().take(3).cloned().collect();
    let (found, _) = read_all(&ours, &json!({"ids": [id]})).await;
    let (mut event, from_book) = if let Some(event) = found.into_iter().find(|event| event.id == id)
    {
        (event, false)
    } else if let Some(event) = crate::profile_earnings::event(&app, &id).await {
        (event, true)
    } else {
        return missing();
    };
    // A relay copy cannot restore visibility after the owner hides a service receipt.
    if !from_book && nostr::earnings::read(&event).is_ok() {
        let Some(selected) = crate::profile_earnings::event(&app, &id).await else {
            return missing();
        };
        event = selected;
    }
    let pretty = serde_json::to_string_pretty(&event).unwrap_or_default();
    let content = if let Some(receipt) = crate::profile_earnings::receipt_card(&event) {
        receipt
    } else {
        html! {
            section.oa-card aria-labelledby="event-title" {
                (MarkdownRoot::new(html! {
                    h1 id="event-title" { "Signed event" }
                    p {
                        "Kind " (event.kind) ", signed by "
                        a href=(super::href(&event.pubkey, None)) { (super::short_npub(&event.pubkey)) }
                        ". Its signature checks out."
                    }
                    pre { code { (pretty) } }
                }))
            }
        }
    };
    let mut response = UiPage::new("Signed event")
        .path(format!("/event/{id}"))
        .scriptless()
        .content(PageColumn::new(content))
        .respond(&headers);
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        "no-store, private".parse().unwrap(),
    );
    response
}
