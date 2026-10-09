//! `/admin/analytics`: the owner's view of the counts.
//!
//! Open to a signed-in site admin (the account service's `admin`, from the
//! `invite_only` entries marked `admin`, docs/auth/github.md); the account
//! menu links it for them. Scripts can send the dashboard key instead
//! (`OPENAGENTS_WEB_ANALYTICS_KEY`, a Secret Manager secret) as
//! `Authorization: Bearer KEY`. Anyone else, signed out, signed in without
//! admin, or with a wrong key, gets the site's plain `404`: the page
//! doesn't say it exists. Nothing is set in the browser.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use maud::{Markup, html};
use openagents_ui::content::{Facts, MarkdownRoot, PageColumn, Table};

use super::{Counts, DASHBOARD, Series, hour_now};
use crate::App;
use crate::ui_page::UiPage;

/// The windows every table shows, in days.
const WINDOWS: [(u64, &str); 3] = [(1, "Today"), (7, "7 days"), (30, "30 days")];

pub(super) async fn show(State(app): State<App>, headers: HeaderMap) -> Response {
    if !allowed(&app, &headers).await {
        return guarded(crate::not_found().await);
    }
    let analytics = &app.config.analytics;
    let content = match analytics.load(30).await {
        Ok(counts) => report(&counts, hour_now(), analytics.has_store()),
        Err(_) => html! {
            (MarkdownRoot::new(html! { h1 { "Analytics" } p { "The counts couldn't be read. Try again in a minute." } }))
        },
    };
    guarded(page(&headers, StatusCode::OK, content))
}

/// A bearer dashboard key that matches, or a signed-in site admin. A
/// wrong key waits a moment before the same `404` as everyone else.
async fn allowed(app: &App, headers: &HeaderMap) -> bool {
    let offered = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.trim().is_empty());
    if let Some(offered) = offered {
        if app.config.analytics.admits(offered) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        return false;
    }
    let Some(service) = app.config.cloud.as_deref() else {
        return false;
    };
    service
        .authenticate(headers)
        .await
        .is_ok_and(|viewer| viewer.admin)
}

fn page(headers: &HeaderMap, status: StatusCode, content: Markup) -> Response {
    UiPage::new("Analytics")
        .path(DASHBOARD)
        .scriptless()
        .status(status)
        .content(PageColumn::new(content).wide())
        .respond(headers)
}

/// Never cached, indexed, or sent on as a referrer.
fn guarded(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

/// Sums of the counts, per window.
struct Window<'a> {
    counts: &'a Counts,
    today: u64,
}

impl Window<'_> {
    /// The sum of the counts matching `keep`, for the last `days` days.
    fn sum(&self, days: u64, keep: impl Fn(Series, &str, &str) -> bool) -> u64 {
        let first = self.today.saturating_sub(days - 1);
        self.counts
            .iter()
            .filter(|(key, _)| key.hour / 24 >= first && keep(key.series, &key.name, &key.detail))
            .map(|(_, count)| *count)
            .sum()
    }

    /// Each distinct `(name, detail)` of `series` that `keep` passes, with its
    /// sums per window, largest 30-day count first.
    fn table(
        &self,
        series: Series,
        group: impl Fn(&str, &str) -> Option<String>,
    ) -> Vec<(String, [u64; 3])> {
        let mut by: BTreeMap<String, [u64; 3]> = BTreeMap::new();
        for (key, count) in self.counts {
            if key.series != series {
                continue;
            }
            let Some(label) = group(&key.name, &key.detail) else {
                continue;
            };
            let sums = by.entry(label).or_default();
            for (index, (days, _)) in WINDOWS.iter().enumerate() {
                if key.hour / 24 >= self.today.saturating_sub(days - 1) {
                    sums[index] += count;
                }
            }
        }
        let mut rows: Vec<_> = by.into_iter().collect();
        rows.sort_by(|a, b| b.1[2].cmp(&a.1[2]).then(a.0.cmp(&b.0)));
        rows
    }

    /// Per day (oldest first), the sum of the counts `keep` passes.
    fn daily(&self, days: u64, keep: impl Fn(Series, &str, &str) -> bool) -> Vec<(u64, u64)> {
        let first = self.today.saturating_sub(days - 1);
        let mut out: Vec<(u64, u64)> = (first..=self.today).map(|d| (d, 0)).collect();
        for (key, count) in self.counts {
            let day = key.hour / 24;
            if day >= first && keep(key.series, &key.name, &key.detail) {
                out[(day - first) as usize].1 += count;
            }
        }
        out
    }

    /// Today's 24 hours, the sum of the counts `keep` passes.
    fn hourly(&self, keep: impl Fn(Series, &str, &str) -> bool) -> Vec<(u64, u64)> {
        let first = self.today * 24;
        let mut out: Vec<(u64, u64)> = (first..first + 24).map(|h| (h, 0)).collect();
        for (key, count) in self.counts {
            if key.hour >= first
                && key.hour < first + 24
                && keep(key.series, &key.name, &key.detail)
            {
                out[(key.hour - first) as usize].1 += count;
            }
        }
        out
    }
}

