//! Dispatch plans for multi-part asks (#11474, phase 4 of #11451).
//!
//! A message that lists several things to do can be several pieces of
//! work. At a routing point for new work, when the message is laid out as a
//! list of two or more items ([`parts`], a parser of the message's layout,
//! never of its words), one typed System One question (`dispatch-v1`)
//! asks whether it is several separate pieces of work. When System One says
//! yes, code makes a plan of the items: each part runs as its own piece of
//! work, one after another in the chat, routed on its own (its harness, its
//! briefing, its own routing record with `inputs.plan`), and one combined
//! report comes back when the last part ends. Parts run in turn rather than
//! at once because they share one checkout.
//!
//! When System One says no or does not answer within [`BUDGET`], the
//! message runs as one piece of work, as before.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::harness_plugin::Harness;

/// The question set's frozen name.
pub const QUESTION_SET: &str = "dispatch-v1";

/// How long a message waits for the question.
pub const BUDGET: Duration = Duration::from_millis(1000);

/// The most parts a plan has.
pub const MAX_PARTS: usize = 8;

const MULTI_PART: &str = "Does the person's latest message ask for several separate pieces of work, each of which could be done and checked on its own, rather than one piece of work described in steps?";

/// The `dispatch-v1` question.
#[must_use]
pub fn questions() -> jev::Questions {
    jev::Questions::new().with("multi_part", jev::Noul::new(MULTI_PART))
}

/// Whether `line` starts a list item: `1.`, `1)`, `-`, `*` or `•` and a
/// space, indented by at most three spaces. Returns the item's text.
fn item(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start().len();
    if indent > 3 {
        return None;
    }
    let rest = line.trim_start();
    for bullet in ["- ", "* ", "• "] {
        if let Some(text) = rest.strip_prefix(bullet) {
            return Some(text);
        }
    }
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 2 {
        return None;
    }
    let after = &rest[digits..];
    after
        .strip_prefix(". ")
        .or_else(|| after.strip_prefix(") "))
}

/// The message's layout as a plan: the text around the list (shared by
/// every part) and the list's items, when it holds two to [`MAX_PARTS`]
/// top-level items. Lines under an item belong to it.
#[must_use]
pub fn parts(message: &str) -> Option<(String, Vec<String>)> {
    let mut around: Vec<&str> = Vec::new();
    let mut items: Vec<String> = Vec::new();
    let mut in_item = false;
    for line in message.lines() {
        if let Some(text) = item(line) {
            items.push(text.trim().to_owned());
            in_item = true;
        } else if line.trim().is_empty() {
            in_item = false;
            around.push(line);
        } else if in_item && line.starts_with(char::is_whitespace) {
            if let Some(last) = items.last_mut() {
                last.push('\n');
                last.push_str(line.trim());
            }
        } else {
            in_item = false;
            around.push(line);
        }
    }
    items.retain(|item| !item.is_empty());
    if !(2..=MAX_PARTS).contains(&items.len()) {
        return None;
    }
    Some((around.join("\n").trim().to_owned(), items))
}

/// How one part went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub harness: Option<Harness>,
    /// The harness's name as the rail shows it.
    pub agent: String,
    pub error: Option<String>,
    pub checks: Option<bool>,
}

/// A plan under way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub id: String,
    /// The text around the list, which every part carries.
    pub around: String,
    pub parts: Vec<String>,
    /// The part running now, from zero.
    pub at: usize,
    pub outcomes: Vec<Outcome>,
}

impl Plan {
    #[must_use]
    pub fn new(around: String, parts: Vec<String>) -> Self {
        Self {
            id: format!("plan-{}", atif::now_ms()),
            around,
            parts,
            at: 0,
            outcomes: Vec::new(),
        }
    }

    /// The message that runs part `index`.
    #[must_use]
    pub fn message(&self, index: usize) -> String {
        let mut text = format!(
            "Part {} of {}: {}",
            index + 1,
            self.parts.len(),
            self.parts[index]
        );
        if !self.around.is_empty() {
            text.push_str(&format!("\n\nFrom the whole request: {}", self.around));
        }
        text
    }

    /// What the chat shows when the plan starts.
    #[must_use]
    pub fn outline(&self) -> String {
        let mut text = format!(
            "This is {} separate pieces of work, so I'll do them one at a time:",
            self.parts.len()
        );
        for (index, part) in self.parts.iter().enumerate() {
            text.push_str(&format!(
                "\n{}. {}",
                index + 1,
                part.lines().next().unwrap_or_default()
            ));
        }
        text
    }

