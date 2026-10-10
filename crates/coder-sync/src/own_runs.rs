//! Coding runs for the person's own API key, on this computer (#11080).
//!
//! A request to the OpenAgents API for `openagents/code` with
//! `pay: "mine"` may run on the caller's own linked computers, with their
//! own Codex or Claude Code subscriptions. While sync is on, Coder reports
//! this computer's subscription accounts, each with how many more runs it
//! can take now ([`Account`]), to the website's
//! `POST /v1/computers/{name}/runs`; the answer is the runs waiting for
//! this computer ([`Run`]), each handed out once. Coder reports each run's
//! progress lines, then its answer or why it stopped, to
//! `POST /v1/computers/{name}/runs/{id}`; the answer says whether the
//! caller left, so the run stops.
//!
//! Only the account's own key reaches these runs: the website hands them
//! out under this computer's own sign-in, and the gateway starts them only
//! for that account's requests. Nothing here is pooled or shared.

use openagents_login::Saved;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::activity::segment;
use crate::{Answer, call};

/// The coding agent a subscription runs, as the website names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    Codex,
    ClaudeCode,
}

impl Agent {
    /// The agent's name as people know it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
        }
    }
}

/// One subscription account on this computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Coder's id for it ("codex", "claude-code").
    pub id: String,
    /// Its name, as the person sees it.
    pub label: String,
    pub agent: Agent,
    /// How many more runs it can take now.
    pub free_sessions: u32,
}

/// One earlier turn of the conversation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// What a run is asked to do: the conversation as text, the task last.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Brief {
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub history: Vec<Turn>,
    pub task: String,
}

impl Brief {
    /// The brief as one task for an agent, at most `max` bytes: the
    /// instructions, the earlier turns (the oldest dropped first to fit),
    /// then the task. `None` when the task alone is longer.
    #[must_use]
    pub fn text(&self, max: usize) -> Option<String> {
        let task = self.task.trim();
        let instructions = self
            .instructions
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let mut head = String::new();
        if let Some(instructions) = instructions {
            head.push_str(instructions);
            head.push_str("\n\n");
        }
        let tail = if head.is_empty() && self.history.is_empty() {
            task.to_owned()
        } else {
            format!("The task:\n{task}")
        };
        if head.len() + tail.len() > max {
            if tail.len() > max {
                return None;
            }
            head.clear();
        }
        let room = max - head.len() - tail.len();
        let mut turns: Vec<String> = Vec::new();
        let mut used = 0;
        for turn in self.history.iter().rev() {
            let who = if turn.role == "assistant" {
                "Assistant"
            } else {
                "User"
            };
            let line = format!("{who}: {}\n", turn.content.trim());
            if used + line.len() + 32 > room {
                break;
            }
            used += line.len();
            turns.push(line);
        }
        let mut text = head;
        if !turns.is_empty() {
            text.push_str("Earlier in this conversation:\n");
            for line in turns.iter().rev() {
                text.push_str(line);
            }
            text.push('\n');
        }
        text.push_str(&tail);
        Some(text)
    }
}

/// A run handed to this computer.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Run {
    pub id: String,
    /// The [`Account::id`] it runs on.
    pub account: String,
    pub agent: Agent,
    pub brief: Brief,
}

/// Run ids: `run` and 32 lowercase hex digits.
fn run_id(id: &str) -> bool {
    id.len() == 35
        && id.starts_with("run")
        && id[3..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Report this computer's accounts; the answer is the runs waiting for it.
pub async fn take(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    accounts: &[Account],
) -> Result<Vec<Run>, Answer> {
    let body = json!({"accounts": accounts});
    match call(
        http,
        saved,
        reqwest::Method::POST,
        &format!("/v1/computers/{}/runs", segment(computer)),
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(body["runs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|run| serde_json::from_value::<Run>(run.clone()).ok())
            .filter(|run| run_id(&run.id))
            .collect()),
        (answer, _) => Err(answer),
    }
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// The answer, and token counts when the agent reported them.
    Done {
        text: String,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    /// Why it stopped, in plain words; `limited` when the subscription
    /// reached its usage limit.
    Failed { why: String, limited: bool },
}

/// Report a run's new progress `lines`, and how it ended once it did. The
/// answer is whether the caller left, so the run should stop.
pub async fn report(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    id: &str,
    lines: &[String],
    end: Option<&End>,
) -> Result<bool, Answer> {
    if !run_id(id) {
        return Err(Answer::Unknown);
    }
    let mut body = json!({"lines": lines});
    match end {
        Some(End::Done {
            text,
            input_tokens,
            output_tokens,
        }) => {
            body["done"] = json!({
                "text": text,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            });
        }
        Some(End::Failed { why, limited }) => {
            body["failed"] = json!({"why": why, "limited": limited});
        }
        None => {}
    }
    match call(
        http,
        saved,
        reqwest::Method::POST,
        &format!("/v1/computers/{}/runs/{id}", segment(computer)),
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(body["cancel"] == Value::Bool(true)),
        (answer, _) => Err(answer),
    }
}

/// A client for these calls, or `None` when one can't be built.
#[must_use]
pub fn client() -> Option<reqwest::Client> {
    crate::client()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief(history: usize) -> Brief {
        Brief {
            instructions: Some("Open a pull request.".into()),
            history: (0..history)
                .map(|n| Turn {
                    role: if n % 2 == 0 { "user" } else { "assistant" }.into(),
                    content: format!("turn {n}"),
                })
                .collect(),
            task: "fix the login bug".into(),
        }
    }

    #[test]
    fn a_brief_is_one_task_with_the_newest_turns_that_fit() {
        let text = brief(2).text(64 * 1024).unwrap();
        assert!(text.starts_with("Open a pull request.\n\nEarlier in this conversation:\nUser: turn 0\nAssistant: turn 1\n"));
        assert!(text.ends_with("The task:\nfix the login bug"));
        let short = brief(200).text(200).unwrap();
        assert!(short.len() <= 200);
        assert!(short.contains("turn 199") && !short.contains("turn 0\n"));
        let bare = Brief {
            instructions: None,
            history: Vec::new(),
            task: "fix it".into(),
        };
        assert_eq!(bare.text(100).as_deref(), Some("fix it"));
        assert_eq!(bare.text(3), None);
    }

    #[test]
    fn runs_and_accounts_read_and_write_the_websites_words() {
        let run: Run = serde_json::from_value(json!({
            "id": "run0123456789abcdef0123456789abcdef",
            "account": "codex", "agent": "claude_code",
            "brief": {"instructions": null, "history": [], "task": "x"}
        }))
        .unwrap();
        assert_eq!(run.agent, Agent::ClaudeCode);
        assert!(run_id(&run.id));
        assert!(!run_id("run../../x"));
        let account = Account {
            id: "codex".into(),
            label: "Codex".into(),
            agent: Agent::Codex,
            free_sessions: 1,
        };
        assert_eq!(serde_json::to_value(&account).unwrap()["agent"], "codex");
    }
}
