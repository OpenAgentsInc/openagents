//! `openagents.payment-receipt.v1` in the ledger (#11138): one row per
//! settled payment, whatever the protocol (x402, the `Payment` scheme,
//! L402, Cashu, ...) or rail, next to the plugin and payout records so the
//! pay host's `/stats` counts every protocol.
//!
//! The row is written once: its id is derived from the payment's replay
//! key, so a payment that was given back and settled again (its request
//! got no answer the first time) still has one receipt, and a second write
//! of the same id changes nothing. `request_hash` (the `http:1` hash of
//! the request as it was received) groups attempts; `replay_key` names the
//! one payment. A receipt here is only ever for an answered request
//! (`served`); refusals are not receipts.
//!
//! Bearer secrets never reach this table: a record that carries a
//! preimage, token, proof, macaroon, or invoice anywhere is refused before
//! it is written.

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Ledger, Result};

pub const SCHEMA: &str = "openagents.payment-receipt.v1";

pub(crate) const TABLES: &str = "CREATE TABLE IF NOT EXISTS payment_receipt (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 id TEXT NOT NULL UNIQUE,
 protocol TEXT NOT NULL,
 rail TEXT NOT NULL,
 network TEXT NOT NULL,
 asset TEXT NOT NULL,
 amount TEXT NOT NULL,
 usd_micros INTEGER NOT NULL CHECK(usd_micros >= 0),
 replay_key TEXT NOT NULL UNIQUE,
 request_hash TEXT NOT NULL,
 resource TEXT NOT NULL,
 settled_at INTEGER NOT NULL,
 outcome TEXT NOT NULL,
 record TEXT NOT NULL
);";

/// The protocols a receipt may name (the schema's enum).
pub const PROTOCOLS: [&str; 9] = [
    "x402",
    "mpp",
    "l402",
    "cashu",
    "acp",
    "ucp",
    "ap2",
    "stripe-checkout",
    "zap",
];

/// The rails a receipt may name.
pub const RAILS: [&str; 6] = ["lightning", "evm", "solana", "tempo", "card", "ecash"];

/// The outcomes a receipt may name.
pub const OUTCOMES: [&str; 3] = ["served", "refunded", "failed_before_answer"];

/// Field names that would carry a bearer secret. None may appear anywhere
/// in a receipt.
const SECRETS: [&str; 7] = [
    "preimage", "token", "proof", "proofs", "macaroon", "invoice", "secret",
];

/// Who paid, when they said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payer {
    pub account: Option<String>,
    pub nostr: Option<String>,
    pub mandate: Option<String>,
}

/// One `openagents.payment-receipt.v1` record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentReceipt {
    pub v: String,
    pub id: String,
    pub protocol: String,
    pub rail: String,
    pub network: String,
    pub asset: String,
    pub amount: String,
    pub usd_micros: u64,
    pub replay_key: String,
    pub request_hash: String,
    pub resource: String,
    pub payer: Payer,
    pub settled_at: u64,
    pub outcome: String,
    /// Which transfer paid, on its rail: the payment hash on Lightning,
    /// the mint and proof point on Cashu. Public by construction (never a
    /// preimage or a proof's secret).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
}

/// How many served receipts one protocol has, and what they were quoted at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProtocolCount {
    pub receipts: u64,
    pub usd_micros: u64,
}

fn is_lower_hex(text: &str) -> bool {
    text.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn carries_secret(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(name, value)| {
            SECRETS.contains(&name.to_ascii_lowercase().as_str()) || carries_secret(value)
        }),
        Value::Array(items) => items.iter().any(carries_secret),
        _ => false,
    }
}

