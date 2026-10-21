//! The patch viewer (`review.html`): NIP-34 patches to the configured
//! repository, read back from the relay, with their diffs, the runner's
//! check comments, other NIP-22 comments, and the newest valid status.

use std::collections::BTreeSet;

use nostr::domain::Event;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;
use web_sys::Document;

use super::relay::Relay;
use super::util::random32;
use crate::cj::Key;

fn el(doc: &Document, tag: &str, class: &str, text: &str) -> web_sys::Element {
    let node = doc
        .create_element(tag)
        .unwrap_or_else(|_| wasm_bindgen::throw_str("no element"));
    if !class.is_empty() {
        node.set_class_name(class);
    }
    if !text.is_empty() {
        node.set_text_content(Some(text));
    }
    node
}

fn short(npub_or_hex: &str) -> String {
    let n = npub_or_hex.len();
    if n > 20 {
        format!("{}…{}", &npub_or_hex[..12], &npub_or_hex[n - 6..])
    } else {
        npub_or_hex.to_string()
    }
}

fn npub(hex: &str) -> String {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2).unwrap_or("00"), 16).unwrap_or(0);
    }
    nostr::nip19::encode_npub(&bytes)
}

fn when(unix: u64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64(unix as f64 * 1000.0));
    String::from(d.to_iso_string()).replace('T', " ")[..16].to_string()
}

pub async fn run() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(doc) = window.document() else {
        return;
    };
    let Some(root) = doc.get_element_by_id("patches") else {
        return;
    };
    let status = doc.get_element_by_id("status");
    let say = |text: &str| {
        if let Some(status) = &status {
            status.set_text_content(Some(text));
        }
    };
    let config: Value = match super::app::fetch_json("config.json").await {
        Ok(config) => config,
        Err(e) => {
            say(&format!("No config.json: {e}"));
            return;
        }
    };
    let repo = config["repo"].as_str().unwrap_or_default().to_string();
    let relay_url = config["relay"]
        .as_str()
        .unwrap_or("wss://relay.openagents.com")
        .to_string();
    if repo.is_empty() {
        say("No repository is configured; kiln-dev names it with --repo.");
        return;
    }
    say(&format!("Reading patches to {repo} from {relay_url}…"));
    let Ok(key) = Key::from_bytes(random32()) else {
        return;
    };
    let mut relay = match Relay::connect(&relay_url, &key).await {
        Ok(relay) => relay,
        Err(e) => {
            say(&e);
            return;
        }
    };
    let owner = repo.split(':').nth(1).unwrap_or_default().to_string();
    let identifier = repo.split(':').nth(2).unwrap_or_default().to_string();
    // Maintainers: the announcement's author and its `maintainers` tag.
    let mut maintainers: BTreeSet<String> = BTreeSet::from([owner.clone()]);
    let mut title = identifier.clone();
    if let Ok(found) = relay
        .query(
            "repo",
            &json!({"kinds": [30617], "authors": [owner], "#d": [identifier], "limit": 1}),
        )
        .await
        && let Some(event) = found.iter().max_by_key(|e| e.created_at)
        && let Ok(announced) = nostr::domain::open_repository(event)
    {
        maintainers.extend(announced.maintainers);
        title = announced.name.unwrap_or(title);
    }
    if let Some(h) = doc.get_element_by_id("title") {
        h.set_text_content(Some(&format!("Patches to {title}")));
    }
    let patches = match relay
        .query(
            "patches",
            &json!({"kinds": [1617, 1618], "#a": [repo], "limit": 50}),
        )
        .await
    {
        Ok(patches) => patches,
        Err(e) => {
            say(&e);
            return;
        }
    };
    let ids: Vec<String> = patches.iter().map(|p| p.id.clone()).collect();
    let replies = if ids.is_empty() {
        Vec::new()
    } else {
        relay
            .query(
                "replies",
                &json!({"kinds": [1111, 1630, 1631, 1632, 1633], "#e": ids, "limit": 500}),
            )
            .await
            .unwrap_or_default()
    };
    let wanted = window
        .location()
        .hash()
        .unwrap_or_default()
        .trim_start_matches('#')
        .to_string();
    let mut patches = patches;
    patches.sort_by_key(|p| std::cmp::Reverse(p.created_at));
    say(&format!(
        "{} patches; checks and status count only from the repository's maintainers ({}).",
        patches.len(),
        maintainers.len()
    ));
    for patch in &patches {
        render(
            &doc,
            &root,
            patch,
            &replies,
            &maintainers,
            patch.id == wanted,
        );
    }
    let _ = js_sys::Reflect::set(
        &window,
        &"kilnReviewed".into(),
        &JsValue::from(patches.len() as u32),
    );
}

