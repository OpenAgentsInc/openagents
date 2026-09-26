//! `kb harvest`: proposing entries from a finished run.
//!
//! One structured model call — through the operator's Codex login by
//! default ([`CodexProposer`]), or OpenRouter — reads a bounded record of the run (each
//! step's rationale, commands, and output, the acceptance tests, and the
//! verifier's verdict) and proposes general entries: what went wrong, what
//! fixed it, and what would have saved steps. Each proposal is checked
//! like any entry, with the run's own task name added to the names the
//! lint refuses, and written as a `candidate`. A proposal whose search text
//! is a near-duplicate of an existing entry (cosine similarity 0.9 or more)
//! becomes that entry's next version instead of a new entry.

use std::path::{Path, PathBuf};

use microluna::price::Basis;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::lint::{Corpus, lint};
use crate::search::{Embed, Retriever};
use crate::{Base, Entry, Kind, Status, archive, pending, valid_id};

/// The default model.
pub const MODEL: &str = "openai/gpt-6-luna";

/// Cosine similarity at or above which a proposal is a new version of an
/// existing entry.
pub const DUPLICATE: f64 = 0.9;

/// Characters of run record the model reads, at most.
pub const RECORD_CHARS: usize = 60_000;

/// Entries one harvest proposes, at most.
pub const MAX_PROPOSALS: usize = 3;

/// What the model is told.
pub const SYSTEM: &str = "You write entries for a knowledge base that coding agents search \
while they work. You read the record of one agent's run on one task and propose general \
entries that would help a future agent on a different task of the same kind: a mistake the \
agent made and how to notice it, what finally fixed the problem, or what would have saved \
steps.

Rules for every entry:
- Make it general. Never name the task, its files, directories, datasets, or services. Never \
quote test code, test names, expected outputs, or any value specific to this task's data.
- Cite a source for any definition: a textbook, a paper, a standard, or official \
documentation, by author, title, and section.
- The id is `<topic>.<short-name>` in lowercase letters, digits, dots, and hyphens, such as \
`statistics.welch-t-test` or `slip.editing-the-wrong-copy`.
- kind is method (a standard definition and its variants), edge-case (an input that breaks \
common code), slip (a mistake agents make), environment (how a class of environment behaves), \
or tool (how to use a command or library correctly).
- summary is one or two sentences, under 600 characters: what the entry is about and when it \
applies. applies_when says what code or state it bears on.
- body is Markdown with a `## Details` section and a `## How to check` section that gives a \
property or a runnable snippet.
- If an existing entry already teaches the same lesson, set updates to its id and write the \
improved version: its whole body with your change folded in, dropping nothing it already \
says; otherwise leave updates empty and give the entry a new id.

Propose at most three entries, and none when the run teaches nothing general. Fewer, sharper \
entries are better than many.";

/// What the model is told when it reads another agent's winning trajectory.
pub const TRACE_SYSTEM: &str = "You write entries for a knowledge base that coding agents search \
while they work. You read the trajectory of a strong agent that solved one task: its messages, the \
commands it ran, and what they printed. A weaker, cheaper agent will later face tasks of the same \
kind. Propose general entries that carry what the strong agent knew or did that the weaker one \
would likely miss: the domain method and its exact conventions, how a tool is really used and \
read, the checks that caught its mistakes, and the order of work that got it there.

Rules for every entry:
- Make it general. Never name the task, its files, directories, datasets, or services. Never \
quote test code, test names, expected outputs, or any value specific to this task's data. Describe \
the method so it applies to any task of the kind.
- Cite a source for any definition: a textbook, a paper, a standard, or official \
documentation, by author, title, and section.
- The id is `<topic>.<short-name>` in lowercase letters, digits, dots, and hyphens, such as \
`statistics.welch-t-test` or `tool.gsea-cli`.
- kind is method (a standard definition and its variants), edge-case (an input that breaks \
common code), slip (a mistake agents make), environment (how a class of environment behaves), \
or tool (how to use a command or library correctly).
- summary is one or two sentences, under 600 characters: what the entry is about and when it \
applies. applies_when says what code or state it bears on.
- body is Markdown with a `## Details` section and a `## How to check` section that gives a \
property or a runnable snippet.
- If an existing entry already teaches the same lesson, set updates to its id and write the \
improved version: its whole body with your change folded in, dropping nothing it already \
says; otherwise leave updates empty and give the entry a new id.

