//! Measurement and the credit ledger (`docs/inference/gateway.md`,
//! sections 5 and 6).
//!
//! Adapters and the router report each upstream attempt as an
//! [`Attempt`] through a [`Recorder`]. The [`Meter`] is the recorder the
//! gateway mounts: it prices each attempt from the [`RateCard`], keeps it
//! in a bounded [`store::Store`] (and day files when configured), burns
//! its cost down from the billed account in the [`Ledger`], and raises
//! [`Alert`]s through an [`AlertSink`] (a log line by default). Its
//! [`Meter::status`] is what the gateway's admin status endpoint serves:
//! per-account burn-downs, the alerts that hold now, and live rates per
//! upstream and model over 5 minutes, 1 hour, and 24 hours.

mod attempt;
pub mod card;
pub mod ledger;
pub mod live;
pub mod reconcile;
pub mod store;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub use attempt::{Api, Attempt, Collect, ErrorClass, NoRecorder, Outcome, Recorder, Tokens};
pub use card::{Promotion, RateCard, RateRow};
pub use ledger::{Alert, Basis, BurnDown, CreditAccount, Ledger};
pub use live::Rate;
pub use reconcile::{Billed, FakeBilling, ProviderBilling, Reconciled};

use store::{DAY_MS, Journal, Store, date};

/// The schema tag on [`Status`].
pub const STATUS_SCHEMA: &str = "openagents.inference.status.v1";

/// How a meter is set up. Deserializes from the gateway's config.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub rates: Vec<RateRow>,
    #[serde(default)]
    pub accounts: Vec<CreditAccount>,
    /// Most attempts kept in memory (default 200,000).
    #[serde(default)]
    pub max_records: Option<usize>,
    /// How long attempts stay in memory, ms (default 25 hours, so the
    /// 24-hour rates are whole).
    #[serde(default)]
    pub retain_ms: Option<u64>,
    /// A directory for day files of attempt records; none keeps them in
    /// memory only.
    #[serde(default)]
    pub journal: Option<PathBuf>,
    /// Days of day files kept (default 30).
    #[serde(default)]
    pub keep_days: Option<u32>,
}

/// Where alerts go.
pub trait AlertSink: Send + Sync {
    fn alert(&self, alert: &Alert);
}

/// Writes each alert as one log line on standard error.
#[derive(Clone, Copy, Debug, Default)]
pub struct LogAlerts;

impl AlertSink for LogAlerts {
    fn alert(&self, alert: &Alert) {
        let line = serde_json::to_string(alert).unwrap_or_default();
        eprintln!("inference alert: {line}");
    }
}

/// Keeps alerts in memory, for tests.
#[derive(Debug, Default)]
pub struct CollectAlerts(pub Mutex<Vec<Alert>>);

impl CollectAlerts {
    pub fn taken(&self) -> Vec<Alert> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

impl AlertSink for CollectAlerts {
    fn alert(&self, alert: &Alert) {
        if let Ok(mut v) = self.0.lock() {
            v.push(alert.clone());
        }
    }
}

impl<S: AlertSink + ?Sized> AlertSink for std::sync::Arc<S> {
    fn alert(&self, alert: &Alert) {
        (**self).alert(alert);
    }
}

#[derive(Debug)]
struct Inner {
    card: RateCard,
    store: Store,
    ledger: Ledger,
    journal: Option<Journal>,
    unpriced: u64,
    journal_errors: u64,
    reconciled_day: Option<u64>,
    reconciliations: Vec<Reconciled>,
}

/// The gateway's recorder: prices, keeps, burns down, and alerts.
pub struct Meter {
    inner: Mutex<Inner>,
    sink: Box<dyn AlertSink>,
}

/// The admin view: burn-downs, alerts, live rates.
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub v: &'static str,
    pub at_ms: u64,
    pub accounts: Vec<BurnDown>,
    pub alerts: Vec<Alert>,
    /// Keyed by window: `5m`, `1h`, `24h`.
    pub rates: BTreeMap<&'static str, Vec<Rate>>,
    pub records: Records,
    /// The last day reconciled against provider billing, and how it went.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciled_day: Option<String>,
    pub reconciliations: Vec<Reconciled>,
}

/// How the attempt store stands.
#[derive(Clone, Debug, Serialize)]
pub struct Records {
    pub kept: usize,
    pub max: usize,
    pub retain_ms: u64,
    pub dropped: u64,
    /// Attempts with no rate row, so no cost.
    pub unpriced: u64,
    pub journal_errors: u64,
}