fn render(
    doc: &Document,
    root: &web_sys::Element,
    patch: &Event,
    replies: &[Event],
    maintainers: &BTreeSet<String>,
    open: bool,
) {
    let card = el(doc, "details", "patch", "");
    let _ = card.set_attribute("id", &patch.id);
    if open {
        let _ = card.set_attribute("open", "");
    }
    let about: Vec<&Event> = replies
        .iter()
        .filter(|r| r.tag_values("e").any(|e| e == patch.id))
        .collect();
    let status = about
        .iter()
        .filter(|r| (1630..=1633).contains(&r.kind))
        .filter(|r| maintainers.contains(&r.pubkey) || r.pubkey == patch.pubkey)
        .max_by_key(|r| r.created_at)
        .map_or("open", |r| match r.kind {
            1631 => "merged",
            1632 => "closed",
            1633 => "draft",
            _ => "open",
        });
    let check = about
        .iter()
        .filter(|r| maintainers.contains(&r.pubkey))
        .filter_map(|r| crate::patch::read_check(r).map(|c| (r.created_at, c)))
        .max_by_key(|(at, _)| *at)
        .map(|(_, c)| c);
    let summary = el(doc, "summary", "", "");
    let subject = if patch.kind == 1617 {
        crate::patch::subject(&patch.content)
    } else {
        patch
            .tag_values("subject")
            .next()
            .unwrap_or("pull request")
            .to_string()
    };
    let _ = summary.append_child(&el(doc, "span", "subject", &subject));
    let _ = summary.append_child(&el(doc, "span", &format!("badge {status}"), status));
    match &check {
        Some((true, _)) => {
            let _ = summary.append_child(&el(doc, "span", "badge pass", "tests pass"));
        }
        Some((false, _)) => {
            let _ = summary.append_child(&el(doc, "span", "badge fail", "tests fail"));
        }
        None => {
            let _ = summary.append_child(&el(doc, "span", "badge none", "no check"));
        }
    }
    let author = npub(&patch.pubkey);
    let crate_tag = patch
        .tag_values("t")
        .find_map(|t| t.strip_prefix("crate:"))
        .unwrap_or("");
    let _ = summary.append_child(&el(
        doc,
        "span",
        "meta",
        &format!(
            "{} · {} · {} · unverified author",
            short(&author),
            crate_tag,
            when(patch.created_at)
        ),
    ));
    let _ = card.append_child(&summary);
    if let Some((_, text)) = &check {
        let _ = card.append_child(&el(doc, "div", "check", text));
    }
    for comment in about
        .iter()
        .filter(|r| r.kind == 1111 && crate::patch::read_check(r).is_none())
    {
        let _ = card.append_child(&el(
            doc,
            "div",
            "comment",
            &format!("{}: {}", short(&npub(&comment.pubkey)), comment.content),
        ));
    }
    let found = crate::patch::secret_findings(&patch.content);
    if !found.is_empty() {
        let _ = card.append_child(&el(
            doc,
            "div",
            "warn",
            &format!("This patch seems to hold {}.", found.join(", ")),
        ));
    }
    let diff = el(doc, "pre", "diff", "");
    for line in patch.content.lines() {
        let class =
            if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") {
                "h"
            } else if line.starts_with('+') {
                "add"
            } else if line.starts_with('-') {
                "del"
            } else if line.starts_with("@@") {
                "hunk"
            } else {
                ""
            };
        let _ = diff.append_child(&el(doc, "div", class, &format!("{line} ")));
    }
    let _ = card.append_child(&diff);
    let _ = card.append_child(&el(
        doc,
        "div",
        "meta",
        &format!("event {} · apply with: git am patch.mbox", patch.id),
    ));
    let _ = root.append_child(&card);
}
