//! System One hooks as decision hooks (`docs/coder/design/system-one-hooks.md`),
//! declared once and run on every backend at the tier it gives:
//!
//! - [`monitor`] (#11488, idea 3): after each tool, continue, steer with
//!   one line the host observed, fetch evidence with a host probe, or
//!   escalate (end the session so the host re-briefs a stronger executor);
//! - [`route`] (#11491, idea 6): each later message in a chat continues
//!   the session, is answered by the host, or starts a re-briefed session.
//!
//! Both are rule first: code reads the events and asks the Decisions API
//! only when its rule finds something to judge. Everything they add is
//! evidence the host observed, never advice.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use jev::decisions::{ChoiceOption, DecisionQuestion};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::decide::Decider;
use super::decision::{Acted, Costs, DecisionHook, Recorder, Ruled, Timing, choice_p};
use super::events::{Call, Event};
use super::outcome::{Context, HookOutcome, Permission, Stop};
use super::registry::Disclosure;

fn choice(name: &str, instructions: &str, options: &[(&str, &str)]) -> DecisionQuestion {
    DecisionQuestion::Choice {
        name: name.to_owned(),
        instructions: instructions.to_owned(),
        choices: options
            .iter()
            .map(|(value, description)| ChoiceOption {
                value: (*value).to_owned(),
                description: Some((*description).to_owned()),
            })
            .collect(),
    }
}

/// The first `head` and last `tail` characters of `text`.
fn ends(text: &str, head: usize, tail: usize) -> String {
    let count = text.chars().count();
    if count <= head + tail {
        return text.to_owned();
    }
    let first: String = text.chars().take(head).collect();
    let last: String = text.chars().skip(count - tail).collect();
    format!("{first}\n…\n{last}")
}

pub mod monitor {
    //! The in-session monitor (#11488), from the calibrated stall
    //! detector's rules (`crates/coder-one/src/stall.rs`): code counts
    //! repeated failures, repeated commands and calls without an edit over
    //! the last [`WINDOW`] tool calls, and only a suspect window is put to
    //! the Decisions API, at most once every [`EVERY`] calls. It runs
    //! async: the answer lands on the next tool call and never holds one up.

    use super::*;

    pub const ID: &str = "monitor";
    /// The question set's version: any wording change is a new one.
    pub const QUESTION_SET: &str = "monitor-v1";
    /// Tool calls the rule and the question read.
    pub const WINDOW: usize = 12;
    /// Calls without an edit that make a session quiet (stall.rs's
    /// `QUIET_TURNS`).
    pub const QUIET_CALLS: usize = 8;
    /// Failed calls in the window repeating an earlier failure that make
    /// it look like a loop (stall.rs's `REPEATED_FAILURES`).
    pub const REPEATED_FAILURES: usize = 2;
    /// Calls in the window repeating an earlier one (stall.rs's
    /// `REPEATED_COMMANDS`).
    pub const REPEATED_COMMANDS: usize = 4;
    /// Calls between two questions.
    pub const EVERY: usize = 8;
    /// How long one decision may take.
    pub const DEADLINE: Duration = Duration::from_secs(3);

    /// Escalating ends the session and re-briefs a stronger executor: a
    /// needless one throws away the session's context (10 minutes); a
    /// missed one lets a stall run on (3). Escalate past 0.77.
    pub const ESCALATE: Costs = Costs {
        wrong_act: 10.0,
        missed: 3.0,
    };
    /// A steer is one line re-read on every later call (1); a missed one
    /// leaves the agent blind to what the host saw (2). Steer past 0.33.
    pub const STEER: Costs = Costs {
        wrong_act: 1.0,
        missed: 2.0,
    };

    const QUESTION: &str = "Read the agent's recent tool calls in the input. Which action should \
the host take now? The host only adds facts it observed; it never gives advice.";
    const OPTIONS: [(&str, &str); 4] = [
        (
            "continue",
            "The agent is making progress or exploring sensibly; add nothing.",
        ),
        (
            "steer",
            "The agent repeats a failing step or misses something the outputs show; tell it the one fact the host observed.",
        ),
        (
            "fetch_evidence",
            "The agent seems unsure what changed; the host should show the working tree's current changes.",
        ),
        (
            "escalate",
            "The agent is stuck in a loop it will not leave; end the session so a stronger agent starts fresh with the evidence.",
        ),
    ];

    /// What the monitor sends to the decision model.
    #[must_use]
    pub fn disclosure() -> Disclosure {
        Disclosure {
            sends: "the session's recent tool calls and their outputs".to_owned(),
            to: "the Decisions API".to_owned(),
        }
    }

