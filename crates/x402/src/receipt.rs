//! `openagents.payment-receipt.v1` (#11138): one record per settled
//! payment, whatever the protocol or rail, written once.
//!
//! The id is derived from the payment's replay key, so a payment that is
//! given back and settled again (its request got no answer the first
//! time) still has exactly one receipt. The record holds no bearer secret:
//! no preimage, token, proof, or invoice. The schema is
//! `nips/openagents/schemas/payment-receipt.v1.json`.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA: &str = "openagents.payment-receipt.v1";

/// The header a paid answer carries its receipt id in.
pub const HEADER: &str = "x-openagents-receipt";

/// Who paid, when they said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payer {
    pub account: Option<String>,
    pub nostr: Option<String>,
    pub mandate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentReceipt {
    pub v: String,
    /// `pr_` and 32 hex characters.
    pub id: String,
    /// `x402`, `mpp`, ...
    pub protocol: String,
    /// `lightning`, ...
    pub rail: String,
    pub network: String,
    pub asset: String,
    /// The smallest unit of `asset` (msat for Lightning), decimal.
    pub amount: String,
    /// The quote in micros of a dollar, fixed when it was challenged.
    pub usd_micros: u64,
    pub replay_key: String,
    /// The `http:1` binding hash of the request paid for.
    pub request_hash: String,
    /// `POST /v1/responses`.
    pub resource: String,
    pub payer: Payer,
    pub settled_at: u64,
    /// `served`, `refunded`, `failed_before_answer`.
    pub outcome: String,
}

/// The receipt id for a replay key.
#[must_use]
pub fn receipt_id(replay_key: &str) -> String {
    let digest = Sha256::digest(replay_key.as_bytes());
    format!("pr_{}", hex::encode(&digest[..16]))
}

/// One JSON file per receipt; `create_new` makes the write once-only
/// across processes.
pub struct FileReceiptStore {
    dir: PathBuf,
}

impl FileReceiptStore {
    /// # Errors
    ///
    /// When the directory cannot be created.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        let ok = id.len() == 35
            && id.starts_with("pr_")
            && id[3..].bytes().all(|b| b.is_ascii_hexdigit());
        ok.then(|| self.dir.join(format!("{id}.json")))
    }

    /// Write `receipt` unless one with its id exists. `Ok(true)` when this
    /// call wrote it.
    ///
    /// # Errors
    ///
    /// A malformed id or an I/O failure.
    pub fn write(&self, receipt: &PaymentReceipt) -> std::io::Result<bool> {
        let path = self
            .path(&receipt.id)
            .ok_or_else(|| std::io::Error::other("malformed receipt id"))?;
        let mut file = match fs::File::create_new(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(false),
            Err(error) => return Err(error),
        };
        let bytes = serde_json::to_vec(receipt).map_err(std::io::Error::other)?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        Ok(true)
    }

    /// # Errors
    ///
    /// An I/O or decoding failure.
    pub fn get(&self, id: &str) -> std::io::Result<Option<PaymentReceipt>> {
        let Some(path) = self.path(id) else {
            return Ok(None);
        };
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(std::io::Error::other),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Every receipt, in no particular order.
    ///
    /// # Errors
    ///
    /// An I/O failure reading the directory.
    pub fn all(&self) -> std::io::Result<Vec<PaymentReceipt>> {
        let mut out = Vec::new();
        for item in fs::read_dir(&self.dir)? {
            let path = item?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            if let Ok(bytes) = fs::read(&path)
                && let Ok(receipt) = serde_json::from_slice(&bytes)
            {
                out.push(receipt);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sample() -> PaymentReceipt {
        PaymentReceipt {
            v: SCHEMA.into(),
            id: receipt_id("lnbtc:000000000019d6689c085ae165831e93:ab"),
            protocol: "mpp".into(),
            rail: "lightning".into(),
            network: nostr::x402::MAINNET.into(),
            asset: "BTC".into(),
            amount: "3000".into(),
            usd_micros: 2100,
            replay_key: "lnbtc:000000000019d6689c085ae165831e93:ab".into(),
            request_hash: "cd".repeat(32),
            resource: "POST /v1/responses".into(),
            payer: Payer::default(),
            settled_at: 1_791_590_400,
            outcome: "served".into(),
        }
    }

    #[test]
    fn written_once_and_matches_the_schema() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileReceiptStore::open(dir.path()).unwrap();
        let receipt = sample();
        assert!(store.write(&receipt).unwrap());
        assert!(!store.write(&receipt).unwrap());
        assert_eq!(store.all().unwrap().len(), 1);
        assert_eq!(store.get(&receipt.id).unwrap(), Some(receipt.clone()));

        let schema = include_bytes!("../../../nips/openagents/schemas/payment-receipt.v1.json");
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/payment-receipt.v1.json"))
                .unwrap();
        let digest = nostr::contracts::digest_bytes(schema);
        let mut documents = std::collections::BTreeMap::new();
        documents.insert(digest.clone(), schema.to_vec());
        let closure = nostr::contracts::prepare_closure(&documents).expect("a supported schema");
        let written = serde_json::to_value(&receipt).unwrap();
        for value in [&written, &fixture] {
            nostr::contracts::validate_instance(&closure, &digest, value).expect("valid receipt");
        }
        let mut bad = written.clone();
        bad["preimage"] = Value::String("00".repeat(32));
        assert!(
            nostr::contracts::validate_instance(&closure, &digest, &bad).is_err(),
            "a receipt never carries a preimage"
        );
        // The fixture is a real record.
        let _: PaymentReceipt = serde_json::from_value(fixture).unwrap();
    }
}
