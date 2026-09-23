//! `task.profile`: Jev's features of a task, the router's input.
//!
//! One request asks a fixed battery over the task text alone: five Nouls
//! (builds code, installs packages, parses data, recovers Git history,
//! concurrency) and a difficulty Score. The battery is fixed before any
//! outcome is seen, so a router that picks among these features in a
//! cross-validation fold selects from a set no held-out task shaped.
//!
//! The request may also carry a Choice that names an executor profile. Task
//! text alone says nothing about which executor is cheapest and still
//! reliable, so the Choice is asked only when the state carries each
//! executor's measured behavior: tasks, passes over trials, mean cost, and
//! mean agent time from the outcome matrix. The request refuses a Choice
//! without it. The measured behavior a fixture carries leaves out the
//! fixture's own task, so the Choice for a held-out task never saw its
//! outcomes.

use std::collections::BTreeMap;

use jev::{Choice, Noul, Questions, Score};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::record::Implementation;

/// The schema of an exported feature file.
pub const SCHEMA: &str = "openagents.coder-one.task-features.v1";

/// The Noul features, in order: ID and question.
pub const NOULS: &[(&str, &str)] = &[
    (
        "builds_code",
        "Does the task in `issue` require building or compiling code, such as a C extension, a native module, or a project with a build system?",
    ),
    (
        "installs_packages",
        "Does the task in `issue` require installing packages or libraries with a package manager before the work can be done?",
    ),
    (
        "parses_data",
        "Does the task in `issue` require reading and parsing logs, data files, or a database to compute or extract exact results?",
    ),
    (
        "recovers_git",
        "Does the task in `issue` require inspecting, recovering, or rewriting Git history, such as lost commits, other branches, or secrets in past commits?",
    ),
    (
        "concurrency",
        "Does the task in `issue` involve concurrent or asynchronous code, processes, or terminals, where cancellation, interruption, timing, or cleanup matters?",
    ),
];

/// The difficulty Score's levels, from zero.
pub const DIFFICULTY_LEVELS: &[&str] = &[
    "A single, direct change: one command or a few lines in one file, with the exact result stated.",
    "A small change in one place that needs a little investigation or one check.",
    "Several related changes, or one change that needs careful reading of existing code or data, and tests to confirm.",
    "A multi-step job across files or tools, with build, environment, or format details that are easy to get wrong.",
    "An open-ended job that needs diagnosis, several interacting components, and edge cases the task text doesn't spell out.",
];

/// The difficulty question.
pub const DIFFICULTY: &str = "How much work would a competent engineer need to complete the task in `issue` correctly on the first try?";

/// The executor Choice's question.
pub const EXECUTOR: &str = "Which executor in `executors` is the cheapest one likely to complete the task in `issue` on the first try? Use each executor's measured behavior on other tasks, in `executors[].measured`, and how the task compares with those tasks.";

/// An executor profile the Choice may name, with its measured behavior.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Executor {
    /// The option name, such as `luna-direct`.
    pub name: String,
    pub description: String,
    /// The arm or policy key whose trials measured it.
    pub key: String,
    pub measured: Measured,
}

/// An executor's measured behavior over other tasks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measured {
    pub tasks: usize,
    pub passes: usize,
    pub trials: usize,
    pub mean_cost_usd: Option<f64>,
    pub mean_agent_seconds: Option<f64>,
    /// The tasks it failed at least once.
    #[serde(default)]
    pub failed_tasks: Vec<String>,
    /// The task the measurement leaves out, when it leaves one out.
    #[serde(default)]
    pub excludes: Option<String>,
}

/// The task profile's request.
///
/// # Errors
///
/// Returns a message when an executor carries no measured trials: a Choice
/// that names an executor needs the executors' behavior in its state.
pub fn request(issue: &Value, executors: &[Executor]) -> Result<(Value, Questions), String> {
    let mut questions = Questions::new();
    for (id, question) in NOULS {
        questions = questions.with(*id, Noul::new(*question));
    }
    let mut score = Score::new(DIFFICULTY, Vec::new());
    for level in DIFFICULTY_LEVELS {
        score = score.level(*level);
    }
    questions = questions.with("difficulty", score);
    let mut state = json!({ "issue": issue });
    if !executors.is_empty() {
        if let Some(bare) = executors.iter().find(|e| e.measured.trials == 0) {
            return Err(format!(
                "executor {} has no measured trials: a Choice that names an executor needs its measured behavior",
                bare.name
            ));
        }
        let mut choice = Choice::new(EXECUTOR, indexmap::IndexMap::new());
        for executor in executors {
            choice = choice.option(executor.name.clone(), executor.description.clone());
        }
        questions = questions.with("executor", choice);
        state["executors"] = json!(executors
            .iter()
            .map(|e| json!({ "name": e.name, "description": e.description, "measured": e.measured }))
            .collect::<Vec<_>>());
    }
    Ok((state, questions))
}

