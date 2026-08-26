//! The tools a session declares to the model, and what running them does.
//!
//! Five tools: `shell`, `skill`, `openagents`, `capability`, and — only where
//! a delegation gate exists — `delegate`. Each is declared to the model and
//! each has an implementation in [`HarnessToolRegistry::execute_tool`]; the
//! list and the match arms carry the same names, which is the only property
//! that keeps a declared tool from being a promise nothing keeps. This
//! module's header once claimed `capability` while `list_tools` did not
//! declare it and no arm implemented it; the rule the mistake bought is that
//! a name is written here only after something answers it.
//!
//! `capability` searches the local catalog of digest-pinned WebAssembly
//! plugins and loads one into the session, at which point the plugin's own
//! manifest declares a second tool under its own name. The sandbox that runs
//! it — digest verification, import inspection, confined read-only mounts,
//! the memory ceiling, the timeout — is [`crate::plugins`]. A plugin whose
//! limits cannot be enforced does not run.
//!
//! The tool runtime is the client's. The inference proxy forwards the
//! declarations and returns the calls the model asks for; nothing runs
//! server-side.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::plugins::{
    self, answer_capability, capability_tool_definition, plugin_tool_definition, Approval,
    CatalogEntry, LoadedPlugin,
};

pub const OUTPUT_LIMIT: usize = 30_000;
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const MAXIMUM_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
}

/// One skill: a directory holding a `SKILL.md` with YAML front matter naming
/// it and saying when it applies, then a body of instructions.
///
/// The format is shared with the other agents that read this repository, so
/// the same file serves all of them and none of them owns it.
#[derive(Debug, Clone)]
pub struct SkillInfo {
    /// The name the model asks for, from the front matter.
    pub name: String,
    /// When to use it, from the front matter. The sentence the model chooses
    /// on, and the reason a body is never in the catalog.
    pub description: String,
    /// The instructions, front matter removed.
    pub body: String,
    /// Whether the body is put in front of the model without being asked for.
    ///
    /// Set with `auto: true` in the front matter. The catalog exists so a body
    /// is read only when it is wanted; a skill that says how to approach the
    /// work is the exception, because a session needs the method before its
    /// first decision and will not think to ask for it. Every auto-loaded body
    /// is paid for on every turn, so it is used sparingly.
    pub auto: bool,
    /// Where it was read from, so a reader can open it.
    pub path: PathBuf,
}

/// How much of one skill body is handed back.
pub const SKILL_BODY_LIMIT: usize = 32_000;

/// What the `delegate` tool is allowed to start.
///
/// Present on the session the reader is talking to and absent on the children
/// it starts. A fan-out whose children fan out has no ceiling: three children
/// each starting three is nine agents on one grant, and none of them told the
/// reader.
#[derive(Debug, Clone)]
pub struct DelegationGate {
    /// The lane children run on.
    pub lane: String,
    /// The credential children spend against.
    pub user_token: Option<String>,
    /// The most children one call may start.
    pub max_count: usize,
}

pub struct HarnessToolRegistry {
    pub cwd: PathBuf,
    /// Discovered skills by name, in catalog order.
    pub skills: BTreeMap<String, SkillInfo>,
    /// `None` on a delegated child, so it cannot delegate further.
    pub delegation: Option<DelegationGate>,
    /// The digest-pinned WebAssembly plugins installed at or above `cwd`.
    pub catalog: Vec<CatalogEntry>,
    /// Which capability tiers may load in this session. Pure compute always
    /// may; read-only mounts need an operator and refuse without one.
    pub plugin_approval: Approval,
    /// Plugins the model loaded through `capability`, in load order. Each one
    /// declares a further tool under its own manifest name.
    loaded: Mutex<Vec<Arc<LoadedPlugin>>>,
}

impl HarnessToolRegistry {
    pub fn new(cwd: Option<PathBuf>) -> Self {
        Self::build(cwd, None)
    }

    /// The registry a session gets when it may start children.
    pub fn with_delegation(cwd: Option<PathBuf>, gate: DelegationGate) -> Self {
        Self::build(cwd, Some(gate))
    }

    /// The registry a delegated child gets: rooted at the child's own
    /// directory, and with no `delegate` tool.
    pub fn child(cwd: Option<PathBuf>) -> Self {
        Self::build(cwd, None)
    }

    /// Grant the read-only mount tier, for a caller with an operator behind
    /// it. Without this a plugin that declares mounts refuses to load, which
    /// is the safe default for an unattended session.
    pub fn allowing_plugin_mounts(mut self) -> Self {
        self.plugin_approval.mounts_allowed = true;
        self
    }