    /// The combined report when the last part ends.
    #[must_use]
    pub fn report(&self) -> String {
        let mut text = format!("All {} parts are done:", self.parts.len());
        for (index, (part, outcome)) in self.parts.iter().zip(&self.outcomes).enumerate() {
            let how = match (&outcome.error, outcome.checks) {
                (Some(_), _) => "stopped with an error",
                (None, Some(false)) => "finished, but its checks still fail",
                (None, Some(true)) => "finished, and its checks pass",
                (None, None) => "finished",
            };
            text.push_str(&format!(
                "\n{}. {}: {} ({how})",
                index + 1,
                part.lines().next().unwrap_or_default(),
                outcome.agent
            ));
        }
        text
    }
}

/// The question about a message, under way.
pub struct Asking {
    pub receiver: mpsc::Receiver<Result<f64, String>>,
    pub started: Instant,
    pub around: String,
    pub parts: Vec<String>,
    pub answer: Option<Result<f64, String>>,
}

impl Asking {
    /// Moves the answer in once it is known; returns whether it is.
    pub fn settled(&mut self) -> bool {
        if self.answer.is_none() {
            self.answer = match self.receiver.try_recv() {
                Ok(answer) => Some(answer),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("System One could not be asked.".into()))
                }
                Err(mpsc::TryRecvError::Empty) if self.started.elapsed() >= BUDGET => {
                    Some(Err("System One did not answer in time.".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
        }
        self.answer.is_some()
    }

    /// Waits for the answer.
    pub fn settle(&mut self) {
        if self.answer.is_none() {
            let left = BUDGET.saturating_sub(self.started.elapsed());
            self.answer = Some(
                self.receiver
                    .recv_timeout(left)
                    .unwrap_or_else(|_| Err("System One did not answer in time.".into())),
            );
        }
    }

    /// The plan, when System One judged the message several pieces of work.
    #[must_use]
    pub fn plan(self) -> Option<Plan> {
        matches!(self.answer, Some(Ok(p)) if p >= 0.5).then(|| Plan::new(self.around, self.parts))
    }
}

/// Asks whether `message` is several pieces of work, when its layout is a
/// list; `None` when it is not.
#[must_use]
pub fn ask(earlier: &[crate::router::Said], message: &str) -> Option<Asking> {
    let (around, parts) = parts(message)?;
    let (sender, receiver) = mpsc::channel();
    let request = jev::SystemOneRequest::new(crate::router::state(earlier, message), questions());
    #[cfg(test)]
    {
        if let Some(Some(p)) = TEST_JUDGE.with(std::cell::Cell::get) {
            let _ = sender.send(Ok(p));
        }
        drop(request);
    }
    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            let _ = sender.send(ask_now(request));
        });
    }
    Some(Asking {
        receiver,
        started: Instant::now(),
        around,
        parts,
        answer: None,
    })
}

/// The question, answered or given up on within [`BUDGET`].
///
/// # Errors
/// No door answered in time, or the answer could not be read.
pub fn ask_now(request: jev::SystemOneRequest) -> Result<f64, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let (response, _) = runtime
        .block_on(async {
            tokio::time::timeout(
                BUDGET,
                crate::issue_run::decide::ask(crate::issue_run::Decider::Auto, request),
            )
            .await
        })
        .map_err(|_| "System One did not answer in time.".to_owned())??;
    response
        .noul("multi_part")
        .map(|answer| answer.noul)
        .map_err(|error| error.to_string())
}

/// Whether routing points make plans. `CODER_DISPATCH=off` turns it off.
#[must_use]
pub fn enabled() -> bool {
    #[cfg(test)]
    {
        TEST_JUDGE.with(std::cell::Cell::get).is_some()
    }
    #[cfg(not(test))]
    {
        std::env::var("CODER_DISPATCH").map_or(true, |value| value.trim() != "off")
    }
}

#[cfg(test)]
thread_local! {
    static TEST_JUDGE: std::cell::Cell<Option<Option<f64>>> = const { std::cell::Cell::new(None) };
}

/// Sets this test thread's answer to `multi_part`.
#[cfg(test)]
pub(crate) fn set_test_judge(judge: Option<Option<f64>>) {
    TEST_JUDGE.with(|cell| cell.set(judge));
}

