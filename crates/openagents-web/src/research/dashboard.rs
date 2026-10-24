//! `/admin/research`: the launch funnel, drop-off, time per step, the
//! funnel by campaign and device, recent sessions, one session's timeline,
//! and `/admin/research.csv`. Behind the analytics dashboard's gate (a
//! signed-in site admin or the dashboard key); anyone else gets the site's
//! plain `404`.

use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use maud::{Markup, html};
use openagents_ui::content::{Facts, MarkdownRoot, PageColumn, Table};

use super::funnel::{self, Funnel, Session, Stage};
use super::{Count, DASHBOARD, EXPORT, Event};
use crate::App;
use crate::analytics::dashboard::{allowed, guarded, number};
use crate::ui_page::UiPage;

/// The windows a view may ask for, in days.
pub const DAYS: [(u64, &str); 4] = [(1, "Today"), (2, "48 hours"), (7, "7 days"), (30, "30 days")];
/// The most sessions listed.
const MOST_LISTED: usize = 100;
/// The most events a timeline shows.
const MOST_TIMELINE: usize = 2_000;

/// What the address asks for.
#[derive(Debug, PartialEq, Eq)]
pub struct Asked {
    pub days: u64,
    pub session: Option<String>,
    /// For the CSV: `events` (default), `sessions`, or `funnel`.
    pub what: String,
}

impl Asked {
    pub fn from_query(query: Option<&str>) -> Self {
        let mut asked = Self {
            days: 2,
            session: None,
            what: "events".into(),
        };
        for (key, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
            match key.as_ref() {
                "days" => {
                    if let Some((days, _)) = DAYS.iter().find(|(d, _)| d.to_string() == value) {
                        asked.days = *days;
                    }
                }
                "session" if super::ingest::valid_session(&value) => {
                    asked.session = Some(value.into_owned());
                }
                "what" if matches!(value.as_ref(), "events" | "sessions" | "funnel") => {
                    asked.what = value.into_owned();
                }
                _ => {}
            }
        }
        asked
    }
}

pub(super) async fn show(
    State(app): State<App>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    if !allowed(&app, &headers).await {
        return guarded(crate::not_found().await);
    }
    let asked = Asked::from_query(query.as_deref());
    let research = &app.config.research;
    let days = if asked.session.is_some() {
        research.retention_days().min(30)
    } else {
        asked.days
    };
    let content = match research.load(days).await {
        Ok((events, counts)) => match &asked.session {
            Some(id) => timeline(&events, id),
            None => report(&events, &counts, &asked, research.has_store()),
        },
        Err(_) => html! {
            (MarkdownRoot::new(html! { h1 { "Playtest research" } p { "The events couldn't be read. Try again in a minute." } }))
        },
    };
    guarded(
        UiPage::new("Playtest research")
            .path(DASHBOARD)
            .scriptless()
            .status(StatusCode::OK)
            .content(PageColumn::new(content).wide())
            .respond(&headers),
    )
}

pub(super) async fn export(
    State(app): State<App>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    if !allowed(&app, &headers).await {
        return guarded(crate::not_found().await);
    }
    let asked = Asked::from_query(query.as_deref());
    let Ok((events, _)) = app.config.research.load(asked.days).await else {
        return guarded(
            (StatusCode::SERVICE_UNAVAILABLE, "The events couldn't be read.").into_response(),
        );
    };
    let body = match asked.what.as_str() {
        "sessions" => sessions_csv(&funnel::sessions(&events)),
        "funnel" => funnel_csv(&funnel::sessions(&events)),
        _ => events_csv(&events),
    };
    let mut response = (
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        body,
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&format!(
        "attachment; filename=\"research-{}-{}d.csv\"",
        asked.what, asked.days
    )) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    guarded(response)
}

