//! The delegate recipe (#10208): what Coder does around every engine it
//! delegates to, and the digest a route records as its adapter.
//!
//! The cost audit (`docs/cost/2026-10-02-system-one-cost-efficiency-audit.md`)
//! measured the same model 61% cheaper and 37% faster than Claude Code
//! alone at equal passes, from settings around the model, not from the
//! model: a Jev briefing in front of the task, effort matched to the task,
//! few tools and a short system prompt, the five-minute prompt cache,
//! Jev-chosen knowledge, and "done" as a program state. This module is the
//! one table of what each engine gets of that, since engines differ in what
//! they let a host set. The dispatchers apply it
//! (`microcoder::repository::recipe` for task routes,
//! `coder_delegate::terminal` for the CLI fallback, with the shared
//! groundwork in `coder_delegate::recipe`), and the router records
//! [`adapter_digest`] of the rows a route's runs use, so a route record
//! names the recipe version that ran.
//!
//! Data only, like the rest of this crate. No step, time, or spend budget
//! is part of the recipe: the early stop ends a run when its frozen checks
//! pass, which is success, not a limit.

use serde::Serialize;
use serde_json::{Value, json};

use crate::Digest;

/// The recipe document's schema.
pub const RECIPE_SCHEMA: &str = "openagents.route.delegate-recipe.v1";

/// The recipe's version. A change to any row is a new version.
pub const RECIPE_VERSION: &str = "delegate-recipe-v1";

/// How an engine takes a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Runs {
    /// Microcoder's step loop generates each step through the engine:
    /// the host runs the commands, so the host owns the loop.
    MicrocoderLoop,
    /// A whole coding agent over ACP takes the turn with its own tools.
    AcpAgent,
    /// The delegate door's CLI fallback: a whole Claude Code or Codex CLI
    /// session in a filesystem boundary.
    Cli,
}

/// The class of task Jev judged a request to be, which sets the effort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    /// Asks for information and changes no file.
    Question,
    /// An ordinary change.
    Change,
    /// Correctness depends on precise specialized knowledge or subtle
    /// reasoning a fast model is likely to get plausibly wrong.
    Hard,
}

impl TaskClass {
    /// The class's word in records.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            TaskClass::Question => "question",
            TaskClass::Change => "change",
            TaskClass::Hard => "hard",
        }
    }
}

