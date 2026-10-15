//! Affiliate commission on the site (#11698): the admin's commission
//! table and payouts-due list on `/admin/attribution`, and an affiliate's
//! own "Your affiliate earnings" section in Settings.
//!
//! The numbers come from the account service (`GET /v1/admin/affiliates`
//! for a site admin, `GET /v1/account/affiliate` for the signed-in
//! account), which works them out from the recorded sales under the
//! published terms. Nothing here sends money: payouts are listed for the
//! owner, who approves and sends them.

use std::collections::BTreeMap;

use maud::{Markup, html};
use openagents_ui::content::{MarkdownRoot, Table};
use serde::Deserialize;

use super::Day;
use crate::analytics::dashboard::number;

/// How many days of clicks and sign-ups Settings counts.
pub const SETTINGS_DAYS: u64 = 90;

/// The program's numbers as the account service reports them.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Program {
    #[serde(default)]
    pub rate_percent: u64,
    #[serde(default)]
    pub paid_months: u64,
    #[serde(default)]
    pub hold_days: u64,
    #[serde(default)]
    pub minimum_sats: u64,
}

/// The bitcoin price payouts would be figured at.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct SatsRate {
    pub usd_per_btc: u64,
    #[serde(default)]
    pub as_of: String,
}

/// One affiliate's numbers. Amounts are millionths of a US dollar.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Affiliate {
    #[serde(default)]
    pub referrer: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub paid_conversions: u64,
    #[serde(default)]
    pub refunds: u64,
    #[serde(default)]
    pub accrued_micros: u64,
    #[serde(default)]
    pub held_micros: u64,
    #[serde(default)]
    pub payable_micros: u64,
    #[serde(default)]
    pub paid_micros: u64,
    #[serde(default)]
    pub clawed_back_micros: u64,
    #[serde(default)]
    pub owed_micros: u64,
}

/// One affiliate the owner may pay this month.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Due {
    #[serde(default)]
    pub referrer: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub usd_micros: u64,
    #[serde(default)]
    pub sats: u64,
}

/// What the account service answers.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Report {
    #[serde(default)]
    pub program: Program,
    #[serde(default)]
    pub sats_rate: Option<SatsRate>,
    #[serde(default)]
    pub affiliates: Vec<Affiliate>,
    #[serde(default)]
    pub payouts_due: Vec<Due>,
}

impl Report {
    /// The answer, when it is one.
    #[must_use]
    pub fn parse(value: serde_json::Value) -> Option<Self> {
        serde_json::from_value(value).ok()
    }
}

/// Millionths of a dollar as dollars and cents, rounded down: `$12.34`.
#[must_use]
pub fn dollars(micros: u64) -> String {
    let cents = micros / 10_000;
    format!("${}.{:02}", number(cents / 100), cents % 100)
}

fn name(affiliate: &Affiliate) -> &str {
    if affiliate.label.is_empty() {
        &affiliate.referrer
    } else {
        &affiliate.label
    }
}