/// One CSV field, quoted when it must be, and never read as a formula.
pub fn field(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

fn line(fields: &[&str]) -> String {
    let mut out = fields.iter().map(|f| field(f)).collect::<Vec<_>>().join(",");
    out.push('\n');
    out
}

pub fn events_csv(events: &[Event]) -> String {
    let mut out = line(&[
        "at", "session", "account", "kind", "path", "label", "props", "device", "source",
        "campaign",
    ]);
    for e in events {
        let props = serde_json::to_string(&e.props).unwrap_or_default();
        out.push_str(&line(&[
            &iso(e.at),
            &e.session,
            e.account.as_deref().unwrap_or_default(),
            &e.kind,
            &e.path,
            &e.label,
            if e.props.is_empty() { "" } else { &props },
            &e.device,
            &e.source,
            &e.campaign,
        ]));
    }
    out
}

pub fn sessions_csv(sessions: &[Session]) -> String {
    let mut header = vec![
        "session", "account", "source", "campaign", "device", "first", "last", "events",
        "furthest",
    ];
    header.extend(Stage::ALL.iter().map(|s| s.key()));
    let mut out = line(&header);
    for s in sessions {
        let reached: Vec<String> = Stage::ALL
            .iter()
            .map(|stage| s.reached.get(stage).map(|at| iso(*at)).unwrap_or_default())
            .collect();
        let mut row = vec![
            s.id.clone(),
            s.account.clone().unwrap_or_default(),
            s.source.clone(),
            s.campaign.clone(),
            s.device.clone(),
            iso(s.first),
            iso(s.last),
            s.events.to_string(),
            s.furthest().key().to_owned(),
        ];
        row.extend(reached);
        out.push_str(&line(&row.iter().map(String::as_str).collect::<Vec<_>>()));
    }
    out
}

pub fn funnel_csv(sessions: &[Session]) -> String {
    let mut header = vec!["group", "value"];
    header.extend(Stage::ALL.iter().map(|s| s.key()));
    let mut out = line(&header);
    let mut groups = vec![("all".to_owned(), "all".to_owned(), Funnel::of(sessions))];
    for (name, f) in funnel::by(sessions, |s| s.source.clone()) {
        groups.push(("source".into(), name, f));
    }
    for (name, f) in funnel::by(sessions, |s| s.campaign.clone()) {
        groups.push(("campaign".into(), name, f));
    }
    for (name, f) in funnel::by(sessions, |s| s.device.clone()) {
        groups.push(("device".into(), name, f));
    }
    for (group, name, f) in groups {
        let mut row = vec![group, name];
        row.extend(f.reached.iter().map(usize::to_string));
        out.push_str(&line(&row.iter().map(String::as_str).collect::<Vec<_>>()));
    }
    out
}

/// `2026-10-24T18:03:07Z`.
pub fn iso(ms: u64) -> String {
    let s = ms / 1000;
    let day = s / 86_400;
    let rest = s % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        crate::analytics::date(day),
        rest / 3600,
        (rest / 60) % 60,
        rest % 60
    )
}

fn percent(value: Option<f64>) -> String {
    value.map_or_else(|| "–".into(), |v| format!("{v:.0}%"))
}

fn windows(asked: &Asked) -> Markup {
    html! {
        p {
            @for (i, (days, label)) in DAYS.iter().enumerate() {
                @if i > 0 { " · " }
                @if *days == asked.days {
                    strong { (label) }
                } @else {
                    a href=(format!("{DASHBOARD}?days={days}")) { (label) }
                }
            }
            " · "
            a href=(format!("{DASHBOARD}?days={}", asked.days)) { "Refresh" }
        }
    }
}

