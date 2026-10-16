//! One participant's host: its own key, its own directory and journal, and
//! the private `3188` records it seals to the other roles and receives from
//! them.
use crate::book::{Applied, Book};
use crate::records::Record;
use crate::terms::Terms;
use crate::{Result, digest_of};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Line {
    /// The original sealed event for received records; null for own ones.
    event: Option<Event>,
    record: serde_json::Value,
    signer: String,
    observed_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Journal {
    /// When this host first admitted the terms; a restart re-admits them as
    /// of that time rather than refusing an order already underway.
    admitted_at: u64,
    #[serde(default)]
    terms_digest: Option<String>,
    lines: Vec<Line>,
    funding: Option<(String, String)>,
}

pub struct Host {
    dir: PathBuf,
    secret: secp256k1::SecretKey,
    pub pubkey: String,
    pub book: Book,
    journal: Journal,
    _writer: std::fs::File,
    poisoned: bool,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Host({}, key redacted)", self.pubkey)
    }
}

fn pubkey(secret: &secp256k1::SecretKey) -> String {
    secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), secret)
        .x_only_public_key()
        .0
        .to_string()
}

impl Host {
    /// Open (or create) this participant's directory and replay its journal
    /// through the same checks.
    ///
    /// # Errors
    ///
    /// When the terms are not admitted, the key holds no role, or the
    /// journal does not replay.
    pub fn open(dir: &Path, secret: secp256k1::SecretKey, terms: Terms, now: u64) -> Result<Self> {
        let me = pubkey(&secret);
        if ![
            &terms.buyer,
            &terms.provider,
            &terms.worker,
            &terms.reviewer,
            &terms.resolver,
        ]
        .contains(&&me)
        {
            return Err("this key holds no role in the order".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let metadata = std::fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        if !metadata.is_dir() {
            return Err("the participant directory must be an ordinary private directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err("the participant directory must be private".into());
            }
        }
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let lock_path = dir.join("writer.lock");
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let (writer, fresh) = match options.create_new(true).open(&lock_path) {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let meta = std::fs::symlink_metadata(&lock_path).map_err(|e| e.to_string())?;
                if !meta.is_file() {
                    return Err("the participant lock is not an ordinary file".into());
                }
                (
                    options
                        .create_new(false)
                        .open(&lock_path)
                        .map_err(|e| e.to_string())?,
                    false,
                )
            }
            Err(e) => return Err(e.to_string()),
        };
        writer
            .try_lock()
            .map_err(|_| "the participant already has a writer")?;
        if !fresh && !dir.join("journal.json").exists() {
            return Err("the participant journal is missing; history cannot restart".into());
        }
        let terms_digest = terms.digest()?;
        let path = dir.join("journal.json");
        let journal: Journal = if path.exists() {
            use std::io::Read;
            let mut raw = Vec::new();
            std::fs::File::open(&path)
                .map_err(|e| e.to_string())?
                .take(16 * 1_048_576 + 1)
                .read_to_end(&mut raw)
                .map_err(|e| e.to_string())?;
            if raw.len() > 16 * 1_048_576 {
                return Err("the participant journal exceeds its byte limit".into());
            }
            let journal: Journal =
                serde_json::from_slice(&raw).map_err(|_| "the journal does not parse")?;
            if journal.terms_digest.as_deref() != Some(terms_digest.as_str()) {
                return Err("the participant journal belongs to different or unbound terms".into());
            }
            journal
        } else {
            Journal {
                admitted_at: now,
                terms_digest: Some(terms_digest),
                ..Journal::default()
            }
        };
        let mut book = Book::new(terms, journal.admitted_at)?;
        for line in &journal.lines {
            if let Some(event) = &line.event {
                let opened =
                    nostr::private_artifact::open(event, &secret).map_err(|e| e.to_string())?;
                if opened.signer() != line.signer || opened.recipient() != me {
                    return Err("a journal line's identity differs from its event".into());
                }
                let original: serde_json::Value = nostr::contracts::parse_strict_bounded(
                    opened.inline_bytes().ok_or("journal event is not inline")?,
                    1_048_576,
                )
                .map_err(|e| e.to_string())?;
                if original != line.record {
                    return Err("a journal line differs from its original signed bytes".into());
                }
            } else if line.signer != me {
                return Err("a received journal line has no original signed event".into());
            }
            book.apply(Record::parse(&line.record)?, &line.signer, line.observed_at)?;
        }
        if let Some((acceptance, hash)) = journal.funding.clone() {
            book.restore_funding(acceptance, hash);
        }
        let mut host = Self {
            dir,
            secret,
            pubkey: me,
            book,
            journal,
            _writer: writer,
            poisoned: false,
        };
        host.persist()?;
        Ok(host)
    }

    fn check(&self) -> Result<()> {
        if self.journal.terms_digest.as_deref() != Some(self.book.terms.digest()?.as_str()) {
            return Err("the participant terms changed after admission".into());
        }
        if self.poisoned {
            return Err("the participant journal needs reopening after a failed write".into());
        }
        let path =
            std::fs::symlink_metadata(self.dir.join("writer.lock")).map_err(|e| e.to_string())?;
        let held = self._writer.metadata().map_err(|e| e.to_string())?;
        if !path.is_file() {
            return Err("the participant writer lock changed".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if path.nlink() != 1 || path.dev() != held.dev() || path.ino() != held.ino() {
                return Err("the participant writer lock changed".into());
            }
        }
        Ok(())
    }
    fn persist(&mut self) -> Result<()> {
        self.check()?;
        self.poisoned = true;
        if self.journal.lines.len() > 4096 {
            return Err("the participant journal exceeds its record limit".into());
        }
        let path = self.dir.join("journal.json");
        let tmp = self.dir.join("journal.json.tmp");
        let bytes = serde_json::to_vec(&self.journal).map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1_048_576 {
            return Err("the participant journal exceeds its byte limit".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // A prior interrupted write can only be discarded while holding the
        // stable writer lock; the last complete journal remains untouched.
        match std::fs::remove_file(&tmp) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path)
            .and_then(|()| std::fs::File::open(&self.dir)?.sync_all())
            .map_err(|e| e.to_string())?;
        self.poisoned = false;
        Ok(())
    }

    fn recipients(&self) -> Vec<String> {
        let t = &self.book.terms;
        let mut keys: Vec<String> = [&t.buyer, &t.provider, &t.worker, &t.reviewer, &t.resolver]
            .into_iter()
            .filter(|k| **k != self.pubkey)
            .cloned()
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// Check and retain one of this participant's own records, then seal it
    /// to every other role. `nonce` seeds the per-recipient encryption.
    ///
    /// # Errors
    ///
    /// When the record breaks the terms or the records before it.
    pub fn issue(&mut self, record: &Record, now: u64, nonce: [u8; 32]) -> Result<Vec<Event>> {
        self.check()?;
        let applied = self.book.apply(record.clone(), &self.pubkey.clone(), now)?;
        if let Applied::New(_) = applied {
            self.journal.lines.push(Line {
                event: None,
                record: serde_json::to_value(record).map_err(|e| e.to_string())?,
                signer: self.pubkey.clone(),
                observed_at: now,
            });
            self.persist()?;
        }
        let inline = serde_json::to_value(record).map_err(|e| e.to_string())?;
        let bytes = nostr::contracts::jcs(&inline).map_err(|e| e.to_string())?;
        let body = serde_json::json!({
            "v": "openagents.artifact-envelope.v1", "requires": [],
            "artifact": {"digest": nostr::contracts::digest_bytes(&bytes), "size": bytes.len(),
                "media_type": "application/json", "schema": record.version()},
            "inline": inline, "issued_at": now, "retain_until": self.book.terms.retain_until,
        });
        let mut out = Vec::new();
        for (i, recipient) in self.recipients().iter().enumerate() {
            let key: secp256k1::XOnlyPublicKey =
                recipient.parse().map_err(|_| "a role key is not a key")?;
            let mut n = nonce;
            n[0] ^= u8::try_from(i).unwrap_or(0);
            out.push(
                nostr::private_artifact::seal(
                    &body,
                    &self.secret,
                    &key,
                    &self.book.terms.order,
                    now,
                    n,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(out)
    }

    /// Open, check and retain one record sealed to this participant. Events
    /// for other recipients are ignored as `None`.
    ///
    /// # Errors
    ///
    /// When it does not open, or the record breaks the terms.
    pub fn receive(&mut self, event: &Event, now: u64) -> Result<Option<Applied>> {
        self.check()?;
        if event.tag_values("p").next() != Some(self.pubkey.as_str()) {
            return Ok(None);
        }
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if opened.recipient() != self.pubkey
            || opened.body().issued_at > now.saturating_add(300)
            || opened.body().retain_until < now
        {
            return Err("the received record has a different recipient or lifetime".into());
        }
        let bytes = opened.inline_bytes().ok_or("the record is not inline")?;
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "the record is not JSON")?;
        let record = Record::parse(&value)?;
        if opened.artifact().schema.as_deref() != Some(record.version()) {
            return Err("the record schema differs from its envelope".into());
        }
        let signer = opened.signer().to_string();
        let applied = self.book.apply(record, &signer, now)?;
        if let Applied::New(_) = applied {
            self.journal.lines.push(Line {
                event: Some(event.clone()),
                record: value,
                signer,
                observed_at: now,
            });
            self.persist()?;
        }
        Ok(Some(applied))
    }

    /// Fund the accepted obligation in the ledger from a central receive
    /// adapter's receipt, and retain that in the journal.
    ///
    /// # Errors
    ///
    /// As [`Book::fund`].
    pub fn fund(
        &mut self,
        ledger: &mut pay_ledger::Ledger,
        pins: &BTreeMap<&str, String>,
        receipt: &pay_ledger::markets::worker::FundingReceipt,
    ) -> Result<pay_ledger::Recorded> {
        self.check()?;
        let recorded = self.book.fund(ledger, pins, receipt)?;
        self.journal.funding = self.book.funding().cloned();
        self.persist()?;
        Ok(recorded)
    }

    /// The digest a record will have.
    ///
    /// # Errors
    ///
    /// When it does not encode.
    pub fn digest(record: &Record) -> Result<String> {
        digest_of(record)
    }
}
