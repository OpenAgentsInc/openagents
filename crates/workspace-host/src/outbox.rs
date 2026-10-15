//! Exact queued admissions. A corrupt request keeps its identity as a tombstone.
use crate::{
    Result,
    artifact::{self, Artifacts, MAX_BYTES},
};
use nostr::{
    contracts,
    domain::{Event, RelaySigner},
    workspace,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};
const BUNDLE_LIMIT: usize = 24 * MAX_BYTES;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    pub attachments: BTreeMap<String, Vec<u8>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    fingerprint: String,
    state: String,
    receipt: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    entries: BTreeMap<String, Entry>,
}
pub struct Outbox {
    store: Artifacts,
    secret: SecretKey,
    signer: RelaySigner,
}
impl Outbox {
    pub fn open(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let store = Artifacts::new(root)?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|_| "invalid outbox key")?;
        let value = Self {
            store,
            secret,
            signer,
        };
        let _guard = value.store.lock()?;
        if !value.store.root().join("state.json").exists() {
            value.save(&Book {
                v: "openagents.workspace-outbox.v1".into(),
                entries: BTreeMap::new(),
            })?;
        }
        drop(_guard);
        Ok(value)
    }
    fn book(&self) -> Result<Book> {
        let book: Book = self.store.load(&self.signer)?;
        if book.v != "openagents.workspace-outbox.v1" {
            return Err("unsupported outbox version".into());
        }
        Ok(book)
    }
    fn save(&self, book: &Book) -> Result<()> {
        self.store.save(book, &self.signer)
    }
    fn identity(&self, bundle: &Bundle) -> Result<(String, String, Value)> {
        if bundle.event.pubkey != self.signer.pubkey() {
            return Err("outbox signer differs".into());
        }
        let envelope = nostr::private_artifact::open(&bundle.event, &self.secret)
            .map_err(|e| e.to_string())?;
        if envelope.artifact().schema.as_deref() != Some("openagents.workspace-action-admission.v1")
        {
            return Err("outbox accepts only durable workspace admissions".into());
        }
        let admission =
            workspace::parse(envelope.inline_bytes().ok_or("inline admission required")?)
                .map_err(|e| e.to_string())?;
        if admission["actor"] != self.signer.pubkey() {
            return Err("outbox actor differs".into());
        }
        let r = contracts::parse_artifact(&admission["proposal"]).map_err(|e| e.to_string())?;
        let bytes = bundle
            .attachments
            .get(&r.digest)
            .ok_or("queued proposal bytes are required")?;
        contracts::check_artifact_bytes(&r, bytes).map_err(|e| e.to_string())?;
        let proposal = workspace::parse(bytes).map_err(|e| e.to_string())?;
        if !matches!(
            proposal["v"].as_str(),
            Some("openagents.resource-change.v1")
        ) {
            return Err("this queue accepts conditional document changes; restoration and live input require fresh admission".into());
        }
        if bundle.attachments.len() > 64
            || bundle.attachments.values().map(Vec::len).sum::<usize>() > 4 * MAX_BYTES
        {
            return Err("queued request exceeds attachment bounds".into());
        }
        for (digest, bytes) in &bundle.attachments {
            if contracts::digest_bytes(bytes) != *digest {
                return Err("queued attachment digest differs".into());
            }
        }
        let command = proposal["command"].as_str().ok_or("queued command")?;
        let key = contracts::digest_bytes(
            format!(
                "{}:{}:{command}",
                envelope.recipient(),
                self.signer.pubkey()
            )
            .as_bytes(),
        )[7..]
            .to_string();
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&admission).map_err(|e| e.to_string())?);
        Ok((key, fingerprint, admission))
    }
    pub fn enqueue(&self, bundle: &Bundle) -> Result<String> {
        let (key, fingerprint, _) = self.identity(bundle)?;
        let _guard = self.store.lock()?;
        let mut book = self.book()?;
        if let Some(entry) = book.entries.get(&key) {
            if entry.fingerprint != fingerprint {
                return Err("idempotency_conflict: queued command bytes differ".into());
            }
            if entry.state == "quarantined" {
                return Err("queued command was quarantined; its identity was retained".into());
            }
            return Ok(key);
        }
        if book.entries.len() >= 4096 {
            return Err("outbox retention bound exceeded".into());
        }
        let bytes = serde_json::to_vec(bundle).map_err(|e| e.to_string())?;
        if bytes.len() > BUNDLE_LIMIT {
            return Err("encoded queue byte limit exceeded".into());
        }
        artifact::atomic(&self.store.root().join(format!("{key}.json")), &bytes)?;
        book.entries.insert(
            key.clone(),
            Entry {
                fingerprint,
                state: "pending".into(),
                receipt: None,
            },
        );
        self.save(&book)?;
        Ok(key)
    }
    pub fn retry(&self, key: &str) -> Result<Bundle> {
        identity_key(key)?;
        let _guard = self.store.lock()?;
        let mut book = self.book()?;
        let entry = book.entries.get(key).ok_or("queued command unavailable")?;
        if entry.state != "pending" {
            return Err("queued command requires review or is already recorded".into());
        }
        let fingerprint = entry.fingerprint.clone();
        let path = self.store.root().join(format!("{key}.json"));
        let read = (|| {
            let value = contracts::parse_strict_bounded(
                &artifact::read(&path, BUNDLE_LIMIT)?,
                BUNDLE_LIMIT,
            )
            .map_err(|e| e.to_string())?;
            let bundle: Bundle =
                serde_json::from_value(value).map_err(|_| "queued request is unreadable")?;
            let (actual, actual_fingerprint, _) = self.identity(&bundle)?;
            if actual != key || actual_fingerprint != fingerprint {
                return Err("queued request identity differs".into());
            }
            Ok(bundle)
        })();
        if read.is_err() {
            if path.exists() {
                std::fs::rename(
                    &path,
                    self.store
                        .root()
                        .join(format!("quarantined-{key}-{}.json", artifact::token())),
                )
                .map_err(|e| e.to_string())?;
            }
            book.entries.get_mut(key).ok_or("queued command")?.state = "quarantined".into();
            self.save(&book)?;
        }
        read
    }
    /// Store only a host-authenticated result that pins this exact wrapper.
    /// Unknown effects require reconciliation rather than a new command ID.
    pub fn acknowledge(&self, key: &str, event: &Event) -> Result<()> {
        let bundle = self.retry(key)?;
        let (_, _, admission) = self.identity(&bundle)?;
        let host = nostr::private_artifact::open(&bundle.event, &self.secret)
            .map_err(|e| e.to_string())?
            .recipient()
            .to_string();
        let envelope =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if envelope.signer() != host || envelope.recipient() != self.signer.pubkey() {
            return Err("outbox result has another host or recipient".into());
        }
        let result = workspace::parse(
            envelope
                .inline_bytes()
                .ok_or("inline command result required")?,
        )
        .map_err(|e| e.to_string())?;
        if result["v"] != "openagents.resource-change-result.v1"
            || result["request"] != artifact::json_ref(&admission)?
        {
            return Err("outbox result names another command".into());
        }
        let _guard = self.store.lock()?;
        let mut book = self.book()?;
        let entry = book.entries.get_mut(key).ok_or("queued command")?;
        entry.state = if result["outcome"] == "unknown" {
            "reconcile"
        } else {
            "recorded"
        }
        .into();
        entry.receipt = Some(result);
        self.save(&book)
    }
}
fn identity_key(key: &str) -> Result<()> {
    if key.len() != 64
        || !key
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err("invalid queued command identity".into());
    }
    Ok(())
}