/// The funnel table for one group.
fn funnel_table(label: &str, overall: &Funnel) -> Markup {
    let mut table = Table::new()
        .label(label)
        .header(["Step", "Sessions", "Of previous", "Drop-off", "Median time from previous"])
        .numeric(1)
        .numeric(2)
        .numeric(3)
        .numeric(4);
    for (i, stage) in Stage::ALL.iter().enumerate() {
        let (kept, drop, time) = if i == 0 {
            ("–".to_owned(), "–".to_owned(), "–".to_owned())
        } else {
            let from = overall.reached[i - 1];
            (
                if from == 0 {
                    "–".into()
                } else {
                    format!("{:.0}%", 100.0 * overall.reached[i] as f64 / from as f64)
                },
                percent(overall.drop_off(i - 1)),
                overall.median_ms[i - 1].map_or_else(|| "–".into(), funnel::duration),
            )
        };
        table = table.row([
            html! { (stage.name()) },
            html! { (number(overall.reached[i] as u64)) },
            html! { (kept) },
            html! { (drop) },
            html! { (time) },
        ]);
    }
    html! { (table) }
}

/// A table of the funnel's stage counts per group.
fn grouped(label: &str, first: &str, rows: &[(String, Funnel)]) -> Markup {
    if rows.is_empty() {
        return html! { p { "No sessions yet." } };
    }
    let mut header = vec![first];
    header.extend(Stage::ALL.iter().map(|s| s.name()));
    header.push("Completed");
    let mut table = Table::new().label(label).header(header);
    for i in 1..=Stage::ALL.len() + 1 {
        table = table.numeric(i);
    }
    for (name, f) in rows.iter().take(40) {
        let mut cells = vec![html! { code { (name) } }];
        cells.extend(f.reached.iter().map(|n| html! { (number(*n as u64)) }));
        let rate = (f.reached[0] > 0)
            .then(|| 100.0 * f.reached[5] as f64 / f.reached[0] as f64);
        cells.push(html! { (percent(rate)) });
        table = table.row(cells);
    }
    html! { (table) }
}

/// The whole report.
pub fn report(events: &[Event], counts: &[Count], asked: &Asked, stored: bool) -> Markup {
    let sessions = funnel::sessions(events);
    let overall = Funnel::of(&sessions);
    let signed_in = sessions.iter().filter(|s| s.account.is_some()).count();
    let opted_out: u64 = counts.iter().map(|c| c.count).sum();
    let window = DAYS
        .iter()
        .find(|(d, _)| *d == asked.days)
        .map_or("", |(_, l)| l);
    let facts = Facts::new()
        .fact("Page views", number(funnel::impressions(events, counts)))
        .fact("Sessions", number(sessions.len() as u64))
        .fact("Signed in", number(signed_in as u64))
        .fact(
            "Surveys completed",
            number(overall.reached[5] as u64),
        )
        .fact("Opted-out events (counted only)", number(opted_out))
        .fact("Events", number(events.len() as u64));
    let mut recent: Vec<&Session> = sessions.iter().collect();
    recent.sort_by(|a, b| b.last.cmp(&a.last));
    let milestones = funnel::milestones(&sessions);
    let mut opted: std::collections::BTreeMap<(String, String), u64> = Default::default();
    for c in counts {
        *opted.entry((c.kind.clone(), c.path.clone())).or_default() += c.count;
    }
    let mut opted: Vec<_> = opted.into_iter().collect();
    opted.sort_by(|a, b| b.1.cmp(&a.1));
    let csv = |what: &str| format!("{EXPORT}?days={}&what={what}", asked.days);
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Playtest research" }
            p { "The launch funnel from the playtest research stream, " (window.to_lowercase()) ". "
                @if !stored { "Nothing is stored on this server; these are this process's events only." } }
        }))
        (windows(asked))
        (facts)
        h2 { "Funnel" }
        (funnel_table("The launch funnel", &overall))
        h2 { "Tutorial milestones" }
        (milestones_table(&milestones))
        h2 { "By source" }
        (grouped("Funnel by source", "Source", &funnel::by(&sessions, |s| s.source.clone())))
        h2 { "By campaign" }
        (grouped("Funnel by campaign", "Campaign", &funnel::by(&sessions, |s| s.campaign.clone())))
        h2 { "By device" }
        (grouped("Funnel by device", "Device", &funnel::by(&sessions, |s| s.device.clone())))
        h2 { "Recent sessions" }
        (recent_table(&recent))
        h2 { "Opted-out counts" }
        (opted_table(&opted))
        h2 { "Export" }
        p {
            a href=(csv("events")) { "Events (CSV)" } " · "
            a href=(csv("sessions")) { "Sessions (CSV)" } " · "
            a href=(csv("funnel")) { "Funnel by group (CSV)" }
        }
    }
}

