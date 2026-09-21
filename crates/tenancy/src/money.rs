//! Fixed-point prices and a durable workspace spending ledger.
//!
//! Amounts are millionths of one currency unit. This module supplies no launch
//! prices, payment processor, exchange rate, or implicit top-up. Unknown work
//! retains its entire hold until an explicit settlement or release reconciles it.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA: &str = "openagents.money.v1";
const MAX_LOG: u64 = 16 * 1024 * 1024;

/// Explicit billable units. Input excludes cached input; output excludes
/// reasoning. An adapter must resolve provider-specific overlapping counters.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    InputTokens,
    CachedInputTokens,
    OutputTokens,
    ReasoningTokens,
    ComputeMilliseconds,
}

pub type Usage = BTreeMap<Resource, u64>;

/// A rational rate avoids floating-point rounding. Each resource is rounded up
/// once per attempt to the nearest millionth of a currency unit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rate {
    pub millionths: u64,
    pub per_units: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub version: String,
    pub currency: String,
    pub model: String,
    pub capacity: String,
    pub policy: String,
    pub rates: BTreeMap<Resource, Rate>,
}

impl Price {
    pub fn quote(&self, usage: &Usage) -> Result<u64, String> {
        for value in [&self.version, &self.model, &self.capacity, &self.policy] {
            identity(value)?;
        }
        currency(&self.currency)?;
        if self.rates.is_empty() || self.rates.keys().ne(usage.keys()) {
            return Err("usage must explicitly supply every priced resource and no others".into());
        }
        let mut total = 0_u64;
        for (resource, rate) in &self.rates {
            if rate.per_units == 0 {
                return Err("price denominator must be positive".into());
            }
            let numerator = u128::from(usage[resource]) * u128::from(rate.millionths);
            let amount = numerator.div_ceil(u128::from(rate.per_units));
            total = total
                .checked_add(u64::try_from(amount).map_err(|_| "price overflow")?)
                .ok_or("price overflow")?;
        }
        Ok(total)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreditKind {
    Grant,
    TopUp,
    Adjustment,
}

/// These are privileged accounting operations. HTTP handlers must authorize the
/// workspace and action before calling the ledger; a key is not an account ID.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    Create {
        currency: String,
        spend_limit: u64,
        topups_allowed: bool,
    },
    Credit {
        amount: u64,
        credit_kind: CreditKind,
    },
    Reserve {
        attempt: String,
        request_digest: String,
        price: Price,
        maximum_usage: Usage,
    },
    Settle {
        attempt: String,
        usage: Usage,
        receipt: String,
        /// Same currency and fixed-point scale as the account. None is unknown.
        provider_cost: Option<u64>,
        hosting_cost: Option<u64>,
    },
    Unknown {
        attempt: String,
    },
    /// Requires independent evidence that no charge is due, especially after
    /// unknown completion. Timeout alone is not release evidence.
    Release {
        attempt: String,
    },
    Refund {
        attempt: String,
        amount: u64,
    },
    ReverseRefund {
        attempt: String,
        amount: u64,
    },
    /// Correct an unused credit. Outstanding holds and settled charges still
    /// have to fit; this cannot remove funds already committed to work.
    Debit {
        amount: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    pub workspace: String,
    pub source: String,
    pub audit: String,
    pub operation: Operation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Held,
    Unknown,
    Settled,
    Released,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hold {
    pub price: Price,
    pub request_digest: String,
    pub maximum_usage: Usage,
    pub reserved: u64,
    pub phase: Phase,
    pub retail_charge: Option<u64>,
    pub refunded: u64,
    pub provider_cost: Option<u64>,
    pub hosting_cost: Option<u64>,
    pub receipt: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Balance {
    pub currency: String,
    pub credited: u64,
    pub reserved: u64,
    pub settled: u64,
    pub refunded: u64,
    pub available: u64,
    pub spend_remaining: u64,
    /// Every price version the account has transacted under, sorted.
    ///
    /// An account API serves the versions beside the amounts so a caller
    /// can name the terms a charge was quoted under rather than inferring
    /// them from a current price list.
    pub price_versions: Vec<String>,
}

#[derive(Clone, Debug)]
struct Account {
    currency: String,
    spend_limit: u64,
    topups_allowed: bool,
    credited: u64,
    holds: BTreeMap<String, Hold>,
    prices: BTreeMap<String, Price>,
}

impl Account {
    fn balance(&self) -> Result<Balance, String> {
        let (mut reserved, mut settled, mut refunded) = (0_u64, 0_u64, 0_u64);
        for hold in self.holds.values() {
            if matches!(hold.phase, Phase::Held | Phase::Unknown) {
                reserved = reserved
                    .checked_add(hold.reserved)
                    .ok_or("balance overflow")?;
            }
            settled = settled
                .checked_add(hold.retail_charge.unwrap_or(0))
                .ok_or("balance overflow")?;
            refunded = refunded
                .checked_add(hold.refunded)
                .ok_or("balance overflow")?;
        }
        let used = settled
            .checked_sub(refunded)
            .and_then(|n| n.checked_add(reserved))
            .ok_or("invalid balance")?;
        Ok(Balance {
            currency: self.currency.clone(),
            credited: self.credited,
            reserved,
            settled,
            refunded,
            available: self
                .credited
                .checked_sub(used)
                .ok_or("insufficient credit")?,
            spend_remaining: self
                .spend_limit
                .checked_sub(used)
                .ok_or("workspace spend limit exceeded")?,
            price_versions: self.prices.keys().cloned().collect(),
        })
    }
}

#[derive(Clone, Default)]
struct State {
    accounts: BTreeMap<String, Account>,
    sources: BTreeMap<(String, String), Mutation>,
}

impl State {
    fn apply(&mut self, mutation: &Mutation) -> Result<bool, String> {
        identity(&mutation.workspace)?;
        identity(&mutation.source)?;
        identity(&mutation.audit)?;
        let source = (mutation.workspace.clone(), mutation.source.clone());
        if let Some(prior) = self.sources.get(&source) {
            return if prior == mutation {
                Ok(false)
            } else {
                Err("idempotency conflict".into())
            };
        }
        if let Operation::Create {
            currency: code,
            spend_limit,
            topups_allowed,
        } = &mutation.operation
        {
            currency(code)?;
            if self.accounts.contains_key(&mutation.workspace) {
                return Err("workspace account already exists".into());
            }
            self.accounts.insert(
                mutation.workspace.clone(),
                Account {
                    currency: code.clone(),
                    spend_limit: *spend_limit,
                    topups_allowed: *topups_allowed,
                    credited: 0,
                    holds: BTreeMap::new(),
                    prices: BTreeMap::new(),
                },
            );
        } else {
            let account = self
                .accounts
                .get_mut(&mutation.workspace)
                .ok_or("workspace account is missing")?;
            match &mutation.operation {
                Operation::Credit {
                    amount,
                    credit_kind,
                } => {
                    if *amount == 0
                        || (*credit_kind == CreditKind::TopUp && !account.topups_allowed)
                    {
                        return Err("credit is zero or top-up is not authorized".into());
                    }
                    account.credited = account
                        .credited
                        .checked_add(*amount)
                        .ok_or("credit overflow")?;
                }
                Operation::Debit { amount } => {
                    if *amount == 0 {
                        return Err("debit must be positive".into());
                    }
                    account.credited = account
                        .credited
                        .checked_sub(*amount)
                        .ok_or("debit exceeds credits")?;
                }
                Operation::Reserve {
                    attempt,
                    request_digest,
                    price,
                    maximum_usage,
                } => {
                    identity(attempt)?;
                    identity(request_digest)?;
                    if account.holds.contains_key(attempt) {
                        return Err(
                            "attempt already reserved; retry the original mutation source".into(),
                        );
                    }
                    if price.currency != account.currency {
                        return Err("price currency differs from workspace currency".into());
                    }
                    if account
                        .prices
                        .get(&price.version)
                        .is_some_and(|prior| prior != price)
                    {
                        return Err("price version was reused for changed terms".into());
                    }
                    let reserved = price.quote(maximum_usage)?;
                    account.prices.insert(price.version.clone(), price.clone());
                    account.holds.insert(
                        attempt.clone(),
                        Hold {
                            price: price.clone(),
                            request_digest: request_digest.clone(),
                            maximum_usage: maximum_usage.clone(),
                            reserved,
                            phase: Phase::Held,
                            retail_charge: None,
                            refunded: 0,
                            provider_cost: None,
                            hosting_cost: None,
                            receipt: None,
                        },
                    );
                }
                Operation::Settle {
                    attempt,
                    usage,
                    receipt,
                    provider_cost,
                    hosting_cost,
                } => {
                    identity(receipt)?;
                    let hold = pending(account, attempt)?;
                    if usage.iter().any(|(resource, actual)| {
                        hold.maximum_usage
                            .get(resource)
                            .is_none_or(|maximum| actual > maximum)
                    }) {
                        return Err("actual usage exceeds authorized reservation; retain hold for reconciliation".into());
                    }
                    let charge = hold.price.quote(usage)?;
                    if charge > hold.reserved {
                        return Err("charge exceeds reservation".into());
                    }
                    hold.phase = Phase::Settled;
                    hold.retail_charge = Some(charge);
                    hold.provider_cost = *provider_cost;
                    hold.hosting_cost = *hosting_cost;
                    hold.receipt = Some(receipt.clone());
                }
                Operation::Unknown { attempt } => pending(account, attempt)?.phase = Phase::Unknown,
                Operation::Release { attempt } => {
                    pending(account, attempt)?.phase = Phase::Released
                }
                Operation::Refund { attempt, amount } => {
                    let hold = account.holds.get_mut(attempt).ok_or("unknown attempt")?;
                    let refunded = hold
                        .refunded
                        .checked_add(*amount)
                        .ok_or("refund overflow")?;
                    if *amount == 0
                        || hold.phase != Phase::Settled
                        || refunded > hold.retail_charge.ok_or("unsettled charge")?
                    {
                        return Err("refund exceeds a settled retail charge".into());
                    }
                    hold.refunded = refunded;
                }
                Operation::ReverseRefund { attempt, amount } => {
                    let hold = account.holds.get_mut(attempt).ok_or("unknown attempt")?;
                    if *amount == 0 || hold.phase != Phase::Settled {
                        return Err("refund reversal requires a settled charge".into());
                    }
                    hold.refunded = hold
                        .refunded
                        .checked_sub(*amount)
                        .ok_or("reversal exceeds refund")?;
                }
                Operation::Create { .. } => unreachable!(),
            }
            account.balance()?;
        }
        self.sources.insert(source, mutation.clone());
        Ok(true)
    }
}

fn pending<'a>(account: &'a mut Account, attempt: &str) -> Result<&'a mut Hold, String> {
    let hold = account.holds.get_mut(attempt).ok_or("unknown attempt")?;
    if !matches!(hold.phase, Phase::Held | Phase::Unknown) {
        return Err("attempt is terminal".into());
    }
    Ok(hold)
}

fn identity(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err("accounting references must be nonempty bounded identifiers".into());
    }
    Ok(())
}

