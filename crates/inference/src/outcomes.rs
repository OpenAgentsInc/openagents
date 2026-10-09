//! Verified paid-work outcomes. Only configured Coder/Gym verifiers can
//! contribute; their signatures bind payment, evaluation, and run identity.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, Seek, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

use crate::router::{Scores, TaskClass, micros_usd};

pub const SCHEMA: &str = "openagents.inference.paid-outcome.v1";
const MAX_RECORDS: usize = 100_000;
const MAX_FILE: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    CoderAcceptance,
    GymHeadToHead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Real,
    Fixture,
    Synthetic,
}

/// A verifier signs this after checking the final payment and evaluation.
/// A rejected outcome is still a verified result of paid work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub v: String,
    pub source: Source,
    /// Global work identity, shared across sources, retries, and verifier rotations.
    pub run_id: String,
    pub class: TaskClass,
    pub model: String,
    pub mode: Mode,
    pub accepted: bool,
    /// Final settled bitcoin payment, in millisatoshis.
    pub paid_msat: u64,
    /// Total inference cost of the evaluated work, including failed attempts.
    pub cost_micros: u64,
    /// SHA-256 identities of the source's payment and evaluation receipts.
    pub payment_digest: String,
    pub verification_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub outcome: Outcome,
    /// Base64 Ed25519 public key; this must be configured for this source.
    pub issuer: String,
    pub signature: String,
}

fn payload(outcome: &Outcome) -> Vec<u8> {
    // A typed body fixes field order; arbitrary JSON order cannot change it.
    serde_json::to_vec(outcome).expect("an outcome serializes")
}

impl Receipt {
    /// Coder and Gym verifiers use the same envelope after checking their
    /// own payment and acceptance receipts. Never sign an unverified claim.
    pub fn sign(outcome: Outcome, key: &Ed25519KeyPair) -> Self {
        Self {
            signature: STANDARD.encode(key.sign(&payload(&outcome)).as_ref()),
            issuer: STANDARD.encode(key.public_key().as_ref()),
            outcome,
        }
    }

    fn verify(&self, issuers: &[Issuer]) -> Result<(), String> {
        let o = &self.outcome;
        let word =
            |s: &str| !s.is_empty() && s.len() <= 256 && s.bytes().all(|c| c.is_ascii_graphic());
        let digest = |s: &str| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit());
        if o.v != SCHEMA
            || !word(&o.run_id)
            || !word(&o.model)
            || !digest(&o.payment_digest)
            || !digest(&o.verification_digest)
            || (o.mode == Mode::Real && o.paid_msat == 0)
        {
            return Err("The outcome needs a paid run and checked receipt references.".into());
        }
        if !issuers
            .iter()
            .any(|i| i.source == o.source && i.key == self.issuer)
        {
            return Err("This verifier is not allowed to report these outcomes.".into());
        }
        let key = STANDARD
            .decode(&self.issuer)
            .map_err(|_| "The verifier key is invalid.")?;
        let signature = STANDARD
            .decode(&self.signature)
            .map_err(|_| "The outcome signature is invalid.")?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(&payload(o), &signature)
            .map_err(|_| "The outcome signature is invalid.".to_owned())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issuer {
    pub source: Source,
    pub key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub path: PathBuf,
    pub issuers: Vec<Issuer>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub samples: u64,
    pub accepted: u64,
    pub cost_micros: u64,
    pub paid_msat: u64,
}

impl Counts {
    pub fn rate(&self) -> Option<f64> {
        (self.samples != 0).then(|| self.accepted as f64 / self.samples as f64)
    }

