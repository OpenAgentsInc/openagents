//! `/stats`: the payment numbers as tables and small series charts
//! (#10196), the companion to `/live`.
//!
//! The page is drawn on the server from the pay host's public reads
//! (#10195): `/stats` for the totals, the per-plugin and per-author
//! breakdowns, the 24 hour and 30 day series, and the reconciliation state;
//! `/flow/snapshot` for the recent payouts and the time of the last event.
//! Both are the same public projection `/api/stats` and `/api/flow/*`
//! serve, so the page shows nothing a visitor couldn't read there: plugin
//! names, published authors, amounts, and times, never a payer. It runs no
//! script and works with JavaScript off. Nothing is made up: when the pay
//! host doesn't answer the page says so, and with no payments yet it says
//! that instead of drawing empty tables.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, Render, html};
use openagents_ui::actions::{Alert, Color};
use openagents_ui::content::{Facts, MarkdownRoot, PageColumn};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::App;
use crate::ui_page::{UiPage, action_link};

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/stats", get(stats))
}

/// How long the page waits for the pay host before saying it is
/// unreachable.
const WAIT: Duration = Duration::from_secs(3);

/// The most bytes read from one pay host answer.
const LIMIT: usize = 4 * 1024 * 1024;

/// The most recent payouts listed.
const RECENT: usize = 20;

/// An exact amount in millisatoshis, read from the wire's sats number
/// (`12` or `12.345`) without going through a float.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Msat(u64);

impl<'de> Deserialize<'de> for Msat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        Msat::parse(&number.to_string())
            .ok_or_else(|| serde::de::Error::custom("not a whole number of millisatoshis"))
    }
}

impl Msat {
    fn parse(text: &str) -> Option<Self> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        if whole.is_empty()
            || !whole.bytes().all(|b| b.is_ascii_digit())
            || fraction.len() > 3
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let fraction = format!("{fraction:0<3}").parse::<u64>().ok()?;
        whole
            .parse::<u64>()
            .ok()?
            .checked_mul(1000)?
            .checked_add(fraction)
            .map(Msat)
    }

    fn saturating_add(self, other: Self) -> Self {
        Msat(self.0.saturating_add(other.0))
    }

    /// `1,234 sats`, or `1,234.5 sats` with millisatoshis.
    fn sats(self) -> String {
        let whole = grouped(self.0 / 1000);
        let fraction = self.0 % 1000;
        let unit = if self.0 == 1000 { "sat" } else { "sats" };
        if fraction == 0 {
            format!("{whole} {unit}")
        } else {
            let fraction = format!("{fraction:03}");
            format!("{whole}.{} {unit}", fraction.trim_end_matches('0'))
        }
    }
}

/// `1234567` as `1,234,567`.
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Totals {
    received_sats: Msat,
    paid_out_sats: Msat,
    pending_accruals_sats: Msat,
    calls: u64,
    earnings_sats: Msat,
}

