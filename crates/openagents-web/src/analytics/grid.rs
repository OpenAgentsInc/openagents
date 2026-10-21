//! The Grid homepage's visits on `/admin/analytics` (#12439): the funnel,
//! the top paths, zones, Pylons, the terminal's chat, time on the page,
//! the latest visits, and one visit's timeline. Built from
//! [`super::visits`]' rows; the math is plain functions so it is tested
//! without a server.

use std::collections::{BTreeMap, HashMap};

use maud::{Markup, html};
use openagents_ui::content::{Facts, Table};

use super::dashboard::number;
use super::visits::{TIME, VisitRow, ZONES, valid_id};
use super::{DASHBOARD, date};

/// The funnel's steps: a label and the event that marks it.
pub const FUNNEL: [(&str, &str); 6] = [
    ("Opened the page", "page_view"),
    ("Saw the Grid", "verse_ready"),
    ("Joined", "joined"),
    ("Reached the terminal", "terminal_reached"),
    ("Sent a message", "message_sent"),
    ("Got an answer", "answered"),
];

/// Game Lab's funnel (#12433): a label, the event, and a detail it must
/// carry, if any.
pub const GAME_FUNNEL: [(&str, &str, Option<(&str, &str)>); 4] = [
    (
        "Opened Game Lab",
        "game_lab_opened",
        Some(("result", "open")),
    ),
    ("Sent a change", "game_lab_check", None),
    ("Saved a version", "game_lab_version_saved", None),
    ("Played it", "game_lab_played", None),
];

/// Events left out of a visit's path: the ones that repeat or only say
/// how the page is doing.
const NOT_PATH: [&str; 9] = [
    "heartbeat",
    "tab_hidden",
    "tab_left",
    "pylon_hovered",
    "zoom_used",
    "verse_load_start",
    "verse_first_frame",
    "overview_watched",
    "zone_load_start",
];
/// The most steps a path shows.
const PATH_STEPS: usize = 8;

/// One visit's rows, in the order the page numbered them.
pub struct Visit<'a> {
    pub id: &'a str,
    pub rows: Vec<&'a VisitRow>,
}

impl Visit<'_> {
    fn has(&self, event: &str) -> bool {
        self.rows.iter().any(|row| row.event == event)
    }

    pub fn last_minute(&self) -> u64 {
        self.rows.iter().map(|row| row.minute).max().unwrap_or(0)
    }

    pub fn first_minute(&self) -> u64 {
        self.rows.iter().map(|row| row.minute).min().unwrap_or(0)
    }

    /// How many of [`FUNNEL`]'s steps this visit reached, in order.
    pub fn steps(&self) -> usize {
        let mut reached = 0;
        for row in &self.rows {
            if reached < FUNNEL.len() && row.event == FUNNEL[reached].1 {
                reached += 1;
            }
        }
        reached
    }

    /// How many of [`GAME_FUNNEL`]'s steps this visit reached, in order.
    pub fn game_steps(&self) -> usize {
        let mut reached = 0;
        for row in &self.rows {
            if let Some((_, event, detail)) = GAME_FUNNEL.get(reached)
                && row.event == *event
                && detail.is_none_or(|(key, value)| row.get(key) == value)
            {
                reached += 1;
            }
        }
        reached
    }

    /// The longest time on the page this visit reported, as an index into
    /// [`TIME`].
    pub fn time_on_page(&self) -> Option<usize> {
        self.rows
            .iter()
            .filter(|row| matches!(row.event.as_str(), "heartbeat" | "tab_hidden" | "tab_left"))
            .filter_map(|row| TIME.iter().position(|t| *t == row.get("time")))
            .max()
    }

    /// The visit's path: its milestones, a portal with its zone, repeats
    /// in a row folded into one, at most [`PATH_STEPS`].
    pub fn path(&self) -> String {
        let mut steps: Vec<String> = Vec::new();
        for row in &self.rows {
            if NOT_PATH.contains(&row.event.as_str()) {
                continue;
            }
            let step = match row.event.as_str() {
                "portal_entered" if row.get("opened") == "no" => {
                    format!("desktop_only:{}", row.get("zone"))
                }
                "portal_entered" | "zone_ready" | "zone_failed" => {
                    format!("{}:{}", row.event, row.get("zone"))
                }
                _ => row.event.clone(),
            };
            if steps.last() != Some(&step) {
                steps.push(step);
            }
        }
        let more = steps.len() > PATH_STEPS;
        steps.truncate(PATH_STEPS);
        let mut path = steps.join(" > ");
        if more {
            path.push_str(" > ...");
        }
        path
    }
}

