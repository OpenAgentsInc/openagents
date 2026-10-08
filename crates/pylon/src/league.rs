//! The pylon league: per hardware class (family and tier), each pylon's
//! pass rate on its class's pinned Gym suite, its accepted jobs, median
//! job time, and cost per accepted job, from verified beacons, receipts,
//! and trusted check verdicts. Nothing here is typed by hand: every number
//! is recomputed from signed records a reader fetched.
//!
//! Only canary verdicts that name the class's pinned suite count toward the
//! pass rate, so classes never pool results across suites. Every trusted
//! verdict (canaries and redundant runs) decides a pylon's standing, and
//! a pylon with passing checks and no failure carries the sigil.

use std::collections::{BTreeMap, BTreeSet};

use nostr::pylon::{BeaconBook, CheckTotals, Family, Outcome, Receipt, Standing, Tier, Verdict};
use serde::Serialize;
use serde_json::json;

use crate::check::{self, Verdicts};
use crate::client::beacons;
use crate::identity::Identity;
use crate::now;
use crate::relay::{self, LIFETIME};

/// One pylon's row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// The `30200` address.
    pub pylon: String,
    pub label: String,
    pub model: String,
    /// Verdicts on the class's pinned suite.
    pub suite: CheckTotals,
    /// `pass / (pass + fail)` on the suite; `None` before any decisive one.
    pub pass_rate: Option<f64>,
    /// Accepted, receipt-backed jobs, never self-dealt.
    pub jobs: u64,
    /// Median seconds from start to finish over accepted jobs.
    pub median_secs: Option<u64>,
    /// Paid msat per accepted job (all networks); `None` while all work is
    /// free.
    pub msat_per_job: Option<u64>,
    pub standing: Standing,
    /// Passing checks and no failure.
    pub sigil: bool,
}

/// One class's board.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Class {
    pub family: Family,
    pub tier: Tier,
    /// The pinned suite's ID and digest.
    pub suite_id: String,
    pub suite: String,
    /// Best pass rate first, then most jobs.
    pub rows: Vec<Row>,
}

/// The league.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct League {
    pub generated_at: u64,
    /// The checkers whose verdicts count.
    pub checkers: Vec<String>,
    pub classes: Vec<Class>,
}

/// Build the league from verified beacons, receipts by event ID, and the
/// counted verdicts.
#[must_use]
pub fn build(
    book: &BeaconBook,
    receipts: &BTreeMap<String, Receipt>,
    verdicts: &Verdicts,
    checkers: &BTreeSet<String>,
    at: u64,
) -> League {
    let standings = verdicts.standings();
    let mut seen = BTreeSet::new();
    let mut runs: BTreeMap<String, Vec<&Receipt>> = BTreeMap::new();
    for receipt in receipts.values() {
        if receipt.outcome == Outcome::Accepted
            && !book.self_dealt(&receipt.buyer, &receipt.address())
            && seen.insert((receipt.buyer.clone(), receipt.request.clone()))
        {
            runs.entry(receipt.address()).or_default().push(receipt);
        }
    }
    let mut classes: BTreeMap<(Family, u8), Class> = BTreeMap::new();
    for (_, beacon) in book.iter() {
        let suite = check::suite_for(beacon.class.family);
        let digest = suite.digest();
        let address = beacon.address();
        let mut totals = CheckTotals::default();
        for c in &verdicts.checks {
            let on_pylon = verdicts
                .receipts
                .get(&c.receipt)
                .is_some_and(|r| r.address() == address);
            if on_pylon && c.suite() == Some(digest.as_str()) {
                match c.verdict {
                    Verdict::Pass => totals.pass += 1,
                    Verdict::Fail => totals.fail += 1,
                    Verdict::Inconclusive => totals.inconclusive += 1,
                }
            }
        }
        let decisive = totals.pass + totals.fail;
        let jobs = runs.get(&address).map_or(&[][..], Vec::as_slice);
        let mut secs: Vec<u64> = jobs.iter().map(|r| r.finished_at - r.started_at).collect();
        secs.sort_unstable();
        let paid: u64 = jobs
            .iter()
            .filter_map(|r| r.payment.as_ref().map(|p| p.amount_msat))
            .sum();
        let standing = standings
            .get(&address)
            .map(|r| r.standing)
            .unwrap_or_default();
        let row = Row {
            pylon: address,
            label: beacon.label.clone(),
            model: beacon
                .services
                .first()
                .map(|s| s.model.clone())
                .unwrap_or_default(),
            pass_rate: (decisive > 0).then(|| totals.pass as f64 / decisive as f64),
            suite: totals,
            jobs: jobs.len() as u64,
            median_secs: secs.get(secs.len() / 2).copied(),
            msat_per_job: (paid > 0 && !jobs.is_empty()).then(|| paid / jobs.len() as u64),
            standing,
            sigil: standing == Standing::Passing,
        };
        classes
            .entry((beacon.class.family, tier_rank(beacon.class.tier)))
            .or_insert_with(|| Class {
                family: beacon.class.family,
                tier: beacon.class.tier,
                suite_id: suite.id.clone(),
                suite: digest.clone(),
                rows: Vec::new(),
            })
            .rows
            .push(row);
    }
    let mut classes: Vec<Class> = classes.into_values().collect();
    for class in &mut classes {
        class.rows.sort_by(|a, b| {
            let rate = |r: &Row| r.pass_rate.unwrap_or(-1.0);
            rate(b)
                .total_cmp(&rate(a))
                .then(b.jobs.cmp(&a.jobs))
                .then(a.pylon.cmp(&b.pylon))
        });
    }
    League {
        generated_at: at,
        checkers: checkers.iter().cloned().collect(),
        classes,
    }
}

