//! `exec.system`: the executor's system prompt as a library of sections.
//!
//! Each executor ships a default system prompt. This module holds each
//! default split into its sections, verbatim as captured on the wire
//! (Claude Code 2.1.280 and Codex 0.155.1 on GPT-6 Luna), and a library of
//! sections written for headless work:
//!
//! - the **protected** security policy, byte for byte the one Claude Code
//!   sends, which every variant carries and no study may remove or edit;
//! - the **core**: identity, the episode's own authority statement,
//!   verification, faithful reporting, and code style;
//! - **optional** guidance that Jev selects per task, one Noul question
//!   per section.
//!
//! A variant names its sections and how the executor takes them: `replace`
//! swaps the default's main prompt for the variant's text (Claude Code's
//! `--system-prompt-file`, Codex's `model_instructions_file`), and `append`
//! adds the text to the default (`--append-system-prompt-file`, Codex's
//! `developer_instructions`). Neither flag removes what the CLI adds on its
//! own: Claude Code's identity line, environment block, and attribution
//! reminder, or Codex's permissions, skills, collaboration, and multi-agent
//! messages and its tool definitions. `docs/terminal-bench/delegate-prompts/`
//! holds the captured requests that show this.
//!
//! The variant's digest is the SHA-256 of its agent, mode, and rendered
//! text, so two runs that sent the same prompt share it.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::delegate::Agent;
use crate::record::Implementation;

/// The schema of a variant record.
pub const SCHEMA: &str = "openagents.coder-one.system-prompt.v1";

/// The Claude Code version whose default the library splits.
pub const CLAUDE_CODE_VERSION: &str = "2.1.280";

/// The Codex version and model whose default the library splits.
pub const CODEX_VERSION: &str = "0.155.1";
pub const CODEX_MODEL: &str = "gpt-6-luna";

/// A Noul at or above this selects an optional section. An unmeasured
/// development value.
pub const SELECT: f64 = 0.5;

/// A section's role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// A safety boundary: present in every variant, never edited.
    Protected,
    /// Part of the headless core.
    Core,
    /// Guidance Jev may select per task.
    Optional,
    /// A default section a headless run keeps.
    Keep,
    /// A default section worth tuning.
    Tune,
    /// A default section the headless core replaces with its own.
    Replace,
    /// A default section with no use in a headless run.
    Remove,
}

impl Status {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Status::Protected => "protected",
            Status::Core => "core",
            Status::Optional => "optional",
            Status::Keep => "keep",
            Status::Tune => "tune",
            Status::Replace => "replace",
            Status::Remove => "remove",
        }
    }
}

/// One section of a system prompt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    /// The ID a manifest names: a library ID such as `verify`, or
    /// `default:<id>` for a section of the agent's default.
    pub id: &'static str,
    pub status: Status,
    pub text: &'static str,
    /// Why the section has its status.
    pub note: &'static str,
    /// For an optional section, the Jev question that selects it.
    pub question: Option<&'static str>,
}

impl Section {
    /// Characters of text.
    #[must_use]
    pub fn chars(&self) -> usize {
        self.text.chars().count()
    }
}

const SECURITY: &str = include_str!("../prompts/claude-code-2.1.280/02-security.md");

/// The protected sections: every variant carries each one.
pub const PROTECTED: &[&str] = &["security"];

/// The headless core, in order.
pub const CORE: &[&str] = &[
    "role",
    "security",
    "authority",
    "verify",
    "report",
    "code-style",
];

/// Optional sections Jev may select, in the order they render.
pub const OPTIONAL: &[&str] = &[
    "long-builds",
    "packages",
    "data-parsing",
    "git-recovery",
    "concurrency",
    "services",
];