impl crate::App {
    /// Where the next turn sits in the chat's plan, for its record.
    pub(crate) fn plan_part(&self) -> Option<crate::routing_record::PlanPart> {
        let plan = self.plan.as_ref()?;
        Some(crate::routing_record::PlanPart {
            plan: plan.id.clone(),
            part: u32::try_from(plan.at + 1).unwrap_or(u32::MAX),
            parts: u32::try_from(plan.parts.len()).unwrap_or(u32::MAX),
        })
    }

    /// Starts `plan` in place of the held `message`: the message and the
    /// outline show, and the first part runs as its own piece of work.
    pub(crate) fn start_plan(&mut self, message: String, plan: Plan) {
        self.live.entries.push(crate::live::Entry::User(message));
        self.live.entries.push(crate::live::Entry::Assistant {
            text: plan.outline(),
            model: None,
            elapsed_ms: None,
        });
        self.history.dirty = true;
        self.plan = Some(plan);
        self.send_part();
    }

    /// Sends the plan's current part as a new piece of work, routed on its
    /// own.
    fn send_part(&mut self) {
        let Some(plan) = &self.plan else {
            return;
        };
        let text = plan.message(plan.at);
        let typed = std::mem::take(&mut self.draft);
        let typed_composer = std::mem::take(&mut self.composer);
        self.draft.cursor = text.len();
        self.draft.text = text;
        self.fresh_work = true;
        self.sending_part = true;
        self.submit_live();
        self.sending_part = false;
        self.fresh_work = false;
        self.draft = typed;
        self.composer = typed_composer;
    }

    /// After a part's turn (and any checkpoint call) ends: notes how it
    /// went, then runs the next part or shows the combined report.
    pub(crate) fn advance_plan(&mut self, error: Option<&str>) {
        if self.plan.is_none() || self.live.busy || self.pending_escalation.is_some() {
            return;
        }
        let harness = self.live.runs_on;
        let agent = harness.map_or_else(String::new, |harness| {
            crate::escalate::harness_words(self, harness)
        });
        let checks =
            crate::routing_record::turn_checks(crate::escalate::last_turn(&self.live.entries));
        let Some(plan) = self.plan.as_mut() else {
            return;
        };
        plan.outcomes.push(Outcome {
            harness,
            agent,
            error: error.map(str::to_owned),
            checks,
        });
        plan.at += 1;
        if plan.at < plan.parts.len() {
            self.send_part();
            return;
        }
        let report = plan.report();
        self.plan = None;
        self.live.entries.push(crate::live::Entry::Assistant {
            text: report,
            model: None,
            elapsed_ms: None,
        });
        self.history.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_of_two_or_more_items_is_a_plan_and_prose_is_not() {
        let (around, items) = parts(
            "Please do these:\n1. Add a --verbose flag\n   to the CLI\n2) Fix the typo in README\n- Bump the version\n\nThanks.",
        )
        .unwrap();
        assert_eq!(
            items,
            [
                "Add a --verbose flag\nto the CLI",
                "Fix the typo in README",
                "Bump the version"
            ]
        );
        assert_eq!(around, "Please do these:\n\nThanks.");
        assert!(parts("Fix the bug in main.rs and add a test.").is_none());
        assert!(parts("- only one item").is_none());
        assert!(parts("    - indented code\n    - more code").is_none());
        assert!(parts("2026. was a year\n100. too many digits").is_none());
    }

    #[test]
    fn a_plan_names_each_part_and_reports_each_outcome() {
        let mut plan = Plan::new(
            "In the CLI:".into(),
            vec!["Add a flag".into(), "Fix a typo".into()],
        );
        assert_eq!(
            plan.message(1),
            "Part 2 of 2: Fix a typo\n\nFrom the whole request: In the CLI:"
        );
        assert!(plan.outline().contains("\n1. Add a flag\n2. Fix a typo"));
        plan.outcomes = vec![
            Outcome {
                harness: Some(Harness::ClaudeCode),
                agent: "Claude (Haiku 5.5)".into(),
                error: None,
                checks: Some(true),
            },
            Outcome {
                harness: Some(Harness::CoderLoop),
                agent: "Coder".into(),
                error: Some("boom".into()),
                checks: None,
            },
        ];
        assert_eq!(
            plan.report(),
            "All 2 parts are done:\n1. Add a flag: Claude (Haiku 5.5) (finished, and its checks pass)\n2. Fix a typo: Coder (stopped with an error)"
        );
    }
}
