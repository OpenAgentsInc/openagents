//! Lightning top-ups of the compute balance (#10707).
//!
//! A top-up is one exact invoice the receiver wallet issued for one account,
//! one amount, and one purchase ID. Only an observation that the invoice was
//! paid in full credits the balance, and it credits it once: the credit's
//! source is `topup:<payment hash>`, unique in the ledger, so a duplicate
//! callback, a concurrent observer, or a replay after a crash finds the
//! credit already posted. Pending, expired, and unknown purchases never add
//! to what an account can spend.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Error, Ledger, Result};

/// The largest single top-up: 1,000,000 sats.
pub const TOP_UP_MAX_MSAT: i64 = 1_000_000_000;
/// The longest an invoice may stay open: one hour.
pub const TOP_UP_EXPIRY_MAX_SECS: i64 = 3_600;

/// A top-up the receiver wallet issued an invoice for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TopUp {
    /// The purchase identity; a retry of the same purchase reuses it.
    pub id: String,
    pub account: String,
    pub amount_msat: i64,
    /// 64 lowercase hex digits.
    pub payment_hash: String,
    pub invoice: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PurchaseState {
    /// Issued and not yet paid.
    Pending,
    /// Paid in full and credited.
    Paid,
    /// Past its expiry and never paid.
    Expired,
    /// The wallet's account of it does not settle the question; nothing is
    /// credited until it does.
    Unknown,
}

impl PurchaseState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Paid => "paid",
            Self::Expired => "expired",
            Self::Unknown => "unknown",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "pending" => Self::Pending,
            "paid" => Self::Paid,
            "expired" => Self::Expired,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

/// A top-up as the ledger keeps it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Purchase {
    pub top_up: TopUp,
    pub state: PurchaseState,
    pub observed_at: Option<i64>,
    /// Why a purchase is unknown, when it is.
    pub detail: Option<String>,
}

/// What the receiver wallet reports for an invoice.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Receipt {
    /// Paid; `received_msat` is what the wallet received.
    Paid { received_msat: i64, at: i64 },
    /// Not paid yet.
    Pending,
    /// The wallet failed it or it expired unpaid.
    Failed { at: i64 },
    /// The wallet cannot say.
    Unknown { at: i64, detail: String },
}

