//! Agents hiring agents on pooled compute (`docs/compute/verse-compute.md`,
//! P4).
//!
//! One agent hires another through a NIP-MKT order under the NIP-LAB
//! profile; the order's compute runs on a pylon the broker buys it from;
//! and the buyer pays the order's fixed price to OpenAgents' receiver
//! after acceptance. That payment settles here under
//! [`crate::Split::AgentOrder`]: the selling agent's service fee first
//! (role `author`), the pylon provider's `[pylon_job]` share of what
//! remains (role `provider`), and OpenAgents the rest. Each settlement
//! names the order and the `3201` receipt of the job that ran it, one
//! settlement per order and per receipt, so a reader matches every share
//! to a public receipt.
//!
//! The provider's share is a pylon job like any other: it is paid by
//! balance sweeps, and a trusted checker's `check-fail` forfeits it while
//! unpaid ([`crate::Ledger::forfeit_pylon_job`]); a forfeit never takes
//! the seller's fee.

use rusqlite::{Connection, OptionalExtension};

use crate::{Ledger, Result};

/// The resource an agent order settlement records.
pub const RESOURCE: &str = "openagents.agent.order.v1";

pub(crate) const TABLES: &str = "CREATE TABLE IF NOT EXISTS agent_order (
 settlement TEXT PRIMARY KEY REFERENCES settlement(payment_hash),
 order_id TEXT NOT NULL UNIQUE,
 seller TEXT NOT NULL
);";

/// One settled agent order as the ledger holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// The settlement key: the buyer's payment hash.
    pub settlement: String,
    /// The NIP-MKT order ID.
    pub order: String,
    /// The `3201` receipt of the job that ran the order.
    pub receipt: String,
    /// The selling agent's party.
    pub seller: String,
    /// The pylon provider's party.
    pub provider: String,
    pub received_msat: i64,
    /// The seller's fee as recorded.
    pub seller_msat: i64,
    /// The provider's share as recorded.
    pub provider_msat: i64,
    /// What OpenAgents kept.
    pub openagents_msat: i64,
}

pub(crate) fn settlement_for(c: &Connection, order: &str) -> Result<Option<String>> {
    Ok(c.query_row(
        "SELECT settlement FROM agent_order WHERE order_id=?",
        [order],
        |r| r.get(0),
    )
    .optional()?)
}

fn order_in(c: &Connection, settlement: &str) -> Result<Option<Order>> {
    Ok(c.query_row(
        "SELECT a.settlement,a.order_id,j.receipt,a.seller,j.provider,t.received_msat,
         COALESCE((SELECT amount_msat FROM share WHERE settlement=a.settlement AND party=a.seller AND role='author'),0),
         COALESCE((SELECT amount_msat FROM share WHERE settlement=a.settlement AND party=j.provider AND role='provider'),0),
         COALESCE((SELECT SUM(amount_msat) FROM share WHERE settlement=a.settlement AND role='openagents'),0)
         FROM agent_order a JOIN pylon_job j ON j.settlement=a.settlement
         JOIN settlement t ON t.payment_hash=a.settlement WHERE a.settlement=?",
        [settlement],
        |r| {
            Ok(Order {
                settlement: r.get(0)?,
                order: r.get(1)?,
                receipt: r.get(2)?,
                seller: r.get(3)?,
                provider: r.get(4)?,
                received_msat: r.get(5)?,
                seller_msat: r.get(6)?,
                provider_msat: r.get(7)?,
                openagents_msat: r.get(8)?,
            })
        },
    )
    .optional()?)
}

impl Ledger {
    /// The settled agent order `order` (its NIP-MKT order ID), if any.
    ///
    /// # Errors
    ///
    /// A storage failure.
    pub fn agent_order(&self, order: &str) -> Result<Option<Order>> {
        match settlement_for(&self.connection, order)? {
            Some(settlement) => order_in(&self.connection, &settlement),
            None => Ok(None),
        }
    }

    /// Every settled agent order, in settlement order.
    ///
    /// # Errors
    ///
    /// A storage failure.
    pub fn agent_orders(&self) -> Result<Vec<Order>> {
        let mut statement = self.connection.prepare(
            "SELECT a.settlement FROM agent_order a JOIN settlement t ON t.payment_hash=a.settlement ORDER BY t.seq",
        )?;
        let keys = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        keys.iter()
            .filter_map(|key| order_in(&self.connection, key).transpose())
            .collect()
    }
}
