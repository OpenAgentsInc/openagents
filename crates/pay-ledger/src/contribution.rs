//! Read-only attribution of exact release shares and their recorded payout attempts.

use crate::{Error, Ledger, PayoutState, Result};

/// A local ledger receipt for one exact author share, without payer or invoice content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub settlement: String,
    pub resource: String,
    pub release_id: String,
    pub party: String,
    pub role: String,
    pub share_msat: i64,
    pub settled_at: i64,
    pub payout_id: Option<String>,
    pub payout_state: Option<PayoutState>,
    pub wallet_reference: Option<String>,
    /// The entire payout batch's sent amount, not this share's individual payment.
    pub sent_msat: Option<i64>,
}

impl Receipt {
    /// The exact share belongs to a payout recorded as sent with a wallet reference.
    /// This is a local ledger claim, not independent wallet verification.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.payout_state == Some(PayoutState::Sent)
            && self
                .wallet_reference
                .as_deref()
                .is_some_and(|s| !s.is_empty())
            && self.sent_msat.is_some_and(|amount| amount > 0)
    }
}

impl Ledger {
    /// Read author shares for an exact plugin, signed release, and payee.
    /// Failed and uncertain payout attempts remain visible. A party's unrelated
    /// payouts cannot establish settlement for these shares.
    pub fn contribution_receipts(
        &self,
        plugin_id: &str,
        release_id: &str,
        party: &str,
    ) -> Result<Vec<Receipt>> {
        if [plugin_id, release_id, party].iter().any(|value| {
            value.is_empty() || value.len() > 512 || value.chars().any(char::is_control)
        }) {
            return Err(Error::Invalid("contribution receipt identity"));
        }
        let mut statement = self.connection.prepare(
            "SELECT t.payment_hash,t.resource,t.release_id,s.party,s.role,s.amount_msat,
                    t.settled_at,p.id,p.state,p.wallet_reference,p.sent_msat
             FROM settlement t JOIN share s ON s.settlement=t.payment_hash
             LEFT JOIN payout_item i ON i.settlement=s.settlement
                  AND i.party=s.party AND i.role=s.role
             LEFT JOIN payout p ON p.id=i.payout AND p.party=s.party
             WHERE t.plugin_id=?1 AND t.release_id=?2 AND s.party=?3 AND s.role='author'
             ORDER BY t.seq,p.created_at,p.id LIMIT 1025",
        )?;
        let receipts = statement
            .query_map([plugin_id, release_id, party], |row| {
                let state: Option<String> = row.get(8)?;
                Ok(Receipt {
                    settlement: row.get(0)?,
                    resource: row.get(1)?,
                    release_id: row.get(2)?,
                    party: row.get(3)?,
                    role: row.get(4)?,
                    share_msat: row.get(5)?,
                    settled_at: row.get(6)?,
                    payout_id: row.get(7)?,
                    payout_state: state
                        .map(|s| PayoutState::parse(&s).unwrap_or(PayoutState::Unknown)),
                    wallet_reference: row.get(9)?,
                    sent_msat: row.get(10)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if receipts.len() > 1024 {
            return Err(Error::Invalid("contribution receipts exceed 1024 rows"));
        }
        Ok(receipts)
    }
}