/// The library of headless sections, the protected one included.
pub const LIBRARY: &[Section] = &[
    Section {
        id: "role",
        status: Status::Core,
        text: include_str!("../prompts/headless/role.md"),
        note: "Headless identity: no reader, no answers.",
        question: None,
    },
    Section {
        id: "security",
        status: Status::Protected,
        text: SECURITY,
        note: "Claude Code's security policy, verbatim. Never removed or edited by a study.",
        question: None,
    },
    Section {
        id: "authority",
        status: Status::Core,
        text: include_str!("../prompts/headless/authority.md"),
        note: "Replaces the rule to confirm hard-to-reverse actions, which nobody answers headless.",
        question: None,
    },
    Section {
        id: "verify",
        status: Status::Core,
        text: include_str!("../prompts/headless/verify.md"),
        note: "The checker grades the final state; replaces Codex's rule against running tests unasked.",
        question: None,
    },
    Section {
        id: "report",
        status: Status::Core,
        text: include_str!("../prompts/headless/report.md"),
        note: "Faithful final report.",
        question: None,
    },
    Section {
        id: "code-style",
        status: Status::Core,
        text: include_str!("../prompts/headless/code-style.md"),
        note: "Claude Code's code-style line, kept.",
        question: None,
    },
    Section {
        id: "long-builds",
        status: Status::Optional,
        text: include_str!("../prompts/headless/long-builds.md"),
        note: "Long builds and compilations.",
        question: Some(
            "Does the task in `issue` require building or compiling code, such as a C extension, a native library, or a project with a build system, that may take minutes?",
        ),
    },
    Section {
        id: "packages",
        status: Status::Optional,
        text: include_str!("../prompts/headless/packages.md"),
        note: "Package installation.",
        question: Some(
            "Does the task in `issue` require installing packages or libraries, with pip, npm, apt, or another package manager, before the work can be done?",
        ),
    },
    Section {
        id: "data-parsing",
        status: Status::Optional,
        text: include_str!("../prompts/headless/data-parsing.md"),
        note: "Parsing logs and data by field.",
        question: Some(
            "Does the task in `issue` require reading and parsing logs, data files, or a database to compute or extract results?",
        ),
    },
    Section {
        id: "git-recovery",
        status: Status::Optional,
        text: include_str!("../prompts/headless/git-recovery.md"),
        note: "Recovering lost Git work.",
        question: Some(
            "Does the task in `issue` involve recovering, repairing, or inspecting Git history, such as lost commits, a detached HEAD, or leaked secrets in past commits?",
        ),
    },
    Section {
        id: "concurrency",
        status: Status::Optional,
        text: include_str!("../prompts/headless/concurrency.md"),
        note: "Cancellation and cleanup in concurrent code.",
        question: Some(
            "Does the task in `issue` involve concurrent or asynchronous code, where cancellation, interruption, or cleanup of running tasks matters?",
        ),
    },
    Section {
        id: "services",
        status: Status::Optional,
        text: include_str!("../prompts/headless/services.md"),
        note: "Servers and other long-running programs.",
        question: Some(
            "Does the task in `issue` require starting a server, a daemon, or another program that keeps running while the task is checked?",
        ),
    },
];