Propose at most three entries, and none when the trajectory teaches nothing general. Fewer, \
sharper entries are better than many.";

/// What the model is told when it compares a failed run with another
/// agent's winning trajectory on the same task.
pub const CONTRAST_SYSTEM: &str =
    "You write entries for a knowledge base that coding agents search \
while they work. You read two records of the same task: a cheap agent's run that failed, with the \
task verifier's last lines, and a strong agent's trajectory that passed. Find the decisions where \
they differ that explain the failure: a definition, convention, parameter, data preparation step, \
tool usage, or check the winner got right and the failed run got wrong or skipped. Propose \
general entries that would have led the cheap agent to the winner's choice on any task of the \
same kind. Be precise about the decisive detail; a broad description of the method is not enough.

Rules for every entry:
- Make it general. Never name the task, its files, directories, datasets, or services. Never \
quote test code, test names, expected outputs, or any value specific to this task's data.
- Cite a source for any definition: a textbook, a paper, a standard, or official \
documentation, by author, title, and section.
- The id is `<topic>.<short-name>` in lowercase letters, digits, dots, and hyphens.
- kind is method, edge-case, slip, environment, or tool.
- summary is one or two sentences, under 600 characters. applies_when says what code or state \
it bears on.
- body is Markdown with a `## Details` section and a `## How to check` section.
- If an existing entry already teaches the lesson but misses the decisive detail, set updates to \
its id and write the improved version: its whole body with your change folded in, dropping nothing it already says; otherwise give the entry a new id.

Propose at most three entries, and none when the difference teaches nothing general.";

/// One proposed entry, as the model writes it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Proposal {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub applies_when: String,
    pub body: String,
    pub cites: Vec<String>,
    /// The existing entry this revises, or empty.
    pub updates: String,
}

/// The model's reply.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Proposals {
    pub entries: Vec<Proposal>,
}

/// The reply's JSON schema.
#[must_use]
pub fn schema() -> Value {
    let text = json!({"type": "string"});
    let list = json!({"type": "array", "items": {"type": "string"}});
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["entries"],
        "properties": {"entries": {"type": "array", "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "kind", "title", "summary", "tags", "applies_when", "body", "cites", "updates"],
            "properties": {
                "id": text, "kind": {"type": "string", "enum": ["method", "edge-case", "slip", "environment", "tool"]},
                "title": text, "summary": text, "tags": list, "applies_when": text,
                "body": text, "cites": list, "updates": text,
            },
        }}},
    })
}

/// What one call cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Cost {
    /// Dollars, or `None` when the cost is unknown.
    pub usd: Option<f64>,
    /// The known part: `usd` when that's known, else a lower bound.
    pub known_usd: f64,
    /// Why `usd` is unknown, when it is.
    pub unknown: Option<String>,
    /// How the figure was reached.
    pub basis: Basis,
}

impl Cost {
    /// A known cost.
    #[must_use]
    pub fn known(usd: f64, basis: Basis) -> Self {
        Cost {
            usd: Some(usd),
            known_usd: usd,
            unknown: None,
            basis,
        }
    }

    /// The cost as one line: dollars, or unknown with the known part and
    /// why.
    #[must_use]
    pub fn describe(&self) -> String {
        match (&self.usd, &self.unknown) {
            (Some(usd), _) => format!("${usd:.5} ({})", self.basis),
            (None, why) => format!(
                "unknown (at least ${:.5}, {}): {}",
                self.known_usd,
                self.basis,
                why.as_deref().unwrap_or("no reason recorded")
            ),
        }
    }
}

