//! Named sales for affiliate attribution (#11698).
//!
//! When the Stripe webhook records a paid Pro month (`invoice.paid`), and
//! the paying account signed up through a `/go/{code}` or `/r/{token}`
//! link (its captured source carries the click, `referrals::Campaign`),
//! the sale is written as a [`Conversion`] joined to that attribution: the
//! code or affiliate, the series, the amount and the period. The first
//! month is a `start`, later months `renewal`s. A full refund or a lost
//! dispute on that invoice writes a `reversal` of the same amount.
//!
//! An account with no click has no conversion: nothing is attributed
//! without a click. Commission is not computed here: [`crate::affiliates`]
//! reads this journal into the affiliate program's ledger
//! (`tenancy::accounts::referrals::affiliate`), and payouts are only
//! listed for the owner.
//!
//! The journal is `attribution-conversions.jsonl` in the gateway's state
//! directory, one conversion per line, written once per Stripe event.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tenancy::accounts::referrals::{LinkKind, Outcome, Source};
use tenancy::billing::Event;

use crate::subscriptions::Meaning;

pub const SCHEMA: &str = "openagents.attribution.conversion.v1";
/// The journal's file name in the gateway's state directory.
pub const JOURNAL: &str = "attribution-conversions.jsonl";

/// What a conversion records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The subscription's first paid month.
    Start,
    /// A later paid month.
    Renewal,
    /// A full refund or a lost dispute undid a recorded month.
    Reversal,
}

/// One named sale (or its reversal).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Conversion {
    pub schema: String,
    /// The Stripe event, so a resent event writes nothing new.
    pub event: String,
    pub kind: Kind,
    pub account: String,
    pub workspace: Option<String>,
    /// The invoice the month was paid with.
    pub invoice: String,
    /// The service period paid for, in Unix seconds.
    pub period: Option<(i64, i64)>,
    /// Cents of `currency`.
    pub amount_cents: u64,
    pub currency: Option<String>,
    /// The tax inside `amount_cents`, when the invoice named it. Commission
    /// leaves tax out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax_cents: Option<u64>,
    pub link: LinkKind,
    /// The `/go` campaign code.
    pub code: Option<String>,
    /// The affiliate (the referral system's stable referrer id).
    pub referrer: Option<String>,
    pub series: Option<String>,
    pub click: String,
    pub at: u64,
}

/// What a verified Stripe event may become once it is applied: read from
/// the event and its meaning before the book moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Paid {
        event: String,
        account: String,
        workspace: Option<String>,
        invoice: String,
        period: Option<(i64, i64)>,
        amount_cents: u64,
        currency: Option<String>,
        tax_cents: Option<u64>,
        first: bool,
    },
    Reversed {
        event: String,
        invoice: String,
    },
}