/// The admin's commission section: per affiliate, then the payouts due.
/// `None`: the numbers couldn't be read (or the page was opened with the
/// dashboard key, which has no sign-in to ask with).
pub(crate) fn admin_section(report: Option<&Report>) -> Markup {
    let Some(report) = report else {
        return html! {
            (MarkdownRoot::new(html! {
                h2 { "Commission" }
                p { "Sign in as a site admin to see commission. If you are signed in, the numbers couldn't be read; try again in a minute." }
            }))
        };
    };
    let program = &report.program;
    let mut table = Table::new()
        .label("Commission per affiliate")
        .header([
            "Affiliate",
            "Paid sales",
            "Refunds",
            "Earned",
            "Waiting",
            "Ready to pay",
            "Paid",
            "Taken back",
        ])
        .numeric(1)
        .numeric(2)
        .numeric(3)
        .numeric(4)
        .numeric(5)
        .numeric(6)
        .numeric(7);
    for a in &report.affiliates {
        table = table.row([
            html! { (name(a)) },
            html! { (number(a.paid_conversions)) },
            html! { (number(a.refunds)) },
            html! { (dollars(a.accrued_micros)) },
            html! { (dollars(a.held_micros)) },
            html! {
                (dollars(a.payable_micros))
                @if a.owed_micros > 0 { " (" (dollars(a.owed_micros)) " still to take back)" }
            },
            html! { (dollars(a.paid_micros)) },
            html! { (dollars(a.clawed_back_micros)) },
        ]);
    }
    let mut due = Table::new()
        .label("Payouts due")
        .header(["Affiliate", "Amount", "Sats"])
        .numeric(1)
        .numeric(2);
    for d in &report.payouts_due {
        let label = if d.label.is_empty() {
            &d.referrer
        } else {
            &d.label
        };
        due = due.row([
            html! { (label) },
            html! { (dollars(d.usd_micros)) },
            html! { (number(d.sats)) },
        ]);
    }
    html! {
        (MarkdownRoot::new(html! {
            h2 { "Commission" }
            p {
                (program.rate_percent) "% of net Pro revenue for each referred account's first "
                (program.paid_months) " paid months. Commission waits " (program.hold_days)
                " days after each payment before it is ready to pay; a refund in that time cancels it, and a later one is taken back."
            }
        }))
        @if report.affiliates.is_empty() {
            p { "No affiliate sales yet." }
        } @else {
            (table)
        }
        (MarkdownRoot::new(html! {
            h2 { "Payouts due" }
            p {
                "Affiliates whose ready-to-pay commission is at least " (number(program.minimum_sats)) " sats"
                @if let Some(rate) = &report.sats_rate {
                    " at $" (number(rate.usd_per_btc)) " per bitcoin"
                    @if !rate.as_of.is_empty() { " (" (rate.as_of) ")" }
                }
                ". Payouts are sent in sats over Lightning once a month, and each one needs the owner's approval. Nothing is sent from this page."
            }
        }))
        @if report.sats_rate.is_none() {
            p { "No bitcoin price is set, so sats can't be worked out yet." }
        } @else if report.payouts_due.is_empty() {
            p { "No payouts are due." }
        } @else {
            (due)
        }
    }
}

/// Clicks we may credit and sign-ups, for `keys`, over `days`.
#[must_use]
pub fn counts(keys: &[String], days: &BTreeMap<u64, Day>) -> (u64, u64) {
    let mine = |key: &String| keys.contains(key);
    days.values().fold((0, 0), |(clicks, signups), day| {
        (
            clicks
                + day
                    .clicks
                    .iter()
                    .filter(|c| c.credited && mine(&c.key))
                    .count() as u64,
            signups + day.signups.iter().filter(|s| mine(&s.key)).count() as u64,
        )
    })
}