/// Proposes entries: one model call.
pub trait Propose {
    /// The model's name, for the entries' author.
    fn model(&self) -> &str;

    /// The provider that answers: `codex` or `openrouter`.
    fn provider(&self) -> &str;

    /// The proposals and what the call cost. A failed call's error says
    /// what it cost too.
    fn propose(
        &self,
        system: &str,
        prompt: &str,
    ) -> impl std::future::Future<Output = Result<(Proposals, Cost), String>>;
}

/// Proposals through the operator's Codex login, with Microluna's
/// transport: one request declares one strict tool, `knowledge_entries`,
/// whose parameters are [`schema`], and the proposals are its arguments.
/// The cost is the model's list price for the reported tokens.
pub struct CodexProposer<T: microluna::Transport = microluna::codex::CodexTransport> {
    pub transport: T,
    /// The Codex model slug, such as `gpt-6-luna`.
    pub model: String,
    /// `low`, `medium`, or `high`, or `None` for the model's default.
    pub effort: Option<String>,
}

/// The one tool a Codex harvest declares.
pub const TOOL: &str = "knowledge_entries";

impl CodexProposer {
    /// A proposer on the Codex login in `$CODEX_HOME/auth.json` or
    /// `~/.codex/auth.json`. A provider prefix on `model`, such as
    /// `openai/`, is dropped.
    ///
    /// # Errors
    ///
    /// No login, or one that can't be used now.
    pub fn from_login(model: &str) -> Result<Self, String> {
        let login = microluna::codex::Login::default_path()
            .ok_or("no Codex login: can't find ~/.codex/auth.json; run `codex login`")?;
        let session = format!("kb-harvest-{}", std::process::id());
        let transport = microluna::codex::CodexTransport::new(login, &session)
            .map_err(|e| format!("the Codex login can't be used: {e}; run `codex login`"))?;
        Ok(CodexProposer {
            transport,
            model: model.rsplit('/').next().unwrap_or(model).to_string(),
            effort: None,
        })
    }
}

impl<T: microluna::Transport> Propose for CodexProposer<T> {
    fn model(&self) -> &str {
        &self.model
    }

    fn provider(&self) -> &str {
        "codex"
    }

    async fn propose(&self, system: &str, prompt: &str) -> Result<(Proposals, Cost), String> {
        let request = microluna::Request {
            model: self.model.clone(),
            instructions: format!("{system}\n\nReply by calling {TOOL} exactly once."),
            input: vec![json!({
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }],
            })],
            tools: vec![json!({
                "type": "function",
                "name": TOOL,
                "description": "Give the proposed knowledge-base entries, or an empty list.",
                "parameters": schema(),
                "strict": true,
            })],
            effort: self.effort.clone(),
            cache_key: format!("kb-harvest-{}", std::process::id()),
            parallel_tools: false,
        };
        let called = microluna::oneshot::call(&self.transport, &request, TOOL).await;
        let cost = Cost {
            usd: called.usd,
            known_usd: called.known_usd,
            unknown: called.cost_unknown,
            basis: called.basis,
        };
        let proposals = called.arguments.and_then(|arguments| {
            serde_json::from_str::<Proposals>(&arguments)
                .map_err(|e| format!("{TOOL}'s arguments didn't parse: {e}"))
        });
        match proposals {
            Ok(proposals) => Ok((proposals, cost)),
            Err(error) => Err(format!("{error} (the call cost {})", cost.describe())),
        }
    }
}

/// Proposals through `crates/openrouter`.
pub struct OpenRouterProposer {
    pub client: openrouter::Client,
    pub model: String,
}