fn tier_rank(tier: Tier) -> u8 {
    match tier {
        Tier::Small => 0,
        Tier::Medium => 1,
        Tier::Large => 2,
        Tier::Xl => 3,
    }
}

/// Fetch the relay's beacons, the last day's receipts, and the trusted
/// verdicts, and build the league.
///
/// # Errors
///
/// When the relay cannot be read.
pub async fn fetch(
    reader: &Identity,
    relay_url: &str,
    checkers: &BTreeSet<String>,
) -> Result<League, String> {
    let since = now().saturating_sub(check::CHECK_WINDOW_SECS);
    let mut conn = relay::connect(relay_url, reader, LIFETIME).await?;
    let book = beacons(&mut conn, None).await?;
    let mut receipts = relay::query(
        &mut conn,
        "league-receipts",
        &[json!({
            "kinds": [nostr::pylon::RECEIPT_KIND],
            "#t": [nostr::pylon::RECEIPT_MARKER],
            "since": since,
            "limit": 5_000,
        })],
    )
    .await?;
    let (labels, checked) = check::fetch(&mut conn, checkers, since).await?;
    let _ = conn.close().await;
    receipts.extend(checked);
    let verdicts = Verdicts::new(&labels, &receipts, checkers);
    Ok(build(&book, &verdicts.receipts, &verdicts, checkers, now()))
}

/// The league as text, one block per class.
#[must_use]
pub fn render(league: &League) -> String {
    if league.classes.is_empty() {
        return "no pylons on this relay".into();
    }
    let mut out = String::new();
    for class in &league.classes {
        out.push_str(&format!(
            "{:?}/{:?}  suite {} ({})\n",
            class.family,
            class.tier,
            class.suite_id,
            &class.suite[..12]
        ));
        for row in &class.rows {
            out.push_str(&format!(
                "  {} {:<20} {:<22} pass {:>4}  ({}/{}/{})  jobs {:>4}  median {}  cost {}\n",
                if row.sigil { "*" } else { " " },
                row.label,
                row.model,
                row.pass_rate
                    .map_or_else(|| "-".into(), |r| format!("{:.0}%", r * 100.0)),
                row.suite.pass,
                row.suite.fail,
                row.suite.inconclusive,
                row.jobs,
                row.median_secs
                    .map_or_else(|| "-".into(), |s| format!("{s}s")),
                row.msat_per_job
                    .map_or_else(|| "free".into(), |m| format!("{m} msat")),
            ));
        }
    }
    out.push_str("* passing checks (pass/fail/inconclusive on the pinned suite)");
    out
}
