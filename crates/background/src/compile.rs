//! Rules from conversation (phase 2): a message such as "keep my disk
//! above 50 GB free" or "only keep 2 agent target dirs" becomes a typed
//! rule, or a typed change to one, once. The rule is then run by code.
//!
//! Jev chooses, over typed catalogs listed here, what the message asks
//! for: a new rule, a change, a pause, a resume, or a removal ([`INTENT`]);
//! which existing rule it means; what a new rule does ([`WHAT`]); what an
//! edit changes ([`CHANGE`]); when it runs; which class of folder; and for
//! how long a pause lasts. No keyword or string matching selects any of
//! these. Only after they are chosen does code read bounded fields from
//! the words: sizes, percents, counts, durations, times of day, dates, and
//! paths ([`fields`]). When a choice that matters reads below
//! [`COMPILE`], or a field it needs is missing, the result is one
//! clarifying question instead of a guess.
//!
//! A compiled rule is a [`Draft`]: shown with a plain summary (or, for an
//! edit, the lines that change) and a dry run, and saved only when the
//! person confirms it ([`apply`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::{Answer, Clock, Judge, Question, Setting};
use crate::paths::Layout;
use crate::rule::{
    self, Action, Class, Condition, GB, Level, Origin, Rule, TaskOutcome, Trigger, disk,
};
use crate::store::{self, write_atomic};

/// `background.compile`: how sure each choice the compiler acts on must
/// be. Below it, the compiler asks one question instead of guessing.
pub const COMPILE: Setting = Setting::new("background.compile", 0.6);

/// The schema of a saved draft.
pub const DRAFT_SCHEMA: &str = "openagents.background.draft.v1";

/// What the message asks for.
pub const INTENT: &[(&str, &str)] = &[
    (
        "define",
        "Something new that should keep happening on its own, over time or whenever \
         something happens (a standing instruction or background rule), that no listed rule \
         already does.",
    ),
    (
        "edit",
        "A change to how a listed background rule works: a level, a count, a schedule, a \
         folder to leave alone, or what it deletes or only reports; or asking a listed rule \
         for something it already does, such as cleaning old build caches first.",
    ),
    (
        "pause",
        "Pause, stop for now, or turn off a listed background rule.",
    ),
    ("resume", "Turn a paused or off background rule back on."),
    ("remove", "Delete or get rid of a background rule for good."),
    (
        "none",
        "Something to do once, now; a question; or anything else that is not about an \
         ongoing rule.",
    ),
];

/// What a new rule does: the built-in actions a rule can be made of.
pub const WHAT: &[(&str, &str)] = &[
    (
        "free_space",
        "Keep free disk space above a level by deleting old build caches and finished \
         worktrees when it falls below.",
    ),
    (
        "prune_worktrees",
        "Regularly remove Coder worktrees of finished tasks, even when the disk is not full.",
    ),
    (
        "disk_alert",
        "Tell the user when free disk space falls below a level, deleting nothing.",
    ),
    (
        "task_failed",
        "Tell the user when a Coder run or task fails.",
    ),
    (
        "task_ended",
        "Tell the user whenever a Coder run or task ends, however it ends.",
    ),
    (
        "git_update",
        "Keep a Git checkout up to date: pull or fast-forward a branch such as main from its \
         remote.",
    ),
    (
        "unsupported",
        "Something none of the above does, such as running any other command, sending email, \
         or changing other files.",
    ),
];

/// What an edit changes.
pub const CHANGE: &[(&str, &str)] = &[
    (
        "free_level",
        "How much free disk space to keep, or the level at which cleaning or a warning starts.",
    ),
    (
        "keep_count",
        "Always keep a number of the most recently used agent build (target) folders.",
    ),
    ("deny_path", "Never touch or delete a particular folder."),
    (
        "report_only",
        "Stop deleting one kind of folder and only report or tell about it.",
    ),
    (
        "delete_again",
        "Go back to deleting a kind of folder it was only reporting.",
    ),
    (
        "idle_days",
        "How many days unused before a build folder counts as old.",
    ),
    ("interval", "How often it checks."),
    ("daily_time", "The time of day it runs."),
    (
        "already",
        "Something the rule already does as it is, such as cleaning old build caches first.",
    ),
    ("other", "Any other change."),
];

/// When a new rule runs, when the message says.
pub const WHEN: &[(&str, &str)] = &[
    ("interval", "Every so often: every few minutes or hours."),
    (
        "daily",
        "Once a day: every morning, evening, or night, or at a time of day.",
    ),
    ("task_end", "Whenever a Coder run or task ends."),
    ("start", "When the computer or OpenAgents starts."),
    (
        "file_change",
        "Whenever a particular file or folder changes.",
    ),
    ("unstated", "The message does not say when."),
];

/// A part of the day, for "every morning".
pub const PART_OF_DAY: &[(&str, &str, &str)] = &[
    ("morning", "In the morning.", "09:00"),
    ("midday", "Around noon.", "12:00"),
    ("afternoon", "In the afternoon.", "15:00"),
    ("evening", "In the evening.", "18:00"),
    ("night", "At night or overnight.", "02:00"),
    ("unstated", "No part of the day is named.", ""),
];

/// How long a pause lasts.
pub const PAUSE_FOR: &[(&str, &str)] = &[
    ("hour", "For about an hour."),
    ("day", "Until tomorrow, or for a day."),
    ("week", "For a week."),
    (
        "stated",
        "For a stated length (such as 3 hours) or until a stated date.",
    ),
    ("indefinite", "With no end: until it is turned back on."),
];

/// The candidate classes, as a person names them.
#[must_use]
pub fn class_option(class: Class) -> (&'static str, &'static str) {
    match class {
        Class::EndedTargets => (
            "ended_targets",
            "Build caches (target folders) of Coder tasks that ended.",
        ),
        Class::StaleTargets => (
            "stale_targets",
            "Old build folders: agent target folders, idle build slots, and checkouts' target \
             folders.",
        ),
        Class::Worktrees => ("worktrees", "Worktrees of finished Coder tasks."),
        Class::GatePools => ("gate_pools", "Release-gate build folders."),
        Class::Incremental => (
            "incremental",
            "Incremental compile caches inside build folders.",
        ),
        Class::Trash => ("trash", "The background trash."),
        Class::Judged => ("judged", "Folders the person confirmed as caches."),
        Class::ClaudeWorktrees => (
            "claude_worktrees",
            "Claude Code worktrees that are clean, pushed, and idle.",
        ),
        Class::Kache => (
            "kache",
            "The kache compile cache, through kache's own collector.",
        ),
        Class::Scratch => (
            "scratch",
            "Agent scratch of sessions that ended a week ago.",
        ),
    }
}

