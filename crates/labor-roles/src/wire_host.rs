//! A persistent participant speaking the canonical NIP-LAB wire format.
use crate::{
    Result,
    book::{Applied, Book},
    terms::Terms,
    wire::{Admission, Bytes},
};
use nostr::{
    contracts,
    domain::{Event, RelaySigner},
    private_artifact,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use workspace_host::artifact::{Artifacts, Lock, token};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    event: Event,
    observed_at: u64,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    config: Value,
    admitted_at: u64,
    lines: Vec<Line>,
    bytes: Bytes,
    funding: Option<(String, String)>,
}
pub struct Host {
    store: Artifacts,
    signer: RelaySigner,
    secret: SecretKey,
    admission: Admission,
    book: Book,
    journal: Journal,
    poisoned: bool,
    _owner: Lock,
}
impl Host {
    /// The admission can only be made through a confirmed MKT negotiation.
    /// Each participant keeps its own key, private state, and writer lock.
    pub fn open(
        root: PathBuf,
        secret: SecretKey,
        terms: Terms,
        admission: Admission,
        now: u64,
    ) -> Result<Self> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        if !terms
            .rights
            .recipients
            .contains(&signer.pubkey().to_string())
        {
            return Err("this key is not an admitted order recipient".into());
        }
        let store = Artifacts::new(root)?;
        let owner = store.lock()?;
        let marker = store.root().join("wire-host.initialized");
        if marker.exists() && !store.root().join("state.json").exists() {
            return Err("the initialized wire journal is missing; history cannot restart".into());
        }
        if store.root().join("journal.json").exists() {
            return Err("canonical wire records need their own participant directory".into());
        }
        let config = json!({"terms":terms,"admission":admission});
        let journal: Journal = if store.root().join("state.json").exists() {
            store.load(&signer)?
        } else {
            Journal {
                config: config.clone(),
                admitted_at: now,
                ..Default::default()
            }
        };
        if journal.config != config {
            return Err("the wire host belongs to different frozen terms".into());
        }
        let mut book = Book::new(terms, journal.admitted_at)?;
        for line in &journal.lines {
            let opened = private_artifact::open(&line.event, &secret).map_err(|e| e.to_string())?;
            admission.ingest(
                &mut book,
                &opened,
                signer.pubkey(),
                &journal.bytes,
                line.observed_at,
            )?;
        }
        if let Some((acceptance, payment)) = journal.funding.clone() {
            book.restore_funding(acceptance, payment);
        }
        let mut host = Self {
            store,
            signer,
            secret,
            admission,
            book,
            journal,
            poisoned: false,
            _owner: owner,
        };
        host.save()?;
        workspace_host::artifact::atomic(&marker, b"openagents.lab-wire-host.v1")?;
        Ok(host)
    }
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }
    pub fn book(&self) -> &Book {
        &self.book
    }
    pub fn bytes(&self) -> &Bytes {
        &self.journal.bytes
    }
    fn check(&self) -> Result<()> {
        if self.poisoned {
            return Err("the wire host needs reopening after a failed write".into());
        }
        Ok(())
    }
    fn save(&mut self) -> Result<()> {
        self.check()?;
        self.poisoned = true;
        if self.journal.lines.len() > 4096
            || self.journal.bytes.len() > 4096
            || serde_json::to_vec(&self.journal)
                .map_err(|e| e.to_string())?
                .len()
                > 16 * 1_048_576
        {
            return Err("the wire host exceeds its retention limit".into());
        }
        self.store.save(&self.journal, &self.signer)?;
        self.poisoned = false;
        Ok(())
    }
    fn proposed(&self, artifacts: &Bytes) -> Result<Bytes> {
        let mut bytes = self.journal.bytes.clone();
        for (digest, raw) in artifacts {
            if contracts::digest_bytes(raw) != *digest {
                return Err("wire attachment digest differs".into());
            }
            bytes.insert(digest.clone(), raw.clone());
        }
        if bytes.len() > 4096 || bytes.values().map(Vec::len).sum::<usize>() > 8 * 1_048_576 {
            return Err("wire attachment byte limit".into());
        }
        Ok(bytes)
    }
    /// Check and save an original private declaration before acknowledging it.
    pub fn receive(&mut self, event: &Event, artifacts: &Bytes, now: u64) -> Result<Applied> {
        self.check()?;
        let opened = private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        let mut bytes = self.proposed(artifacts)?;
        let raw = opened
            .inline_bytes()
            .ok_or("wire record must be inline")?
            .to_vec();
        bytes.insert(opened.artifact().digest.clone(), raw);
        let mut book = self.book.clone();
        let applied = self
            .admission
            .ingest(&mut book, &opened, self.pubkey(), &bytes, now)?;
        if matches!(applied, Applied::New(_)) {
            self.journal.lines.push(Line {
                event: event.clone(),
                observed_at: now,
            });
            self.journal.bytes = bytes;
            self.book = book;
            self.save()?;
        }
        Ok(applied)
    }
    fn seal(&self, wire: &Value, recipient: &str, now: u64) -> Result<Event> {
        let raw = contracts::jcs(wire).map_err(|e| e.to_string())?;
        let reference =
            workspace_host::artifact::reference(&raw, "application/json", wire["v"].as_str());
        let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference,"inline":wire,
            "issued_at":now,"retain_until":self.book.terms.retain_until});
        private_artifact::seal(
            &body,
            &self.secret,
            &recipient.parse().map_err(|_| "wire recipient key")?,
            &token(),
            now,
            secp256k1::rand::random(),
        )
        .map_err(|e| e.to_string())
    }
    /// Save this participant's canonical record first, then seal the same
    /// plaintext independently to the other admitted roles. A failed save
    /// returns no events; retry never invents another record identity.
    pub fn issue(&mut self, wire: &Value, artifacts: &Bytes, now: u64) -> Result<Vec<Event>> {
        self.check()?;
        if wire["issuer"] != self.pubkey() {
            return Err("the wire issuer is not this participant".into());
        }
        let original = self.seal(wire, self.pubkey(), now)?;
        self.receive(&original, artifacts, now)?;
        let t = &self.book.terms;
        let mut recipients = vec![&t.buyer, &t.provider, &t.worker, &t.reviewer, &t.resolver];
        recipients.sort();
        recipients.dedup();
        recipients
            .into_iter()
            .filter(|r| r.as_str() != self.pubkey())
            .map(|recipient| self.seal(wire, recipient, now))
            .collect()
    }
    /// Collection is a separate central receive receipt, never an effect of a
    /// passing check, review, or this host's publication.
    pub fn fund(
        &mut self,
        ledger: &mut pay_ledger::Ledger,
        pins: &BTreeMap<&str, String>,
        receipt: &pay_ledger::markets::worker::FundingReceipt,
    ) -> Result<pay_ledger::Recorded> {
        self.check()?;
        let recorded = self.book.fund(ledger, pins, receipt)?;
        self.journal.funding = self.book.funding().cloned();
        self.save()?;
        Ok(recorded)
    }
}