/// Claude Code 2.1.280's main system prompt, split verbatim. The CLI adds
/// its identity line and environment block around it on its own.
pub const CLAUDE_CODE_DEFAULT: &[Section] = &[
    default(
        "default:role",
        Status::Tune,
        include_str!("../prompts/claude-code-2.1.280/01-role.md"),
        "Role line; interactive framing.",
    ),
    default(
        "default:security",
        Status::Protected,
        SECURITY,
        "A safety boundary.",
    ),
    default(
        "default:harness",
        Status::Remove,
        include_str!("../prompts/claude-code-2.1.280/03-harness.md"),
        "Markdown display, permission modes, hooks, clickable references: no reader.",
    ),
    default(
        "default:code-style",
        Status::Keep,
        include_str!("../prompts/claude-code-2.1.280/04-code-style.md"),
        "Relevant to edits.",
    ),
    default(
        "default:pronouns",
        Status::Remove,
        include_str!("../prompts/claude-code-2.1.280/05-pronouns.md"),
        "No reader.",
    ),
    default(
        "default:confirm-actions",
        Status::Replace,
        include_str!("../prompts/claude-code-2.1.280/06-confirm-actions.md"),
        "Conflicts with headless work, where nobody answers; the core's authority statement replaces it.",
    ),
    default(
        "default:report-faithfully",
        Status::Keep,
        include_str!("../prompts/claude-code-2.1.280/07-report-faithfully.md"),
        "Relevant to the final report.",
    ),
    default(
        "default:memory",
        Status::Remove,
        include_str!("../prompts/claude-code-2.1.280/08-memory.md"),
        "Writes files outside the task.",
    ),
    default(
        "default:environment-catalog",
        Status::Remove,
        include_str!("../prompts/claude-code-2.1.280/09-environment-catalog.md"),
        "Model catalog, Claude Code surfaces, fast mode: irrelevant.",
    ),
    default(
        "default:context-management",
        Status::Tune,
        include_str!("../prompts/claude-code-2.1.280/10-context-management.md"),
        "Rarely relevant to a short task.",
    ),
    default(
        "default:act-when-ready",
        Status::Keep,
        include_str!("../prompts/claude-code-2.1.280/11-act-when-ready.md"),
        "Discourages re-deriving established facts.",
    ),
    default(
        "default:token-budget",
        Status::Tune,
        include_str!("../prompts/claude-code-2.1.280/12-token-budget.md"),
        "The context budget line.",
    ),
];

/// Codex 0.155.1's base instructions for GPT-6 Luna, split verbatim. With
/// that model's metadata Codex sends them as the first developer message;
/// `model_instructions_file` replaces them.
pub const CODEX_DEFAULT: &[Section] = &[
    default(
        "default:role",
        Status::Tune,
        include_str!("../prompts/codex-0.155.1/01-role.md"),
        "Identity; collaborative framing.",
    ),
    default(
        "default:personality",
        Status::Remove,
        include_str!("../prompts/codex-0.155.1/02-personality.md"),
        "Personality and writing style: no reader.",
    ),
    default(
        "default:permission",
        Status::Replace,
        include_str!("../prompts/codex-0.155.1/03-permission.md"),
        "When to ask the user; nobody answers headless.",
    ),
    default(
        "default:autonomy",
        Status::Keep,
        include_str!("../prompts/codex-0.155.1/04-autonomy.md"),
        "Persistence until the goal is met.",
    ),
    default(
        "default:working-with-user",
        Status::Tune,
        include_str!("../prompts/codex-0.155.1/05-working-with-user.md"),
        "Commentary and final channels, formatting, visualizations: mostly for a reader.",
    ),
    default(
        "default:rules",
        Status::Tune,
        include_str!("../prompts/codex-0.155.1/06-rules.md"),
        "Search and shell rules; includes a rule against running tests unasked.",
    ),
    default(
        "default:skills",
        Status::Remove,
        include_str!("../prompts/codex-0.155.1/07-skills.md"),
        "No skills are installed in a task container.",
    ),
    default(
        "default:apps",
        Status::Remove,
        include_str!("../prompts/codex-0.155.1/08-apps.md"),
        "No connectors.",
    ),
    default(
        "default:plugins",
        Status::Remove,
        include_str!("../prompts/codex-0.155.1/09-plugins.md"),
        "No plugins.",
    ),
];

const fn default(
    id: &'static str,
    status: Status,
    text: &'static str,
    note: &'static str,
) -> Section {
    Section {
        id,
        status,
        text,
        note,
        question: None,
    }
}

/// An agent's default sections, in order.
#[must_use]
pub fn defaults(agent: Agent) -> &'static [Section] {
    match agent {
        Agent::ClaudeCode => CLAUDE_CODE_DEFAULT,
        Agent::Codex | Agent::Microluna => CODEX_DEFAULT,
    }
}

/// An agent's default main prompt, as the CLI sends it.
#[must_use]
pub fn default_text(agent: Agent) -> String {
    defaults(agent).iter().map(|section| section.text).collect()
}

/// Whether the agent's default carries the protected sections.
#[must_use]
pub fn default_is_protected(agent: Agent) -> bool {
    agent == Agent::ClaudeCode
}

