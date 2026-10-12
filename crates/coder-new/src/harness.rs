//! Coder's harness as one typed configuration per run (#11289).
//!
//! The harness design study (arXiv:2609.20804,
//! `docs/research/2026-10-11-harness-design-study.md`) found that the best
//! context policy, planning and action space depend on the model, the task
//! and the budget. So each run gets a [`Config`], chosen for its [`Cell`]:
//!
//! - **The cell** is a typed Jev judgment, never a keyword or rule table:
//!   one Noul on whether the model finishes multi-step coding work without
//!   a written plan, one Choice on whether the task is repository work or
//!   shell work. The budget is the context window, a number.
//! - **The cell's default** is the promoted entry in `harness_cells.json`
//!   when there is one, and otherwise the study's defaults ([`Config::study`]).
//! - **Exploration:** a bounded share of runs ([`EXPLORE_PER_MILLE`]) gets a
//!   sibling configuration with exactly one part changed.
//! - **Promotion:** a sibling replaces a cell's default only on a paired win
//!   ([`paired_verdict`]); each promotion is an entry in `harness_cells.json`
//!   and a row in `docs/coder/harness-promotions.md`.
//!
//! The chosen configuration is recorded in each run's ATIF export
//! (`extra.harness`).

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};

pub use crate::long_session::ContextPolicy;

/// Runs, per thousand, that try a sibling configuration.
pub const EXPLORE_PER_MILLE: u64 = 100;
/// Below this context window a run's budget is tight.
pub const TIGHT_WINDOW_TOKENS: u64 = 64_000;
/// Fewest paired tasks a promotion is judged on.
pub const MIN_PAIRS: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Switch {
    Off,
    On,
}

/// Which tools a run is offered: the typed file tools and the shell with a
/// preference for the typed ones, the shell alone, or both with no
/// preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tools {
    Typed,
    Shell,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Diagnostics {
    Off,
    Fast,
}

/// Stuck detection's thresholds ([`crate::stuck`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stuck {
    pub remind: u8,
    pub stop: u8,
}

/// One run's harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub context: ContextPolicy,
    pub plan: Switch,
    pub tools: Tools,
    pub stuck: Stuck,
    pub diagnostics: Diagnostics,
}

/// The part a sibling configuration changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    Context,
    Plan,
    Tools,
    Diagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelClass {
    /// Needs scaffolding (a plan, typed tools) to finish multi-step work.
    Weaker,
    Stronger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskClass {
    Repository,
    Shell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Budget {
    Tight,
    Roomy,
}

/// A (model, task, budget) cell: the unit defaults are kept and promoted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cell {
    pub model: ModelClass,
    pub task: TaskClass,
    pub budget: Budget,
}

impl Budget {
    #[must_use]
    pub fn of_window(tokens: u64) -> Self {
        if tokens < TIGHT_WINDOW_TOKENS {
            Self::Tight
        } else {
            Self::Roomy
        }
    }
}

impl Default for Config {
    /// The configuration when no cell is known: the study's choices that
    /// help every model it tried.
    fn default() -> Self {
        Self {
            context: ContextPolicy::T4,
            plan: Switch::On,
            tools: Tools::Both,
            stuck: Stuck {
                remind: crate::stuck::REMIND_AT as u8,
                stop: crate::stuck::STOP_AT as u8,
            },
            diagnostics: Diagnostics::Fast,
        }
    }
}

impl Config {
    /// The study's starting point for a cell: T4 everywhere, a plan and
    /// typed tools for weaker models, both tool styles for stronger ones,
    /// stuck detection at 5/8 and fast diagnostics.
    #[must_use]
    pub fn study(cell: Cell) -> Self {
        let weaker = cell.model == ModelClass::Weaker;
        Self {
            plan: if weaker { Switch::On } else { Switch::Off },
            tools: if weaker { Tools::Typed } else { Tools::Both },
            ..Self::default()
        }
    }

    /// The cell's default: its promoted entry, or [`Config::study`].
    #[must_use]
    pub fn for_cell(cell: Cell) -> Self {
        promoted()
            .iter()
            .find(|entry| entry.cell == cell)
            .map_or_else(|| Self::study(cell), |entry| entry.config)
    }

    /// Configurations with exactly one part changed.
    #[must_use]
    pub fn siblings(&self) -> Vec<(Part, Self)> {
        let mut found = vec![
            (
                Part::Context,
                Self {
                    context: match self.context {
                        ContextPolicy::T3 => ContextPolicy::T4,
                        ContextPolicy::T4 => ContextPolicy::T3,
                    },
                    ..*self
                },
            ),
            (
                Part::Plan,
                Self {
                    plan: match self.plan {
                        Switch::On => Switch::Off,
                        Switch::Off => Switch::On,
                    },
                    ..*self
                },
            ),
            (
                Part::Diagnostics,
                Self {
                    diagnostics: match self.diagnostics {
                        Diagnostics::Fast => Diagnostics::Off,
                        Diagnostics::Off => Diagnostics::Fast,
                    },
                    ..*self
                },
            ),
        ];
        for tools in [Tools::Typed, Tools::Shell, Tools::Both] {
            if tools != self.tools {
                found.push((Part::Tools, Self { tools, ..*self }));
            }
        }
        found
    }

    /// Whether a tool is offered under this configuration.
    #[must_use]
    pub fn offers(&self, name: &str) -> bool {
        if name == crate::plan::TOOL {
            return self.plan == Switch::On;
        }
        if name == crate::workflow::TOOL {
            return true;
        }
        !(self.tools == Tools::Shell && crate::file_tools::is_tool(name))
    }

    /// The workflow authoring skill, loaded into every run's instructions.
    ///
    /// Agents that read the Agent Skills format load the same file from
    /// `plugins/skills/openagents-workflow/SKILL.md`.
    pub const WORKFLOW_SKILL: &str =
        include_str!("../../../plugins/skills/openagents-workflow/SKILL.md");

    /// Extra guidance this configuration adds to the tool instructions.
    #[must_use]
    pub fn guidance(&self) -> Option<&'static str> {
        match self.tools {
            Tools::Typed => Some(
                "Use Read, Edit, Write, Grep and Glob for files rather than shell commands; use Run for builds, tests and other commands.\n",
            ),
            Tools::Shell => Some(
                "Work through Run: read, search and change files with shell commands, and bundle related steps into one command where it is safe.\n",
            ),
            Tools::Both => None,
        }
    }
}