impl OpenRouterProposer {
    /// A proposer with the key from `OPENROUTER_API_KEY` or
    /// `~/.openagents/openrouter.json`.
    ///
    /// # Errors
    ///
    /// No key, or the HTTP client can't start.
    pub fn from_env(model: &str) -> Result<Self, String> {
        let config = openrouter::Config::from_env().map_err(|e| e.to_string())?;
        Ok(OpenRouterProposer {
            client: openrouter::Client::new(config).map_err(|e| e.to_string())?,
            model: model.to_string(),
        })
    }
}

impl Propose for OpenRouterProposer {
    fn model(&self) -> &str {
        &self.model
    }

    fn provider(&self) -> &str {
        "openrouter"
    }

    async fn propose(&self, system: &str, prompt: &str) -> Result<(Proposals, Cost), String> {
        let request = openrouter::ChatRequest::new(
            &self.model,
            vec![
                openrouter::Message::system(system),
                openrouter::Message::user(prompt),
            ],
        );
        let reply = self
            .client
            .structured::<Proposals>(request, "knowledge_entries", schema())
            .await
            .map_err(|e| {
                // A reply that misses the shape still cost what it cost; any
                // other failure's cost isn't reported.
                let cost = match &e {
                    openrouter::Error::Schema { usage, .. } => usage.cost.map_or_else(
                        || "unknown: OpenRouter reported no cost".to_string(),
                        |usd| format!("${usd:.5} (billed)"),
                    ),
                    _ => "unknown".to_string(),
                };
                format!("{e} (the call cost {cost})")
            })?;
        let cost = match reply.usage.cost {
            Some(usd) => Cost::known(usd, Basis::Billed),
            None => Cost {
                usd: None,
                known_usd: 0.0,
                unknown: Some("OpenRouter reported no cost for the call".to_string()),
                basis: Basis::Billed,
            },
        };
        Ok((reply.value, cost))
    }
}

/// Either proposer, chosen at run time.
pub enum AnyProposer {
    Codex(CodexProposer),
    OpenRouter(OpenRouterProposer),
}

impl Propose for AnyProposer {
    fn model(&self) -> &str {
        match self {
            AnyProposer::Codex(p) => p.model(),
            AnyProposer::OpenRouter(p) => p.model(),
        }
    }

    fn provider(&self) -> &str {
        match self {
            AnyProposer::Codex(p) => p.provider(),
            AnyProposer::OpenRouter(p) => p.provider(),
        }
    }

    async fn propose(&self, system: &str, prompt: &str) -> Result<(Proposals, Cost), String> {
        match self {
            AnyProposer::Codex(p) => p.propose(system, prompt).await,
            AnyProposer::OpenRouter(p) => p.propose(system, prompt).await,
        }
    }
}

/// A run's record, as the model reads it.
#[derive(Clone, Debug)]
pub struct Record {
    /// The run directory's name, or for a trajectory the task's name.
    pub run: String,
    pub task: String,
    pub text: String,
    /// Whether it's another agent's winning trajectory rather than a
    /// Microcoder run.
    pub trace: bool,
    /// Whether it pairs a failed Microcoder run with a winning trajectory on
    /// the same task.
    pub contrast: bool,
}

/// A failed Microcoder run and a winning trajectory on the same task, as one
/// record: each gets half of [`RECORD_CHARS`].
///
/// # Errors
///
/// When either can't be read.
pub fn contrast_record(run_dir: &Path, trajectory: &Path, task: &str) -> Result<Record, String> {
    let failed = record(run_dir)?;
    let won = trace_record(trajectory, task)?;
    let half = RECORD_CHARS / 2;
    let fit = |text: &str| {
        if text.chars().count() <= half {
            text.to_string()
        } else {
            format!("{}\n[…]\n{}", head(text, half / 2), tail(text, half / 2))
        }
    };
    Ok(Record {
        run: failed.run,
        task: task.to_string(),
        text: format!(
            "## The failed run\n\n{}\n\n## The winning trajectory\n\n{}",
            fit(&failed.text),
            fit(&won.text)
        ),
        trace: false,
        contrast: true,
    })
}