impl PaymentReceipt {
    /// Check the record against the schema's rules before it is kept.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] naming the first field out of shape.
    pub fn check(&self) -> Result<()> {
        if self.v != SCHEMA {
            return Err(Error::Invalid("payment receipt version"));
        }
        if self.id.len() != 35 || !self.id.starts_with("pr_") || !is_lower_hex(&self.id[3..]) {
            return Err(Error::Invalid("payment receipt id"));
        }
        if !PROTOCOLS.contains(&self.protocol.as_str()) {
            return Err(Error::Invalid("payment receipt protocol"));
        }
        if !RAILS.contains(&self.rail.as_str()) {
            return Err(Error::Invalid("payment receipt rail"));
        }
        if !OUTCOMES.contains(&self.outcome.as_str()) {
            return Err(Error::Invalid("payment receipt outcome"));
        }
        if self.request_hash.len() != 64 || !is_lower_hex(&self.request_hash) {
            return Err(Error::Invalid("payment receipt request hash"));
        }
        if self.network.is_empty()
            || self.asset.is_empty()
            || self.resource.is_empty()
            || self.replay_key.is_empty()
        {
            return Err(Error::Invalid("payment receipt field"));
        }
        if self.amount.is_empty() || !self.amount.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::Invalid("payment receipt amount"));
        }
        if i64::try_from(self.usd_micros).is_err() || i64::try_from(self.settled_at).is_err() {
            return Err(Error::Invalid("payment receipt number"));
        }
        let value = serde_json::to_value(self).map_err(|_| Error::Invalid("payment receipt"))?;
        if carries_secret(&value) {
            return Err(Error::Invalid("payment receipt carries a secret"));
        }
        Ok(())
    }
}