fn text(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// The account, workspace and period a paid invoice names: the checkout's
/// metadata on the subscription (either API shape) or on a line.
fn paid_invoice(object: &Value) -> Option<(String, Option<String>, Option<(i64, i64)>)> {
    let metadata = [
        &object["subscription_details"]["metadata"],
        &object["parent"]["subscription_details"]["metadata"],
    ]
    .into_iter()
    .find(|m| m["oa_account"].is_string())
    .cloned()
    .or_else(|| {
        object["lines"]["data"].as_array().and_then(|lines| {
            lines
                .iter()
                .find(|l| l["metadata"]["oa_account"].is_string())
                .map(|l| l["metadata"].clone())
        })
    })?;
    let period = object["lines"]["data"].as_array().and_then(|lines| {
        lines
            .iter()
            .filter_map(|l| Some((l["period"]["start"].as_i64()?, l["period"]["end"].as_i64()?)))
            .filter(|(s, e)| e > s)
            .max_by_key(|(_, e)| *e)
    });
    Some((
        text(&metadata["oa_account"])?,
        text(&metadata["oa_workspace"]),
        period,
    ))
}

/// The tax on a paid invoice, in cents: `total_taxes` (2025 API) or
/// `total_tax_amounts` summed, else `tax`. `None` when it names none.
fn invoice_tax(object: &Value) -> Option<u64> {
    for list in ["total_taxes", "total_tax_amounts"] {
        if let Some(rows) = object[list].as_array() {
            return Some(rows.iter().filter_map(|r| r["amount"].as_u64()).sum());
        }
    }
    object["tax"].as_u64()
}

/// The hook point in the webhook: what this event may record once the
/// book has applied it. Only a paid invoice, a full refund and a lost
/// dispute can.
#[must_use]
pub fn pending(event: &Value, meaning: &Meaning) -> Option<Pending> {
    let Meaning::Book(book_event) = meaning else {
        return None;
    };
    let Event {
        id, kind, invoice, ..
    } = book_event.as_ref();
    let invoice = invoice.clone()?;
    match kind.as_str() {
        "invoice-paid" => {
            let object = &event["data"]["object"];
            let (account, workspace, period) = paid_invoice(object)?;
            Some(Pending::Paid {
                event: id.clone(),
                account,
                workspace,
                invoice,
                period,
                amount_cents: object["amount_paid"].as_u64().unwrap_or(0),
                currency: text(&object["currency"]).map(|c| c.to_ascii_uppercase()),
                tax_cents: invoice_tax(object),
                first: object["billing_reason"] == "subscription_create",
            })
        }
        "charge-refunded" | "dispute-lost" => Some(Pending::Reversed {
            event: id.clone(),
            invoice,
        }),
        _ => None,
    }
}

/// The conversion a pending event records, given the paying account's
/// captured source and the conversions already written. `None`: the
/// account came through no link, the event was already written, or (for a
/// reversal) the invoice was never a conversion or was already reversed.
#[must_use]
pub fn conversion(
    pending: &Pending,
    source: Option<&Source>,
    prior: &[Conversion],
    now: u64,
) -> Option<Conversion> {
    let event = match pending {
        Pending::Paid { event, .. } | Pending::Reversed { event, .. } => event,
    };
    if prior.iter().any(|c| &c.event == event) {
        return None;
    }
    match pending {
        Pending::Paid {
            event,
            account,
            workspace,
            invoice,
            period,
            amount_cents,
            currency,
            tax_cents,
            first,
        } => {
            let source = source.filter(|s| &s.account == account)?;
            let campaign = source.campaign.as_ref()?;
            let referrer = match campaign.link {
                LinkKind::Affiliate if source.outcome == Outcome::Captured => {
                    Some(source.referrer.as_ref()?.id.clone())
                }
                LinkKind::Affiliate => return None,
                LinkKind::Go => None,
            };
            if prior
                .iter()
                .any(|c| &c.invoice == invoice && c.kind != Kind::Reversal)
            {
                return None;
            }
            Some(Conversion {
                schema: SCHEMA.into(),
                event: event.clone(),
                kind: if *first { Kind::Start } else { Kind::Renewal },
                account: account.clone(),
                workspace: workspace.clone(),
                invoice: invoice.clone(),
                period: *period,
                amount_cents: *amount_cents,
                currency: currency.clone(),
                tax_cents: *tax_cents,
                link: campaign.link,
                code: campaign.code.clone(),
                referrer,
                series: campaign.series.clone(),
                click: campaign.click.clone(),
                at: now,
            })
        }
        Pending::Reversed { event, invoice } => {
            let sale = prior
                .iter()
                .find(|c| &c.invoice == invoice && c.kind != Kind::Reversal)?;
            if prior
                .iter()
                .any(|c| &c.invoice == invoice && c.kind == Kind::Reversal)
            {
                return None;
            }
            Some(Conversion {
                event: event.clone(),
                kind: Kind::Reversal,
                at: now,
                ..sale.clone()
            })
        }
    }
}

/// Every conversion written so far; unreadable lines are skipped.
pub fn read_journal(dir: &Path) -> std::io::Result<Vec<Conversion>> {
    match std::fs::read_to_string(dir.join(JOURNAL)) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str::<Conversion>(line).ok())
            .filter(|c| c.schema == SCHEMA)
            .collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn append(dir: &Path, conversion: &Conversion) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(conversion).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(JOURNAL))?;
    file.write_all(&line)?;
    file.sync_data()
}

