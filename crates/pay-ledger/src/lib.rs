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

pub mod payee;

pub const V1: &str = include_str!("../rules/v1.toml");
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
pub enum Split {
    Plugin { author: String, fee_msat: i64 },
    HostedResource { owner: String },
    OpenAgents,
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
    /// Pending and unknown payouts remain reserved until explicitly failed.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayoutState {
    Pending,
    Unknown,
    Succeeded,
    Failed,
}
impl PayoutState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Unknown => "unknown",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

pub struct Ledger {
    connection: Connection,
}
impl Ledger {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(include_str!("schema.sql"))?;
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
        let mut ledger = Self { connection };
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
                let mut stmt = tx.prepare("SELECT amount_msat FROM bonus WHERE party=? AND month=? AND kind='launch_match'")?;
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
        let recorded = read_record(&tx, &input.key)?.ok_or(Error::Invalid("missing settlement"))?;
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
            "SELECT COALESCE(SUM(amount_msat),0) FROM payout WHERE state IN ('pending','unknown')",
            [],
            |r| r.get(0),
        )?;
        total.paid_msat = self.connection.query_row(
            "SELECT COALESCE(SUM(amount_msat),0) FROM payout WHERE state='succeeded'",
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
    pub fn register_payee(&mut self, payee: Payee) -> Result<()> {
        if payee.party.is_empty()
            || payee.destination_value.is_empty()
            || payee.destination_kind.is_empty()
            || payee.source.is_empty()
        {
            return Err(Error::Invalid("payee destination"));
        }
        self.connection.execute("INSERT INTO payee VALUES(?,?,?,?,?) ON CONFLICT(party) DO UPDATE SET destination_kind=excluded.destination_kind,destination_value=excluded.destination_value,source=excluded.source,verified_at=excluded.verified_at", params![payee.party,payee.destination_kind,payee.destination_value,payee.source,payee.verified_at])?;
        Ok(())
    }
    /// Reserve whole shares atomically. Failed attempts release their shares;
    /// pending and unknown attempts do not. This method performs no payment.
    pub fn reserve_payout(
        &mut self,
        id: &str,
        party: &str,
        items: &[Share],
        at: i64,
    ) -> Result<i64> {
        if id.is_empty() || items.is_empty() {
            return Err(Error::Invalid("empty payout"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let destination: String = tx.query_row(
            "SELECT destination_kind || ':' || destination_value FROM payee WHERE party=?",
            [party],
            |r| r.get(0),
        )?;
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
        tx.execute(
            "INSERT INTO payout VALUES(?,?,?,?,?,'pending',NULL,1,?,?)",
            params![id, party, amount, destination, "lightning", at, at],
        )?;
        for item in items {
            let table = if item.role == "first_paid_call" {
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
    pub fn set_payout_state(
        &mut self,
        id: &str,
        state: PayoutState,
        wallet_reference: Option<&str>,
        at: i64,
    ) -> Result<()> {
        if state == PayoutState::Succeeded && wallet_reference.is_none_or(str::is_empty) {
            return Err(Error::Invalid("successful payout needs a wallet reference"));
        }
        let changed = self.connection.execute("UPDATE payout SET state=?,wallet_reference=?,updated_at=? WHERE id=? AND state IN ('pending','unknown')", params![state.as_str(),wallet_reference,at,id])?;
        if changed != 1 {
            return Err(Error::Invalid("payout absent or terminal"));
        }
        Ok(())
    }
}
const AVAILABLE: &str = "NOT EXISTS (SELECT 1 FROM payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state!='failed') AND NOT EXISTS (SELECT 1 FROM bonus_payout_item i JOIN payout p ON p.id=i.payout WHERE i.settlement=s.settlement AND i.party=s.party AND i.role=s.role AND p.state!='failed')";
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