/// What kind of draft it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Define,
    Edit,
    Pause,
    Resume,
    Remove,
}

/// One reading the compiler acted on: the question, the answer, and its
/// probability, kept with the draft.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub question: String,
    pub answer: String,
    pub p: f64,
}

/// A compiled rule waiting for the person to confirm it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub schema: String,
    /// The draft's id: the chat thread's, or one `add` made.
    pub id: String,
    pub kind: Kind,
    pub message: String,
    pub made: u64,
    /// The rule as it is now, for an edit, pause, resume, or removal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Rule>,
    /// The rule as it would be saved (for a removal, as it is now).
    pub rule: Rule,
    /// A line the card adds, such as why a pause stands in for a removal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub readings: Vec<Reading>,
}

/// What a message compiled to.
#[derive(Clone, Debug, PartialEq)]
pub enum Compiled {
    /// A rule to show and confirm.
    Draft(Box<Draft>),
    /// One clarifying question, asked instead of guessing.
    Question {
        text: String,
        readings: Vec<Reading>,
    },
    /// Nothing to change: the rule already does it.
    Unchanged { text: String },
}

/// Where the message came from.
#[derive(Clone, Debug)]
pub struct Context {
    /// The chat thread (or `cli`), recorded in the rule's origin.
    pub thread: String,
    /// The chat's project folder: a `git_update` rule's checkout when the
    /// message names none.
    pub project: Option<PathBuf>,
    pub clock: Clock,
    /// The words come from `add`: only a new rule may come of them, never
    /// a change to a listed one.
    pub new_only: bool,
}

/// The new-rule kinds that only tell the person something and delete
/// nothing (ids in [`WHAT`]).
const TELLS: &[&str] = &["disk_alert", "task_failed", "task_ended"];

/// The questions the compiler asks, over the rules there are now.
#[must_use]
pub fn questions(rules: &[Rule]) -> Vec<(String, Question)> {
    let choice = |instructions: &str, options: Vec<(String, String)>| Question::Choice {
        instructions: instructions.to_owned(),
        options,
    };
    let pairs = |list: &[(&str, &str)]| -> Vec<(String, String)> {
        list.iter()
            .map(|(id, what)| ((*id).to_owned(), (*what).to_owned()))
            .collect()
    };
    let mut rule_options: Vec<(String, String)> = rules
        .iter()
        .map(|rule| (rule.id.clone(), option_of(rule)))
        .collect();
    rule_options.push((
        "new".into(),
        "None of the listed rules: something new.".into(),
    ));
    vec![
        (
            "intent".into(),
            choice(
                "What does the user's message ask for about background rules: the rules this \
                 computer runs on its own?",
                pairs(INTENT),
            ),
        ),
        (
            "rule".into(),
            choice(
                "Which listed background rule is the message about, including one whose \
                 settings it changes, pauses, or removes? Pick new only for something none of \
                 them does.",
                rule_options,
            ),
        ),
        (
            "what".into(),
            choice(
                "If the message asks for a new background rule, what should it do?",
                pairs(WHAT),
            ),
        ),
        (
            "change".into(),
            choice(
                "If the message changes an existing background rule, what does it change?",
                pairs(CHANGE),
            ),
        ),
        (
            "when".into(),
            choice(
                "When should the rule run, as the message says?",
                pairs(WHEN),
            ),
        ),
        (
            "part_of_day".into(),
            choice(
                "Which part of the day does the message name for the rule, if any?",
                PART_OF_DAY
                    .iter()
                    .map(|(id, what, _)| ((*id).to_owned(), (*what).to_owned()))
                    .collect(),
            ),
        ),
        (
            "class".into(),
            choice(
                "Which kind of folder does the message name, if any?",
                Class::ALL
                    .iter()
                    .map(|class| {
                        let (id, what) = class_option(*class);
                        (id.to_owned(), what.to_owned())
                    })
                    .collect(),
            ),
        ),
        (
            "pause_for".into(),
            choice(
                "If the message pauses a rule, for how long?",
                pairs(PAUSE_FOR),
            ),
        ),
    ]
}

/// The state Jev reads: the message and the rules there are.
#[must_use]
pub fn state(message: &str, rules: &[Rule]) -> String {
    let mut lines = vec![format!("The user's message: {message}"), String::new()];
    lines.push("Background rules on this computer:".into());
    if rules.is_empty() {
        lines.push("(none)".into());
    }
    for rule in rules {
        let status = if rule.enabled { "on" } else { "off" };
        lines.push(format!(
            "- {} ({}, {status}): {}",
            rule.id,
            rule.name,
            one_line(rule)
        ));
    }
    lines.join("\n")
}

/// Compile `message` against the rules on this computer.
///
/// # Errors
/// Jev could not be asked.
pub fn compile(
    layout: &Layout,
    message: &str,
    context: &Context,
    judge: &dyn Judge,
) -> Result<Compiled, String> {
    let rules: Vec<Rule> = store::list(layout)
        .into_iter()
        .filter_map(Result::ok)
        .collect();
    let answers = judge.ask(&state(message, &rules), &questions(&rules))?;
    Ok(from_answers(
        message,
        &rules,
        context,
        &answers,
        &layout.home,
    ))
}

fn top<'a>(answers: &'a BTreeMap<String, Answer>, id: &str) -> Option<(&'a str, f64)> {
    answers.get(id).and_then(Answer::top)
}