impl Ledger {
    /// Keep `receipt` unless one with its id is already kept. `Ok(true)`
    /// when this call wrote it.
    ///
    /// # Errors
    ///
    /// A record out of shape (see [`PaymentReceipt::check`]), another
    /// receipt already under the same replay key with a different id, or
    /// a storage failure.
    pub fn record_payment_receipt(&mut self, receipt: &PaymentReceipt) -> Result<bool> {
        receipt.check()?;
        let record =
            serde_json::to_string(receipt).map_err(|_| Error::Invalid("payment receipt"))?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM payment_receipt WHERE id=?1 OR replay_key=?2",
                params![receipt.id, receipt.replay_key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != receipt.id {
                return Err(Error::Conflict("payment receipt replay key"));
            }
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO payment_receipt(id,protocol,rail,network,asset,amount,usd_micros,replay_key,request_hash,resource,settled_at,outcome,record)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                receipt.id,
                receipt.protocol,
                receipt.rail,
                receipt.network,
                receipt.asset,
                receipt.amount,
                i64::try_from(receipt.usd_micros).unwrap_or(i64::MAX),
                receipt.replay_key,
                receipt.request_hash,
                receipt.resource,
                i64::try_from(receipt.settled_at).unwrap_or(i64::MAX),
                receipt.outcome,
                record,
            ],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// The receipt `id`, if kept.
    ///
    /// # Errors
    ///
    /// A storage failure or an unreadable record.
    pub fn payment_receipt(&self, id: &str) -> Result<Option<PaymentReceipt>> {
        let record: Option<String> = self
            .connection
            .query_row(
                "SELECT record FROM payment_receipt WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        record
            .map(|record| {
                serde_json::from_str(&record).map_err(|_| Error::Invalid("payment receipt record"))
            })
            .transpose()
    }

    /// Kept receipts after `seq`, oldest first, with their sequence
    /// numbers: what a reader that keeps a cursor (the pay host) syncs.
    ///
    /// # Errors
    ///
    /// A storage failure or an unreadable record.
    pub fn payment_receipts_after(&self, seq: i64) -> Result<Vec<(i64, PaymentReceipt)>> {
        let mut statement = self
            .connection
            .prepare("SELECT seq,record FROM payment_receipt WHERE seq>?1 ORDER BY seq")?;
        let rows = statement
            .query_map([seq], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(seq, record)| {
                serde_json::from_str(&record)
                    .map(|receipt| (seq, receipt))
                    .map_err(|_| Error::Invalid("payment receipt record"))
            })
            .collect()
    }

    /// Served receipts per protocol: what `/stats` shows.
    ///
    /// # Errors
    ///
    /// A storage failure.
    pub fn payment_receipt_counts(&self) -> Result<BTreeMap<String, ProtocolCount>> {
        let mut statement = self.connection.prepare(
            "SELECT protocol,COUNT(*),COALESCE(SUM(usd_micros),0) FROM payment_receipt WHERE outcome='served' GROUP BY protocol",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .map(|(protocol, receipts, micros)| {
                (
                    protocol,
                    ProtocolCount {
                        receipts: u64::try_from(receipts).unwrap_or_default(),
                        usd_micros: u64::try_from(micros).unwrap_or_default(),
                    },
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(protocol: &str, replay_key: &str) -> PaymentReceipt {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(replay_key.as_bytes());
        PaymentReceipt {
            v: SCHEMA.into(),
            id: format!("pr_{}", hex::encode(&digest[..16])),
            protocol: protocol.into(),
            rail: "lightning".into(),
            network: "lnbtc:000000000019d6689c085ae165831e93".into(),
            asset: "BTC".into(),
            amount: "3000".into(),
            usd_micros: 2100,
            replay_key: replay_key.into(),
            request_hash: "cd".repeat(32),
            resource: "POST /v1/responses".into(),
            payer: Payer::default(),
            settled_at: 1_791_590_400,
            outcome: "served".into(),
            locator: Some("ab".repeat(32)),
        }
    }

    #[test]
    fn a_receipt_is_kept_once_and_counted_per_protocol() {
        let mut ledger = Ledger::in_memory().unwrap();
        let first = sample("x402", "lnbtc:000000000019d6689c085ae165831e93:01");
        assert!(ledger.record_payment_receipt(&first).unwrap());
        assert!(!ledger.record_payment_receipt(&first).unwrap());
        assert_eq!(
            ledger.payment_receipt(&first.id).unwrap(),
            Some(first.clone())
        );
        ledger
            .record_payment_receipt(&sample("mpp", "lnbtc:000000000019d6689c085ae165831e93:02"))
            .unwrap();
        ledger
            .record_payment_receipt(&sample("x402", "lnbtc:000000000019d6689c085ae165831e93:03"))
            .unwrap();
        let counts = ledger.payment_receipt_counts().unwrap();
        assert_eq!(
            counts["x402"],
            ProtocolCount {
                receipts: 2,
                usd_micros: 4200
            }
        );
        assert_eq!(counts["mpp"].receipts, 1);
        let after = ledger.payment_receipts_after(1).unwrap();
        assert_eq!(after.len(), 2);
        assert!(after.iter().all(|(seq, _)| *seq > 1));
    }

    #[test]
    fn a_receipt_out_of_shape_or_with_a_secret_is_refused() {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut bad = sample("x402", "k:1");
        bad.protocol = "bitcoin".into();
        assert!(ledger.record_payment_receipt(&bad).is_err());
        let mut bad = sample("x402", "k:2");
        bad.request_hash = "zz".repeat(32);
        assert!(ledger.record_payment_receipt(&bad).is_err());
        let mut bad = sample("x402", "k:3");
        bad.amount = "1.5".into();
        assert!(ledger.record_payment_receipt(&bad).is_err());
        // The record type has no place for a secret, and a JSON record that
        // smuggles one in is refused by the reader too.
        let mut value = serde_json::to_value(sample("x402", "k:4")).unwrap();
        value["preimage"] = Value::String("00".repeat(32));
        assert!(serde_json::from_value::<PaymentReceipt>(value.clone()).is_err());
        assert!(carries_secret(&value));
        let mut nested = serde_json::to_value(sample("x402", "k:5")).unwrap();
        nested["payer"]["token"] = Value::String("cashuB...".into());
        assert!(carries_secret(&nested));
        assert!(ledger.payment_receipt_counts().unwrap().is_empty());
    }

    #[test]
    fn one_replay_key_has_one_id() {
        let mut ledger = Ledger::in_memory().unwrap();
        let first = sample("x402", "lnbtc:aa:01");
        ledger.record_payment_receipt(&first).unwrap();
        let mut other = sample("x402", "lnbtc:aa:02");
        other.replay_key = first.replay_key.clone();
        assert!(matches!(
            ledger.record_payment_receipt(&other),
            Err(Error::Conflict(_))
        ));
    }
}
