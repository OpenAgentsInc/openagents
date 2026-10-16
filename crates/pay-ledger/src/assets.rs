//! Quoted Taproot Assets receipts, kept separately from Bitcoin collections.
use crate::{Error, Ledger, Result};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
pub(crate) const TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS asset_quote (
 node TEXT NOT NULL, payment_hash TEXT NOT NULL, account TEXT NOT NULL REFERENCES compute_account(id),
 document TEXT NOT NULL, PRIMARY KEY(node,payment_hash), UNIQUE(payment_hash)
);
CREATE TABLE IF NOT EXISTS asset_receipt (
 node TEXT NOT NULL, payment_hash TEXT NOT NULL, settle_index TEXT NOT NULL,
 document TEXT NOT NULL, credited_msat INTEGER NOT NULL CHECK(credited_msat>0),
 PRIMARY KEY(node,payment_hash), UNIQUE(node,settle_index),
 FOREIGN KEY(node,payment_hash) REFERENCES asset_quote(node,payment_hash)
);
"#;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rate {
    /// Asset units per BTC, represented by coefficient / 10^scale.
    pub coefficient: String,
    pub scale: u32,
}
impl Rate {
    /// Integer millisatoshis, rounded down; unsupported precision fails closed.
    pub fn value_msat(&self, units: u64) -> Result<i64> {
        if self.scale > 18
            || self.coefficient.starts_with('0')
            || self.coefficient.len() > 38
            || !self.coefficient.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Error::Invalid("asset quote rate"));
        }
        let rate = self
            .coefficient
            .parse::<u128>()
            .map_err(|_| Error::Invalid("asset quote rate"))?;
        let value = u128::from(units)
            .checked_mul(100_000_000_000)
            .and_then(|v| v.checked_mul(10u128.pow(self.scale)))
            .and_then(|v| v.checked_div(rate))
            .ok_or(Error::Invalid("asset quote arithmetic"))?;
        let value = i64::try_from(value).map_err(|_| Error::Invalid("asset quote value"))?;
        if value <= 0 {
            return Err(Error::Invalid("asset quote value"));
        }
        Ok(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub node: String,
    pub network: String,
    pub account: String,
    pub asset_id: String,
    pub edge: String,
    pub rfq_id: String,
    pub payment_hash: String,
    pub invoice: String,
    pub units: u64,
    pub rate: Rate,
    pub credit_msat: i64,
    pub created_at: i64,
    pub expires_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub settle_index: u64,
    pub accepted_at: i64,
    pub settled_at: i64,
    pub units: u64,
    /// Authenticated, settled HTLC asset details; no preimage or credential.
    pub evidence: String,
}
fn hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn encode(v: &impl Serialize) -> Result<String> {
    serde_json::to_string(v).map_err(|_| Error::Invalid("asset record"))
}
impl Ledger {
    /// Freeze the asset, payee account, rate, and invoice before any credit.
    pub fn open_asset_quote(&mut self, q: &Quote) -> Result<()> {
        if self.shared_retail_binding(&q.account)?.is_some() || self.shared_pool(&q.account)? {
            return Err(Error::Denied(
                "asset funding is not enabled for shared accounts",
            ));
        }
        if !hex(&q.node, 66)
            || !hex(&q.edge, 66)
            || !hex(&q.asset_id, 64)
            || !hex(&q.rfq_id, 64)
            || !hex(&q.payment_hash, 64)
            || !["mainnet", "regtest", "signet", "testnet"].contains(&q.network.as_str())
            || q.units == 0
            || q.invoice.is_empty()
            || q.credit_msat != q.rate.value_msat(q.units)?
            || q.credit_msat > crate::compute::purchase::TOP_UP_MAX_MSAT
            || q.created_at < 0
            || q.expires_at <= q.created_at
            || q.expires_at - q.created_at > 3600
        {
            return Err(Error::Invalid("asset quote"));
        }
        let document = encode(q)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let shared: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shared_binding WHERE pool=? OR retail_account=?)",
            params![q.account, q.account],
            |r| r.get(0),
        )?;
        if shared {
            return Err(Error::Denied(
                "asset funding is not enabled for shared accounts",
            ));
        }
        let btc: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM compute_purchase WHERE payment_hash=?)",
            [&q.payment_hash],
            |r| r.get(0),
        )?;
        if btc {
            return Err(Error::Conflict("invoice already used for Bitcoin funding"));
        }
        let old: Option<String> = tx
            .query_row(
                "SELECT document FROM asset_quote WHERE payment_hash=?",
                [&q.payment_hash],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            if old == document {
                return Ok(());
            }
            return Err(Error::Conflict("asset quote terms"));
        }
        tx.execute(
            "INSERT INTO asset_quote(node,payment_hash,account,document) VALUES(?,?,?,?)",
            params![q.node, q.payment_hash, q.account, document],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn asset_quote(&self, node: &str, hash: &str) -> Result<Option<Quote>> {
        let s: Option<String> = self
            .connection
            .query_row(
                "SELECT document FROM asset_quote WHERE node=? AND payment_hash=?",
                params![node, hash],
                |r| r.get(0),
            )
            .optional()?;
        s.map(|s| serde_json::from_str(&s).map_err(|_| Error::Invalid("saved asset quote")))
            .transpose()
    }
    /// Credit a settled asset receipt once in the same transaction as its evidence.
    pub fn credit_asset(&mut self, node: &str, hash: &str, r: &Receipt) -> Result<i64> {
        let q = self
            .asset_quote(node, hash)?
            .ok_or(Error::Invalid("no such asset quote"))?;
        if self.shared_retail_binding(&q.account)?.is_some() || self.shared_pool(&q.account)? {
            return Err(Error::Denied(
                "asset funding is not enabled for shared accounts",
            ));
        }
        if r.settle_index == 0
            || r.units < q.units
            || r.accepted_at < q.created_at
            || r.accepted_at > q.expires_at
            || r.settled_at < r.accepted_at
            || r.evidence.is_empty()
            || r.evidence.len() > 1024 * 1024
        {
            return Err(Error::Invalid("asset receipt"));
        }
        let document = encode(r)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let shared: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shared_binding WHERE pool=? OR retail_account=?)",
            params![q.account, q.account],
            |r| r.get(0),
        )?;
        if shared {
            return Err(Error::Denied(
                "asset funding is not enabled for shared accounts",
            ));
        }
        let old: Option<(String, i64)> = tx
            .query_row(
                "SELECT document,credited_msat FROM asset_receipt WHERE node=? AND payment_hash=?",
                params![node, hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((old, amount)) = old {
            if old == document {
                return Ok(amount);
            }
            return Err(Error::Conflict("asset receipt terms"));
        }
        tx.execute("INSERT INTO asset_receipt(node,payment_hash,settle_index,document,credited_msat) VALUES(?,?,?,?,?)",params![node,hash,r.settle_index.to_string(),document,q.credit_msat])?;
        tx.execute(
            "INSERT INTO compute_credit(account,source,amount_msat,at) VALUES(?,?,?,?)",
            params![
                q.account,
                format!("taproot-assets:{node}:{hash}"),
                q.credit_msat,
                r.settled_at
            ],
        )?;
        tx.commit()?;
        Ok(q.credit_msat)
    }
    pub fn asset_receipts(&self) -> Result<Vec<(Quote, Receipt, i64)>> {
        let mut s=self.connection.prepare("SELECT q.document,r.document,r.credited_msat FROM asset_receipt r JOIN asset_quote q USING(node,payment_hash) ORDER BY r.node,r.settle_index")?;
        let rows = s.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (q, r, n) = row?;
            Ok((
                serde_json::from_str(&q).map_err(|_| Error::Invalid("saved asset quote"))?,
                serde_json::from_str(&r).map_err(|_| Error::Invalid("saved asset receipt"))?,
                n,
            ))
        })
        .collect()
    }
}
