//! A local money ledger. Mutations require exclusive access to one writer.
//! Times are Unix seconds; amounts are nonnegative integer millisatoshis.
//! LSP fees are receiver costs, not payable claims: the fee is stored on the
//! settlement and its `lsp_fee` share is zero. Splits use net receipts, so
//! OpenAgents absorbs that cost rather than reducing the author's declared fee.
//! First-call bonus funding never rewrites settlement shares. Payout callers
//! use `Ledger::available_shares` to read claims after those transfers.

use chrono::{DateTime, Datelike};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub mod adjustment;
pub mod agent_order;
pub mod commission;
pub mod commission_abuse;
pub mod compute;
pub mod contribution;
pub mod earnings;
pub mod markets;
pub mod payee;
pub mod payout;
pub mod pylon;
pub mod reconcile;
pub mod session;
pub mod shared;

pub const V1: &str = include_str!("../rules/v1.toml");
/// The rule that gives a pylon's provider a share of a brokered compute
/// sale (`[pylon_job]`). A ledger that brokers pylon jobs installs it with
/// [`Ledger::load_rule`]; [`pylon`] does so when it opens.
pub const V2: &str = include_str!("../rules/v2.toml");
pub const OPENAGENTS: &str = "openagents";
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error("Invalid ledger input: {0}")]
    Invalid(&'static str),
    #[error("No rule is effective at this settlement time")]
    NoRule,
    /// The same identity was used again with other terms.
    #[error("Conflicting ledger input: {0}")]
    Conflict(&'static str),
    /// A hold asked for more than the account has available.
    #[error("Insufficient purchased balance: {available_msat} msat available")]
    Insufficient { available_msat: i64 },
    /// The principal may not read or spend this balance.
    #[error("Not permitted: {0}")]
    Denied(&'static str),
}

pub fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    version: i64,
    effective: String,
    plugin_call: PluginRule,
    hosted_resource: ResourceRule,
    bonus: BonusRule,
    #[serde(default)]
    pylon_job: Option<PylonJobRule>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PylonJobRule {
    provider_bps: u16,
    openagents: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginRule {
    author: String,
    openagents: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceRule {
    resource_owner_bps: u16,
    openagents: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BonusRule {
    first_paid_call_msat: u64,
    launch_match_bps: u16,
    launch_match_until: String,
    launch_match_cap_msat_per_month: u64,
    funded_by: String,
}
fn parse_rule(text: &str) -> Result<(Rule, i64)> {
    let rule: Rule = toml::from_str(text)?;
    let effective = DateTime::parse_from_rfc3339(&rule.effective)
        .map_err(|_| Error::Invalid("rule effective time"))?
        .timestamp();
    if rule.version <= 0
        || rule.plugin_call.author != "fee"
        || rule.plugin_call.openagents != "rest"
        || rule.hosted_resource.openagents != "rest"
        || rule.hosted_resource.resource_owner_bps > 10_000
        || rule.bonus.funded_by != "openagents_share"
        || rule.bonus.launch_match_bps > 10_000
        || rule.bonus.first_paid_call_msat > i64::MAX as u64
        || rule.bonus.launch_match_cap_msat_per_month > i64::MAX as u64
        || DateTime::parse_from_rfc3339(&rule.bonus.launch_match_until).is_err()
        || rule
            .pylon_job
            .as_ref()
            .is_some_and(|p| p.provider_bps > 10_000 || p.openagents != "rest")
    {
        return Err(Error::Invalid("unsupported split rule"));
    }
    Ok((rule, effective))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    Lightning,
    Balance,
}
impl Rail {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lightning => "lightning",
            Self::Balance => "balance",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EarnedKind {
    Worker,
    Contribution,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Split {
    Plugin {
        author: String,
        fee_msat: i64,
    },
    HostedResource {
        owner: String,
    },
    OpenAgents,
    /// An accepted, fully funded obligation; no plugin launch bonuses.
    Earned {
        beneficiary: String,
        amount_msat: i64,
        kind: EarnedKind,
    },
    /// A brokered pylon compute job: the provider's share under the
    /// effective rule's `[pylon_job]`, bound to the NIP-PYLON receipt
    /// (`3201` event ID) it pays.
    ///
    /// When the job used a priced plugin, `plugin` pays its author the
    /// plugin's declared per-call fee first, as `[plugin_call]` does; the
    /// provider's share is then taken from what remains.
    PylonJob {
        provider: String,
        receipt: String,
        plugin: Option<PluginFee>,
    },
    /// One agent's NIP-MKT order to another, whose compute ran on the pool
    /// (`docs/compute/verse-compute.md`, P4): the selling agent's service
    /// fee comes first, as an author's per-call fee does, the provider's
    /// share under `[pylon_job]` is of what remains, and OpenAgents keeps
    /// the rest. It binds the order (`order`, its 64-hex order ID) and the
    /// NIP-PYLON receipt (`3201` event ID) of the job that ran it, one
    /// settlement each.
    AgentOrder {
        seller: String,
        fee_msat: i64,
        provider: String,
        receipt: String,
        order: String,
    },
}
/// A priced plugin a pylon job used: its author's per-call fee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFee {
    /// The plugin's ID; the settlement records it as `plugin_id`.
    pub plugin_id: String,
    pub author: String,
    pub fee_msat: i64,
}
#[derive(Debug, Clone)]
pub struct SettlementInput {
    /// A payment hash, or `debit:{id}` for a prepaid balance debit.
    pub key: String,
    pub resource: String,
    pub plugin_id: Option<String>,
    /// The plugin release the payment bought, when it bought one.
    pub release_id: Option<String>,
    pub price_msat: i64,
    pub received_msat: i64,
    pub rail: Rail,
    pub payer_alias: Option<String>,
    pub settled_at: i64,
    pub split: Split,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub settlement: String,
    pub party: String,
    pub role: String,
    pub amount_msat: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bonus {
    pub kind: String,
    pub party: String,
    pub requested_msat: i64,
    pub amount_msat: i64,
    pub outcome: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub seq: i64,
    pub key: String,
    pub resource: String,
    pub plugin_id: Option<String>,
    pub release_id: Option<String>,
    pub price_msat: i64,
    pub received_msat: i64,
    pub lsp_fee_msat: i64,
    pub rail: Rail,
    pub payer_alias: Option<String>,
    pub settled_at: i64,
    pub rule_version: i64,
    pub short: bool,
    pub shares: Vec<Share>,
    /// Launch matches are also in `shares`. First-call awards are separate
    /// claims funded by a journal over unpaid OpenAgents shares.
    pub bonuses: Vec<Bonus>,
}
/// One request that reached a route, paid or free. It names no payer,
/// payment hash, or request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRecord {
    pub at: i64,
    pub route: String,
    pub resource: String,
    pub plugin_id: Option<String>,
    pub release_id: Option<String>,
    pub outcome: String,
    pub paid: bool,
    pub price_msat: Option<i64>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Totals {
    pub settlements: i64,
    pub received_msat: i64,
    pub lsp_fee_msat: i64,
    pub accrued_msat: i64,
    /// Planned, sending, and unknown payouts stay reserved until they fail.
    pub reserved_msat: i64,
    pub paid_msat: i64,
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PluginTotals {
    pub settlements: i64,
    pub received_msat: i64,
    pub author_msat: i64,
    pub openagents_msat: i64,
    pub bonus_msat: i64,
}
#[derive(Debug, Clone)]
pub struct Payee {
    pub party: String,
    pub destination_kind: String,
    pub destination_value: String,
    pub source: String,
    pub verified_at: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
/// A payout's state. `planned` reserves the shares; `sending` means the
/// wallet reference (payment hash or Spark transfer id) is on disk and the
/// send may have started; `unknown` is a send whose outcome only a wallet
/// lookup can tell. `sent` and `failed` are final, and only `failed`
/// returns the shares to accrued.
pub enum PayoutState {
    Planned,
    Sending,
    Unknown,
    Sent,
    Failed,
}
impl PayoutState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
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

/// Replace the payable views only when their stored text differs. Reopening an
/// unchanged ledger must not rewrite it: a rewrite bumps the schema cookie and
/// file change counter, so byte-level checkpoint comparisons would see live
/// accounting change on every open.
fn install_payable_views(connection: &mut Connection) -> Result<()> {
    const SQL: &str = include_str!("payable.sql");
    let expected: Vec<(&str, &str)> = SQL
        .match_indices("CREATE VIEW ")
        .map(|(start, _)| {
            let rest = &SQL[start..];
            let statement = rest[..rest.find(';').unwrap_or(rest.len())].trim_end();
            let name = statement["CREATE VIEW ".len()..]
                .split_whitespace()
                .next()
                .unwrap_or_default();
            (name, statement)
        })
        .collect();
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut current = true;
    for (name, statement) in &expected {
        let stored: Option<String> = tx
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='view' AND name=?",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        current &= stored.as_deref() == Some(*statement);
    }
    if !current || expected.is_empty() {
        tx.execute_batch(SQL)?;
    }
    tx.commit()?;
    Ok(())
}

pub struct Ledger {
    connection: Connection,
    #[cfg(unix)]
    custody: Option<commission::Custody>,
    shared_client: Option<shared::Client>,
    shared_writer: bool,
}
impl Ledger {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }
    /// Open an existing ledger without creating files, installing rules, or migrating tables.
    /// Queries against an unsupported schema return an error.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self {
            connection,
            #[cfg(unix)]
            custody: None,
            shared_client: None,
            shared_writer: false,
        })
    }
    /// Stable identity of the native ledger records. It survives reopen and
    /// backup; paths, configured issuer names, and payer labels do not define it.
    /// A legacy read-only book must first receive the ordinary native migration.
    pub fn origin(&self) -> Result<String> {
        let id: String = self.connection.query_row(
            "SELECT id FROM ledger_origin WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        if id.len() != 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid("native ledger origin"));
        }
        Ok(id)
    }
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(include_str!("schema.sql"))?;
        payout::create_table(&mut connection)?;
        connection.execute_batch(include_str!("compute.sql"))?;
        connection.execute_batch(earnings::TABLES)?;
        connection.execute_batch(adjustment::TABLES)?;
        connection.execute_batch(commission::TABLES)?;
        connection.execute_batch(commission_abuse::TABLES)?;
        install_payable_views(&mut connection)?;
        // Existing ledgers predate plugin release attribution. Serialize the
        // check and alteration so concurrent receiver opens migrate once.
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let has_release: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('settlement') WHERE name='release_id')",
            [],
            |row| row.get(0),
        )?;
        if !has_release {
            tx.execute_batch("ALTER TABLE settlement ADD COLUMN release_id TEXT;")?;
        }
        tx.commit()?;
        connection.execute_batch(shared::TABLES)?;
        connection.execute_batch(shared::accounting::TABLES)?;
        connection.execute_batch(pylon::TABLES)?;
        connection.execute_batch(agent_order::TABLES)?;
        let mut ledger = Self {
            connection,
            #[cfg(unix)]
            custody: None,
            shared_client: None,
            shared_writer: false,
        };
        ledger.load_rule(V1, &digest(V1))?;
        Ok(ledger)
    }
    /// Verify the reviewed file's SHA-256 before installing an immutable version.
    pub fn load_rule(&mut self, text: &str, expected_digest: &str) -> Result<()> {
        if digest(text) != expected_digest {
            return Err(Error::Invalid("rule digest mismatch"));
        }
        let (rule, effective) = parse_rule(text)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT digest FROM rule WHERE version=?",
                [rule.version],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != expected_digest {
                return Err(Error::Invalid("rule version is immutable"));
            }
            return Ok(());
        }
        tx.execute(
            "INSERT INTO rule(version,digest,effective,toml) VALUES(?,?,?,?)",
            params![rule.version, expected_digest, effective, text],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn record_settlement(&mut self, input: SettlementInput) -> Result<Recorded> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let recorded = record_settlement_in(&tx, input)?;
        tx.commit()?;
        Ok(recorded)
    }
    /// Record one `call` usage record (a request that reached a route,
    /// paid or free) for the flow stream; returns its sequence number.
    pub fn record_call(&mut self, call: &CallRecord) -> Result<i64> {
        if call.route.is_empty() || call.price_msat.is_some_and(|msat| msat < 0) {
            return Err(Error::Invalid("call route or price"));
        }
        self.connection.execute(
            "INSERT INTO call(at,route,resource,plugin_id,release_id,outcome,paid,price_msat) VALUES(?,?,?,?,?,?,?,?)",
            params![call.at, call.route, call.resource, call.plugin_id, call.release_id, call.outcome, call.paid, call.price_msat],
        )?;
        Ok(self.connection.last_insert_rowid())
    }
    /// The `call` records after `seq`, oldest first.
    pub fn calls_since(&self, seq: i64) -> Result<Vec<(i64, CallRecord)>> {
        let mut statement = self.connection.prepare(
            "SELECT seq,at,route,resource,plugin_id,release_id,outcome,paid,price_msat FROM call WHERE seq>? ORDER BY seq",
        )?;
        let rows = statement.query_map([seq], |r| {
            Ok((
                r.get(0)?,
                CallRecord {
                    at: r.get(1)?,
                    route: r.get(2)?,
                    resource: r.get(3)?,
                    plugin_id: r.get(4)?,
                    release_id: r.get(5)?,
                    outcome: r.get(6)?,
                    paid: r.get(7)?,
                    price_msat: r.get(8)?,
                },
            ))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    /// Unreserved, unpaid claims for this party, in msat.
    pub fn accrued(&self, party: &str) -> Result<i64> {
        Ok(self.connection.query_row(
            &format!(
                "SELECT COALESCE(SUM(s.amount_msat),0) FROM payable_share s WHERE s.party=? AND {AVAILABLE}"
            ),
            [party],
            |r| r.get(0),
        )?)
    }
    /// Return whole payable claims at the current ledger state. Funding a
    /// first-call bonus can reduce an OpenAgents claim, so reserve these values
    /// rather than the immutable shares in a recorded settlement.
    pub fn available_shares(&self, party: &str) -> Result<Vec<Share>> {
        available_shares(&self.connection, party)
    }
    pub fn totals(&self) -> Result<Totals> {
        let mut total = self.connection.query_row("SELECT COUNT(*),COALESCE(SUM(received_msat),0),COALESCE(SUM(lsp_fee_msat),0) FROM settlement", [], |r| Ok(Totals { settlements:r.get(0)?, received_msat:r.get(1)?, lsp_fee_msat:r.get(2)?, ..Totals::default() }))?;
        total.accrued_msat = self.connection.query_row(
            &format!(
                "SELECT COALESCE(SUM(s.amount_msat),0) FROM payable_share s WHERE {AVAILABLE}"
            ),
            [],
            |r| r.get(0),
        )?;
        total.reserved_msat = self.connection.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM payout WHERE state IN ('planned','sending','unknown')",
            [],
            |r| r.get(0),
        )?;
        total.paid_msat = self.connection.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM payout WHERE state='sent'",
            [],
            |r| r.get(0),
        )?;
        Ok(total)
    }
    pub fn per_plugin(&self) -> Result<BTreeMap<String, PluginTotals>> {
        let mut stmt = self.connection.prepare("SELECT plugin_id,COUNT(*),SUM(received_msat),COALESCE(SUM((SELECT SUM(amount_msat) FROM share WHERE settlement=payment_hash AND role='author')),0),COALESCE(SUM((SELECT SUM(amount_msat) FROM payable_share WHERE settlement=payment_hash AND role='openagents')),0),COALESCE(SUM((SELECT SUM(amount_msat) FROM bonus WHERE settlement=payment_hash)),0) FROM settlement WHERE plugin_id IS NOT NULL GROUP BY plugin_id ORDER BY plugin_id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                PluginTotals {
                    settlements: r.get(1)?,
                    received_msat: r.get(2)?,
                    author_msat: r.get(3)?,
                    openagents_msat: r.get(4)?,
                    bonus_msat: r.get(5)?,
                },
            ))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    /// Return settlements after the exclusive sequence cursor, in commit order.
    pub fn since(&self, seq: i64) -> Result<Vec<Recorded>> {
        let mut stmt = self
            .connection
            .prepare("SELECT payment_hash FROM settlement WHERE seq>? ORDER BY seq")?;
        let keys = stmt
            .query_map([seq], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        keys.into_iter()
            .map(|key| {
                read_record(&self.connection, &key)?.ok_or(Error::Invalid("missing settlement"))
            })
            .collect()
    }
    /// Read one immutable settlement by its authoritative payment/debit key.
    /// Reporting adapters must still authorize their source and customer scope.
    pub fn settlement(&self, key: &str) -> Result<Option<Recorded>> {
        if key.is_empty() || key.len() > 512 || key.chars().any(char::is_control) {
            return Err(Error::Invalid("settlement identity"));
        }
        read_record(&self.connection, key)
    }
    /// Outstanding claims for one settlement. Reserved/unknown claims remain
    /// book liabilities; only a sent payout consumes them. A sent label is not
    /// wallet attestation. This performs no payment.
    pub fn settlement_liabilities(&self, key: &str) -> Result<Vec<Share>> {
        if key.is_empty() || key.len() > 512 || key.chars().any(char::is_control) {
            return Err(Error::Invalid("settlement identity"));
        }
        let mut stmt=self.connection.prepare("SELECT s.settlement,s.party,s.role,s.amount_msat FROM payable_share s WHERE s.settlement=? AND s.amount_msat>0 AND NOT EXISTS (SELECT 1 FROM payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state='sent') AND NOT EXISTS (SELECT 1 FROM bonus_payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state='sent') AND NOT EXISTS (SELECT 1 FROM commission_payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state='sent') ORDER BY s.party,s.role LIMIT 33")?;
        let rows = stmt
            .query_map([key], |r| {
                Ok(Share {
                    settlement: r.get(0)?,
                    party: r.get(1)?,
                    role: r.get(2)?,
                    amount_msat: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if rows.len() > 32 {
            return Err(Error::Invalid("settlement liabilities exceed bound"));
        }
        Ok(rows)
    }
    pub fn register_payee(&mut self, payee: Payee) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        register_payee_in(&tx, payee)?;
        tx.commit()?;
        Ok(())
    }
    /// Reserve whole shares atomically. Failed attempts release their shares;
    /// planned, sending, and unknown attempts do not. This method performs no payment.
    pub fn reserve_payout(
        &mut self,
        id: &str,
        party: &str,
        items: &[Share],
        at: i64,
    ) -> Result<i64> {
        self.reserve_payout_expected(id, party, items, at, None)
    }

    /// Reserve only if the destination still matches the resolved planning
    /// terms. A concurrent destination change leaves all shares unreserved.
    pub fn reserve_payout_at_destination(
        &mut self,
        id: &str,
        party: &str,
        items: &[Share],
        at: i64,
        payee: &Payee,
    ) -> Result<i64> {
        if payee.party != party {
            return Err(Error::Invalid("payout payee mismatch"));
        }
        let expected = format!("{}:{}", payee.destination_kind, payee.destination_value);
        self.reserve_payout_expected(id, party, items, at, Some(&expected))
    }

    fn reserve_payout_expected(
        &mut self,
        id: &str,
        party: &str,
        items: &[Share],
        at: i64,
        expected: Option<&str>,
    ) -> Result<i64> {
        if id.is_empty() || items.is_empty() {
            return Err(Error::Invalid("empty payout"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if commission::payouts_held_in(&tx)? {
            return Err(Error::Denied(
                "native commission collection, refund, or reversal funding remains unresolved",
            ));
        }
        let destination: String = tx.query_row(
            "SELECT destination_kind || ':' || destination_value FROM payee WHERE party=?",
            [party],
            |r| r.get(0),
        )?;
        if expected.is_some_and(|value| value != destination) {
            return Err(Error::Conflict("payout destination changed"));
        }
        let mut amount = 0i64;
        for item in items {
            if item.party != party {
                return Err(Error::Invalid("payout party mismatch"));
            }
            let value: Option<i64> = tx.query_row(&format!("SELECT s.amount_msat FROM payable_share s WHERE s.settlement=? AND s.party=? AND s.role=? AND {AVAILABLE}"), params![item.settlement,party,item.role], |r| r.get(0)).optional()?;
            let value = value.ok_or(Error::Invalid("share already reserved or absent"))?;
            if value <= 0 || item.amount_msat != value {
                return Err(Error::Invalid("payout must drain whole positive shares"));
            }
            amount = amount
                .checked_add(value)
                .ok_or(Error::Invalid("payout overflow"))?;
        }
        if !commission::qualify_reservation(&tx, party, &destination, amount, items)? {
            return Err(Error::Denied(
                "original commission destination or minimum is unqualified",
            ));
        }
        let rail = if destination.starts_with("spark:") {
            "spark"
        } else {
            "lightning"
        };
        let attempts = payout::failure_streak(&tx, party)?.0 + 1;
        tx.execute(
            "INSERT INTO payout(id,party,amount_msat,destination,rail,state,wallet_reference,attempts,created_at,updated_at) VALUES(?,?,?,?,?,'planned',NULL,?,?,?)",
            params![id, party, amount, destination, rail, attempts, at, at],
        )?;
        for item in items {
            tx.execute(
                "INSERT INTO native_payout_claim VALUES(?,?,?,?,?)",
                params![id, item.settlement, party, item.role, item.amount_msat],
            )?;
            let table = if item.role == "commission" {
                "commission_payout_item"
            } else if item.role == "first_paid_call" {
                "bonus_payout_item"
            } else {
                "payout_item"
            };
            tx.execute(
                &format!("INSERT INTO {table} VALUES(?,?,?,?)"),
                params![id, item.settlement, party, item.role],
            )?;
        }
        tx.commit()?;
        Ok(amount)
    }
    /// Move an open payout to `state`. A payout never goes back to
    /// `planned`, `sending` and `sent` need a wallet reference, and a
    /// reference once recorded never changes (`None` keeps it).
    pub fn set_payout_state(
        &mut self,
        id: &str,
        state: PayoutState,
        wallet_reference: Option<&str>,
        at: i64,
    ) -> Result<()> {
        if state == PayoutState::Planned {
            return Err(Error::Invalid("a payout never returns to planned"));
        }
        let existing: Option<Option<String>> = self
            .connection
            .query_row(
                "SELECT wallet_reference FROM payout WHERE id=? AND state IN ('planned','sending','unknown')",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(existing) = existing else {
            return Err(Error::Invalid("payout absent or terminal"));
        };
        if existing.is_some()
            && wallet_reference.is_some()
            && existing.as_deref() != wallet_reference
        {
            return Err(Error::Invalid("a payout's wallet reference never changes"));
        }
        let reference = existing.as_deref().or(wallet_reference);
        if matches!(state, PayoutState::Sent | PayoutState::Sending)
            && reference.is_none_or(str::is_empty)
        {
            return Err(Error::Invalid(
                "a sending or sent payout needs a wallet reference",
            ));
        }
        let changed = self.connection.execute("UPDATE payout SET state=?,wallet_reference=?,updated_at=? WHERE id=? AND state IN ('planned','sending','unknown')", params![state.as_str(),reference,at,id])?;
        if changed != 1 {
            return Err(Error::Invalid("payout absent or terminal"));
        }
        Ok(())
    }
}
pub(crate) fn register_payee_in(connection: &Connection, payee: Payee) -> Result<()> {
    if payee.party.is_empty()
        || payee.destination_value.is_empty()
        || payee.destination_kind.is_empty()
        || payee.source.is_empty()
    {
        return Err(Error::Invalid("payee destination"));
    }
    if payee.party.starts_with("referrer:") {
        let old: Option<(String, String)> = connection
            .query_row(
                "SELECT destination_kind,destination_value FROM payee WHERE party=?",
                [&payee.party],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if old
            .as_ref()
            .is_some_and(|v| v.0 != payee.destination_kind || v.1 != payee.destination_value)
        {
            commission_abuse::hold_payee_rebinding_in(
                connection,
                &payee.party,
                &payee.destination_kind,
                &payee.destination_value,
                u64::try_from(payee.verified_at)
                    .map_err(|_| Error::Invalid("native payee clock"))?,
            )?;
        }
    }
    connection.execute("INSERT INTO payee VALUES(?,?,?,?,?) ON CONFLICT(party) DO UPDATE SET destination_kind=excluded.destination_kind,destination_value=excluded.destination_value,source=excluded.source,verified_at=excluded.verified_at", params![payee.party,payee.destination_kind,payee.destination_value,payee.source,payee.verified_at])?;
    Ok(())
}

pub(crate) const AVAILABLE: &str = "(s.role!='commission' OR NOT EXISTS (SELECT 1 FROM commission_admission a JOIN commission_abuse h ON h.admission=a.id WHERE a.payment_hash=s.settlement AND h.state='held')) AND NOT EXISTS (SELECT 1 FROM commission_payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state!='failed') AND NOT EXISTS (SELECT 1 FROM payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state!='failed') AND NOT EXISTS (SELECT 1 FROM bonus_payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state!='failed')";
fn available_shares(connection: &Connection, party: &str) -> Result<Vec<Share>> {
    let mut stmt = connection.prepare(&format!("SELECT s.settlement,s.party,s.role,s.amount_msat FROM payable_share s JOIN settlement t ON t.payment_hash=s.settlement WHERE s.party=? AND s.amount_msat>0 AND {AVAILABLE} ORDER BY t.seq,s.role"))?;
    let rows = stmt.query_map([party], |r| {
        Ok(Share {
            settlement: r.get(0)?,
            party: r.get(1)?,
            role: r.get(2)?,
            amount_msat: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}
/// Record one settlement inside the caller's transaction. Replaying a key
/// returns the recorded settlement unchanged.
pub(crate) fn record_settlement_in(tx: &Connection, input: SettlementInput) -> Result<Recorded> {
    // Check the key before validating the replay payload or choosing a rule.
    if let Some(existing) = read_record(&tx, &input.key)? {
        return Ok(existing);
    }
    if input.key.is_empty()
        || input.price_msat < 0
        || input.received_msat < 0
        || input.received_msat > input.price_msat
        || (input.rail == Rail::Balance) != input.key.starts_with("debit:")
        || input.key == "debit:"
    {
        return Err(Error::Invalid("settlement key, rail, or amounts"));
    }
    let (version, text, expected): (i64, String, String) = tx.query_row(
            "SELECT version,toml,digest FROM rule WHERE effective<=? ORDER BY effective DESC LIMIT 1",
            [input.settled_at], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or(Error::NoRule)?;
    if digest(&text) != expected {
        return Err(Error::Invalid("stored rule digest mismatch"));
    }
    let (rule, _) = parse_rule(&text)?;
    let mut shares = vec![];
    let mut short = false;
    let allocated = match &input.split {
        Split::Plugin { author, fee_msat } => {
            if author.is_empty()
                || input.plugin_id.as_deref().is_none_or(str::is_empty)
                || *fee_msat < 0
                || *fee_msat > input.price_msat
            {
                return Err(Error::Invalid("plugin id, author, or declared fee"));
            }
            short = input.received_msat < *fee_msat;
            let amount = input.received_msat.min(*fee_msat);
            shares.push((author.as_str(), "author", amount));
            amount
        }
        Split::HostedResource { owner } => {
            if owner.is_empty() {
                return Err(Error::Invalid("resource owner"));
            }
            let amount = ((input.received_msat as i128
                * rule.hosted_resource.resource_owner_bps as i128)
                / 10_000) as i64;
            shares.push((owner.as_str(), "resource", amount));
            amount
        }
        Split::Earned {
            beneficiary,
            amount_msat,
            kind,
        } => {
            if beneficiary.is_empty()
                || beneficiary == OPENAGENTS
                || *amount_msat <= 0
                || *amount_msat > input.received_msat
                || input.received_msat != input.price_msat
                || input.plugin_id.is_some()
                || input.release_id.is_some()
            {
                return Err(Error::Invalid(
                    "earned beneficiary, funding, or classification",
                ));
            }
            let role = match kind {
                EarnedKind::Worker => "provider",
                EarnedKind::Contribution => "author",
            };
            shares.push((beneficiary.as_str(), role, *amount_msat));
            *amount_msat
        }
        Split::PylonJob {
            provider,
            receipt,
            plugin,
        } => {
            if input.plugin_id.as_deref() != plugin.as_ref().map(|p| p.plugin_id.as_str()) {
                return Err(Error::Invalid(
                    "pylon job provider, receipt, or classification",
                ));
            }
            // The plugin author's fee comes first, as `[plugin_call]` pays
            // it; the provider's share is of what remains.
            let author = match plugin {
                Some(p) => {
                    if p.plugin_id.is_empty() {
                        return Err(Error::Invalid("plugin id, author, or declared fee"));
                    }
                    Some((p.author.as_str(), p.fee_msat))
                }
                None => None,
            };
            pylon_split(
                tx,
                &rule,
                &input,
                provider,
                receipt,
                author,
                &mut shares,
                &mut short,
            )?
        }
        Split::AgentOrder {
            seller,
            fee_msat,
            provider,
            receipt,
            order,
        } => {
            if input.plugin_id.is_some()
                || !pylon::is_event_id(order)
                || input.resource != agent_order::RESOURCE
            {
                return Err(Error::Invalid("agent order, resource, or classification"));
            }
            if agent_order::settlement_for(tx, order)?.is_some() {
                return Err(Error::Conflict("the order already has a settlement"));
            }
            pylon_split(
                tx,
                &rule,
                &input,
                provider,
                receipt,
                Some((seller.as_str(), *fee_msat)),
                &mut shares,
                &mut short,
            )?
        }
        Split::OpenAgents => 0,
    };
    let mut openagents = input.received_msat - allocated;
    let mut launch_match = None;
    if let Split::Plugin { author, fee_msat } = &input.split
        && input.received_msat > 0
    {
        let month = month(input.settled_at)?;
        let until = DateTime::parse_from_rfc3339(&rule.bonus.launch_match_until)
            .map_err(|_| Error::Invalid("bonus window"))?
            .timestamp();
        if input.settled_at < until {
            let requested =
                (*fee_msat as i128 * rule.bonus.launch_match_bps as i128 / 10_000) as i64;
            let mut stmt = tx.prepare(
                "SELECT amount_msat FROM bonus WHERE party=? AND month=? AND kind='launch_match'",
            )?;
            let spent = stmt
                .query_map(params![author, month], |r| r.get::<_, i64>(0))?
                .try_fold(0i64, |sum, amount| amount.map(|n| sum.saturating_add(n)))?;
            let remaining = (rule.bonus.launch_match_cap_msat_per_month as i64)
                .saturating_sub(spent)
                .max(0);
            let amount = requested.min(openagents).min(remaining);
            openagents -= amount;
            shares.push((author.as_str(), "bonus", amount));
            launch_match = Some((month, requested, amount));
        }
    }
    shares.extend([
        (OPENAGENTS, "openagents", openagents),
        (OPENAGENTS, "lsp_fee", 0),
        (OPENAGENTS, "provider", 0),
    ]);
    tx.execute("INSERT INTO settlement(payment_hash,resource,plugin_id,release_id,price_msat,received_msat,lsp_fee_msat,rail,payer_alias,settled_at,rule_version,short) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            params![input.key,input.resource,input.plugin_id,input.release_id,input.price_msat,input.received_msat,input.price_msat-input.received_msat,input.rail.as_str(),input.payer_alias,input.settled_at,version,short])?;
    for (party, role, amount) in shares {
        tx.execute(
            "INSERT INTO share(settlement,party,role,amount_msat) VALUES(?,?,?,?)",
            params![input.key, party, role, amount],
        )?;
    }
    if let Split::PylonJob {
        provider, receipt, ..
    }
    | Split::AgentOrder {
        provider, receipt, ..
    } = &input.split
    {
        tx.execute(
            "INSERT INTO pylon_job(settlement,receipt,provider) VALUES(?,?,?)",
            params![input.key, receipt, provider],
        )?;
    }
    if let Split::AgentOrder { seller, order, .. } = &input.split {
        tx.execute(
            "INSERT INTO agent_order(settlement,order_id,seller) VALUES(?,?,?)",
            params![input.key, order, seller],
        )?;
    }
    if let Split::Plugin { author, .. } = &input.split
        && input.received_msat > 0
    {
        if let Some((month, requested, amount)) = launch_match {
            tx.execute(
                "INSERT INTO bonus VALUES(?,?,'launch_match',?,?,?,?,'awarded')",
                params![input.key, author, input.plugin_id, month, requested, amount],
            )?;
        }
        first_call_bonus(&tx, &input, author, rule.bonus.first_paid_call_msat as i64)?;
    }
    read_record(tx, &input.key)?.ok_or(Error::Invalid("missing settlement"))
}
/// A job on the pool's split: `author`'s fee (a plugin author's per-call
/// fee, or a selling agent's service fee) first, then the provider's
/// `[pylon_job]` share of what remains. Returns what it allocated.
#[allow(clippy::too_many_arguments)]
fn pylon_split<'a>(
    tx: &Connection,
    rule: &Rule,
    input: &SettlementInput,
    provider: &'a str,
    receipt: &str,
    author: Option<(&'a str, i64)>,
    shares: &mut Vec<(&'a str, &'static str, i64)>,
    short: &mut bool,
) -> Result<i64> {
    if provider.is_empty()
        || provider == OPENAGENTS
        || !pylon::is_event_id(receipt)
        || input.release_id.is_some()
    {
        return Err(Error::Invalid(
            "pylon job provider, receipt, or classification",
        ));
    }
    let bps = rule
        .pylon_job
        .as_ref()
        .ok_or(Error::Invalid("the effective rule has no pylon job split"))?
        .provider_bps;
    if pylon::settlement_for(tx, receipt)?.is_some() {
        return Err(Error::Conflict("the receipt already has a settlement"));
    }
    let fee = match author {
        Some((author, fee_msat)) => {
            if author.is_empty()
                || author == OPENAGENTS
                || fee_msat < 0
                || fee_msat > input.price_msat
            {
                return Err(Error::Invalid("plugin id, author, or declared fee"));
            }
            *short = input.received_msat < fee_msat;
            let fee = input.received_msat.min(fee_msat);
            shares.push((author, "author", fee));
            fee
        }
        None => 0,
    };
    let amount = (((input.received_msat - fee) as i128 * bps as i128) / 10_000) as i64;
    shares.push((provider, "provider", amount));
    Ok(fee + amount)
}
fn month(at: i64) -> Result<String> {
    let at = DateTime::from_timestamp(at, 0).ok_or(Error::Invalid("settlement time"))?;
    Ok(format!("{:04}-{:02}", at.year(), at.month()))
}
fn first_call_bonus(
    connection: &Connection,
    input: &SettlementInput,
    author: &str,
    requested: i64,
) -> Result<()> {
    let plugin = input
        .plugin_id
        .as_deref()
        .ok_or(Error::Invalid("plugin id"))?;
    // Existing ledgers can contain paid calls recorded before bonus support.
    // Do not give an established plugin a second first-call opportunity.
    let seen: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM bonus WHERE plugin_id=? AND kind='first_paid_call') OR EXISTS(SELECT 1 FROM settlement t JOIN share s ON s.settlement=t.payment_hash WHERE t.plugin_id=? AND t.payment_hash!=? AND t.received_msat>0 AND s.role='author')",
        params![plugin,plugin,input.key], |r| r.get(0))?;
    if seen {
        return Ok(());
    }
    let mut remaining = requested;
    let mut funding = vec![];
    for source in available_shares(connection, OPENAGENTS)? {
        if remaining == 0 {
            break;
        }
        if source.role != "openagents" {
            continue;
        }
        let amount = source.amount_msat.min(remaining);
        remaining -= amount;
        funding.push((source.settlement, amount));
    }
    let funded = remaining == 0;
    connection.execute(
        "INSERT INTO bonus VALUES(?,?,'first_paid_call',?,?,?,?,?)",
        params![
            input.key,
            author,
            plugin,
            month(input.settled_at)?,
            requested,
            if funded { requested } else { 0 },
            if funded { "awarded" } else { "bonus_unfunded" }
        ],
    )?;
    if funded {
        for (source, amount) in funding {
            connection.execute("INSERT INTO bonus_funding VALUES(?,?,'first_paid_call',?,'openagents','openagents',?)",
                params![input.key,author,source,amount])?;
        }
    }
    Ok(())
}
fn read_record(connection: &Connection, key: &str) -> Result<Option<Recorded>> {
    let mut record = connection.query_row("SELECT seq,payment_hash,resource,plugin_id,price_msat,received_msat,lsp_fee_msat,rail,payer_alias,settled_at,rule_version,short,release_id FROM settlement WHERE payment_hash=?", [key], |r| Ok(Recorded {
        seq:r.get(0)?,key:r.get(1)?,resource:r.get(2)?,plugin_id:r.get(3)?,release_id:r.get(12)?,price_msat:r.get(4)?,received_msat:r.get(5)?,lsp_fee_msat:r.get(6)?,rail:if r.get::<_,String>(7)? == "balance" { Rail::Balance } else { Rail::Lightning },payer_alias:r.get(8)?,settled_at:r.get(9)?,rule_version:r.get(10)?,short:r.get(11)?,shares:vec![],bonuses:vec![]
    })).optional()?;
    if let Some(record) = &mut record {
        record.shares = connection.prepare("SELECT settlement,party,role,amount_msat FROM share WHERE settlement=? ORDER BY party,role")?.query_map([key], |r| Ok(Share { settlement:r.get(0)?,party:r.get(1)?,role:r.get(2)?,amount_msat:r.get(3)? }))?.collect::<std::result::Result<_,_>>()?;
        record.bonuses = connection.prepare("SELECT kind,party,requested_msat,amount_msat,outcome FROM bonus WHERE settlement=? ORDER BY kind,party")?.query_map([key], |r| Ok(Bonus { kind:r.get(0)?,party:r.get(1)?,requested_msat:r.get(2)?,amount_msat:r.get(3)?,outcome:r.get(4)? }))?.collect::<std::result::Result<_,_>>()?;
    }
    Ok(record)
}

#[cfg(test)]
mod origin_tests {
    use super::*;
    #[test]
    fn reopening_an_unchanged_ledger_leaves_its_bytes_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.db");
        drop(Ledger::open(&path).unwrap());
        let before = std::fs::read(&path).unwrap();
        drop(Ledger::open(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        // A changed view definition is still replaced on open.
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP VIEW payable_share; CREATE VIEW payable_share AS SELECT 1 AS stale;",
            )
            .unwrap();
        drop(connection);
        let book = Ledger::open(&path).unwrap();
        let stored: String = book
            .connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='view' AND name='payable_share'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stored.contains("FROM share s"));
    }
    #[test]
    fn native_origin_survives_reopen_copy_and_read_only_without_relabeling() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.db");
        let book = Ledger::open(&path).unwrap();
        let origin = book.origin().unwrap();
        assert!(
            book.connection
                .execute("UPDATE ledger_origin SET id=lower(hex(randomblob(32)))", [])
                .is_err()
        );
        assert!(
            book.connection
                .execute("DELETE FROM ledger_origin", [])
                .is_err()
        );
        assert!(book.connection.execute("INSERT OR REPLACE INTO ledger_origin(singleton,id) VALUES(1,lower(hex(randomblob(32))))", []).is_err());
        drop(book);
        assert_eq!(Ledger::open(&path).unwrap().origin().unwrap(), origin);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            Ledger::open_read_only(&path).unwrap().origin().unwrap(),
            origin
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let backup = dir.path().join("backup.db");
        std::fs::copy(&path, &backup).unwrap();
        assert_eq!(Ledger::open(&backup).unwrap().origin().unwrap(), origin);
        assert_ne!(
            Ledger::open(dir.path().join("other.db"))
                .unwrap()
                .origin()
                .unwrap(),
            origin
        );
    }
}
