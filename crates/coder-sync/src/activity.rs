//! What Coder runs on this computer, for the phone to supervise (#11165).
//!
//! While sync is on, Coder reports its running work ([`Item`]: the open
//! chat's reply, delegated agents) to the website's
//! `POST /v1/computers/{name}/activity`, and the answer carries the
//! actions sent from the phone ([`Command`]: stop, approve, deny, or a
//! message), each handed out once. Coder reports when the list changes,
//! every [`BUSY_EVERY`] while something works or asks (so a tap on the
//! phone lands within seconds), and every [`IDLE_EVERY`] otherwise.

use std::time::Duration;

use openagents_login::Saved;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Answer, call, line};

/// How often Coder reports while something works or asks.
pub const BUSY_EVERY: Duration = Duration::from_secs(5);
/// How often Coder reports while nothing runs.
pub const IDLE_EVERY: Duration = Duration::from_secs(15);
/// The most items one report carries.
pub const MAX_ITEMS: usize = 32;

/// A question an item waits on, such as an approval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
}

/// One piece of running work.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// Stable while it runs: the chat's session id, or the agent's id.
    pub id: String,
    /// `chat` or `agent`.
    pub kind: String,
    pub title: String,
    pub engine: Option<String>,
    /// `working`, `asking`, `done`, `failed`, or `stopped`.
    pub status: String,
    pub started_unix: u64,
    pub finished_unix: Option<u64>,
    pub cost_usd: Option<f64>,
    pub tokens: Option<u64>,
    /// The synced chat it belongs to, so the phone can open it.
    pub session: Option<String>,
    pub question: Option<Question>,
    /// A short line about what it is doing.
    pub line: Option<String>,
}

impl Item {
    /// Whether it is working or waiting on a question.
    #[must_use]
    pub fn live(&self) -> bool {
        matches!(self.status.as_str(), "working" | "asking")
    }
}

/// An action sent from the phone for one item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    /// The [`Item::id`] it is for.
    pub item: String,
    /// `stop`, `approve`, `deny`, or `message`.
    pub action: String,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// How long until the next report of `items`.
#[must_use]
pub fn every(items: &[Item]) -> Duration {
    if items.iter().any(Item::live) {
        BUSY_EVERY
    } else {
        IDLE_EVERY
    }
}

/// The computer's name as a path segment.
pub(crate) fn segment(computer: &str) -> String {
    line(computer, 64)
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Report `items` for `computer`; the answer is the actions waiting for it.
pub async fn report(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    items: &[Item],
) -> Result<Vec<Command>, Answer> {
    let items = &items[..items.len().min(MAX_ITEMS)];
    let body = json!({"items": items});
    match call(
        http,
        saved,
        reqwest::Method::POST,
        &format!("/v1/computers/{}/activity", segment(computer)),
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(body["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|command| serde_json::from_value(command.clone()).ok())
            .collect()),
        (answer, _) => Err(answer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(status: &str) -> Item {
        Item {
            id: "s1".into(),
            kind: "chat".into(),
            title: "Fix".into(),
            engine: None,
            status: status.into(),
            started_unix: 1,
            finished_unix: None,
            cost_usd: None,
            tokens: None,
            session: Some("s1".into()),
            question: None,
            line: None,
        }
    }

    #[test]
    fn reports_come_often_only_while_something_runs() {
        assert_eq!(every(&[item("working")]), BUSY_EVERY);
        assert_eq!(every(&[item("asking")]), BUSY_EVERY);
        assert_eq!(every(&[item("done")]), IDLE_EVERY);
        assert_eq!(every(&[]), IDLE_EVERY);
    }

    #[test]
    fn a_computer_name_is_one_path_segment() {
        assert_eq!(segment("Chris's Mac/Studio"), "Chris%27s%20Mac%2FStudio");
    }

    #[test]
    fn a_command_reads_with_or_without_its_optional_fields() {
        let command: Command =
            serde_json::from_value(json!({"id": "a", "item": "s1", "action": "stop"})).unwrap();
        assert_eq!(command.question, None);
        assert_eq!(command.text, None);
    }
}
