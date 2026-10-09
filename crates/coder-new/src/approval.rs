//! The approval gate for agent-driven chats (#10752).
//!
//! A workshop agent or studio seat that runs its work through Coder keeps
//! the host's approval policy: `openagents coder chat --approvals stdin`
//! classifies each command before it runs with the workshop agent's effect
//! classes ([`coder::task::agent::effect`]). A read-only command runs, a
//! deny-listed command is refused, and every other command waits for the
//! person's answer. The chat streams the question as an `approval` event,
//! and the answer arrives as one line on standard input:
//! `confirm ID` or `reject ID`, or `{"approval":ID,"decision":"confirm"}`.
//! When standard input ends, every open and later question is rejected.
//!
//! The gate is process-wide while a gated chat runs, because the commands
//! run on the plugin runtime's worker thread; a chat without the flag
//! leaves it unset, and nothing changes.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Value, json};

/// How often a waiting question looks for its answer.
const POLL: Duration = Duration::from_millis(50);

/// What the gate decided about one command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It runs.
    Run,
    /// It does not run; the text is what the model reads instead of its
    /// output.
    Refused(String),
}

/// Where questions wait for the person's answers.
#[derive(Debug, Default)]
pub struct Desk {
    next: AtomicU64,
    events: Mutex<VecDeque<Value>>,
    answers: Mutex<BTreeMap<u64, Option<bool>>>,
    closed: AtomicBool,
    tool_free: bool,
}

impl Desk {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// A host charter that disables all model tools, including reads.
    pub fn tool_free() -> Arc<Self> {
        Arc::new(Self {
            tool_free: true,
            ..Self::default()
        })
    }

    /// Takes one answer line. Blank lines are ignored.
    ///
    /// # Errors
    /// The line is neither `confirm ID`, `reject ID`, nor the JSON form.
    pub fn answer(&self, line: &str) -> Result<(), String> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        let parsed = if line.starts_with('{') {
            serde_json::from_str::<Value>(line).ok().and_then(|value| {
                let id = value.get("approval").and_then(|id| {
                    id.as_u64()
                        .or_else(|| id.as_str().and_then(|text| text.parse().ok()))
                })?;
                let confirm = match value.get("decision").and_then(Value::as_str)? {
                    "confirm" => true,
                    "reject" => false,
                    _ => return None,
                };
                Some((id, confirm))
            })
        } else {
            let mut words = line.split_whitespace();
            match (words.next(), words.next().and_then(|id| id.parse().ok())) {
                (Some("confirm"), Some(id)) => Some((id, true)),
                (Some("reject"), Some(id)) => Some((id, false)),
                _ => None,
            }
        };
        let (id, confirm) = parsed.ok_or("Answer with `confirm ID` or `reject ID`.".to_string())?;
        let mut answers = self
            .answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let pending = answers
            .get_mut(&id)
            .filter(|answer| answer.is_none())
            .ok_or("That approval is not pending.".to_string())?;
        if self.closed.load(Ordering::SeqCst) {
            return Err("The approval desk is closed.".into());
        }
        *pending = Some(confirm);
        Ok(())
    }

    /// No more answers will come: open and later questions are rejected.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    /// The events waiting to be streamed, oldest first.
    pub fn drain(&self) -> Vec<Value> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    fn push(&self, event: Value) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(event);
    }

    fn begin(&self, mut event: Value) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, None);
        event["event"] = json!("approval");
        event["id"] = json!(id);
        self.push(event);
        id
    }

    fn answered(&self, id: u64) -> Option<bool> {
        self.answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .copied()
            .flatten()
    }

    fn finish(&self, id: u64, confirm: bool) {
        self.answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        self.push(json!({"event":"approval_answered","id":id,
            "decision":if confirm {"confirm"} else {"reject"}}));
    }

    /// Waits for the owner's exact outbound disclosure decision. This never
    /// uses a command's read-only classification or grants other effects.
    pub(crate) async fn disclose(
        &self,
        recipient: &str,
        input: Value,
        cancel: &AtomicBool,
        still_valid: impl Fn() -> bool,
    ) -> bool {
        if self.closed.load(Ordering::SeqCst) || cancel.load(Ordering::Relaxed) || !still_valid() {
            return false;
        }
        let id = self.begin(json!({"kind":"disclosure","recipient":recipient,"input":input,
            "why":"Send exactly this lookup input to Brainstorm? No files or conversation are added."}));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        let confirm = loop {
            if self.closed.load(Ordering::SeqCst)
                || cancel.load(Ordering::Relaxed)
                || !still_valid()
                || tokio::time::Instant::now() >= deadline
            {
                break false;
            }
            if let Some(confirm) = self.answered(id) {
                break confirm;
            }
            tokio::time::sleep(POLL).await;
        };
        self.finish(id, confirm);
        confirm
    }

    /// Waits for the owner's answer to one action on another computer
    /// (the `computer` tool): a command that changes it, or a file that
    /// replaces one there. The question names the computer, the exact
    /// action, and why it asks; nothing else is approved by the answer.
    pub(crate) async fn confirm_computer(
        &self,
        host: &str,
        action: &str,
        why: &str,
        cancel: &AtomicBool,
    ) -> bool {
        if self.closed.load(Ordering::SeqCst) || cancel.load(Ordering::Relaxed) {
            return false;
        }
        let id = self.begin(json!({"kind":"computer","host":host,"command":action,"why":why}));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        let confirm = loop {
            if self.closed.load(Ordering::SeqCst)
                || cancel.load(Ordering::Relaxed)
                || tokio::time::Instant::now() >= deadline
            {
                break false;
            }
            if let Some(confirm) = self.answered(id) {
                break confirm;
            }
            tokio::time::sleep(POLL).await;
        };
        self.finish(id, confirm);
        confirm
    }

    /// Asks about `command`, which needs approval for `why`, and waits for
    /// the answer. `cancel` ends the wait as a rejection.
    pub fn ask(&self, command: &str, why: &str, cancel: &AtomicBool) -> bool {
        let id = self.begin(json!({"command":command,"why":why}));
        let confirm = loop {
            if let Some(confirm) = self.answered(id) {
                break confirm;
            }
            if self.closed.load(Ordering::SeqCst) || cancel.load(Ordering::Relaxed) {
                break false;
            }
            std::thread::sleep(POLL);
        };
        self.finish(id, confirm);
        confirm
    }
}

