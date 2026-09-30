//! The Gym's published results: `/gym`, `/gym/results`, and the pages
//! under it.
//!
//! The results are the files this repository publishes under
//! `bench/terminal-bench/published/`: `index.json`, `leaderboard.v1.json`,
//! and one trace bundle per attempt. A page reads them from the configured
//! directory and verifies them as the apps do: the leaderboard must hash
//! to its own digest and to the digest of the index's newest publication,
//! and a bundle to the SHA-256 the leaderboard names. A publication that
//! fails its digest shows the reason and no numbers.
//!
//! Every figure, label, caveat, and sentence comes from the shared view
//! model (`gym_leaderboard::view`), which applies the presentation rules in
//! `docs/verse/gym-leaderboard.md`. This module lays those pages out as
//! HTML and never computes, sums, or relabels a number. Where the app
//! keeps a screen in memory, a page here keeps it in its URL.

use std::path::PathBuf;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use gym_leaderboard::contract::{Index, Leaderboard, TraceBundle};
use gym_leaderboard::view::{
    self, AttemptPage, BoardPage, BoardsPage, CaveatRow, Chip, Filter, Header, Nav, Page, Tab,
    TracePage,
};
use serde::Deserialize;

use crate::App;
use crate::layout::{escape, page, problem, segment};

const ROOT: &str = "/gym/results";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/gym", get(gym))
        .route(ROOT, get(boards))
        .route("/gym/results/{board}", get(board))
        .route("/gym/results/{board}/{attempt}", get(attempt))
        .route("/gym/results/{board}/{attempt}/trace", get(trace))
}

/// The newest publication, verified, and where it came from.
struct Loaded {
    leaderboard: Leaderboard,
    source: view::Source,
    dir: PathBuf,
}

/// Reads and verifies the publication. The error is a sentence for the
/// page.
async fn load(app: &App) -> Result<Loaded, String> {
    let dir = app.config.published.clone();
    tokio::task::spawn_blocking(move || {
        let index = std::fs::read(dir.join("index.json"))
            .map_err(|e| format!("The published results couldn't be read ({e})."))?;
        let index: Index = serde_json::from_slice(&index)
            .map_err(|e| format!("The publication index is malformed ({e})."))?;
        let newest = index
            .publications
            .last()
            .ok_or_else(|| "The publication index lists no publication.".to_owned())?;
        let bytes = std::fs::read(dir.join("leaderboard.v1.json"))
            .map_err(|e| format!("The published results couldn't be read ({e})."))?;
        let leaderboard = gym_leaderboard::verify::leaderboard(&bytes, Some(&newest.digest))
            .map_err(|why| {
                format!(
                    "Can't verify this publication: {why}. Nothing from it is shown, because \
                     its bytes don't match the digest the index publishes."
                )
            })?;
        let source = view::Source {
            digest: leaderboard.digest.clone(),
            commit: newest.commit.clone(),
            freshness: view::Freshness::Current,
            age_seconds: None,
            signature: Default::default(),
        };
        Ok(Loaded {
            leaderboard,
            source,
            dir,
        })
    })
    .await
    .unwrap_or_else(|_| Err("The published results couldn't be read.".to_owned()))
}

/// Reads one attempt's bundle, verified against the leaderboard's
/// reference to it.
async fn bundle(
    loaded: &Loaded,
    trace: gym_leaderboard::contract::TraceRef,
) -> Result<TraceBundle, String> {
    if trace.path.contains("..") || trace.path.starts_with('/') {
        return Err("The trace's path leaves the publication.".to_owned());
    }
    let path = loaded.dir.join(&trace.path);
    tokio::task::spawn_blocking(move || {
        let bytes =
            std::fs::read(&path).map_err(|e| format!("The trace couldn't be read ({e})."))?;
        gym_leaderboard::verify::bundle(&bytes, &trace)
            .map_err(|why| format!("Can't verify this trace: {why}. Nothing from it is shown."))
    })
    .await
    .unwrap_or_else(|_| Err("The trace couldn't be read.".to_owned()))
}

