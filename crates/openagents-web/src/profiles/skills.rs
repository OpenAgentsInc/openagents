//! Published skills as relay events (NIP-SKILL; #12303).
//!
//! The account service reviews and publishes skills (`GET /v1/skills`).
//! This site is the directory that signs them: at most every
//! [`SYNC_EVERY`] it reads the published skills and puts one kind 30188
//! listing per skill on the profile relays, signed by its directory key (a
//! key derived from the site's agent key seed), with the author's key when
//! the author has one. A skill that leaves the directory gets a withdrawn
//! replacement. `/explore` then reads the listings back from the relays,
//! like any other client would.

use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use nostr::domain::{Event, RelaySigner};
use nostr::skill_listing::{self, Skill};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::App;

/// How often the directory is read and published again.
pub(crate) const SYNC_EVERY: Duration = Duration::from_secs(600);

/// The directory key: derived from the agent key seed, so it is stable and
/// nothing is stored.
pub(crate) fn directory(app: &App) -> Option<RelaySigner> {
    let seed = app.config.agent_key_seed.as_ref()?;
    for round in 0u8..8 {
        let mut mac = Hmac::<Sha256>::new_from_slice(seed).ok()?;
        mac.update(b"openagents.skill-directory.v1\0");
        mac.update(&[round]);
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if let Ok(signer) = RelaySigner::from_secret_hex(&hex) {
            return Some(signer);
        }
    }
    None
}

/// The skills an account service's `GET /v1/skills` answer publishes, with
/// each author's key.
#[must_use]
pub(crate) fn published(answer: &Value) -> Vec<(Skill, Option<String>)> {
    answer["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let latest = &entry["latest"];
            let text = |v: &Value| v.as_str().map(str::to_string);
            Some((
                Skill {
                    name: text(&entry["name"])?,
                    version: text(&latest["version"])?,
                    digest: text(&latest["digest"])?,
                    url: text(&latest["links"]["markdown"])?,
                    license: text(&latest["license"]).unwrap_or_default(),
                    category: text(&latest["category"]).unwrap_or_default(),
                    tags: latest["tags"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .take(16)
                        .collect(),
                },
                text(&entry["author_pubkey"]),
            ))
        })
        .collect()
}

/// The events that bring the relays' listings (`held`) in line with the
/// directory (`wanted`): a listing for each new or changed skill, a
/// withdrawal for each skill that left.
#[must_use]
pub(crate) fn changes(
    signer: &RelaySigner,
    wanted: &[(Skill, Option<String>)],
    held: &[Event],
    now: u64,
) -> Vec<Event> {
    let mine = vec![signer.pubkey().to_string()];
    let current = skill_listing::current(held, &mine);
    let newest_at = held.iter().map(|e| e.created_at).max().unwrap_or(0);
    // A replacement must be newer than what it replaces.
    let at = now.max(newest_at + 1);
    let mut out = Vec::new();
    for (skill, author) in wanted {
        let same = current
            .iter()
            .any(|l| l.skill == *skill && l.author.as_deref() == author.as_deref());
        if !same
            && let Ok(event) = skill_listing::listing_event(signer, skill, author.as_deref(), at)
        {
            out.push(event);
        }
    }
    for listing in &current {
        if !wanted.iter().any(|(s, _)| s.name == listing.skill.name) {
            out.push(skill_listing::withdrawn_event(
                signer,
                &listing.skill.name,
                at,
            ));
        }
    }
    out
}

fn last() -> &'static Mutex<Option<Instant>> {
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LAST.get_or_init(Mutex::default)
}

/// Read the published skills and publish what changed, at most every
/// [`SYNC_EVERY`]. Quietly does nothing without a seed, an account
/// service, or relays.
pub(crate) async fn sync(app: &App) {
    {
        let mut held = last().lock().unwrap_or_else(PoisonError::into_inner);
        if held.is_some_and(|at| at.elapsed() < SYNC_EVERY) {
            return;
        }
        *held = Some(Instant::now());
    }
    let (Some(signer), Some(service)) = (directory(app), app.config.cloud.as_deref()) else {
        return;
    };
    let Ok(answer) = service
        .nostr_call(reqwest::Method::GET, "/v1/skills?limit=100", None, None)
        .await
    else {
        return;
    };
    if answer.status != 200 {
        return;
    }
    let wanted = published(&answer.body);
    let ours: Vec<String> = app.config.profile_relays.iter().take(3).cloned().collect();
    let (held, _) = super::read_all(
        &ours,
        &json!({"kinds": [skill_listing::KIND], "authors": [signer.pubkey()], "limit": 500}),
    )
    .await;
    for event in changes(&signer, &wanted, &held, crate::cloud::session::now()) {
        for relay in &ours {
            let _ = super::relays::publish(relay, &event).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer() -> Value {
        json!({"data": [{
            "name": "repo-map",
            "author": "acct_1",
            "author_pubkey": "c".repeat(64),
            "latest": {
                "version": "1.0.0",
                "digest": format!("sha256:{}", "a".repeat(64)),
                "license": "MIT",
                "category": "coding",
                "tags": ["git"],
                "links": {"markdown": "https://openagents.com/v1/skills/repo-map/versions/1.0.0/SKILL.md"},
            },
        }]})
    }

    #[test]
    fn the_relays_follow_the_directory() {
        let signer = RelaySigner::from_secret_hex(&"07".repeat(32)).unwrap();
        let wanted = published(&answer());
        assert_eq!(wanted.len(), 1);
        let first = changes(&signer, &wanted, &[], 100);
        assert_eq!(first.len(), 1);
        // Nothing changed: nothing to publish.
        assert!(changes(&signer, &wanted, &first, 200).is_empty());
        // A new version replaces it, newer than what it replaces.
        let mut next = answer();
        next["data"][0]["latest"]["version"] = json!("1.1.0");
        let second = changes(&signer, &published(&next), &first, 50);
        assert_eq!(second.len(), 1);
        assert!(second[0].created_at > first[0].created_at);
        // A skill that left is withdrawn.
        let gone = changes(&signer, &[], &first, 300);
        assert_eq!(gone.len(), 1);
        assert!(
            gone[0]
                .tags
                .iter()
                .any(|t| t.as_slice() == ["status", "withdrawn"])
        );
    }
}
