//! `/admin/attribution`: clicks and sign-ups per campaign code, affiliate
//! and series, per week. Open to the same people as `/admin/analytics`
//! (a signed-in site admin, or the dashboard key); anyone else gets the
//! site's plain `404`.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use maud::{Markup, html};
use openagents_ui::content::{MarkdownRoot, PageColumn, Table};

use super::{Counted, Day, Link, SCOREBOARD};
use crate::App;
use crate::analytics::dashboard::{allowed, guarded, number};
use crate::ui_page::UiPage;

/// How many weeks the scoreboard shows.
pub const WEEKS: u64 = 8;

pub(super) async fn show(State(app): State<App>, headers: HeaderMap) -> Response {
    if !allowed(&app, &headers).await {
        return guarded(crate::not_found().await);
    }
    let attribution = &app.config.attribution;
    let content = match attribution.load(WEEKS * 7).await {
        Ok(days) => report(&days),
        Err(_) => html! {
            (MarkdownRoot::new(html! { h1 { "Links" } p { "The numbers couldn't be read. Try again in a minute." } }))
        },
    };
    guarded(page(&headers, content))
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