/// The version string of the default the library holds for `agent`.
#[must_use]
pub fn default_label(agent: Agent) -> String {
    match agent {
        Agent::ClaudeCode => format!("claude-code {CLAUDE_CODE_VERSION}"),
        Agent::Codex | Agent::Microluna => format!("codex {CODEX_VERSION} ({CODEX_MODEL})"),
    }
}

/// The section `id` names for `agent`: a library ID or `default:<id>`.
#[must_use]
pub fn section(agent: Agent, id: &str) -> Option<&'static Section> {
    if id.starts_with("default:") {
        defaults(agent).iter().find(|section| section.id == id)
    } else {
        LIBRARY.iter().find(|section| section.id == id)
    }
}

/// How the executor takes a variant's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Replace the default's main prompt.
    Replace,
    /// Add to the default.
    Append,
}

impl Mode {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Mode::Replace => "replace",
            Mode::Append => "append",
        }
    }

    /// The CLI setting that carries the text.
    #[must_use]
    pub fn setting(self, agent: Agent) -> &'static str {
        match (agent, self) {
            (Agent::ClaudeCode, Mode::Replace) => "--system-prompt-file",
            (Agent::ClaudeCode, Mode::Append) => "--append-system-prompt-file",
            (Agent::Codex | Agent::Microluna, Mode::Replace) => "model_instructions_file",
            (Agent::Codex | Agent::Microluna, Mode::Append) => "developer_instructions",
        }
    }
}

/// The manifest's `executor.system`: which sections the executor is sent
/// and how. Absent, the executor runs its own default prompt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub mode: Mode,
    /// Section IDs, in the order they render.
    pub sections: Vec<String>,
    /// Optional sections Jev may add per task. Empty asks nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub select: Vec<String>,
}

impl Policy {
    /// A named preset: `core`, the headless core replacing the default, or
    /// `core-select`, the same with every optional section open to Jev.
    #[must_use]
    pub fn preset(name: &str) -> Option<Self> {
        let core = || CORE.iter().map(|id| (*id).to_string()).collect();
        match name {
            "core" => Some(Policy {
                mode: Mode::Replace,
                sections: core(),
                select: Vec::new(),
            }),
            "core-select" => Some(Policy {
                mode: Mode::Replace,
                sections: core(),
                select: OPTIONAL.iter().map(|id| (*id).to_string()).collect(),
            }),
            _ => None,
        }
    }

    /// Refuses a policy `agent` can't run as written, or one that drops a
    /// protected section.
    ///
    /// # Errors
    ///
    /// Returns each problem found.
    pub fn validate(&self, agent: Agent) -> Vec<String> {
        let mut problems = Vec::new();
        if agent == Agent::Microluna {
            problems.push(
                "executor.system applies to claude-code and codex; Microluna sends its own \
                 instructions"
                    .to_string(),
            );
            return problems;
        }
        if self.sections.is_empty() {
            problems.push("executor.system.sections must name at least one section".to_string());
        }
        for id in &self.sections {
            if section(agent, id).is_none() {
                problems.push(format!(
                    "executor.system.sections names {id}, which {} has no section for",
                    agent.word()
                ));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for id in self.sections.iter().chain(&self.select) {
            if !seen.insert(id) {
                problems.push(format!("executor.system names {id} twice"));
            }
        }
        for id in &self.select {
            if !OPTIONAL.contains(&id.as_str()) {
                problems.push(format!(
                    "executor.system.select names {id}, which is not an optional section"
                ));
            }
        }
        if !self.protected(agent) {
            problems.push(format!(
                "executor.system must carry the protected section {}: {} {} the default{}",
                PROTECTED.join(", "),
                self.mode.word(),
                if self.mode == Mode::Replace {
                    "removes"
                } else {
                    "keeps"
                },
                if self.mode == Mode::Append {
                    ", which has no security policy"
                } else {
                    ""
                }
            ));
        }
        problems
    }

    /// Whether every protected section's text reaches the executor.
    #[must_use]
    pub fn protected(&self, agent: Agent) -> bool {
        let named = |id: &str| {
            self.sections
                .iter()
                .any(|s| s == id || s.strip_prefix("default:") == Some(id))
        };
        PROTECTED
            .iter()
            .all(|id| named(id) || (self.mode == Mode::Append && default_is_protected(agent)))
    }
}

/// A resolved variant: the policy's sections, plus what Jev selected.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub agent: Agent,
    pub policy: Policy,
    /// Jev's answer for each optional section it was asked about, in the
    /// policy's `select` order; `None` when unknown.
    pub asked: Vec<(String, Option<f64>)>,
}

/// One section as a variant record carries it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Sent {
    pub id: String,
    pub status: Status,
    pub chars: usize,
    /// `manifest` or `jev`.
    pub by: &'static str,
}