/// Whether the recipe sets something on an engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "how")]
pub enum Setting {
    /// The host sets it, as described.
    Applied(&'static str),
    /// The engine decides; the host has no lever for it.
    EnginesOwn(&'static str),
    /// Not applied on this engine yet, and why.
    Absent(&'static str),
}

impl Setting {
    #[must_use]
    pub const fn applied(self) -> bool {
        matches!(self, Setting::Applied(_))
    }
}

/// The effort each task class runs at. `None` keeps the admitted route's
/// effort (or the engine's own when the route names none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Effort {
    pub question: Option<&'static str>,
    pub change: Option<&'static str>,
    pub hard: Option<&'static str>,
}

/// One engine's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct EngineRecipe {
    /// The engine's name as a dispatch plan's run names it (`codex`,
    /// `claude`, `grok`, `devin`, `opencode`), or the CLI fallback's.
    pub engine: &'static str,
    pub runs_as: Runs,
    /// The Jev workspace briefing in front of the task.
    pub briefing: Setting,
    /// Knowledge-base entries Jev chose for the briefing.
    pub knowledge: Setting,
    /// How the effort is set, and the effort per class.
    pub effort_setting: Setting,
    pub effort: Effort,
    pub tools: Setting,
    pub system_prompt: Setting,
    pub prompt_cache: Setting,
    /// Frozen checks, and the early stop once they pass.
    pub checks: Setting,
}

const BRIEFING: Setting = Setting::Applied(
    "Jev probes the workspace (read-only probe battery, 40-file survey) and the kept evidence goes in front of the task, capped at 12,000 characters",
);
const BRIEFING_LOOP: Setting = Setting::Applied(
    "the briefing is the loop's Task section, the stable prefix of every step's prompt",
);
const KNOWLEDGE: Setting = Setting::Applied(
    "the knowledge base is searched with the request; Jev keeps entries the request's outputs depend on (question set v2, p >= 0.5, 16,000 characters) and flags easy-to-miss requirements (p >= 0.7)",
);
const CHECKS_LOOP: Setting = Setting::Applied(
    "Jev picks commands that check the request's outcome (p >= 0.7, at most 2); the host freezes those that fail before the run, runs them after each step that ran a command, and ends the run once they all pass",
);
const CHECKS_AGENT: Setting = Setting::Applied(
    "Jev picks and the host freezes checks as on a loop; while the agent works the host runs them whenever the workspace changed and then held still, and ends the turn once they all pass",
);
const ENGINE_TOOLS: Setting = Setting::EnginesOwn("the agent's own tools; ACP sets none");
const ENGINE_SYSTEM: Setting = Setting::EnginesOwn("the agent's own system prompt; ACP sets none");
const ENGINE_CACHE: Setting = Setting::EnginesOwn("the agent's own prompt caching");

/// The engine name of a Claude route that runs as one lean Claude Code
/// session (#10246) instead of Microcoder's loop.
pub const CLAUDE_SESSION: &str = "claude-session";

/// The engine name of a Codex route that runs as one lean `codex exec`
/// session (#10250) instead of Microcoder's loop.
pub const CODEX_SESSION: &str = "codex-session";

/// Every engine's row, task-route engines first.
pub const ENGINES: [EngineRecipe; 9] = [
    EngineRecipe {
        engine: "codex",
        runs_as: Runs::MicrocoderLoop,
        briefing: BRIEFING_LOOP,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied(
            "the Codex request's reasoning effort: gpt-6.1-sol stays at medium by default, high when Jev judges the task hard",
        ),
        effort: Effort {
            question: None,
            change: None,
            hard: Some("high"),
        },
        tools: Setting::Applied("none: each step is one JSON action the host runs"),
        system_prompt: Setting::Applied("the loop's own short system prompt"),
        prompt_cache: Setting::Applied(
            "Codex's automatic prompt cache, keyed per session (prompt_cache_key)",
        ),
        checks: CHECKS_LOOP,
    },
    EngineRecipe {
        engine: "claude",
        runs_as: Runs::MicrocoderLoop,
        briefing: BRIEFING_LOOP,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied(
            "Claude Code's --effort: low by default, medium when Jev judges the task hard",
        ),
        effort: Effort {
            question: Some("low"),
            change: None,
            hard: Some("medium"),
        },
        tools: Setting::Applied("none (--tools \"\"), one turn per step"),
        system_prompt: Setting::Applied("replaced (--system-prompt) with the loop's own"),
        prompt_cache: Setting::Applied("five minutes (CLAUDE_CODE_PROMPT_CACHE_TTL=5m)"),
        checks: CHECKS_LOOP,
    },
    EngineRecipe {
        engine: CLAUDE_SESSION,
        runs_as: Runs::Cli,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied(
            "Claude Code's --effort: medium, the cost audit's lean session, low for a question",
        ),
        effort: Effort {
            question: Some("low"),
            change: Some("medium"),
            hard: Some("medium"),
        },
        tools: Setting::Applied("six: Bash, Read, Edit, Write, Glob, Grep"),
        system_prompt: Setting::Applied(
            "replaced (--system-prompt-file) with the headless core sections",
        ),
        prompt_cache: Setting::Applied("five minutes (CLAUDE_CODE_PROMPT_CACHE_TTL=5m)"),
        checks: Setting::Applied(
            "Jev picks and the host freezes checks as on a loop; the session is told them, and the host runs them once the session ends",
        ),
    },
    EngineRecipe {
        engine: CODEX_SESSION,
        runs_as: Runs::Cli,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied(
            "Codex's model_reasoning_effort: medium, the owner's gpt-6.1-sol default, low for a question",
        ),
        effort: Effort {
            question: Some("low"),
            change: Some("medium"),
            hard: Some("medium"),
        },
        tools: Setting::EnginesOwn("Codex's own tools; codex exec has no tool-list setting"),
        system_prompt: Setting::Applied(
            "replaced (model_instructions_file) with the headless core sections",
        ),
        prompt_cache: Setting::EnginesOwn(
            "Codex's own per-session prompt cache; it has no TTL setting",
        ),
        checks: Setting::Applied(
            "Jev picks and the host freezes checks as on a loop; the session is told them, and the host runs them once the session ends",
        ),
    },
    EngineRecipe {
        engine: "grok",
        runs_as: Runs::AcpAgent,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied(
            "Grok Build's --reasoning-effort: low for a question, high when Jev judges the task hard, Grok Build's own otherwise",
        ),
        effort: Effort {
            question: Some("low"),
            change: None,
            hard: Some("high"),
        },
        tools: ENGINE_TOOLS,
        system_prompt: ENGINE_SYSTEM,
        prompt_cache: ENGINE_CACHE,
        checks: CHECKS_AGENT,
    },
    EngineRecipe {
        engine: "devin",
        runs_as: Runs::AcpAgent,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::EnginesOwn("devin acp has no effort setting"),
        effort: Effort {
            question: None,
            change: None,
            hard: None,
        },
        tools: ENGINE_TOOLS,
        system_prompt: ENGINE_SYSTEM,
        prompt_cache: ENGINE_CACHE,
        checks: CHECKS_AGENT,
    },
    EngineRecipe {
        engine: "opencode",
        runs_as: Runs::AcpAgent,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::EnginesOwn("opencode acp has no effort setting"),
        effort: Effort {
            question: None,
            change: None,
            hard: None,
        },
        tools: ENGINE_TOOLS,
        system_prompt: ENGINE_SYSTEM,
        prompt_cache: ENGINE_CACHE,
        checks: CHECKS_AGENT,
    },
    EngineRecipe {
        engine: "claude-code-cli",
        runs_as: Runs::Cli,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied("the terminal policy's effort, low"),
        effort: Effort {
            question: Some("low"),
            change: Some("low"),
            hard: Some("low"),
        },
        tools: Setting::Applied("six: Bash, Read, Edit, Write, Glob, Grep"),
        system_prompt: Setting::Applied("the policy's trimmed system prompt"),
        prompt_cache: Setting::Applied("five minutes (CLAUDE_CODE_PROMPT_CACHE_TTL=5m)"),
        checks: Setting::Absent("the CLI session has no host hook to run checks while it works"),
    },
    EngineRecipe {
        engine: "codex-cli",
        runs_as: Runs::Cli,
        briefing: BRIEFING,
        knowledge: KNOWLEDGE,
        effort_setting: Setting::Applied("the terminal policy's effort as model_reasoning_effort"),
        effort: Effort {
            question: Some("low"),
            change: Some("low"),
            hard: Some("low"),
        },
        tools: Setting::EnginesOwn("Codex CLI has no tool-list setting"),
        system_prompt: Setting::Applied("the policy's system prompt as Codex's instructions"),
        prompt_cache: Setting::EnginesOwn("Codex caches prompts on its own"),
        checks: Setting::Absent("the CLI session has no host hook to run checks while it works"),
    },
];

/// The row for `engine`, when the recipe has one.
#[must_use]
pub fn engine(engine: &str) -> Option<&'static EngineRecipe> {
    ENGINES.iter().find(|row| row.engine == engine)
}