    fn build(cwd: Option<PathBuf>, delegation: Option<DelegationGate>) -> Self {
        let root = cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let catalog = plugins::discover_catalog(&root);
        let mut registry = Self {
            cwd: root,
            skills: BTreeMap::new(),
            delegation,
            catalog,
            plugin_approval: Approval::default(),
            loaded: Mutex::new(Vec::new()),
        };
        registry.load_local_skills();
        registry
    }

    /// Where skills live, nearest first. The first source to claim a name
    /// keeps it, so a repository or a person replaces a shipped skill by
    /// writing one of the same name and nothing has to be uninstalled.
    fn skill_directories(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.cwd.join(".agents").join("skills")];
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join(".agents").join("skills"));
        }
        // The skills this CLI ships, read from the package they live in.
        dirs.push(self.cwd.join("packages").join("openagents-cli").join("skills"));
        dirs
    }

    /// Read every skill this workspace offers.
    ///
    /// A directory that is missing, unreadable, or holds no `SKILL.md`
    /// contributes nothing: a skills directory is optional, and a session in a
    /// repository without one is a session with no skills rather than a
    /// session that failed to start. A `SKILL.md` with no `name` cannot be
    /// asked for and one with no `description` gives the model nothing to
    /// choose on, so both are required and a file missing either is skipped.
    pub fn load_local_skills(&mut self) {
        for dir in self.skill_directories() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            let mut names: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            names.sort();
            for path in names {
                let skill_md = path.join("SKILL.md");
                if !skill_md.is_file() {
                    continue;
                }
                let Ok(content) = fs::read_to_string(&skill_md) else {
                    continue;
                };
                let Some((name, description, auto)) = parse_skill_front_matter(&content) else {
                    continue;
                };
                if self.skills.contains_key(&name) {
                    continue;
                }
                self.skills.insert(
                    name.clone(),
                    SkillInfo {
                        name,
                        description,
                        body: skill_body(&content),
                        auto,
                        path: skill_md,
                    },
                );
            }
        }
    }

    /// The standing context a session starts with: every auto-loaded skill
    /// body, in one block, plus what this workspace is when it is one of the
    /// two OpenAgents repositories.
    ///
    /// `None` when there is none, so a caller adds nothing rather than an
    /// empty heading.
    pub fn standing_context(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(workspace) = openagents_workspace_note(&self.cwd) {
            parts.push(workspace);
        }
        for skill in self.skills.values() {
            if !skill.auto {
                continue;
            }
            parts.push(format!(
                "The `{}` skill, which applies to this session:\n\n{}",
                skill.name, skill.body
            ));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }

    /// The plugins loaded into this session so far.
    pub fn loaded_plugins(&self) -> Vec<Arc<LoadedPlugin>> {
        self.loaded.lock().map(|held| held.clone()).unwrap_or_default()
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let mut skill_list = String::new();
        for (name, info) in &self.skills {
            skill_list.push_str(&format!("\n- `{}`: {}", name, info.description));
        }

        let mut tools = vec![
            ToolDefinition {
                name: "shell".to_string(),
                description: format!(
                    "Run a shell command on this machine. The working directory is {}, so paths are \
                    relative to it and you do not need to ask where you are. Returns combined stdout \
                    and stderr with the exit code. Batch independent commands into one call with && \
                    instead of one call each: every call replays the conversation so far.",
                    self.cwd.display()
                ),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "The command line to run through /bin/sh -c."},
                        "timeout_seconds": {"type": "integer", "description": "How long to wait. Defaults to 120; raise for a build or test run."}
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "skill".to_string(),
                description: format!("Read one of this repository skill procedures: a written procedure with conventions, commands, and rules. Call it before doing work a skill covers. Skills available:{}", skill_list),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "The skill to read."}
                    },
                    "required": ["name"]
                }),
            },
            ToolDefinition {
                name: "openagents".to_string(),
                description: "Run the OpenAgents CLI commands (issue, project, repo, auth, etc.). Pass the arguments as a list without openagents itself.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "args": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "The arguments after openagents as a list."
                        }
                    },
                    "required": ["args"]
                }),
            },
        ];

        // The standing capability tool. Constant-size: it names no installed
        // plugin, so the declaration does not grow as the catalog does.
        tools.push(capability_tool_definition());

        // Declared only where it can be run. A child's registry has no gate,
        // so a child neither sees the tool nor can call it.
        if let Some(gate) = &self.delegation {
            tools.push(ToolDefinition {
                name: "delegate".to_string(),
                description: format!(
                    "Run one prompt on independent child coding agents in parallel and return what \
                    each one found or did. Use it when work splits into parts that do not depend on \
                    each other: several files to change the same way, several hypotheses to check, \
                    several tests to run down. Each child is a full coding agent with its own shell \
                    tool, working in a git worktree of its own so children cannot overwrite each \
                    other, and it starts with no context from this conversation and cannot ask \
                    questions — so the prompt has to be self-contained. Every child runs the same \
                    prompt and each is told separately which number it is, so write the prompt for \
                    whichever child reads it: say \"read the file at your own number\" rather than \
                    naming one child. Children run on {} and on this session's budget. Prefer one \
                    call with a count over several calls, and prefer `shell` over this for a single \
                    command — a child agent is for work worth a whole agent, not one line of \
                    output. At most {} children.",
                    gate.lane, gate.max_count
                ),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The complete, self-contained instruction every child performs. Name the files, the command, and what to report back."
                        },
                        "count": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": gate.max_count,
                            "description": "How many children run this prompt. Defaults to 1."
                        }
                    },
                    "required": ["prompt"]
                }),
            });
        }

        // A plugin the model loaded through `capability` declares a tool of
        // its own, under its manifest name and over its manifest's input
        // schema. Nothing appears here that has not been digest-verified,
        // import-inspected, and instantiated at least once at load.
        for plugin in self.loaded_plugins() {
            tools.push(plugin_tool_definition(&plugin));
        }

        tools
    }

    pub async fn execute_tool(&self, call: &ToolCall) -> ToolOutput {
        match call.name.as_str() {
            "shell" => {
                let cmd = call.arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
                let timeout_secs = call.arguments.get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(DEFAULT_TIMEOUT_SECS)
                    .min(MAXIMUM_TIMEOUT_SECS);

                if let Some(refusal) = check_shell_refusal(cmd) {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: refusal,
                        is_error: true,
                    };
                }

                let output_str = run_real_shell(cmd, &self.cwd, timeout_secs).await;
                ToolOutput {
                    call_id: call.id.clone(),
                    output: output_str,
                    is_error: false,
                }
            }
            "skill" => {
                let name = call.arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(skill_info) = self.skills.get(name) {
                    ToolOutput {
                        call_id: call.id.clone(),
                        output: render_skill(skill_info),
                        is_error: false,
                    }
                } else {
                    ToolOutput {
                        call_id: call.id.clone(),
                        output: format!("Skill '{}' not found.", name),
                        is_error: true,
                    }
                }
            }
            "openagents" => {
                let args_array = call.arguments.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect::<Vec<_>>())
                    .unwrap_or_default();

                let output_str = run_openagents_cli(&args_array).await;
                ToolOutput {
                    call_id: call.id.clone(),
                    output: output_str,
                    is_error: false,
                }
            }
            "delegate" => {
                let Some(gate) = &self.delegation else {
                    // Reachable only if a model invents the name, since the
                    // tool is not declared without a gate.
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: "This session cannot start child agents.".to_string(),
                        is_error: true,
                    };
                };

                let prompt = call
                    .arguments
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if prompt.is_empty() {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: "No children were started: `prompt` is required and must say what the child does.".to_string(),
                        is_error: true,
                    };
                }

                let count = call
                    .arguments
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, gate.max_count as u64) as usize;

                let report = crate::delegate::fanout_for_tool(
                    &prompt,
                    count,
                    &gate.lane,
                    gate.user_token.clone(),
                )
                .await;

                ToolOutput {
                    call_id: call.id.clone(),
                    output: report,
                    is_error: false,
                }
            }
            "capability" => {
                let (text, loaded) = answer_capability(
                    &self.catalog,
                    self.plugin_approval,
                    &self.cwd,
                    &call.arguments,
                );
                // A refusal is text the model can act on, not an error the
                // turn dies of; the only `is_error` here is the absence of a
                // plugin where one was named.
                let is_error = loaded.is_none()
                    && call.arguments.get("name").and_then(|v| v.as_str()).is_some();
                if let Some(plugin) = loaded {
                    if let Ok(mut held) = self.loaded.lock() {
                        held.retain(|existing| existing.manifest.name != plugin.manifest.name);
                        held.push(Arc::new(plugin));
                    }
                }
                ToolOutput {
                    call_id: call.id.clone(),
                    output: text,
                    is_error,
                }
            }
            other => {
                // A loaded plugin answers under its own manifest name.
                let plugin = self
                    .loaded_plugins()
                    .into_iter()
                    .find(|plugin| plugin.manifest.name == other);
                match plugin {
                    Some(plugin) => ToolOutput {
                        call_id: call.id.clone(),
                        output: plugins::run_plugin_text(plugin, &call.arguments).await,
                        is_error: false,
                    },
                    None => ToolOutput {
                        call_id: call.id.clone(),
                        output: format!("Unknown tool: {}", call.name),
                        is_error: true,
                    },
                }
            }
        }
    }
}