impl Variant {
    /// The variant for `policy` before Jev has selected anything.
    #[must_use]
    pub fn new(agent: Agent, policy: Policy) -> Self {
        Variant {
            agent,
            policy,
            asked: Vec::new(),
        }
    }

    /// The optional sections Jev is asked about.
    #[must_use]
    pub fn options(&self) -> &[String] {
        &self.policy.select
    }

    /// Records Jev's answers for the optional sections.
    pub fn select(&mut self, answers: Vec<(String, Option<f64>)>) {
        self.asked = answers;
    }

    /// The sections sent, in render order: the policy's, then the selected
    /// optional ones.
    #[must_use]
    pub fn sections(&self) -> Vec<(&'static Section, &'static str)> {
        let mut out: Vec<(&'static Section, &'static str)> = self
            .policy
            .sections
            .iter()
            .filter_map(|id| section(self.agent, id).map(|s| (s, "manifest")))
            .collect();
        for id in OPTIONAL {
            let chosen = self.asked.iter().any(|(asked, p)| {
                asked == id && p.is_some_and(|p| crate::decision::SYSTEM_SELECT.yes(p))
            });
            if chosen && let Some(section) = section(self.agent, id) {
                out.push((section, "jev"));
            }
        }
        out
    }

    /// The text the executor is sent: each section trimmed, separated by a
    /// blank line, ending with a newline.
    #[must_use]
    pub fn text(&self) -> String {
        let parts: Vec<&str> = self
            .sections()
            .iter()
            .map(|(section, _)| section.text.trim())
            .collect();
        format!("{}\n", parts.join("\n\n"))
    }

    /// SHA-256 of the agent, mode, and text.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(self.agent, self.policy.mode, &self.text())
    }

    /// What the delegation record and the Gym read.
    #[must_use]
    pub fn record(&self) -> Value {
        let sections: Vec<Sent> = self
            .sections()
            .iter()
            .map(|(section, by)| Sent {
                id: section.id.to_string(),
                status: section.status,
                chars: section.chars(),
                by,
            })
            .collect();
        let text = self.text();
        json!({
            "schema": SCHEMA,
            "variant": "manifest",
            "agent": self.agent.word(),
            "mode": self.policy.mode.word(),
            "setting": self.policy.mode.setting(self.agent),
            "digest": self.digest(),
            "chars": text.chars().count(),
            "default": default_label(self.agent),
            "default_chars": default_text(self.agent).chars().count(),
            "protected": self.policy.protected(self.agent),
            "sections": sections,
            "asked": self.asked.iter().map(|(id, p)| json!({ "id": id, "p": p })).collect::<Vec<_>>(),
        })
    }
}

/// The record for an executor that runs its own default prompt.
#[must_use]
pub fn default_record(agent: Agent) -> Value {
    let text = default_text(agent);
    json!({
        "schema": SCHEMA,
        "variant": "default",
        "agent": agent.word(),
        "mode": "default",
        "setting": Value::Null,
        "digest": Value::Null,
        "chars": text.chars().count(),
        "default": default_label(agent),
        "default_chars": text.chars().count(),
        "protected": default_is_protected(agent),
        "sections": defaults(agent).iter().map(|section| Sent {
            id: section.id.to_string(),
            status: section.status,
            chars: section.chars(),
            by: "cli",
        }).collect::<Vec<_>>(),
        "asked": [],
    })
}

