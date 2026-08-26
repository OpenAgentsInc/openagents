//! `oa coder --resume`: back into a thread the account already holds.
//!
//! The shape is the one `openagents coder` settled on and this CLI copies so
//! the two agree: bare `--resume` shows a picker over this repository's recent
//! threads, `--resume <id>` names one directly (the positional argument is the
//! id, not a prompt), `--resume --last` continues the most recent without
//! asking, and `--all` drops the repository filter.
//!
//! Three server reads and one write:
//!
//! - `GET /api/v1/threads?limit=50` is the picker's list.
//! - `GET /api/v1/threads/{id}` is `--resume <id>`.
//! - `GET /api/v1/threads/{id}/events?limit=50&after=<id>` is the transcript,
//!   paged because the listing caps at fifty and a working session passes
//!   fifty events inside an hour.
//! - `POST /api/v1/threads/{id}/grants` re-mints the thread's authority, which
//!   is what makes this a continuation rather than a new thread that has read
//!   an old one. The server's own fence: it revokes every active grant on the
//!   thread and bumps its generation, so a resumed session cannot race a
//!   zombie of its former self.
//!
//! ## The replay is read-only
//!
//! [`replay_wire`] rebuilds the messages the live turn loop would have
//! accumulated and nothing else. Nothing here posts an event: the server
//! already holds these, and writing them back would double the record.
//! Reasoning is deliberately absent, because the live loop never puts a
//! thought on the wire.
//!
//! ## What a Rust-opened thread replays to
//!
//! Its own conversation. [`crate::runtime`] records `turn.user`, `tool.ran`
//! and `turn.assistant` as a turn runs, which is the vocabulary below, so a
//! thread this CLI opened resumes the same way one `openagents coder` opened
//! does. It did not always: the runtime recorded nothing at all, every thread
//! held one `thread.opened` and no more, and `--resume` on one replayed zero
//! messages. A thread from before that fix still replays to an empty
//! conversation — that is the record, not a parse failure, and the caller says
//! how many messages came back rather than implying a transcript that is not
//! there.

use crate::runtime::ChatMessage;
use serde::Deserialize;
use std::time::Duration;

/// The server's listing cap. Pages are read at exactly this size.
const PAGE_LIMIT: usize = 50;

/// One thread as `GET /api/v1/threads` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSummary {
    pub id: String,
    pub status: String,
    pub objective: String,
    pub event_count: u64,
    pub started_at: Option<String>,
    /// The thread's own `repository` field when the server reports one, else
    /// parsed back out of the objective sentence this CLI composes.
    pub repository: Option<String>,
}

impl ThreadSummary {
    fn from_view(value: &serde_json::Value) -> Option<Self> {
        let id = value.get("id")?.as_str()?.to_string();
        let objective = value
            .get("objective")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let repository = value
            .get("repository")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| repository_of(&objective));
        Some(Self {
            id,
            status: value
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            objective,
            event_count: value
                .get("event_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            started_at: value
                .get("started_at")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            repository,
        })
    }

    /// One line in the picker.
    pub fn line(&self) -> String {
        let when = self.started_at.as_deref().unwrap_or("—");
        let where_ = self.repository.as_deref().unwrap_or("no repository");
        format!(
            "{}  {}  {}  {} events  {}",
            &self.id[..self.id.len().min(8)],
            self.status,
            when,
            self.event_count,
            where_
        )
    }
}

/// The repository a thread's objective names, when this CLI named it.
///
/// Deterministic parsing of a bounded field this same program wrote — the
/// session opener composes `openagents coder in <repo> on <branch>` — not a
/// guess at free text. Anything else parses to nothing and is simply a thread
/// without a repository.
pub fn repository_of(objective: &str) -> Option<String> {
    let rest = objective.strip_prefix("openagents coder in ")?;
    let (repository, _branch) = rest.rsplit_once(" on ")?;
    if repository.is_empty() {
        return None;
    }
    Some(repository.to_string())
}

