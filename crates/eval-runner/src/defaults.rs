//! Coder's defaults as the hosted runner admits them: the newest
//! `coder-defaults` release on the relay, read under the same checks a
//! ledger makes (`xp_ledger::defaults`), and the admitted extensions this
//! runner holds in its catalog, resolved like any subject.
//!
//! A run admits the defaults in both arms, so its report is marginal: the
//! candidate on top of the current defaults against the current defaults
//! alone (`docs/extensions/evaluation.md`, "Checks, adoption, and
//! credit"). The report's `meta.ext_eval.defaults` names the release, and
//! each arm's lock names the defaults lock and every admitted program and
//! skill by digest. An admitted extension the catalog doesn't hold is
//! named as missing and admits nothing; the run still records the release
//! it read.
//!
//! The documents a release pins (its manifest and the admissions it cites)
//! are read from the operator's documents directory first
//! (`~/.openagents/coder-defaults/documents`, where `microcoder xp adopt`
//! keeps them on the referee host), then from the NIP-94 locators the
//! adopter published, each checked against its digest.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ext_eval::arms;
use nostr::contracts;
use nostr::domain::Event;
use nostr::eval_ext;
use serde_json::{Value, json};
use xp_ledger::defaults::{self, Defaults};
use xp_ledger::eval::Documents;

use crate::catalog::Catalog;
use crate::wire::Wire;

/// The most releases of the package read in one pass.
const LIMIT: usize = 100;

/// One reading of the defaults.
#[derive(Clone, Debug, Default)]
pub struct Read {
    /// The defaults as the ledger reads them; `None` when the relay holds
    /// no release whose manifest is available.
    pub defaults: Option<Defaults>,
    /// What both arms admit: the release, its lock, and the admitted
    /// extensions the catalog holds. `None` when there are no defaults.
    pub arms: Option<arms::Defaults>,
    /// Admitted extensions' release IDs the catalog doesn't hold.
    pub missing: Vec<String>,
}

impl Read {
    /// One line for the log: the release, what's admitted, and what isn't.
    #[must_use]
    pub fn line(&self, names: &BTreeMap<String, String>) -> String {
        let Some(defaults) = &self.defaults else {
            return "defaults none published; the baseline arm admits nothing".to_string();
        };
        let admitted: Vec<String> = defaults
            .admitted
            .iter()
            .map(|a| {
                names
                    .get(&a.subject.id)
                    .cloned()
                    .unwrap_or_else(|| short(&a.subject.id).to_string())
            })
            .collect();
        let mut line = format!(
            "defaults release {} (version {}) admits {}",
            short(&defaults.release.id),
            defaults.version,
            if admitted.is_empty() {
                "nothing".to_string()
            } else {
                admitted.join(", ")
            }
        );
        if !self.missing.is_empty() {
            line.push_str(&format!(
                "; not held here: {}",
                self.missing
                    .iter()
                    .map(|id| short(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !defaults.lapsed.is_empty() {
            line.push_str(&format!(
                "; lapsed: {}",
                defaults
                    .lapsed
                    .iter()
                    .map(|id| short(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        line
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

/// Every document in `dir`, by digest. A missing directory holds none.
fn read_documents(dir: Option<&Path>) -> Documents {
    let Some(dir) = dir else {
        return Documents::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Documents::new();
    };
    xp_ledger::eval::documents(
        entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .filter_map(|p| std::fs::read(p).ok()),
    )
}

/// Fetches one document a NIP-94 locator names over HTTPS, checking its
/// digest. Built, used, and dropped on a blocking thread.
async fn fetch(url: String, digest: String) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .ok()?;
        let response = client.get(&url).send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().ok()?;
        (bytes.len() <= xp_ledger::eval::MAX_DOCUMENT_BYTES
            && contracts::digest_bytes(&bytes) == digest)
            .then(|| bytes.to_vec())
    })
    .await
    .ok()
    .flatten()
}

/// The documents `releases` pin: those in `dir`, then those the relay's
/// locators point to.
async fn documents_for(wire: &dyn Wire, releases: &[Event], dir: Option<&Path>) -> Documents {
    let mut documents = read_documents(dir);
    for _ in 0..2 {
        let missing: BTreeSet<String> = releases
            .iter()
            .flat_map(|r| defaults::wanted_digests(r, &documents))
            .filter(|d| !documents.contains_key(d))
            .collect();
        if missing.is_empty() {
            break;
        }
        let hexes: Vec<&str> = missing
            .iter()
            .map(|d| d.trim_start_matches("sha256:"))
            .collect();
        let locators = wire
            .query(json!({"kinds": [nostr::ext::LOCATOR_KIND], "#x": hexes, "limit": LIMIT}))
            .await
            .unwrap_or_default();
        for locator in locators {
            let (Some(url), Some(hex)) = (
                locator.tag_values("url").next(),
                locator.tag_values("x").next(),
            ) else {
                continue;
            };
            let digest = format!("sha256:{hex}");
            if documents.contains_key(&digest) || !missing.contains(&digest) {
                continue;
            }
            if let Some(bytes) = fetch(url.to_string(), digest.clone()).await {
                documents.insert(digest, bytes);
            }
        }
    }
    documents
}

/// Reads the defaults of `<root>:coder-defaults` from `wire` as of `now`,
/// and resolves the admitted extensions against `catalog`, where
/// `releases` maps each catalog tool's definition ID to its release
/// (`{id, pubkey, kind}`).
///
/// # Errors
///
/// When the relay can't be read.
pub async fn read(
    wire: &dyn Wire,
    root: &str,
    documents_dir: Option<&Path>,
    catalog: &Catalog,
    releases: &BTreeMap<String, Value>,
    now: u64,
) -> Result<Read, String> {
    let package = xp_ledger::adopt::package_of(root);
    let events = wire
        .query(json!({"kinds": [nostr::ext::RELEASE_KIND], "authors": [root], "limit": LIMIT}))
        .await?;
    let documents = documents_for(wire, &events, documents_dir).await;
    let Some(defaults) = defaults::current(&events, &package, &documents, now) else {
        return Ok(Read::default());
    };
    let mut subjects = Vec::new();
    let mut missing = Vec::new();
    for admitted in &defaults.admitted {
        let held = catalog.tools.iter().find(|tool| {
            releases
                .get(&tool.definition.id)
                .and_then(|r| r["id"].as_str())
                == Some(admitted.subject.id.as_str())
        });
        match held {
            Some(tool) => subjects.push(tool.subject.clone()),
            None => missing.push(admitted.subject.id.clone()),
        }
    }
    let lock = defaults::lock_document(&defaults);
    let arms = arms::Defaults {
        release: defaults.pointer(),
        lock,
        subjects,
    };
    Ok(Read {
        defaults: Some(defaults),
        arms: Some(arms),
        missing,
    })
}

/// The `{id, pubkey, kind}` of a release event.
#[must_use]
pub fn pointer(event: &Event) -> Value {
    json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind})
}

/// Whether `report` says it was a marginal run under `release`.
#[must_use]
pub fn names_defaults(report: &eval_ext::Report, release: &str) -> bool {
    report
        .profile
        .defaults
        .as_ref()
        .is_some_and(|d| d.id == release)
}