fn milestones_table(milestones: &[(String, usize, Option<u64>)]) -> Markup {
    if milestones.is_empty() {
        return html! { p { "No tutorial milestones yet." } };
    }
    let mut table = Table::new()
        .label("Tutorial milestones")
        .header(["Milestone", "Sessions", "Median time from start"])
        .numeric(1)
        .numeric(2);
    for (name, n, time) in milestones {
        table = table.row([
            html! { code { (name) } },
            html! { (number(*n as u64)) },
            html! { (time.map_or_else(|| "–".into(), funnel::duration)) },
        ]);
    }
    html! { (table) }
}

fn recent_table(recent: &[&Session]) -> Markup {
    if recent.is_empty() {
        return html! { p { "No sessions yet." } };
    }
    let mut table = Table::new()
        .label("Recent sessions")
        .header(["Session", "Last seen", "Furthest step", "Source", "Device", "Events"])
        .numeric(5);
    for s in recent.iter().take(MOST_LISTED) {
        table = table.row([
            html! { a href=(format!("{DASHBOARD}?session={}", s.id)) { code { (s.id.chars().take(16).collect::<String>()) } } },
            html! { (iso(s.last)) },
            html! { (s.furthest().name()) },
            html! { (s.source) " / " (s.campaign) },
            html! { (s.device) },
            html! { (number(s.events as u64)) },
        ]);
    }
    html! { (table) }
}

fn opted_table(opted: &[((String, String), u64)]) -> Markup {
    if opted.is_empty() {
        return html! { p { "None." } };
    }
    let mut table = Table::new()
        .label("Opted-out counts")
        .header(["Kind", "Path", "Count"])
        .numeric(2);
    for ((kind, path), n) in opted.iter().take(40) {
        table = table.row([html! { (kind) }, html! { code { (path) } }, html! { (number(*n)) }]);
    }
    html! { (table) }
}

/// One session's events, in order.
pub fn timeline(events: &[Event], id: &str) -> Markup {
    let mine: Vec<&Event> = events.iter().filter(|e| e.session == id).collect();
    let back = html! { p { a href=(DASHBOARD) { "Back to the funnel" } } };
    if mine.is_empty() {
        return html! {
            (MarkdownRoot::new(html! { h1 { "Session" } p { "No events for this session." } }))
            (back)
        };
    }
    let session = funnel::sessions(&mine.iter().map(|e| (*e).clone()).collect::<Vec<_>>())
        .into_iter()
        .next()
        .unwrap_or_default();
    let start = session.first;
    let facts = Facts::new()
        .fact("Session", html! { code { (id) } })
        .fact("Account", session.account.clone().unwrap_or_else(|| "Not signed in".into()))
        .fact("Source", format!("{} / {}", session.source, session.campaign))
        .fact("Device", mine.iter().find(|e| !e.device.is_empty()).map_or("unknown".into(), |e| e.device.clone()))
        .fact("Started", iso(start))
        .fact("Length", funnel::duration(session.last - start))
        .fact("Furthest step", session.furthest().name());
    let mut table = Table::new()
        .label("Events")
        .header(["Time", "Kind", "Path", "What", "Details"]);
    for e in mine.iter().take(MOST_TIMELINE) {
        let details = e
            .props
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        table = table.row([
            html! { "+" (funnel::duration(e.at - start)) },
            html! { (e.kind) },
            html! { code { (e.path) } },
            html! { (e.label) },
            html! { (details) },
        ]);
    }
    html! {
        (MarkdownRoot::new(html! { h1 { "Session" } }))
        (back)
        (facts)
        (table)
        @if mine.len() > MOST_TIMELINE { p { "Showing the first " (MOST_TIMELINE) " events." } }
    }
}
