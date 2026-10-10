//! Scheduled prompts on the account (#11177).
//!
//! A scheduled prompt runs on one computer as a host background rule
//! (`coder-new` `/schedule`). While the person's sync choice is on, Coder
//! sends this computer's scheduled prompts and the deletions it remembers
//! to the website's `POST /v1/schedules/sync` `{computer, schedules}`,
//! and the answer is the account's list for this computer after merging:
//! prompts made, paused, or deleted on openagents.com (Settings, Scheduled
//! prompts) or in the apps. Coder applies those to its background rules,
//! so the computer stays the one that runs them. The newer change of each
//! id wins and a delete wins a tie. Nothing is sent while sync is off.
//!
//! Records travel as JSON objects (`{id, computer, prompt, days, time,
//! every_secs, chat, chat_title, workspace, paused, updated, deleted}`), so
//! this crate needs none of Coder's own types. A prompt that looks like it
//! holds a credential is never sent.

use std::time::Duration;

use openagents_login::Saved;
use serde_json::{Value, json};

use crate::{Answer, call, client};

/// How often Coder looks for changed scheduled prompts to send.
pub const LOOK_EVERY: Duration = Duration::from_secs(20);
/// How often Coder asks for the account's list even when nothing changed
/// here, so a prompt made or changed on the website arrives.
pub const PULL_EVERY: Duration = Duration::from_secs(60);
/// The most records one exchange sends.
pub const MAX_RECORDS: usize = 500;

/// Whether a record may leave this computer: it has an id, and its prompt
/// doesn't look like it holds a credential.
#[must_use]
pub fn sendable(record: &Value) -> bool {
    record["id"].as_str().is_some_and(|id| !id.is_empty())
        && ["prompt", "chat_title"].iter().all(|field| {
            record[*field]
                .as_str()
                .is_none_or(|text| secret_screen::credential_in(text).is_none())
        })
}

/// Send `records` for `computer` and get the account's list for it back.
pub async fn exchange(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    records: &[Value],
) -> Result<Vec<Value>, Answer> {
    let schedules: Vec<&Value> = records
        .iter()
        .filter(|record| sendable(record))
        .take(MAX_RECORDS)
        .collect();
    let body = json!({ "computer": computer, "schedules": schedules });
    match call(
        http,
        saved,
        reqwest::Method::POST,
        "/v1/schedules/sync",
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(schedules_of(&body, computer)),
        (answer, _) => Err(answer),
    }
}

/// The records in an answer's `schedules` that belong to `computer`.
#[must_use]
pub fn schedules_of(body: &Value, computer: &str) -> Vec<Value> {
    body["schedules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|record| record.is_object() && record["id"].is_string())
        .filter(|record| record["computer"].as_str() == Some(computer))
        .take(MAX_RECORDS * 2)
        .cloned()
        .collect()
}

/// [`exchange`] from a plain thread, waiting at most half a minute. `Err`
/// carries what the website said (or [`Answer::Retry`] when it wasn't
/// reached).
pub fn exchange_now(
    saved: &Saved,
    computer: &str,
    records: &[Value],
) -> Result<Vec<Value>, Answer> {
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
        tokio::time::timeout(
            Duration::from_secs(30),
            exchange(&http, saved, computer, records),
        )
        .await
        .unwrap_or(Err(Answer::Retry))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_that_looks_like_a_credential_never_leaves() {
        // Assembled at run time so no credential-shaped literal sits here.
        let key = format!("sk-ant-{}", "a1".repeat(20));
        assert!(sendable(
            &json!({"id": "prompt-triage", "prompt": "triage the new issues"})
        ));
        assert!(!sendable(
            &json!({"id": "prompt-key", "prompt": format!("use {key}")})
        ));
        assert!(!sendable(&json!({"prompt": "no id"})));
        // A deletion carries no prompt and is always sendable.
        assert!(sendable(&json!({"id": "prompt-gone", "deleted": true})));
    }

    #[test]
    fn an_answer_keeps_only_this_computers_records() {
        let body = json!({"schedules": [
            {"id": "prompt-a", "computer": "mac"},
            {"id": "prompt-b", "computer": "other"},
            {"computer": "mac"},
            "text",
        ]});
        let records = schedules_of(&body, "mac");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["id"], "prompt-a");
        assert!(schedules_of(&json!({}), "mac").is_empty());
    }
}