/// Which way the run was decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecidedBy {
    Jev,
    /// Jev was unavailable or did not answer: the cell is unknown.
    Fallback,
}

/// The harness one run used, as the export records it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub config: Config,
    pub cell: Option<Cell>,
    pub decided_by: DecidedBy,
    /// The part a sibling run changed; `None` on the cell's default.
    pub sibling: Option<Part>,
}

impl Run {
    /// The run for a cell (or none): the cell's default, or for a bounded
    /// share of seeds a sibling with one part changed.
    #[must_use]
    pub fn choose(cell: Option<Cell>, decided_by: DecidedBy, seed: u64) -> Self {
        let base = cell.map_or_else(Config::default, Config::for_cell);
        let siblings = base.siblings();
        let explore = seed % 1000 < EXPLORE_PER_MILLE && !siblings.is_empty();
        let (sibling, config) = if explore {
            let (part, config) = siblings[(seed / 1000) as usize % siblings.len()];
            (Some(part), config)
        } else {
            (None, base)
        };
        Self {
            config,
            cell,
            decided_by,
            sibling,
        }
    }
}

/// A promoted cell default, as `harness_cells.json` lists it.
#[derive(Clone, Debug, Deserialize)]
struct Promoted {
    cell: Cell,
    config: Config,
}

fn promoted() -> &'static [Promoted] {
    static PROMOTED: OnceLock<Vec<Promoted>> = OnceLock::new();
    PROMOTED.get_or_init(|| {
        #[derive(Deserialize)]
        struct File {
            cells: Vec<Promoted>,
        }
        serde_json::from_str::<File>(include_str!("harness_cells.json"))
            .map(|file| file.cells)
            .unwrap_or_default()
    })
}

/// The question that places the model.
const MODEL_QUESTION: &str = "Does the model named in `model` reliably finish multi-step coding tasks in a repository (find the code, edit it, run the tests) without being given a written step-by-step plan?";
/// The question that places the task.
const TASK_QUESTION: &str = "What does the request in `task` mostly need?";

/// Ask Jev where a run sits. `None` when Jev has no answer.
pub async fn judge(
    client: &jev::Client,
    model: &str,
    task: &str,
) -> Option<(ModelClass, TaskClass)> {
    let mut kind = jev::Choice::default()
        .option(
            "repository",
            "Reading, changing and testing code or documents in a repository",
        )
        .option(
            "shell",
            "Running terminal commands: builds, installs, services, system or file administration",
        );
    kind.instructions = Some(TASK_QUESTION.into());
    let state = serde_json::json!({
        "model": model,
        "task": crate::long_session::clip(task, 4 * 1024),
    });
    let request = jev::SystemOneRequest::new(
        state,
        jev::Questions::new()
            .with("capable", jev::Noul::new(MODEL_QUESTION))
            .with("kind", kind),
    );
    let response = client.system_one(request).await.ok()?;
    let capable = response.noul("capable").ok()?;
    let model = if capable.noul >= 0.5 {
        ModelClass::Stronger
    } else {
        ModelClass::Weaker
    };
    let task = match response.choice("kind").ok()?.choice.as_str() {
        "shell" => TaskClass::Shell,
        _ => TaskClass::Repository,
    };
    Some((model, task))
}