/// Where the front matter ends, as a byte offset of the closing `\n---`.
fn front_matter_end(content: &str) -> Option<usize> {
    if !content.starts_with("---") {
        return None;
    }
    content[3..].find("\n---").map(|at| at + 3)
}

/// Read `name`, `description`, and `auto` out of YAML front matter.
///
/// Deliberately not a YAML parser. These are three bounded scalar fields at
/// the top of a known file, and a dependency that can parse anchors and merge
/// keys is a dependency that can also do something surprising with a file
/// anyone may drop in a skills directory.
///
/// `>` and `|` say the value is the indented block beneath, which is how a
/// description longer than a line is written. Taking the marker as the value
/// is how a skill came to describe itself as ">-".
fn parse_skill_front_matter(content: &str) -> Option<(String, String, bool)> {
    let end = front_matter_end(content)?;
    let lines: Vec<&str> = content[3..end].split('\n').collect();

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut auto = false;

    for (at, line) in lines.iter().enumerate() {
        if let Some(value) = line.strip_prefix("auto:") {
            let value = value.trim();
            if value == "true" || value == "false" {
                auto = value == "true";
                continue;
            }
        }
        let (key, inline) = if let Some(rest) = line.strip_prefix("name:") {
            ("name", rest.trim())
        } else if let Some(rest) = line.strip_prefix("description:") {
            ("description", rest.trim())
        } else {
            continue;
        };

        let value = if inline.is_empty() || is_block_marker(inline) {
            let mut block = Vec::new();
            for next in &lines[at + 1..] {
                if next.trim().is_empty() || !next.starts_with([' ', '\t']) {
                    break;
                }
                block.push(next.trim());
            }
            if block.is_empty() {
                continue;
            }
            // A folded block is one paragraph; a literal one keeps its breaks.
            block.join(if inline.starts_with('|') { "\n" } else { " " })
        } else {
            unquote(inline)
        };

        match key {
            "name" => name = Some(value),
            _ => description = Some(value),
        }
    }

    // A skill with no name cannot be asked for, and one with no description
    // gives the model nothing to choose on. Both are required.
    Some((name?, description?, auto))
}

