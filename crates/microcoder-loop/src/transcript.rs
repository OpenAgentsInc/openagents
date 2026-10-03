//! The run's prompt as an append-only log (#10244).
//!
//! Each step is one model call with no conversation, so a provider's prompt
//! cache can only reuse a step's prompt when the step before it sent the
//! same text first. A prompt rebuilt from the current state each step
//! changes near its start (the files in view, Jev's judgments), and the
//! routed Claude runs of the shadow baseline wrote 85 to 88% of their input
//! to the cache at 1.25 times the input price and read back only 12 to 15%
//! (docs/cost/2026-10-02-shadow-baseline-measurement.md).
//!
//! [`Transcript`] builds each step's prompt as the previous step's prompt,
//! byte for byte, with only new text after it:
//!
//! ```text
//! # Task, # Instruction, # Environment, layout note     fixed for the run
//! # State before step 1   files, tests, Jev, knowledge, notes
//! # Step 1                rationale, commands, output
//! # State before step 2   only what changed is repeated
//! ...
//! ```
//!
//! A file, the acceptance tests, or the knowledge section that is the same
//! as when last shown says so instead of repeating itself. When the log
//! passes [`MAX_CHARS`], or the fixed head changes, it starts again from a
//! compact rendering of the state (one cache miss).
//!
//! The Claude lane sends the log as one content block per step and marks
//! the last for caching ([`crate::claude`]), so each call reads every
//! earlier step from the cache and writes only its own.

use std::collections::HashMap;

use crate::state::State;

/// The longest log, in bytes, before it starts again compacted: about
/// 150,000 tokens.
pub const MAX_CHARS: usize = 600_000;

/// How the log reads, told once after the task.
pub const LAYOUT: &str = "This prompt is a log, oldest first, and grows at its end. After the \
task, each step adds the state it starts from (# State before step N: the files in view, read \
after the last step's commands ran, Jev's judgments, and notes) and then what it ran and printed \
(# Step N). The last state section is current; a part of it that hasn't changed since an earlier \
state section says so instead of repeating it.";

/// The heading every state section starts with.
pub const STATE_HEADING: &str = "# State before step ";

/// What a step's prompt is built from, besides the state.
pub struct Inputs<'a> {
    pub user_prompt: &'a str,
    /// Jev's judgments, rendered.
    pub jev: &'a str,
    /// The knowledge section, when the knowledge base is on.
    pub knowledge: Option<&'a str>,
    /// Whether the run writes and freezes acceptance tests.
    pub acceptance: bool,
    /// The step the prompt is for.
    pub step: usize,
}

/// The log, and what it has shown so far.
#[derive(Debug, Default)]
pub struct Transcript {
    text: String,
    head: String,
    /// The number of the state's actions the log holds.
    actions: usize,
    /// The number of the person's messages the log holds.
    steering: usize,
    /// Each file as last shown, and the step whose state section showed it.
    files: HashMap<String, (Option<String>, usize)>,
    tests: Option<(String, usize)>,
    knowledge: Option<(String, usize)>,
    /// How many times the log started again.
    restarts: usize,
}

impl Transcript {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many times the log started again after its first step.
    #[must_use]
    pub fn restarts(&self) -> usize {
        self.restarts.saturating_sub(1)
    }

    /// The prompt for `inputs.step`: the previous prompt with the actions
    /// since and the current state appended.
    pub fn next(&mut self, state: &State, inputs: &Inputs<'_>) -> String {
        let head = head(state, inputs.user_prompt);
        if head != self.head || self.text.len() > MAX_CHARS || self.actions > state.actions.len() {
            self.restart(head, state);
        }
        // The state section a new action follows was shown last step;
        // the action comes after it.
        for action in &state.actions[self.actions..] {
            self.text.push_str(&format!(
                "\n\n# Step {}\n\n{}",
                action.step,
                action.render().trim_end()
            ));
        }
        self.actions = state.actions.len();
        self.push_state(state, inputs);
        self.text.clone()
    }

    fn restart(&mut self, head: String, state: &State) {
        self.restarts += 1;
        self.text.clone_from(&head);
        self.head = head;
        if !state.actions.is_empty() {
            self.text.push_str("\n\n# Steps so far\n\n");
            self.text.push_str(state.render_actions().trim_end());
        }
        self.actions = state.actions.len();
        self.steering = 0;
        self.files.clear();
        self.tests = None;
        self.knowledge = None;
    }

    fn push_state(&mut self, state: &State, inputs: &Inputs<'_>) {
        let step = inputs.step;
        let mut out = format!("\n\n{STATE_HEADING}{step}\n\n## Files in view\n\n");
        if state.files.is_empty() {
            out.push_str("None. List paths in `view` to see files here.");
        }
        let mut files = Vec::new();
        for (path, contents) in &state.files {
            files.push(match self.files.get(path) {
                Some((shown, at)) if shown == contents => {
                    format!("### {path}\n\nUnchanged since the state before step {at}.")
                }
                _ => {
                    self.files.insert(path.clone(), (contents.clone(), step));
                    match contents {
                        Some(text) => format!("### {path}\n\n```\n{}\n```", text.trim_end()),
                        None => format!("### {path}\n\n(no such file)"),
                    }
                }
            });
        }
        out.push_str(&files.join("\n\n"));
        if inputs.acceptance {
            let tests = state.render_tests(crate::run::ACCEPT_DIR);
            out.push_str("\n\n## Acceptance tests\n\n");
            out.push_str(unchanged_or(&mut self.tests, tests, step).trim_end());
        }
        out.push_str("\n\n## Jev's judgments of the current state\n\n");
        out.push_str(inputs.jev.trim_end());
        if let Some(text) = inputs.knowledge {
            out.push_str("\n\n## Knowledge base\n\n");
            out.push_str(unchanged_or(&mut self.knowledge, text.to_owned(), step).trim_end());
        }
        if state.steering.len() > self.steering {
            out.push_str(
                "\n\n## Messages from the user while you worked\n\nThey come after the task; \
                 where they differ from it, they are what the user wants now.\n",
            );
            for (before, text) in &state.steering[self.steering..] {
                out.push_str(&format!("\n- Before step {before}: {text}"));
            }
            self.steering = state.steering.len();
        }
        if !state.notes.is_empty() {
            out.push_str("\n\n## Notes from the host\n");
            for note in &state.notes {
                out.push_str(&format!("\n- {note}"));
            }
        }
        self.text.push_str(&out);
    }
}