fn frame(title: &str, body: &str) -> Response {
    page(title, Some("/gym"), body)
}

fn failed(status: StatusCode, text: &str) -> Response {
    problem(status, "Gym results", text, (ROOT, "All published results"))
}

fn board_href(board: &str) -> String {
    format!("{ROOT}/{}", segment(board))
}

fn attempt_href(board: &str, attempt: &str) -> String {
    format!("{ROOT}/{}/{}", segment(board), segment(attempt))
}

/// The trail of links above a page.
fn crumbs(trail: &[(String, Option<String>)]) -> String {
    let mut out = format!("<p class=\"crumbs\"><a href=\"{ROOT}\">published results</a>");
    for (text, href) in trail {
        out.push_str(" / ");
        match href {
            Some(href) => out.push_str(&format!("<a href=\"{href}\">{}</a>", escape(text))),
            None => out.push_str(&escape(text)),
        }
    }
    out.push_str("</p>");
    out
}

fn chips(chips: &[Chip]) -> String {
    if chips.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul class=\"chips\" aria-label=\"Labels\">");
    for chip in chips {
        out.push_str(&format!("<li>{}</li>", escape(chip.text)));
    }
    out.push_str("</ul>");
    out
}

fn caveats(rows: &[CaveatRow]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul>");
    for row in rows {
        out.push_str(&format!("<li>{}</li>", escape(&row.text)));
    }
    out.push_str("</ul>");
    out
}

fn lines(items: &[String]) -> String {
    let mut out = String::new();
    for item in items {
        out.push_str(&format!("<p>{}</p>", escape(item)));
    }
    out
}

fn boards_html(page: &BoardsPage) -> String {
    let mut out = String::new();
    if let Some(summary) = &page.summary {
        out.push_str(&format!(
            "<section class=\"box\"><h2 class=\"box-title\">summary</h2><p><a href=\"{}\">{}</a></p><p class=\"hint\">{}</p></section>",
            board_href(&summary.board),
            escape(&summary.text),
            escape(&summary.source)
        ));
    }
    out.push_str("<ul class=\"list\">");
    for row in &page.rows {
        out.push_str(&format!(
            "<li><a class=\"title\" href=\"{}\">{}</a> <span class=\"dim\">{}</span><p>{}</p>",
            board_href(&row.id),
            escape(&row.title),
            escape(&row.benchmark),
            escape(&row.headline)
        ));
        if let Some(note) = row.headline_note {
            out.push_str(&format!("<p class=\"hint\">{}</p>", escape(note)));
        }
        out.push_str(&chips(&row.labels));
        out.push_str("</li>");
    }
    out.push_str("</ul>");
    if let Some(footer) = &page.footer {
        out.push_str(&format!("<p class=\"hint\">{}</p>", escape(footer)));
    }
    out
}

/// `/gym`: what the Gym is, then the published results.
async fn gym(State(app): State<App>) -> Response {
    let intro = "<h1>Gym</h1><p>The Gym runs coding agents on public benchmarks and publishes \
every attempt: the task, the result, the cost, and the trace. Numbers come from committed \
evidence and are checked against their digests before they are shown.</p>";
    match load(&app).await {
        Ok(loaded) => match view::render(
            &Nav::default(),
            &loaded.leaderboard,
            Some(&loaded.source),
            None,
        ) {
            Ok(Page::Boards(boards)) => frame(
                "Gym",
                &format!(
                    "{intro}<h2>Published results</h2>{}<p><a href=\"{ROOT}\">[ All published results ]</a></p>",
                    boards_html(&boards)
                ),
            ),
            Ok(_) | Err(_) => failed(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The results couldn't be drawn.",
            ),
        },
        Err(text) => frame(
            "Gym",
            &format!(
                "{intro}<p class=\"error\" role=\"alert\">{}</p>",
                escape(&text)
            ),
        ),
    }
}