/// The run for a conversation, decided once per (model, opening request)
/// and kept for the rest of it, so a chat never switches arms midway.
pub async fn for_conversation(
    client: Option<&jev::Client>,
    model: &str,
    opening: &str,
    window_tokens: u64,
) -> Run {
    static DECIDED: OnceLock<Mutex<HashMap<(String, u64), Run>>> = OnceLock::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    opening.hash(&mut hasher);
    let key = (model.to_owned(), hasher.finish());
    let decided = DECIDED.get_or_init(Default::default);
    if let Some(run) = decided.lock().ok().and_then(|map| map.get(&key).cloned()) {
        return run;
    }
    let judged = match client {
        Some(client) => judge(client, model, opening).await,
        None => None,
    };
    let budget = Budget::of_window(window_tokens);
    let (cell, by) = match judged {
        Some((model, task)) => (
            Some(Cell {
                model,
                task,
                budget,
            }),
            DecidedBy::Jev,
        ),
        None => (None, DecidedBy::Fallback),
    };
    let mut seed = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut seed);
    atif::now_ms().hash(&mut seed);
    // Tests see the cell's default, never a sibling.
    let seed = if cfg!(test) { 999 } else { seed.finish() };
    let run = Run::choose(cell, by, seed);
    if let Ok(mut map) = decided.lock() {
        map.insert(key, run.clone());
    }
    run
}

/// One task run under both configurations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    pub default_ok: bool,
    pub default_cost: f64,
    pub sibling_ok: bool,
    pub sibling_cost: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// Too few pairs to judge.
    NotEnough,
    /// The sibling wins: promote it.
    Promote,
    /// Keep the default.
    Keep,
}

/// The two-sided exact binomial p-value for `k` of `n` at one half.
fn sign_test(k: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let tail = k.min(n - k);
    let mut p = 0.0;
    let mut term = 0.5f64.powi(n as i32);
    for i in 0..=tail {
        if i > 0 {
            term *= (n - i + 1) as f64 / i as f64;
        }
        p += term;
    }
    (2.0 * p).min(1.0)
}

