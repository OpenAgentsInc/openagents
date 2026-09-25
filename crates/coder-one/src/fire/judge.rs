//! The fire loop's judge: code rules that stop a run at once, and a Jev
//! request after each action that compares the run with the winners'
//! strategy card.
//!
//! Code rules need no model: a run past its time budget, a run with no
//! edit long after the winners had one, a model with no action for five
//! minutes, the same command with the same output three times, spend past
//! the limit, or a second refused finish.
//! Jev answers five questions per action: whether the run is on track,
//! which of the card's phases it's in, how it deviates, which known
//! pitfall it shows, and whether it should stop. Code decides the stop
//! from those answers. A judgment votes to stop on a high stop answer, a
//! low on-track answer with a bad deviation, or a named pitfall with a
//! stop answer of 0.7 or more. Two votes among the last three judgments
//! stop the run, and so do four judgments in a row with a stop answer of
//! 0.7 or more.

use serde::Serialize;
use serde_json::{Value, json};

use super::card::Card;
use super::events::{Event, Kind};
use crate::component::jev::{self as jev_component, JevMode};
use crate::record::Recorder;

/// When the judge stops a run.
#[derive(Clone, Debug)]
pub struct Rules {
    /// A run stops past this multiple of the winners' time to done, to
    /// first edit, or to first check.
    pub budget_x: f64,
    /// The fewest seconds before the no-edit rule applies.
    pub min_edit_s: f64,
    /// Dollars of model and Jev spend, counted from the logs.
    pub max_usd: f64,
    /// Jev's stop answer at or above which a judgment votes to stop.
    pub stop_p: f64,
    /// Jev's stop answer at or above which a judgment that names a known
    /// pitfall votes to stop.
    pub pitfall_p: f64,
    /// Seconds with no action from the model that stop the run.
    pub idle_s: f64,
    /// Jev's on-track answer at or below which a judgment with a bad
    /// deviation votes to stop.
    pub off_track_p: f64,
    /// Votes among the last `votes + 1` judgments that stop the run.
    pub votes: usize,
    /// Judgments in a row with a stop answer of `pitfall_p` or more that
    /// stop the run, votes or not.
    pub steady: usize,
    /// Actions before Jev's votes can stop a run.
    pub min_actions: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            budget_x: 4.0,
            min_edit_s: 120.0,
            max_usd: 0.50,
            stop_p: 0.85,
            pitfall_p: 0.7,
            idle_s: 300.0,
            off_track_p: 0.15,
            votes: 2,
            steady: 4,
            min_actions: 5,
        }
    }
}

/// The deviations Jev chooses from, with the words it reads.
pub const DEVIATIONS: [(&str, &str); 7] = [
    (
        "none",
        "The run follows the winners' strategy: it's in one of its phases, in order.",
    ),
    (
        "good",
        "The run departs from the winners' route, but its departure serves the task as well: another way to the same result.",
    ),
    (
        "wrong_goal",
        "The run works on something the task doesn't ask for, or solves a different problem from the one stated.",
    ),
    (
        "skipped_phase",
        "The run skipped a phase the winners needed, such as getting a check that doesn't depend on its own answer, and moves on without it. A phase the host's briefing already covers isn't skipped.",
    ),
    (
        "looping",
        "The run repeats the same kind of action without new information or progress.",
    ),
    (
        "too_slow",
        "The run spends far longer on its current phase than the winners did, without a result that justifies it.",
    ),
    (
        "forbidden",
        "The run does something the task forbids, or claims done while its own checks fail.",
    ),
];

/// Deviations that count against a run.
const BAD: [&str; 5] = [
    "wrong_goal",
    "skipped_phase",
    "looping",
    "too_slow",
    "forbidden",
];

/// One judgment of the run after an action.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Judgment {
    /// Seconds since the run started.
    pub t: f64,
    /// The number of actions so far.
    pub actions: usize,
    pub phase: Option<String>,
    pub on_track: Option<f64>,
    pub deviation: Option<String>,
    pub pitfall: Option<String>,
    pub stop: Option<f64>,
    pub usd: f64,
    pub milliseconds: Option<u64>,
    pub error: Option<String>,
    /// Whether this judgment voted to stop.
    pub vote: bool,
}

/// Why a run stopped.
#[derive(Clone, Debug, Serialize)]
pub struct Stop {
    /// The rule: `over_time`, `no_edit`, `idle`, `repeating`, `spend`,
    /// `finish_refused`, or `jev`.
    pub rule: String,
    /// What the rule saw, in a sentence.
    pub why: String,
    /// Seconds since the run started.
    pub t: f64,
}

/// One action as the judge keeps it.
#[derive(Clone, Debug, Serialize)]
pub struct Action {
    pub t: f64,
    pub tool: String,
    pub input: String,
    pub output: String,
    pub edit: bool,
}