/// The threads the picker offers, newest first as the server ordered them.
///
/// Filtered to the named repository unless `all`, because a reader resuming
/// work is almost always resuming it where they are standing. Terminal threads
/// stay in the list: this CLI revokes its thread on a clean exit, so an
/// open-only list would usually be empty, and picking a terminal one gets the
/// refusal that teaches why rather than a listing that hides it.
pub fn resumable_threads(
    threads: &[ThreadSummary],
    repository: Option<&str>,
    all: bool,
) -> Vec<ThreadSummary> {
    if all {
        return threads.to_vec();
    }
    let Some(repository) = repository else {
        return Vec::new();
    };
    threads
        .iter()
        .filter(|t| t.repository.as_deref() == Some(repository))
        .cloned()
        .collect()
}

/// Refuse a thread that cannot be continued.
///
/// A terminal thread holds no authority and its transcript is closed — the
/// server refuses both a re-mint and a new event — so resuming one could only
/// ever show history. The refusal names the status, because `cancelled` after
/// a clean exit and `failed` after an error call for different next steps.
pub fn assert_resumable(thread: &ThreadSummary) -> Result<(), String> {
    if thread.status == "open" {
        return Ok(());
    }
    Err(format!(
        "thread {} is {}: its transcript is closed and it holds no authority to re-grant. \
         Start a new session with `oa coder` instead.",
        thread.id, thread.status
    ))
}

/// One event as `GET /api/v1/threads/{id}/events` reports it.
#[derive(Debug, Clone, Deserialize)]
pub struct ThreadEvent {
    /// The cursor: a client continues from the last id it read.
    pub id: i64,
    pub event_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// The model-facing transcript, rebuilt in the shape the live loop feeds it.
///
/// `turn.user` is a user message as sent. `tool.ran` becomes the standard chat
/// exchange: an assistant message carrying the call in `tool_calls` with the
/// arguments as the raw JSON string the record kept, then a `tool` message
/// named by `tool_call_id`. `turn.assistant` is the turn's whole answer, and
/// an empty one is dropped — a turn that only ran tools is recorded with no
/// answer, and a blank assistant message on the wire is a message the provider
/// has to be asked to ignore. `turn.reasoning` is absent on purpose.
///
/// Event types outside this vocabulary are skipped rather than refused: the
/// transcript is append-only and a future writer may know words this reader
/// does not.
pub fn replay_wire(events: &[ThreadEvent]) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    for event in events {
        let payload = &event.payload;
        match event.event_type.as_str() {
            "turn.user" => messages.push(ChatMessage {
                role: "user".to_string(),
                content: Some(text(payload, "text")),
                tool_calls: None,
                tool_call_id: None,
            }),
            "tool.ran" => {
                let call_id = text(payload, "call_id");
                let name = {
                    let named = text(payload, "tool");
                    if named.is_empty() {
                        "tool".to_string()
                    } else {
                        named
                    }
                };
                let outcome = payload
                    .get("output")
                    .and_then(|v| v.as_str())
                    .or_else(|| payload.get("error").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![serde_json::json!({
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": text(payload, "arguments"),
                        }
                    })]),
                    tool_call_id: None,
                });
                messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(outcome),
                    tool_calls: None,
                    tool_call_id: Some(call_id),
                });
            }
            "turn.assistant" => {
                let said = text(payload, "text");
                if said.trim().is_empty() {
                    continue;
                }
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(said),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            _ => {}
        }
    }
    messages
}

