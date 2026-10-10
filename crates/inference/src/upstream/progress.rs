//! An Open Responses stream for long-running work: progress first, the
//! answer last.
//!
//! A coding run on the caller's own computer ([`super::coder`]) takes
//! minutes, not seconds, and reports what it is doing as it goes. [`Progress`] turns those reports into the
//! spec's events, in order:
//!
//! 1. `response.created`, `response.in_progress`;
//! 2. one `reasoning` item whose summary grows a line per report
//!    (`response.reasoning_summary_part.added`, then one
//!    `response.reasoning_summary_text.delta` per line), so a caller sees
//!    progress as it happens and the router counts the first line as the
//!    attempt's first token;
//! 3. when the work is done, the summary closes and a `message` item
//!    carries the answer, then `response.completed`; when it fails or is
//!    refused, the summary closes and `response.failed` says why in plain
//!    words.
//!
//! The progress lines are written for people (no internal words). They
//! never hold the caller's prompt.

use serde_json::{Value, json};

use super::{AttemptError, ErrorClass};
use crate::event::Event;
use crate::item::{ContentPart, Item, MessageContent, Role};
use crate::request::CreateResponse;
use crate::response::{Response, ResponseStatus, Usage};
use crate::seal::random_id;

/// What long-running work reports as it goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Report {
    /// A line of progress, for people ("Running the tests.").
    Step(String),
    /// The work finished: its answer, and token counts when the agent
    /// reported them.
    Done { text: String, usage: Option<Usage> },
    /// The work stopped: why, in plain words.
    Failed(String),
}

/// The work's reports. Dropping the stream stops the work.
pub type Reports = std::pin::Pin<Box<dyn futures_util::Stream<Item = Report> + Send>>;

/// One earlier text turn of the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    /// `user` or `assistant`.
    pub role: String,
    pub content: String,
}

/// What a piece of long-running work is asked to do: the conversation as
/// text, its last user message the task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brief {
    /// `instructions`, `system`, and `developer` messages, joined.
    pub instructions: Option<String>,
    /// The turns before the task.
    pub history: Vec<Turn>,
    /// The last user message.
    pub task: String,
}

impl Brief {
    /// The brief's size in bytes of text.
    #[must_use]
    pub fn size(&self) -> usize {
        self.task.len()
            + self.instructions.as_ref().map_or(0, String::len)
            + self
                .history
                .iter()
                .map(|turn| turn.content.len())
                .sum::<usize>()
    }
}

