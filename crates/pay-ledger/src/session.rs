//! Metered Lightning payment sessions (#10721), profile
//! `openagents.mpp.lightning-session.v1` (`docs/cloud/mpp-sessions.md`).
//!
//! A session is a separate rail from the prepaid compute balance: a payer
//! deposits once over Lightning, the service debits admitted resource
//! records against the deposit at a frozen rate up to a ceiling, and closing
//! (or expiry) turns the remainder into a refund owed to the payer's explicit
//! return destination. x402 `exact` is a fixed charge and provides none of
//! this.
//!
//! - A session opens once per deposit payment hash, so a reconnect cannot
//!   fund it twice.
//! - Each debit names one admitted resource record (a task or a graph node)
//!   and posts once, as the settlement `mpp:<session>:<record>`, in the same
//!   transaction. The record must carry the session's admission digest and
//!   one of its recipients: a session cannot widen recipients or effects.
//! - A refund is a liability until the wallet proves it sent. `sending` and
//!   `unknown` stay owed and are never retried blindly; `failed` is owed and
//!   may be planned again.
//!
//! Conservation: deposit = debited + refunded + owed + open remainder.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Error, Ledger, Rail, Result, SettlementInput, Split, record_settlement_in};

/// The session profile this module implements.
pub const PROFILE: &str = "openagents.mpp.lightning-session.v1";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mpp_session (
    id TEXT PRIMARY KEY,
    profile TEXT NOT NULL,
    deposit_hash TEXT NOT NULL UNIQUE,
    deposit_msat INTEGER NOT NULL CHECK(deposit_msat > 0),
    ceiling_msat INTEGER NOT NULL CHECK(ceiling_msat > 0 AND ceiling_msat <= deposit_msat),
    rate_msat_per_unit INTEGER NOT NULL CHECK(rate_msat_per_unit > 0),
    unit TEXT NOT NULL,
    admission TEXT NOT NULL,
    recipients TEXT NOT NULL,
    return_destination TEXT NOT NULL,
    opened_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK(expires_at > opened_at),
    state TEXT NOT NULL CHECK(state IN ('open','closed','expired')),
    closed_at INTEGER
);
CREATE TABLE IF NOT EXISTS mpp_debit (
    session TEXT NOT NULL REFERENCES mpp_session(id),
    record TEXT NOT NULL,
    node TEXT NOT NULL,
    recipient TEXT NOT NULL,
    units INTEGER NOT NULL CHECK(units > 0),
    amount_msat INTEGER NOT NULL CHECK(amount_msat > 0),
    at INTEGER NOT NULL,
    PRIMARY KEY(session, record)
);
CREATE TABLE IF NOT EXISTS mpp_refund (
    session TEXT PRIMARY KEY REFERENCES mpp_session(id),
    amount_msat INTEGER NOT NULL CHECK(amount_msat > 0),
    destination TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('planned','sending','unknown','sent','failed')),
    reference TEXT,
    updated_at INTEGER NOT NULL
);
";

pub(crate) fn create_tables(connection: &Connection) -> Result<()> {
    connection.execute_batch(SCHEMA)?;
    Ok(())
}

/// A session to open, after its deposit was observed paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSession {
    pub id: String,
    pub deposit_hash: String,
    /// What the wallet received for the deposit invoice.
    pub deposit_msat: i64,
    pub ceiling_msat: i64,
    /// The charge basis: millisatoshis per unit.
    pub rate_msat_per_unit: i64,
    /// What a unit is: `second`, `token`, `node`.
    pub unit: String,
    /// The admission digest every debited record must carry.
    pub admission: String,
    /// The only recipients a debit may name.
    pub recipients: Vec<String>,
    /// Where the remainder returns: an explicit BOLT12 offer or Lightning
    /// address the payer gave. Never inferred.
    pub return_destination: String,
    pub opened_at: i64,
    pub expires_at: i64,
}

/// One admitted resource record to charge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Debit {
    /// The resource record's identity; a retry reuses it.
    pub record: String,
    /// The task or graph node it belongs to.
    pub node: String,
    pub admission: String,
    pub recipient: String,
    pub units: i64,
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Open,
    Closed,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundState {
    Planned,
    /// The wallet reference is on disk and the send may have started.
    Sending,
    /// Only a wallet lookup can tell.
    Unknown,
    Sent,
    /// Owed again; may be planned again.
    Failed,
}

impl RefundState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Sending => "sending",
            Self::Unknown => "unknown",
            Self::Sent => "sent",
            Self::Failed => "failed",
        }
    }
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "planned" => Self::Planned,
            "sending" => Self::Sending,
            "unknown" => Self::Unknown,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

/// A session's money, which always conserves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub id: String,
    pub state: SessionState,
    pub deposit_msat: i64,
    pub debited_msat: i64,
    pub refunded_msat: i64,
    /// Planned, sending, unknown, or failed refunds: a liability.
    pub owed_msat: i64,
    /// Still spendable while the session is open.
    pub open_remainder_msat: i64,
    pub refund: Option<RefundState>,
}

impl Ledger {
    fn session_tables(&self) -> Result<()> {
        create_tables(&self.connection)
    }

