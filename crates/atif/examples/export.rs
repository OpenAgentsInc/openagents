//! Render one representative session as an ATIF document on stdout.
//!
//! The point is the pipeline, not the session: a log is created, steps are
//! appended, the log is finished, and the document is read back through
//! `log::read` — the same path a real session's export takes. Consumers
//! that validate the wire shape (the Terminal-Bench harness checks it
//! against Harbor's Pydantic models) run this rather than trusting a
//! hand-written fixture:
//!
//! ```sh
//! cargo run -p atif --example export > trajectory.json
//! ```

use atif::document::{Call, Decision, Outcome, Session, Source, Step, Usage};
use atif::log;
use serde_json::json;

fn main() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!("atif-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;

    let session = Session::opening(
        "example-session",
        "example-model",
        "example-door",
        "/work/example",
        "0.0.0-example",
    );
    let mut log = log::Log::create(&dir, &session)?;

    log.append(&Step::said(Source::System, "you are Coder"))?;
    log.append(&Step::said(Source::User, "count the crates"))?;

    let mut thought = Step::thought("list the workspace members first");
    thought.spent(Usage {
        prompt: 40,
        completion: 9,
    });
    log.append(&thought)?;

    log.append(&Step::called(Call {
        id: "call-1".to_string(),
        name: "shell".to_string(),
        arguments: json!({"command": "ls crates", "workdir": "/work/example"}),
        output: "atif\ncoder".to_string(),
        outcome: Outcome::Completed,
        milliseconds: 12,
        purpose: Some("look".to_string()),
        extra: Default::default(),
    }))?;

    log.append(&Step::called(
        Decision {
            id: "call-2".to_string(),
            name: "classify".to_string(),
            door: "https://api.typesafe.ai".to_string(),
            model: "jev-latest".to_string(),
            request: json!({
                "state": {"task": "count the crates"},
                "model": "jev-latest",
                "questions": {"action": {"type": "choice"}},
            }),
            answers: json!({"action": {"choice": "respond", "confidence": 0.8}}),
            route: Some("respond".to_string()),
            error: None,
            attempts: Vec::new(),
            review: None,
            milliseconds: 240,
        }
        .call(),
    ))?;

    log.append(&Step::said(Source::Agent, "two crates"))?;
    log.finish("ended")?;

    let recording = log::read_whole(log.path())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&recording.document()).unwrap_or_default()
    );
    Ok(())
}