/// A task's profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Each Noul's probability; `None` when unknown.
    pub features: BTreeMap<String, Option<f64>>,
    /// The difficulty Score scaled to 0 to 1; `None` when unknown.
    pub difficulty: Option<f64>,
    /// The executor the Choice named, when asked.
    pub executor: Option<String>,
    /// The Choice's probability for each executor.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub executor_probabilities: BTreeMap<String, f64>,
}

/// Reads the profile out of an answers object.
#[must_use]
pub fn read(answers: Option<&Value>) -> Profile {
    let get = |id: &str| answers.and_then(|a| a.get(id));
    let features = NOULS
        .iter()
        .map(|(id, _)| {
            (
                (*id).to_string(),
                get(id).and_then(|a| a.get("noul")).and_then(Value::as_f64),
            )
        })
        .collect();
    let levels = (DIFFICULTY_LEVELS.len() - 1) as f64;
    let difficulty = get("difficulty")
        .and_then(|a| a.get("score"))
        .and_then(Value::as_f64)
        .map(|score| ((score / levels) * 1000.0).round() / 1000.0);
    let executor = get("executor")
        .and_then(|a| a.get("choice"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let executor_probabilities = get("executor")
        .and_then(|a| a.get("probabilities"))
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_f64().map(|p| (k.clone(), p)))
                .collect()
        })
        .unwrap_or_default();
    Profile {
        features,
        difficulty,
        executor,
        executor_probabilities,
    }
}

/// The battery's parameters, digested into its implementation.
#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        "task.profile",
        "fixed Jev feature battery",
        &json!({
            "nouls": NOULS,
            "difficulty": { "question": DIFFICULTY, "levels": DIFFICULTY_LEVELS },
            "executor": EXECUTOR,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executor(trials: usize) -> Executor {
        Executor {
            name: "luna-direct".to_string(),
            description: "Codex on GPT-6 Luna".to_string(),
            key: "codex-gpt-6-luna".to_string(),
            measured: Measured {
                tasks: 7,
                passes: trials,
                trials,
                mean_cost_usd: Some(0.003),
                mean_agent_seconds: Some(40.0),
                failed_tasks: Vec::new(),
                excludes: Some("fix-git".to_string()),
            },
        }
    }

    #[test]
    fn the_battery_asks_five_nouls_and_a_score() {
        let (state, questions) = request(&json!({ "title": "t", "body": "b" }), &[]).unwrap();
        assert_eq!(questions.len(), 6);
        assert!(state.get("executors").is_none());
    }

    #[test]
    fn a_choice_naming_an_executor_needs_its_measured_behavior() {
        let issue = json!({ "title": "t", "body": "b" });
        let (state, questions) = request(&issue, &[executor(21)]).unwrap();
        assert_eq!(questions.len(), 7);
        assert_eq!(state["executors"][0]["measured"]["trials"], json!(21));
        let error = request(&issue, &[executor(0)]).unwrap_err();
        assert!(error.contains("measured behavior"), "{error}");
    }

    #[test]
    fn the_profile_reads_nouls_the_scaled_score_and_the_choice() {
        let answers = json!({
            "builds_code": { "type": "noul", "noul": 0.9 },
            "difficulty": { "type": "score", "score": 3.0, "confidence": 0.5 },
            "executor": { "type": "choice", "choice": "luna-direct", "probabilities": { "luna-direct": 0.7, "opus-lean": 0.3 } },
        });
        let profile = read(Some(&answers));
        assert_eq!(profile.features["builds_code"], Some(0.9));
        assert_eq!(profile.features["concurrency"], None);
        assert_eq!(profile.difficulty, Some(0.75));
        assert_eq!(profile.executor.as_deref(), Some("luna-direct"));
        assert_eq!(profile.executor_probabilities["opus-lean"], 0.3);
    }
}