    /// Open a session on a paid deposit. Opening again with the same deposit
    /// returns the session unchanged; another session ID or other terms for
    /// the same deposit conflict.
    pub fn open_session(&mut self, open: &OpenSession) -> Result<Summary> {
        self.session_tables()?;
        if open.id.is_empty()
            || open.deposit_hash.len() != 64
            || open.deposit_msat <= 0
            || open.ceiling_msat <= 0
            || open.ceiling_msat > open.deposit_msat
            || open.rate_msat_per_unit <= 0
            || open.unit.is_empty()
            || open.admission.is_empty()
            || open.recipients.is_empty()
            || open.return_destination.trim().is_empty()
            || open.expires_at <= open.opened_at
        {
            return Err(Error::Invalid(
                "session deposit, ceiling, rate, admission, recipients, return destination, or expiry",
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let recipients = open.recipients.join("\n");
        let existing: Option<(String, String, i64, i64, i64, String, String, String)> = tx
            .query_row(
                "SELECT id,deposit_hash,deposit_msat,ceiling_msat,rate_msat_per_unit,admission,recipients,return_destination FROM mpp_session WHERE id=? OR deposit_hash=?",
                params![open.id, open.deposit_hash],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .optional()?;
        if let Some(found) = existing {
            let same = found
                == (
                    open.id.clone(),
                    open.deposit_hash.clone(),
                    open.deposit_msat,
                    open.ceiling_msat,
                    open.rate_msat_per_unit,
                    open.admission.clone(),
                    recipients,
                    open.return_destination.clone(),
                );
            drop(tx);
            if same {
                return self
                    .session(&open.id)?
                    .ok_or(Error::Invalid("missing session"));
            }
            return Err(Error::Conflict(
                "the deposit or session exists with other terms",
            ));
        }
        tx.execute(
            "INSERT INTO mpp_session(id,profile,deposit_hash,deposit_msat,ceiling_msat,rate_msat_per_unit,unit,admission,recipients,return_destination,opened_at,expires_at,state) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,'open')",
            params![
                open.id,
                PROFILE,
                open.deposit_hash,
                open.deposit_msat,
                open.ceiling_msat,
                open.rate_msat_per_unit,
                open.unit,
                open.admission,
                recipients,
                open.return_destination,
                open.opened_at,
                open.expires_at
            ],
        )?;
        tx.commit()?;
        self.session(&open.id)?
            .ok_or(Error::Invalid("missing session"))
    }

    /// Debit one admitted resource record. A retry of the same record with
    /// the same terms returns the session unchanged.
    ///
    /// # Errors
    ///
    /// A closed or expired session, another admission or recipient, a
    /// debit past the ceiling, or a reused record with other terms.
    pub fn debit_session(&mut self, session: &str, debit: &Debit) -> Result<Summary> {
        self.session_tables()?;
        if debit.record.is_empty() || debit.node.is_empty() || debit.units <= 0 {
            return Err(Error::Invalid("debit record, node, or units"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (state, rate, ceiling, admission, recipients, expires_at): (
            String,
            i64,
            i64,
            String,
            String,
            i64,
        ) = tx
            .query_row(
                "SELECT state,rate_msat_per_unit,ceiling_msat,admission,recipients,expires_at FROM mpp_session WHERE id=?",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()?
            .ok_or(Error::Invalid("no such session"))?;
        let amount = debit
            .units
            .checked_mul(rate)
            .ok_or(Error::Invalid("debit amount"))?;
        let previous: Option<(String, String, i64, i64)> = tx
            .query_row(
                "SELECT node,recipient,units,amount_msat FROM mpp_debit WHERE session=? AND record=?",
                params![session, debit.record],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        if let Some(found) = previous {
            drop(tx);
            if found
                == (
                    debit.node.clone(),
                    debit.recipient.clone(),
                    debit.units,
                    amount,
                )
                && debit.admission == admission
            {
                return self
                    .session(session)?
                    .ok_or(Error::Invalid("missing session"));
            }
            return Err(Error::Conflict("the record was debited with other terms"));
        }
        if state != "open" || debit.at >= expires_at {
            return Err(Error::Denied("the session is not open"));
        }
        if debit.admission != admission {
            return Err(Error::Denied(
                "the record is not under the session's admission",
            ));
        }
        if !recipients.split('\n').any(|r| r == debit.recipient) {
            return Err(Error::Denied("the session cannot add a recipient"));
        }
        let debited: i64 = tx.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM mpp_debit WHERE session=?",
            [session],
            |r| r.get(0),
        )?;
        if debited + amount > ceiling {
            return Err(Error::Insufficient {
                available_msat: ceiling - debited,
            });
        }
        tx.execute(
            "INSERT INTO mpp_debit(session,record,node,recipient,units,amount_msat,at) VALUES(?,?,?,?,?,?,?)",
            params![session, debit.record, debit.node, debit.recipient, debit.units, amount, debit.at],
        )?;
        record_settlement_in(
            &tx,
            SettlementInput {
                key: format!("mpp:{session}:{}", debit.record),
                resource: PROFILE.into(),
                plugin_id: None,
                release_id: None,
                price_msat: amount,
                received_msat: amount,
                rail: Rail::Lightning,
                payer_alias: None,
                settled_at: debit.at,
                split: Split::OpenAgents,
            },
        )?;
        tx.commit()?;
        self.session(session)?
            .ok_or(Error::Invalid("missing session"))
    }

    /// Close a session, or expire it when `at` is past its expiry. The
    /// remainder becomes one planned refund to the return destination.
    /// Closing a closed session changes nothing.
    pub fn close_session(&mut self, session: &str, at: i64) -> Result<Summary> {
        self.session_tables()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (state, deposit, destination, expires_at): (String, i64, String, i64) = tx
            .query_row(
                "SELECT state,deposit_msat,return_destination,expires_at FROM mpp_session WHERE id=?",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .ok_or(Error::Invalid("no such session"))?;
        if state == "open" {
            let next = if at >= expires_at {
                "expired"
            } else {
                "closed"
            };
            tx.execute(
                "UPDATE mpp_session SET state=?, closed_at=? WHERE id=?",
                params![next, at, session],
            )?;
            let debited: i64 = tx.query_row(
                "SELECT COALESCE(SUM(amount_msat),0) FROM mpp_debit WHERE session=?",
                [session],
                |r| r.get(0),
            )?;
            let remainder = deposit - debited;
            if remainder > 0 {
                tx.execute(
                    "INSERT INTO mpp_refund(session,amount_msat,destination,state,updated_at) VALUES(?,?,?,'planned',?)",
                    params![session, remainder, destination, at],
                )?;
            }
        }
        tx.commit()?;
        self.session(session)?
            .ok_or(Error::Invalid("missing session"))
    }

    /// Expire every open session past its expiry.
    pub fn expire_sessions(&mut self, now: i64) -> Result<Vec<String>> {
        self.session_tables()?;
        let ids = {
            let mut statement = self.connection.prepare(
                "SELECT id FROM mpp_session WHERE state='open' AND expires_at<=? ORDER BY id",
            )?;
            statement
                .query_map([now], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for id in &ids {
            self.close_session(id, now)?;
        }
        Ok(ids)
    }

    /// Move a session's refund. `planned -> sending` needs the wallet
    /// reference; `sending -> sent|failed|unknown`; `unknown -> sent|failed`
    /// only from a wallet lookup; `failed -> planned` to try again. A sent
    /// refund is final.
    pub fn set_refund(
        &mut self,
        session: &str,
        next: RefundState,
        reference: Option<&str>,
        at: i64,
    ) -> Result<Summary> {
        self.session_tables()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: String = tx
            .query_row(
                "SELECT state FROM mpp_refund WHERE session=?",
                [session],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::Invalid("no refund for this session"))?;
        let current = RefundState::parse(&current).ok_or(Error::Invalid("refund state"))?;
        use RefundState::{Failed, Planned, Sending, Sent, Unknown};
        let allowed = matches!(
            (current, next),
            (Planned, Sending)
                | (Sending, Sent | Failed | Unknown)
                | (Unknown, Sent | Failed)
                | (Failed, Planned)
        ) || current == next;
        if !allowed {
            return Err(Error::Invalid("refund transition"));
        }
        if next == Sending && reference.is_none_or(str::is_empty) {
            return Err(Error::Invalid(
                "a sending refund names its wallet reference",
            ));
        }
        if current != next {
            tx.execute(
                "UPDATE mpp_refund SET state=?, reference=COALESCE(?, reference), updated_at=? WHERE session=?",
                params![next.as_str(), reference, at, session],
            )?;
        }
        tx.commit()?;
        self.session(session)?
            .ok_or(Error::Invalid("missing session"))
    }

    /// A session's money and states.
    pub fn session(&self, session: &str) -> Result<Option<Summary>> {
        self.session_tables()?;
        let row: Option<(String, i64)> = self
            .connection
            .query_row(
                "SELECT state,deposit_msat FROM mpp_session WHERE id=?",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((state, deposit)) = row else {
            return Ok(None);
        };
        let debited: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM mpp_debit WHERE session=?",
            [session],
            |r| r.get(0),
        )?;
        let refund: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT amount_msat,state FROM mpp_refund WHERE session=?",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let state = match state.as_str() {
            "open" => SessionState::Open,
            "closed" => SessionState::Closed,
            _ => SessionState::Expired,
        };
        let (refunded, owed, refund_state) = match refund {
            Some((amount, s)) => {
                let s = RefundState::parse(&s).ok_or(Error::Invalid("refund state"))?;
                if s == RefundState::Sent {
                    (amount, 0, Some(s))
                } else {
                    (0, amount, Some(s))
                }
            }
            None => (0, 0, None),
        };
        let open_remainder = if state == SessionState::Open {
            deposit - debited
        } else {
            0
        };
        Ok(Some(Summary {
            id: session.into(),
            state,
            deposit_msat: deposit,
            debited_msat: debited,
            refunded_msat: refunded,
            owed_msat: owed,
            open_remainder_msat: open_remainder,
            refund: refund_state,
        }))
    }
}
