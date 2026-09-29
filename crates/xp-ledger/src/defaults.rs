//! What Coder's defaults admit right now: the newest valid
//! `coder-defaults` release, and each dependency of its manifest that a
//! live `openagents.eval-admission.v1` decision admits.
//!
//! A runtime that consumes the defaults (the hosted runner, `coder -p`,
//! a resident host) reads the releases signed by the package's root and
//! the documents they pin, asks [`current`] which extensions are admitted
//! as of now, and records the answer in the run as a [`lock_document`].
//! Every check the ledger makes of an adoption is made here too: the
//! release is a valid NIP-EXT release of the package, its manifest bytes
//! are the ones it pins, the admission's bytes are the ones the manifest
//! cites, the admission admits, names the dependency as its subject, and
//! hasn't expired. A dependency without such an admission is named as
//! `lapsed` and admits nothing. Nothing here fetches: the caller holds
//! the events and the bytes ([`wanted_digests`] says which bytes).

use serde::Serialize;
use serde_json::{Value, json};

use nostr::contracts;
use nostr::domain::Event;
use nostr::eval_ext::{self, EventPointer};
use nostr::{ext, kinds};

use crate::adopt;
use crate::eval::{self, Documents};

/// The schema of the lock a run records.
pub const LOCK_SCHEMA: &str = "openagents.coder-defaults-lock.v1";

/// One extension the defaults admit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Admitted {
    /// The extension's NIP-EXT release, which the manifest depends on.
    pub subject: EventPointerRecord,
    /// The admitted component, `<pubkey>:<package>/<component>`, as the
    /// admission's subject names it.
    pub definition: String,
    /// The admission's `sha256:` digest.
    pub admission: String,
    /// When the admission lapses, Unix seconds.
    pub expires_at: u64,
}

/// An event reference as the lock writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EventPointerRecord {
    pub id: String,
    pub pubkey: String,
    pub kind: u16,
}

impl From<&EventPointer> for EventPointerRecord {
    fn from(pointer: &EventPointer) -> Self {
        Self {
            id: pointer.id.clone(),
            pubkey: pointer.pubkey.clone(),
            kind: pointer.kind,
        }
    }
}

/// The defaults as of one moment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Defaults {
    /// The newest valid release of the package.
    pub release: EventPointerRecord,
    /// The release's version label.
    pub version: String,
    /// The manifest's `sha256:` digest.
    pub manifest: String,
    /// The dependencies a live admission admits, in the manifest's order.
    pub admitted: Vec<Admitted>,
    /// Dependencies the release names that no live admission held here
    /// admits: expired, refused, not held, or never issued. They are named
    /// so a reader can tell "not admitted" from "not read".
    pub lapsed: Vec<String>,
}

impl Defaults {
    /// The release as an `{id, pubkey, kind}` value, which a report's
    /// `meta.ext_eval.defaults` carries.
    #[must_use]
    pub fn pointer(&self) -> Value {
        json!({
            "id": self.release.id,
            "pubkey": self.release.pubkey,
            "kind": self.release.kind,
        })
    }

    /// The admitted subjects' release IDs.
    #[must_use]
    pub fn subjects(&self) -> Vec<String> {
        self.admitted.iter().map(|a| a.subject.id.clone()).collect()
    }
}

/// The digests a runtime needs to read `release`: its manifest, and once
/// the manifest is held, the admissions it cites.
#[must_use]
pub fn wanted_digests(release: &Event, documents: &Documents) -> Vec<String> {
    let Ok(body) = ext::parse_record(release) else {
        return Vec::new();
    };
    let Ok(manifest) = contracts::parse_artifact(&body["manifest"]) else {
        return Vec::new();
    };
    let mut out = vec![manifest.digest.clone()];
    if let Some(bytes) = documents.get(&manifest.digest) {
        out.extend(
            adopt::receipts_of(bytes)
                .unwrap_or_default()
                .iter()
                .filter_map(|r| contracts::parse_artifact(r).ok())
                .map(|a| a.digest),
        );
    }
    out
}