/// `>`, `|`, and their chomping variants: a marker, never a value.
fn is_block_marker(inline: &str) -> bool {
    let mut chars = inline.chars();
    matches!(chars.next(), Some('>') | Some('|'))
        && matches!(chars.next(), None | Some('-') | Some('+'))
        && chars.next().is_none()
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if value.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && (bytes[value.len() - 1] == b'"' || bytes[value.len() - 1] == b'\'')
    {
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

/// The body after the front matter, or the whole file when there is none.
fn skill_body(content: &str) -> String {
    match front_matter_end(content) {
        None => content.trim().to_string(),
        Some(end) => match content[end + 1..].find('\n') {
            Some(at) => content[end + 1 + at + 1..].trim().to_string(),
            None => String::new(),
        },
    }
}

/// What a skill hands back when it is read, bounded so one file cannot spend
/// a whole context on itself.
pub fn render_skill(skill: &SkillInfo) -> String {
    let body = if skill.body.len() > SKILL_BODY_LIMIT {
        let mut cut = SKILL_BODY_LIMIT;
        while cut > 0 && !skill.body.is_char_boundary(cut) {
            cut -= 1;
        }
        format!(
            "{}\n\n[truncated; the rest is in {}]",
            &skill.body[..cut],
            skill.path.display()
        )
    } else {
        skill.body.clone()
    };
    format!("Skill `{}` ({}):\n\n{}", skill.name, skill.path.display(), body)
}

/// What the two OpenAgents repositories are, when the session is in one.
///
/// A session in `openagents.com` spent turns working out that it was in the
/// Phoenix application, and one in `openagents` that the CLI lives under
/// `packages/`. Both are facts about the workspace rather than about the work,
/// and neither is discoverable without reading around.
fn openagents_workspace_note(cwd: &Path) -> Option<String> {
    let shown = cwd.to_string_lossy();
    if !shown.to_lowercase().contains("openagents") {
        return None;
    }
    let mut lines = vec![
        format!(
            "This session is working in {shown}, which is part of OpenAgents. Two repositories \
             carry most of the work, and they are easy to confuse:"
        ),
        String::new(),
        "- **`openagents.com`** is the web application: a Phoenix and Elixir codebase serving the"
            .to_string(),
        "  site, the forge, and the `/api/v1` API. Its issues are the site's issues.".to_string(),
        "- **`openagents`** is the monorepo: the `openagents` CLI lives in".to_string(),
        "  `packages/openagents-cli`, alongside the other packages. Its issues are the CLI's and \
         the monorepo's."
            .to_string(),
        String::new(),
        "They are separate repositories with separate issue lists, so name the one you mean when \
         you read or write issues, and do not assume the current directory is the one being asked \
         about."
            .to_string(),
    ];

    // Where the other one is, when it is checked out beside this one. Naming
    // the two without saying where the other lives sent a session grepping the
    // whole workspace root, which holds every read-only reference clone, and
    // it spent the tool's whole budget before being stopped.
    if let Some(sibling) = sibling_checkout(cwd) {
        let parent = sibling.parent().map(|p| p.display().to_string()).unwrap_or_default();
        lines.push(String::new());
        lines.push(format!(
            "The other one is checked out at `{}`. To search or read it, change directory first — \
             `cd {} && git grep …`. `git grep` refuses a path outside the repository it is run in, \
             and it is the one command most likely to be reached for here.",
            sibling.display(),
            sibling.display()
        ));
        lines.push(String::new());
        lines.push(format!(
            "Do not search `{parent}` itself. It is the workspace root, and it holds large \
             read-only clones of other people's repositories; a recursive grep there does not \
             finish. Search one repository at a time."
        ));
    }

    Some(lines.join("\n"))
}

/// The other OpenAgents repository, if it is checked out beside this one.
///
/// Checked rather than assumed: a machine with only one of the two would
/// otherwise be told to `cd` somewhere that does not exist, which is a worse
/// instruction than none.
fn sibling_checkout(cwd: &Path) -> Option<PathBuf> {
    let here = cwd.file_name()?.to_str()?;
    let other = match here {
        "openagents.com" => "openagents",
        "openagents" => "openagents.com",
        _ => return None,
    };
    let path = cwd.parent()?.join(other);
    path.join(".git").exists().then_some(path)
}

/// Commands that cannot be undone, and are never what was meant.
///
/// ## What this list is, and is not
///
/// It stops a small number of irreversible mistakes: erasing a home directory
/// or a disk, reformatting, halting the machine. It is not a security boundary
/// and cannot be one — a command can be assembled from variables, decoded, or
/// written to a file and run, and no list of patterns sees that. It catches
/// the accident, not the intent.
///
/// So it is kept short and aimed only at what cannot be undone. `rm -rf` on a
/// build directory is ordinary work and is allowed; `rm -rf` on `/` or `~` is
/// not, because no one means it.
///
/// Each pattern is paired with what to say, because a bare refusal reads as
/// the tool being broken rather than as the command being the problem.
const REFUSED: &[(&str, &str)] = &[
    // A recursive `rm` aimed at a root, a home, or everything in one. Aimed at
    // a build directory it is ordinary work, so the target is what decides:
    // `rm -rf target/debug` runs and `rm -rf ~/` does not. The flag run has to
    // carry an `r`, long options are allowed between the flags and the target
    // (`--no-preserve-root` is exactly the phrase that precedes the worst
    // version of this), and a trailing `/` or `/*` on the target still names
    // the same thing.
    (
        r"(?i)\brm\s+(--?[a-zA-Z][a-zA-Z-]*\s+)*-[a-zA-Z]*r[a-zA-Z]*\s+(--?[a-zA-Z][a-zA-Z-]*\s+)*(/|~|\$HOME|\$\{HOME\})(/\*|/)?(\s|$)",
        "That would erase a root or a home directory.",
    ),
    (r"(?i)\bmkfs(\.\w+)?\b", "That would reformat a filesystem."),
    (
        r"(?i)\bdd\b[^\n]*\bof=/dev/(disk|rdisk|sd|nvme|hd)",
        "That would write over a raw device.",
    ),
    (
        r"(?i)\bdiskutil\s+(erase|reformat|partition)",
        "That would erase or repartition a disk.",
    ),
    // Anchored to command position — the start of the line, after a `;`, `&&`,
    // `||`, or a pipe, or behind `sudo`. A bare word match refused
    // `echo 'shutdown the server' >> notes.md`, and a gate that refuses
    // ordinary work is one an agent learns to route around.
    (
        r"(?i)(^|[;&|]\s*|\bsudo\s+)(shutdown|reboot|halt|poweroff)\b",
        "That would stop this machine.",
    ),
    (r":\(\)\s*\{\s*:\|:&\s*\}\s*;\s*:", "That is a fork bomb."),
    (
        r"(?i)\bchmod\s+(-[a-zA-Z]+\s+)*(-R|--recursive)\s+[0-7]{3,4}\s+(/|~|\$HOME)(\s|$)",
        "That would change the permissions of a whole root or home directory.",
    ),
    (
        r"(?i)>\s*/dev/(disk|rdisk|sd|nvme|hd)",
        "That would write over a raw device.",
    ),
];

/// Why this command will not be run, or `None` when it will.
pub fn check_shell_refusal(cmd: &str) -> Option<String> {
    for (pattern, reason) in REFUSED {
        // A pattern that does not compile is a bug in this table, not a reason
        // to let the command through, so it is skipped loudly in debug and
        // treated as no-match otherwise.
        let Ok(re) = regex::Regex::new(pattern) else {
            debug_assert!(false, "the refusal pattern `{pattern}` does not compile");
            continue;
        };
        if re.is_match(cmd) {
            return Some(format!(
                "{reason} This session refuses it. If you meant something narrower, name the \
                 directory."
            ));
        }
    }
    None
}

async fn run_real_shell(cmd: &str, cwd: &Path, timeout_secs: u64) -> String {
    let child = match Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return format!("Failed to spawn shell command: {}", e),
    };

    let execution = child.wait_with_output();
    match timeout(Duration::from_secs(timeout_secs), execution).await {
        Ok(Ok(output)) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));

            let total_len = combined.len();
            let bounded = if total_len > OUTPUT_LIMIT {
                format!("{}\n\n[Output truncated: printed {} characters, limit is {}]", &combined[..OUTPUT_LIMIT], total_len, OUTPUT_LIMIT)
            } else {
                combined
            };

            if output.status.success() {
                if bounded.trim().is_empty() {
                    "The command succeeded and printed nothing.".to_string()
                } else {
                    bounded.trim().to_string()
                }
            } else {
                let code = output.status.code().unwrap_or(1);
                format!("The command exited with code {}.\n\n{}", code, bounded.trim())
            }
        }
        Ok(Err(e)) => format!("Shell execution error: {}", e),
        Err(_) => format!("The command timed out after {} seconds and was stopped.", timeout_secs),
    }
}

