//! `gateway-feedback` — the operator's triage for `POST /v1/feedback`
//! submissions.
//!
//! Records live under `<registry>/feedback/<id>/` beside the registry.
//! One invocation appends one lifecycle line to the record's
//! `transitions.jsonl` and rewrites its `status.json`:
//!
//! ```text
//! gateway-feedback --registry <dir> <id> accept [note...]
//! gateway-feedback --registry <dir> <id> needs-information [note...]
//! gateway-feedback --registry <dir> <id> resolve [note...]
//! gateway-feedback --registry <dir> <id> reject [note...]
//! gateway-feedback --registry <dir> <id> redact [note...]
//! ```
//!
//! `redact` additionally replaces the record's stored content with its
//! digests and removes decoded attachment bodies — the record and its
//! receipt stay, the content is gone. Deleting a record is `rm` on the
//! directory: an index line without a record answers the submitter
//! `submission_deleted` on replay and `submission_not_found` on lookup.

use std::path::PathBuf;
use std::process::ExitCode;

fn options(args: &[String]) -> Result<(PathBuf, String, String, Option<String>), String> {
    let mut registry = None;
    let mut positional = Vec::new();
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--registry" => {
                registry = Some(args.next().ok_or("--registry needs a directory")?.into());
            }
            "-h" | "--help" => {
                return Err("usage: gateway-feedback --registry <dir> <id> \
                     <accept|needs-information|resolve|reject|redact> [note...]"
                    .to_string());
            }
            other if other.starts_with('-') => return Err(format!("unknown flag: {other}")),
            other => positional.push(other.to_string()),
        }
    }
    let registry = registry.ok_or("--registry is required")?;
    let id = positional
        .first()
        .cloned()
        .ok_or("a submission id is required")?;
    let step = positional
        .get(1)
        .cloned()
        .ok_or("a transition is required")?;
    let note = positional.get(2..).map(|words| words.join(" "));
    Ok((registry, id, step, note))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match options(&args).and_then(|(registry, id, step, note)| {
        gateway::feedback::transition(&registry, &id, &step, note.as_deref())
    }) {
        Ok(status) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&status).unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
