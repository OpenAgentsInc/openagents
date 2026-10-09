//! The credit ledger: one row per account we hold (balance, currency,
//! expiry, cost basis), burned down by each attempt's cost, with the
//! burn-down numbers and owner alerts of `docs/inference/gateway.md`,
//! section 5 ("Credit-aware routing").

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::Attempt;
use super::store::{DAY_MS, date};

/// How an account is paid for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// A prepaid balance that runs down and may expire.
    #[default]
    Prepaid,
    /// Capacity that costs us nothing (the Pro door).
    FreeCapacity,
    /// Billed in arrears; no balance.
    PayAsYouGo,
}

/// One account we hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditAccount {
    /// Our name for it (`google-vertex`, `zai`).
    pub id: String,
    /// The upstream it pays for.
    pub upstream: String,
    #[serde(default = "usd")]
    pub currency: String,
    /// What the credit started at, micros; the base for the percentage
    /// alerts.
    pub granted: u64,
    /// What is left, micros.
    pub balance: u64,
    /// When unspent credit expires, Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    #[serde(default)]
    pub basis: Basis,
}

fn usd() -> String {
    "USD".into()
}

/// The percentages left at which the owner hears about an account.
pub const REMAINING_ALERTS: [u8; 3] = [50, 25, 10];
/// Projected runway under this many days alerts.
pub const RUNWAY_DAYS: f64 = 30.0;
/// Days of spend kept per account.
const KEEP_DAYS: u64 = 60;

/// One account's spend on one UTC day, micros.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DaySpend {
    pub attempts: u64,
    /// Our cost from the rate table.
    pub cost: u64,
    /// What upstreams that report their own cost said, and over how many
    /// attempts.
    pub reported: u64,
    pub reported_attempts: u64,
    /// Our cost over those same attempts, to compare like with like.
    pub cost_of_reported: u64,
}

/// An account's burn-down at one moment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BurnDown {
    pub account: String,
    pub upstream: String,
    pub currency: String,
    pub basis: Basis,
    pub granted: u64,
    pub balance: u64,
    /// Percent of `granted` left.
    pub remaining_pct: f64,
    pub spent_today: u64,
    /// Average daily spend over the last seven days (fewer when the
    /// account is newer).
    pub average_daily: u64,
    /// `balance / average_daily`, when anything is being spent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_left: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_to_expiry: Option<f64>,
    /// What would be left at expiry at the average rate, when positive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unspent_at_expiry: Option<u64>,
    /// Attempts billed to this account whose cost was in another
    /// currency, so they could not be burned down.
    pub unconverted_attempts: u64,
}

/// Something the owner should hear about.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Alert {
    /// The account crossed `threshold_pct` remaining.
    Remaining {
        account: String,
        threshold_pct: u8,
        remaining_pct: f64,
        balance: u64,
        currency: String,
    },
    /// Projected runway is under 30 days.
    Runway { account: String, days_left: f64 },
    /// At the current rate the credit expires with money left.
    ExpiringUnspent {
        account: String,
        days_to_expiry: f64,
        unspent_at_expiry: u64,
        currency: String,
    },
    /// Our cost for a day and the provider's billing differ by more than
    /// the allowed gap.
    ReconcileGap {
        account: String,
        day: String,
        source: String,
        ours: u64,
        theirs: u64,
        gap_pct: f64,
    },
}

impl Alert {
    /// The key an alert fires once under until it clears.
    fn key(&self) -> String {
        match self {
            Alert::Remaining {
                account,
                threshold_pct,
                ..
            } => format!("{account}/remaining/{threshold_pct}"),
            Alert::Runway { account, .. } => format!("{account}/runway"),
            Alert::ExpiringUnspent { account, .. } => format!("{account}/expiring"),
            Alert::ReconcileGap {
                account,
                day,
                source,
                ..
            } => format!("{account}/reconcile/{source}/{day}"),
        }
    }

    pub fn account(&self) -> &str {
        match self {
            Alert::Remaining { account, .. }
            | Alert::Runway { account, .. }
            | Alert::ExpiringUnspent { account, .. }
            | Alert::ReconcileGap { account, .. } => account,
        }
    }
}