/// "Your affiliate earnings" in Settings, for an account with at least one
/// affiliate link; nothing otherwise.
pub(crate) fn settings_section(report: &Report, days: &BTreeMap<u64, Day>) -> Markup {
    if report.affiliates.is_empty() {
        return html! {};
    }
    html! {
        section class="oa-settings-group" aria-labelledby="settings-affiliate" {
            h2 #settings-affiliate { "Your affiliate earnings" }
            @for a in &report.affiliates {
                @let (clicks, signups) = counts(&a.keys, days);
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { (name(a)) }
                        span class="oa-settings-hint" {
                            "Your link's short code: "
                            @for (i, key) in a.keys.iter().enumerate() {
                                @if i > 0 { ", " }
                                code { (key) }
                            }
                            @if a.keys.is_empty() { "none yet" }
                            ". In the last " (SETTINGS_DAYS) " days, clicks: " (number(clicks)) ", sign-ups: "
                            (number(signups)) ". Paid sales: " (number(a.paid_conversions)) "."
                        }
                        span class="oa-settings-hint" {
                            "Waiting: " (dollars(a.held_micros))
                            ". Ready to pay: " (dollars(a.payable_micros))
                            ". Paid: " (dollars(a.paid_micros)) "."
                            @if a.owed_micros > 0 {
                                " Still to take back for refunds: " (dollars(a.owed_micros)) "."
                            }
                        }
                    }
                }
            }
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-hint" {
                        "Commission waits " (report.program.hold_days) " days after each payment. We pay in sats over Lightning once your ready-to-pay balance is at least "
                        (number(report.program.minimum_sats)) " sats."
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribution::{Click, Link, Signup};
    use serde_json::json;

    fn fixture(admin: bool) -> Report {
        let mut value = json!({
            "v": "openagents.affiliates.v1",
            "program": {"rate_percent": 20, "paid_months": 12, "hold_days": 30, "minimum_sats": 10000, "terms_version": "affiliate-2026-10-15"},
            "affiliates": [{
                "referrer": "ref_0123456789abcdef0123456789abcdef",
                "label": "Ada's channel",
                "keys": ["aff-0123456789abcdef"],
                "paid_conversions": 3, "refunds": 1,
                "accrued_micros": 9_456_000, "held_micros": 3_152_000,
                "payable_micros": 3_152_000, "paid_micros": 0,
                "clawed_back_micros": 3_152_000, "owed_micros": 0
            }]
        });
        if admin {
            value["sats_rate"] = json!({"usd_per_btc": 100_000, "as_of": "2026-10-15"});
            value["payouts_due"] = json!([{"referrer": "ref_0123456789abcdef0123456789abcdef", "label": "Ada's channel", "usd_micros": 12_000_000, "sats": 12_000}]);
        }
        Report::parse(value).unwrap()
    }

    #[test]
    fn dollars_round_down_to_cents() {
        assert_eq!(dollars(0), "$0.00");
        assert_eq!(dollars(3_152_999), "$3.15");
        assert_eq!(dollars(1_234_567_890_000), "$1,234,567.89");
    }

    #[test]
    fn the_admin_commission_section_lists_balances_and_payouts_due() {
        let html = admin_section(Some(&fixture(true))).into_string();
        crate::copy_guard::assert_plain(super::super::SCOREBOARD, &html);
        assert!(html.contains("Commission"));
        assert!(html.contains("Ada&#39;s channel") || html.contains("Ada's channel"));
        assert!(html.contains("$9.45"));
        assert!(html.contains("Payouts due"));
        assert!(html.contains("12,000"));
        assert!(html.contains("$12.00"));
        assert!(
            html.contains("needs the owner&#39;s approval")
                || html.contains("needs the owner's approval")
        );
        assert!(html.contains("Nothing is sent from this page."));
        // No price: no sats worked out.
        let mut no_rate = fixture(true);
        no_rate.sats_rate = None;
        let html = admin_section(Some(&no_rate)).into_string();
        assert!(html.contains("No bitcoin price is set"));
        // Unreadable, or opened with the key.
        let html = admin_section(None).into_string();
        crate::copy_guard::assert_plain(super::super::SCOREBOARD, &html);
        assert!(html.contains("Sign in as a site admin"));
    }

    #[test]
    fn the_settings_section_shows_an_affiliate_their_own_numbers() {
        let mut days = BTreeMap::new();
        let mut day = Day::default();
        let click = |key: &str, credited: bool| Click {
            id: format!("clk_{}", "a".repeat(32)),
            at: 1,
            link: Link::Affiliate,
            key: key.into(),
            series: None,
            destination: "/".into(),
            traffic: "human".into(),
            credited,
        };
        day.clicks.push(click("aff-0123456789abcdef", true));
        day.clicks.push(click("aff-0123456789abcdef", true));
        day.clicks.push(click("aff-0123456789abcdef", false));
        day.clicks.push(click("aff-ffffffffffffffff", true));
        day.signups.push(Signup {
            at: 1,
            click: format!("clk_{}", "a".repeat(32)),
            link: Link::Affiliate,
            key: "aff-0123456789abcdef".into(),
            series: None,
        });
        days.insert(1, day);
        let report = fixture(false);
        assert_eq!(counts(&report.affiliates[0].keys, &days), (2, 1));
        let html = settings_section(&report, &days).into_string();
        crate::copy_guard::assert_plain(crate::settings::PAGE, &html);
        assert!(html.contains("Your affiliate earnings"));
        assert!(html.contains("aff-0123456789abcdef"));
        assert!(html.contains("clicks: 2, sign-ups: 1"));
        assert!(html.contains("Paid sales: 3"));
        assert!(html.contains("Waiting: $3.15"));
        assert!(html.contains("10,000 sats"));
        // The terms page isn't linked from anywhere yet (owner, 2026-10-15).
        assert!(!html.contains("href=\"/affiliates\""));
        // No affiliate: no section.
        let none = Report {
            affiliates: Vec::new(),
            ..report
        };
        assert!(settings_section(&none, &days).into_string().is_empty());
    }
}