/// The defaults as of `now`: the newest valid release of `package`
/// (`<root>:coder-defaults`) among `events` whose manifest `documents`
/// holds, with each dependency admitted only by a live admission held in
/// `documents`. `None` when no valid release is held.
#[must_use]
pub fn current(
    events: &[Event],
    package: &str,
    documents: &Documents,
    now: u64,
) -> Option<Defaults> {
    let releases = eval::defaults_releases(events, package, documents);
    let (event, parsed) = releases.last()?;
    let (manifest, admissions) = eval::adoption_documents(event, documents).ok()?;
    let admissions: Vec<(String, eval_ext::Admission)> = admissions
        .into_iter()
        .filter_map(|bytes| {
            eval_ext::parse_admission(bytes)
                .ok()
                .map(|a| (contracts::digest_bytes(bytes), a))
        })
        .collect();
    let mut admitted = Vec::new();
    let mut lapsed = Vec::new();
    for dependency in &parsed.manifest.dependencies {
        let live = admissions.iter().find(|(_, a)| {
            a.decision == "admit"
                && a.expires_at > now
                && a.subject
                    .event
                    .as_ref()
                    .is_some_and(|e| e.id == *dependency)
        });
        match live {
            Some((digest, admission)) => {
                let event = admission
                    .subject
                    .event
                    .as_ref()
                    .expect("a live admission names its subject's release");
                admitted.push(Admitted {
                    subject: EventPointerRecord {
                        id: event.id.clone(),
                        pubkey: event.pubkey.clone(),
                        kind: kinds::EXT_RELEASE,
                    },
                    definition: admission.subject.id.clone(),
                    admission: digest.clone(),
                    expires_at: admission.expires_at,
                });
            }
            None => lapsed.push(dependency.clone()),
        }
    }
    Some(Defaults {
        release: EventPointerRecord {
            id: event.id.clone(),
            pubkey: event.pubkey.clone(),
            kind: event.kind,
        },
        version: parsed.manifest.version.clone(),
        manifest: contracts::digest_bytes(manifest),
        admitted,
        lapsed,
    })
}

/// The lock a run records for the defaults it admitted, as exact bytes
/// (JCS): the release, its manifest digest, and each admitted subject
/// with the admission that admits it and when that lapses.
///
/// # Panics
///
/// Never: the document is built from checked values.
#[must_use]
pub fn lock_document(defaults: &Defaults) -> Vec<u8> {
    let value = json!({
        "v": LOCK_SCHEMA,
        "requires": [],
        "release": defaults.pointer(),
        "version": defaults.version,
        "manifest": defaults.manifest,
        "admitted": defaults.admitted,
        "lapsed": defaults.lapsed,
    });
    contracts::jcs(&value).expect("a lock built from checked values canonicalizes")
}

/// Reads a lock a run recorded back into its [`Defaults`].
///
/// # Errors
///
/// When the bytes aren't a lock of this schema.
pub fn parse_lock(bytes: &[u8]) -> Result<Defaults, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if value["v"].as_str() != Some(LOCK_SCHEMA) {
        return Err(format!("not a {LOCK_SCHEMA} document"));
    }
    let pointer = |v: &Value| -> Result<EventPointerRecord, String> {
        Ok(EventPointerRecord {
            id: v["id"].as_str().ok_or("an event id")?.to_owned(),
            pubkey: v["pubkey"].as_str().ok_or("an event pubkey")?.to_owned(),
            kind: u16::try_from(v["kind"].as_u64().ok_or("an event kind")?)
                .map_err(|_| "an event kind")?,
        })
    };
    let admitted = value["admitted"]
        .as_array()
        .ok_or("admitted")?
        .iter()
        .map(|a| {
            Ok(Admitted {
                subject: pointer(&a["subject"])?,
                definition: a["definition"].as_str().ok_or("definition")?.to_owned(),
                admission: a["admission"].as_str().ok_or("admission")?.to_owned(),
                expires_at: a["expires_at"].as_u64().ok_or("expires_at")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Defaults {
        release: pointer(&value["release"])?,
        version: value["version"].as_str().unwrap_or_default().to_owned(),
        manifest: value["manifest"].as_str().ok_or("manifest")?.to_owned(),
        admitted,
        lapsed: value["lapsed"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    })
}