fn is_hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Ledger {
    /// Record a top-up's invoice. Recording the same purchase again returns
    /// it unchanged; the same ID or payment hash with other terms conflicts.
    pub fn open_top_up(&mut self, top_up: &TopUp) -> Result<Purchase> {
        if self.shared_retail_binding(&top_up.account)?.is_some()
            || (self.shared_pool(&top_up.account)? && !self.shared_writer)
        {
            return Err(Error::Denied(
                "shared retail funding belongs to the canonical custodian",
            ));
        }
        if top_up.id.is_empty()
            || top_up.id.len() > 128
            || top_up.amount_msat <= 0
            || top_up.amount_msat > TOP_UP_MAX_MSAT
            || top_up.amount_msat % 1000 != 0
            || !is_hash(&top_up.payment_hash)
            || top_up.invoice.is_empty()
            || top_up.expires_at <= top_up.created_at
            || top_up.expires_at - top_up.created_at > TOP_UP_EXPIRY_MAX_SECS
        {
            return Err(Error::Invalid(
                "top-up id, whole-sat amount, payment hash, or expiry",
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM compute_account WHERE id=?)",
            [&top_up.account],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(Error::Invalid("no such compute account"));
        }
        let existing = match read_purchase(&tx, "id", &top_up.id)? {
            Some(found) => Some(found),
            None => read_purchase(&tx, "payment_hash", &top_up.payment_hash)?,
        };
        if let Some(existing) = existing {
            if &existing.top_up == top_up {
                return Ok(existing);
            }
            return Err(Error::Conflict("the purchase exists with other terms"));
        }
        tx.execute(
            "INSERT INTO compute_purchase(id,account,amount_msat,payment_hash,invoice,created_at,expires_at,state) VALUES(?,?,?,?,?,?,?,'pending')",
            params![
                top_up.id,
                top_up.account,
                top_up.amount_msat,
                top_up.payment_hash,
                top_up.invoice,
                top_up.created_at,
                top_up.expires_at
            ],
        )?;
        let purchase =
            read_purchase(&tx, "id", &top_up.id)?.ok_or(Error::Invalid("missing purchase"))?;
        tx.commit()?;
        Ok(purchase)
    }

    /// Apply the wallet's report for `payment_hash`. A full payment credits
    /// the account exactly once; anything else credits nothing. A paid
    /// purchase stays paid whatever is observed later.
    pub fn observe_top_up(&mut self, payment_hash: &str, receipt: &Receipt) -> Result<Purchase> {
        if let Some(purchase) = read_purchase(&self.connection, "payment_hash", payment_hash)? {
            if self.shared_pool(&purchase.top_up.account)? && !self.shared_writer {
                return Err(Error::Denied(
                    "canonical funding confirmation belongs to the custodian",
                ));
            }
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let purchase = read_purchase(&tx, "payment_hash", payment_hash)?
            .ok_or(Error::Invalid("no such top-up"))?;
        if purchase.state == PurchaseState::Paid {
            return Ok(purchase);
        }
        match receipt {
            Receipt::Pending => return Ok(purchase),
            Receipt::Paid { received_msat, at }
                if *received_msat == purchase.top_up.amount_msat =>
            {
                let shared: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM shared_binding WHERE pool=?)",
                    [&purchase.top_up.account],
                    |r| r.get(0),
                )?;
                if shared {
                    crate::shared::claim_inbound_in(
                        &tx,
                        payment_hash,
                        "funding",
                        &purchase.top_up.id,
                    )?;
                }
                tx.execute(
                    "INSERT INTO compute_credit(account,source,amount_msat,at) VALUES(?,?,?,?) ON CONFLICT(source) DO NOTHING",
                    params![
                        purchase.top_up.account,
                        format!("topup:{payment_hash}"),
                        received_msat,
                        at
                    ],
                )?;
                tx.execute(
                    "UPDATE compute_purchase SET state='paid', observed_at=?, detail=NULL WHERE payment_hash=?",
                    params![at, payment_hash],
                )?;
            }
            Receipt::Paid { received_msat, at } => {
                tx.execute(
                    "UPDATE compute_purchase SET state='unknown', observed_at=?, detail=? WHERE payment_hash=?",
                    params![
                        at,
                        format!("received {received_msat} msat for an exact invoice"),
                        payment_hash
                    ],
                )?;
            }
            Receipt::Failed { at } => {
                if *at < purchase.top_up.expires_at && purchase.state == PurchaseState::Pending {
                    // A failure before expiry leaves the invoice payable.
                    return Ok(purchase);
                }
                tx.execute(
                    "UPDATE compute_purchase SET state='expired', observed_at=? WHERE payment_hash=?",
                    params![at, payment_hash],
                )?;
            }
            Receipt::Unknown { at, detail } => {
                tx.execute(
                    "UPDATE compute_purchase SET state='unknown', observed_at=?, detail=? WHERE payment_hash=?",
                    params![at, detail, payment_hash],
                )?;
            }
        }
        let purchase = read_purchase(&tx, "payment_hash", payment_hash)?
            .ok_or(Error::Invalid("missing purchase"))?;
        tx.commit()?;
        Ok(purchase)
    }

    /// One purchase by its ID.
    pub fn top_up(&self, id: &str) -> Result<Option<Purchase>> {
        read_purchase(&self.connection, "id", id)
    }

    /// An account's top-ups, newest first.
    pub fn top_ups(&self, account: &str) -> Result<Vec<Purchase>> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM compute_purchase WHERE account=? ORDER BY created_at DESC, id",
        )?;
        let ids = statement
            .query_map([account], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| read_purchase(&self.connection, "id", id)?.ok_or(Error::Invalid("missing")))
            .collect()
    }

    /// Top-ups still waiting on the wallet: pending or unknown.
    pub fn open_top_ups(&self) -> Result<Vec<Purchase>> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM compute_purchase WHERE state IN ('pending','unknown') ORDER BY created_at, id",
        )?;
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| read_purchase(&self.connection, "id", id)?.ok_or(Error::Invalid("missing")))
            .collect()
    }

    /// The account's credited purchases, in millisatoshis.
    pub fn credited(&self, account: &str) -> Result<i64> {
        Ok(self.connection.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM compute_credit WHERE account=?",
            [account],
            |r| r.get(0),
        )?)
    }
}

fn read_purchase(connection: &Connection, column: &str, value: &str) -> Result<Option<Purchase>> {
    let sql = match column {
        "id" => {
            "SELECT id,account,amount_msat,payment_hash,invoice,created_at,expires_at,state,observed_at,detail FROM compute_purchase WHERE id=?"
        }
        _ => {
            "SELECT id,account,amount_msat,payment_hash,invoice,created_at,expires_at,state,observed_at,detail FROM compute_purchase WHERE payment_hash=?"
        }
    };
    let row = connection
        .query_row(sql, [value], |r| {
            Ok((
                TopUp {
                    id: r.get(0)?,
                    account: r.get(1)?,
                    amount_msat: r.get(2)?,
                    payment_hash: r.get(3)?,
                    invoice: r.get(4)?,
                    created_at: r.get(5)?,
                    expires_at: r.get(6)?,
                },
                r.get::<_, String>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })
        .optional()?;
    let Some((top_up, state, observed_at, detail)) = row else {
        return Ok(None);
    };
    Ok(Some(Purchase {
        top_up,
        state: PurchaseState::parse(&state).ok_or(Error::Invalid("purchase state"))?,
        observed_at,
        detail,
    }))
}
