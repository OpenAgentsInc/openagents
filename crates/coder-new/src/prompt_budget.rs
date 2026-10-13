//! What a Coder-loop turn sends before the conversation, and how big it
//! may be (#11550).
//!
//! [`system_parts`] is the one place the tool-calling loop
//! ([`crate::provider::Provider::chat_with_plugins`]) builds its system
//! messages, so [`sections`] measures exactly what is sent: each system
//! part and each tool family's JSON schemas, in estimated tokens.

use serde_json::Value;

use crate::plugin_tools::{ExecutionSettings, Family};

/// The most a Coder-loop turn's opening prefix (system parts and tool
/// schemas) may estimate at by [`sections`] (#11550). It measures about
/// 2,500 for a chat with every plugin on; Claude's tokenizer reads such a
/// prefix at roughly 1.4 times this estimate, so the cap keeps a plain
/// question's first call under the 5,000 reported prompt tokens the
/// acceptance gate checks ([`FIRST_TURN_REPORTED`]).
pub const FIRST_TURN_BUDGET: usize = 3_500;

/// The most prompt tokens a provider may report for the first call of a
/// one-line question on the Coder loop (#11550): the acceptance gate's
/// number.
pub const FIRST_TURN_REPORTED: u64 = 5_000;

/// The system messages a turn opens with, in order, each with the name the
/// measurement table gives it.
#[must_use]
pub fn system_parts(
    execution: &ExecutionSettings,
    model: &str,
    definitions: &[Value],
) -> Vec<(&'static str, String)> {
    let mut parts = Vec::new();
    // Coder's own identity first, whichever model answers (#11264).
    // The /models menu it names is the one this chat has (#11519).
    let menu = crate::identity::Menu::coder_loop(
        execution.openrouter,
        execution
            .connections
            .as_ref()
            .is_some_and(crate::provider::gateway_open),
        execution.claude,
    );
    parts.push(("identity", crate::identity::prompt_with(menu, model)));
    if let Some(standing) = execution
        .instructions
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        parts.push(("standing instructions", standing.to_owned()));
    }
    // The chat's briefing for this piece of work (#11473), or, when none
    // was chosen, which AGENTS.md/CLAUDE.md files apply and the saved
    // notes' index, read fresh each turn (#11176); the model reads a file
    // or a note when a task needs it (#11550).
    let memory_tools = definitions
        .iter()
        .any(|tool| tool["function"]["name"] == "remember");
    if let Some(context) = execution
        .briefing
        .as_ref()
        .map(|briefing| briefing.render(memory_tools))
        .or_else(|| {
            execution.memory.as_ref().map(|memory| {
                if execution.shell {
                    memory.index(crate::memory::INDEX_BUDGET)
                } else {
                    // No file tools to read them with: the files and
                    // notes themselves.
                    memory.context(memory_tools, crate::memory::PROVIDER_BUDGET)
                }
            })
        })
        .filter(|text| !text.trim().is_empty())
    {
        parts.push(("project instructions and memory", context));
    }
    if !definitions.is_empty() {
        // With the shell, a family's guidance comes with its tools when it
        // loads; without it every tool is sent, so all of it is here.
        parts.push((
            "tool instructions",
            if execution.shell {
                execution.core_instructions()
            } else {
                execution.instructions()
            },
        ));
    }
    parts
}

/// The tools a Coder-loop turn offers (#11550): the core, `load_tools`
/// while families are left to load, and the families it loaded. A loaded
/// family stays for the rest of the turn; its tools go after the ones
/// already sent, so the earlier prefix is unchanged.
#[derive(Clone, Debug, Default)]
pub struct Toolset {
    pub definitions: Vec<Value>,
    unloaded: Vec<(Family, Vec<Value>)>,
    loaded: Vec<Family>,
}

impl Toolset {
    /// The turn's opening tools: the core, and `load_tools` naming the
    /// families this chat has.
    /// A chat without the shell and file tools (a crew charter, a
    /// single-plugin chat) has no core to start from, so it sends every
    /// tool it has, as before.
    #[must_use]
    pub fn opening(execution: &ExecutionSettings) -> Self {
        if !execution.shell {
            return Self {
                definitions: execution.defs(),
                ..Self::default()
            };
        }
        let mut definitions = Vec::new();
        let mut unloaded = Vec::new();
        for (family, tools) in execution.families() {
            if family == Family::Core {
                definitions.extend(tools);
            } else {
                unloaded.push((family, tools));
            }
        }
        if !unloaded.is_empty() {
            let available: Vec<Family> = unloaded.iter().map(|(family, _)| *family).collect();
            definitions.push(crate::plugin_tools::load_tools_definition(&available));
        }
        Self {
            definitions,
            unloaded,
            loaded: Vec::new(),
        }
    }