fn number(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn windowed_table(label: &str, first: &str, rows: &[(String, [u64; 3])], limit: usize) -> Markup {
    if rows.is_empty() {
        return html! { p { "Nothing counted yet." } };
    }
    let mut table = Table::new()
        .label(label)
        .header([first, WINDOWS[0].1, WINDOWS[1].1, WINDOWS[2].1])
        .numeric(1)
        .numeric(2)
        .numeric(3);
    for (name, sums) in rows.iter().take(limit) {
        table = table.row([
            html! { code { (name) } },
            html! { (number(sums[0])) },
            html! { (number(sums[1])) },
            html! { (number(sums[2])) },
        ]);
    }
    html! { (table) }
}

/// A bar per point, as inline SVG (no script, no inline style), each
/// bar's value in its title.
fn bars(caption: &str, points: &[(String, u64)]) -> Markup {
    let max = points.iter().map(|p| p.1).max().unwrap_or(0);
    let total: u64 = points.iter().map(|p| p.1).sum();
    if max == 0 {
        return html! { figure.oa-chart { figcaption { (caption) ": nothing yet." } } };
    }
    let (width, height) = (points.len() * 10, 60u64);
    html! {
        figure.oa-chart {
            svg viewBox=(format!("0 0 {width} {height}")) preserveAspectRatio="none" role="img" aria-label=(caption) {
                @for (index, (label, value)) in points.iter().enumerate() {
                    @let bar = if *value == 0 { 0 } else { (value * height / max).max(1) };
                    g {
                        title { (label) ": " (number(*value)) }
                        rect.oa-chart-slot x=(index * 10) y="0" width="9" height=(height) {}
                        rect.oa-chart-bar x=(index * 10) y=(height - bar) width="9" height=(bar) {}
                    }
                }
            }
            figcaption { (caption) ": " (number(total)) " in all; the tallest bar is " (number(max)) "." }
        }
    }
}

/// The whole report for `counts`, as of `hour`.
pub(super) fn report(counts: &Counts, hour: u64, stored: bool) -> Markup {
    let w = Window {
        counts,
        today: hour / 24,
    };
    let views =
        |class: &'static str| move |s: Series, _: &str, d: &str| s == Series::View && d == class;
    let event =
        |name: &'static str| move |s: Series, n: &str, _: &str| s == Series::Event && n == name;
    let view_of = |route: &'static str| {
        move |s: Series, n: &str, d: &str| s == Series::View && n == route && d == "human"
    };
    let mut facts = Facts::new();
    for (days, label) in WINDOWS {
        facts = facts.fact(
            format!("Page views by people, {}", label.to_lowercase()),
            number(w.sum(days, views("human"))),
        );
    }
    facts = facts
        .fact("By agents, 30 days", number(w.sum(30, views("agent"))))
        .fact("By bots, 30 days", number(w.sum(30, views("bot"))));
    let day_points = |keep: &dyn Fn(Series, &str, &str) -> bool| -> Vec<(String, u64)> {
        w.daily(30, keep)
            .into_iter()
            .map(|(day, n)| (super::date(day), n))
            .collect()
    };
    let hour_points: Vec<(String, u64)> = w
        .hourly(views("human"))
        .into_iter()
        .map(|(h, n)| (format!("{:02}:00 UTC", h % 24), n))
        .collect();
    let traffic = w.table(Series::View, |_, d| Some(d.to_owned()));
    let routes = w.table(Series::View, |n, d| (d == "human").then(|| n.to_owned()));
    let agent_routes = w.table(Series::View, |n, d| (d == "agent").then(|| n.to_owned()));
    let referrers = w.table(Series::Referrer, |n, _| Some(n.to_owned()));
    let devices = w.table(Series::Device, |n, _| Some(n.to_owned()));
    let events = w.table(Series::Event, |n, d| {
        Some(if d.is_empty() {
            n.to_owned()
        } else {
            format!("{n} {d}")
        })
    });
    let statuses = w.table(Series::Status, |n, d| {
        (d.starts_with('4') || d.starts_with('5')).then(|| format!("{d} {n}"))
    });
    let latency = w.table(Series::Latency, |_, d| Some(d.to_owned()));
    let slow = w.table(Series::Latency, |n, d| {
        matches!(d, "1-3s" | ">3s").then(|| format!("{d} {n}"))
    });
    let funnel = |steps: &[(&str, u64, u64)]| {
        let mut table = Table::new()
            .label("Steps")
            .header(["Step", "7 days", "30 days"])
            .numeric(1)
            .numeric(2);
        for (name, week, month) in steps {
            table = table.row([
                html! { (name) },
                html! { (number(*week)) },
                html! { (number(*month)) },
            ]);
        }
        table
    };
    let chat_funnel = funnel(&[
        (
            "Home page views",
            w.sum(7, view_of("/")),
            w.sum(30, view_of("/")),
        ),
        (
            "Chats sent",
            w.sum(7, event("chat_sent")),
            w.sum(30, event("chat_sent")),
        ),
        (
            "Answers shown",
            w.sum(7, event("answer_shown")),
            w.sum(30, event("answer_shown")),
        ),
    ]);
    let download_funnel = funnel(&[
        (
            "Download page views",
            w.sum(7, view_of("/download")),
            w.sum(30, view_of("/download")),
        ),
        (
            "Downloads clicked",
            w.sum(7, event("download_clicked")),
            w.sum(30, event("download_clicked")),
        ),
        (
            "Install command copied",
            w.sum(7, event("install_copied")),
            w.sum(30, event("install_copied")),
        ),
        (
            "Installer fetched",
            w.sum(7, event("installer_fetched")),
            w.sum(30, event("installer_fetched")),
        ),
    ]);
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Analytics" }
            p {
                "Counts only: no cookies, no IP addresses, no message text, no accounts. "
                "Times are UTC. "
                @if stored { "Updated every minute." } @else { "This server keeps counts in memory only." }
            }
        }))
        (facts)
        h2 { "People, last 30 days" }
        (bars("Page views by people per day", &day_points(&views("human"))))
        h2 { "People, today by hour" }
        (bars("Page views by people per hour", &hour_points))
        h2 { "Agents, last 30 days" }
        (bars("Page views by agents per day", &day_points(&views("agent"))))
        h2 { "Who" }
        (windowed_table("Page views by kind", "Kind", &traffic, 10))
        h2 { "Top pages for people" }
        (windowed_table("Top pages", "Page", &routes, 25))
        h2 { "Top pages for agents" }
        (windowed_table("Top pages for agents", "Page", &agent_routes, 15))
        h2 { "Where people came from" }
        (windowed_table("Referring sites", "Site", &referrers, 25))
        h2 { "Devices" }
        (windowed_table("Devices", "Device", &devices, 5))
        h2 { "Chat" }
        (chat_funnel)
        h2 { "Download" }
        (download_funnel)
        h2 { "Events" }
        (windowed_table("Events", "Event", &events, 40))
        h2 { "Errors" }
        (windowed_table("Error responses", "Status and page", &statuses, 25))
        h2 { "Speed" }
        (windowed_table("Time to first byte", "Time", &latency, 10))
        h3 { "Slow pages" }
        (windowed_table("Slow responses", "Time and page", &slow, 15))
    }
}