fn text(payload: &serde_json::Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// the server reads
// ---------------------------------------------------------------------------

/// The reads `--resume` makes, against one origin with one account token.
pub struct ResumeApi {
    api_base: String,
    token: String,
    http: reqwest::Client,
}

impl ResumeApi {
    pub fn new(api_base: &str, token: &str) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn get(&self, url: &str, what: &str) -> Result<serde_json::Value, String> {
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|error| format!("{what}: {url} could not be reached: {error}"))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("{what}: {url} refused the read: {status} {body}"));
        }
        serde_json::from_str(&body).map_err(|error| {
            format!("{what}: {url} answered with something that is not JSON: {error}")
        })
    }

    /// The account's threads, newest first, as the server reports them.
    pub async fn list_threads(&self) -> Result<Vec<ThreadSummary>, String> {
        let url = format!("{}/threads?limit={PAGE_LIMIT}", self.api_base);
        let body = self
            .get(&url, "the account's threads could not be listed")
            .await?;
        Ok(body
            .get("threads")
            .and_then(|v| v.as_array())
            .map(|list| list.iter().filter_map(ThreadSummary::from_view).collect())
            .unwrap_or_default())
    }

    /// One thread by id, for `--resume <id>`.
    pub async fn fetch_thread(&self, thread_id: &str) -> Result<ThreadSummary, String> {
        let url = format!("{}/threads/{thread_id}", self.api_base);
        let body = self
            .get(&url, &format!("thread {thread_id} could not be read"))
            .await?;
        let view = body.get("thread").unwrap_or(&body);
        ThreadSummary::from_view(view).ok_or_else(|| {
            format!("{url} answered without a thread id, so there is nothing to resume")
        })
    }

    /// The whole transcript, oldest first, through the cursor.
    ///
    /// Pages of [`PAGE_LIMIT`], each continuing from the last event id read,
    /// until a page comes back short. The cap is the server's; a session's
    /// history is exactly the thing that outgrows it.
    pub async fn fetch_all_events(&self, thread_id: &str) -> Result<Vec<ThreadEvent>, String> {
        let mut collected: Vec<ThreadEvent> = Vec::new();
        let mut after: Option<i64> = None;
        loop {
            let cursor = match after {
                Some(id) => format!("&after={id}"),
                None => String::new(),
            };
            let url = format!(
                "{}/threads/{thread_id}/events?limit={PAGE_LIMIT}{cursor}",
                self.api_base
            );
            let body = self
                .get(
                    &url,
                    &format!("the transcript of thread {thread_id} could not be read"),
                )
                .await?;
            let page: Vec<ThreadEvent> = body
                .get("events")
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|raw| serde_json::from_value(raw.clone()).ok())
                        .collect()
                })
                .unwrap_or_default();
            let short = page.len() < PAGE_LIMIT;
            let last = page.last().map(|e| e.id);
            collected.extend(page);
            match last {
                Some(id) if !short => after = Some(id),
                _ => return Ok(collected),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// the picker
// ---------------------------------------------------------------------------

/// Ask which thread to continue, over the terminal.
///
/// Returns `Ok(None)` when the reader cancels with an empty line, which is not
/// a failure. The caller checks for a terminal before calling this: the
/// non-interactive forms are `--resume <id>` and `--resume --last`.
pub fn pick_thread(candidates: &[ThreadSummary]) -> Result<Option<ThreadSummary>, String> {
    use std::io::Write;
    println!("Threads you can continue:");
    for (index, thread) in candidates.iter().enumerate() {
        println!("  {:>2}. {}", index + 1, thread.line());
    }
    print!(
        "Continue which? (1-{}, or Enter to cancel) ",
        candidates.len()
    );
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("the picker could not read your answer: {error}"))?;
    let answer = answer.trim();
    if answer.is_empty() {
        return Ok(None);
    }
    let chosen: usize = answer
        .parse()
        .map_err(|_| format!("{answer:?} is not one of the numbers listed"))?;
    candidates
        .get(chosen.wrapping_sub(1))
        .cloned()
        .map(Some)
        .ok_or_else(|| format!("{chosen} is not one of the {} listed", candidates.len()))
}

// ---------------------------------------------------------------------------
// resolving `--resume` into a thread and a transcript
// ---------------------------------------------------------------------------