/// The effort a run on `engine` takes for a task of `class`, given the
/// effort its route admitted: the class's effort when the row names one,
/// else the admitted one. An engine with no row keeps the admitted effort.
#[must_use]
pub fn effort(engine: &str, class: Option<TaskClass>, admitted: Option<&str>) -> Option<String> {
    let chosen = match (self::engine(engine), class) {
        (Some(row), Some(TaskClass::Question)) => row.effort.question,
        (Some(row), Some(TaskClass::Change)) => row.effort.change,
        (Some(row), Some(TaskClass::Hard)) => row.effort.hard,
        _ => None,
    };
    chosen
        .map(str::to_owned)
        .or_else(|| admitted.map(str::to_owned))
}

/// The recipe document for the engines a route's runs use, in the order
/// given and without repeats. An engine with no row is named with
/// `"recipe": "none"`, so the digest still says what ran.
#[must_use]
pub fn adapter(engines: &[&str]) -> Value {
    let mut seen: Vec<&str> = Vec::new();
    for name in engines {
        if !seen.contains(name) {
            seen.push(name);
        }
    }
    let rows: Vec<Value> = seen
        .iter()
        .map(|name| match engine(name) {
            Some(row) => json!(row),
            None => json!({"engine": name, "recipe": "none"}),
        })
        .collect();
    json!({"schema": RECIPE_SCHEMA, "version": RECIPE_VERSION, "engines": rows})
}