    /// The families loaded so far.
    #[must_use]
    pub fn loaded(&self) -> &[Family] {
        &self.loaded
    }

    /// Runs a `load_tools` call: adds each named family's tools and
    /// returns their guidance for the tool result.
    ///
    /// # Errors
    /// The arguments name no family this chat has.
    pub fn load(
        &mut self,
        execution: &ExecutionSettings,
        arguments: &Value,
    ) -> Result<Value, String> {
        let names: Vec<&str> = arguments["families"]
            .as_array()
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if names.is_empty() {
            return Err("Name at least one family in families.".into());
        }
        let mut loaded = Vec::new();
        let mut guidance = String::new();
        let mut unknown = Vec::new();
        for name in names {
            let Some(family) = Family::parse(name) else {
                unknown.push(name.to_owned());
                continue;
            };
            if self.loaded.contains(&family) {
                loaded.push(family.id());
                continue;
            }
            let Some(at) = self.unloaded.iter().position(|(f, _)| *f == family) else {
                unknown.push(name.to_owned());
                continue;
            };
            let (_, tools) = self.unloaded.remove(at);
            self.definitions.extend(tools);
            self.loaded.push(family);
            loaded.push(family.id());
            guidance.push_str(&execution.family_instructions(family));
        }
        if loaded.is_empty() {
            return Err(format!(
                "No such tool family here: {}. Use the families load_tools lists.",
                unknown.join(", ")
            ));
        }
        let mut result = serde_json::json!({ "loaded": loaded });
        if !guidance.trim().is_empty() {
            result["guidance"] = Value::String(guidance);
        }
        if !unknown.is_empty() {
            result["unknown"] = serde_json::json!(unknown);
        }
        Ok(result)
    }
}

/// Whether a model on OpenRouter caches only at the request's
/// `cache_control` breakpoints: Anthropic's and Google's models, by their
/// exact provider prefix. OpenAI-style providers cache a repeated prefix
/// on their own.
#[must_use]
pub fn needs_breakpoints(model: &str) -> bool {
    let provider = model.split('/').next().unwrap_or_default();
    model.contains('/') && matches!(provider, "anthropic" | "google")
}