/// The thread a resumed session continues, and the transcript it continues it
/// with.
#[derive(Debug, Clone)]
pub struct Resumption {
    pub thread: ThreadSummary,
    /// The wire transcript rebuilt from the thread's recorded events. Empty
    /// where the thread recorded no turns, which is what a thread this CLI
    /// opened looks like today.
    pub messages: Vec<ChatMessage>,
    /// How many events were read, so the caller can say what came back rather
    /// than implying a transcript that is not there.
    pub events_read: usize,
}

/// How `--resume` was written on the command line.
pub struct ResumeRequest<'a> {
    /// The positional argument, which `--resume` reads as a thread id.
    pub thread_id: Option<&'a str>,
    /// `--last`.
    pub last: bool,
    /// `--all`.
    pub all: bool,
    /// The repository the picker filters to, when this checkout names one.
    pub repository: Option<String>,
    /// Whether a picker can be shown. `false` on a pipe, under `--plain`, and
    /// under `--json`.
    pub interactive: bool,
}

/// Pick the thread, read its transcript, and re-mint its authority.
///
/// `Ok(None)` means the reader cancelled the picker, which ends the command
/// without being a failure. Everything else is either a resumption or a
/// refusal naming what could not be reached.
pub async fn resolve(
    api_base: &str,
    token: Option<&str>,
    request: ResumeRequest<'_>,
) -> Result<Option<Resumption>, String> {
    let Some(token) = token else {
        return Err(
            "resuming reads the account's threads, and this API has no stored token. \
             Run `oa auth login` first."
                .to_string(),
        );
    };
    let api = ResumeApi::new(api_base, token);

    let thread = match request.thread_id {
        Some(id) => api.fetch_thread(id).await?,
        None => {
            let candidates = resumable_threads(
                &api.list_threads().await?,
                request.repository.as_deref(),
                request.all,
            );
            if candidates.is_empty() {
                return Err(if request.all {
                    "this account holds no threads to resume".to_string()
                } else {
                    match request.repository.as_deref() {
                        Some(repository) => format!(
                            "no threads were opened from {repository}. \
                             Use --all to list every thread on the account."
                        ),
                        None => "this directory is not a checkout of a repository the API knows, \
                                 so there is nothing to filter on. Use --all to list every thread \
                                 on the account, or --resume <id>."
                            .to_string(),
                    }
                });
            }
            if request.last {
                candidates[0].clone()
            } else if request.interactive {
                match pick_thread(&candidates)? {
                    Some(chosen) => chosen,
                    None => return Ok(None),
                }
            } else {
                return Err(
                    "the picker needs a terminal. Use `--resume <id>` or `--resume --last`."
                        .to_string(),
                );
            }
        }
    };

    assert_resumable(&thread)?;
    let events = api.fetch_all_events(&thread.id).await?;
    let messages = replay_wire(&events);
    Ok(Some(Resumption {
        thread,
        messages,
        events_read: events.len(),
    }))
}

