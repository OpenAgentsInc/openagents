//! Affiliate commission (#11698): each affiliate's accrued, held, payable,
//! paid and clawed-back commission, from the sales the Stripe webhook
//! records ([`crate::conversions`]) under the affiliate program's terms
//! (`tenancy::accounts::referrals::affiliate`).
//!
//! - `GET /v1/admin/affiliates`: every affiliate, and the payouts due this
//!   month, for site admins (anyone else gets `404`).
//! - `GET /v1/account/affiliate`: the signed-in account's own affiliate
//!   links and commission.
//!
//! Nothing here sends money. Payouts due are listed for the owner, who
//! approves and sends them over Lightning through the payout bridge, then
//! records each one in `affiliate-payouts.jsonl` in the gateway's state
//! directory (one [`affiliate::Payout`] per line), which this reads as
//! "paid". Dollars become sats at the gateway's bitcoin price, the
//! operator-set `inference.sats_rate` the rate card uses.

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use serde_json::{Value, json};
use tenancy::accounts::referrals::affiliate::{
    self, Balance, Directory, Ledger, PROGRAM, Payout, Sale, SaleKind,
};

use crate::accounts::{self, refused};
use crate::conversions::{self, Conversion, Kind};
use crate::serve::ServeState;

pub const SCHEMA: &str = "openagents.affiliates.v1";
/// Payouts the owner sent, recorded after the fact. Nothing here writes it.
pub const PAYOUTS: &str = "affiliate-payouts.jsonl";

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/admin/affiliates", get(admin)),
        ("/v1/account/affiliate", get(own)),
    ]
}

/// A recorded conversion as a sale of the affiliate program.
#[must_use]
pub fn sale(c: &Conversion) -> Sale {
    Sale {
        kind: match c.kind {
            Kind::Start | Kind::Renewal => SaleKind::Paid,
            Kind::Reversal => SaleKind::Reversed,
        },
        link: c.link,
        referrer: c.referrer.clone(),
        account: c.account.clone(),
        workspace: c.workspace.clone(),
        invoice: c.invoice.clone(),
        at: c.at,
        amount_cents: c.amount_cents,
        currency: c.currency.clone(),
        tax_cents: c.tax_cents,
        // Stripe's invoice carries no fee: the program's card fee applies.
        fee_cents: None,
    }
}

/// Every payout recorded so far; unreadable lines are skipped.
pub fn read_payouts(dir: &Path) -> std::io::Result<Vec<Payout>> {
    match std::fs::read_to_string(dir.join(PAYOUTS)) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// The ledger as of `now` from the state directory.
pub fn ledger_at(dir: &Path, directory: &Directory, now: u64) -> std::io::Result<Ledger> {
    let sales: Vec<Sale> = conversions::read_journal(dir)?.iter().map(sale).collect();
    let payouts = read_payouts(dir)?;
    Ok(affiliate::ledger(
        &PROGRAM, &sales, directory, &payouts, now,
    ))
}

fn program() -> Value {
    json!({
        "rate_percent": PROGRAM.rate_percent(),
        "paid_months": PROGRAM.paid_months,
        "hold_days": PROGRAM.hold_days(),
        "minimum_sats": PROGRAM.minimum_sats,
        "terms_version": affiliate::TERMS_VERSION,
    })
}

fn row(referrer: &str, directory: &Directory, b: &Balance) -> Value {
    let a = directory.affiliates.get(referrer);
    json!({
        "referrer": referrer,
        "label": a.map(|a| a.label.as_str()).unwrap_or(""),
        "keys": a.map(|a| a.keys.clone()).unwrap_or_default(),
        "paid_conversions": b.paid_conversions,
        "refunds": b.refunds,
        "accrued_micros": b.accrued,
        "held_micros": b.held,
        "payable_micros": b.payable,
        "paid_micros": b.paid,
        "clawed_back_micros": b.clawed_back,
        "owed_micros": b.owed,
    })
}

/// The admin report: every affiliate with commission, and who is due a
/// payout at `usd_per_btc` (none without a bitcoin price).
#[must_use]
pub fn admin_report(
    directory: &Directory,
    ledger: &Ledger,
    sats_rate: Option<Value>,
    usd_per_btc: Option<u64>,
) -> Value {
    let affiliates: Vec<Value> = ledger
        .balances
        .iter()
        .map(|(referrer, b)| row(referrer, directory, b))
        .collect();
    let due: Vec<Value> = usd_per_btc
        .map(|rate| affiliate::payouts_due(&PROGRAM, ledger, rate))
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            json!({
                "referrer": d.referrer,
                "label": directory.affiliates.get(&d.referrer).map(|a| a.label.as_str()).unwrap_or(""),
                "usd_micros": d.usd_micros,
                "sats": d.sats,
            })
        })
        .collect();
    json!({
        "v": SCHEMA,
        "program": program(),
        "sats_rate": sats_rate,
        "affiliates": affiliates,
        "payouts_due": due,
    })
}

/// One account's own report: every referrer it owns, with its commission
/// (zero when none yet).
#[must_use]
pub fn own_report(account: &str, directory: &Directory, ledger: &Ledger) -> Value {
    let empty = Balance::default();
    let affiliates: Vec<Value> = directory
        .owned_by(account)
        .map(|referrer| {
            row(
                referrer,
                directory,
                ledger.balances.get(referrer).unwrap_or(&empty),
            )
        })
        .collect();
    json!({"v": SCHEMA, "program": program(), "affiliates": affiliates})
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn unavailable(problem: impl std::fmt::Display) -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "affiliates_unavailable",
        format!("The affiliate numbers couldn't be read: {problem}"),
    )
}

