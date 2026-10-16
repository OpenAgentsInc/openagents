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
    lines: Vec<Line>,
    funding: Option<(String, String)>,
}

pub struct Host {
    dir: PathBuf,
    secret: secp256k1::SecretKey,
    pub pubkey: String,
    pub book: Book,
    journal: Journal,
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
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let path = dir.join("journal.json");
        let journal: Journal = if path.exists() {
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|_| "the journal does not parse")?
        } else {
            Journal {
                admitted_at: now,
                ..Journal::default()
            }
        };
        let mut book = Book::new(terms, journal.admitted_at)?;
        for line in &journal.lines {
            if let Some(event) = &line.event {
                let opened =
                    nostr::private_artifact::open(event, &secret).map_err(|e| e.to_string())?;
                if opened.signer() != line.signer {
                    return Err("a journal line's signer differs from its event".into());
                }
            }
            book.apply(Record::parse(&line.record)?, &line.signer, line.observed_at)?;
        }
        if let Some((acceptance, hash)) = journal.funding.clone() {
            book.restore_funding(acceptance, hash);
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            secret,
            pubkey: me,
            book,
            journal,
        })
    }

    fn persist(&self) -> Result<()> {
        let path = self.dir.join("journal.json");
        let tmp = self.dir.join("journal.json.tmp");
        let bytes = serde_json::to_vec(&self.journal).map_err(|e| e.to_string())?;
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
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
        if event.tag_values("p").next() != Some(self.pubkey.as_str()) {
            return Ok(None);
        }
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        let bytes = opened.inline_bytes().ok_or("the record is not inline")?;
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "the record is not JSON")?;
        let record = Record::parse(&value)?;
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