impl Totals {
    fn is_empty(&self) -> bool {
        self.calls == 0
            && self.received_sats == Msat::default()
            && self.paid_out_sats == Msat::default()
            && self.earnings_sats == Msat::default()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SeriesPoint {
    at: i64,
    #[serde(default)]
    totals: Totals,
}

/// The pay host's `/stats`.
#[derive(Debug, Deserialize)]
pub(crate) struct Stats {
    #[serde(default)]
    totals: Totals,
    #[serde(default)]
    per_plugin: BTreeMap<String, Totals>,
    #[serde(default)]
    per_author: BTreeMap<String, Totals>,
    #[serde(default)]
    series_24h: Vec<SeriesPoint>,
    #[serde(default)]
    series_30d: Vec<SeriesPoint>,
    #[serde(default)]
    reconciliation: String,
}

/// One public flow event, the fields this page shows.
#[derive(Debug, Deserialize)]
pub(crate) struct Event {
    at: i64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    plugin: Option<String>,
    #[serde(default)]
    amount_sats: Option<Msat>,
    #[serde(default)]
    split: BTreeMap<String, Msat>,
    #[serde(default)]
    author: Option<String>,
}

impl Event {
    /// What a payout sent to its author: the author's, resource's, and
    /// bonus parts of the split, or the whole amount without a split.
    fn paid_to_author(&self) -> Msat {
        if self.split.is_empty() {
            return self.amount_sats.unwrap_or_default();
        }
        self.split
            .iter()
            .filter(|(role, _)| matches!(role.as_str(), "author" | "resource" | "bonus"))
            .fold(Msat::default(), |sum, (_, amount)| {
                sum.saturating_add(*amount)
            })
    }
}

/// The pay host's `/flow/snapshot`, the part this page reads.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Snapshot {
    #[serde(default)]
    events: Vec<Event>,
}

/// Reads `path` from the pay host as JSON; `None` when there is no pay
/// host or it doesn't answer in time with a `200` and well-formed JSON.
async fn read<T: DeserializeOwned>(app: &App, path: &str) -> Option<T> {
    let upstream = app.config.pay_upstream.as_ref()?;
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .ok()?;
    let response = tokio::time::timeout(WAIT, upstream.forward(request))
        .await
        .ok()?;
    if response.status() != StatusCode::OK {
        return None;
    }
    let body = tokio::time::timeout(WAIT, to_bytes(response.into_body(), LIMIT))
        .await
        .ok()?
        .ok()?;
    serde_json::from_slice(&body).ok()
}

async fn stats(State(app): State<App>, headers: HeaderMap) -> Response {
    let (stats, snapshot) = tokio::join!(
        read::<Stats>(&app, "/stats"),
        read::<Snapshot>(&app, "/flow/snapshot")
    );
    let mut response = UiPage::new("Stats")
        .path("/stats")
        .scriptless()
        .content(body(stats.as_ref(), snapshot.as_ref()))
        .respond(&headers);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// The page's markup from what the pay host answered.
pub(crate) fn body(stats: Option<&Stats>, snapshot: Option<&Snapshot>) -> Markup {
    let intro = html! {
        (MarkdownRoot::new(html! {
            h1 id="stats-title" { "Stats" }
            p.oa-page-lead {
                "What OpenAgents has received for plugin calls and paid out to plugin authors, \
    from the payment ledger. Every number is a settled payment; nothing is estimated."
            }
        }))
        div.oa-page-actions { (action_link("Watch it live", "/live")) }
    };
    PageColumn::new(html! {
        section aria-labelledby="stats-title" {
            (intro)
            (numbers(stats, snapshot))
        }
    })
    .wide()
    .render()
}

/// Everything under the intro: a notice, or the totals, tables, and series.
fn numbers(stats: Option<&Stats>, snapshot: Option<&Snapshot>) -> Markup {
    let Some(stats) = stats else {
        return Alert::new()
            .id("stats-unreachable")
            .color(Color::Warning)
            .description(
                "The payment statistics are unreachable right now, so there are no numbers \
to show. Try again shortly.",
            )
            .render();
    };
    let events = snapshot.map_or(&[][..], |snapshot| &snapshot.events[..]);
    let last = events.iter().map(|event| event.at).max();
    if stats.totals.is_empty() && stats.per_plugin.is_empty() && events.is_empty() {
        return html! {
            (Alert::new()
                .id("stats-empty")
                .description(
                    "No payments yet. The first paid plugin call will show here and on the \
        live map.",
                ))
            (MarkdownRoot::new(footing(&stats.reconciliation, last)))
        };
    }
    let totals = &stats.totals;
    let facts = Facts::new()
        .id("stats-totals")
        .fact("Received", totals.received_sats.sats())
        .fact("Paid out", totals.paid_out_sats.sats())
        .fact("Pending", totals.pending_accruals_sats.sats())
        .fact("Calls", grouped(totals.calls))
        .fact("Author earnings", totals.earnings_sats.sats());

    let mut plugins: Vec<_> = stats.per_plugin.iter().collect();
    plugins.sort_by(|a, b| {
        (b.1.earnings_sats, b.1.calls)
            .cmp(&(a.1.earnings_sats, a.1.calls))
            .then(a.0.cmp(b.0))
    });
    let mut authors: Vec<_> = stats.per_author.iter().collect();
    authors.sort_by(|a, b| {
        b.1.paid_out_sats
            .cmp(&a.1.paid_out_sats)
            .then(b.1.earnings_sats.cmp(&a.1.earnings_sats))
            .then(a.0.cmp(b.0))
    });
    let mut payouts: Vec<&Event> = events
        .iter()
        .filter(|event| event.kind == "payout" && event.paid_to_author() != Msat::default())
        .collect();
    payouts.sort_by_key(|event| std::cmp::Reverse(event.at));

    html! {
        (facts)
        (MarkdownRoot::new(html! {
            (footing(&stats.reconciliation, last))
            h2 { "Plugins" }
            @if plugins.is_empty() {
                p.oa-page-meta { "No plugin has been called for pay yet." }
            } @else {
                table id="stats-plugins" {
                    thead { tr { th { "Plugin" } th { "Calls" } th { "Earned" } th { "Paid out" } } }
                    tbody {
                        @for (name, totals) in &plugins {
                            tr {
                                td { (name) }
                                td { (grouped(totals.calls)) }
                                td { (totals.earnings_sats.sats()) }
                                td { (totals.paid_out_sats.sats()) }
                            }
                        }
                    }
                }
            }
            h2 { "Authors" }
            @if authors.is_empty() {
                p.oa-page-meta { "No author has earned yet." }
            } @else {
                table id="stats-authors" {
                    thead { tr { th { "Author" } th { "Earned" } th { "Paid out" } th { "Pending" } } }
                    tbody {
                        @for (name, totals) in &authors {
                            tr {
                                td { (name) }
                                td { (totals.earnings_sats.sats()) }
                                td { (totals.paid_out_sats.sats()) }
                                td { (totals.pending_accruals_sats.sats()) }
                            }
                        }
                    }
                }
            }
            h2 { "Recent payouts" }
            @if payouts.is_empty() {
                p.oa-page-meta { "No payouts yet." }
            } @else {
                table id="stats-payouts" {
                    thead { tr { th { "When (UTC)" } th { "Plugin" } th { "Author" } th { "Amount" } } }
                    tbody {
                        @for event in payouts.iter().take(RECENT) {
                            tr {
                                td { (utc(event.at)) }
                                td { (event.plugin.as_deref().unwrap_or("\u{2014}")) }
                                td { (event.author.as_deref().unwrap_or("\u{2014}")) }
                                td { (event.paid_to_author().sats()) }
                            }
                        }
                    }
                }
            }
            h2 { "Received over time" }
            (series("stats-24h", "The last 24 hours, by hour", &stats.series_24h, false))
            (series("stats-30d", "The last 30 days, by day", &stats.series_30d, true))
        }))
    }
}

/// The reconciliation state and the time of the last event.
fn footing(reconciliation: &str, last: Option<i64>) -> Markup {
    let state = match reconciliation {
        "ok" => "the ledger matches the wallet",
        "drift" => "the ledger and the wallet disagree; payouts are being checked",
        _ => "not checked yet",
    };
    let last = last.map_or_else(|| "none yet".to_owned(), |at| format!("{} UTC", utc(at)));
    html! {
        p.oa-page-meta id="stats-footing" { "Reconciliation: " (state) ". Last event: " (last) "." }
    }
}

/// A bar per bucket of received sats, as an inline SVG (no script, no
/// inline style), with each bar's exact value in its title.
fn series(id: &str, caption: &str, points: &[SeriesPoint], daily: bool) -> Markup {
    let received: u64 = points
        .iter()
        .fold(Msat::default(), |sum, p| {
            sum.saturating_add(p.totals.received_sats)
        })
        .0;
    let calls: u64 = points.iter().map(|p| p.totals.calls).sum();
    if points.is_empty() || (received == 0 && calls == 0) {
        return html! {
            figure.oa-chart id=(id) { figcaption { (caption) ": nothing received." } }
        };
    }
    let max = points
        .iter()
        .map(|p| p.totals.received_sats.0)
        .max()
        .unwrap_or(0)
        .max(1);
    let (width, height) = (points.len() * 10, 60);
    html! {
        figure.oa-chart id=(id) {
            svg viewBox=(format!("0 0 {width} {height}")) preserveAspectRatio="none" role="img"
                aria-label=(caption) {
                @for (index, point) in points.iter().enumerate() {
                    @let value = point.totals.received_sats.0;
                    // At least a hairline for a bucket with calls but no
                    // sats, so activity is visible without inventing an
                    // amount.
                    @let bar = if value == 0 {
                        0
                    } else {
                        ((u128::from(value) * height as u128) / u128::from(max)).max(1) as usize
                    };
                    @let when = if daily {
                        utc(point.at)[..10].to_owned()
                    } else {
                        format!("{} UTC", utc(point.at))
                    };
                    g {
                        title {
                            (when) ": " (point.totals.received_sats.sats()) ", "
                            (grouped(point.totals.calls)) " calls"
                        }
                        rect.oa-chart-slot x=(index * 10) y="0" width="9" height=(height) {}
                        rect.oa-chart-bar x=(index * 10) y=(height - bar) width="9" height=(bar) {}
                    }
                }
            }
            figcaption {
                (caption) ": " (Msat(received).sats()) " received over " (grouped(calls))
                " calls; the tallest bar is " (Msat(max).sats()) "."
            }
        }
    }
}

/// Milliseconds since the epoch as `YYYY-MM-DD HH:MM`, in UTC.
pub(crate) fn utc(at: i64) -> String {
    let seconds = at.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        of_day / 3600,
        of_day % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_are_exact_and_grouped() {
        assert_eq!(Msat::parse("12").unwrap().sats(), "12 sats");
        assert_eq!(Msat::parse("1").unwrap().sats(), "1 sat");
        assert_eq!(Msat::parse("1234567.5").unwrap().sats(), "1,234,567.5 sats");
        assert_eq!(Msat::parse("0.001").unwrap().sats(), "0.001 sats");
        assert!(Msat::parse("1.0001").is_none());
        assert!(Msat::parse("-1").is_none());
        assert!(Msat::parse("1e3").is_none());
    }

    #[test]
    fn times_are_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00");
        assert_eq!(utc(1_790_000_000_000), "2026-09-21 14:13");
        assert_eq!(utc(951_782_400_000), "2000-02-29 00:00");
    }
}
