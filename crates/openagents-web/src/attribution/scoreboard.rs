//! `/admin/attribution`: clicks and sign-ups per campaign code, affiliate
//! and series, per week. Open to the same people as `/admin/analytics`
//! (a signed-in site admin, or the dashboard key); anyone else gets the
//! site's plain `404`.

use std::collections::BTreeMap;

use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use maud::{Markup, html};
use openagents_ui::content::{MarkdownRoot, PageColumn, Table};
use serde_json::json;

use super::{Counted, Day, Link, SCOREBOARD};
use crate::App;
use crate::analytics::dashboard::{allowed, guarded, number};
use crate::ui_page::UiPage;

/// How many weeks the scoreboard shows.
pub const WEEKS: u64 = 8;

pub(super) async fn show(
    State(app): State<App>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    if !allowed(&app, &headers).await {
        return guarded(crate::not_found().await);
    }
    if let Some(page) =
        crate::pages::gpui_chat::page_for(&app, &headers, query.as_deref(), "admin-attribution")
    {
        return guarded(page);
    }
    let attribution = &app.config.attribution;
    let content = match attribution.load(WEEKS * 7).await {
        Ok(days) => report(&days),
        Err(_) => html! {
            (MarkdownRoot::new(html! { h1 { "Links" } p { "The numbers couldn't be read. Try again in a minute." } }))
        },
    };
    let commission = commission_report(&app, &headers).await;
    let content = html! {
        (content)
        (super::commission::admin_section(commission.as_ref()))
    };
    guarded(page(&headers, content))
}

/// Every affiliate's commission, asked of the account service under the
/// admin's sign-in. `None` with the dashboard key (no sign-in to ask
/// with) or when it can't be read.
async fn commission_report(app: &App, headers: &HeaderMap) -> Option<super::commission::Report> {
    if headers.contains_key(axum::http::header::AUTHORIZATION) {
        return None;
    }
    let service = app.config.cloud.as_deref()?;
    super::commission::Report::parse(service.affiliates(headers, true).await?)
}

fn page(headers: &HeaderMap, content: Markup) -> Response {
    UiPage::new("Links")
        .path(SCOREBOARD)
        .scriptless()
        .status(StatusCode::OK)
        .content(PageColumn::new(content).wide())
        .respond(headers)
}

/// The Monday a day (days since 1970) falls in.
pub fn week_of(day: u64) -> u64 {
    // 1970-01-01 was a Thursday.
    day.saturating_sub((day + 3) % 7)
}

/// One scoreboard row's numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    /// Clicks by people, remembered for credit.
    pub clicks: u64,
    /// Clicks by agents and crawlers: recorded, never credited.
    pub machines: u64,
    pub signups: u64,
}

/// A row's key: the week, the kind of link, the code or affiliate, and
/// the series.
pub type Row = (u64, Link, String, String);

/// Every row's numbers, newest week first when iterated in reverse.
pub fn scores(days: &BTreeMap<u64, Day>) -> BTreeMap<Row, Score> {
    let mut out: BTreeMap<Row, Score> = BTreeMap::new();
    let series = |s: &Option<String>| s.clone().unwrap_or_default();
    for (day, records) in days {
        let week = week_of(*day);
        for click in &records.clicks {
            let score = out
                .entry((week, click.link, click.key.clone(), series(&click.series)))
                .or_default();
            if click.credited {
                score.clicks += 1;
            } else {
                score.machines += 1;
            }
        }
        for signup in &records.signups {
            out.entry((
                week_of(signup.at / 86_400),
                signup.link,
                signup.key.clone(),
                series(&signup.series),
            ))
            .or_default()
            .signups += 1;
        }
    }
    out
}

/// Clicks counted without a record, per week and reason.
pub fn totals(days: &BTreeMap<u64, Day>) -> BTreeMap<(u64, Counted), u64> {
    let mut out = BTreeMap::new();
    for (day, records) in days {
        for total in &records.totals {
            *out.entry((week_of(*day), total.counted)).or_default() += total.count;
        }
    }
    out
}

fn rate(signups: u64, clicks: u64) -> String {
    if clicks == 0 {
        "-".into()
    } else {
        format!("{:.1}%", signups as f64 * 100.0 / clicks as f64)
    }
}