/// Turn Jev's answers into a draft, a question, or no change. Pure: the
/// tests call it with a stand-in's answers.
#[must_use]
pub fn from_answers(
    message: &str,
    rules: &[Rule],
    context: &Context,
    answers: &BTreeMap<String, Answer>,
    home: &Path,
) -> Compiled {
    let mut readings = Vec::new();
    let mut read = |question: &str| -> Option<(String, f64)> {
        let (answer, p) = top(answers, question)?;
        readings.push(Reading {
            question: question.into(),
            answer: answer.into(),
            p,
        });
        Some((answer.to_owned(), p))
    };
    let intent = read("intent");
    // An unsure intent is settled by the other readings when they agree:
    // a sure new action over a sure "no listed rule" is a new rule, and a
    // sure change of a sure listed rule is an edit. Otherwise, one question.
    let sure = |question: &str| {
        top(answers, question)
            .filter(|(_, p)| COMPILE.yes(*p))
            .map(|(answer, _)| answer.to_owned())
    };
    // A sure "tell me" reading is a new rule that only notifies: asking to
    // be told is never asking a rule that deletes to delete sooner, and it
    // is already standing, so it needs no "keep happening?" question.
    let tells = sure("what").filter(|what| TELLS.contains(&what.as_str()));
    let deletes = |id: &str| rules.iter().any(|rule| rule.id == id && rule.cleans());
    let settled = if context.new_only {
        Some("define".to_owned())
    } else {
        match intent.filter(|(_, p)| COMPILE.yes(*p)) {
            Some((intent, _)) if intent == "edit" && tells.is_some() => {
                match sure("rule").as_deref() {
                    Some(rule) if rule != "new" && !deletes(rule) => Some(intent),
                    _ => Some("define".to_owned()),
                }
            }
            Some((intent, _)) => Some(intent),
            None => match (sure("rule").as_deref(), sure("what"), sure("change")) {
                (Some("new"), Some(what), _) if what != "unsupported" => Some("define".to_owned()),
                (Some(rule), Some(_), _) if tells.is_some() && deletes(rule) => {
                    Some("define".to_owned())
                }
                (Some(rule), _, Some(change))
                    if rule != "new" && change != "other" && rules.iter().any(|r| r.id == rule) =>
                {
                    Some("edit".to_owned())
                }
                _ => None,
            },
        }
    };
    let Some(intent) = settled else {
        return Compiled::Question {
            text: "Should this keep happening on its own as a background rule, or be done once \
                   now?"
                .into(),
            readings,
        };
    };
    if intent == "none" {
        return Compiled::Question {
            text: "That reads as something to do once, not a background rule. What should keep \
                   happening on its own?"
                .into(),
            readings,
        };
    }
    if intent == "define" {
        let what = read("what");
        let when = read("when");
        let part = read("part_of_day");
        return define(message, rules, context, what, when, part, readings, home);
    }
    // Edit, pause, resume, remove: which rule?
    let rule = read("rule");
    let target = rule
        .filter(|(id, p)| COMPILE.yes(*p) && id != "new")
        .and_then(|(id, _)| rules.iter().find(|rule| rule.id == id));
    let Some(target) = target else {
        let names: Vec<String> = rules
            .iter()
            .map(|rule| format!("{} ({})", rule.name, rule.id))
            .collect();
        return Compiled::Question {
            text: if names.is_empty() {
                "There are no background rules on this computer yet. What should a new one do?"
                    .into()
            } else {
                format!("Which rule do you mean: {}?", names.join(", "))
            },
            readings,
        };
    };
    let change = (intent == "edit").then(|| read("change")).flatten();
    let class = read("class");
    let pause_for = (intent == "pause").then(|| read("pause_for")).flatten();
    let part = (intent == "edit").then(|| read("part_of_day")).flatten();
    edit(
        message, target, &intent, change, class, pause_for, part, context, readings, home,
    )
}

fn draft(
    kind: Kind,
    message: &str,
    context: &Context,
    before: Option<&Rule>,
    rule: Rule,
    readings: Vec<Reading>,
) -> Compiled {
    Compiled::Draft(Box::new(Draft {
        schema: DRAFT_SCHEMA.into(),
        id: draft_id(&context.thread),
        kind,
        message: message.into(),
        made: context.clock.now,
        before: before.cloned(),
        rule,
        note: None,
        readings,
    }))
}

/// A draft's id from a thread id: its safe characters, at most 64.
#[must_use]
pub fn draft_id(thread: &str) -> String {
    let id: String = thread
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if id.is_empty() { "cli".into() } else { id }
}

/// A new rule's shell: the built-in rule's goal, classes, and safety
/// (which only cleaning actions read), made in conversation, on.
fn base(id: &str, name: &str, message: &str, context: &Context) -> Rule {
    let mut rule = disk();
    rule.id = id.into();
    rule.name = name.into();
    rule.version = 1;
    rule.origin = Origin::Conversation {
        thread: context.thread.clone(),
        message: message.chars().take(280).collect(),
    };
    rule.enabled = true;
    rule.triggers = Vec::new();
    rule.actions = Vec::new();
    rule.conditions = Vec::new();
    rule.cooldown_secs = 0;
    rule
}

/// An id not taken by a rule there is.
fn unique(id: &str, rules: &[Rule]) -> String {
    let taken = |id: &str| rules.iter().any(|rule| rule.id == id) || rule::built_in(id).is_some();
    if !taken(id) {
        return id.into();
    }
    (2..100)
        .map(|n| format!("{id}-{n}"))
        .find(|id| !taken(id))
        .unwrap_or_else(|| id.into())
}

/// A free-space level from the words: bytes or a percent.
fn level(message: &str) -> Option<Level> {
    if let Some(bytes) = fields::size(message) {
        return Some(Level { bytes, percent: 0 });
    }
    fields::percent(message).map(|percent| Level { bytes: 0, percent })
}

fn level_words(level: Level) -> String {
    match (level.bytes, level.percent) {
        (0, percent) => format!("{percent}% of the disk"),
        (bytes, 0) => crate::paths::bytes(bytes),
        (bytes, percent) => format!("{} or {percent}% of the disk", crate::paths::bytes(bytes)),
    }
}

fn level_id(level: Level) -> String {
    match level.bytes {
        0 => format!("{}pct", level.percent),
        bytes => format!("{}gb", (bytes + GB / 2) / GB),
    }
}

/// Start at `level`; stop a margin above it; an emergency well below.
fn keep_free(rule: &mut Rule, level: Level) {
    rule.goal.start = level;
    if level.bytes > 0 {
        let margin = (level.bytes / 5).max(10 * GB);
        rule.goal.stop = Level {
            bytes: level.bytes + margin,
            percent: 0,
        };
        rule.goal.emergency = Level {
            bytes: rule.goal.emergency.bytes.min(level.bytes / 2),
            percent: 0,
        };
    } else {
        rule.goal.stop = Level {
            bytes: 0,
            percent: (level.percent + 5).min(90),
        };
        rule.goal.emergency = Level {
            bytes: 0,
            percent: level.percent / 3,
        };
    }
}

use crate::rule::ALWAYS;