async fn run_openagents_cli(args: &[String]) -> String {
    let mut cmd = Command::new("openagents");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match cmd.output().await {
        Ok(output) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            combined.trim().to_string()
        }
        Err(e) => format!("Failed to run openagents CLI: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, dir: &str, source: &str) {
        let at = root.join(".agents").join("skills").join(dir);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join("SKILL.md"), source).unwrap();
    }

    // ───────────────────────────────────────────── the destructive-command gate

    #[test]
    fn the_gate_refuses_a_destructive_command_that_is_no_literal_it_knows() {
        // Every one of these passed the three-string check this replaced.
        for command in [
            "rm -fr ~/",
            "rm -rf --no-preserve-root /",
            "sudo rm -Rf $HOME/*",
            "mkfs.ext4 /dev/sda1",
            "dd if=/dev/zero of=/dev/disk2 bs=1m",
            "diskutil eraseDisk JHFS+ Blank /dev/disk3",
            "sudo shutdown -h now",
            ":(){ :|:& };:",
            "chmod -R 777 /",
            "cat payload > /dev/rdisk0",
        ] {
            assert!(
                check_shell_refusal(command).is_some(),
                "`{command}` should be refused"
            );
        }
    }

    #[test]
    fn the_gate_leaves_ordinary_work_alone() {
        // A refusal list that catches ordinary commands is one an agent learns
        // to work around, which is worse than not having it.
        for command in [
            "rm -rf target/debug",
            "rm -rf ./node_modules",
            "cargo test -p openagents-cli",
            "git rm -r --cached .",
            "mkdir -p /tmp/build && dd if=in of=out",
            "echo 'shutdown the server' >> notes.md",
        ] {
            assert_eq!(
                check_shell_refusal(command),
                None,
                "`{command}` is ordinary work and should run"
            );
        }
    }

    #[test]
    fn a_refusal_says_what_the_command_would_have_done() {
        let refusal = check_shell_refusal("rm -rf ~/").expect("refused");
        assert!(refusal.contains("erase a root or a home directory"), "{refusal}");
        assert!(refusal.contains("name the directory"), "{refusal}");
    }

    // ─────────────────────────────────────────────────── skills, as they are read

    #[test]
    fn a_block_scalar_description_is_the_block_and_not_its_marker() {
        // A skill written with `description: |` was catalogued as describing
        // itself as ">-" by a parser that took the marker for the value.
        let source = "---\nname: effect\ndescription: |\n  Opinionated guide for Effect v4.\n  Use when implementing workflows.\nlicense: MIT\n---\n\n# Effect\n\nBody.\n";
        let (name, description, auto) = parse_skill_front_matter(source).expect("parsed");
        assert_eq!(name, "effect");
        assert_eq!(
            description,
            "Opinionated guide for Effect v4.\nUse when implementing workflows."
        );
        assert!(!auto);
        assert_eq!(skill_body(source), "# Effect\n\nBody.");
    }

    #[test]
    fn a_folded_block_description_is_one_paragraph() {
        let source = "---\nname: folded\ndescription: >-\n  First line\n  second line.\n---\nBody.\n";
        let (_, description, _) = parse_skill_front_matter(source).expect("parsed");
        assert_eq!(description, "First line second line.");
    }

    #[test]
    fn a_skill_missing_a_name_or_a_description_is_not_a_skill() {
        // It could not be asked for, or gives the model nothing to choose on.
        assert!(parse_skill_front_matter("---\ndescription: no name here\n---\nBody").is_none());
        assert!(parse_skill_front_matter("---\nname: nameless\n---\nBody").is_none());
        assert!(parse_skill_front_matter("# No front matter at all\n").is_none());
    }

    #[test]
    fn the_nearest_skills_directory_keeps_a_contested_name() {
        let root = tempfile::tempdir().unwrap();
        write_skill(root.path(), "shared", "---\nname: shared\ndescription: The repository's.\n---\nRepo body.\n");
        let shipped = root.path().join("packages").join("openagents-cli").join("skills").join("shared");
        std::fs::create_dir_all(&shipped).unwrap();
        std::fs::write(
            shipped.join("SKILL.md"),
            "---\nname: shared\ndescription: The CLI's.\n---\nShipped body.\n",
        )
        .unwrap();

        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        let skill = registry.skills.get("shared").expect("found");
        assert_eq!(skill.description, "The repository's.");
        assert_eq!(skill.body, "Repo body.");
    }

    #[test]
    fn the_skill_tool_offers_names_and_descriptions_and_never_a_body() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "brewing",
            "---\nname: brewing\ndescription: How to make tea.\n---\nSTEEP_FOR_FOUR_MINUTES\n",
        );
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        let tools = registry.list_tools();
        let skill_tool = tools.iter().find(|t| t.name == "skill").expect("declared");

        assert!(skill_tool.description.contains("`brewing`: How to make tea."));
        // The catalog is what a session pays for on every turn; a body in it
        // is 46 KB of instructions the model may never use.
        assert!(
            !skill_tool.description.contains("STEEP_FOR_FOUR_MINUTES"),
            "the catalog carried a body"
        );
    }

    #[tokio::test]
    async fn reading_a_skill_returns_its_body_and_says_where_it_came_from() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "brewing",
            "---\nname: brewing\ndescription: How to make tea.\n---\nSTEEP_FOR_FOUR_MINUTES\n",
        );
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));

        let out = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "skill".to_string(),
                arguments: serde_json::json!({"name": "brewing"}),
            })
            .await;
        assert!(!out.is_error);
        assert!(out.output.contains("STEEP_FOR_FOUR_MINUTES"), "{}", out.output);
        assert!(out.output.contains("SKILL.md"), "{}", out.output);

        let missing = registry
            .execute_tool(&ToolCall {
                id: "2".to_string(),
                name: "skill".to_string(),
                arguments: serde_json::json!({"name": "smelting"}),
            })
            .await;
        assert!(missing.is_error);
    }

    #[test]
    fn only_an_auto_skill_reaches_the_standing_context() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "method",
            "---\nname: method\ndescription: How to approach the work.\nauto: true\n---\nWORK_THIS_WAY\n",
        );
        write_skill(
            root.path(),
            "asked-for",
            "---\nname: asked-for\ndescription: Read on request.\n---\nONLY_ON_REQUEST\n",
        );
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));

        let context = registry.standing_context().expect("an auto skill is injected");
        assert!(context.contains("WORK_THIS_WAY"), "{context}");
        assert!(
            !context.contains("ONLY_ON_REQUEST"),
            "a skill nobody asked for was injected: {context}"
        );
    }

    #[test]
    fn a_workspace_with_no_auto_skill_injects_nothing() {
        let root = tempfile::tempdir().unwrap();
        write_skill(root.path(), "plain", "---\nname: plain\ndescription: Read on request.\n---\nBody.\n");
        // A temporary directory is not named for either OpenAgents repository,
        // so the workspace note does not apply either.
        assert!(HarnessToolRegistry::new(Some(root.path().to_path_buf()))
            .standing_context()
            .is_none());
    }

    #[test]
    fn the_shipped_repository_skills_are_discovered_and_the_auto_one_is_injected() {
        // End to end against the real `.agents/skills` tree rather than a
        // fixture, because the fixture is what a broken discovery path still
        // passes.
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo.join(".agents").join("skills").is_dir() {
            return;
        }
        let registry = HarnessToolRegistry::new(Some(repo.clone()));
        assert!(
            registry.skills.contains_key("fast-follow"),
            "discovered: {:?}",
            registry.skills.keys().collect::<Vec<_>>()
        );
        // The `effect` skill's description is a `|` block; a parser that took
        // the marker would catalogue it as "|".
        let effect = registry.skills.get("effect").expect("the effect skill");
        assert!(effect.description.len() > 20, "{:?}", effect.description);

        // `superdelegate` ships with `auto: true`, so its body is the standing
        // context this session starts with.
        let context = registry.standing_context().expect("something is injected");
        assert!(
            registry.skills.get("superdelegate").is_some_and(|skill| skill.auto),
            "superdelegate is the repository's auto skill"
        );
        let body = &registry.skills["superdelegate"].body;
        assert!(context.contains(&body[..80.min(body.len())]), "the auto body was not injected");
    }

    // ───────────────────────────────────────────── the capability tool wiring

    #[test]
    fn every_declared_tool_has_an_arm_that_answers_it() {
        // The property the missing `capability` implementation broke: a name
        // in `list_tools` that no arm answers is a promise nothing keeps.
        let root = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::with_delegation(
            Some(root.path().to_path_buf()),
            DelegationGate { lane: "test".to_string(), user_token: None, max_count: 2 },
        );
        let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["shell", "skill", "openagents", "capability", "delegate"]);

        let runtime = tokio::runtime::Runtime::new().unwrap();
        for name in &names {
            if name == "delegate" {
                continue; // Starting real children is not this test's business.
            }
            let out = runtime.block_on(registry.execute_tool(&ToolCall {
                id: "1".to_string(),
                name: name.clone(),
                arguments: serde_json::json!({}),
            }));
            assert!(
                !out.output.starts_with("Unknown tool:"),
                "`{name}` is declared and unanswered"
            );
        }
    }

    #[tokio::test]
    async fn a_capability_search_names_a_plugin_and_loading_it_declares_its_tool() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo.join("plugins").join("word-stats").join("manifest.json").is_file() {
            return;
        }
        let registry = HarnessToolRegistry::new(Some(repo));
        assert!(!registry.catalog.is_empty(), "the checked-in catalog was not discovered");

        let search = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"query": "statistics about the longest word in a piece of text"}),
            })
            .await;
        assert!(search.output.contains("word_stats"), "{}", search.output);
        // Nothing is loaded by searching, so no plugin tool is declared yet.
        assert!(registry.list_tools().iter().all(|t| t.name != "word_stats"));

        let load = registry
            .execute_tool(&ToolCall {
                id: "2".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "word_stats"}),
            })
            .await;
        assert!(load.output.contains("digest verified"), "{}", load.output);
        let word_stats = registry
            .list_tools()
            .into_iter()
            .find(|t| t.name == "word_stats")
            .expect("the loaded plugin declares its tool");
        assert_eq!(word_stats.parameters["properties"]["text"]["type"], "string");

        // And the tool the plugin declared runs the plugin.
        let ran = registry
            .execute_tool(&ToolCall {
                id: "3".to_string(),
                name: "word_stats".to_string(),
                arguments: serde_json::json!({"text": "alpha beta beta"}),
            })
            .await;
        let value: serde_json::Value = serde_json::from_str(&ran.output).expect(&ran.output);
        assert_eq!(value["ok"]["words"], 3);
        assert_eq!(value["ok"]["top_word"]["word"], "beta");
    }

    #[tokio::test]
    async fn a_mounted_capability_refuses_to_load_without_an_operator() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo.join("plugins").join("file-stats").join("manifest.json").is_file() {
            return;
        }
        let unattended = HarnessToolRegistry::new(Some(repo.clone()));
        let refused = unattended
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "file_stats"}),
            })
            .await;
        assert!(refused.output.contains("approval_unavailable"), "{}", refused.output);
        assert!(unattended.list_tools().iter().all(|t| t.name != "file_stats"));

        let attended = HarnessToolRegistry::new(Some(repo)).allowing_plugin_mounts();
        let loaded = attended
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "file_stats"}),
            })
            .await;
        assert!(loaded.output.contains("digest verified"), "{}", loaded.output);
        assert!(attended.list_tools().iter().any(|t| t.name == "file_stats"));
    }

    #[tokio::test]
    async fn a_capability_that_is_not_installed_is_said_to_be_missing() {
        let root = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        let out = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "does_not_exist"}),
            })
            .await;
        assert!(out.is_error);
        assert!(out.output.contains("does_not_exist"), "{}", out.output);
    }
}