/// Marks `history`'s cache breakpoints for `model` (#11550): the last of
/// the opening system messages, so the tools and system prefix are read
/// back from the cache on every later call, and the newest user or tool
/// message, so a turn's later calls and the next turn read the
/// conversation so far. Two of Anthropic's four breakpoints.
pub fn mark_cache(history: &mut [Value], model: &str) {
    if !needs_breakpoints(model) {
        return;
    }
    let system = history
        .iter()
        .take_while(|message| message["role"] == "system")
        .count();
    let mut marks = Vec::new();
    if system > 0 {
        marks.push(system - 1);
    }
    if let Some(last) = history
        .iter()
        .rposition(|message| matches!(message["role"].as_str(), Some("user" | "tool")))
        .filter(|at| *at >= system)
    {
        marks.push(last);
    }
    for at in marks {
        let message = &mut history[at];
        let content = message["content"].take();
        message["content"] = match content {
            Value::String(text) if !text.is_empty() => serde_json::json!([
                {"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}
            ]),
            Value::Array(mut parts) if !parts.is_empty() => {
                if let Some(last) = parts.last_mut().filter(|part| part.is_object()) {
                    last["cache_control"] = serde_json::json!({"type": "ephemeral"});
                }
                Value::Array(parts)
            }
            other => other,
        };
    }
}

/// A token estimate for `text`: four bytes a token for prose, the usual
/// rule; JSON schemas run nearer three, so [`sections`] counts them so.
#[must_use]
pub fn estimate(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn estimate_json(text: &str) -> usize {
    text.len().div_ceil(3)
}

/// One row of the measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub bytes: usize,
    pub tokens: usize,
}

/// Every part of a turn's opening prefix: the system parts, then each
/// tool's schema.
#[must_use]
pub fn sections(execution: &ExecutionSettings, model: &str) -> Vec<Section> {
    let definitions = Toolset::opening(execution).definitions;
    let mut rows: Vec<Section> = system_parts(execution, model, &definitions)
        .into_iter()
        .map(|(name, text)| Section {
            name: name.to_owned(),
            bytes: text.len(),
            tokens: estimate(&text),
        })
        .collect();
    for tool in &definitions {
        let text = tool.to_string();
        rows.push(Section {
            name: format!(
                "tool {}",
                tool["function"]["name"].as_str().unwrap_or("(unnamed)")
            ),
            bytes: text.len(),
            tokens: estimate_json(&text),
        });
    }
    rows
}

/// The rows' total estimated tokens.
#[must_use]
pub fn total(rows: &[Section]) -> usize {
    rows.iter().map(|row| row.tokens).sum()
}

/// The rows as a Markdown table, largest first, with the total.
#[must_use]
pub fn table(rows: &[Section]) -> String {
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    let mut out = String::from("| section | bytes | ~tokens |\n|---|---:|---:|\n");
    for row in &sorted {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            row.name, row.bytes, row.tokens
        ));
    }
    out.push_str(&format!(
        "| **total** | {} | **{}** |\n",
        rows.iter().map(|row| row.bytes).sum::<usize>(),
        total(rows)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The settings a signed-in chat with every bundled plugin on has, in
    /// `cwd`, with memory read from `memory` (`None` leaves it off).
    pub(crate) fn everything_on(
        cwd: &std::path::Path,
        memory: Option<crate::memory::Memory>,
    ) -> ExecutionSettings {
        ExecutionSettings {
            prompt_inbox: None,
            fleet: None,
            connections: None,
            boat: Default::default(),
            gce: crate::cloud_settings::Configuration::gce(),
            cloud_root: "fixture-state".into(),
            microcoder: true,
            cli: true,
            acp: true,
            claude: true,
            openrouter: true,
            solver: true,
            comms: true,
            swarm: true,
            channel: None,
            jev_enabled: true,
            jev_key: None,
            redaction_keys: vec![],
            jev_model: "jev-latest".into(),
            jev_endpoint: "https://api.typesafe.ai".into(),
            agents: vec![],
            cwd: cwd.to_path_buf(),
            boundary: None,
            instructions: None,
            shell: true,
            brainstorm: None,
            disclosure_desk: None,
            harness: Some(crate::harness::Run::choose(
                None,
                crate::harness::DecidedBy::Fallback,
                999,
            )),
            memory,
            favor: crate::route::Favor::default(),
            routing: Some("fixture-routing.json".into()),
            briefing: None,
            gateway_model: None,
            cheap: None,
            door: None,
            no_harness: None,
        }
    }

    /// The owner's measurement (#11550): the prefix a turn in
    /// `CODER_PROMPT_CWD` sends, with the memory under
    /// `CODER_PROMPT_HOME/.openagents/memory`, read only. Run with
    /// `--ignored --nocapture`.
    #[test]
    #[ignore = "measures this computer's real instruction files"]
    fn print_the_prefix_table() {
        let cwd =
            std::path::PathBuf::from(std::env::var("CODER_PROMPT_CWD").expect("CODER_PROMPT_CWD"));
        let home = std::path::PathBuf::from(
            std::env::var("CODER_PROMPT_HOME").expect("CODER_PROMPT_HOME"),
        );
        let memory = crate::memory::Memory::at(
            home.join(".openagents/memory"),
            Some(home.join(".openagents/AGENTS.md")),
            Some(home),
            &cwd,
        );
        let rows = sections(
            &everything_on(&cwd, Some(memory)),
            "anthropic/claude-fable-5.1",
        );
        println!("{}", table(&rows));
    }

    /// #11550: a plain question's first call stays in budget with every
    /// plugin on and a large AGENTS.md in the repository, because the
    /// instruction files and notes go by index and the plugin families
    /// load on demand.
    #[test]
    fn a_plain_questions_prefix_stays_in_budget() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join("AGENTS.md"), "Rules.\n".repeat(10_000)).unwrap();
        let memory = crate::memory::Memory::at(
            dir.path().join("memory"),
            None,
            Some(dir.path().to_path_buf()),
            &repo,
        );
        for index in 0..60 {
            memory
                .remember(&crate::memory::Note {
                    name: format!("Note {index}"),
                    kind: "feedback".into(),
                    body: "A long body. ".repeat(200),
                    description: Some("A note the index lists in one line. ".repeat(4)),
                    scope: None,
                })
                .unwrap();
        }
        let rows = sections(
            &everything_on(&repo, Some(memory)),
            "anthropic/claude-fable-5.1",
        );
        let total = total(&rows);
        assert!(
            total <= FIRST_TURN_BUDGET,
            "the opening prefix is ~{total} tokens, over {FIRST_TURN_BUDGET}:\n{}",
            table(&rows)
        );
        let context = rows
            .iter()
            .find(|row| row.name == "project instructions and memory")
            .unwrap();
        assert!(context.bytes <= crate::memory::INDEX_BUDGET);
        // Every plugin's tools are still there, a load away.
        let names: Vec<String> = rows.iter().map(|row| row.name.clone()).collect();
        assert!(names.contains(&"tool Run".to_owned()));
        assert!(names.contains(&format!("tool {}", crate::plugin_tools::LOAD_TOOLS)));
        assert!(!names.contains(&"tool openagents_cli".to_owned()));
    }

    /// #11550: `load_tools` adds a family's tools after the ones already
    /// sent and returns its guidance; an unknown family is refused.
    #[test]
    fn load_tools_adds_a_family_and_its_guidance() {
        let dir = tempfile::tempdir().unwrap();
        let execution = everything_on(dir.path(), None);
        let mut toolset = Toolset::opening(&execution);
        let before = toolset.definitions.clone();
        let loaded = toolset
            .load(
                &execution,
                &serde_json::json!({"families": ["github", "workflow"]}),
            )
            .unwrap();
        assert_eq!(loaded["loaded"], serde_json::json!(["github", "workflow"]));
        assert!(loaded["guidance"].as_str().unwrap().contains("github tool"));
        assert_eq!(&toolset.definitions[..before.len()], &before[..]);
        let names: Vec<&str> = toolset
            .definitions
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect();
        assert!(names.contains(&"github") && names.contains(&"workflow"));
        assert!(
            toolset
                .load(&execution, &serde_json::json!({"families": ["nope"]}))
                .is_err()
        );
        // The full instructions still carry every family's guidance.
        assert!(execution.instructions().contains("github tool"));
    }

    /// #11550: Anthropic and Google models on OpenRouter get cache
    /// breakpoints on the system prefix and the newest message; others
    /// are left as they are.
    #[test]
    fn cache_breakpoints_mark_the_prefix_and_the_newest_message() {
        let history = vec![
            serde_json::json!({"role":"system","content":"identity"}),
            serde_json::json!({"role":"system","content":"tools"}),
            serde_json::json!({"role":"user","content":"hi"}),
            serde_json::json!({"role":"assistant","content":"hello"}),
            serde_json::json!({"role":"user","content":"what can you do"}),
        ];
        let mut marked = history.clone();
        mark_cache(&mut marked, "anthropic/claude-fable-5.1");
        assert_eq!(marked[0], history[0]);
        assert_eq!(
            marked[1]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(marked[1]["content"][0]["text"], "tools");
        assert_eq!(marked[2], history[2]);
        assert_eq!(
            marked[4]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        let mut plain = history.clone();
        mark_cache(&mut plain, "openai/gpt-6.1-sol");
        assert_eq!(plain, history);
    }

    /// #11550 live check: two turns of a plain question on
    /// `CODER_LIVE_MODEL` through OpenRouter with the key in
    /// `OPENROUTER_LIVE_KEY`, printing each call's prompt tokens and its
    /// cache reads and writes. Costs a few cents; run by hand only.
    #[test]
    #[ignore = "a live, billed call"]
    fn live_prompt_size_and_cache() {
        let key = std::env::var("OPENROUTER_LIVE_KEY").expect("OPENROUTER_LIVE_KEY");
        let model = std::env::var("CODER_LIVE_MODEL").expect("CODER_LIVE_MODEL");
        let cwd =
            std::path::PathBuf::from(std::env::var("CODER_PROMPT_CWD").expect("CODER_PROMPT_CWD"));
        let execution = everything_on(&cwd, None);
        let toolset = Toolset::opening(&execution);
        let mut history: Vec<Value> = system_parts(&execution, &model, &toolset.definitions)
            .into_iter()
            .map(|(_, content)| serde_json::json!({"role":"system","content":content}))
            .collect();
        let client =
            openrouter::Client::new(openrouter::Config::new(openrouter::ApiKey::new(&key)))
                .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for question in ["what can you do", "and in one sentence?"] {
            history.push(serde_json::json!({"role":"user","content":question}));
            let mut sent = history.clone();
            mark_cache(&mut sent, &model);
            let request = openrouter::ChatRequest::new(&model, vec![]).max_tokens(300);
            let streamed = runtime
                .block_on(client.stream_tools_for_repair(
                    &request,
                    &sent,
                    &toolset.definitions,
                    &mut |_| {},
                    &mut |_| {},
                ))
                .unwrap();
            let usage = &streamed.reply.usage;
            println!(
                "{question:?}: prompt {} completion {} cached {:?} written {:?} cost {:?}",
                usage.prompt_tokens,
                usage.completion_tokens,
                usage
                    .prompt_tokens_details
                    .as_ref()
                    .and_then(|d| d.cached_tokens),
                usage
                    .prompt_tokens_details
                    .as_ref()
                    .and_then(|d| d.cache_write_tokens),
                usage.cost
            );
            history.push(serde_json::json!({"role":"assistant","content":streamed.reply.text}));
        }
    }
}