/// `text`, or a line saying it is unchanged since the step that last showed
/// it.
fn unchanged_or(last: &mut Option<(String, usize)>, text: String, step: usize) -> String {
    match last {
        Some((shown, at)) if *shown == text => {
            format!("Unchanged since the state before step {at}.")
        }
        _ => {
            *last = Some((text.clone(), step));
            text
        }
    }
}

/// The fixed head of the log.
fn head(state: &State, user_prompt: &str) -> String {
    format!(
        "# Task\n\n{}\n\n# Instruction\n\n{user_prompt}\n\n# Environment\n\n{}\n\n# How this prompt is laid out\n\n{LAYOUT}",
        state.task.trim_end(),
        state.environment.trim_end()
    )
}

/// The current state section of a prompt: everything after its last
/// [`STATE_HEADING`].
#[must_use]
pub fn current(prompt: &str) -> &str {
    prompt
        .rfind(STATE_HEADING)
        .map_or(prompt, |at| &prompt[at..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Action, CommandResult};

    fn inputs(step: usize, jev: &str) -> Inputs<'_> {
        Inputs {
            user_prompt: "Do the task.",
            jev,
            knowledge: None,
            acceptance: false,
            step,
        }
    }

    fn ran(step: usize, output: &str) -> Action {
        Action {
            step,
            rationale: format!("reason {step}"),
            results: vec![CommandResult {
                command: format!("echo {step}"),
                exit: Some(0),
                timed_out: false,
                seconds: 0.1,
                output: output.to_owned(),
            }],
            skipped: Vec::new(),
        }
    }

    #[test]
    fn each_prompt_starts_with_the_previous_one_byte_for_byte() {
        let mut state = State {
            task: "Fix the bug.".into(),
            environment: "cwd: /repo".into(),
            files: vec![("a.py".into(), Some("one".into()))],
            ..State::default()
        };
        let mut log = Transcript::new();
        let first = log.next(&state, &inputs(1, "- done: probability 0.10"));
        assert!(first.starts_with("# Task\n\nFix the bug."));
        state.actions.push(ran(1, "printed one"));
        state.notes.push("a note".into());
        let second = log.next(&state, &inputs(2, "- done: probability 0.40"));
        assert!(second.starts_with(&first), "step 2 extends step 1");
        state.actions.push(ran(2, "printed two"));
        state.files = vec![("a.py".into(), Some("two".into())), ("b.py".into(), None)];
        state.notes.clear();
        state.steering.push((3, "use b.py".into()));
        let third = log.next(&state, &inputs(3, "- done: probability 0.70"));
        assert!(third.starts_with(&second), "step 3 extends step 2");
        let fourth = log.next(&state, &inputs(4, "- done: probability 0.90"));
        assert!(fourth.starts_with(&third));

        // The steps run in order after the state each started from.
        let at = |text: &str| fourth.find(text).unwrap();
        assert!(at("# State before step 1") < at("# Step 1\n"));
        assert!(at("# Step 1\n") < at("# State before step 2"));
        assert!(at("# State before step 2") < at("# Step 2\n"));
        // The current section repeats only what changed.
        let now = current(&fourth);
        assert!(now.starts_with("# State before step 4"));
        assert!(now.contains("### a.py\n\nUnchanged since the state before step 3."));
        assert!(now.contains("### b.py\n\nUnchanged since the state before step 3."));
        assert!(now.contains("probability 0.90"));
        assert!(!now.contains("use b.py"), "a message is shown once");
        assert!(current(&third).contains("### a.py\n\n```\ntwo\n```"));
        assert!(current(&third).contains("- Before step 3: use b.py"));
        assert!(current(&second).contains("Unchanged since the state before step 1."));
        assert!(current(&second).contains("- a note"));
        assert!(!current(&third).contains("a note"));
        assert_eq!(log.restarts(), 0);
    }

    #[test]
    fn a_changed_head_or_a_long_log_starts_again_compacted() {
        let mut state = State {
            task: "Fix the bug.".into(),
            ..State::default()
        };
        let mut log = Transcript::new();
        let first = log.next(&state, &inputs(1, "j"));
        for step in 1..=30 {
            state.actions.push(ran(step, &"x".repeat(30_000)));
        }
        let second = log.next(&state, &inputs(31, "j"));
        assert!(second.starts_with(&first));
        assert!(second.len() > MAX_CHARS);
        state.actions.push(ran(31, "short"));
        let third = log.next(&state, &inputs(32, "j"));
        assert!(!third.starts_with(&second), "the long log started again");
        assert!(third.contains("# Steps so far"));
        assert!(third.len() < MAX_CHARS);
        assert_eq!(log.restarts(), 1);
        let fourth = log.next(&state, &inputs(33, "j"));
        assert!(fourth.starts_with(&third));
        state.task = "Another task.".into();
        let fifth = log.next(&state, &inputs(34, "j"));
        assert!(fifth.starts_with("# Task\n\nAnother task."));
        assert_eq!(log.restarts(), 2);
    }
}