/// The adapter digest a route records for runs on `engines`.
#[must_use]
pub fn adapter_digest(engines: &[&str]) -> Digest {
    crate::digest_of(&adapter(engines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_task_route_engine_has_a_row_with_the_briefing_knowledge_and_checks() {
        for name in ["codex", "claude", "grok", "devin", "opencode"] {
            let row = engine(name).unwrap_or_else(|| panic!("no row for {name}"));
            assert!(row.briefing.applied(), "{name}");
            assert!(row.knowledge.applied(), "{name}");
            assert!(row.checks.applied(), "{name}");
        }
        let session = engine(CLAUDE_SESSION).expect("a row for the lean session");
        assert!(session.briefing.applied() && session.checks.applied());
        assert_eq!(
            effort(CLAUDE_SESSION, Some(TaskClass::Change), Some("low")).as_deref(),
            Some("medium")
        );
        let codex = engine(CODEX_SESSION).expect("a row for the lean Codex session");
        assert!(codex.briefing.applied() && codex.checks.applied());
        assert_eq!(
            effort(CODEX_SESSION, Some(TaskClass::Hard), Some("medium")).as_deref(),
            Some("medium")
        );
        for name in ["claude-code-cli", "codex-cli"] {
            let row = engine(name).unwrap_or_else(|| panic!("no row for {name}"));
            assert_eq!(row.runs_as, Runs::Cli);
            assert!(row.briefing.applied() && row.knowledge.applied(), "{name}");
        }
    }

    /// Owner rule: Codex gpt-6.1-sol at medium stays the default; only a
    /// task Jev judges hard raises it.
    #[test]
    fn codex_keeps_its_admitted_medium_except_on_a_hard_task() {
        for class in [None, Some(TaskClass::Question), Some(TaskClass::Change)] {
            assert_eq!(
                effort("codex", class, Some("medium")).as_deref(),
                Some("medium")
            );
        }
        assert_eq!(
            effort("codex", Some(TaskClass::Hard), Some("medium")).as_deref(),
            Some("high")
        );
    }

    #[test]
    fn effort_follows_the_class_and_otherwise_the_admitted_route() {
        assert_eq!(
            effort("claude", Some(TaskClass::Question), None).as_deref(),
            Some("low")
        );
        assert_eq!(effort("claude", Some(TaskClass::Change), None), None);
        assert_eq!(
            effort("claude", Some(TaskClass::Hard), Some("low")).as_deref(),
            Some("medium")
        );
        assert_eq!(effort("grok", Some(TaskClass::Change), None), None);
        assert_eq!(
            effort("grok", Some(TaskClass::Hard), None).as_deref(),
            Some("high")
        );
        assert_eq!(
            effort("devin", Some(TaskClass::Hard), Some("x")).as_deref(),
            Some("x")
        );
        assert_eq!(effort("unknown", Some(TaskClass::Hard), None), None);
    }

    #[test]
    fn the_adapter_digest_names_the_version_and_moves_with_the_engines() {
        let codex = adapter(&["codex", "codex"]);
        assert_eq!(codex["version"], RECIPE_VERSION);
        assert_eq!(codex["engines"].as_array().map(Vec::len), Some(1));
        assert_eq!(codex["engines"][0]["checks"]["state"], "applied");
        assert_ne!(adapter_digest(&["codex"]), adapter_digest(&["grok"]));
        assert_eq!(
            adapter_digest(&["codex"]),
            adapter_digest(&["codex", "codex"])
        );
        assert_eq!(adapter(&["mystery"])["engines"][0]["recipe"], "none");
    }
}