/// The gate a gated chat installs: the desk and the chat's cancel flag.
#[derive(Clone, Debug)]
pub struct Gate {
    pub desk: Arc<Desk>,
    pub cancel: Arc<AtomicBool>,
}

fn slot() -> &'static Mutex<Option<Gate>> {
    static GATE: OnceLock<Mutex<Option<Gate>>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(None))
}

/// Installs `gate` for this process, or removes it with `None`.
pub fn install(gate: Option<Gate>) {
    *slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = gate;
}

/// A mutex every test that installs a gate, or asserts what only holds
/// without one, holds for its whole body; the gate is process-global, so
/// parallel tests race without it.
#[cfg(test)]
pub(crate) fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn current() -> Option<Gate> {
    slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub(crate) fn desk() -> Option<Arc<Desk>> {
    current().map(|gate| gate.desk)
}

/// Whether a gated chat runs in this process.
#[must_use]
pub fn gated() -> bool {
    current().is_some()
}

/// The gate's verdict on `command`: always [`Verdict::Run`] when no gated
/// chat runs. Blocks while the person decides.
#[must_use]
pub fn check(command: &str) -> Verdict {
    let Some(gate) = current() else {
        return Verdict::Run;
    };
    verdict(&gate, command)
}

/// Whether the current host charter permits model tools.
pub(crate) fn tools_allowed() -> bool {
    current().is_none_or(|gate| !gate.desk.tool_free)
}

fn verdict(gate: &Gate, command: &str) -> Verdict {
    if gate.desk.tool_free {
        return Verdict::Refused(
            "The host refuses all model tools under this crew charter.".into(),
        );
    }
    use coder::task::agent::{Effect, effect};
    match effect(command) {
        Effect::ReadOnly => Verdict::Run,
        Effect::Denied(why) => Verdict::Refused(format!(
            "The host refuses this command ({why}). Do not try it another way."
        )),
        Effect::Approval(why) => {
            if gate.desk.ask(command, &why, &gate.cancel) {
                Verdict::Run
            } else {
                Verdict::Refused(
                    "The owner rejected this command. Do not run it or work around it; \
                     continue without it or finish and say what you would have done."
                        .into(),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> Gate {
        Gate {
            desk: Desk::new(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn read_only_commands_run_and_deny_listed_commands_are_refused_without_asking() {
        let gate = gate();
        assert_eq!(verdict(&gate, "git status"), Verdict::Run);
        assert_eq!(verdict(&gate, "cargo test -p atif"), Verdict::Run);
        assert!(matches!(
            verdict(&gate, "rm -rf /"),
            Verdict::Refused(why) if why.contains("refuses")
        ));
        assert!(gate.desk.drain().is_empty());
    }

    #[test]
    fn a_change_waits_for_the_answer_on_the_desk() {
        let gate = gate();
        let desk = gate.desk.clone();
        let answering = std::thread::spawn(move || {
            loop {
                let events = desk.drain();
                if let Some(event) = events.first() {
                    assert_eq!(event["event"], "approval");
                    assert_eq!(event["command"], "touch notes.txt");
                    desk.answer(&format!("confirm {}", event["id"])).unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        assert_eq!(verdict(&gate, "touch notes.txt"), Verdict::Run);
        answering.join().unwrap();
        let answered = gate.desk.drain();
        assert_eq!(answered[0]["event"], "approval_answered");
        assert_eq!(answered[0]["decision"], "confirm");

        let desk = gate.desk.clone();
        let answering = std::thread::spawn(move || {
            loop {
                if let Some(event) = desk
                    .drain()
                    .into_iter()
                    .find(|event| event["event"] == "approval")
                {
                    desk.answer(&format!(
                        r#"{{"approval":{},"decision":"reject"}}"#,
                        event["id"]
                    ))
                    .unwrap();
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        assert!(matches!(
            verdict(&gate, "rm notes.txt"),
            Verdict::Refused(why) if why.contains("rejected")
        ));
        answering.join().unwrap();
    }

    #[test]
    fn closed_input_or_a_cancel_rejects_and_bad_lines_are_refused() {
        let gate = gate();
        gate.desk.close();
        assert!(matches!(verdict(&gate, "touch a"), Verdict::Refused(_)));
        let gate = super::tests::gate();
        gate.cancel.store(true, Ordering::SeqCst);
        assert!(matches!(verdict(&gate, "touch a"), Verdict::Refused(_)));
        assert!(gate.desk.answer("maybe 1").is_err());
        assert!(gate.desk.answer("   ").is_ok());
    }
}