/// The directory and ledger, read fresh.
fn read(state: &ServeState) -> Result<(Directory, Ledger), Response> {
    let store = accounts::accounts_store(state)?
        .store()
        .map_err(unavailable)?;
    let directory = Directory::of(&store);
    let ledger = ledger_at(&state.dir, &directory, now()).map_err(unavailable)?;
    Ok((directory, ledger))
}

fn signed_in(state: &ServeState, headers: &HeaderMap) -> Result<String, Response> {
    let principal = accounts::principal(state, headers)?;
    Ok(accounts::member_account(&principal)?.to_owned())
}

async fn admin(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let account = match signed_in(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    if !accounts::account_is_site_admin(&state, &account) {
        return refused(StatusCode::NOT_FOUND, "not_found", "Not found.");
    }
    let (directory, ledger) = match read(&state) {
        Ok(read) => read,
        Err(response) => return response,
    };
    let rate = state
        .config
        .inference
        .as_ref()
        .and_then(|config| config.sats_rate.clone());
    let usd_per_btc = rate.as_ref().map(|r| r.usd_per_btc);
    let sats_rate = rate.map(|r| json!({"usd_per_btc": r.usd_per_btc, "as_of": r.as_of}));
    (
        StatusCode::OK,
        Json(admin_report(&directory, &ledger, sats_rate, usd_per_btc)),
    )
        .into_response()
}

async fn own(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let account = match signed_in(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let (directory, ledger) = match read(&state) {
        Ok(read) => read,
        Err(response) => return response,
    };
    (
        StatusCode::OK,
        Json(own_report(&account, &directory, &ledger)),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tenancy::accounts::referrals::LinkKind;
    use tenancy::accounts::referrals::affiliate::Affiliate;

    const DAY: u64 = 86_400;

    fn conversion(kind: Kind, invoice: &str, at: u64, link: LinkKind) -> Conversion {
        Conversion {
            schema: conversions::SCHEMA.into(),
            event: format!("evt_{invoice}_{at}"),
            kind,
            account: "acct_buyer".into(),
            workspace: Some("ws_buyer".into()),
            invoice: invoice.into(),
            period: None,
            amount_cents: 2000,
            currency: Some("USD".into()),
            tax_cents: Some(0),
            link,
            code: (link == LinkKind::Go).then(|| "yt-cost".into()),
            referrer: (link == LinkKind::Affiliate).then(|| "ref_a".into()),
            series: None,
            click: format!("clk_{}", "c".repeat(32)),
            at,
        }
    }

    fn directory() -> Directory {
        let mut d = Directory::default();
        d.affiliates.insert(
            "ref_a".into(),
            Affiliate {
                owner: "acct_aff".into(),
                label: "Creator".into(),
                keys: vec!["aff-0123456789abcdef".into()],
            },
        );
        d
    }

    #[test]
    fn the_journal_becomes_the_admin_and_own_reports() {
        let dir = tempfile::tempdir().unwrap();
        let t0 = 1_800_000_000;
        let lines: Vec<Conversion> = vec![
            conversion(Kind::Start, "in_1", t0, LinkKind::Affiliate),
            conversion(Kind::Renewal, "in_2", t0 + 30 * DAY, LinkKind::Affiliate),
            conversion(Kind::Reversal, "in_2", t0 + 31 * DAY, LinkKind::Affiliate),
            conversion(Kind::Start, "in_3", t0, LinkKind::Go),
        ];
        let text: String = lines
            .iter()
            .map(|c| serde_json::to_string(c).unwrap() + "\n")
            .collect();
        std::fs::write(dir.path().join(conversions::JOURNAL), text).unwrap();
        let d = directory();
        let ledger = ledger_at(dir.path(), &d, t0 + 40 * DAY).unwrap();
        // $20.00 less the $0.88 card fee is $19.12; 20% is $3.824. The
        // renewal was refunded inside its hold; the /go/ sale earns nothing.
        let b = ledger.balances["ref_a"];
        assert_eq!(b.payable, 3_824_000);
        assert_eq!(b.refunds, 1);
        let report = admin_report(&d, &ledger, None, Some(38_000));
        assert_eq!(report["affiliates"][0]["label"], "Creator");
        assert_eq!(report["affiliates"][0]["payable_micros"], 3_824_000);
        // 3.824 USD at $38,000 is 10,063 sats: due.
        assert_eq!(report["payouts_due"][0]["sats"], 10_063);
        assert!(
            admin_report(&d, &ledger, None, None)["payouts_due"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        // A recorded payout counts as paid and empties the balance.
        let payout = Payout {
            referrer: "ref_a".into(),
            usd_micros: 3_824_000,
            sats: 10_063,
            at: t0 + 39 * DAY,
        };
        std::fs::write(
            dir.path().join(PAYOUTS),
            serde_json::to_string(&payout).unwrap() + "\n",
        )
        .unwrap();
        let ledger = ledger_at(dir.path(), &d, t0 + 40 * DAY).unwrap();
        let own = own_report("acct_aff", &d, &ledger);
        assert_eq!(own["affiliates"][0]["paid_micros"], 3_824_000);
        assert_eq!(own["affiliates"][0]["payable_micros"], 0);
        assert_eq!(own["affiliates"][0]["keys"][0], "aff-0123456789abcdef");
        assert!(
            own_report("acct_other", &d, &ledger)["affiliates"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