/// Write what `pending` records, after the book applied its event. A
/// failure is reported and never fails the webhook: the paid month stands
/// either way, and the journal can be rebuilt from Stripe's events.
pub fn settle(dir: &Path, pending: &Pending, now: u64) {
    let result = (|| -> Result<Option<Conversion>, String> {
        let prior = read_journal(dir).map_err(|e| e.to_string())?;
        let source = match pending {
            Pending::Paid { account, .. } => tenancy::Accounts::open(dir)
                .map_err(|e| e.to_string())?
                .acquisition(account)
                .ok()
                .flatten(),
            Pending::Reversed { .. } => None,
        };
        let Some(made) = conversion(pending, source.as_ref(), &prior, now) else {
            return Ok(None);
        };
        append(dir, &made).map_err(|e| e.to_string())?;
        Ok(Some(made))
    })();
    if let Err(problem) = result {
        eprintln!("attribution: a conversion was not written: {problem}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tenancy::accounts::referrals::{Basis, Campaign, Identity, Kind as ReferrerKind};

    fn source(account: &str, campaign: Option<Campaign>, referrer: bool) -> Source {
        Source {
            schema: tenancy::accounts::referrals::SCHEMA.into(),
            account: account.into(),
            request: "signup-x".into(),
            outcome: if referrer {
                Outcome::Captured
            } else {
                Outcome::Missing
            },
            referrer: referrer.then(|| Identity {
                id: "ref_0123456789abcdef0123456789abcdef".into(),
                version: 1,
                kind: ReferrerKind::Author,
                source_only: false,
            }),
            consent_version: None,
            captured_at: 1,
            campaign,
        }
    }

    fn click(link: LinkKind, code: Option<&str>) -> Campaign {
        Campaign {
            link,
            code: code.map(str::to_owned),
            series: Some("cost".into()),
            click: format!("clk_{}", "c".repeat(32)),
            clicked_at: 1,
            basis: Basis::Consent,
        }
    }

    /// A fake `invoice.paid` event in Stripe's 2025 shape, and the meaning
    /// `subscriptions::read` gives it once the subscription is started.
    fn paid(event: &str, invoice: &str, reason: &str) -> (Value, Meaning) {
        let value = json!({
            "id": event,
            "type": "invoice.paid",
            "data": {"object": {
                "id": invoice,
                "amount_paid": 2000,
                "currency": "usd",
                "total_taxes": [{"amount": 150}],
                "billing_reason": reason,
                "parent": {"subscription_details": {
                    "subscription": "sub_1",
                    "metadata": {"oa_account": "acct_buyer", "oa_workspace": "ws_buyer", "oa_plan": "pro"}
                }},
                "lines": {"data": [{"period": {"start": 100, "end": 200}}]}
            }}
        });
        let mut e = Event {
            provider: "stripe".into(),
            id: event.into(),
            kind: "invoice-paid".into(),
            checkout: None,
            subscription: Some("sub_book".into()),
            invoice: Some(invoice.into()),
            period: 1,
            amount: 0,
            currency: None,
            at_period_end: false,
            provider_ref: None,
            provider_subscription: None,
            customer: None,
            charges: Vec::new(),
            received: 0,
            applied: false,
            outcome: String::new(),
        };
        e.amount = 2000 * 10_000;
        (value, Meaning::Book(Box::new(e)))
    }

    fn refunded(event: &str, invoice: &str) -> (Value, Meaning) {
        let (value, meaning) = paid(event, invoice, "x");
        let Meaning::Book(mut e) = meaning else {
            unreachable!()
        };
        e.kind = "charge-refunded".into();
        (value, Meaning::Book(e))
    }

    #[test]
    fn a_paid_month_through_a_campaign_link_is_a_named_start() {
        let (value, meaning) = paid("evt_1", "in_1", "subscription_create");
        let pending = pending(&value, &meaning).unwrap();
        let buyer = source(
            "acct_buyer",
            Some(click(LinkKind::Go, Some("yt-cost"))),
            false,
        );
        let made = conversion(&pending, Some(&buyer), &[], 9).unwrap();
        assert_eq!(made.kind, Kind::Start);
        assert_eq!(made.code.as_deref(), Some("yt-cost"));
        assert_eq!(made.series.as_deref(), Some("cost"));
        assert_eq!(made.amount_cents, 2000);
        assert_eq!(made.tax_cents, Some(150));
        assert_eq!(made.currency.as_deref(), Some("USD"));
        assert_eq!(made.period, Some((100, 200)));
        assert_eq!(made.workspace.as_deref(), Some("ws_buyer"));
        assert!(made.referrer.is_none());
        // The same event again writes nothing.
        assert!(conversion(&pending, Some(&buyer), &[made], 10).is_none());
    }

    #[test]
    fn renewals_recur_for_an_affiliate_and_a_refund_reverses_one() {
        let buyer = source("acct_buyer", Some(click(LinkKind::Affiliate, None)), true);
        let (v1, m1) = paid("evt_1", "in_1", "subscription_create");
        let start = conversion(&pending(&v1, &m1).unwrap(), Some(&buyer), &[], 1).unwrap();
        let (v2, m2) = paid("evt_2", "in_2", "subscription_cycle");
        let renewal = conversion(
            &pending(&v2, &m2).unwrap(),
            Some(&buyer),
            &[start.clone()],
            2,
        )
        .unwrap();
        assert_eq!(renewal.kind, Kind::Renewal);
        assert_eq!(
            renewal.referrer.as_deref(),
            Some("ref_0123456789abcdef0123456789abcdef")
        );
        let prior = vec![start, renewal.clone()];
        let (v3, m3) = refunded("evt_3", "in_2");
        let reversal = conversion(&pending(&v3, &m3).unwrap(), None, &prior, 3).unwrap();
        assert_eq!(reversal.kind, Kind::Reversal);
        assert_eq!(reversal.invoice, "in_2");
        assert_eq!(reversal.amount_cents, renewal.amount_cents);
        assert_eq!(reversal.referrer, renewal.referrer);
        // A second refund event for the same invoice reverses nothing more.
        let mut after = prior.clone();
        after.push(reversal);
        let (v4, m4) = refunded("evt_4", "in_2");
        assert!(conversion(&pending(&v4, &m4).unwrap(), None, &after, 4).is_none());
    }

    #[test]
    fn no_click_no_conversion() {
        let (value, meaning) = paid("evt_1", "in_1", "subscription_create");
        let pending = pending(&value, &meaning).unwrap();
        assert!(conversion(&pending, None, &[], 1).is_none());
        let plain = source("acct_buyer", None, false);
        assert!(conversion(&pending, Some(&plain), &[], 1).is_none());
        // Someone else's source never names this sale.
        let other = source(
            "acct_other",
            Some(click(LinkKind::Go, Some("x-cost"))),
            false,
        );
        assert!(conversion(&pending, Some(&other), &[], 1).is_none());
        // A refund of an invoice that was never a conversion writes nothing.
        let (v, m) = refunded("evt_9", "in_1");
        assert!(conversion(&super::pending(&v, &m).unwrap(), None, &[], 1).is_none());
        // Other events are not hook points.
        assert!(super::pending(&value, &Meaning::Ignored("x".into())).is_none());
    }

    #[test]
    fn the_journal_appends_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_journal(dir.path()).unwrap().is_empty());
        let (value, meaning) = paid("evt_1", "in_1", "subscription_create");
        let buyer = source(
            "acct_buyer",
            Some(click(LinkKind::Go, Some("yt-cost"))),
            false,
        );
        let made = conversion(&pending(&value, &meaning).unwrap(), Some(&buyer), &[], 1).unwrap();
        append(dir.path(), &made).unwrap();
        assert_eq!(read_journal(dir.path()).unwrap(), vec![made]);
    }
}