async fn boards(State(app): State<App>) -> Response {
    let loaded = match load(&app).await {
        Ok(loaded) => loaded,
        Err(text) => return failed(StatusCode::SERVICE_UNAVAILABLE, &text),
    };
    match view::render(
        &Nav::default(),
        &loaded.leaderboard,
        Some(&loaded.source),
        None,
    ) {
        Ok(Page::Boards(page)) => frame(
            "Gym results",
            &format!(
                "{}<h1>Published results</h1>{}",
                crumbs(&[]),
                boards_html(&page)
            ),
        ),
        _ => failed(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The results couldn't be drawn.",
        ),
    }
}

#[derive(Deserialize)]
struct BoardQuery {
    filter: Option<Filter>,
    caveats: Option<String>,
}

fn board_html(page: &BoardPage) -> String {
    let mut out = format!(
        "{}<h1>{}</h1><p class=\"dim\">{} \u{b7} {}</p><p>{}</p><p class=\"loud\">{}</p>",
        crumbs(&[(page.title.clone(), None)]),
        escape(&page.title),
        escape(&page.benchmark),
        escape(&page.question),
        escape(&page.summary),
        escape(&page.headline)
    );
    if let Some(note) = page.headline_note {
        out.push_str(&format!("<p class=\"hint\">{}</p>", escape(note)));
    }
    out.push_str(&chips(&page.labels));
    out.push_str("<table><thead><tr><th>Split</th><th>Passed</th><th>Beat</th><th>Cost unknown</th><th>Faults</th></tr></thead><tbody>");
    for tally in &page.tallies {
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&tally.name),
            escape(&tally.passed),
            escape(&tally.beat),
            escape(&tally.cost_unknown),
            escape(&tally.faults)
        ));
    }
    out.push_str("</tbody></table>");
    out.push_str(&lines(&page.spend));
    out.push_str(&format!(
        "<p class=\"hint\">Bar: {} \u{b7} {} \u{b7} {}</p>",
        escape(&page.reference.name),
        escape(&page.reference.rule),
        escape(&page.reference.conditions)
    ));
    out.push_str(&format!(
        "<h2>Caveats ({})</h2>{}",
        page.caveat_count,
        caveats(&page.caveats)
    ));
    if !page.caveats_open && page.caveat_count > page.caveats.len() {
        out.push_str(&format!(
            "<p><a href=\"{}?filter={}&amp;caveats=all\">[ All caveats ]</a></p>",
            board_href(&page.id),
            filter_code(page.filter)
        ));
    }
    out.push_str("<h2>Tasks</h2><ul class=\"tabs\">");
    for chip in &page.filters {
        let text = format!("{} ({})", chip.text, chip.count);
        if chip.selected {
            out.push_str(&format!(
                "<li><a aria-current=\"page\" href=\"{}?filter={}\">{}</a></li>",
                board_href(&page.id),
                filter_code(chip.filter),
                escape(&text)
            ));
        } else {
            out.push_str(&format!(
                "<li><a href=\"{}?filter={}\">{}</a></li>",
                board_href(&page.id),
                filter_code(chip.filter),
                escape(&text)
            ));
        }
    }
    out.push_str("</ul><ul class=\"list\">");
    for task in &page.tasks {
        out.push_str(&format!(
            "<li><span class=\"title\">{}</span> <span class=\"dim\">{} \u{b7} {} \u{b7} {}</span><ul class=\"cells\">",
            escape(&task.task),
            escape(task.status),
            escape(task.knowledge),
            escape(&task.bar)
        ));
        for cell in &task.attempts {
            let class = if cell.beat {
                "beat"
            } else if cell.passed {
                "pass"
            } else {
                "fail"
            };
            out.push_str(&format!(
                "<li><a class=\"{class}\" href=\"{}\" aria-label=\"{}\">{}</a></li>",
                attempt_href(&page.id, &cell.id),
                escape(&cell.accessibility),
                escape(&cell.text)
            ));
        }
        out.push_str("</ul></li>");
    }
    out.push_str("</ul>");
    out
}