/// Text of an ATIF value that may be a string or a list of text parts.
fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| {
                p["text"]
                    .as_str()
                    .map_or_else(|| text_of(p), str::to_string)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Reads another agent's ATIF trajectory into a record of at most
/// [`RECORD_CHARS`] characters: the task, then each step's message, the
/// commands it ran, and the head and tail of what they printed. `task` names
/// the task, so no entry can name it.
///
/// # Errors
///
/// When the file can't be read or isn't a trajectory.
pub fn trace_record(path: &Path, task: &str) -> Result<Record, String> {
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(path)
            .map_err(|e| format!("can't read {}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let steps = doc["steps"]
        .as_array()
        .ok_or_else(|| format!("{} has no steps; is it an ATIF trajectory?", path.display()))?;
    let model = doc["agent"]["model_name"].as_str().unwrap_or("an agent");
    let mut instruction = String::new();
    let mut parts: Vec<String> = Vec::new();
    for step in steps {
        let message = text_of(&step["message"]);
        if step["source"] == "user" {
            if instruction.is_empty() {
                instruction = head(&message, 6_000);
            }
            continue;
        }
        let mut part = String::new();
        if !message.trim().is_empty() {
            part.push_str(&format!("{}\n", head(message.trim(), 1_200)));
        }
        for call in step["tool_calls"].as_array().cloned().unwrap_or_default() {
            let args = &call["arguments"];
            let shown = args["command"]
                .as_str()
                .or_else(|| args["content"].as_str())
                .map_or_else(|| args.to_string(), str::to_string);
            part.push_str(&format!(
                "[{}] {}\n",
                call["function_name"].as_str().unwrap_or("tool"),
                head(&shown, 1_500)
            ));
        }
        for result in step["observation"]["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let out = text_of(&result["content"]);
            let out = out.trim();
            if !out.is_empty() {
                part.push_str(&format!("→ {}\n", tail(&head(out, 2_000), 700)));
            }
        }
        if !part.is_empty() {
            parts.push(part);
        }
    }
    let header = format!(
        "A winning trajectory by {model}, {} steps. The task it solved:\n\n{instruction}\n\n# What it did\n\n",
        steps.len()
    );
    let budget = RECORD_CHARS.saturating_sub(header.len());
    let all = parts.concat();
    let body = if all.chars().count() <= budget {
        all
    } else {
        format!(
            "{}\n[… middle steps omitted …]\n{}",
            head(&all, budget / 2),
            tail(&all, budget / 2)
        )
    };
    Ok(Record {
        run: task.to_string(),
        task: task.to_string(),
        text: format!("{header}{body}"),
        trace: true,
        contrast: false,
    })
}

fn tail(text: &str, chars: usize) -> String {
    let count = text.chars().count();
    if count <= chars {
        text.to_string()
    } else {
        let skipped: String = text.chars().skip(count - chars).collect();
        format!("[…]{skipped}")
    }
}

fn head(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        text.to_string()
    } else {
        format!("{}[…]", text.chars().take(chars).collect::<String>())
    }
}

/// Reads a run directory's `summary.json` and `events.jsonl` into a record
/// of at most [`RECORD_CHARS`] characters.
///
/// # Errors
///
/// When the summary or events can't be read.
pub fn record(dir: &Path) -> Result<Record, String> {
    let run = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("the run directory has no name")?;
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("summary.json"))
            .map_err(|e| format!("can't read {}/summary.json: {e}", dir.display()))?,
    )
    .map_err(|e| format!("{}/summary.json: {e}", dir.display()))?;
    let events = std::fs::read_to_string(dir.join("events.jsonl"))
        .map_err(|e| format!("can't read {}/events.jsonl: {e}", dir.display()))?;
    let task = summary["task"].as_str().unwrap_or_default().to_string();
    let mut steps: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in events.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event["event"].as_str().unwrap_or_default() {
            "generated" => {
                if !current.is_empty() {
                    steps.push(std::mem::take(&mut current));
                }
                let action = &event["generated"]["action"]["Ok"];
                if action.is_null() {
                    current = format!(
                        "Step {}: the reply missed the format.\n",
                        event["step"].as_u64().unwrap_or(0)
                    );
                    continue;
                }
                current = format!(
                    "Step {}: {}\n",
                    event["step"].as_u64().unwrap_or(0),
                    head(action["rationale"].as_str().unwrap_or_default(), 600)
                );
                if action["finished"].as_bool() == Some(true) {
                    current.push_str("(said the task is finished)\n");
                }
            }
            "ran" => {
                let result = &event["result"];
                current.push_str(&format!(
                    "$ {}\n→ exit {}: {}\n",
                    head(result["command"].as_str().unwrap_or_default(), 500),
                    result["exit"],
                    tail(result["output"].as_str().unwrap_or_default().trim(), 400)
                ));
            }
            "tested" => {
                let results = event["results"].as_array().cloned().unwrap_or_default();
                let passing = results.iter().filter(|r| r["exit"] == json!(0)).count();
                current.push_str(&format!(
                    "acceptance tests: {passing} of {} pass\n",
                    results.len()
                ));
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        steps.push(current);
    }
    let outcome = &summary["outcome"];
    let used: Vec<&str> = outcome["knowledge"]
        .as_array()
        .map(|l| l.iter().filter_map(|u| u["id"].as_str()).collect())
        .unwrap_or_default();
    let header = format!(
        "Outcome: reward {} ({}), {} steps, ended by {}.\nKnowledge entries shown: {}.\n",
        summary["reward"],
        if summary["reward"].as_f64().is_some_and(|r| r >= 1.0) {
            "the task's tests passed"
        } else {
            "the task's tests did not all pass"
        },
        outcome["steps"],
        outcome["ending"],
        if used.is_empty() {
            "none".to_string()
        } else {
            used.join(", ")
        }
    );
    let verdict = format!(
        "\nThe task's verifier, last lines:\n{}\n",
        tail(
            summary["verifier_output"].as_str().unwrap_or_default(),
            1_500
        )
    );
    let budget = RECORD_CHARS.saturating_sub(header.len() + verdict.len());
    let all = steps.concat();
    let body = if all.chars().count() <= budget {
        all
    } else {
        format!(
            "{}\n[… later steps omitted …]\n{}",
            head(&all, budget * 2 / 5),
            tail(&all, budget * 3 / 5)
        )
    };
    Ok(Record {
        run,
        task,
        trace: false,
        contrast: false,
        text: format!("{header}\n{body}{verdict}"),
    })
}

/// The prompt: the existing entries' IDs and titles, then the record.
#[must_use]
pub fn prompt(record: &Record, base: &Base) -> String {
    let existing: Vec<String> = base
        .entries
        .iter()
        .map(|e| format!("- {} ({}): {}", e.id, e.kind, e.title))
        .collect();
    format!(
        "# Existing entries\n\n{}\n\n# The {}\n\n{}",
        existing.join("\n"),
        if record.contrast {
            "two records"
        } else if record.trace {
            "trajectory"
        } else {
            "run"
        },
        record.text
    )
}

/// A revision's body: the current body whole, then the proposal's body
/// under an "Added in version N" heading, its own headings one level down.
#[must_use]
pub fn merged_body(current: &str, proposal: &str, version: u32) -> String {
    let added: String = proposal
        .trim()
        .lines()
        .map(|line| {
            if line.starts_with('#') {
                format!("#{line}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    format!(
        "{}\n\n## Added in version {version}\n\n{}",
        current.trim_end(),
        added.trim_end()
    )
}

/// What happened to one proposal.
#[derive(Clone, Debug, PartialEq)]
pub enum Written {
    /// A new entry at this path.
    New(PathBuf),
    /// A new version of an existing entry: its path, and whether it waits
    /// in `versions/` because the current version is admitted.
    Version { path: PathBuf, pending: bool },
    /// Not written, and why.
    Refused(String),
}

/// A harvest's result.
#[derive(Clone, Debug)]
pub struct Harvest {
    /// Each proposal's ID and what became of it.
    pub proposals: Vec<(String, Written)>,
    /// What the model call cost.
    pub model_cost: Cost,
    /// Dollars the near-duplicate searches' embeddings cost, or `None` when
    /// that's unknown.
    pub embedding_usd: Option<f64>,
    /// Why a near-duplicate search ranked by words alone, when one did: its
    /// proposal was then matched only by ID and the model's `updates`.
    pub lexical: Vec<String>,
}

impl Harvest {
    /// Dollars in all, or `None` when any part is unknown.
    #[must_use]
    pub fn usd(&self) -> Option<f64> {
        Some(self.model_cost.usd? + self.embedding_usd?)
    }

    /// The known part of the cost: the total when it's known, else a lower
    /// bound.
    #[must_use]
    pub fn known_usd(&self) -> f64 {
        self.model_cost.known_usd + self.embedding_usd.unwrap_or(0.0)
    }
}

/// The existing entry a proposal revises: the one with its ID, else the
/// nearest one when their cosine similarity is [`DUPLICATE`] or more.
/// Without embeddings, the entry the model names in `updates` stands in
/// for the similarity check.
async fn original<E: Embed>(
    proposal: &Proposal,
    candidate: &Entry,
    base: &Base,
    retriever: Option<&Retriever<E>>,
    usd: &mut Option<f64>,
    lexical: &mut Vec<String>,
) -> Option<(String, Option<f64>)> {
    let id = proposal.id.trim();
    if base.get(id).is_some() {
        return Some((id.to_string(), None));
    }
    let named = proposal.updates.trim();
    let by_name = || base.get(named).map(|_| (named.to_string(), None));
    let Some(retriever) = retriever else {
        return by_name();
    };
    let search = retriever.search(&candidate.search_text(), 1).await;
    *usd = usd.zip(search.usd).map(|(a, b)| a + b);
    if let Some(why) = search.lexical_only {
        lexical.push(why);
        return by_name();
    }
    let hit = search.hits.first()?;
    let similarity = hit.semantic?;
    (similarity >= DUPLICATE).then(|| (hit.id.clone(), Some(similarity)))
}

/// Proposes entries from the run in `run_dir` and writes the ones that pass
/// the lint to `dir` as candidates. `retriever` finds near-duplicates; it
/// should search every entry in `dir`, candidates included.
///
/// # Errors
///
/// When the run can't be read or the model call fails.
pub async fn harvest<P: Propose, E: Embed>(
    run_dir: &Path,
    dir: &Path,
    proposer: &P,
    retriever: Option<&Retriever<E>>,
    corpus: &Corpus,
) -> Result<Harvest, String> {
    harvest_record(record(run_dir)?, dir, proposer, retriever, corpus).await
}

/// Proposes entries from a record already read, a Microcoder run's or
/// another agent's trajectory, and writes the ones that pass.
///
/// # Errors
///
/// When the model call fails.
pub async fn harvest_record<P: Propose, E: Embed>(
    record: Record,
    dir: &Path,
    proposer: &P,
    retriever: Option<&Retriever<E>>,
    corpus: &Corpus,
) -> Result<Harvest, String> {
    let (entries, _) = Base::read(dir);
    let base = Base { entries };
    let system = if record.contrast {
        CONTRAST_SYSTEM
    } else if record.trace {
        TRACE_SYSTEM
    } else {
        SYSTEM
    };
    let (proposals, model_cost) = proposer.propose(system, &prompt(&record, &base)).await?;
    let mut embedding_usd = Some(0.0);
    let mut lexical = Vec::new();
    let mut corpus = corpus.clone();
    if !record.task.is_empty() && !corpus.names.contains(&record.task) {
        corpus.names.push(record.task.clone());
    }
    let mut out = Vec::new();
    for proposal in proposals.entries.iter().take(MAX_PROPOSALS) {
        let id = proposal.id.trim().to_string();
        let refuse = |why: String| (id.clone(), Written::Refused(why));
        let Some(kind) = Kind::parse(proposal.kind.trim()) else {
            out.push(refuse(format!("unknown kind {}", proposal.kind)));
            continue;
        };
        if !valid_id(&id) {
            out.push(refuse(format!("the id `{id}` isn't valid")));
            continue;
        }
        let mut entry = Entry {
            id: id.clone(),
            version: 1,
            kind,
            title: proposal.title.trim().to_string(),
            summary: proposal.summary.trim().to_string(),
            // A tag that happens to equal a task's name, such as a domain
            // name, is split into its words so the lint doesn't refuse it.
            tags: proposal
                .tags
                .iter()
                .map(|t| t.trim().to_lowercase().replace(' ', "-"))
                .flat_map(|t| {
                    if corpus.names.contains(&t) {
                        t.split(['-', '_']).map(str::to_string).collect::<Vec<_>>()
                    } else {
                        vec![t]
                    }
                })
                .filter(|t| !t.is_empty())
                .collect(),
            applies_when: proposal.applies_when.trim().to_string(),
            status: Status::Candidate,
            author: format!("microcoder kb harvest ({})", proposer.model()),
            written_from: vec![record.run.clone()],
            cites: proposal
                .cites
                .iter()
                .map(|c| c.trim().to_string())
                .collect(),
            evidence: Vec::new(),
            body: proposal.body.trim().to_string(),
            digest: String::new(),
        };
        let revises = original(
            proposal,
            &entry,
            &base,
            retriever,
            &mut embedding_usd,
            &mut lexical,
        )
        .await;
        let current = revises.as_ref().and_then(|(id, _)| base.get(id)).cloned();
        if let Some(current) = &current {
            entry.id = current.id.clone();
            let newest = pending(dir, &current.id, current.version)
                .map_or(current.version, |(_, e)| e.version);
            entry.version = newest + 1;
            // The model sees only titles, so a revision keeps the current
            // body whole and adds the proposal under its own heading.
            entry.body = merged_body(&current.body, &entry.body, entry.version);
            entry.summary = current.summary.clone();
            entry.cites.extend(current.cites.iter().cloned());
            entry.cites.dedup();
            for from in &current.written_from {
                if from != "reference" && !entry.written_from.contains(from) {
                    entry.written_from.insert(0, from.clone());
                }
            }
        }
        let text = entry.render();
        let parsed = match Entry::parse(&text) {
            Ok(parsed) => parsed,
            Err(error) => {
                out.push(refuse(error));
                continue;
            }
        };
        let problems = lint(std::slice::from_ref(&parsed), &corpus);
        if let Some(problem) = problems.first() {
            out.push(refuse(format!(
                "{} lint problems; the first: {}",
                problems.len(),
                problem.message
            )));
            continue;
        }
        let written = match &current {
            None => {
                let path = dir.join(format!("{}.md", parsed.id));
                write_file(&path, &text).map(|()| Written::New(path))
            }
            Some(current) if current.status == Status::Admitted => {
                let path = crate::write::version_path(dir, &parsed.id, parsed.version);
                write_file(&path, &text).map(|()| Written::Version {
                    path,
                    pending: true,
                })
            }
            Some(current) => archive(dir, &current.id).and_then(|_| {
                let path = dir.join(format!("{}.md", parsed.id));
                write_file(&path, &text).map(|()| Written::Version {
                    path,
                    pending: false,
                })
            }),
        };
        out.push((parsed.id.clone(), written.unwrap_or_else(Written::Refused)));
    }
    Ok(Harvest {
        proposals: out,
        model_cost,
        embedding_usd,
        lexical,
    })
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("can't make {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