/// Put a resumption onto a session: its transcript, then its authority.
///
/// The replayed messages carry no system prompt — the record does not hold one
/// — so the session's own is prepended. Without it a resumed turn would reach
/// the model with the conversation but none of the instructions that make its
/// tools usable, which reads as a model that has forgotten how to work.
pub async fn apply(
    session: &mut crate::runtime::CoderRuntimeSession,
    resumption: &Resumption,
) -> Result<(), String> {
    if !resumption.messages.is_empty() {
        let tool_defs = session.tools.list_tools();
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: Some(session.build_system_prompt(&tool_defs)),
            tool_calls: None,
            tool_call_id: None,
        }];
        messages.extend(resumption.messages.iter().cloned());
        session.messages = messages;
    }
    session
        .adopt_thread(&resumption.thread.id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// What a resumed session says before its first turn.
pub fn resumed_line(resumption: &Resumption, model: &str) -> String {
    format!(
        "Resumed thread {} on {model}: {} events read, {} messages replayed.",
        resumption.thread.id,
        resumption.events_read,
        resumption.messages.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_this_cli_opened_reports_its_own_repository() {
        let view = serde_json::json!({
            "id": "t1", "status": "open", "event_count": 3,
            "repository": "OpenAgentsInc/openagents",
            "objective": "Coding assistant session",
            "started_at": "2026-08-26T07:00:00Z",
        });
        let summary = ThreadSummary::from_view(&view).expect("a summary");
        assert_eq!(
            summary.repository.as_deref(),
            Some("OpenAgentsInc/openagents")
        );
    }

    /// Threads opened before the server carried a `repository` field still
    /// name one in the objective this CLI composed.
    #[test]
    fn an_older_thread_falls_back_to_the_objective_sentence() {
        let view = serde_json::json!({
            "id": "t2", "status": "open", "event_count": 1,
            "repository": serde_json::Value::Null,
            "objective": "openagents coder in OpenAgentsInc/openagents on main",
        });
        let summary = ThreadSummary::from_view(&view).expect("a summary");
        assert_eq!(
            summary.repository.as_deref(),
            Some("OpenAgentsInc/openagents")
        );
    }

    #[test]
    fn a_thread_with_neither_has_no_repository_and_appears_only_under_all() {
        let view = serde_json::json!({
            "id": "t3", "status": "open", "event_count": 1,
            "objective": "Coding assistant session",
        });
        let summary = ThreadSummary::from_view(&view).expect("a summary");
        assert_eq!(summary.repository, None);
        let all = resumable_threads(std::slice::from_ref(&summary), Some("a/b"), true);
        assert_eq!(all.len(), 1);
        let filtered = resumable_threads(&[summary], Some("a/b"), false);
        assert!(filtered.is_empty());
    }

    #[test]
    fn a_terminal_thread_is_refused_by_status() {
        let thread = ThreadSummary {
            id: "t4".into(),
            status: "cancelled".into(),
            objective: String::new(),
            event_count: 0,
            started_at: None,
            repository: None,
        };
        let error = assert_resumable(&thread).unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
    }

    /// The replay is the wire transcript, not the interface's: a recorded
    /// thought never becomes a message.
    #[test]
    fn the_replay_rebuilds_the_wire_and_leaves_reasoning_out() {
        let events = vec![
            ThreadEvent {
                id: 1,
                event_type: "thread.opened".into(),
                payload: serde_json::json!({}),
            },
            ThreadEvent {
                id: 2,
                event_type: "turn.user".into(),
                payload: serde_json::json!({"text": "count the crates"}),
            },
            ThreadEvent {
                id: 3,
                event_type: "turn.reasoning".into(),
                payload: serde_json::json!({"text": "I should look"}),
            },
            ThreadEvent {
                id: 4,
                event_type: "tool.ran".into(),
                payload: serde_json::json!({
                    "call_id": "c1", "tool": "repo_grep",
                    "arguments": "{\"pattern\":\"x\"}", "output": "none",
                }),
            },
            ThreadEvent {
                id: 5,
                event_type: "turn.assistant".into(),
                payload: serde_json::json!({"text": "There are 12."}),
            },
        ];
        let wire = replay_wire(&events);
        let roles: Vec<&str> = wire.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, vec!["user", "assistant", "tool", "assistant"]);
        assert_eq!(wire[2].tool_call_id.as_deref(), Some("c1"));
        assert!(
            !wire.iter().any(|m| m
                .content
                .as_deref()
                .unwrap_or_default()
                .contains("I should look")),
            "a recorded thought reached the wire"
        );
    }

    /// A turn that only ran tools is recorded with an empty answer, and an
    /// empty assistant message is not a message.
    #[test]
    fn an_empty_assistant_turn_is_not_replayed() {
        let events = vec![ThreadEvent {
            id: 1,
            event_type: "turn.assistant".into(),
            payload: serde_json::json!({"text": ""}),
        }];
        assert!(replay_wire(&events).is_empty());
    }
}