pub(crate) fn report(days: &BTreeMap<u64, Day>) -> Markup {
    let scores = scores(days);
    let totals = totals(days);
    let mut table = Table::new()
        .label("Clicks and sign-ups per link, per week")
        .header([
            "Week of",
            "Link",
            "Code or affiliate",
            "Series",
            "Clicks",
            "Sign-ups",
            "Sign-ups per click",
            "Agents and bots",
        ])
        .numeric(4)
        .numeric(5)
        .numeric(6)
        .numeric(7);
    for ((week, link, key, series), score) in scores.iter().rev() {
        table = table.row([
            html! { (crate::analytics::date(*week)) },
            html! { (match link { Link::Go => "Our post", Link::Affiliate => "Affiliate" }) },
            html! { code { (key) } },
            html! { (series) },
            html! { (number(score.clicks)) },
            html! { (number(score.signups)) },
            html! { (rate(score.signups, score.clicks)) },
            html! { (number(score.machines)) },
        ]);
    }
    let mut counted = Table::new()
        .label("Clicks we only counted")
        .header(["Week of", "Why", "Clicks"])
        .numeric(2);
    for ((week, why), count) in totals.iter().rev() {
        counted = counted.row([
            html! { (crate::analytics::date(*week)) },
            html! { (why.name()) },
            html! { (number(*count)) },
        ]);
    }
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Links" }
            p {
                "Who clicked our " code { "/go/" } " links and our affiliates' " code { "/r/" }
                " links, and how many of them signed up, for the last " (WEEKS) " weeks. "
                "Clicks are people's clicks we may credit; agents and crawlers are counted apart and never credited. "
                "Affiliates are shown by a short code made from their link."
            }
            h2 { "Per link" }
        }))
        @if scores.is_empty() {
            p { "No clicks yet." }
        } @else {
            (table)
        }
        (MarkdownRoot::new(html! {
            h2 { "Counted only" }
            p { "Clicks from browsers that asked not to be tracked, people who said no, unknown codes, and previews. No record is kept of these, only the number." }
        }))
        @if totals.is_empty() {
            p { "Nothing counted yet." }
        } @else {
            (counted)
        }
    }
}

/// The same aggregate weekly report, for the GPUI app. Click records,
/// account ids, and browser tokens never leave the server.
pub(super) async fn api(State(app): State<App>, headers: HeaderMap) -> Response {
    if !crate::analytics::dashboard::api_allowed(&app, &headers).await {
        return guarded(crate::coder_sync::refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "Not found.",
        ));
    }
    let days = match app.config.attribution.load(WEEKS * 7).await {
        Ok(days) => days,
        Err(_) => {
            return guarded(crate::coder_sync::refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "The numbers couldn't be read. Try again in a minute.",
            ));
        }
    };
    let mut sections = vec![
        json!({"title":"Per link", "rows": scores(&days).iter().rev().map(|((week,link,key,series),s)| json!({"label":format!("{} · {} · {key} {series}", crate::analytics::date(*week), match link { Link::Go => "Our post", Link::Affiliate => "Affiliate" }), "detail":format!("Clicks: {} · Sign-ups: {} · Sign-ups per click: {} · Agents and bots: {}", number(s.clicks), number(s.signups), rate(s.signups,s.clicks), number(s.machines))})).collect::<Vec<_>>()}),
        json!({"title":"Counted only", "rows":totals(&days).iter().rev().map(|((week,why),n)| json!({"label":format!("{} · {}",crate::analytics::date(*week),why.name()),"detail":number(*n)})).collect::<Vec<_>>()}),
    ];
    // The affiliate controls and figures obey the feature registry too.
    if app.config.features.on(crate::features::Feature::Affiliates) {
        // The bridge keeps the session cookie; use it only after the bearer
        // has passed the site-admin check above.
        if let Some(report) = app
            .config
            .cloud
            .as_deref()
            .map(|s| s.affiliates(&headers, true))
        {
            if let Some(report) = report.await.and_then(super::commission::Report::parse) {
                use super::commission::dollars;
                sections.push(json!({"title":"Commission", "rows":report.affiliates.iter().map(|a| json!({"label":if a.label.is_empty() { &a.referrer } else { &a.label },"detail":format!("Paid sales: {} · Refunds: {} · Earned: {} · Waiting: {} · Ready to pay: {} · Paid: {} · Taken back: {}",number(a.paid_conversions),number(a.refunds),dollars(a.accrued_micros),dollars(a.held_micros),dollars(a.payable_micros),dollars(a.paid_micros),dollars(a.clawed_back_micros))})).collect::<Vec<_>>()}));
                sections.push(json!({"title":"Payouts due", "rows":report.payouts_due.iter().map(|d| json!({"label":if d.label.is_empty() {&d.referrer} else {&d.label},"detail":format!("{} · {} sats",dollars(d.usd_micros),number(d.sats))})).collect::<Vec<_>>()}));
            } else {
                sections.push(json!({"title":"Commission", "rows":[{"label":"The numbers couldn't be read. Try again in a minute.", "detail":""}]}));
            }
        }
    }
    guarded(crate::coder_sync::answer(
        StatusCode::OK,
        json!({"description":"Clicks and sign-ups for the last 8 weeks. Agents and bots are counted separately.","sections": sections}),
    ))
}