#[derive(Debug)]
struct Row {
    account: CreditAccount,
    days: BTreeMap<u64, DaySpend>,
    first_day: Option<u64>,
    unconverted: u64,
}

/// Every account, its daily spend, and which alerts have fired.
#[derive(Debug, Default)]
pub struct Ledger {
    rows: BTreeMap<String, Row>,
    fired: BTreeSet<String>,
    /// Reconciliation gaps raised, kept so the status shows them.
    gaps: Vec<Alert>,
}

impl Ledger {
    pub fn new(accounts: impl IntoIterator<Item = CreditAccount>) -> Self {
        let mut ledger = Self::default();
        for account in accounts {
            ledger.open(account);
        }
        ledger
    }

    /// Add an account, or replace its terms and keep its history.
    pub fn open(&mut self, account: CreditAccount) {
        match self.rows.get_mut(&account.id) {
            Some(row) => row.account = account,
            None => {
                self.rows.insert(
                    account.id.clone(),
                    Row {
                        account,
                        days: BTreeMap::new(),
                        first_day: None,
                        unconverted: 0,
                    },
                );
            }
        }
    }

    pub fn account(&self, id: &str) -> Option<&CreditAccount> {
        self.rows.get(id).map(|row| &row.account)
    }

    pub fn accounts(&self) -> impl Iterator<Item = &CreditAccount> {
        self.rows.values().map(|row| &row.account)
    }

    /// Burn the attempt's cost down from its account. Returns false when
    /// the attempt names no account we hold.
    pub fn debit(&mut self, attempt: &Attempt) -> bool {
        let Some(row) = attempt
            .account
            .as_ref()
            .and_then(|id| self.rows.get_mut(id))
        else {
            return false;
        };
        let Some(cost) = attempt.cost else {
            return true;
        };
        if attempt.currency != row.account.currency {
            row.unconverted += 1;
            return true;
        }
        let day = attempt.at_ms / DAY_MS;
        row.first_day = Some(row.first_day.map_or(day, |first| first.min(day)));
        let spend = row.days.entry(day).or_default();
        spend.attempts += 1;
        spend.cost = spend.cost.saturating_add(cost);
        if let Some(reported) = attempt.reported_cost {
            spend.reported = spend.reported.saturating_add(reported);
            spend.reported_attempts += 1;
            spend.cost_of_reported = spend.cost_of_reported.saturating_add(cost);
        }
        if row.account.basis == Basis::Prepaid {
            row.account.balance = row.account.balance.saturating_sub(cost);
        }
        while row
            .days
            .first_key_value()
            .is_some_and(|(first, _)| first + KEEP_DAYS <= day)
        {
            row.days.pop_first();
        }
        true
    }

    /// Set a balance from the provider's own figure.
    pub fn set_balance(&mut self, id: &str, balance: u64) {
        if let Some(row) = self.rows.get_mut(id) {
            row.account.balance = balance;
        }
    }

    /// One account's spend on one day.
    pub fn day(&self, id: &str, day: u64) -> DaySpend {
        self.rows
            .get(id)
            .and_then(|row| row.days.get(&day))
            .copied()
            .unwrap_or_default()
    }

    pub fn burn_down(&self, id: &str, now_ms: u64) -> Option<BurnDown> {
        let row = self.rows.get(id)?;
        let account = &row.account;
        let today = now_ms / DAY_MS;
        let spent_today = row.days.get(&today).map_or(0, |d| d.cost);
        let window_start = today.saturating_sub(6);
        let observed = row
            .first_day
            .map_or(1, |first| today.saturating_sub(first.max(window_start)) + 1)
            .clamp(1, 7);
        let week: u64 = row
            .days
            .range(window_start..=today)
            .map(|(_, d)| d.cost)
            .sum();
        let average_daily = week / observed;
        let prepaid = account.basis == Basis::Prepaid;
        let days_left =
            (prepaid && average_daily > 0).then(|| account.balance as f64 / average_daily as f64);
        let days_to_expiry = account
            .expires_at_ms
            .map(|at| at.saturating_sub(now_ms) as f64 / DAY_MS as f64);
        let unspent_at_expiry = days_to_expiry.filter(|_| prepaid).and_then(|days| {
            let left = account.balance as f64 - average_daily as f64 * days;
            (left >= 1.0).then_some(left as u64)
        });
        Some(BurnDown {
            account: account.id.clone(),
            upstream: account.upstream.clone(),
            currency: account.currency.clone(),
            basis: account.basis,
            granted: account.granted,
            balance: account.balance,
            remaining_pct: if account.granted == 0 {
                0.0
            } else {
                account.balance as f64 * 100.0 / account.granted as f64
            },
            spent_today,
            average_daily,
            days_left,
            days_to_expiry,
            unspent_at_expiry,
            unconverted_attempts: row.unconverted,
        })
    }