/// SHA-256 of a variant's agent, mode, and text.
#[must_use]
pub fn digest(agent: Agent, mode: Mode, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent.word().as_bytes());
    hasher.update([0]);
    hasher.update(mode.word().as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The selection request: the issue, and one Noul per optional section.
#[must_use]
pub fn selection_request(issue: &Value, ids: &[String]) -> (Value, jev::Questions) {
    let mut questions = jev::Questions::new();
    for id in ids {
        if let Some(question) = LIBRARY
            .iter()
            .find(|section| section.id == id)
            .and_then(|section| section.question)
        {
            questions = questions.with(
                format!("section_{}", id.replace('-', "_")),
                jev::Noul::new(question),
            );
        }
    }
    (json!({ "issue": issue }), questions)
}

/// Each asked section's answer from a Noul lookup.
#[must_use]
pub fn selection_answers(
    ids: &[String],
    noul: impl Fn(&str) -> Option<f64>,
) -> Vec<(String, Option<f64>)> {
    ids.iter()
        .map(|id| {
            (
                id.clone(),
                noul(&format!("section_{}", id.replace('-', "_"))),
            )
        })
        .collect()
}

/// The selector's parameters, digested into its implementation.
#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        "exec.system",
        "section library with Jev selection",
        &json!({
            "threshold": SELECT,
            "core": CORE,
            "optional": OPTIONAL.iter().map(|id| json!({
                "id": id,
                "question": LIBRARY.iter().find(|s| s.id == *id).and_then(|s| s.question),
                "text": LIBRARY.iter().find(|s| s.id == *id).map(|s| s.text),
            })).collect::<Vec<_>>(),
            "protected": PROTECTED,
        }),
    )
}

/// The whole library as JSON: each agent's default split, and the headless
/// sections. `coder-one prompt list --json` prints it, and the Gym reads
/// the checked-in copy for attempts that recorded no variant.
#[must_use]
pub fn library_record() -> Value {
    let list = |sections: &[Section]| {
        sections
            .iter()
            .map(|section| {
                json!({
                    "id": section.id,
                    "status": section.status,
                    "chars": section.chars(),
                    "note": section.note,
                    "question": section.question,
                })
            })
            .collect::<Vec<_>>()
    };
    let default_list: Vec<Value> = [Agent::ClaudeCode, Agent::Codex]
        .iter()
        .map(|agent| {
            json!({
                "agent": agent.word(),
                "default": default_label(*agent),
                "chars": default_text(*agent).chars().count(),
                "protected": default_is_protected(*agent),
                "sections": list(defaults(*agent)),
            })
        })
        .collect();
    let presets: Vec<Value> = ["core", "core-select"]
        .iter()
        .filter_map(|name| Policy::preset(name).map(|policy| (name, policy)))
        .map(|(name, policy)| {
            json!({
                "name": name,
                "policy": policy,
                "chars": {
                    "claude-code": Variant::new(Agent::ClaudeCode, policy.clone()).text().chars().count(),
                    "codex": Variant::new(Agent::Codex, policy.clone()).text().chars().count(),
                },
            })
        })
        .collect();
    json!({
        "schema": "openagents.coder-one.system-library.v1",
        "protected": PROTECTED,
        "core": CORE,
        "optional": OPTIONAL,
        "select_threshold": SELECT,
        "defaults": default_list,
        "library": list(LIBRARY),
        "presets": presets,
    })
}