/// The schedule a new rule gets: what the message said, or `default`.
fn schedule(
    message: &str,
    when: Option<&(String, f64)>,
    part: Option<&(String, f64)>,
    default: Vec<Trigger>,
) -> Vec<Trigger> {
    let when = when
        .filter(|(_, p)| COMPILE.yes(*p))
        .map(|(w, _)| w.as_str());
    let at = || {
        fields::time_of_day(message).or_else(|| {
            part.filter(|(_, p)| COMPILE.yes(*p)).and_then(|(id, _)| {
                PART_OF_DAY
                    .iter()
                    .find(|(part, _, at)| part == id && !at.is_empty())
                    .map(|(_, _, at)| (*at).to_owned())
            })
        })
    };
    match when {
        Some("interval") => fields::duration(message).map_or(default, |every| {
            vec![Trigger::Interval {
                every_secs: every.max(60),
            }]
        }),
        Some("daily") => vec![Trigger::Daily {
            at: at().unwrap_or_else(|| "09:00".into()),
        }],
        Some("task_end") => vec![Trigger::TaskEnded],
        Some("start") => vec![Trigger::HostStart],
        Some("file_change") => {
            let paths = fields::paths(message);
            if paths.is_empty() {
                default
            } else {
                vec![Trigger::FsEvent { paths }]
            }
        }
        _ => at().map_or(default, |at| vec![Trigger::Daily { at }]),
    }
}