    pub fn public(&self) -> serde_json::Value {
        serde_json::json!({
            "samples": self.samples, "accepted": self.accepted,
            "accepted_rate": self.rate(), "cost_usd": micros_usd(self.cost_micros),
            "cost_per_accepted_usd": (self.accepted != 0).then(|| micros_usd(self.cost_micros / self.accepted)),
            "paid_msat": self.paid_msat,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub by_class: BTreeMap<TaskClass, BTreeMap<String, Counts>>,
    /// Visible only to operators; never part of public samples or scores.
    pub fixtures: u64,
    pub synthetic: u64,
}

impl Summary {
    pub fn apply(&self, scores: &mut Scores) {
        for (class, models) in &self.by_class {
            for (model, counts) in models {
                if let Some(rate) = counts.rate() {
                    scores
                        .by_class
                        .entry(*class)
                        .or_default()
                        .entry(model.clone())
                        .and_modify(|gym| *gym = gym.min(rate))
                        .or_insert(rate);
                }
            }
        }
    }

    pub fn model(&self, model: &str) -> serde_json::Value {
        let rows: BTreeMap<_, _> = self
            .by_class
            .iter()
            .filter_map(|(class, models)| {
                models
                    .get(model)
                    .map(|counts| (class.as_str(), counts.public()))
            })
            .collect();
        serde_json::to_value(rows).expect("counts serialize")
    }

    fn add(&mut self, outcome: &Outcome) -> Result<(), String> {
        match outcome.mode {
            Mode::Fixture => self.fixtures += 1,
            Mode::Synthetic => self.synthetic += 1,
            Mode::Real => {
                let counts = self
                    .by_class
                    .entry(outcome.class)
                    .or_default()
                    .entry(outcome.model.clone())
                    .or_default();
                counts.samples += 1;
                counts.accepted += u64::from(outcome.accepted);
                counts.cost_micros = counts
                    .cost_micros
                    .checked_add(outcome.cost_micros)
                    .ok_or("Outcome costs are too large.")?;
                counts.paid_msat = counts
                    .paid_msat
                    .checked_add(outcome.paid_msat)
                    .ok_or("Outcome payments are too large.")?;
            }
        }
        Ok(())
    }
}

struct Inner {
    file: File,
    records: BTreeMap<String, Outcome>,
    summary: Summary,
    healthy: bool,
    written: u64,
}

impl Inner {
    fn available(&mut self) -> Result<(), String> {
        if !self.healthy || self.file.metadata().map(|meta| meta.len()).ok() != Some(self.written) {
            self.healthy = false;
            return Err("Outcome storage is unavailable.".into());
        }
        Ok(())
    }
}

pub struct Book {
    issuers: Vec<Issuer>,
    inner: Mutex<Inner>,
}

impl Book {
    pub fn open(config: &Config) -> Result<Self, String> {
        use std::io::SeekFrom;
        if !config.path.is_absolute()
            || config.issuers.is_empty()
            || config
                .issuers
                .iter()
                .any(|i| STANDARD.decode(&i.key).map_or(true, |key| key.len() != 32))
        {
            return Err("Outcome storage needs an absolute path and verifier keys.".into());
        }
        if let Some(parent) = config.path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "Outcome storage could not be opened.")?;
        }
        let mut options = OpenOptions::new();
        options.read(true).append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&config.path)
            .map_err(|_| "Outcome storage could not be opened.")?;
        file.try_lock()
            .map_err(|_| "Outcome storage is already in use.")?;
        if file
            .metadata()
            .map_err(|_| "Outcome storage could not be read.")?
            .len()
            > MAX_FILE
        {
            return Err("Outcome storage is full.".into());
        }
        let mut records = BTreeMap::new();
        let mut summary = Summary::default();
        let mut reader = std::io::BufReader::new(&file);
        let mut line = String::new();
        loop {
            line.clear();
            if reader
                .read_line(&mut line)
                .map_err(|_| "Outcome storage could not be read.")?
                == 0
            {
                break;
            }
            if !line.ends_with('\n') {
                return Err("A saved outcome is incomplete.".into());
            }
            let receipt: Receipt =
                serde_json::from_str(&line).map_err(|_| "A saved outcome is invalid.")?;
            receipt.verify(&config.issuers)?;
            let identity = receipt.outcome.run_id.clone();
            match records.get(&identity) {
                Some(previous) if previous == &receipt.outcome => continue,
                Some(_) => return Err("This run already has a different outcome.".into()),
                None => {}
            }
            if records.len() >= MAX_RECORDS {
                return Err("Outcome storage is full.".into());
            }
            summary.add(&receipt.outcome)?;
            records.insert(identity, receipt.outcome);
        }
        let written = file
            .seek(SeekFrom::End(0))
            .map_err(|_| "Outcome storage could not be opened.")?;
        #[cfg(unix)]
        if let Some(parent) = config.path.parent() {
            File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| "Outcome storage could not be saved.")?;
        }
        Ok(Self {
            issuers: config.issuers.clone(),
            inner: Mutex::new(Inner {
                file,
                records,
                summary,
                healthy: true,
                written,
            }),
        })
    }

    /// Idempotent by work identity, even across sources and verifier rotations.
    pub fn record(&self, receipt: Receipt) -> Result<bool, String> {
        receipt.verify(&self.issuers)?;
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Outcome storage is unavailable.")?;
        inner.available()?;
        let identity = receipt.outcome.run_id.clone();
        if let Some(previous) = inner.records.get(&identity) {
            return if previous == &receipt.outcome {
                Ok(false)
            } else {
                Err("This run already has a different outcome.".into())
            };
        }
        if inner.records.len() >= MAX_RECORDS {
            return Err("Outcome storage is full.".into());
        }
        let mut next = inner.summary.clone();
        next.add(&receipt.outcome)?;
        let mut bytes = serde_json::to_vec(&receipt).expect("a receipt serializes");
        bytes.push(b'\n');
        if inner.written.saturating_add(bytes.len() as u64) > MAX_FILE {
            return Err("Outcome storage is full.".into());
        }
        if inner
            .file
            .write_all(&bytes)
            .and_then(|()| inner.file.sync_all())
            .is_err()
        {
            inner.healthy = false;
            return Err("The outcome could not be saved.".into());
        }
        inner.written += bytes.len() as u64;
        inner.summary = next;
        inner.records.insert(identity, receipt.outcome);
        Ok(true)
    }

    pub fn summary(&self) -> Result<Summary, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Outcome storage is unavailable.")?;
        inner.available()?;
        Ok(inner.summary.clone())
    }
}

#[cfg(test)]
mod tests;