    pub fn burn_downs(&self, now_ms: u64) -> Vec<BurnDown> {
        self.rows
            .keys()
            .filter_map(|id| self.burn_down(id, now_ms))
            .collect()
    }

    /// The conditions that hold now: thresholds crossed, short runway,
    /// credit on course to expire unspent, and reconciliation gaps raised.
    pub fn active(&self, now_ms: u64) -> Vec<Alert> {
        let mut alerts = Vec::new();
        for burn in self.burn_downs(now_ms) {
            if burn.basis != Basis::Prepaid || burn.granted == 0 {
                continue;
            }
            if let Some(threshold) = REMAINING_ALERTS
                .iter()
                .rev()
                .find(|t| burn.remaining_pct <= f64::from(**t))
            {
                // Every threshold at or above the crossed one holds; list
                // each so each fires once.
                for t in REMAINING_ALERTS.iter().filter(|t| *t >= threshold) {
                    alerts.push(Alert::Remaining {
                        account: burn.account.clone(),
                        threshold_pct: *t,
                        remaining_pct: burn.remaining_pct,
                        balance: burn.balance,
                        currency: burn.currency.clone(),
                    });
                }
            }
            if let Some(days_left) = burn.days_left.filter(|d| *d < RUNWAY_DAYS) {
                alerts.push(Alert::Runway {
                    account: burn.account.clone(),
                    days_left,
                });
            }
            if let (Some(days_to_expiry), Some(unspent)) =
                (burn.days_to_expiry, burn.unspent_at_expiry)
            {
                alerts.push(Alert::ExpiringUnspent {
                    account: burn.account.clone(),
                    days_to_expiry,
                    unspent_at_expiry: unspent,
                    currency: burn.currency.clone(),
                });
            }
        }
        alerts.extend(self.gaps.iter().cloned());
        alerts
    }

    /// Alerts that hold now and have not fired since they last cleared.
    /// A cleared condition (a top-up, a longer runway) re-arms.
    pub fn check(&mut self, now_ms: u64) -> Vec<Alert> {
        let active = self.active(now_ms);
        let keys: BTreeSet<String> = active.iter().map(Alert::key).collect();
        self.fired.retain(|key| keys.contains(key));
        active
            .into_iter()
            .filter(|alert| self.fired.insert(alert.key()))
            .collect()
    }