fn currency(value: &str) -> Result<(), String> {
    if value.len() != 3 || !value.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err("currency must be an explicit three-letter uppercase code".into());
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    schema: String,
    previous: String,
    mutation: Mutation,
    digest: String,
}

impl Entry {
    fn computed(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(&self.schema, &self.previous, &self.mutation))
            .map_err(|e| e.to_string())?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

/// One protected local ledger, locked for its entire lifetime. Each accepted
/// mutation is appended and synced before the caller may dispatch work.
/// A damaged or partially written log refuses to open; recovery never discards
/// a tail or releases unresolved funds. This is not a multi-host database.
pub struct Ledger {
    file: File,
    state: State,
    head: String,
    bytes: u64,
    poisoned: bool,
}

impl Ledger {
    pub fn open(path: &Path) -> Result<Self, String> {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("money ledger must not be a symlink".into());
        }
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("money ledger requires a private regular file".into());
        }
        file.try_lock()
            .map_err(|e| format!("money ledger is locked: {e}"))?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_LOG + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_LOG || (!bytes.is_empty() && bytes.last() != Some(&b'\n')) {
            return Err("money ledger is oversized or has an incomplete tail".into());
        }
        let mut ledger = Self {
            file,
            state: State::default(),
            head: String::new(),
            bytes: bytes.len() as u64,
            poisoned: false,
        };
        for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let entry: Entry = serde_json::from_slice(line).map_err(|e| e.to_string())?;
            if entry.schema != SCHEMA
                || entry.previous != ledger.head
                || entry.computed()? != entry.digest
            {
                return Err("money ledger chain or schema is invalid".into());
            }
            if !ledger.state.apply(&entry.mutation)? {
                return Err("duplicate mutation in money ledger".into());
            }
            ledger.head = entry.digest;
        }
        // Any pending attempt after a writer restart may have been dispatched.
        // Retain its full liability; reconciliation must name the existing hold.
        for account in ledger.state.accounts.values_mut() {
            for hold in account
                .holds
                .values_mut()
                .filter(|h| h.phase == Phase::Held)
            {
                hold.phase = Phase::Unknown;
            }
        }
        ledger.file.sync_all().map_err(|e| e.to_string())?;
        File::open(path.parent().ok_or("ledger needs a parent directory")?)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(ledger)
    }

    /// Returns false for an exact idempotent replay. A failed write poisons this
    /// instance so no caller can keep spending against uncertain durable state.
    pub fn apply(&mut self, mutation: Mutation) -> Result<bool, String> {
        if self.poisoned {
            return Err("money ledger requires recovery after a write failure".into());
        }
        let mut next = self.state.clone();
        if !next.apply(&mutation)? {
            return Ok(false);
        }
        let mut entry = Entry {
            schema: SCHEMA.into(),
            previous: self.head.clone(),
            mutation,
            digest: String::new(),
        };
        entry.digest = entry.computed()?;
        let mut bytes = serde_json::to_vec(&entry).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        let length = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or("ledger size overflow")?;
        if length > MAX_LOG {
            return Err("money ledger reached its storage bound".into());
        }
        if let Err(error) = self
            .file
            .write_all(&bytes)
            .and_then(|()| self.file.sync_all())
        {
            self.poisoned = true;
            return Err(error.to_string());
        }
        self.state = next;
        self.head = entry.digest;
        self.bytes = length;
        Ok(true)
    }

    pub fn balance(&self, workspace: &str) -> Result<Balance, String> {
        self.state
            .accounts
            .get(workspace)
            .ok_or("workspace account is missing")?
            .balance()
    }

    #[must_use]
    pub fn hold(&self, workspace: &str, attempt: &str) -> Option<&Hold> {
        self.state.accounts.get(workspace)?.holds.get(attempt)
    }

    /// Every workspace with an account, sorted — the set an account API
    /// enumerates. Read-only; opening the ledger is still the writer's
    /// exclusive lock, so a reader takes its own open of the file.
    #[must_use]
    pub fn workspaces(&self) -> Vec<&str> {
        self.state.accounts.keys().map(String::as_str).collect()
    }

    /// One workspace's holds, ordered by attempt — the open and terminal
    /// positions an account view lists. The hold carries its phase, so a
    /// reader tells an outstanding liability from settled history.
    #[must_use]
    pub fn holds(&self, workspace: &str) -> Vec<(&str, &Hold)> {
        self.state
            .accounts
            .get(workspace)
            .map(|account| {
                account
                    .holds
                    .iter()
                    .map(|(attempt, hold)| (attempt.as_str(), hold))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price() -> Price {
        Price {
            version: "synthetic-v1".into(),
            currency: "USD".into(),
            model: "fixture".into(),
            capacity: "shared".into(),
            policy: "every-attempt-v1".into(),
            rates: [(
                Resource::InputTokens,
                Rate {
                    millionths: 3,
                    per_units: 2,
                },
            )]
            .into(),
        }
    }

    fn usage(units: u64) -> Usage {
        [(Resource::InputTokens, units)].into()
    }

    fn mutation(source: &str, operation: Operation) -> Mutation {
        Mutation {
            workspace: "workspace-a".into(),
            source: source.into(),
            audit: format!("audit:{source}"),
            operation,
        }
    }

    fn funded(limit: u64) -> (tempfile::TempDir, Ledger) {
        let root = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
        ledger
            .apply(mutation(
                "create",
                Operation::Create {
                    currency: "USD".into(),
                    spend_limit: limit,
                    topups_allowed: false,
                },
            ))
            .unwrap();
        ledger
            .apply(mutation(
                "grant",
                Operation::Credit {
                    amount: 100,
                    credit_kind: CreditKind::Grant,
                },
            ))
            .unwrap();
        (root, ledger)
    }

    fn reserve(attempt: &str, units: u64) -> Operation {
        Operation::Reserve {
            attempt: attempt.into(),
            request_digest: format!("digest:{attempt}"),
            price: price(),
            maximum_usage: usage(units),
        }
    }

    fn settle(attempt: &str, units: u64) -> Operation {
        Operation::Settle {
            attempt: attempt.into(),
            usage: usage(units),
            receipt: format!("receipt:{attempt}"),
            provider_cost: None,
            hosting_cost: Some(9),
        }
    }

    #[test]
    fn rational_prices_round_up_and_missing_usage_is_not_zero() {
        assert_eq!(price().quote(&usage(1)).unwrap(), 2);
        assert_eq!(price().quote(&usage(3)).unwrap(), 5);
        assert!(price().quote(&Usage::new()).is_err());
        let mut p = price();
        p.rates.get_mut(&Resource::InputTokens).unwrap().per_units = 0;
        assert!(p.quote(&usage(1)).is_err());
        p.rates.insert(
            Resource::InputTokens,
            Rate {
                millionths: u64::MAX,
                per_units: 1,
            },
        );
        assert!(p.quote(&usage(u64::MAX)).is_err());
    }

    #[test]
    fn holds_settlement_and_refund_reversal_reconcile_exactly() {
        let (_root, mut ledger) = funded(100);
        ledger
            .apply(mutation("reserve", reserve("one", 20)))
            .unwrap();
        let balance = ledger.balance("workspace-a").unwrap();
        assert_eq!((balance.reserved, balance.available), (30, 70));
        ledger.apply(mutation("settle", settle("one", 10))).unwrap();
        let balance = ledger.balance("workspace-a").unwrap();
        assert_eq!(
            (balance.reserved, balance.settled, balance.available),
            (0, 15, 85)
        );
        let hold = ledger.hold("workspace-a", "one").unwrap();
        assert_eq!((hold.provider_cost, hold.hosting_cost), (None, Some(9)));
        ledger
            .apply(mutation(
                "refund",
                Operation::Refund {
                    attempt: "one".into(),
                    amount: 10,
                },
            ))
            .unwrap();
        ledger
            .apply(mutation(
                "reverse",
                Operation::ReverseRefund {
                    attempt: "one".into(),
                    amount: 4,
                },
            ))
            .unwrap();
        let balance = ledger.balance("workspace-a").unwrap();
        assert_eq!((balance.refunded, balance.available), (6, 91));
        assert!(
            ledger
                .apply(mutation(
                    "excess-refund",
                    Operation::Refund {
                        attempt: "one".into(),
                        amount: 10
                    }
                ))
                .is_err()
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().available, 91);
    }

    #[test]
    fn duplicate_mutations_are_stable_and_changed_content_refuses() {
        let (_root, mut ledger) = funded(100);
        let request = mutation("reserve", reserve("one", 20));
        assert!(ledger.apply(request.clone()).unwrap());
        assert!(!ledger.apply(request).unwrap());
        assert!(
            ledger
                .apply(mutation("reserve", reserve("one", 21)))
                .is_err()
        );
        assert!(
            ledger
                .apply(mutation("another-key", reserve("one", 20)))
                .is_err()
        );
        let resolved = mutation("settle", settle("one", 10));
        assert!(ledger.apply(resolved.clone()).unwrap());
        assert!(!ledger.apply(resolved).unwrap());
        assert!(
            ledger
                .apply(mutation("other-settlement", settle("one", 10)))
                .is_err()
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().settled, 15);
    }

    #[test]
    fn hard_limits_include_all_holds_and_cannot_be_reset_by_credits() {
        let (_root, mut ledger) = funded(45);
        ledger.apply(mutation("first", reserve("one", 20))).unwrap();
        assert!(
            ledger
                .apply(mutation("second", reserve("two", 20)))
                .is_err()
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().reserved, 30);
        assert!(ledger.hold("workspace-a", "two").is_none());
        assert!(
            ledger
                .apply(mutation(
                    "topup",
                    Operation::Credit {
                        amount: 100,
                        credit_kind: CreditKind::TopUp
                    }
                ))
                .is_err()
        );
        ledger
            .apply(mutation(
                "another-grant",
                Operation::Credit {
                    amount: 100,
                    credit_kind: CreditKind::Grant,
                },
            ))
            .unwrap();
        assert!(
            ledger
                .apply(mutation("second", reserve("two", 20)))
                .is_err()
        );
        assert!(
            ledger
                .apply(mutation("debit", Operation::Debit { amount: 180 }))
                .is_err()
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().credited, 200);
    }

    #[test]
    fn restart_preserves_unknown_liability_and_excludes_other_writers() {
        let (root, mut ledger) = funded(100);
        let path = root.path().join("money.jsonl");
        ledger
            .apply(mutation("reserve", reserve("one", 20)))
            .unwrap();
        assert!(Ledger::open(&path).is_err());
        drop(ledger);
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(
            ledger.hold("workspace-a", "one").unwrap().phase,
            Phase::Unknown
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().reserved, 30);
        assert!(
            ledger
                .apply(mutation("too-much", settle("one", 21)))
                .is_err()
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().reserved, 30);
        ledger
            .apply(mutation("reconcile", settle("one", 12)))
            .unwrap();
        drop(ledger);
        let ledger = Ledger::open(&path).unwrap();
        assert_eq!(
            ledger.hold("workspace-a", "one").unwrap().phase,
            Phase::Settled
        );
        assert_eq!(ledger.balance("workspace-a").unwrap().settled, 18);
    }

    #[test]
    fn release_and_price_identity_have_explicit_guards() {
        let (_root, mut ledger) = funded(100);
        ledger
            .apply(mutation("reserve", reserve("one", 20)))
            .unwrap();
        ledger
            .apply(mutation(
                "unknown",
                Operation::Unknown {
                    attempt: "one".into(),
                },
            ))
            .unwrap();
        assert_eq!(ledger.balance("workspace-a").unwrap().available, 70);
        ledger
            .apply(mutation(
                "release",
                Operation::Release {
                    attempt: "one".into(),
                },
            ))
            .unwrap();
        assert_eq!(ledger.balance("workspace-a").unwrap().available, 100);
        assert!(ledger.apply(mutation("settle", settle("one", 1))).is_err());
        let mut op = reserve("two", 10);
        if let Operation::Reserve { price, .. } = &mut op {
            price.model = "changed".into();
        }
        assert!(ledger.apply(mutation("changed-price", op)).is_err());
        let mut op = reserve("two", 10);
        if let Operation::Reserve { price, .. } = &mut op {
            price.currency = "EUR".into();
        }
        assert!(ledger.apply(mutation("changed-currency", op)).is_err());
    }

    #[test]
    fn workspace_sources_are_isolated_and_partial_or_tampered_logs_refuse() {
        let (root, mut ledger) = funded(100);
        let mut other = mutation(
            "create",
            Operation::Create {
                currency: "USD".into(),
                spend_limit: 100,
                topups_allowed: true,
            },
        );
        other.workspace = "workspace-b".into();
        ledger.apply(other).unwrap();
        assert_eq!(ledger.balance("workspace-b").unwrap().available, 0);
        let path = root.path().join("money.jsonl");
        drop(ledger);
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed.extend_from_slice(b"{unfinished");
        std::fs::write(&path, changed).unwrap();
        assert!(Ledger::open(&path).is_err());
        std::fs::write(
            &path,
            String::from_utf8(original)
                .unwrap()
                .replacen("workspace-a", "workspace-z", 1),
        )
        .unwrap();
        assert!(Ledger::open(&path).is_err());
    }

    #[test]
    fn the_balance_names_its_price_versions_and_readers_enumerate_accounts() {
        let (_root, mut ledger) = funded(100);
        ledger
            .apply(mutation("reserve", reserve("one", 20)))
            .unwrap();
        let balance = ledger.balance("workspace-a").unwrap();
        assert_eq!(balance.price_versions, ["synthetic-v1".to_string()]);

        // A second price version transacted under is a second version the
        // account API must serve, not a relabeling of the first.
        let mut second = reserve("two", 10);
        if let Operation::Reserve { price, .. } = &mut second {
            price.version = "synthetic-v2".into();
        }
        ledger.apply(mutation("reserve-two", second)).unwrap();
        assert_eq!(
            ledger.balance("workspace-a").unwrap().price_versions,
            ["synthetic-v1".to_string(), "synthetic-v2".to_string()]
        );

        assert_eq!(ledger.workspaces(), ["workspace-a"]);
        let holds = ledger.holds("workspace-a");
        assert_eq!(
            holds
                .iter()
                .map(|(attempt, _)| *attempt)
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert!(ledger.holds("workspace-none").is_empty());
    }
}
