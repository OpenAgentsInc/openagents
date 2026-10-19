//! `/agent/foreman`: Foreman as Agent Lab defines it, drawn as one map.
//!
//! Foreman (#11926) is the agent that keeps the whole picture of open
//! issues and active agents. This page draws its definition: the runtime
//! and model, its instructions and playbook, its tools, where it looks, its
//! check-ins, its grants, its memory, its stall ladder, the suites that test
//! it, and what we measure it by. Every value comes from the definition this
//! build carries (`agent_lab::templates::foreman()`, its playbook files under
//! `crates/agent-lab/src/templates/foreman/`, `agent_contract`'s grants, and
//! the 2026-10-17 replay fixture), so the page changes when Foreman does.
//! A tool or command is listed only while Foreman's text names it.
//!
//! The map is an inline SVG drawn on the server; hover and focus are CSS
//! (`agent-map.css` in `openagents-ui`). The page runs no script and has
//! no controls.

use axum::Router;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, Render, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};

use agent_contract::{GrantScope, NEVER_GRANTED_REFUSAL, WakeEvent};
use agent_lab::{Check, Experiment, replay, templates};

use crate::App;
use crate::ui_page::UiPage;

pub(crate) const PATH: &str = "/agent/foreman";

const REPO: &str = "https://github.com/OpenAgentsInc/openagents";

/// The stall window a computer uses when a definition sets none. It is
/// `DEFAULT_STALL_MINUTES` in the app's `agent_wakes`, which this
/// workspace doesn't link.
const HOST_STALL_MINUTES: u32 = 20;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PATH, get(foreman_page))
}

async fn foreman_page(headers: HeaderMap) -> Response {
    page(&headers)
}

pub(crate) fn page(headers: &HeaderMap) -> Response {
    UiPage::new("Foreman")
        .path(PATH)
        .description(
            "Foreman, the agent that keeps the whole picture of OpenAgents work, \
             as Agent Lab defines it.",
        )
        .scriptless()
        .content(body())
        .respond(headers)
}

/// All of Foreman's own text: its instructions and every playbook skill.
fn foreman_text() -> String {
    let mut text = templates::FOREMAN_INSTRUCTIONS.to_owned();
    for (_, skill) in templates::FOREMAN_SKILLS {
        text.push('\n');
        text.push_str(skill);
    }
    text
}

/// Whether Foreman's text names `name` in code formatting.
fn named(text: &str, name: &str) -> bool {
    text.contains(&format!("`{name}`")) || text.contains(&format!("`{name} "))
}