    /// Record a reconciliation gap so `check` fires it once and the
    /// status shows it. Gaps older than `KEEP_DAYS` are forgotten.
    pub fn raise_gap(&mut self, gap: Alert, today: u64) {
        let oldest = date(today.saturating_sub(KEEP_DAYS));
        self.gaps.retain(|g| match g {
            Alert::ReconcileGap { day, .. } => day.as_str() >= oldest.as_str(),
            _ => false,
        });
        if !self.gaps.iter().any(|g| g.key() == gap.key()) {
            self.gaps.push(gap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 100 * DAY_MS + 12 * 3_600_000;

    fn vertex(granted: u64, expires_in_days: Option<u64>) -> CreditAccount {
        CreditAccount {
            id: "google-vertex".into(),
            upstream: "vertex".into(),
            currency: "USD".into(),
            granted,
            balance: granted,
            expires_at_ms: expires_in_days.map(|d| NOW + d * DAY_MS),
            basis: Basis::Prepaid,
        }
    }

    fn spend(at_ms: u64, cost: u64) -> Attempt {
        Attempt {
            account: Some("google-vertex".into()),
            cost: Some(cost),
            currency: "USD".into(),
            ..Attempt::new("r", 1, "vertex", "gemini", at_ms)
        }
    }

    #[test]
    fn burns_down_and_projects() {
        let mut ledger = Ledger::new([vertex(1_000_000, Some(50))]);
        for day in 0..7 {
            assert!(ledger.debit(&spend(NOW - day * DAY_MS, 10_000)));
        }
        let burn = ledger.burn_down("google-vertex", NOW).unwrap();
        assert_eq!(burn.balance, 930_000);
        assert_eq!(burn.spent_today, 10_000);
        assert_eq!(burn.average_daily, 10_000);
        assert_eq!(burn.days_left, Some(93.0));
        assert_eq!(burn.days_to_expiry, Some(50.0));
        assert_eq!(burn.unspent_at_expiry, Some(430_000));
    }

    #[test]
    fn a_new_account_averages_over_the_days_it_has() {
        let mut ledger = Ledger::new([vertex(1_000_000, None)]);
        ledger.debit(&spend(NOW - DAY_MS, 30_000));
        ledger.debit(&spend(NOW, 10_000));
        assert_eq!(
            ledger
                .burn_down("google-vertex", NOW)
                .unwrap()
                .average_daily,
            20_000
        );
    }

    #[test]
    fn thresholds_fire_once_and_rearm_on_top_up() {
        let mut ledger = Ledger::new([vertex(1_000_000, None)]);
        ledger.debit(&spend(NOW, 400_000));
        // 600k left at 400k a day: runway, but no threshold yet.
        let first = ledger.check(NOW);
        assert!(first.iter().all(|a| !matches!(a, Alert::Remaining { .. })));
        assert!(first.iter().any(|a| matches!(a, Alert::Runway { .. })));
        ledger.debit(&spend(NOW, 100_000)); // 50% left
        let fired = ledger.check(NOW);
        assert!(fired.iter().any(|a| matches!(
            a,
            Alert::Remaining {
                threshold_pct: 50,
                ..
            }
        )));
        assert!(!fired.iter().any(|a| matches!(a, Alert::Runway { .. })));
        assert!(ledger.check(NOW).is_empty(), "nothing fires twice");
        ledger.debit(&spend(NOW, 420_000)); // 8% left
        let pcts: Vec<u8> = ledger
            .check(NOW)
            .iter()
            .filter_map(|a| match a {
                Alert::Remaining { threshold_pct, .. } => Some(*threshold_pct),
                _ => None,
            })
            .collect();
        assert_eq!(pcts, [25, 10]);
        ledger.set_balance("google-vertex", 1_000_000);
        let after = ledger.check(NOW);
        assert!(after.iter().all(|a| !matches!(a, Alert::Remaining { .. })));
        ledger.set_balance("google-vertex", 50_000);
        assert_eq!(
            ledger
                .check(NOW)
                .iter()
                .filter(|a| matches!(a, Alert::Remaining { .. }))
                .count(),
            3
        );
    }

    #[test]
    fn credit_on_course_to_expire_unspent_alerts() {
        let mut ledger = Ledger::new([vertex(30_000_000_000, Some(90))]);
        ledger.debit(&spend(NOW, 1_000_000)); // $1 a day against $30,000
        let fired = ledger.check(NOW);
        assert!(fired.iter().any(|a| matches!(
            a,
            Alert::ExpiringUnspent { unspent_at_expiry, .. } if *unspent_at_expiry > 29_000_000_000
        )));
        assert!(!fired.iter().any(|a| matches!(a, Alert::Runway { .. })));
    }

    #[test]
    fn other_currencies_and_bases_do_not_burn_down() {
        let mut free = vertex(0, None);
        free.id = "pro".into();
        free.basis = Basis::FreeCapacity;
        let mut ledger = Ledger::new([vertex(1_000, None), free]);
        let mut cny = spend(NOW, 500);
        cny.currency = "CNY".into();
        ledger.debit(&cny);
        let mut pro = spend(NOW, 500);
        pro.account = Some("pro".into());
        ledger.debit(&pro);
        let burn = ledger.burn_down("google-vertex", NOW).unwrap();
        assert_eq!(burn.balance, 1_000);
        assert_eq!(burn.unconverted_attempts, 1);
        assert_eq!(ledger.burn_down("pro", NOW).unwrap().spent_today, 500);
        assert!(ledger.check(NOW).is_empty());
        assert!(!ledger.debit(&Attempt::new("r", 1, "x", "y", NOW)));
    }
}