fn filter_code(filter: Filter) -> String {
    serde_json::to_value(filter)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn tab_code(tab: Tab) -> String {
    serde_json::to_value(tab)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

async fn board(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<BoardQuery>,
) -> Response {
    let loaded = match load(&app).await {
        Ok(loaded) => loaded,
        Err(text) => return failed(StatusCode::SERVICE_UNAVAILABLE, &text),
    };
    let mut nav = Nav::default();
    let chosen = nav
        .select_board(&loaded.leaderboard, &id)
        .and_then(|()| nav.set_filter(query.filter.unwrap_or_default()))
        .and_then(|()| nav.set_caveats_open(query.caveats.as_deref() == Some("all")));
    if let Err(why) = chosen {
        return failed(StatusCode::NOT_FOUND, &why);
    }
    match view::render(&nav, &loaded.leaderboard, Some(&loaded.source), None) {
        Ok(Page::Board(page)) => frame(&page.title.clone(), &board_html(&page)),
        _ => failed(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The board couldn't be drawn.",
        ),
    }
}

fn header_html(header: &Header) -> String {
    format!(
        "<p class=\"loud\">{} \u{b7} {} \u{b7} {} \u{b7} {}</p>{}",
        escape(header.result),
        escape(header.beat),
        escape(&header.cost),
        escape(&header.time),
        chips(&header.labels)
    )
}

fn attempt_html(board: &str, page: &AttemptPage) -> String {
    let mut out = format!(
        "{}<h1>{}</h1>{}<p class=\"dim\">{} \u{b7} {}</p>{}<p>{}</p>",
        crumbs(&[
            (page.board_title.clone(), Some(board_href(board))),
            (page.id.clone(), None)
        ]),
        escape(&page.header.task),
        header_html(&page.header),
        escape(&page.series),
        escape(&page.trial),
        lines(&page.numbers),
        escape(&page.misses)
    );
    out.push_str(&format!(
        "<p class=\"hint\">Bar: {} \u{b7} {} \u{b7} {}</p>",
        escape(&page.reference.name),
        escape(&page.reference.rule),
        escape(&page.reference.conditions)
    ));
    if !page.phases.is_empty() {
        out.push_str("<h2>Phases</h2>");
        out.push_str(&lines(&page.phases));
    }
    for (title, text) in [
        ("How it ended", &page.how_it_ended),
        ("Jev", &page.jev),
        ("Verifier", &page.verifier),
    ] {
        if let Some(text) = text {
            out.push_str(&format!("<h2>{title}</h2><p>{}</p>", escape(text)));
        }
    }
    if !page.failed_tests.is_empty() {
        out.push_str("<h2>Failed tests</h2><ul>");
        for test in &page.failed_tests {
            out.push_str(&format!("<li><code>{}</code></li>", escape(test)));
        }
        out.push_str("</ul>");
    }
    if !page.caveats.is_empty() {
        out.push_str(&format!("<h2>Caveats</h2>{}", caveats(&page.caveats)));
    }
    if let Some(trace) = &page.trace {
        out.push_str(&format!(
            "<p><a href=\"{}/trace\">[ {} ]</a></p>",
            attempt_href(board, &page.id),
            escape(trace)
        ));
    }
    out
}

async fn attempt(State(app): State<App>, Path((board, id)): Path<(String, String)>) -> Response {
    let loaded = match load(&app).await {
        Ok(loaded) => loaded,
        Err(text) => return failed(StatusCode::SERVICE_UNAVAILABLE, &text),
    };
    let mut nav = Nav::default();
    if let Err(why) = nav
        .select_board(&loaded.leaderboard, &board)
        .and_then(|()| nav.select_attempt(&loaded.leaderboard, &id))
    {
        return failed(StatusCode::NOT_FOUND, &why);
    }
    match view::render(&nav, &loaded.leaderboard, Some(&loaded.source), None) {
        Ok(Page::Attempt(page)) => frame(&page.header.task.clone(), &attempt_html(&board, &page)),
        _ => failed(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The attempt couldn't be drawn.",
        ),
    }
}

#[derive(Deserialize)]
struct TraceQuery {
    tab: Option<Tab>,
    page: Option<usize>,
    open: Option<usize>,
}

fn trace_html(board: &str, board_title: &str, page: &TracePage) -> String {
    let base = format!("{}/trace", attempt_href(board, &page.attempt));
    let mut out = format!(
        "{}<h1>{}</h1>{}<p class=\"dim\">{}</p><ul class=\"tabs\">",
        crumbs(&[
            (board_title.to_owned(), Some(board_href(board))),
            (
                page.attempt.clone(),
                Some(attempt_href(board, &page.attempt))
            ),
            ("trace".to_owned(), None)
        ]),
        escape(&page.header.task),
        header_html(&page.header),
        escape(&page.clock.text)
    );
    for chip in &page.tabs {
        if !chip.available {
            out.push_str(&format!("<li><span>{}</span></li>", escape(chip.text)));
        } else if chip.selected {
            out.push_str(&format!(
                "<li><a aria-current=\"page\" href=\"{base}?tab={}\">{}</a></li>",
                tab_code(chip.tab),
                escape(chip.text)
            ));
        } else {
            out.push_str(&format!(
                "<li><a href=\"{base}?tab={}\">{}</a></li>",
                tab_code(chip.tab),
                escape(chip.text)
            ));
        }
    }
    out.push_str("</ul>");
    match page.tab {
        Tab::Jev => match &page.jev {
            Some(jev) => {
                out.push_str(&format!(
                    "<p>{}</p><p class=\"hint\">Question set {} \u{b7} keep at {} \u{b7} flag at {}</p>",
                    escape(&jev.summary),
                    escape(&jev.question_set),
                    jev.keep_threshold,
                    jev.flag_threshold
                ));
                if !jev.questions.is_empty() {
                    out.push_str("<ul>");
                    for question in &jev.questions {
                        out.push_str(&format!("<li>{}</li>", escape(question)));
                    }
                    out.push_str("</ul>");
                }
                if !jev.candidates.is_empty() {
                    out.push_str("<h2>Candidates</h2><ul class=\"list\">");
                    for row in &jev.candidates {
                        out.push_str(&format!(
                            "<li aria-label=\"{}\"><span class=\"title\">#{} {}</span> <span class=\"dim\">p {:.2}{}{} \u{b7} {}</span></li>",
                            escape(&row.accessibility),
                            row.rank,
                            escape(row.title.as_deref().unwrap_or(&row.id)),
                            row.p,
                            if row.kept { " \u{b7} kept" } else { "" },
                            if row.own { " \u{b7} own" } else { "" },
                            escape(&row.fate)
                        ));
                    }
                    out.push_str("</ul>");
                }
                if !jev.requirements.is_empty() {
                    out.push_str("<h2>Requirements</h2><ul>");
                    for row in &jev.requirements {
                        out.push_str(&format!(
                            "<li>{} <span class=\"dim\">p {:.2}{}</span></li>",
                            escape(&row.text),
                            row.p,
                            if row.flagged { " \u{b7} flagged" } else { "" }
                        ));
                    }
                    out.push_str("</ul>");
                }
            }
            None => out.push_str("<p class=\"dim\">This attempt has no Jev decision.</p>"),
        },
        Tab::Briefing => match &page.briefing {
            Some(text) => {
                out.push_str(&format!("<pre>{}</pre>", escape(&text.text)));
                if let Some(cut) = &text.cut {
                    out.push_str(&format!("<p class=\"hint\">{}</p>", escape(cut)));
                }
            }
            None => out.push_str("<p class=\"dim\">This attempt has no briefing.</p>"),
        },
        Tab::Agent => match &page.agent {
            Some(agent) => {
                out.push_str(&format!(
                    "<p class=\"hint\">Page {} of {} \u{b7} {}</p><ol class=\"steps\">",
                    agent.page + 1,
                    agent.pages.max(1),
                    escape(&agent.tokens)
                ));
                for row in &agent.rows {
                    out.push_str(&format!(
                        "<li><span class=\"at\">{} {}</span> {}",
                        escape(&row.at),
                        escape(row.kind),
                        escape(&row.text)
                    ));
                    if let Some(code) = row.exit_code {
                        out.push_str(&format!(" <span class=\"dim\">exit {code}</span>"));
                    }
                    if let Some(output) = &row.output {
                        out.push_str(&format!("<pre>{}</pre>", escape(&output.text)));
                        if let Some(cut) = &output.cut {
                            out.push_str(&format!("<p class=\"hint\">{}</p>", escape(cut)));
                        }
                    } else if row.expandable {
                        out.push_str(&format!(
                            " <a href=\"{base}?tab=agent&amp;page={}&amp;open={}\">[ output ]</a>",
                            agent.page, row.index
                        ));
                    }
                    if let Some(cut) = &row.cut {
                        out.push_str(&format!(" <span class=\"hint\">{}</span>", escape(cut)));
                    }
                    out.push_str("</li>");
                }
                out.push_str("</ol><p>");
                if agent.page > 0 {
                    out.push_str(&format!(
                        "<a href=\"{base}?tab=agent&amp;page={}\">[ previous ]</a> ",
                        agent.page - 1
                    ));
                }
                if agent.page + 1 < agent.pages {
                    out.push_str(&format!(
                        "<a href=\"{base}?tab=agent&amp;page={}\">[ next ]</a>",
                        agent.page + 1
                    ));
                }
                out.push_str("</p>");
            }
            None => out.push_str("<p class=\"dim\">This attempt has no agent steps.</p>"),
        },
        Tab::Verifier => match &page.verifier {
            Some(verifier) => {
                out.push_str(&format!("<p>{}</p><ul>", escape(&verifier.summary)));
                for test in &verifier.tests {
                    out.push_str(&format!(
                        "<li><code>{}</code> <span class=\"{}\">{}</span></li>",
                        escape(&test.name),
                        if test.passed { "loud" } else { "dim" },
                        escape(&test.status)
                    ));
                }
                out.push_str(&format!(
                    "</ul><pre>{}</pre>",
                    escape(&verifier.output_tail.text)
                ));
            }
            None => out.push_str("<p class=\"dim\">This attempt has no verifier result.</p>"),
        },
    }
    out
}

async fn trace(
    State(app): State<App>,
    Path((board, id)): Path<(String, String)>,
    Query(query): Query<TraceQuery>,
) -> Response {
    let loaded = match load(&app).await {
        Ok(loaded) => loaded,
        Err(text) => return failed(StatusCode::SERVICE_UNAVAILABLE, &text),
    };
    let mut nav = Nav::default();
    let reference = nav
        .select_board(&loaded.leaderboard, &board)
        .and_then(|()| nav.select_attempt(&loaded.leaderboard, &id))
        .and_then(|()| nav.open_trace(&loaded.leaderboard));
    let reference = match reference {
        Ok(reference) => reference,
        Err(why) => return failed(StatusCode::NOT_FOUND, &why),
    };
    let bundle = match bundle(&loaded, reference).await {
        Ok(bundle) => bundle,
        Err(why) => return failed(StatusCode::BAD_GATEWAY, &why),
    };
    let steered = nav
        .set_tab(query.tab.unwrap_or_default())
        .and_then(|()| match query.page {
            Some(page) => nav.set_page(&bundle, page),
            None => Ok(()),
        })
        .and_then(|()| match query.open {
            Some(row) => nav.expand(&bundle, Some(row)),
            None => Ok(()),
        });
    if let Err(why) = steered {
        return failed(StatusCode::NOT_FOUND, &why);
    }
    let title = loaded
        .leaderboard
        .boards
        .iter()
        .find(|b| b.id == board)
        .map_or_else(|| board.clone(), |b| b.title.clone());
    match view::render(
        &nav,
        &loaded.leaderboard,
        Some(&loaded.source),
        Some(&bundle),
    ) {
        Ok(Page::Trace(page)) => frame(
            &format!("{} trace", page.header.task),
            &trace_html(&board, &title, &page),
        ),
        _ => failed(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The trace couldn't be drawn.",
        ),
    }
}
