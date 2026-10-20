//! Published plugins as NIP-EXT listings (kind 30184) say they are: the
//! newest listing per publisher and slug whose state is `published`,
//! newest first. The `openagents` CLI's `search` and the website's
//! `/explore` (#12259) both read plugins through [`published`], so they
//! list the same ones.

use std::collections::BTreeMap;

use nostr::domain::Event;
use serde_json::{Value, json};

/// The `t` tag every listing carries.
pub const LISTING_MARKER: &str = "oa:ext:listing:v1";

/// The hosted runner's sample plugins (`deploy/eval-runner/catalog`), their
/// package records compiled in. They are its test fixtures: listings never
/// show them, and install finds one only when named exactly (#10307).
pub const SAMPLES: [&str; 6] = [
    include_str!("../../plugin-repo-map/package.json"),
    include_str!("../../plugin-code-search/package.json"),
    include_str!("../../plugin-test-report/package.json"),
    include_str!("../../plugin-explain-error/package.json"),
    include_str!("../../plugin-release-notes/package.json"),
    include_str!("../../plugin-dependency-check/package.json"),
];

/// One published plugin, from its newest listing.
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    /// `<publisher>:<slug>`.
    pub package: String,
    pub slug: String,
    pub publisher: String,
    pub title: String,
    pub description: String,
    /// The release the listing points at, an EventRef.
    pub release: Value,
    /// Blob servers the publisher named.
    pub blobs: Vec<String>,
    pub created_at: u64,
}

/// The relay filter for listings, by `author` and `slug` when given.
#[must_use]
pub fn filter(author: Option<&str>, slug: Option<&str>, limit: u64) -> Value {
    let mut filter = json!({
        "kinds": [nostr::ext::LISTING_KIND],
        "#t": [LISTING_MARKER],
        "limit": limit,
    });
    if let Some(author) = author {
        filter["authors"] = json!([author]);
    }
    if let Some(slug) = slug {
        filter["#d"] = json!([slug]);
    }
    filter
}

/// The published listings among `events`, newest per publisher and slug,
/// newest first. Events that fail NIP-EXT's checks don't count.
#[must_use]
pub fn published(events: Vec<Event>) -> Vec<Listing> {
    let mut newest: BTreeMap<(String, String), Event> = BTreeMap::new();
    for event in events {
        if event.kind != nostr::ext::LISTING_KIND {
            continue;
        }
        let Some(d) = event.tag_values("d").next().map(str::to_owned) else {
            continue;
        };
        let key = (event.pubkey.clone(), d);
        if newest
            .get(&key)
            .is_none_or(|kept| (event.created_at, &event.id) > (kept.created_at, &kept.id))
        {
            newest.insert(key, event);
        }
    }
    let mut out = Vec::new();
    for ((publisher, slug), event) in newest {
        let Ok(body) = nostr::ext::parse_record(&event) else {
            continue;
        };
        if body["state"] != "published" {
            continue;
        }
        out.push(Listing {
            package: body["package"].as_str().unwrap_or_default().to_owned(),
            slug,
            publisher,
            title: body["title"].as_str().unwrap_or_default().to_owned(),
            description: body["description"].as_str().unwrap_or_default().to_owned(),
            release: body["release"].clone(),
            blobs: body["meta"]["blobs"]
                .as_array()
                .map(|bases| {
                    bases
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|base| base.starts_with("https://") || base.starts_with("http://"))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            created_at: event.created_at,
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

/// The sample plugins' packages, `<publisher>:<slug>`.
#[must_use]
pub fn sample_packages() -> Vec<String> {
    SAMPLES
        .iter()
        .filter_map(|text| serde_json::from_str::<Value>(text).ok())
        .filter_map(|record| {
            Some(format!(
                "{}:{}",
                record["publisher"].as_str()?,
                record["slug"].as_str()?
            ))
        })
        .collect()
}

/// `listings` without the sample plugins.
#[must_use]
pub fn without_samples(listings: Vec<Listing>) -> Vec<Listing> {
    let samples = sample_packages();
    listings
        .into_iter()
        .filter(|listing| !samples.contains(&listing.package))
        .collect()
}