    /// One tool call, as the monitor reads it.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Seen {
        pub tool: String,
        /// The command, or the path: what makes two calls the same.
        pub key: String,
        pub failed: bool,
        pub edit: bool,
        /// The head and tail of its output.
        pub output: String,
    }

    const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

    impl Seen {
        /// A `PostToolUse` call: the tool's input and response, normalized
        /// by the backend (`failed` set by both).
        #[must_use]
        pub fn of(call: &Call) -> Seen {
            let tool = call.tool.clone().unwrap_or_default();
            let input = &call.input["tool_input"];
            let key = ["command", "cmd", "file_path", "path", "pattern"]
                .iter()
                .find_map(|field| input.get(*field).and_then(Value::as_str))
                .map_or_else(|| input.to_string(), str::to_owned);
            let response = &call.input["tool_response"];
            let output = match response {
                Value::String(text) => text.clone(),
                Value::Null => String::new(),
                other => ["output", "stdout", "error", "stderr"]
                    .iter()
                    .filter_map(|field| other.get(*field).and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            let failed = call.input["failed"].as_bool().unwrap_or(false)
                || response["exit"].as_i64().is_some_and(|exit| exit != 0)
                || response["is_error"].as_bool() == Some(true);
            Seen {
                edit: EDIT_TOOLS.contains(&tool.as_str()) && !failed,
                tool,
                key,
                failed,
                output: ends(&output, 300, 500),
            }
        }
    }

    /// What code reads from the window.
    #[derive(Clone, Debug, Default, PartialEq, Serialize)]
    pub struct Features {
        pub calls: usize,
        /// Calls since the last edit; none when the harness shows no edits
        /// (the Coder loop edits through commands).
        pub calls_since_edit: Option<usize>,
        pub failed_in_window: usize,
        pub repeated_failures: usize,
        pub repeated_commands: usize,
        /// The most repeated failing call, and how often it failed.
        pub worst: Option<(String, usize)>,
    }

    impl Features {
        #[must_use]
        pub fn of(seen: &[Seen], edits_tracked: bool) -> Features {
            let window = &seen[seen.len().saturating_sub(WINDOW)..];
            let mut features = Features {
                calls: seen.len(),
                calls_since_edit: edits_tracked
                    .then(|| seen.iter().rev().take_while(|call| !call.edit).count()),
                ..Features::default()
            };
            let mut failures: Vec<(String, usize)> = Vec::new();
            for (i, call) in window.iter().enumerate() {
                let earlier = &window[..i];
                if earlier.iter().any(|other| other.key == call.key) {
                    features.repeated_commands += 1;
                }
                if call.failed {
                    features.failed_in_window += 1;
                    if earlier
                        .iter()
                        .any(|other| other.failed && other.key == call.key)
                    {
                        features.repeated_failures += 1;
                    }
                    match failures.iter_mut().find(|(key, _)| *key == call.key) {
                        Some((_, n)) => *n += 1,
                        None => failures.push((call.key.clone(), 1)),
                    }
                }
            }
            features.worst = failures.into_iter().max_by_key(|(_, n)| *n);
            features
        }

        /// The same failure, or the same call, again and again.
        #[must_use]
        pub fn looping(&self) -> bool {
            self.repeated_failures >= REPEATED_FAILURES
                || self.repeated_commands >= REPEATED_COMMANDS
        }

        /// No edit for a while, with failures in view.
        #[must_use]
        pub fn quiet(&self) -> bool {
            self.calls_since_edit.is_some_and(|n| n >= QUIET_CALLS) && self.failed_in_window > 0
        }

        /// Whether code finds a stall plausible, so a decision matters.
        #[must_use]
        pub fn suspect(&self) -> bool {
            self.looping() || self.quiet()
        }

        /// What the host observed, in plain facts: the one line a steer adds.
        #[must_use]
        pub fn evidence(&self) -> String {
            let mut facts = Vec::new();
            if let Some((key, n)) = &self.worst
                && *n >= 2
            {
                facts.push(format!(
                    "`{}` failed {n} times in the last {WINDOW} calls",
                    ends(key, 80, 0)
                ));
            }
            if self.repeated_commands >= REPEATED_COMMANDS {
                facts.push(format!(
                    "{} of the last {WINDOW} calls repeat an earlier call",
                    self.repeated_commands
                ));
            }
            if let Some(n) = self.calls_since_edit
                && n >= QUIET_CALLS
            {
                facts.push(format!("no file was changed in the last {n} calls"));
            }
            format!("Host observed: {}.", facts.join("; "))
        }
    }

    /// The host's probe for `fetch_evidence`: its output, or none.
    pub type Probe = Arc<dyn Fn() -> Option<String> + Send + Sync>;

    #[derive(Default)]
    struct Memory {
        seen: Vec<Seen>,
        asked_at: Option<usize>,
    }

    /// The monitor for one session. `edits_tracked`: the harness shows file
    /// edits as tools (Claude Code), so calls without one count.
    #[must_use]
    pub fn hook(
        edits_tracked: bool,
        probe: Option<Probe>,
        decider: Arc<dyn Decider>,
        recorder: Recorder,
    ) -> DecisionHook {
        let memory: Arc<Mutex<Memory>> = Arc::default();
        DecisionHook {
            id: ID.to_owned(),
            event: Event::PostToolUse,
            matcher: None,
            question_set: QUESTION_SET.to_owned(),
            model: super::super::decision::MODEL.to_owned(),
            disclosure: disclosure(),
            costs: ESCALATE,
            timing: Timing::Async,
            deadline: DEADLINE,
            rule: Arc::new(move |call: &Call| {
                let mut memory = memory.lock().unwrap_or_else(PoisonError::into_inner);
                memory.seen.push(Seen::of(call));
                let features = Features::of(&memory.seen, edits_tracked);
                let calls = memory.seen.len();
                if !features.suspect() || memory.asked_at.is_some_and(|at| calls - at < EVERY) {
                    return Ruled::Pass;
                }
                memory.asked_at = Some(calls);
                let window = &memory.seen[calls.saturating_sub(WINDOW)..];
                Ruled::Unsure {
                    input: render(window, &features),
                    questions: vec![choice("next", QUESTION, &OPTIONS)],
                    memo: json!({"features": features, "evidence": features.evidence()}),
                }
            }),
            act: Arc::new(move |memo: &Value, response| act(memo, response, probe.as_ref())),
            fallback: HookOutcome::none(),
            decider,
            recorder,
            shadow: false,
        }
    }

    /// The window as the decision model reads it.
    #[must_use]
    pub fn render(window: &[Seen], features: &Features) -> String {
        let mut out = String::from("Recent tool calls, oldest first:\n");
        for (i, call) in window.iter().enumerate() {
            out.push_str(&format!(
                "\n{}. {} `{}`{}\n{}\n",
                i + 1,
                call.tool,
                ends(&call.key, 200, 0),
                if call.failed { " (failed)" } else { "" },
                ends(&call.output, 200, 300)
            ));
        }
        out.push_str(&format!("\n{}", features.evidence()));
        out
    }

    fn act(
        memo: &Value,
        response: &jev::decisions::DecisionsResponse,
        probe: Option<&Probe>,
    ) -> Acted {
        let p = |value| choice_p(response, "next", value);
        let evidence = memo["evidence"].as_str().unwrap_or_default().to_owned();
        let escalate = p("escalate");
        if escalate >= ESCALATE.threshold() {
            return Acted {
                outcome: HookOutcome {
                    stop: Stop::End,
                    ..HookOutcome::context(Context::new(ID, &evidence, 2))
                },
                action: "escalate".to_owned(),
                question: "next".to_owned(),
                p: Some(escalate),
                threshold: Some(ESCALATE.threshold()),
            };
        }
        let (steer, fetch) = (p("steer"), p("fetch_evidence"));
        let acting = steer + fetch;
        let mut acted = Acted {
            outcome: HookOutcome::none(),
            action: "continue".to_owned(),
            question: "next".to_owned(),
            p: Some(acting),
            threshold: Some(STEER.threshold()),
        };
        if acting < STEER.threshold() {
            return acted;
        }
        let fetched = (fetch > steer)
            .then(|| probe.and_then(|probe| probe()))
            .flatten();
        (acted.action, acted.outcome) = match fetched {
            Some(found) => (
                "fetch_evidence".to_owned(),
                HookOutcome::context(Context::new(
                    ID,
                    &format!(
                        "Host checked the working tree:\n{}",
                        ends(&found, 1_200, 300)
                    ),
                    1,
                )),
            ),
            None => (
                "steer".to_owned(),
                HookOutcome::context(Context::new(ID, &evidence, 1)),
            ),
        };
        acted
    }
}

pub mod route {
    //! Routing each later chat message (#11491): the first message always
    //! goes to the session; each later one is put to one choice, continue,
    //! host answers, or re-brief. It blocks (a message must be routed
    //! before it is sent), within a short deadline whose fallback is
    //! continue.

    use super::*;

    pub const ID: &str = "route";
    pub const QUESTION_SET: &str = "route-v1";
    pub const DEADLINE: Duration = Duration::from_millis(1_500);
    /// Earlier messages the question reads.
    pub const EARLIER: usize = 3;

    /// A needless re-brief re-reads a fresh briefing and loses what the
    /// session learned (3); a missed one keeps re-reading a stale context
    /// on every later call (1). Re-brief past 0.75.
    pub const REBRIEF: Costs = Costs {
        wrong_act: 3.0,
        missed: 1.0,
    };
    /// A wrong host answer leaves the person without the executor's (4);
    /// a missed one spends a whole turn on what the host knew (1). Answer
    /// past 0.8.
    pub const HOST: Costs = Costs {
        wrong_act: 4.0,
        missed: 1.0,
    };

    const QUESTION: &str = "A person sends a new message in an ongoing chat with a coding agent. \
Which route fits the new message?";
    const OPTIONS: [(&str, &str); 3] = [
        (
            "continue",
            "It continues the current work or depends on what the session already knows.",
        ),
        (
            "host_answers",
            "It asks only about the chat's own state (what ran, its cost, its status), which the host can answer without the agent.",
        ),
        (
            "rebrief",
            "It starts a different task that needs none of the earlier conversation.",
        ),
    ];

    /// What the router sends to the decision model.
    #[must_use]
    pub fn disclosure() -> Disclosure {
        Disclosure {
            sends: "the chat's recent messages".to_owned(),
            to: "the Decisions API".to_owned(),
        }
    }

    /// The host's own answer to a message about the chat, if it has one.
    pub type Answerer = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

    /// The router for one chat.
    #[must_use]
    pub fn hook(
        answerer: Option<Answerer>,
        decider: Arc<dyn Decider>,
        recorder: Recorder,
    ) -> DecisionHook {
        let prompts: Arc<Mutex<Vec<String>>> = Arc::default();
        DecisionHook {
            id: ID.to_owned(),
            event: Event::UserPromptSubmit,
            matcher: None,
            question_set: QUESTION_SET.to_owned(),
            model: super::super::decision::MODEL.to_owned(),
            disclosure: disclosure(),
            costs: REBRIEF,
            timing: Timing::Blocking,
            deadline: DEADLINE,
            rule: Arc::new(move |call: &Call| {
                let prompt = call.input["prompt"].as_str().unwrap_or_default().to_owned();
                let mut prompts = prompts.lock().unwrap_or_else(PoisonError::into_inner);
                let earlier: Vec<String> =
                    prompts.iter().rev().take(EARLIER).rev().cloned().collect();
                prompts.push(prompt.clone());
                if earlier.is_empty() {
                    return Ruled::Pass;
                }
                Ruled::Unsure {
                    input: render(&earlier, &prompt),
                    questions: vec![choice("route", QUESTION, &OPTIONS)],
                    memo: json!({"prompt": prompt}),
                }
            }),
            act: Arc::new(move |memo: &Value, response| {
                let p = |value| choice_p(response, "route", value);
                let prompt = memo["prompt"].as_str().unwrap_or_default();
                let (rebrief, host) = (p("rebrief"), p("host_answers"));
                if rebrief >= REBRIEF.threshold() {
                    return Acted {
                        outcome: HookOutcome::stopping(Stop::End),
                        action: "rebrief".to_owned(),
                        question: "route".to_owned(),
                        p: Some(rebrief),
                        threshold: Some(REBRIEF.threshold()),
                    };
                }
                if host >= HOST.threshold() {
                    let answer = answerer.as_ref().and_then(|answer| answer(prompt));
                    return Acted {
                        action: if answer.is_some() {
                            "host_answers"
                        } else {
                            "host_answers_unavailable"
                        }
                        .to_owned(),
                        outcome: answer.map_or_else(HookOutcome::none, |answer| {
                            HookOutcome::permit(Permission::Deny { reason: answer })
                        }),
                        question: "route".to_owned(),
                        p: Some(host),
                        threshold: Some(HOST.threshold()),
                    };
                }
                Acted {
                    outcome: HookOutcome::none(),
                    action: "continue".to_owned(),
                    question: "route".to_owned(),
                    p: Some(p("continue")),
                    threshold: None,
                }
            }),
            fallback: HookOutcome::none(),
            decider,
            recorder,
            shadow: false,
        }
    }

    /// The messages as the decision model reads them.
    #[must_use]
    pub fn render(earlier: &[String], prompt: &str) -> String {
        let mut out = String::from("Earlier messages, oldest first:\n");
        for (i, text) in earlier.iter().enumerate() {
            out.push_str(&format!("\n{}. {}\n", i + 1, ends(text, 600, 200)));
        }
        out.push_str(&format!("\nNew message:\n{}", ends(prompt, 1_500, 500)));
        out
    }
}

#[cfg(test)]
#[path = "system_one_tests.rs"]
mod tests;
