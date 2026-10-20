//! The environments as JSON, for the GPUI web app's work panel (#12225):
//! the person's environments, one environment's setup log, and one Claude
//! Code run, with the same words the pages show. The panel reads them again
//! every few seconds while it's open, and acts through the pages' own posts
//! (message, stop, choose), which check that a post comes from this site.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use coder_environment_operator::activity::{Entry, Stage};
use coder_environment_operator::studio::claude::{self, RunState};
use coder_environment_operator::studio::{Summary, View};
use serde_json::{Value, json};

use super::{owner_of, studio, valid_id, view};
use crate::App;

/// One line of a transcript: who or what, and its words.
fn item(kind: &str, text: impl Into<String>) -> Value {
    json!({"kind": kind, "text": text.into()})
}

fn summary(row: &Summary) -> Value {
    json!({
        "id": row.id,
        "repository": row.repository,
        "branch": row.branch,
        "status": view::status_word(row.status),
        "saved": row.saved,
        "updated_ms": row.updated_ms,
    })
}

/// The setup log, as plain lines.
pub(crate) fn setup_items(v: &View) -> Vec<Value> {
    let exit = |exit: &Option<i64>| match exit {
        Some(0) => String::new(),
        Some(code) => format!(" (exit {code})"),
        None => String::new(),
    };
    let mut items = Vec::new();
    for record in &v.records {
        items.push(match &record.entry {
            Entry::User { text } => item("user", text),
            Entry::Agent { text } | Entry::Question { text } => item("agent", text),
            Entry::Starting => item("note", "Starting a setup computer"),
            Entry::Retried => item("note", "Trying again on a new setup computer"),
            Entry::Source { ok, .. } => item(
                "step",
                if *ok {
                    "Checked out the repository"
                } else {
                    "Couldn't check out the repository"
                },
            ),
            Entry::Explored {
                command, exit: e, ..
            } => item("step", format!("Ran {command}{}", exit(e))),
            Entry::Recipe {
                revision, previous, ..
            } => item(
                "step",
                match previous {
                    None => format!("Wrote the install recipe, revision {revision}"),
                    Some(_) => format!("Edited the install recipe, revision {revision}"),
                },
            ),
            Entry::Install {
                revision, exit: e, ..
            } => item("step", format!("Ran install recipe {revision}{}", exit(e))),
            Entry::Checks { checks } => item("step", format!("{} checks to pass", checks.len())),
            Entry::CheckFailed { name, exit: e, .. } => {
                item("step", format!("Check {name} failed{}", exit(e)))
            }
            Entry::Build { stage, detail } | Entry::Verify { stage, detail } => {
                let word = match (&record.entry, stage) {
                    (Entry::Build { .. }, Stage::Started) => "Building on a clean computer",
                    (Entry::Build { .. }, Stage::Passed) => "The clean build passed",
                    (Entry::Build { .. }, Stage::Failed) => "The clean build failed",
                    (_, Stage::Started) => "Checking on a fresh computer",
                    (_, Stage::Passed) => "The checks passed",
                    (_, Stage::Failed) => "A check failed",
                };
                if detail.is_empty() {
                    item("step", word)
                } else {
                    item("step", format!("{word}: {detail}"))
                }
            }
            Entry::Ready { summary } => item("agent", summary),
            Entry::Saved { number } => item("note", format!("Saved as version {number}")),
            Entry::Failed { reason } => item("note", view::shown_reason(reason)),
        });
    }
    items
}