impl Meter {
    /// A meter that logs alerts.
    pub fn new(config: &Config) -> Self {
        Self::with_sink(config, Box::new(LogAlerts))
    }

    pub fn with_sink(config: &Config, sink: Box<dyn AlertSink>) -> Self {
        Self {
            inner: Mutex::new(Inner {
                card: RateCard::new(config.rates.iter().cloned()),
                store: Store::new(
                    config.max_records.unwrap_or(200_000),
                    config.retain_ms.unwrap_or(25 * 3_600_000),
                ),
                ledger: Ledger::new(config.accounts.iter().cloned()),
                journal: config
                    .journal
                    .as_ref()
                    .map(|dir| Journal::new(dir, config.keep_days.unwrap_or(30))),
                unpriced: 0,
                journal_errors: 0,
                reconciled_day: None,
                reconciliations: Vec::new(),
            }),
            sink,
        }
    }

    fn deliver(&self, alerts: Vec<Alert>) {
        for alert in &alerts {
            self.sink.alert(alert);
        }
    }

    /// Fire the alerts that hold at `now` and have not fired yet (credit
    /// nearing expiry fires even with no new attempts).
    pub fn check(&self, now_ms: u64) {
        let alerts = match self.inner.lock() {
            Ok(mut inner) => inner.ledger.check(now_ms),
            Err(_) => return,
        };
        self.deliver(alerts);
    }

    /// The admin view at `now`.
    pub fn status(&self, now_ms: u64) -> Option<Status> {
        let mut inner = self.inner.lock().ok()?;
        inner.store.prune(now_ms);
        let rates = live::WINDOWS
            .iter()
            .map(|(name, span)| {
                (
                    *name,
                    live::rates(inner.store.since(now_ms.saturating_sub(*span))),
                )
            })
            .collect();
        Some(Status {
            v: STATUS_SCHEMA,
            at_ms: now_ms,
            accounts: inner.ledger.burn_downs(now_ms),
            alerts: inner.ledger.active(now_ms),
            rates,
            records: Records {
                kept: inner.store.len(),
                max: inner.store.max(),
                retain_ms: inner.store.retain_ms(),
                dropped: inner.store.dropped,
                unpriced: inner.unpriced,
                journal_errors: inner.journal_errors,
            },
            reconciled_day: inner.reconciled_day.map(date),
            reconciliations: inner.reconciliations.clone(),
        })
    }

    /// Live rates for one window ending at `now`, for the router.
    pub fn rates(&self, span_ms: u64, now_ms: u64) -> Vec<Rate> {
        self.inner
            .lock()
            .map(|inner| live::rates(inner.store.since(now_ms.saturating_sub(span_ms))))
            .unwrap_or_default()
    }

    /// Adds rate rows the adapters advertise, keeping any row the config
    /// already holds for the same upstream and model (the config wins).
    pub fn add_rates(&self, rows: impl IntoIterator<Item = RateRow>) {
        if let Ok(mut inner) = self.inner.lock() {
            for row in rows {
                if inner.card.get(&row.upstream, &row.model).is_none() {
                    inner.card.set(row);
                }
            }
        }
    }

    /// The rate row for `upstream` and `model`, when there is one.
    pub fn rate_row(&self, upstream: &str, model: &str) -> Option<RateRow> {
        self.inner.lock().ok()?.card.get(upstream, model).cloned()
    }

    /// A copy of the rate card and the credit ledger, for the router to
    /// plan against without holding the meter's lock.
    pub fn snapshot(&self) -> (RateCard, Ledger) {
        self.inner
            .lock()
            .map(|inner| {
                (
                    inner.card.clone(),
                    Ledger::new(inner.ledger.accounts().cloned()),
                )
            })
            .unwrap_or_else(|_| (RateCard::default(), Ledger::default()))
    }

    /// One account's burn-down, for the router's credit ranking.
    pub fn burn_down(&self, account: &str, now_ms: u64) -> Option<BurnDown> {
        self.inner.lock().ok()?.ledger.burn_down(account, now_ms)
    }