/// What the judge knows about the run so far.
#[derive(Default)]
pub struct Run {
    pub started: Option<u64>,
    pub last: u64,
    pub instruction: String,
    /// The first message of the first model session: the host's
    /// briefing, which can already hold the files a winner read.
    pub briefing: String,
    pub actions: Vec<Action>,
    pub first_edit: Option<f64>,
    pub model_usd: f64,
    pub jev_usd: f64,
    pub judge_usd: f64,
    pub notes: Vec<String>,
    pub refused_finishes: usize,
    pub judgments: Vec<Judgment>,
    pub components: Vec<String>,
    pub ended: bool,
}

impl Run {
    /// Seconds from the run's start to `now`, in milliseconds since the
    /// epoch.
    #[must_use]
    pub fn seconds(&self, now: u64) -> f64 {
        self.started
            .map_or(0.0, |start| now.saturating_sub(start) as f64 / 1000.0)
    }

    /// Takes in one event.
    pub fn see(&mut self, event: &Event) {
        if self.started.is_none() && event.at > 0 {
            self.started = Some(event.at);
        }
        self.last = self.last.max(event.at);
        let t = self.seconds(event.at);
        match &event.kind {
            Kind::Say { source, text } if source == "User" && event.log == "episode" => {
                if self.instruction.is_empty() {
                    self.instruction = text.clone();
                }
            }
            Kind::Say { source, text } if source == "User" => {
                if self.briefing.is_empty() {
                    self.briefing = text.clone();
                }
            }
            Kind::Say { source, text }
                if source == "System" && !text.starts_with("invocation ") =>
            {
                if text.contains("turned this finish back") {
                    self.refused_finishes += 1;
                }
                self.notes.push(format!("[{t:.0}s] {}", clip(text, 400)));
            }
            Kind::Think { usd, .. } => self.model_usd += usd,
            Kind::Jev { input_tokens, .. } => {
                self.jev_usd += input_tokens.map_or(0.0, |tokens| {
                    tokens as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
                });
            }
            Kind::Start { component, .. } => {
                if !self.components.contains(component) {
                    self.components.push(component.clone());
                }
            }
            Kind::End { .. } if event.log == "episode" => self.ended = true,
            Kind::Tool {
                name,
                arguments,
                output,
            } if event.is_action() => {
                let input = input_of(name, arguments);
                let edit = is_edit(name, &input);
                if edit && self.first_edit.is_none() {
                    self.first_edit = Some(t);
                }
                self.actions.push(Action {
                    t,
                    tool: name.clone(),
                    input,
                    output: output.clone(),
                    edit,
                });
            }
            _ => {}
        }
    }

    /// Everything the run has spent that the logs show.
    #[must_use]
    pub fn usd(&self) -> f64 {
        self.model_usd + self.jev_usd
    }
}

