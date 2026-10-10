//! Coder's memory on the account (#11182).
//!
//! While the person's sync choice is on, Coder sends its notes and the
//! deletions it remembers (`coder-new` `memory::Memory::account_records`)
//! to the website's `POST /coder/memory/sync`, and the answer is the
//! account's whole list after merging: notes saved or changed on
//! openagents.com (Settings, Memory) and on the person's other computers.
//! Coder merges that back by id, the newer change winning and a delete
//! winning a tie. Nothing is sent while sync is off; notes are private to
//! the account.
//!
//! Records travel as JSON objects (`{id, scope, project, project_name,
//! kind, name, description, body, updated, deleted}`), so this crate needs
//! none of Coder's own types. A note that looks like it holds a credential
//! is never sent.

use std::time::Duration;

use openagents_login::Saved;
use serde_json::{Value, json};

use crate::{Answer, call, client};

/// How often Coder looks for changed notes to send.
pub const LOOK_EVERY: Duration = Duration::from_secs(15);
/// How often Coder asks for the account's notes even when nothing changed
/// here, so a change made on the website arrives.
pub const PULL_EVERY: Duration = Duration::from_secs(120);
/// The most records one exchange sends.
pub const MAX_RECORDS: usize = 1000;

/// Whether a record may leave this computer: it has an id, and none of
/// its text looks like a credential.
#[must_use]
pub fn sendable(record: &Value) -> bool {
    record["id"].as_str().is_some_and(|id| !id.is_empty())
        && ["name", "description", "body"].iter().all(|field| {
            record[*field]
                .as_str()
                .is_none_or(|text| secret_screen::credential_in(text).is_none())
        })
}

/// Send `records` and get the account's whole list back.
pub async fn exchange(
    http: &reqwest::Client,
    saved: &Saved,
    records: &[Value],
) -> Result<Vec<Value>, Answer> {
    let notes: Vec<&Value> = records
        .iter()
        .filter(|record| sendable(record))
        .take(MAX_RECORDS)
        .collect();
    let body = json!({ "notes": notes });
    match call(
        http,
        saved,
        reqwest::Method::POST,
        "/coder/memory/sync",
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(notes_of(&body)),
        (answer, _) => Err(answer),
    }
}

/// The records in an answer's `notes`.
#[must_use]
pub fn notes_of(body: &Value) -> Vec<Value> {
    body["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|record| record.is_object() && record["id"].is_string())
        .take(MAX_RECORDS * 2)
        .cloned()
        .collect()
}

/// [`exchange`] from a plain thread, waiting at most half a minute. `Err`
/// carries what the website said (or [`Answer::Retry`] when it wasn't
/// reached).
pub fn exchange_now(saved: &Saved, records: &[Value]) -> Result<Vec<Value>, Answer> {
    let Some(http) = client() else {
        return Err(Answer::Retry);
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Err(Answer::Retry);
    };
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(30), exchange(&http, saved, records))
            .await
            .unwrap_or(Err(Answer::Retry))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_that_looks_like_a_credential_never_leaves() {
        // Assembled at run time so no credential-shaped literal sits here.
        let key = format!("sk-ant-{}", "a1".repeat(20));
        assert!(sendable(
            &json!({"id": "mem-1", "name": "Tabs", "body": "Use tabs."})
        ));
        assert!(!sendable(
            &json!({"id": "mem-2", "name": "Key", "body": format!("use {key}")})
        ));
        assert!(!sendable(&json!({"name": "No id", "body": "x"})));
        // A deletion carries no text and is always sendable.
        assert!(sendable(&json!({"id": "mem-3", "deleted": true})));
    }

    #[test]
    fn an_answer_keeps_only_records_with_ids() {
        let body = json!({"notes": [
            {"id": "mem-1", "name": "Tabs"},
            {"name": "no id"},
            "text",
        ]});
        let notes = notes_of(&body);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0]["id"], "mem-1");
        assert!(notes_of(&json!({})).is_empty());
    }
}