#[allow(clippy::too_many_arguments)]
fn define(
    message: &str,
    rules: &[Rule],
    context: &Context,
    what: Option<(String, f64)>,
    when: Option<(String, f64)>,
    part: Option<(String, f64)>,
    readings: Vec<Reading>,
    home: &Path,
) -> Compiled {
    let catalog = "A background rule can keep free disk space above a level, remove finished \
                   worktrees on a schedule, tell you when disk space is low, tell you when a \
                   Coder run fails or ends, or keep a Git checkout up to date.";
    let Some((what, _)) = what.filter(|(_, p)| COMPILE.yes(*p)) else {
        return Compiled::Question {
            text: format!("{catalog} Which of these do you want?"),
            readings,
        };
    };
    let question = |text: String| Compiled::Question {
        text,
        readings: readings.clone(),
    };
    let rule = match what.as_str() {
        "free_space" => {
            let Some(level) = level(message) else {
                return question("How much free space should it keep, in GB?".into());
            };
            let words = level_words(level);
            let id = unique(&format!("keep-free-{}", level_id(level)), rules);
            let mut rule = base(&id, &format!("Keep {words} free"), message, context);
            rule.triggers = disk().triggers;
            rule.actions = disk().actions;
            rule.cooldown_secs = disk().cooldown_secs;
            keep_free(&mut rule, level);
            rule
        }
        "prune_worktrees" => {
            let id = unique("prune-worktrees", rules);
            let mut rule = base(&id, "Remove finished worktrees", message, context);
            rule.triggers = schedule(
                message,
                when.as_ref(),
                part.as_ref(),
                vec![Trigger::Daily { at: "03:00".into() }],
            );
            rule.actions = vec![Action::PruneWorktrees];
            rule.goal.start = Level {
                bytes: ALWAYS,
                percent: 0,
            };
            rule.goal.stop = rule.goal.start;
            rule.goal.emergency = Level {
                bytes: 0,
                percent: 0,
            };
            rule
        }
        "disk_alert" => {
            let Some(level) = level(message) else {
                return question("Below how much free space should I tell you, in GB?".into());
            };
            let words = level_words(level);
            let id = unique(&format!("low-disk-{}", level_id(level)), rules);
            let mut rule = base(
                &id,
                &format!("Tell me when free space is below {words}"),
                message,
                context,
            );
            rule.triggers = schedule(
                message,
                when.as_ref(),
                part.as_ref(),
                vec![Trigger::Interval { every_secs: 300 }, Trigger::HostStart],
            );
            rule.conditions = vec![Condition::FreeBelow { level }];
            rule.actions = vec![Action::Notify {
                text: "Disk space is low: {free} free.".into(),
            }];
            rule.cooldown_secs = 6 * 3600;
            rule
        }
        "task_failed" | "task_ended" => {
            let failed = what == "task_failed";
            let id = unique(
                if failed {
                    "notify-run-failed"
                } else {
                    "notify-run-ended"
                },
                rules,
            );
            let mut rule = base(
                &id,
                if failed {
                    "Tell me when a Coder run fails"
                } else {
                    "Tell me when a Coder run ends"
                },
                message,
                context,
            );
            rule.triggers = vec![Trigger::TaskEnded];
            rule.conditions = vec![Condition::TaskOutcome {
                outcomes: if failed {
                    vec![TaskOutcome::Failed]
                } else {
                    vec![
                        TaskOutcome::Succeeded,
                        TaskOutcome::Failed,
                        TaskOutcome::Cancelled,
                    ]
                },
            }];
            rule.actions = vec![Action::Notify {
                text: if failed {
                    "A Coder run failed: task {task}.".into()
                } else {
                    "A Coder run {outcome}: task {task}.".into()
                },
            }];
            rule
        }
        "git_update" => {
            let repo = fields::paths(message)
                .into_iter()
                .next()
                .or_else(|| context.project.as_ref().map(|path| tilde(path, home)));
            let Some(repo) = repo else {
                return question("Which checkout should it keep up to date? Give its folder, such as ~/work/openagents.".into());
            };
            let name = Path::new(&repo).file_name().map_or_else(
                || "checkout".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            let slug: String = name
                .to_ascii_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .take(24)
                .collect();
            let id = unique(&format!("update-{}", slug.trim_matches('-')), rules);
            let mut rule = base(&id, &format!("Keep {repo} up to date"), message, context);
            rule.triggers = schedule(
                message,
                when.as_ref(),
                part.as_ref(),
                vec![Trigger::Daily { at: "09:00".into() }],
            );
            rule.actions = vec![Action::GitFastForward { repo, branch: None }];
            rule
        }
        _ => {
            return question(format!(
                "A background rule can't do that: it only runs the host's built-in actions, never \
                 another command. {catalog} Do you want one of these?"
            ));
        }
    };
    if let Err(why) = rule.validate() {
        return question(format!(
            "That rule would not be valid ({why}). Can you say it another way?"
        ));
    }
    draft(Kind::Define, message, context, None, rule, readings)
}

/// `path` with the home folder written `~`.
fn tilde(path: &Path, home: &Path) -> String {
    path.strip_prefix(home).map_or_else(
        |_| path.display().to_string(),
        |rest| format!("~/{}", rest.display()),
    )
}

#[allow(clippy::too_many_arguments)]
fn edit(
    message: &str,
    target: &Rule,
    intent: &str,
    change: Option<(String, f64)>,
    class: Option<(String, f64)>,
    pause_for: Option<(String, f64)>,
    part: Option<(String, f64)>,
    context: &Context,
    readings: Vec<Reading>,
    _home: &Path,
) -> Compiled {
    let question = |text: String| Compiled::Question {
        text,
        readings: readings.clone(),
    };
    let mut rule = target.clone();
    let now = context.clock.now;
    let kind = match intent {
        "pause" => {
            let until = match pause_for.filter(|(_, p)| COMPILE.yes(*p)) {
                Some((how, _)) => match how.as_str() {
                    "hour" => Some(Some(now + 3600)),
                    "day" => Some(Some(context.clock.next_midnight())),
                    "week" => Some(Some(now + 7 * 86_400)),
                    "stated" => fields::duration(message)
                        .map(|secs| now + secs)
                        .or_else(|| fields::date(message))
                        .map(Some),
                    _ => Some(None),
                },
                None => Some(None),
            };
            let Some(until) = until else {
                return question(format!("Until when should {} pause?", target.name));
            };
            // Paused until a time, or off until turned back on.
            rule.enabled = until.is_some();
            rule.paused_until = until;
            Kind::Pause
        }
        "resume" => {
            if target.active(now) {
                return Compiled::Unchanged {
                    text: format!("{} is already on.", target.name),
                };
            }
            rule.enabled = true;
            rule.paused_until = None;
            Kind::Resume
        }
        "remove" => {
            if !target.conversational() {
                rule.enabled = false;
                rule.paused_until = None;
                let mut compiled = draft(
                    Kind::Pause,
                    message,
                    context,
                    Some(target),
                    rule,
                    readings.clone(),
                );
                if let Compiled::Draft(draft) = &mut compiled {
                    draft.note = Some(format!(
                        "{} is built in or comes from a plugin, so it is turned off rather than \
                         removed.",
                        target.name
                    ));
                }
                return compiled;
            }
            return draft(Kind::Remove, message, context, Some(target), rule, readings);
        }
        _ => {
            let Some((change, _)) = change.filter(|(_, p)| COMPILE.yes(*p)) else {
                return question(format!(
                    "What should change about {}: how much space it keeps, which folders it \
                     leaves alone or only reports, how many build folders it keeps, or when it \
                     runs?",
                    target.name
                ));
            };
            let class = class
                .filter(|(_, p)| COMPILE.yes(*p))
                .and_then(|(id, _)| Class::ALL.into_iter().find(|c| class_option(*c).0 == id));
            match change.as_str() {
                "free_level" => {
                    let Some(level) = level(message) else {
                        return question("How much free space should it keep, in GB?".into());
                    };
                    let mut moved = false;
                    for condition in &mut rule.conditions {
                        if let Condition::FreeBelow { level: at } = condition {
                            *at = level;
                            moved = true;
                        }
                    }
                    if !moved {
                        if !rule.cleans() {
                            return question(format!(
                                "{} does not watch free space. What should change?",
                                target.name
                            ));
                        }
                        keep_free(&mut rule, level);
                    }
                }
                "keep_count" => {
                    let Some(count) = fields::count(message) else {
                        return question("How many build folders should it keep?".into());
                    };
                    rule.classes.keep = usize::try_from(count.min(100)).unwrap_or(0);
                }
                "deny_path" => {
                    let paths = fields::paths(message);
                    if paths.is_empty() {
                        return question(
                            "Which folder should it never touch? Give its path, such as \
                             ~/.openagents/pylon."
                                .into(),
                        );
                    }
                    for path in paths {
                        if !rule.safety.deny.contains(&path) {
                            rule.safety.deny.push(path);
                        }
                    }
                }
                "report_only" | "delete_again" => {
                    let Some(class) = class else {
                        return question("Which kind of folder do you mean?".into());
                    };
                    if change == "report_only" {
                        if !rule.classes.report_only.contains(&class) {
                            rule.classes.report_only.push(class);
                        }
                    } else {
                        rule.classes.report_only.retain(|c| *c != class);
                    }
                }
                "idle_days" => {
                    let days = fields::duration(message)
                        .map(|secs| secs / 86_400)
                        .filter(|days| *days > 0)
                        .or_else(|| fields::count(message));
                    let Some(days) = days else {
                        return question(
                            "After how many days unused should a build folder count as old?".into(),
                        );
                    };
                    rule.classes.idle_days = days.clamp(1, 365);
                }
                "interval" => {
                    let Some(every) = fields::duration(message) else {
                        return question("How often should it check?".into());
                    };
                    rule.triggers
                        .retain(|t| !matches!(t, Trigger::Interval { .. }));
                    rule.triggers.insert(
                        0,
                        Trigger::Interval {
                            every_secs: every.max(60),
                        },
                    );
                }
                "daily_time" => {
                    let at = fields::time_of_day(message).or_else(|| {
                        part.filter(|(_, p)| COMPILE.yes(*p)).and_then(|(id, _)| {
                            PART_OF_DAY
                                .iter()
                                .find(|(part, _, at)| *part == id && !at.is_empty())
                                .map(|(_, _, at)| (*at).to_owned())
                        })
                    });
                    let Some(at) = at else {
                        return question("At what time of day should it run?".into());
                    };
                    rule.triggers
                        .retain(|t| !matches!(t, Trigger::Daily { .. }));
                    rule.triggers.push(Trigger::Daily { at });
                }
                "already" => {
                    return Compiled::Unchanged {
                        text: format!(
                            "{} already does that. It works in this order: {}.",
                            target.name,
                            order(target)
                        ),
                    };
                }
                _ => {
                    return question(format!(
                        "I can change how much space {} keeps, which folders it leaves alone or \
                         only reports, how many build folders it keeps, or when it runs. Which \
                         do you want?",
                        target.name
                    ));
                }
            }
            // Asking for a rule to work some way is asking for it to run:
            // an edit turns an off rule on, which the card shows. A pause
            // stays a pause.
            if !rule.enabled {
                rule.enabled = true;
                rule.paused_until = None;
            }
            Kind::Edit
        }
    };
    if rule == *target {
        return Compiled::Unchanged {
            text: format!("{} already works that way.", target.name),
        };
    }
    if let Err(why) = rule.validate() {
        return question(format!(
            "That change would not be valid ({why}). Can you say it another way?"
        ));
    }
    draft(kind, message, context, Some(target), rule, readings)
}

/// The classes a cleaning rule works through, in order, in words.
fn order(rule: &Rule) -> String {
    let mut classes: Vec<Class> = Vec::new();
    for action in &rule.actions {
        for class in action.classes() {
            if !classes.contains(&class) {
                classes.push(class);
            }
        }
    }
    if classes.is_empty() {
        return "it deletes nothing".into();
    }
    classes
        .iter()
        .map(|class| class.noun(2))
        .collect::<Vec<_>>()
        .join(", then ")
}

fn trigger_words(trigger: &Trigger) -> String {
    match trigger {
        Trigger::Interval { every_secs } => format!("every {}", fields::span(*every_secs)),
        Trigger::Threshold => "when free space falls low".into(),
        Trigger::TaskEnded => "when a Coder run ends".into(),
        Trigger::HostStart => "when OpenAgents starts".into(),
        Trigger::Daily { at } => format!("every day at {at}"),
        Trigger::FsEvent { paths } => format!("when {} changes", paths.join(" or ")),
    }
}

/// A rule as the `rule` question offers it: its name, what it does, and,
/// for a cleaning rule, the settings a message may change.
fn option_of(rule: &Rule) -> String {
    let mut text = format!("{}: {}", rule.name, one_line(rule));
    if rule.cleans() {
        text.push_str(
            " It deletes Cargo build (target) folders, including agent target folders, and \
             finished worktrees. Its settings: how much free space to keep, how many agent \
             build folders to keep, folders it never touches, kinds it only reports, and when \
             a build folder counts as old.",
        );
    }
    text
}

/// One line about a rule, for Jev's state and the rule list.
#[must_use]
pub fn one_line(rule: &Rule) -> String {
    describe(rule)
        .into_iter()
        .filter(|line| line.starts_with("Does:") || line.starts_with("When:"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A rule in plain lines: when, if, does, keeps, never, status.
#[must_use]
pub fn describe(rule: &Rule) -> Vec<String> {
    let mut lines = Vec::new();
    let when: Vec<String> = rule.triggers.iter().map(trigger_words).collect();
    lines.push(if when.is_empty() {
        "When: only when you run it.".to_owned()
    } else {
        format!("When: {}.", when.join(", "))
    });
    let mut ifs: Vec<String> = Vec::new();
    if rule.cleans() && rule.goal.start.bytes < ALWAYS {
        ifs.push(format!(
            "free space is below {}",
            level_words(rule.goal.start)
        ));
    }
    for condition in &rule.conditions {
        ifs.push(match condition {
            Condition::FreeBelow { level } => {
                format!("free space is below {}", level_words(*level))
            }
            Condition::TaskOutcome { outcomes } => {
                let words: Vec<&str> = outcomes
                    .iter()
                    .map(|o| match o {
                        TaskOutcome::Succeeded => "finished",
                        TaskOutcome::Failed => "failed",
                        TaskOutcome::Cancelled => "was cancelled",
                    })
                    .collect();
                format!("the run {}", words.join(" or "))
            }
            Condition::NoTaskRunning => "no Coder run is going".into(),
            Condition::PathExists { path } => format!("{path} exists"),
            Condition::TimeBetween { from, to } => format!("it is between {from} and {to}"),
            Condition::Weekdays { days } => format!(
                "it is {}",
                days.iter()
                    .map(|day| crate::rule::day_name(*day))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Condition::Judgment {
                question,
                setting,
                threshold,
            } => format!("Jev judges \"{question}\" ({setting}, {threshold}%)"),
        });
    }
    if !ifs.is_empty() {
        lines.push(format!("If: {}.", ifs.join(", and ")));
    }
    let mut does: Vec<String> = Vec::new();
    if rule.cleans() {
        let until = if rule.goal.start.bytes >= ALWAYS {
            String::new()
        } else {
            format!(
                ", oldest first, until {} is free",
                level_words(rule.goal.stop)
            )
        };
        does.push(format!("deletes {}{until}", order(rule)));
    }
    for action in &rule.actions {
        match action {
            Action::Notify { text } => does.push(format!("tells you \"{text}\"")),
            Action::GitFastForward { repo, .. } => does.push(format!(
                "fetches {repo} and fast-forwards it when it is clean, never otherwise"
            )),
            _ => {}
        }
    }
    lines.push(format!("Does: {}.", does.join("; ")));
    if rule.cleans() {
        if rule.classes.keep > 0 {
            lines.push(format!(
                "Keeps: the {} most recently used agent build folders.",
                rule.classes.keep
            ));
        }
        if !rule.classes.report_only.is_empty() {
            let names: Vec<&str> = rule
                .classes
                .report_only
                .iter()
                .map(|class| class.noun(2))
                .collect();
            lines.push(format!("Only reports: {}.", names.join(", ")));
        }
        if !rule.safety.deny.is_empty() {
            lines.push(format!("Never touches: {}.", rule.safety.deny.join(", ")));
        }
        if rule.classes.idle_days != disk().classes.idle_days {
            lines.push(format!(
                "Old means: unused for {} days.",
                rule.classes.idle_days
            ));
        }
    }
    lines.push(match (rule.enabled, rule.paused_until) {
        (false, _) => "Status: off.".to_owned(),
        (true, Some(until)) => format!("Status: paused until {}.", crate::view::date(until)),
        (true, None) => "Status: on.".to_owned(),
    });
    lines
}

/// The lines that change between two versions of a rule: `- ` before,
/// `+ ` after.
#[must_use]
pub fn diff(before: &Rule, after: &Rule) -> Vec<String> {
    let old = describe(before);
    let new = describe(after);
    let mut lines: Vec<String> = old
        .iter()
        .filter(|line| !new.contains(line))
        .map(|line| format!("- {line}"))
        .collect();
    lines.extend(
        new.iter()
            .filter(|line| !old.contains(line))
            .map(|line| format!("+ {line}")),
    );
    lines
}

/// What the card says, before the dry run.
#[must_use]
pub fn card(draft: &Draft) -> Vec<String> {
    let rule = &draft.rule;
    let mut lines = Vec::new();
    match (draft.kind, &draft.before) {
        (Kind::Define, _) => {
            lines.push(format!("New background rule: {} ({})", rule.name, rule.id));
            lines.extend(describe(rule).into_iter().map(|line| format!("  {line}")));
        }
        (Kind::Remove, _) => {
            lines.push(format!("Remove the rule {} ({}):", rule.name, rule.id));
            lines.extend(describe(rule).into_iter().map(|line| format!("  {line}")));
        }
        (_, Some(before)) => {
            lines.push(format!("Change {} ({}):", rule.name, rule.id));
            lines.extend(
                diff(before, rule)
                    .into_iter()
                    .map(|line| format!("  {line}")),
            );
        }
        (_, None) => lines.push(format!("{} ({})", rule.name, rule.id)),
    }
    if let Some(note) = &draft.note {
        lines.push(note.clone());
    }
    lines
}

/// Keep a draft until it is confirmed or replaced.
///
/// # Errors
/// The write failed.
pub fn save_draft(layout: &Layout, draft: &Draft) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(draft).map_err(|error| error.to_string())?;
    write_atomic(&layout.drafts().join(format!("{}.json", draft.id)), &bytes)
        .map_err(|error| error.to_string())
}

/// Forget a draft (a question replaced it, or it was applied).
pub fn drop_draft(layout: &Layout, id: &str) {
    let _ = std::fs::remove_file(layout.drafts().join(format!("{}.json", draft_id(id))));
}

/// A question a thread's words led to, kept so the answer is read with
/// them: the answer alone ("keep it as a background rule") says too
/// little, and drafting from it would ask the same question again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    pub message: String,
    pub question: String,
    pub asked: u64,
}

/// How long a question waits for its answer.
pub const PENDING_SECS: u64 = 3600;

fn pending_path(layout: &Layout, id: &str) -> PathBuf {
    layout
        .drafts()
        .join(format!("{}.question.json", draft_id(id)))
}

/// Keep the words a question was asked about, under the thread's id.
///
/// # Errors
/// The write failed.
pub fn save_pending(layout: &Layout, id: &str, pending: &Pending) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(pending).map_err(|error| error.to_string())?;
    write_atomic(&pending_path(layout, id), &bytes).map_err(|error| error.to_string())
}

/// Take the question waiting under `id`, while it is fresh; it is
/// forgotten either way.
pub fn take_pending(layout: &Layout, id: &str, now: u64) -> Option<Pending> {
    let path = pending_path(layout, id);
    let pending: Option<Pending> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let _ = std::fs::remove_file(&path);
    pending.filter(|pending| now.saturating_sub(pending.asked) <= PENDING_SECS)
}

/// The words to compile when `answer` answers `pending`: the request,
/// then the question and its answer.
#[must_use]
pub fn answered(pending: &Pending, answer: &str) -> String {
    format!(
        "{}\nAsked: {}\nThe user answered: {answer}",
        pending.message, pending.question
    )
}

/// How long a draft waits to be confirmed.
pub const DRAFT_SECS: u64 = 86_400;

/// The draft `id`, while it is fresh.
///
/// # Errors
/// There is none, or it is older than a day.
pub fn load_draft(layout: &Layout, id: &str, now: u64) -> Result<Draft, String> {
    let path = layout.drafts().join(format!("{}.json", draft_id(id)));
    let bytes = std::fs::read(&path)
        .map_err(|_| "There is no background rule waiting to be confirmed here.".to_owned())?;
    let draft: Draft =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    if draft.schema != DRAFT_SCHEMA || now.saturating_sub(draft.made) > DRAFT_SECS {
        return Err("That background rule waited more than a day; ask for it again.".into());
    }
    Ok(draft)
}

/// Whether a fresh draft waits under `id`.
#[must_use]
pub fn drafted(layout: &Layout, id: &str, now: u64) -> bool {
    load_draft(layout, id, now).is_ok()
}

/// Save what draft `id` says, then forget the draft. An edit applies only
/// to the rule as it was when the draft was made: if the rule changed
/// since, nothing is saved.
///
/// # Errors
/// No fresh draft, the rule changed since, or the save failed.
pub fn apply(layout: &Layout, id: &str, now: u64) -> Result<(Draft, Option<Rule>), String> {
    let draft = load_draft(layout, id, now)?;
    if let Some(before) = &draft.before {
        let current = store::load(layout, &before.id)?;
        if current.digest() != before.digest() {
            return Err(format!(
                "{} changed since this was drafted; ask again to see the change against it.",
                current.name
            ));
        }
    }
    let saved = match draft.kind {
        Kind::Remove => {
            store::remove(layout, &draft.rule.id)?;
            None
        }
        _ => Some(store::save(layout, &draft.rule)?),
    };
    drop_draft(layout, &draft.id);
    Ok((draft, saved))
}

/// Bounded fields read from the words after the route and the choices
/// are made: sizes, percents, counts, durations, times, dates, paths.
pub mod fields {
    const NUMBER_WORDS: &[(&str, u64)] = &[
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
    ];

    /// The message's words: lowercase, split on spaces and commas, with
    /// a number glued to its unit split off (`50gb` is `50`, `gb`).
    fn words(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for raw in text.split(|c: char| c.is_whitespace() || c == ',') {
            let word = raw
                .trim_matches(|c: char| matches!(c, '.' | ';' | '!' | '?' | '(' | ')' | '"' | '\''))
                .to_ascii_lowercase();
            if word.is_empty() {
                continue;
            }
            let digits = word
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(word.len());
            if digits > 0 && digits < word.len() && !word.contains([':', '-']) {
                out.push(word[..digits].to_owned());
                out.push(word[digits..].to_owned());
            } else {
                out.push(word);
            }
        }
        out
    }

    fn number(word: &str) -> Option<f64> {
        word.parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .or_else(|| {
                NUMBER_WORDS
                    .iter()
                    .find(|(name, _)| *name == word)
                    .map(|(_, n)| *n as f64)
            })
    }

    fn size_unit(word: &str) -> Option<f64> {
        match word {
            "tb" | "t" | "tib" | "terabytes" | "terabyte" => Some(1e12),
            "gb" | "g" | "gib" | "gigs" | "gig" | "gigabytes" | "gigabyte" => Some(1e9),
            "mb" | "mib" | "megabytes" => Some(1e6),
            _ => None,
        }
    }

    fn time_unit(word: &str) -> Option<u64> {
        match word {
            "s" | "sec" | "secs" | "second" | "seconds" => Some(1),
            "m" | "min" | "mins" | "minute" | "minutes" => Some(60),
            "h" | "hr" | "hrs" | "hour" | "hours" => Some(3600),
            "d" | "day" | "days" => Some(86_400),
            "w" | "week" | "weeks" => Some(7 * 86_400),
            _ => None,
        }
    }

    /// A size: a number and a unit (`50 GB`, `50gb`, `1.5 TB`).
    #[must_use]
    pub fn size(text: &str) -> Option<u64> {
        let words = words(text);
        words.windows(2).find_map(|pair| {
            let n = number(&pair[0])?;
            let unit = size_unit(&pair[1])?;
            Some((n * unit).round() as u64).filter(|bytes| *bytes > 0)
        })
    }

    /// A percent: `10%` or `10 percent`, at most 90.
    #[must_use]
    pub fn percent(text: &str) -> Option<u8> {
        let words = words(text);
        words.windows(2).find_map(|pair| {
            let n = number(&pair[0])?;
            matches!(pair[1].as_str(), "%" | "percent" | "pct")
                .then(|| u8::try_from(n.round() as u64).ok())
                .flatten()
                .filter(|p| (1..=90).contains(p))
        })
    }

    /// A count: the first whole number that is not a size, a percent,
    /// a duration, or a time.
    #[must_use]
    pub fn count(text: &str) -> Option<u64> {
        let words = words(text);
        words.iter().enumerate().find_map(|(at, word)| {
            let n = number(word)?;
            if n.fract() != 0.0 {
                return None;
            }
            let next = words.get(at + 1).map(String::as_str).unwrap_or("");
            if size_unit(next).is_some()
                || time_unit(next).is_some()
                || matches!(next, "%" | "percent" | "am" | "pm")
            {
                return None;
            }
            Some(n as u64)
        })
    }

    /// A duration: a number and a unit (`10 minutes`, `2h`), or a unit
    /// alone after "every" or "each" (`every hour`).
    #[must_use]
    pub fn duration(text: &str) -> Option<u64> {
        let words = words(text);
        let counted = words.windows(2).find_map(|pair| {
            let n = number(&pair[0])?;
            let unit = time_unit(&pair[1])?;
            Some((n * unit as f64).round() as u64).filter(|secs| *secs > 0)
        });
        counted.or_else(|| {
            words.windows(2).find_map(|pair| {
                matches!(pair[0].as_str(), "every" | "each" | "an" | "a")
                    .then(|| time_unit(&pair[1]))
                    .flatten()
            })
        })
    }

    /// A time of day as `HH:MM`: `7am`, `7:30 pm`, `19:00`, `07:30`.
    #[must_use]
    pub fn time_of_day(text: &str) -> Option<String> {
        let words = words(text);
        for (at, word) in words.iter().enumerate() {
            let next = words.get(at + 1).map(String::as_str).unwrap_or("");
            let (word, meridiem) = if let Some(base) = word.strip_suffix("am") {
                (base, Some(false))
            } else if let Some(base) = word.strip_suffix("pm") {
                (base, Some(true))
            } else {
                (
                    word.as_str(),
                    match next {
                        "am" | "a.m" => Some(false),
                        "pm" | "p.m" => Some(true),
                        _ => None,
                    },
                )
            };
            let (hours, minutes) = match word.split_once(':') {
                Some((h, m)) => (h.parse::<u32>().ok(), m.parse::<u32>().ok()),
                None if meridiem.is_some() => (word.parse::<u32>().ok(), Some(0)),
                None => continue,
            };
            let (Some(mut hours), Some(minutes)) = (hours, minutes) else {
                continue;
            };
            if minutes >= 60 {
                continue;
            }
            match meridiem {
                Some(pm) if (1..=12).contains(&hours) => {
                    hours %= 12;
                    if pm {
                        hours += 12;
                    }
                }
                Some(_) => continue,
                None if hours >= 24 => continue,
                None => {}
            }
            return Some(format!("{hours:02}:{minutes:02}"));
        }
        None
    }

    /// A date `YYYY-MM-DD`, as midnight UTC.
    #[must_use]
    pub fn date(text: &str) -> Option<u64> {
        words(text).iter().find_map(|word| {
            let parts: Vec<&str> = word.split('-').collect();
            let [year, month, day] = parts[..] else {
                return None;
            };
            if year.len() != 4 {
                return None;
            }
            let (year, month, day): (i64, i64, i64) =
                (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
            if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
                return None;
            }
            // Days from civil (Howard Hinnant).
            let y = if month <= 2 { year - 1 } else { year };
            let era = y.div_euclid(400);
            let yoe = y - era * 400;
            let mp = (month + 9) % 12;
            let doy = (153 * mp + 2) / 5 + day - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            let days = era * 146_097 + doe - 719_468;
            u64::try_from(days * 86_400).ok()
        })
    }

    /// Paths: words that start with `~/` or `/`, without trailing
    /// punctuation.
    #[must_use]
    pub fn paths(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|word| {
                word.trim_matches(|c: char| {
                    matches!(c, '`' | '"' | '\'' | '(' | ')' | ',' | ';' | '!' | '?')
                })
                .trim_end_matches(['.', ':'])
            })
            .filter(|word| {
                (word.starts_with("~/") || (word.starts_with('/') && word.len() > 1))
                    && !word.contains("..")
                    && word.len() <= 512
            })
            .map(|word| word.trim_end_matches('/').to_owned())
            .collect()
    }

    /// A number of seconds in words: `5 minutes`, `1 hour`, `2 days`.
    #[must_use]
    pub fn span(secs: u64) -> String {
        let (n, unit) = if secs % 86_400 == 0 {
            (secs / 86_400, "day")
        } else if secs % 3600 == 0 {
            (secs / 3600, "hour")
        } else if secs % 60 == 0 {
            (secs / 60, "minute")
        } else {
            (secs, "second")
        };
        if n == 1 {
            unit.to_owned()
        } else {
            format!("{n} {unit}s")
        }
    }
}

/// The card's dry run, for a draft: what the rule would do now.
#[must_use]
pub fn dry_run(env: &crate::plan::Env<'_>, draft: &Draft, clock: Clock) -> Vec<String> {
    match draft.kind {
        Kind::Remove => vec!["Nothing runs: the rule is removed.".into()],
        Kind::Pause if !draft.rule.active(clock.now) => {
            vec!["It does nothing while paused.".into()]
        }
        _ => crate::engine::dry_run(env, &draft.rule, clock),
    }
}

/// The dry run's lines in a card, labeled.
#[must_use]
pub fn show_dry_run(lines: &[String]) -> Vec<String> {
    let mut out = vec!["Dry run now (nothing changes):".to_owned()];
    out.extend(lines.iter().map(|line| format!("  {line}")));
    out
}