/// What a tool call did, in one string.
#[must_use]
pub fn input_of(name: &str, arguments: &Value) -> String {
    match name {
        "run_command" => arguments["command"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        "read_file" => format!(
            "{} {}..{}",
            arguments["path"].as_str().unwrap_or_default(),
            arguments["start_line"].as_u64().unwrap_or(1),
            arguments["end_line"]
                .as_u64()
                .map_or("end".to_string(), |line| line.to_string())
        ),
        "write_file" => format!(
            "{} ({} bytes)",
            arguments["path"].as_str().unwrap_or_default(),
            arguments["contents"].as_str().map_or(0, str::len)
        ),
        "apply_patch" => arguments["patch"]
            .as_str()
            .or_else(|| arguments["input"].as_str())
            .unwrap_or_default()
            .to_string(),
        _ => arguments.to_string(),
    }
}

/// Whether an action changed the workspace: a write or a patch, or a
/// command shaped like an in-place edit.
#[must_use]
pub fn is_edit(name: &str, input: &str) -> bool {
    match name {
        "write_file" | "apply_patch" => true,
        "run_command" => [
            "sed -i", "perl -pi", "cat >", "cat <<", "tee ", "> /app/", ">/app/", "patch ",
        ]
        .iter()
        .any(|shape| input.contains(shape)),
        _ => false,
    }
}

/// The code rules, checked at `now`. The first that holds stops the run.
#[must_use]
pub fn rules(run: &Run, card: &Card, rules: &Rules, now: u64) -> Option<Stop> {
    let t = run.seconds(now);
    let budget = card.budget;
    let x = rules.budget_x;
    if budget.done_s > 0.0 && t > x * budget.done_s {
        return Some(Stop {
            rule: "over_time".to_string(),
            why: format!(
                "The run has taken {t:.0} s, past {x}x the winners' {:.0} s to finish.",
                budget.done_s
            ),
            t,
        });
    }
    let edit_limit = (x * budget.first_edit_s).max(rules.min_edit_s);
    if run.first_edit.is_none() && t > edit_limit {
        return Some(Stop {
            rule: "no_edit".to_string(),
            why: format!(
                "No edit after {t:.0} s. The winners made their first edit by {:.0} s, and the limit is {edit_limit:.0} s.",
                budget.first_edit_s
            ),
            t,
        });
    }
    if let [.., a, b, c] = run.actions.as_slice()
        && a.input == b.input
        && b.input == c.input
        && a.output == b.output
        && b.output == c.output
    {
        return Some(Stop {
            rule: "repeating".to_string(),
            why: format!(
                "The run did the same action three times with the same output: {}",
                clip(&c.input, 200)
            ),
            t,
        });
    }
    if let Some(last) = run.actions.last()
        && t - last.t > rules.idle_s
    {
        return Some(Stop {
            rule: "idle".to_string(),
            why: format!(
                "The model has taken no action for {:.0} s, since {}.",
                t - last.t,
                clip(&last.input, 120)
            ),
            t,
        });
    }
    let spent = run.usd() + run.judge_usd;
    if spent > rules.max_usd {
        return Some(Stop {
            rule: "spend".to_string(),
            why: format!(
                "Spend reached ${spent:.4}, past the ${:.2} limit.",
                rules.max_usd
            ),
            t,
        });
    }
    if run.refused_finishes >= 2 {
        return Some(Stop {
            rule: "finish_refused".to_string(),
            why: format!(
                "The run claimed done {} times, and the host turned each finish back because its checks still fail.",
                run.refused_finishes
            ),
            t,
        });
    }
    None
}

/// The Jev request for a judgment of `run` against `card`.
#[must_use]
pub fn request(run: &Run, card: &Card, now: u64) -> (Value, ::jev::Questions) {
    let t = run.seconds(now);
    let recent = run.actions.len().saturating_sub(12);
    let earlier: Vec<String> = run.actions[..recent]
        .iter()
        .map(|action| {
            format!(
                "[{:.0}s] {}: {}",
                action.t,
                action.tool,
                clip(&action.input, 100)
            )
        })
        .collect();
    let recent: Vec<Value> = run.actions[recent..]
        .iter()
        .map(|action| {
            json!({
                "t_s": action.t.round(),
                "tool": action.tool,
                "input": clip(&action.input, 600),
                "output": clip(&action.output, 700),
            })
        })
        .collect();
    let previous: Vec<Value> = run
        .judgments
        .iter()
        .rev()
        .take(3)
        .map(|j| json!({"t_s": j.t.round(), "phase": j.phase, "deviation": j.deviation, "on_track": j.on_track}))
        .collect();
    let state = json!({
        "task": clip(&run.instruction, 4_000),
        "briefing": {
            "sections": run.briefing.lines().filter(|line| line.starts_with('#')).take(40).collect::<Vec<_>>(),
            "start": clip(&run.briefing, 2_500),
            "characters": run.briefing.chars().count(),
        },
        "winning_strategy": card.strategy,
        "phases": card.phases.iter().map(|phase| json!({
            "id": phase.id,
            "what": phase.what,
            "done_when": phase.done_when,
            "winners_started_s": phase.fable_seconds.start,
            "winners_ended_s": phase.fable_seconds.end,
        })).collect::<Vec<_>>(),
        "independent_check": card.independent_check,
        "must_not": card.must_not,
        "pitfalls": card.pitfalls.iter().map(|p| json!({"id": p.id, "what": p.what})).collect::<Vec<_>>(),
        "winners_seconds_to_done": card.fable.median_seconds,
        "run": {
            "elapsed_s": t.round(),
            "actions": run.actions.len(),
            "first_edit_s": run.first_edit.map(f64::round),
            "host_notes": run.notes.iter().rev().take(4).collect::<Vec<_>>(),
        },
        "earlier_actions": earlier,
        "recent_actions": recent,
        "previous_judgments": previous,
    });
    let mut phase = ::jev::Choice::new(
        "Which of the winners' `phases` is the run in now, judging by `recent_actions`? Pick other when its actions match none of them.",
        indexmap::IndexMap::new(),
    );
    for p in &card.phases {
        phase = phase.option(p.id.clone(), p.what.clone());
    }
    phase = phase.option("other", "The run's actions match none of the phases.");
    let mut deviation = ::jev::Choice::new(
        "Compared with `winning_strategy` and `phases`, how does the run deviate now?",
        indexmap::IndexMap::new(),
    );
    for (id, words) in DEVIATIONS {
        deviation = deviation.option(id, words);
    }
    let mut pitfall = ::jev::Choice::new(
        "Which of the known `pitfalls` does the run show now? Pick none when it shows none of them.",
        indexmap::IndexMap::new(),
    );
    for p in &card.pitfalls {
        pitfall = pitfall.option(p.id.clone(), p.what.clone());
    }
    pitfall = pitfall.option("none", "The run shows none of the known pitfalls.");
    let questions = ::jev::Questions::new()
        .with(
            "on_track",
            ::jev::Noul::new(
                "Is the run on track to finish the task the way the winners did: its actions so far follow `phases` in order, or depart in a way that serves the task as well? The host's `briefing` already gave the model the files and evidence it lists, so a phase the briefing covers counts as done even with no action for it. The model is slower than the winners; judge the order and the substance of its work more than its pace.",
            ),
        )
        .with("phase", phase)
        .with("deviation", deviation)
        .with("pitfall", pitfall)
        .with(
            "stop",
            ::jev::Noul::new(
                "If the run keeps to its current path, will it fail to finish the task, so it should be stopped now and fixed? Answer yes only when `recent_actions` show it, not because the run is early.",
            ),
        );
    (state, questions)
}

/// Asks Jev to judge the run after its latest action.
pub async fn judge(mode: &JevMode, run: &Run, card: &Card, rules: &Rules, now: u64) -> Judgment {
    let (state, questions) = request(run, card, now);
    let recorder = Recorder::default();
    let asked = jev_component::ask(
        mode,
        &recorder,
        jev_component::Ask {
            component: "fire.judge",
            name: "jev_fire_judge",
            id: format!("fire-{}", run.actions.len()),
            state,
            questions,
            parent: None,
            deadline: None,
        },
    )
    .await;
    let mut judgment = Judgment {
        t: run.seconds(now),
        actions: run.actions.len(),
        phase: asked.choice("phase").map(str::to_string),
        on_track: asked.noul("on_track"),
        deviation: asked.choice("deviation").map(str::to_string),
        pitfall: asked
            .choice("pitfall")
            .filter(|p| *p != "none")
            .map(str::to_string),
        stop: asked.noul("stop"),
        usd: asked.input_tokens.map_or(0.0, |tokens| {
            tokens as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        }),
        milliseconds: asked.milliseconds,
        error: asked.error.clone(),
        vote: false,
    };
    judgment.vote = votes(&judgment, rules);
    judgment
}

/// Whether one judgment votes to stop.
#[must_use]
pub fn votes(judgment: &Judgment, rules: &Rules) -> bool {
    let bad = judgment
        .deviation
        .as_deref()
        .is_some_and(|deviation| BAD.contains(&deviation));
    judgment.stop.is_some_and(|p| p >= rules.stop_p)
        || (bad && judgment.on_track.is_some_and(|p| p <= rules.off_track_p))
        || (judgment.pitfall.is_some() && judgment.stop.is_some_and(|p| p >= rules.pitfall_p))
}

/// Jev's stop: the last `rules.votes` judgments all voted to stop.
#[must_use]
pub fn jev_stop(run: &Run, rules: &Rules) -> Option<Stop> {
    if run.actions.len() < rules.min_actions {
        return None;
    }
    // Judgments cast before `min_actions` don't count.
    let counted: Vec<&Judgment> = run
        .judgments
        .iter()
        .filter(|j| j.actions >= rules.min_actions)
        .collect();
    let newest = *counted.last()?;
    // `votes` of the last `votes + 1` judgments voted to stop.
    let window = &counted[counted.len().saturating_sub(rules.votes + 1)..];
    let voted = window.iter().filter(|j| j.vote).count();
    // Or the stop answer stayed at `pitfall_p` or more for `steady` judgments.
    let steady = counted.len() >= rules.steady
        && counted[counted.len() - rules.steady..]
            .iter()
            .all(|j| j.stop.is_some_and(|p| p >= rules.pitfall_p));
    let how = if newest.vote && voted >= rules.votes {
        format!(
            "{voted} of its last {} judgments voted to stop",
            window.len()
        )
    } else if steady {
        format!(
            "its stop answer stayed at {:.2} or more for {} judgments in a row",
            rules.pitfall_p, rules.steady
        )
    } else {
        return None;
    };
    Some(Stop {
        rule: "jev".to_string(),
        why: format!(
            "Jev judged the run off the winners' strategy: {how}. Latest: phase {}, deviation {}, pitfall {}, on track {}, stop {}.",
            newest.phase.as_deref().unwrap_or("unknown"),
            newest.deviation.as_deref().unwrap_or("unknown"),
            newest.pitfall.as_deref().unwrap_or("none"),
            newest
                .on_track
                .map_or("unknown".to_string(), |p| format!("{p:.2}")),
            newest
                .stop
                .map_or("unknown".to_string(), |p| format!("{p:.2}")),
        ),
        t: newest.t,
    })
}

/// The first `max` characters of `text`, with the count of the rest.
#[must_use]
pub fn clip(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}… [{} more characters]", count - max)
}
