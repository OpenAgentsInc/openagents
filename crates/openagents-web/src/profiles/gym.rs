//! Gym and benchmark results on agent pages (#12302): the published,
//! verified Terminal-Bench leaderboard (`gym-leaderboard`'s client, which
//! checks the publication's digest and signature), and the boards whose
//! subject runs the agent's model or is the agent itself.
//!
//! Read at most every [`REFRESH`] and only when the site was started with
//! it (`Config::gym_results`); a page never waits on it longer than
//! [`WAIT`].

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use gym_leaderboard::contract::{Board, Leaderboard};
use maud::{Markup, html};
use openagents_ui::content::MarkdownRoot;

use crate::App;

/// How often the publication is read again.
const REFRESH: Duration = Duration::from_secs(600);
/// The longest a page waits for a read.
const WAIT: Duration = Duration::from_secs(3);
const REPO: &str = "https://github.com/OpenAgentsInc/openagents";

type Held = Mutex<Option<(Instant, Arc<Leaderboard>, Option<String>)>>;

fn held() -> &'static Held {
    static HELD: OnceLock<Held> = OnceLock::new();
    HELD.get_or_init(Held::default)
}

async fn leaderboard() -> Option<(Arc<Leaderboard>, Option<String>)> {
    if let Some((at, board, commit)) = held()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        && at.elapsed() < REFRESH
    {
        return Some((board, commit));
    }
    let read = tokio::task::spawn_blocking(|| {
        let dir = std::env::temp_dir().join("openagents-web-gym");
        let fetcher =
            gym_leaderboard::client::Fetcher::new(gym_leaderboard::client::Config::new(dir))
                .ok()?;
        fetcher.refresh(&AtomicBool::new(false)).ok()
    });
    let loaded = tokio::time::timeout(WAIT, read).await.ok()?.ok()??;
    let value = (loaded.leaderboard.clone(), loaded.commit.clone());
    *held().lock().unwrap_or_else(PoisonError::into_inner) =
        Some((Instant::now(), value.0.clone(), value.1.clone()));
    Some(value)
}

/// The boards about an agent named `name` running `model`.
#[must_use]
pub(crate) fn matching<'a>(
    board: &'a Leaderboard,
    name: &str,
    model: Option<&str>,
) -> Vec<&'a Board> {
    board
        .boards
        .iter()
        .filter(|b| {
            b.subject.agent.eq_ignore_ascii_case(name)
                || model.is_some_and(|m| b.subject.model.eq_ignore_ascii_case(m))
        })
        .take(10)
        .collect()
}

/// Where a board's report lives.
fn report_link(board: &Board, commit: Option<&str>) -> String {
    format!(
        "{REPO}/blob/{}/{}",
        commit.unwrap_or("main"),
        board.provenance.report
    )
}

/// The Benchmarks section for an agent, or nothing.
pub(crate) async fn section(app: &App, name: &str, model: Option<&str>) -> Markup {
    if !app.config.gym_results {
        return html! {};
    }
    let Some((board, commit)) = leaderboard().await else {
        return html! {};
    };
    render(&matching(&board, name, model), commit.as_deref())
}

pub(crate) fn render(boards: &[&Board], commit: Option<&str>) -> Markup {
    if boards.is_empty() {
        return html! {};
    }
    html! {
        section.oa-card aria-labelledby="agent-benchmarks" {
            (MarkdownRoot::new(html! {
                h2 id="agent-benchmarks" { "Benchmarks" }
                ul {
                    @for board in boards {
                        li {
                            a href=(report_link(board, commit)) { (board.title) }
                            " · " (board.headline)
                        }
                    }
                }
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boards_match_by_the_agents_name_or_model() {
        let published: Leaderboard = serde_json::from_str(include_str!(
            "../../../../bench/terminal-bench/published/leaderboard.v1.json"
        ))
        .unwrap();
        let first = &published.boards[0];
        let by_model = matching(&published, "Someone else", Some(&first.subject.model));
        assert!(by_model.iter().any(|b| b.id == first.id));
        let by_name = matching(&published, &first.subject.agent.to_uppercase(), None);
        assert!(by_name.iter().any(|b| b.id == first.id));
        assert!(matching(&published, "Nobody", Some("no-such-model")).is_empty());
        let html = render(&by_model, Some("abc123")).into_string();
        assert!(html.contains(&format!("{REPO}/blob/abc123/{}", first.provenance.report)));
        assert!(html.contains(&maud::html! { (first.headline) }.into_string()));
    }
}