    /// Reconcile `day` (days since the epoch, UTC) for every account the
    /// sources cover, plus the costs upstreams reported on the attempts.
    /// Gaps over 2% alert.
    pub fn reconcile(
        &self,
        sources: &[&dyn ProviderBilling],
        day: u64,
        now_ms: u64,
    ) -> Vec<Reconciled> {
        let accounts: Vec<CreditAccount> = match self.inner.lock() {
            Ok(inner) => inner.ledger.accounts().cloned().collect(),
            Err(_) => return Vec::new(),
        };
        // Read billing without holding the lock: a source may be slow.
        let mut billed = Vec::new();
        for account in &accounts {
            for source in sources.iter().filter(|s| s.covers(account)) {
                billed.push((
                    account,
                    source.source().to_owned(),
                    source.billed(account, day),
                ));
            }
        }
        let Ok(mut inner) = self.inner.lock() else {
            return Vec::new();
        };
        let today = now_ms / DAY_MS;
        let mut results = Vec::new();
        for (account, source, answer) in billed {
            let ours = inner.ledger.day(&account.id, day).cost;
            let mut result = Reconciled {
                account: account.id.clone(),
                day: date(day),
                source,
                ours,
                theirs: 0,
                gap_pct: 0.0,
                ok: false,
                balance: None,
                trouble: None,
            };
            match answer {
                Err(trouble) => result.trouble = Some(trouble),
                Ok(None) => result.trouble = Some("no figure for the day".into()),
                Ok(Some(billed)) if billed.currency != account.currency => {
                    result.trouble = Some(format!("billed in {}", billed.currency));
                }
                Ok(Some(billed)) => {
                    result.theirs = billed.cost;
                    result.gap_pct = reconcile::gap_pct(ours, billed.cost);
                    result.ok = result.gap_pct <= reconcile::GAP_PCT;
                    if let Some(balance) = billed.balance {
                        inner.ledger.set_balance(&account.id, balance);
                        result.balance = Some(balance);
                    }
                }
            }
            results.push(result);
        }
        for account in &accounts {
            let spend = inner.ledger.day(&account.id, day);
            if spend.reported_attempts == 0 {
                continue;
            }
            let gap = reconcile::gap_pct(spend.cost_of_reported, spend.reported);
            results.push(Reconciled {
                account: account.id.clone(),
                day: date(day),
                source: "upstream-reported".into(),
                ours: spend.cost_of_reported,
                theirs: spend.reported,
                gap_pct: gap,
                ok: gap <= reconcile::GAP_PCT,
                balance: None,
                trouble: None,
            });
        }
        for result in results.iter().filter(|r| !r.ok && r.trouble.is_none()) {
            inner.ledger.raise_gap(
                Alert::ReconcileGap {
                    account: result.account.clone(),
                    day: result.day.clone(),
                    source: result.source.clone(),
                    ours: result.ours,
                    theirs: result.theirs,
                    gap_pct: result.gap_pct,
                },
                today,
            );
        }
        inner.reconciled_day = Some(day);
        inner.reconciliations = results.clone();
        let alerts = inner.ledger.check(now_ms);
        drop(inner);
        self.deliver(alerts);
        results
    }

    /// The daily hook: reconcile yesterday once, then check alerts. Call
    /// it on a timer (hourly is plenty); it does the day's work once.
    pub fn tick(&self, sources: &[&dyn ProviderBilling], now_ms: u64) {
        let yesterday = (now_ms / DAY_MS).saturating_sub(1);
        let due = self
            .inner
            .lock()
            .is_ok_and(|inner| inner.reconciled_day.is_none_or(|day| day < yesterday));
        if due {
            self.reconcile(sources, yesterday, now_ms);
        }
        self.check(now_ms);
    }
}

impl Recorder for Meter {
    fn record(&self, mut attempt: Attempt) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if attempt.cost.is_none() {
            match inner.card.get(&attempt.upstream, &attempt.model) {
                Some(row) => {
                    let priced = row.price(&attempt.tokens);
                    attempt.currency = priced.currency;
                    attempt.cost = Some(priced.cost);
                    attempt.margin = Some(priced.margin);
                    attempt.price = Some(priced.price);
                }
                None => inner.unpriced += 1,
            }
        }
        let now = attempt.end_ms();
        inner.ledger.debit(&attempt);
        if let Some(journal) = inner.journal.as_mut()
            && let Err(trouble) = journal.append(&attempt)
        {
            inner.journal_errors += 1;
            if inner.journal_errors == 1 {
                eprintln!("inference: attempt journal: {trouble}");
            }
        }
        inner.store.push(attempt);
        inner.store.prune(now);
        let alerts = inner.ledger.check(now);
        drop(inner);
        self.deliver(alerts);
    }
}

#[cfg(test)]
mod tests;