/// The rows grouped by visit, each in order. Newest visits first.
pub fn visits(rows: &[VisitRow]) -> Vec<Visit<'_>> {
    let mut by: HashMap<&str, Vec<&VisitRow>> = HashMap::new();
    for row in rows {
        by.entry(row.visit.as_str()).or_default().push(row);
    }
    let mut out: Vec<Visit<'_>> = by
        .into_iter()
        .map(|(id, mut rows)| {
            rows.sort_by_key(|row| (row.n, row.minute));
            rows.dedup_by(|a, b| a.n == b.n && a.event == b.event);
            Visit { id, rows }
        })
        .collect();
    out.sort_by(|a, b| b.last_minute().cmp(&a.last_minute()).then(a.id.cmp(b.id)));
    out
}

/// How many visits reached each of [`FUNNEL`]'s steps, in order.
pub fn funnel(visits: &[Visit<'_>]) -> [u64; FUNNEL.len()] {
    let mut out = [0; FUNNEL.len()];
    for visit in visits {
        for step in out.iter_mut().take(visit.steps()) {
            *step += 1;
        }
    }
    out
}

/// How many visits reached each of [`GAME_FUNNEL`]'s steps, in order.
pub fn game_funnel(visits: &[Visit<'_>]) -> [u64; GAME_FUNNEL.len()] {
    let mut out = [0; GAME_FUNNEL.len()];
    for visit in visits {
        for step in out.iter_mut().take(visit.game_steps()) {
            *step += 1;
        }
    }
    out
}

/// The most common paths and how many visits took each.
pub fn top_paths(visits: &[Visit<'_>], limit: usize) -> Vec<(String, u64)> {
    let mut by: BTreeMap<String, u64> = BTreeMap::new();
    for visit in visits {
        let path = visit.path();
        if !path.is_empty() {
            *by.entry(path).or_default() += 1;
        }
    }
    let mut out: Vec<_> = by.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out.truncate(limit);
    out
}

/// How many visits spent each [`TIME`] bucket on the page, at most.
pub fn time_on_page(visits: &[Visit<'_>]) -> [u64; TIME.len()] {
    let mut out = [0; TIME.len()];
    for visit in visits {
        if let Some(index) = visit.time_on_page() {
            out[index] += 1;
        }
    }
    out
}

/// Events of `event`, by the value of `key`, most first.
fn by_detail(rows: &[VisitRow], event: &str, key: &str) -> Vec<(String, u64)> {
    let mut by: BTreeMap<String, u64> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.event == event) {
        let value = match row.get(key) {
            "" => "(not given)",
            value => value,
        };
        *by.entry(value.to_owned()).or_default() += 1;
    }
    let mut out: Vec<_> = by.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// Events of `event` by their `time` bucket, in bucket order.
fn by_time(rows: &[VisitRow], event: &str) -> [u64; TIME.len()] {
    let mut out = [0; TIME.len()];
    for row in rows.iter().filter(|row| row.event == event) {
        if let Some(index) = TIME.iter().position(|t| *t == row.get("time")) {
            out[index] += 1;
        }
    }
    out
}

/// One zone's line: portal entries, opened, failed, and load times.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ZoneLine {
    pub entered: u64,
    /// Portal entries this browser couldn't open: only in the desktop app.
    pub desktop_only: u64,
    pub ready: u64,
    pub failed: u64,
    pub returned: u64,
    pub times: [u64; TIME.len()],
}

/// Each zone with any event, in [`ZONES`]' order.
pub fn zones(rows: &[VisitRow]) -> Vec<(&'static str, ZoneLine)> {
    let mut out: Vec<(&'static str, ZoneLine)> = Vec::new();
    for zone in ZONES {
        let mut line = ZoneLine::default();
        for row in rows {
            match row.event.as_str() {
                "portal_entered" if row.get("zone") == zone => {
                    line.entered += 1;
                    if row.get("opened") == "no" {
                        line.desktop_only += 1;
                    }
                }
                "zone_ready" if row.get("zone") == zone => {
                    line.ready += 1;
                    if let Some(index) = TIME.iter().position(|t| *t == row.get("time")) {
                        line.times[index] += 1;
                    }
                }
                "zone_failed" if row.get("zone") == zone => line.failed += 1,
                "plaza_returned" if row.get("from") == zone => line.returned += 1,
                _ => {}
            }
        }
        if line != ZoneLine::default() {
            out.push((zone, line));
        }
    }
    out
}

/// The non-zero buckets of `times`, as `1-3s: 4, 3-10s: 2`.
fn times_text(times: &[u64; TIME.len()]) -> String {
    let parts: Vec<String> = TIME
        .iter()
        .zip(times)
        .filter(|(_, n)| **n > 0)
        .map(|(t, n)| format!("{t}: {}", number(*n)))
        .collect();
    if parts.is_empty() {
        "-".to_owned()
    } else {
        parts.join(", ")
    }
}

fn percent(part: u64, whole: u64) -> String {
    if whole == 0 {
        "-".to_owned()
    } else {
        format!("{}%", part * 100 / whole)
    }
}

fn simple(label: &str, first: &str, rows: &[(String, u64)]) -> Markup {
    if rows.is_empty() {
        return html! { p { "Nothing yet." } };
    }
    let mut table = Table::new()
        .label(label)
        .header([first, "Count"])
        .numeric(1);
    for (name, count) in rows {
        table = table.row([html! { code { (name) } }, html! { (number(*count)) }]);
    }
    html! { (table) }
}

fn buckets(label: &str, counts: &[u64; TIME.len()]) -> Markup {
    if counts.iter().all(|n| *n == 0) {
        return html! { p { "Nothing yet." } };
    }
    let mut table = Table::new()
        .label(label)
        .header(["Time", "Count"])
        .numeric(1);
    for (bucket, count) in TIME.iter().zip(counts) {
        table = table.row([html! { (bucket) }, html! { (number(*count)) }]);
    }
    html! { (table) }
}

/// The days a visit section may cover.
pub const DAYS: [(u64, &str); 3] = [(1, "Today"), (7, "7 days"), (30, "30 days")];

/// The visit report over `rows`, which cover the last `days` days.
pub fn report(rows: &[VisitRow], days: u64) -> Markup {
    let all = visits(rows);
    let funnel = funnel(&all);
    let first = funnel[0].max(1);
    let mut steps = Table::new()
        .label("Grid funnel")
        .header(["Step", "Visits", "Of all visits", "Of the step before"])
        .numeric(1)
        .numeric(2)
        .numeric(3);
    for (index, (label, _)) in FUNNEL.iter().enumerate() {
        let before = if index == 0 {
            funnel[0]
        } else {
            funnel[index - 1]
        };
        steps = steps.row([
            html! { (label) },
            html! { (number(funnel[index])) },
            html! { (percent(funnel[index], first)) },
            html! { (percent(funnel[index], before)) },
        ]);
    }
    let game = game_funnel(&all);
    let mut game_steps = Table::new()
        .label("Game Lab funnel")
        .header(["Step", "Visits", "Of the step before"])
        .numeric(1)
        .numeric(2);
    for (index, (label, _, _)) in GAME_FUNNEL.iter().enumerate() {
        let before = if index == 0 { game[0] } else { game[index - 1] };
        game_steps = game_steps.row([
            html! { (label) },
            html! { (number(game[index])) },
            html! { (percent(game[index], before)) },
        ]);
    }
    let bay = all.iter().filter(|v| v.has("pylon_bay_entered")).count() as u64;
    let opened = all.iter().filter(|v| v.has("pylon_opened")).count() as u64;
    let facts = Facts::new()
        .fact("Visits", number(all.len() as u64))
        .fact("Entered the Pylon Bay", number(bay))
        .fact("Opened a Pylon", number(opened))
        .fact(
            "Went through a portal",
            number(all.iter().filter(|v| v.has("portal_entered")).count() as u64),
        )
        .fact(
            "Clicked Log in or Sign up",
            number(
                all.iter()
                    .filter(|v| v.has("login_clicked") || v.has("signup_clicked"))
                    .count() as u64,
            ),
        )
        .fact(
            "Compared Game Lab versions",
            number(all.iter().filter(|v| v.has("game_lab_compared")).count() as u64),
        )
        .fact(
            "Signed in",
            number(all.iter().filter(|v| v.has("signin_completed")).count() as u64),
        );
    let mut zone_table = Table::new()
        .label("Zones")
        .header([
            "Zone",
            "Entered",
            "Only in the desktop app",
            "Opened",
            "Couldn't open",
            "Back to the plaza",
            "Load times",
        ])
        .numeric(1)
        .numeric(2)
        .numeric(3)
        .numeric(4)
        .numeric(5);
    let zone_lines = zones(rows);
    for (zone, line) in &zone_lines {
        zone_table = zone_table.row([
            html! { code { (zone) } },
            html! { (number(line.entered)) },
            html! { (number(line.desktop_only)) },
            html! { (number(line.ready)) },
            html! { (number(line.failed)) },
            html! { (number(line.returned)) },
            html! { (times_text(&line.times)) },
        ]);
    }
    let mut pylon_table = Table::new()
        .label("Pylons")
        .header(["Kind", "Pointed at", "Opened"])
        .numeric(1)
        .numeric(2);
    let hovered = by_detail(rows, "pylon_hovered", "kind");
    let opened_by = by_detail(rows, "pylon_opened", "kind");
    let mut kinds: Vec<&str> = hovered
        .iter()
        .chain(&opened_by)
        .map(|(k, _)| k.as_str())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    let of = |list: &[(String, u64)], kind: &str| {
        list.iter().find(|(k, _)| k == kind).map_or(0, |(_, n)| *n)
    };
    for kind in &kinds {
        pylon_table = pylon_table.row([
            html! { code { (kind) } },
            html! { (number(of(&hovered, kind))) },
            html! { (number(of(&opened_by, kind))) },
        ]);
    }
    let mut recent = Table::new()
        .label("Latest visits")
        .header(["Visit", "Started (UTC)", "Events", "Furthest step"])
        .numeric(2);
    for visit in all.iter().take(25) {
        let minute = visit.first_minute();
        let furthest = match visit.steps() {
            0 => "-",
            n => FUNNEL[n - 1].0,
        };
        recent = recent.row([
            html! { a href=(format!("{DASHBOARD}?visit={}", visit.id)) { code { (visit.id) } } },
            html! { (clock(minute)) },
            html! { (number(visit.rows.len() as u64)) },
            html! { (furthest) },
        ]);
    }
    let window = DAYS
        .iter()
        .find(|(d, _)| *d == days)
        .map_or("", |(_, l)| *l);
    html! {
        h2 { "Grid visits" }
        p {
            "The Grid homepage's visits, " (window.to_lowercase()) ". Each visit has a random id "
            "that lasts until its tab closes; rows are kept 30 days. Show: "
            @for (index, (d, label)) in DAYS.iter().enumerate() {
                @if index > 0 { " · " }
                @if *d == days { (label) } @else { a href=(format!("{DASHBOARD}?days={d}")) { (label) } }
            }
        }
        (facts)
        h3 { "Funnel" }
        (steps)
        h3 { "Top paths" }
        (simple("Top paths", "Path", &top_paths(&all, 15)))
        h3 { "Arrival" }
        (simple("How the Grid drew", "Graphics", &by_detail(rows, "verse_ready", "gpu")))
        (buckets("Time until the Grid was ready", &by_time(rows, "verse_ready")))
        (simple("Why the Grid didn't load", "Reason", &by_detail(rows, "verse_load_failed", "reason")))
        h3 { "Joining" }
        (simple("How people joined", "By", &by_detail(rows, "joined", "by")))
        (buckets("Time watching the overview before joining", &by_time(rows, "overview_watched")))
        (buckets("Time until reaching the terminal", &by_time(rows, "terminal_reached")))
        h3 { "Zones" }
        @if zone_lines.is_empty() { p { "Nothing yet." } } @else { (zone_table) }
        h3 { "Pylons" }
        @if kinds.is_empty() { p { "Nothing yet." } } @else { (pylon_table) }
        h3 { "Game Lab" }
        (game_steps)
        (simple("Game Lab opens", "Result", &by_detail(rows, "game_lab_opened", "result")))
        (simple("Games played from", "From", &by_detail(rows, "game_lab_played", "from")))
        (buckets("Time Game Lab stayed open", &by_time(rows, "game_lab_closed")))
        h3 { "Chat at the terminal" }
        (simple("Models chosen", "Model", &by_detail(rows, "model_chosen", "model")))
        (simple("Messages sent, by model", "Model", &by_detail(rows, "message_sent", "model")))
        (simple("Who answered", "Source", &by_detail(rows, "answered", "source")))
        (simple("Answers by model family", "Family", &by_detail(rows, "answered", "family")))
        (buckets("Time to the first words of an answer", &by_time(rows, "answered")))
        (simple("Answers that didn't come", "Why", &by_detail(rows, "answer_failed", "why")))
        h3 { "Time on the page" }
        (buckets("Longest time on the page per visit", &time_on_page(&all)))
        h3 { "Latest visits" }
        @if all.is_empty() { p { "Nothing yet." } } @else { (recent) }
    }
}

/// `YYYY-MM-DD HH:MM` of a minute since 1970.
fn clock(minute: u64) -> String {
    format!(
        "{} {:02}:{:02}",
        date(minute / (24 * 60)),
        (minute / 60) % 24,
        minute % 60
    )
}

/// One visit's events in order, for `?visit=ID`.
pub fn timeline(rows: &[VisitRow], id: &str) -> Markup {
    let all = visits(rows);
    let Some(visit) = all.iter().find(|v| valid_id(id) && v.id == id) else {
        return html! {
            h2 { "Visit" }
            p { "There's no visit with that id in the last 30 days." }
            p { a href=(DASHBOARD) { "Back to Analytics" } }
        };
    };
    let mut table = Table::new()
        .label("Visit timeline")
        .header(["#", "Time (UTC)", "Event", "Details"])
        .numeric(0);
    for row in &visit.rows {
        let details: Vec<String> = row
            .detail
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        table = table.row([
            html! { (row.n) },
            html! { (clock(row.minute)) },
            html! { code { (row.event) } },
            html! { (details.join(", ")) },
        ]);
    }
    html! {
        h2 { "Visit " code { (visit.id) } }
        p {
            (number(visit.rows.len() as u64)) " events. Furthest step: "
            (match visit.steps() { 0 => "-", n => FUNNEL[n - 1].0 }) ". "
            a href=(DASHBOARD) { "Back to Analytics" }
        }
        (table)
    }
}