/// Where the checked-in library record lives.
pub const LIBRARY_FILE: &str = "prompts/library.json";

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> Policy {
        Policy::preset("core").unwrap()
    }

    #[test]
    fn each_default_splits_losslessly() {
        let claude: Value = serde_json::from_str(include_str!(
            "../../../docs/terminal-bench/claude-code-delegate-prompt/request.json"
        ))
        .unwrap();
        assert_eq!(
            claude["system"][2]["text"].as_str().unwrap(),
            default_text(Agent::ClaudeCode)
        );
        let codex: Value = serde_json::from_str(include_str!(
            "../../../docs/terminal-bench/delegate-prompts/codex-request.json"
        ))
        .unwrap();
        assert_eq!(
            codex["input"][1]["content"][0]["text"].as_str().unwrap(),
            default_text(Agent::Codex)
        );
    }

    #[test]
    fn the_protected_section_is_claude_codes_own_text() {
        let security = section(Agent::Codex, "security").unwrap();
        assert_eq!(security.status, Status::Protected);
        assert!(
            security
                .text
                .starts_with("IMPORTANT: Assist with authorized security testing")
        );
        assert_eq!(
            section(Agent::ClaudeCode, "default:security").unwrap().text,
            security.text
        );
        assert!(!default_text(Agent::Codex).contains(security.text.trim()));
    }

    #[test]
    fn every_preset_carries_the_protected_section_on_both_executors() {
        for name in ["core", "core-select"] {
            for agent in [Agent::ClaudeCode, Agent::Codex] {
                let policy = Policy::preset(name).unwrap();
                assert!(policy.validate(agent).is_empty(), "{name} {agent:?}");
                let mut variant = Variant::new(agent, policy);
                variant.select(
                    OPTIONAL
                        .iter()
                        .map(|id| ((*id).to_string(), Some(0.9)))
                        .collect(),
                );
                assert!(variant.text().contains(SECURITY.trim()));
                assert_eq!(variant.record()["protected"], json!(true));
            }
        }
    }

    #[test]
    fn a_variant_without_the_protected_section_is_refused() {
        let mut policy = core();
        policy.sections.retain(|id| id != "security");
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let problems = policy.validate(agent);
            assert!(
                problems
                    .iter()
                    .any(|p| p.contains("protected section security")),
                "{problems:?}"
            );
        }
        // Appending to Claude Code's default keeps its security policy;
        // appending to Codex's does not, since it has none.
        let append = Policy {
            mode: Mode::Append,
            sections: vec!["authority".to_string()],
            select: Vec::new(),
        };
        assert!(append.validate(Agent::ClaudeCode).is_empty());
        assert!(!append.validate(Agent::Codex).is_empty());
        let unknown = Policy {
            mode: Mode::Replace,
            sections: vec!["security".into(), "nope".into()],
            select: vec!["role".into()],
        };
        let problems = unknown.validate(Agent::ClaudeCode);
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn jev_adds_only_the_sections_it_reads_as_needed() {
        let mut variant = Variant::new(Agent::Codex, Policy::preset("core-select").unwrap());
        let base = variant.text();
        let digest = variant.digest();
        let (_, questions) =
            selection_request(&json!({ "title": "t", "body": "b" }), variant.options());
        assert_eq!(questions.len(), OPTIONAL.len());
        variant.select(selection_answers(variant.options(), |id| match id {
            "section_long_builds" => Some(0.93),
            "section_git_recovery" => Some(0.2),
            _ => None,
        }));
        let text = variant.text();
        assert!(text.starts_with(base.trim_end()));
        assert!(text.contains("Builds and compilations can take minutes."));
        assert!(!text.contains("git reflog"));
        assert_ne!(variant.digest(), digest);
        let record = variant.record();
        let by: Vec<&str> = record["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["by"].as_str().unwrap())
            .collect();
        assert_eq!(by.last(), Some(&"jev"));
        assert_eq!(by.iter().filter(|b| **b == "jev").count(), 1);
    }

    #[test]
    fn the_core_is_a_fraction_of_each_default() {
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let core = Variant::new(agent, core()).text().chars().count();
            let default = default_text(agent).chars().count();
            assert!(core * 3 < default, "{agent:?}: {core} of {default}");
        }
    }

    #[test]
    fn the_checked_in_library_record_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(LIBRARY_FILE);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let expected = format!(
            "{}\n",
            serde_json::to_string_pretty(&library_record()).unwrap()
        );
        assert!(
            text == expected,
            "{} is stale: run `coder-one prompt list --json > crates/coder-one/{LIBRARY_FILE}`",
            path.display()
        );
    }
}