/// A run's transcript, as plain lines.
pub(crate) fn run_items(run: &claude::Run) -> Vec<Value> {
    let mut items = vec![item("user", &run.prompt)];
    let steps = claude::transcript(&run.events);
    let said = steps.iter().any(|s| matches!(s, claude::Step::Said(_)));
    for step in steps {
        items.push(match step {
            claude::Step::Said(text) => item("agent", text),
            claude::Step::Asked(text) => item("user", text),
            claude::Step::Tool { title, detail } => {
                let first = detail.lines().next().unwrap_or_default();
                if first.is_empty() {
                    item("step", title)
                } else {
                    item("step", format!("{title}: {first}"))
                }
            }
        });
    }
    if !said && let Some(reply) = &run.reply {
        items.push(item("agent", reply));
    }
    if run.state == RunState::Failed {
        items.push(item(
            "note",
            run.error
                .clone()
                .unwrap_or_else(|| "The run failed.".into()),
        ));
    }
    items
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        axum::Json(json!({"error": {"code": "unknown", "message": "There's no environment at this address."}})),
    )
        .into_response()
}

fn answer(value: Value) -> Response {
    super::protect(axum::Json(value).into_response())
}

/// `GET /environments/list.json`.
pub(crate) async fn list(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(studio) = studio(&app) else {
        return answer(json!({"available": false, "environments": []}));
    };
    let Some(scope) = super::mine(&app, &headers).await else {
        return not_found();
    };
    let rows: Vec<Value> = scope.rows(studio).iter().map(summary).collect();
    answer(json!({"available": true, "environments": rows}))
}

/// `GET /environments/{id}/state.json`.
pub(crate) async fn show(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(studio) = studio(&app) else {
        return not_found();
    };
    if owner_of(&app, studio, &headers, &id).await.is_none() {
        return not_found();
    }
    let Some(v) = studio.view(&id) else {
        return not_found();
    };
    let runs: Vec<Value> = v
        .runs
        .iter()
        .map(|run| {
            json!({
                "id": run.id,
                "prompt": run.prompt,
                "state": view::run_word(run.state),
                "finished": run.state.finished(),
                "updated_ms": run.updated_ms,
            })
        })
        .collect();
    answer(json!({
        "environment": summary(&v.summary),
        "question": v.question,
        "items": setup_items(&v),
        "runs": runs,
    }))
}

/// `GET /environments/{id}/runs/{run}/state.json`.
pub(crate) async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, run)): Path<(String, String)>,
) -> Response {
    let Some(studio) = studio(&app).cloned() else {
        return not_found();
    };
    if owner_of(&app, &studio, &headers, &id).await.is_none() || !valid_id(&run) {
        return not_found();
    }
    let Some(r) = studio.claude_run(&id, &run) else {
        return not_found();
    };
    let saved = crate::cloud::byo::saved_class(&app, &headers).await;
    let offered = studio.claude_choices(&id, &run, saved);
    let choices: Vec<Value> = offered
        .iter()
        .filter_map(|choice| {
            view::choice_label(*choice)
                .map(|label| json!({"key": view::choice_key(*choice), "label": label}))
        })
        .collect();
    let paused = (r.state == RunState::Paused).then(|| view::pause_line(r.paused_until, &offered));
    answer(json!({
        "id": r.id,
        "environment": r.environment,
        "state": view::run_word(r.state),
        "finished": r.state.finished(),
        "paused": paused,
        "choices": choices,
        "cost_usd": r.cost_usd,
        "items": run_items(&r),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_reads_as_plain_lines() {
        let run = claude::Run {
            id: "r1".into(),
            environment: "e1".into(),
            prompt: "Fix the build".into(),
            version: Some(2),
            state: RunState::Failed,
            events: vec![json!({"event": "delta", "text": "Looking"})],
            reply: Some("ignored, it said something".into()),
            error: None,
            created_ms: 0,
            updated_ms: 0,
            cost_usd: None,
            paused_until: None,
            openagents: false,
        };
        let items = run_items(&run);
        assert_eq!(items[0], json!({"kind": "user", "text": "Fix the build"}));
        assert_eq!(items[1], json!({"kind": "agent", "text": "Looking"}));
        assert_eq!(items.last().unwrap()["text"], "The run failed.");
        assert_eq!(items.len(), 3);
    }
}