/// A playbook skill's name in plain words.
fn plain(name: &str) -> String {
    const WORDS: [(&str, &str); 1] = [("dispatching", "handing out")];
    name.split(' ')
        .enumerate()
        .map(|(at, word)| {
            let lower = word.to_lowercase();
            let swap = WORDS
                .iter()
                .find(|(from, _)| *from == lower)
                .map(|(_, to)| (*to).to_owned());
            match (swap, at) {
                (Some(to), 0) => capitalized(&to),
                (Some(to), _) => to,
                (None, _) => word.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A line of Foreman's Markdown as markup: `code` spans kept, `**` dropped.
fn inline(text: &str) -> Markup {
    let text = text.replace("**", "");
    html! {
        @for (at, part) in text.split('`').enumerate() {
            @if at % 2 == 1 { code { (part) } } @else { (part) }
        }
    }
}

/// The tools Foreman's app gives it, grouped, in the order a check-in uses
/// them. Each is shown only while Foreman's text names it.
const TOOLS: [(&str, &[&str]); 4] = [
    (
        "Chats",
        &[
            "list_devices",
            "list_chats",
            "read_chat",
            "get_chat",
            "changed_paths",
            "send_message",
            "interrupt_chat",
            "create_chat",
            "create_chats",
            "wait_for_turn",
        ],
    ),
    ("Issue work", &["work_issues", "land_status", "list_runs"]),
    (
        "This computer",
        &["machine_status", "machine_action", "upgrade_host"],
    ),
    ("Memory", &["record_decision"]),
];

/// The commands Foreman runs itself, each shown while its text names it.
const COMMANDS: [&str; 5] = [
    "gh",
    "openagents land submit",
    "openagents issue release",
    "openagents agents revise",
    "openagents agents bind",
];

/// The machine scopes `machine_action` and `upgrade_host` run under.
const MACHINE_SCOPES: [&str; 3] = ["build.prune", "process.stop", "host.upgrade"];

/// Where Foreman looks, with the tools or commands that read each place.
const SOURCES: [(&str, &str, &[&str]); 5] = [
    (
        "GitHub",
        "Open and recently closed issues, claim comments, blockers, commits, and the two boards.",
        &["gh"],
    ),
    (
        "The app",
        "Every device and chat: its status, last activity, and what it changed.",
        &["list_devices", "list_chats", "read_chat", "changed_paths"],
    ),
    (
        "The landing queue",
        "Finished changes waiting to land on main, and the ones that bounced.",
        &["land_status"],
    ),
    (
        "The cloud pool",
        "Issue work running on cloud computers, its events, and what it spends.",
        &["work_issues"],
    ),
    (
        "This computer",
        "Disk, capacity, the landing queue's health, and what waits on the owner.",
        &["machine_status"],
    ),
];

/// Foreman's lessons (`docs/foreman/lessons-learned.md`), by number.
const LESSONS: [(&str, &str); 13] = [
    ("L1", "Coordination ran where no one could see it"),
    ("L2", "Foreman's own turn hung"),
    ("L3", "The stall rule only watched"),
    ("L4", "Check-ins were never switched on"),
    ("L5", "Foreman's watch died with its process"),
    ("L6", "Agents handed off code that never compiled"),
    ("L7", "Branches were merged one at a time by hand"),
    ("L8", "Builds ran on the owner's Mac"),
    ("L9", "A red main was compared by hand"),
    ("L10", "Hand-written rules were wrong"),
    ("L11", "Probes changed production"),
    ("L12", "Actions without a record"),
    ("L13", "Nothing measured Foreman"),
];

fn wake(event: WakeEvent) -> &'static str {
    match event {
        WakeEvent::ChatFinished => "a chat finishes",
        WakeEvent::ChatErrored => "a chat errors",
        WakeEvent::ChatNeedsAnswer => "a chat needs an answer",
        WakeEvent::IssueOpened => "an issue opens",
        WakeEvent::IssueClosed => "an issue closes",
        WakeEvent::ClaimStale => "a claim goes stale",
        WakeEvent::ChatStalled => "a chat stalls",
        WakeEvent::ClaimNoProgress => "a claim makes no progress",
        WakeEvent::Other => "something newer",
    }
}

const WAKES: [WakeEvent; 8] = [
    WakeEvent::ChatFinished,
    WakeEvent::ChatErrored,
    WakeEvent::ChatNeedsAnswer,
    WakeEvent::IssueOpened,
    WakeEvent::IssueClosed,
    WakeEvent::ClaimStale,
    WakeEvent::ChatStalled,
    WakeEvent::ClaimNoProgress,
];

fn grant(scope: GrantScope) -> &'static str {
    match scope {
        GrantScope::ChatMessage => "Message a chat",
        GrantScope::ChatStart => "Start a chat",
        GrantScope::ChatSteer => "Steer a chat",
        GrantScope::ChatStop => "Stop a chat",
        GrantScope::LandSubmit => "Send a change to the landing queue",
        GrantScope::WorkDispatch => "Start issue work on the pool",
        GrantScope::LogWrite => "Keep its decision log",
    }
}

/// One part of Foreman around the center of the map.
struct Part {
    id: &'static str,
    title: &'static str,
    /// The short line under the title on the map.
    line: String,
    /// The accent color's name in `agent-map.css`.
    accent: &'static str,
    lessons: Vec<String>,
    card: Markup,
}

fn lessons(numbers: &[&str]) -> Vec<String> {
    numbers.iter().map(|n| (*n).to_owned()).collect()
}

fn lesson_title(number: &str) -> &'static str {
    LESSONS
        .iter()
        .find(|(n, _)| *n == number)
        .map_or("", |(_, title)| title)
}

fn minutes(seconds: u64) -> String {
    if seconds % 60 == 0 {
        format!("{} minutes", seconds / 60)
    } else {
        format!("{seconds} seconds")
    }
}

/// A replay scenario's title without its roadmap items: "Item 1: a hung
/// tool call is caught (2:11 PM)" reads "A hung tool call is caught (2:11 PM)".
fn scenario_title(title: &str) -> String {
    let short = title
        .split_once(": ")
        .filter(|(head, _)| head.starts_with("Item"))
        .map_or(title, |(_, rest)| rest);
    capitalized(short)
}

fn parts(foreman: &Experiment) -> Vec<Part> {
    let text = foreman_text();
    let spec = &foreman.draft.spec;
    let autonomy = spec.autonomy.clone().unwrap_or_default();
    let stall = autonomy.stall_minutes.unwrap_or(HOST_STALL_MINUTES);
    let model = spec
        .model
        .clone()
        .unwrap_or_else(|| "the runtime's own".into());
    let fallbacks: Vec<String> = spec.fallbacks.iter().map(|f| f.runtime.clone()).collect();
    let access = match spec.access {
        agent_contract::Access::HostDefault => "What the computer allows by default",
        agent_contract::Access::ReadOnly => "Read only",
        agent_contract::Access::WorkspaceWrite => "It may write in its project",
    };

    // The instructions' first paragraph is who Foreman says it is; the
    // first bold line after it is its rule for work.
    let paragraphs: Vec<&str> = templates::FOREMAN_INSTRUCTIONS
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let identity = paragraphs.first().copied().unwrap_or("");
    let rule = paragraphs
        .iter()
        .find_map(|p| p.strip_prefix("**")?.split_once("**").map(|(r, _)| r))
        .unwrap_or("");
    let words = text.split_whitespace().count();

    let tools: Vec<(&str, Vec<&str>)> = TOOLS
        .iter()
        .map(|(group, names)| {
            (
                *group,
                names.iter().copied().filter(|n| named(&text, n)).collect(),
            )
        })
        .filter(|(_, names): &(&str, Vec<&str>)| !names.is_empty())
        .collect();
    let tool_count: usize = tools.iter().map(|(_, names)| names.len()).sum();
    let commands: Vec<&str> = COMMANDS
        .iter()
        .copied()
        .filter(|c| named(&text, c))
        .collect();
    let scopes: Vec<&str> = MACHINE_SCOPES
        .iter()
        .copied()
        .filter(|s| text.contains(s))
        .collect();
    let sources: Vec<_> = SOURCES
        .iter()
        .filter(|(_, _, via)| via.iter().any(|v| named(&text, v)))
        .collect();

    let skill = |name: &str| {
        templates::FOREMAN_SKILLS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or("", |(_, t)| *t)
    };
    let working = skill("Check-ins").lines().find_map(|l| {
        l.strip_prefix("- ")
            .filter(|l| l.starts_with("My working state"))
    });
    let rungs: Vec<&str> = skill("Stalls")
        .lines()
        .filter_map(|l| {
            let (n, rest) = l.split_once(". ")?;
            n.parse::<u8>().ok().map(|_| rest)
        })
        .collect();
    let responsive: Vec<&str> = skill("Staying responsive")
        .lines()
        .filter_map(|l| l.strip_prefix("- "))
        .take(1)
        .collect();

    let day = replay::day();
    let mut replay_lessons: Vec<String> = day
        .scenarios
        .iter()
        .flat_map(|s| s.lessons.iter().cloned())
        .collect();
    replay_lessons.sort_by_key(|l| l.trim_start_matches('L').parse::<u32>().unwrap_or(0));
    replay_lessons.dedup();
    let automatic = foreman
        .suite
        .cases
        .iter()
        .filter(|c| !matches!(c.check, Check::Review { .. }))
        .count();

    vec![
        Part {
            id: "runtime",
            title: "Runtime and model",
            line: model.clone(),
            accent: "info",
            lessons: lessons(&["L1", "L2"]),
            card: html! {
                dl.oa-agent-map-facts {
                    div { dt { "Runtime" } dd { code { (spec.runtime) } } }
                    div { dt { "Model" } dd { code { (model) } } }
                    div {
                        dt { "If the runtime isn't ready" }
                        dd {
                            @if fallbacks.is_empty() {
                                "No fallback: it runs only where its runtime is ready."
                            } @else {
                                "Tries " (fallbacks.join(", ")) "."
                            }
                        }
                    }
                    div { dt { "Access" } dd { (access) } }
                    @if let Some(secs) = autonomy.shell_timeout_secs {
                        div { dt { "Longest shell call" } dd { (minutes(u64::from(secs))) } }
                    }
                }
                @for line in &responsive { p.oa-agent-map-quote { (inline(line)) } }
                p {
                    "It runs as a chat in the OpenAgents app, on a computer you choose, \
                     so the owner can watch and steer it from the desktop, the phone or the web."
                }
            },
        },
        Part {
            id: "instructions",
            title: "Instructions",
            line: format!("{} skills", templates::FOREMAN_SKILLS.len()),
            accent: "discovery",
            lessons: lessons(&["L6", "L7", "L8"]),
            card: html! {
                p.oa-agent-map-quote { (inline(identity)) }
                @if !rule.is_empty() { p { strong { (rule) } } }
                p.oa-page-meta { (words) " words in all." }
                ol.oa-agent-map-skills {
                    @for (name, body) in templates::FOREMAN_SKILLS {
                        li {
                            details {
                                summary { (plain(name)) }
                                pre { (body.trim_end()) }
                            }
                        }
                    }
                }
            },
        },
        Part {
            id: "tools",
            title: "Tools",
            line: format!("{tool_count} app tools"),
            accent: "warning",
            lessons: lessons(&["L10"]),
            card: html! {
                @for (group, names) in &tools {
                    h4 { (group) }
                    ul.oa-agent-map-chips {
                        @for name in names { li { code { (name) } } }
                    }
                }
                @if !commands.is_empty() {
                    h4 { "Commands it runs" }
                    ul.oa-agent-map-chips {
                        @for c in &commands { li { code { (c) } } }
                    }
                }
            },
        },
        Part {
            id: "sources",
            title: "Where I look",
            line: format!("{} places", sources.len()),
            accent: "success",
            lessons: lessons(&["L11"]),
            card: html! {
                ul.oa-agent-map-list {
                    @for (place, what, via) in &sources {
                        li {
                            strong { (place) } " " (what) " "
                            span.oa-page-meta {
                                @for (at, v) in via.iter().filter(|v| named(&text, v)).enumerate() {
                                    @if at > 0 { ", " }
                                    code { (v) }
                                }
                            }
                        }
                    }
                }
                p {
                    "It gathers fresh facts before every answer, and says plainly what it \
                     couldn't reach instead of guessing."
                }
            },
        },
        Part {
            id: "check-ins",
            title: "Check-ins",
            line: autonomy
                .every_minutes
                .map_or_else(|| "when asked".to_owned(), |m| format!("every {m} minutes")),
            accent: "info",
            lessons: lessons(&["L4", "L5"]),
            card: html! {
                dl.oa-agent-map-facts {
                    div {
                        dt { "On a schedule" }
                        dd {
                            @if autonomy.check_ins {
                                @match autonomy.every_minutes {
                                    Some(m) => { "Every " (m) " minutes" }
                                    None => { "On the computer's schedule" }
                                }
                            } @else { "Off" }
                        }
                    }
                    @if let Some(secs) = autonomy.coalesce_secs {
                        div { dt { "Waits for quiet" } dd { (minutes(u64::from(secs))) } }
                    }
                }
                p { "It also wakes when:" }
                ul.oa-agent-map-chips {
                    @if autonomy.wakes.is_empty() {
                        @for w in WAKES { li { (wake(w)) } }
                    } @else {
                        @for w in &autonomy.wakes { li { (wake(*w)) } }
                    }
                }
                p {
                    "The computer, not Foreman's own process, keeps the schedule, so a \
                     restart doesn't lose it. The owner turns check-ins off in Settings."
                }
            },
        },
        Part {
            id: "authority",
            title: "Authority",
            line: format!("{} grants", GrantScope::ALL.len()),
            accent: "danger",
            lessons: lessons(&["L3"]),
            card: html! {
                p {
                    @if autonomy.authority {
                        "Foreman asks for standing authority, so it acts on its check-ins \
                         without asking first. Without a grant it only looks and reports."
                    } @else {
                        "Foreman acts only when someone asks."
                    }
                }
                ul.oa-agent-map-chips {
                    @for scope in GrantScope::ALL {
                        li title=(scope.word()) { (grant(scope)) }
                    }
                }
                @if !scopes.is_empty() {
                    p {
                        "Machine actions need their own grant: "
                        @for (at, s) in scopes.iter().enumerate() {
                            @if at > 0 { ", " }
                            code { (s) }
                        }
                        "."
                    }
                }
                p.oa-agent-map-never { (NEVER_GRANTED_REFUSAL) }
            },
        },
        Part {
            id: "memory",
            title: "Memory",
            line: "a decision log".to_owned(),
            accent: "discovery",
            lessons: lessons(&["L12"]),
            card: html! {
                p {
                    "Every action that changes something gets its own "
                    code { "record_decision" }
                    ", and every check-in ends with one more: what it saw, decided and did. \
                     It reads that log before acting, so it doesn't repeat work."
                }
                @if let Some(line) = working { p.oa-agent-map-quote { (inline(line)) } }
            },
        },
        Part {
            id: "stalls",
            title: "Stall ladder",
            line: format!("{} rungs", rungs.len()),
            accent: "warning",
            lessons: lessons(&["L3"]),
            card: html! {
                p {
                    "Work is stalled after " (stall)
                    " minutes with no output, commit, push, comment or build. Then it climbs:"
                }
                ol.oa-agent-map-ladder {
                    @for rung in &rungs { li { (inline(rung)) } }
                }
                p { "Any progress starts the ladder over." }
            },
        },
        Part {
            id: "tests",
            title: "Tested by",
            line: format!("{} cases", foreman.suite.cases.len() + day.scenarios.len()),
            accent: "success",
            lessons: replay_lessons,
            card: html! {
                h4 { (foreman.suite.title) }
                p.oa-page-meta {
                    (automatic) " checked automatically, "
                    (foreman.suite.cases.len() - automatic) " reviewed by a person. Each trial gets "
                    (minutes(foreman.max_seconds))
                    @if foreman.no_project_files { " and no project files" }
                    "."
                }
                ul.oa-agent-map-list {
                    @for case in &foreman.suite.cases {
                        li {
                            (case.title)
                            @if matches!(case.check, Check::Review { .. }) {
                                " " span.oa-agent-map-tag { "reviewed" }
                            }
                        }
                    }
                }
                h4 { "Replay of " (day.id) }
                p.oa-page-meta {
                    (day.scenarios.len()) " moments from the day we learned most from, each \
                     scored by its deadline. A new revision is saved only if every case passes."
                }
                ul.oa-agent-map-list {
                    @for s in &day.scenarios {
                        li {
                            (scenario_title(&s.title))
                            @for l in &s.lessons { " " span.oa-agent-map-tag title=(lesson_title(l)) { (l) } }
                        }
                    }
                }
            },
        },
        Part {
            id: "measures",
            title: "Measures",
            line: "time to close".to_owned(),
            accent: "danger",
            lessons: lessons(&["L13"]),
            card: html! {
                dl.oa-agent-map-facts {
                    div { dt { "Code complete to close" } dd { "Minutes from the last push to a closed issue" } }
                    div { dt { "Stall minutes" } dd { "Minutes each chat sat with no progress" } }
                }
                p {
                    "On 2026-10-17, issues coordinated by hand took 4 h 47 min to 5 h 17 min \
                     from code complete to close. Foreman's six parallel chats closed #12018 \
                     in 21 minutes, #12022 in 33 and #12019 in 35."
                }
                p {
                    "Over 2026-10-18 and 19, 91 issues closed with a median of 2.3 hours from \
                     filing. Only 3 of 380 commits went through the landing queue, and after an \
                     account switch every Foreman turn ended empty for 35 minutes with no alarm."
                }
            },
        },
    ]
}

fn body() -> Markup {
    let foreman = templates::foreman();
    let parts = parts(&foreman);
    let spec = &foreman.draft.spec;
    PageColumn::new(html! {
        section.oa-agent-map aria-labelledby="foreman-title" {
            (MarkdownRoot::new(html! {
                p.oa-page-eyebrow { "Agent Lab" }
                h1 id="foreman-title" { (spec.name) }
                p.oa-page-lead { (spec.description) }
                p {
                    "We built Foreman in Agent Lab. This map is drawn from the definition you \
                     start from when you fork it, so it shows what Foreman is today: what runs \
                     it, what it reads, what it may do, and how we test it."
                }
            }))
            (map(&parts, &spec.runtime, spec.model.as_deref().unwrap_or("")))
            div.oa-agent-map-cards {
                @for part in &parts {
                    article.oa-agent-map-card id=(format!("part-{}", part.id)) data-accent=(part.accent) {
                        h3 { (part.title) }
                        (MarkdownRoot::new(part.card.clone()))
                        @if !part.lessons.is_empty() {
                            p.oa-agent-map-lessons {
                                "Answers "
                                @for (at, l) in part.lessons.iter().enumerate() {
                                    @if at > 0 { ", " }
                                    span title=(lesson_title(l)) { (l) }
                                }
                            }
                        }
                    }
                }
            }
            (MarkdownRoot::new(html! {
                h2 { "What we learned" }
                p {
                    "Each part above answers a lesson from Foreman's first days. "
                    a href=(format!("{REPO}/blob/main/docs/foreman/lessons-learned.md")) { "Read the lessons" }
                    ", or "
                    a href="/labs/agent" { "open Agent Lab" }
                    " to fork Foreman."
                }
                ol.oa-agent-map-lesson-list {
                    @for (n, title) in LESSONS {
                        li { span.oa-agent-map-tag { (n) } " " (title) }
                    }
                }
            }))
        }
    })
    .wide()
    .render()
}

/// The map: Foreman in the middle, its playbook on the ring around it, and
/// each part on an outer ellipse, linked to its card below.
fn map(parts: &[Part], runtime: &str, model: &str) -> Markup {
    const CX: f64 = 550.0;
    const CY: f64 = 500.0;
    const RX: f64 = 410.0;
    const RY: f64 = 388.0;
    const RING: f64 = 178.0;
    const W: f64 = 236.0;
    const H: f64 = 88.0;
    let n = parts.len().max(1) as f64;
    let at = |i: usize, rx: f64, ry: f64| {
        // Start a half step past the top so no part sits on the vertical axis
        // and the widest pairs, side by side at the top and bottom, clear.
        let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * (i as f64 + 0.5) / n;
        (CX + rx * a.cos(), CY + ry * a.sin())
    };
    let skills = templates::FOREMAN_SKILLS.len().max(1) as f64;
    html! {
        figure.oa-agent-map-figure {
            svg viewBox="0 0 1100 1000" role="img"
                aria-label=(format!("Foreman at the center, with its {} parts around it", parts.len())) {
                ellipse.oa-agent-map-orbit cx=(CX) cy=(CY) rx=(RX) ry=(RY) {}
                circle.oa-agent-map-ring cx=(CX) cy=(CY) r=(RING) {}
                circle.oa-agent-map-ring-spin cx=(CX) cy=(CY) r=(RING + 32.0) {}
                @for (i, part) in parts.iter().enumerate() {
                    @let (x, y) = at(i, RX, RY);
                    @let (sx, sy) = at(i, RING + 32.0, RING + 32.0);
                    g.oa-agent-map-node data-accent=(part.accent) {
                        a href=(format!("#part-{}", part.id)) {
                            title { (part.title) ": " (part.line) }
                            line.oa-agent-map-spoke x1=(f(sx)) y1=(f(sy)) x2=(f(x)) y2=(f(y)) {}
                            circle.oa-agent-map-dot cx=(f(sx)) cy=(f(sy)) r="7" {}
                            rect.oa-agent-map-box x=(f(x - W / 2.0)) y=(f(y - H / 2.0))
                                width=(W) height=(H) rx="16" {}
                            rect.oa-agent-map-bar x=(f(x - W / 2.0)) y=(f(y - H / 2.0 + 16.0))
                                width="5" height=(f(H - 32.0)) rx="2" {}
                            text.oa-agent-map-title x=(f(x)) y=(f(y - 6.0)) text-anchor="middle" { (part.title) }
                            text.oa-agent-map-line x=(f(x)) y=(f(y + 24.0)) text-anchor="middle" { (part.line) }
                            @if !part.lessons.is_empty() {
                                text.oa-agent-map-badge x=(f(x + W / 2.0 - 10.0)) y=(f(y - H / 2.0 - 8.0))
                                    text-anchor="end" { (badge(&part.lessons)) }
                            }
                        }
                    }
                }
                @for (i, (name, _)) in templates::FOREMAN_SKILLS.iter().enumerate() {
                    @let a = -std::f64::consts::FRAC_PI_2
                        + std::f64::consts::TAU * i as f64 / skills;
                    g.oa-agent-map-skill {
                        title { "Playbook: " (plain(name)) }
                        circle cx=(f(CX + RING * a.cos())) cy=(f(CY + RING * a.sin())) r="11" {}
                        text x=(f(CX + RING * a.cos())) y=(f(CY + RING * a.sin() + 5.0))
                            text-anchor="middle" { (i + 1) }
                    }
                }
                circle.oa-agent-map-core-glow cx=(CX) cy=(CY) r="140" {}
                circle.oa-agent-map-core cx=(CX) cy=(CY) r="122" {}
                text.oa-agent-map-name x=(CX) y=(CY - 8.0) text-anchor="middle" { "Foreman" }
                text.oa-agent-map-runtime x=(CX) y=(CY + 28.0) text-anchor="middle" { (runtime) }
                text.oa-agent-map-runtime x=(CX) y=(CY + 54.0) text-anchor="middle" { (model) }
            }
            figcaption {
                "Foreman in the middle, its eight playbook skills on the ring, and each part \
                 of its definition around it. Each part links to its details below; the "
                "small labels name the lesson it answers."
            }
        }
    }
}

/// The lessons over a part on the map: the first three, then how many more.
fn badge(lessons: &[String]) -> String {
    let mut shown = lessons
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if lessons.len() > 3 {
        shown.push_str(&format!(" +{}", lessons.len() - 3));
    }
    shown
}

fn f(value: f64) -> String {
    format!("{value:.1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_and_command_is_named_in_foremans_text() {
        let text = foreman_text();
        for (_, names) in TOOLS {
            for name in names {
                assert!(named(&text, name), "Foreman no longer names {name}");
            }
        }
        for command in COMMANDS {
            assert!(named(&text, command), "Foreman no longer runs {command}");
        }
        for (place, _, via) in SOURCES {
            assert!(via.iter().any(|v| named(&text, v)), "{place}");
        }
    }

    #[test]
    fn skill_names_read_plainly() {
        for (name, _) in templates::FOREMAN_SKILLS {
            let shown = plain(name);
            assert!(oa_copy::violations(&shown, &[]).is_empty(), "{shown}");
        }
        assert_eq!(plain("Dispatching issue work"), "Handing out issue work");
    }

    #[test]
    fn the_map_draws_every_part_from_the_definition() {
        let html = body().into_string();
        assert!(html.contains("<svg viewBox=\"0 0 1100 1000\""));
        let foreman = templates::foreman();
        assert!(html.contains(&foreman.draft.spec.runtime));
        assert!(html.contains(foreman.draft.spec.model.as_deref().unwrap()));
        assert_eq!(html.matches("class=\"oa-agent-map-node\"").count(), 10);
        assert_eq!(html.matches("class=\"oa-agent-map-skill\"").count(), 8);
        for case in &foreman.suite.cases {
            assert!(html.contains(&maud::html! { (case.title) }.into_string()));
        }
        assert!(html.contains("record_decision"));
        assert!(html.contains("wait_for_turn"));
        assert!(!html.contains("<form"));
        assert!(!html.contains("<button"));
        assert!(!html.contains("<script"));
        crate::copy_guard::assert_plain(PATH, &html);
    }

    #[test]
    fn scenario_titles_drop_their_item_numbers() {
        assert_eq!(
            scenario_title("Item 1: a hung tool call is caught (2:11 PM)"),
            "A hung tool call is caught (2:11 PM)"
        );
        assert_eq!(scenario_title("Plain title"), "Plain title");
    }
}