/// The brief for `request`, at most `max_bytes` of text.
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] for anything but text messages (images,
/// files, tool calls), a conversation that does not end with a user
/// message, or one longer than `max_bytes`.
pub fn brief(request: &CreateResponse, max_bytes: usize) -> Result<Brief, AttemptError> {
    let unsupported = |why: String| AttemptError::new(ErrorClass::Unsupported, why);
    let mut instructions: Vec<String> = request.instructions.iter().cloned().collect();
    let mut turns: Vec<Turn> = Vec::new();
    for item in request.input_items() {
        match item {
            Item::Message(message) => {
                let text = match &message.content {
                    MessageContent::Text(text) => text.clone(),
                    MessageContent::Parts(parts) => {
                        let mut text = Vec::new();
                        for part in parts {
                            match part {
                                ContentPart::InputText(_)
                                | ContentPart::OutputText(_)
                                | ContentPart::Text(_) => {
                                    text.push(part.text().unwrap_or_default().to_owned());
                                }
                                ContentPart::Refusal(_) => {}
                                _ => return Err(unsupported("this work takes text only".into())),
                            }
                        }
                        text.join("\n")
                    }
                };
                match message.role {
                    Role::System | Role::Developer => instructions.push(text),
                    Role::User => turns.push(Turn {
                        role: "user".into(),
                        content: text,
                    }),
                    Role::Assistant => turns.push(Turn {
                        role: "assistant".into(),
                        content: text,
                    }),
                }
            }
            Item::Reasoning(_) => {}
            other => {
                return Err(unsupported(format!(
                    "this work takes no `{}` items",
                    other.type_name()
                )));
            }
        }
    }
    let task = match turns.pop() {
        Some(turn) if turn.role == "user" => turn.content,
        _ => {
            return Err(unsupported(
                "the conversation needs a user message last".into(),
            ));
        }
    };
    let brief = Brief {
        instructions: (!instructions.is_empty()).then(|| instructions.join("\n\n")),
        history: turns,
        task,
    };
    if brief.size() > max_bytes {
        return Err(unsupported(format!(
            "the conversation is longer than this work takes ({} KiB)",
            max_bytes / 1024
        )));
    }
    Ok(brief)
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// Builds one long-running answer's events, a few at a time.
#[derive(Clone, Debug)]
pub struct Progress {
    response: Response,
    sequence: u64,
    reasoning_id: String,
    /// The progress lines so far, joined by newlines.
    summary: String,
    /// The reasoning item is open (its summary part added, not done).
    open: bool,
    /// A terminal event has been built.
    ended: bool,
}

impl Progress {
    /// A builder for an answer to `request` from the public model `model`.
    #[must_use]
    pub fn new(request: &CreateResponse, model: &str) -> Self {
        Self {
            response: Response::from_request(random_id("resp_"), unix_secs(), model, request),
            sequence: 0,
            reasoning_id: random_id("rs_"),
            summary: String::new(),
            open: false,
            ended: false,
        }
    }

    fn lifecycle(&self, kind: &str) -> Value {
        json!({"type": kind,
            "response": serde_json::to_value(&self.response).unwrap_or(Value::Null)})
    }

    fn stamp(&mut self, bodies: Vec<Value>) -> Vec<Event> {
        bodies
            .into_iter()
            .filter_map(|mut body| {
                body["sequence_number"] = Value::from(self.sequence);
                let event = serde_json::from_value(body).ok()?;
                self.sequence += 1;
                Some(event)
            })
            .collect()
    }

    /// The opening events and the first progress line.
    #[must_use]
    pub fn start(&mut self, line: &str) -> Vec<Event> {
        if self.open || self.ended {
            return self.step(line);
        }
        self.open = true;
        self.summary = line.to_owned();
        let id = self.reasoning_id.clone();
        let bodies = vec![
            self.lifecycle("response.created"),
            self.lifecycle("response.in_progress"),
            json!({"type": "response.output_item.added", "output_index": 0,
                "item": {"type": "reasoning", "id": id, "status": "in_progress",
                         "summary": []}}),
            json!({"type": "response.reasoning_summary_part.added", "item_id": id,
                "output_index": 0, "summary_index": 0,
                "part": {"type": "summary_text", "text": ""}}),
            json!({"type": "response.reasoning_summary_text.delta", "item_id": id,
                "output_index": 0, "summary_index": 0, "delta": line}),
        ];
        self.stamp(bodies)
    }

    /// One more progress line.
    #[must_use]
    pub fn step(&mut self, line: &str) -> Vec<Event> {
        if self.ended {
            return Vec::new();
        }
        if !self.open {
            return self.start(line);
        }
        let delta = format!("\n{line}");
        self.summary.push_str(&delta);
        let id = self.reasoning_id.clone();
        self.stamp(vec![
            json!({"type": "response.reasoning_summary_text.delta", "item_id": id,
                "output_index": 0, "summary_index": 0, "delta": delta}),
        ])
    }

    /// Closes the progress item, returning its events and the item.
    fn close(&mut self) -> (Vec<Value>, Option<Value>) {
        if !self.open {
            return (Vec::new(), None);
        }
        self.open = false;
        let id = self.reasoning_id.clone();
        let part = json!({"type": "summary_text", "text": self.summary});
        let item = json!({"type": "reasoning", "id": id, "status": "completed",
            "summary": [part.clone()]});
        let bodies = vec![
            json!({"type": "response.reasoning_summary_text.done", "item_id": id,
                "output_index": 0, "summary_index": 0, "text": self.summary}),
            json!({"type": "response.reasoning_summary_part.done", "item_id": id,
                "output_index": 0, "summary_index": 0, "part": part}),
            json!({"type": "response.output_item.done", "output_index": 0,
                "item": item.clone()}),
        ];
        (bodies, Some(item))
    }

    /// The answer: the progress item closes, a message carries `text`, and
    /// the response completes with `usage` when the work reported some.
    #[must_use]
    pub fn finish(&mut self, text: &str, usage: Option<Usage>) -> Vec<Event> {
        if self.ended {
            return Vec::new();
        }
        let mut bodies = Vec::new();
        if !self.open {
            // No progress was reported: the answer arrives whole.
            bodies.push(self.lifecycle("response.created"));
            bodies.push(self.lifecycle("response.in_progress"));
        }
        let (closing, reasoning) = self.close();
        bodies.extend(closing);
        let mut output: Vec<Value> = reasoning.into_iter().collect();
        let index = output.len();
        let item_id = random_id("msg_");
        let part = json!({"type": "output_text", "text": text, "annotations": []});
        let message = json!({"type": "message", "id": item_id, "status": "completed",
            "role": "assistant", "content": [part.clone()]});
        bodies.extend([
            json!({"type": "response.output_item.added", "output_index": index,
                "item": {"type": "message", "id": item_id, "status": "in_progress",
                         "role": "assistant", "content": []}}),
            json!({"type": "response.content_part.added", "item_id": item_id,
                "output_index": index, "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []}}),
            json!({"type": "response.output_text.delta", "item_id": item_id,
                "output_index": index, "content_index": 0, "delta": text}),
            json!({"type": "response.output_text.done", "item_id": item_id,
                "output_index": index, "content_index": 0, "text": text}),
            json!({"type": "response.content_part.done", "item_id": item_id,
                "output_index": index, "content_index": 0, "part": part}),
            json!({"type": "response.output_item.done", "output_index": index,
                "item": message.clone()}),
        ]);
        output.push(message);
        self.response.status = ResponseStatus::Completed;
        self.response.completed_at = Some(unix_secs());
        self.response.usage = usage;
        self.response.output = output
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect();
        bodies.push(self.lifecycle("response.completed"));
        self.ended = true;
        self.stamp(bodies)
    }

    /// The work failed or was refused after progress began: the progress
    /// item closes and `response.failed` carries `message` (plain words for
    /// the caller) under `code`.
    #[must_use]
    pub fn fail(&mut self, code: &str, message: &str) -> Vec<Event> {
        if self.ended {
            return Vec::new();
        }
        let mut bodies = Vec::new();
        if !self.open {
            bodies.push(self.lifecycle("response.created"));
            bodies.push(self.lifecycle("response.in_progress"));
        }
        let (closing, reasoning) = self.close();
        bodies.extend(closing);
        self.response.status = ResponseStatus::Failed;
        self.response.output = reasoning
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect();
        let mut failed = self.lifecycle("response.failed");
        failed["response"]["error"] = json!({"code": code, "message": message});
        bodies.push(failed);
        self.ended = true;
        self.stamp(bodies)
    }

    /// Whether a terminal event has been built.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventBody;
    use crate::sse::StreamItem;
    use crate::stream::{Accumulator, StreamCheck};

    fn request() -> CreateResponse {
        serde_json::from_value(json!({"model": "openagents/code", "input": "fix it"})).unwrap()
    }

    fn check(events: &[Event]) -> Response {
        let items: Vec<StreamItem> = events.iter().cloned().map(StreamItem::Event).collect();
        let violations = StreamCheck::run(items.iter().chain([&StreamItem::Done]));
        assert!(violations.is_empty(), "{violations:?}");
        let mut folded = Accumulator::new();
        for event in events {
            folded.push(event);
        }
        folded.finish().unwrap()
    }

    #[test]
    fn a_brief_is_the_conversation_as_text_with_the_task_last() {
        let request: CreateResponse = serde_json::from_value(json!({
            "instructions": "Open a pull request.",
            "input": [
                {"type": "message", "role": "developer", "content": "Small diffs."},
                {"type": "message", "role": "user", "content": "hi"},
                {"type": "message", "role": "assistant", "content": "hello"},
                {"type": "message", "role": "user",
                 "content": [{"type": "input_text", "text": "fix the login bug"}]}
            ]
        }))
        .unwrap();
        let brief = brief(&request, 1024).unwrap();
        assert_eq!(
            brief.instructions.as_deref(),
            Some("Open a pull request.\n\nSmall diffs.")
        );
        assert_eq!(brief.task, "fix the login bug");
        assert_eq!(brief.history.len(), 2);
        assert!(super::brief(&request, 10).is_err());
        let ends_assistant: CreateResponse = serde_json::from_value(json!({"input": [
            {"type": "message", "role": "assistant", "content": "x"}]}))
        .unwrap();
        assert!(super::brief(&ends_assistant, 1024).is_err());
    }

    #[test]
    fn progress_lines_then_the_answer_stream_in_the_spec_order() {
        let mut progress = Progress::new(&request(), "coder/codex");
        let mut events = progress.start("Started on Studio.");
        assert!(matches!(
            events.last().unwrap().body,
            EventBody::ReasoningSummaryTextDelta(_)
        ));
        events.extend(progress.step("Running the tests."));
        events.extend(progress.finish("Opened the pull request.", Some(Usage::new(10, 0, 5, 0))));
        assert!(progress.ended());
        assert!(progress.step("late").is_empty());
        let numbers: Vec<u64> = events.iter().map(|event| event.sequence_number).collect();
        assert_eq!(numbers, (0..events.len() as u64).collect::<Vec<_>>());
        let response = check(&events);
        assert_eq!(response.output.len(), 2);
        assert_eq!(response.output_text(), "Opened the pull request.");
        assert_eq!(response.status, ResponseStatus::Completed);
        let summary = match &response.output[0] {
            crate::item::Item::Reasoning(reasoning) => reasoning.summary_text(),
            other => panic!("expected reasoning, got {other:?}"),
        };
        assert_eq!(summary, "Started on Studio.\nRunning the tests.");
    }

    #[test]
    fn an_answer_with_no_progress_still_opens_the_response() {
        let mut progress = Progress::new(&request(), "coder/codex");
        let events = progress.finish("done", None);
        assert_eq!(events[0].type_name(), "response.created");
        let response = check(&events);
        assert_eq!(response.output.len(), 1);
    }

    #[test]
    fn a_failure_after_progress_closes_the_item_and_says_why() {
        let mut progress = Progress::new(&request(), "coder/codex");
        let mut events = progress.start("Working.");
        events.extend(progress.fail("run_failed", "The run stopped before it finished."));
        assert_eq!(events.last().unwrap().type_name(), "response.failed");
        let response = events
            .last()
            .and_then(|event| event.body.response())
            .cloned()
            .unwrap();
        assert_eq!(response.status, ResponseStatus::Failed);
        assert_eq!(
            response.error.as_ref().map(|error| error.message.as_str()),
            Some("The run stopped before it finished.")
        );
        check(&events);
    }
}