/// Whether a sibling beat a cell's default on paired tasks: success no
/// worse (McNemar's exact test on the discordant pairs finds no loss and
/// the sibling solved at least as many), and a lower cost per accepted
/// change that a sign test on the tasks both solved calls significant
/// (p < 0.05).
#[must_use]
pub fn paired_verdict(pairs: &[Pair]) -> Verdict {
    if pairs.len() < MIN_PAIRS {
        return Verdict::NotEnough;
    }
    let only_default = pairs
        .iter()
        .filter(|p| p.default_ok && !p.sibling_ok)
        .count();
    let only_sibling = pairs
        .iter()
        .filter(|p| !p.default_ok && p.sibling_ok)
        .count();
    if only_default > only_sibling {
        return Verdict::Keep;
    }
    let solved = |ok: fn(&Pair) -> bool, cost: fn(&Pair) -> f64| {
        let won = pairs.iter().filter(|p| ok(p)).count();
        let spent: f64 = pairs.iter().map(cost).sum();
        if won == 0 {
            f64::INFINITY
        } else {
            spent / won as f64
        }
    };
    let default_per = solved(|p| p.default_ok, |p| p.default_cost);
    let sibling_per = solved(|p| p.sibling_ok, |p| p.sibling_cost);
    let both: Vec<_> = pairs
        .iter()
        .filter(|p| p.default_ok && p.sibling_ok && p.default_cost != p.sibling_cost)
        .collect();
    let cheaper = both
        .iter()
        .filter(|p| p.sibling_cost < p.default_cost)
        .count();
    if sibling_per < default_per
        && cheaper * 2 > both.len()
        && sign_test(cheaper, both.len()) < 0.05
    {
        Verdict::Promote
    } else {
        Verdict::Keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(model: ModelClass) -> Cell {
        Cell {
            model,
            task: TaskClass::Repository,
            budget: Budget::Roomy,
        }
    }

    #[test]
    fn study_defaults_give_weaker_models_a_plan_and_typed_tools() {
        let weak = Config::study(cell(ModelClass::Weaker));
        assert_eq!((weak.plan, weak.tools), (Switch::On, Tools::Typed));
        let strong = Config::study(cell(ModelClass::Stronger));
        assert_eq!((strong.plan, strong.tools), (Switch::Off, Tools::Both));
        for config in [weak, strong, Config::default()] {
            assert_eq!(config.context, ContextPolicy::T4);
            assert_eq!(config.stuck, Stuck { remind: 5, stop: 8 });
            assert_eq!(config.diagnostics, Diagnostics::Fast);
        }
        assert!(!strong.offers(crate::plan::TOOL));
        assert!(weak.offers(crate::plan::TOOL) && weak.offers("Edit"));
        let shell = Config {
            tools: Tools::Shell,
            ..weak
        };
        assert!(!shell.offers("Edit") && shell.offers("Run"));
    }

    #[test]
    fn workflow_skill_is_loadable() {
        let skill = Config::WORKFLOW_SKILL;
        assert!(skill.starts_with("---\nname: openagents-workflow\n"));
        assert!(skill.contains("export const meta"));
        assert!(skill.contains("pipeline("));
        assert!(skill.contains("amount_sats"));
        assert!(skill.contains("`workflow` tool"));
        assert!(Config::default().offers(crate::workflow::TOOL));
        for word in [
            "anthropic",
            "claude",
            "ultracode",
            "opus",
            "sonnet",
            "haiku",
        ] {
            assert!(!skill.to_lowercase().contains(word), "{word}");
        }
    }

    #[test]
    fn each_sibling_changes_exactly_one_part() {
        let base = Config::default();
        let siblings = base.siblings();
        assert_eq!(siblings.len(), 5);
        for (part, sibling) in siblings {
            let changed = [
                base.context != sibling.context,
                base.plan != sibling.plan,
                base.tools != sibling.tools,
                base.diagnostics != sibling.diagnostics,
            ];
            assert_eq!(changed.iter().filter(|c| **c).count(), 1, "{part:?}");
            assert_eq!(base.stuck, sibling.stuck);
        }
    }

    #[test]
    fn a_bounded_share_of_runs_explores() {
        let cell = Some(cell(ModelClass::Weaker));
        let explored = (0..10_000u64)
            .map(|seed| Run::choose(cell, DecidedBy::Jev, seed.wrapping_mul(2_654_435_761)))
            .filter(|run| run.sibling.is_some())
            .count();
        assert!((800..=1_200).contains(&explored), "{explored}");
        let run = Run::choose(cell, DecidedBy::Jev, 999);
        assert_eq!(run.sibling, None);
        assert_eq!(run.config, Config::for_cell(cell.unwrap()));
        let run = Run::choose(None, DecidedBy::Fallback, 1_005);
        assert!(run.sibling.is_some());
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["decided_by"], "fallback");
        assert_eq!(json["config"]["context"], "t4");
    }

    #[test]
    fn promotion_needs_a_significant_paired_win_at_equal_success() {
        let pair = |default_ok, default_cost, sibling_ok, sibling_cost| Pair {
            default_ok,
            default_cost,
            sibling_ok,
            sibling_cost,
        };
        assert_eq!(
            paired_verdict(&[pair(true, 1.0, true, 0.5); 5]),
            Verdict::NotEnough
        );
        // Cheaper on every task, same successes.
        assert_eq!(
            paired_verdict(&[pair(true, 1.0, true, 0.7); 20]),
            Verdict::Promote
        );
        // Cheaper but loses tasks the default solved.
        let mut losing = vec![pair(true, 1.0, true, 0.7); 16];
        losing.extend([pair(true, 1.0, false, 0.2); 4]);
        assert_eq!(paired_verdict(&losing), Verdict::Keep);
        // A cheaper mean that comes from a few tasks is not significant.
        let mut mixed = vec![pair(true, 1.0, true, 1.1); 10];
        mixed.extend([pair(true, 5.0, true, 0.5); 10]);
        assert_eq!(paired_verdict(&mixed), Verdict::Keep);
        assert!((sign_test(0, 20) - 2.0 * 0.5f64.powi(20)).abs() < 1e-12);
    }

    #[test]
    fn the_promoted_cells_file_reads() {
        // The shipped file parses; an empty list means every cell is on
        // the study's defaults.
        assert!(
            serde_json::from_str::<serde_json::Value>(include_str!("harness_cells.json")).is_ok()
        );
        let _ = promoted();
    }
}
