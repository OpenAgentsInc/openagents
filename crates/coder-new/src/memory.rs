//! Project instructions and memory that lasts across sessions (#11176).
//!
//! **Instructions.** Every turn reads the repository's `AGENTS.md` and
//! `CLAUDE.md` files from the working directory up to (not including) the
//! home directory, nearest first, then the user's own
//! `~/.openagents/AGENTS.md`. Instruction files deeper in the checkout are
//! listed by path so the model reads the one for a directory before
//! changing files there.
//!
//! **Memory.** Small typed notes (`user`, `feedback`, `project`,
//! `reference`) kept as markdown files with a `MEMORY.md` index, in two
//! scopes: the user's (everywhere) and the project's (one checkout and all
//! of its worktrees). They live under `~/.openagents/memory` (or
//! `$OPENAGENTS_MEMORY_DIR`); `OPENAGENTS_MEMORY=off` turns memory and
//! instruction loading off. The model saves, updates and deletes them with
//! the `remember`, `forget` and `recall` tools, and `/memory` lists them.
//!
//! **Sharing later.** Every note has a stable id and an update time, and a
//! delete leaves a tombstone, so [`Memory::sync_records`] is the whole
//! state a sync needs: the web chat and the apps can read the same notes
//! through the account once it is on. Nothing leaves this computer unless
//! the person's sync choice says so; notes are private by default. Notes
//! that look like credentials are refused at save time.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Instruction file names read in each directory, in this order.
pub const INSTRUCTION_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
/// The index file in each memory scope.
pub const INDEX: &str = "MEMORY.md";
/// The most of one instruction file loaded into a turn.
const FILE_LIMIT: usize = 64 * 1024;
/// The most of one note's body.
const BODY_LIMIT: usize = 8 * 1024;
/// The most notes in one scope.
const SCOPE_LIMIT: usize = 200;
/// The most nested instruction paths listed.
const NESTED_LIMIT: usize = 40;
/// The default budget for the whole context on a provider turn.
pub const PROVIDER_BUDGET: usize = 160 * 1024;
/// Of that, the most spent on memory notes (index lines always fit first).
const MEMORY_BUDGET: usize = 24 * 1024;
const TOMBSTONES: &str = ".forgotten.json";

/// What kind of thing a note records.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Who the person is and how they like to work.
    User,
    /// A correction or preference about how the agent works.
    Feedback,
    /// A fact about this project that the code doesn't say.
    Project,
    /// Where something lives outside the code: a doc, a dashboard, a ticket.
    Reference,
}

impl Kind {
    pub const fn word(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "user" => Some(Self::User),
            "feedback" => Some(Self::Feedback),
            "project" => Some(Self::Project),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }

    /// Where a note of this kind goes when the caller doesn't say.
    const fn default_scope(self) -> Scope {
        match self {
            Self::User | Self::Feedback => Scope::User,
            Self::Project | Self::Reference => Scope::Project,
        }
    }
}

/// Whose a note is.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// The person's, in every project.
    User,
    /// This project's, in every checkout and worktree of it.
    Project,
}

impl Scope {
    pub const fn word(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "user" => Some(Self::User),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

/// One saved note.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub id: String,
    pub scope: Scope,
    pub kind: Kind,
    pub name: String,
    pub description: String,
    pub body: String,
    /// Unix seconds of the last save.
    pub updated: u64,
    pub file: PathBuf,
}

/// One instruction file read for a turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionFile {
    pub path: PathBuf,
    pub text: String,
    /// Whether `text` was cut at [`FILE_LIMIT`].
    pub truncated: bool,
}

/// What a sync sends or receives: a note, or the fact that one was deleted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncRecord {
    pub id: String,
    pub scope: Scope,
    /// The project a project note belongs to (its key), `None` for the user's.
    pub project: Option<String>,
    pub kind: Option<Kind>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub body: Option<String>,
    pub updated: u64,
    pub deleted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Tombstone {
    id: String,
    name: String,
    at: u64,
}

/// What a `remember` call saves.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub body: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForgetArguments {
    name: String,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecallArguments {
    #[serde(default)]
    name: Option<String>,
}

/// Instructions and memory for one working directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Memory {
    root: PathBuf,
    user_instructions: Option<PathBuf>,
    home: Option<PathBuf>,
    cwd: PathBuf,
    /// The nearest directory with `.git`: this checkout or worktree.
    checkout: PathBuf,
    project_root: PathBuf,
    project_key: String,
}

impl Memory {
    /// The memory for `cwd` on this computer, or `None` when it is turned
    /// off (`OPENAGENTS_MEMORY=off`), there is no home directory, or this
    /// is a unit test (which never reads this computer's home).
    pub fn discover(cwd: &Path) -> Option<Self> {
        if cfg!(test) {
            return None;
        }
        let env = |name: &str| std::env::var(name).ok();
        if env("OPENAGENTS_MEMORY")
            .is_some_and(|value| matches!(value.trim(), "off" | "0" | "false" | "no"))
        {
            return None;
        }
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let openagents = home.join(".openagents");
        let root = env("OPENAGENTS_MEMORY_DIR")
            .filter(|dir| !dir.trim().is_empty())
            .map_or_else(|| openagents.join("memory"), PathBuf::from);
        Some(Self::at(
            root,
            Some(openagents.join("AGENTS.md")),
            Some(home),
            cwd,
        ))
    }

    /// Memory kept under `root` for `cwd`, reading the user's instructions
    /// from `user_instructions` and stopping the upward walk at `home`.
    pub fn at(
        root: PathBuf,
        user_instructions: Option<PathBuf>,
        home: Option<PathBuf>,
        cwd: &Path,
    ) -> Self {
        let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let checkout = cwd
            .ancestors()
            .find(|dir| dir.join(".git").exists())
            .unwrap_or(&cwd)
            .to_path_buf();
        let project_root = project_root(&cwd);
        let project_key = project_key(&project_root);
        Self {
            root,
            user_instructions,
            home: home.map(|home| home.canonicalize().unwrap_or(home)),
            cwd,
            checkout,
            project_root,
            project_key,
        }
    }

    /// The checkout the project notes belong to (the main checkout for a
    /// worktree).
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// The directory holding one scope's notes.
    pub fn scope_dir(&self, scope: Scope) -> PathBuf {
        match scope {
            Scope::User => self.root.join("user"),
            Scope::Project => self.root.join("projects").join(&self.project_key),
        }
    }

    /// The instruction files for this directory, nearest first, then the
    /// user's own file. A file read twice (a `CLAUDE.md` link to
    /// `AGENTS.md`, or the same text) appears once.
    pub fn instructions(&self) -> Vec<InstructionFile> {
        let mut files = Vec::new();
        let mut seen_paths = BTreeSet::new();
        let mut seen_text = BTreeSet::new();
        let mut add = |path: PathBuf, files: &mut Vec<InstructionFile>| {
            let Ok(real) = path.canonicalize() else {
                return;
            };
            if !real.is_file() || !seen_paths.insert(real) {
                return;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                return;
            };
            if text.trim().is_empty() || !seen_text.insert(text.trim().to_string()) {
                return;
            }
            let (text, truncated) = cut(&text, FILE_LIMIT);
            files.push(InstructionFile {
                path,
                text,
                truncated,
            });
        };
        for dir in self.cwd.ancestors() {
            if self.home.as_deref() == Some(dir) {
                break;
            }
            // A worktree kept inside its main checkout reads its own
            // files, not the main checkout's copies above it.
            if self.checkout != self.project_root
                && dir != self.checkout
                && self.checkout.starts_with(dir)
                && dir.starts_with(&self.project_root)
            {
                continue;
            }
            for name in INSTRUCTION_FILES {
                add(dir.join(name), &mut files);
            }
        }
        if let Some(path) = &self.user_instructions {
            add(path.clone(), &mut files);
        }
        files
    }

    /// Instruction files elsewhere in this checkout (tracked files only),
    /// relative to it, leaving out the loaded ones and those in the working
    /// directory's ancestors.
    pub fn nested_instructions(&self, loaded: &[InstructionFile]) -> Vec<PathBuf> {
        let Ok(output) = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.checkout)
            .args(["ls-files", "-z", "--", "*AGENTS.md", "*CLAUDE.md"])
            .stderr(std::process::Stdio::null())
            .output()
        else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let loaded: BTreeSet<PathBuf> = loaded
            .iter()
            .filter_map(|file| file.path.canonicalize().ok())
            .collect();
        String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|path| {
                Path::new(path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| INSTRUCTION_FILES.contains(&name))
            })
            .map(PathBuf::from)
            .filter(|path| {
                let full = self.checkout.join(path);
                full.parent().is_none_or(|dir| !self.cwd.starts_with(dir))
                    && full
                        .canonicalize()
                        .map_or(true, |real| !loaded.contains(&real))
            })
            .take(NESTED_LIMIT)
            .collect()
    }

    /// Every note in `scope`, newest first.
    pub fn entries(&self, scope: Scope) -> Vec<Entry> {
        let dir = self.scope_dir(scope);
        let Ok(read) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut entries: Vec<Entry> = read
            .filter_map(Result::ok)
            .map(|item| item.path())
            .filter(|path| {
                path.extension().is_some_and(|ext| ext == "md")
                    && path.file_name().is_some_and(|name| name != INDEX)
            })
            .filter_map(|path| parse_entry(&path, scope))
            .collect();
        entries.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.name.cmp(&b.name)));
        entries
    }

    /// The user's notes, then the project's.
    pub fn all(&self) -> Vec<Entry> {
        let mut all = self.entries(Scope::User);
        all.extend(self.entries(Scope::Project));
        all
    }

    /// Save a note, or update the one with the same name in its scope.
    pub fn remember(&self, note: &Note) -> Result<Entry, String> {
        let kind = Kind::parse(&note.kind)
            .ok_or("A memory's type is user, feedback, project, or reference.")?;
        let scope = match note.scope.as_deref() {
            None => kind.default_scope(),
            Some(text) => Scope::parse(text).ok_or("A memory's scope is user or project.")?,
        };
        let name = one_line(&note.name);
        if name.is_empty() {
            return Err("Give the memory a short name.".into());
        }
        if name.chars().count() > 80 {
            return Err("Keep the memory's name under 80 characters.".into());
        }
        let body = note.body.trim().to_string();
        if body.is_empty() {
            return Err("Write what to remember in body.".into());
        }
        if body.len() > BODY_LIMIT {
            return Err("Keep one memory under 8 KiB; split it into smaller notes.".into());
        }
        let description = note
            .description
            .as_deref()
            .map(one_line)
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| {
                let first = one_line(body.lines().next().unwrap_or_default());
                cut(&first, 150).0
            });
        for text in [&name, &description, &body] {
            if let Some(rule) = secret_screen::credential_in(text) {
                return Err(format!(
                    "Memory never holds credentials, and this looks like one ({rule}). Save where it is kept instead."
                ));
            }
        }
        let dir = self.scope_dir(scope);
        let existing = self.entries(scope);
        let previous = existing
            .iter()
            .find(|entry| slug(&entry.name) == slug(&name));
        if previous.is_none() && existing.len() >= SCOPE_LIMIT {
            return Err(format!(
                "The {} memory already holds {SCOPE_LIMIT} notes; forget or merge some first.",
                scope.word()
            ));
        }
        fs::create_dir_all(&dir).map_err(|_| "Cannot create the memory folder.")?;
        let entry = Entry {
            id: previous.map_or_else(new_id, |entry| entry.id.clone()),
            scope,
            kind,
            name: name.clone(),
            description,
            body,
            updated: now(),
            file: previous.map_or_else(
                || dir.join(format!("{}.md", slug(&name))),
                |entry| entry.file.clone(),
            ),
        };
        write_atomic(&entry.file, &render_entry(&entry))?;
        self.write_index(scope)?;
        Ok(entry)
    }

    /// Delete the note called `name` (in `scope`, or whichever scope has
    /// it), leaving a tombstone so a sync deletes it everywhere.
    pub fn forget(&self, name: &str, scope: Option<Scope>) -> Result<Entry, String> {
        let wanted = slug(name);
        let scopes: &[Scope] = match scope {
            Some(Scope::User) => &[Scope::User],
            Some(Scope::Project) => &[Scope::Project],
            None => &[Scope::Project, Scope::User],
        };
        let entry = scopes
            .iter()
            .flat_map(|scope| self.entries(*scope))
            .find(|entry| slug(&entry.name) == wanted)
            .ok_or_else(|| format!("No memory is called {}.", one_line(name)))?;
        fs::remove_file(&entry.file).map_err(|_| "Cannot delete that memory.")?;
        let path = self.scope_dir(entry.scope).join(TOMBSTONES);
        let mut stones = read_tombstones(&path);
        stones.retain(|stone| stone.id != entry.id);
        stones.push(Tombstone {
            id: entry.id.clone(),
            name: entry.name.clone(),
            at: now(),
        });
        write_atomic(
            &path,
            &serde_json::to_string_pretty(&stones).unwrap_or_else(|_| "[]".into()),
        )?;
        self.write_index(entry.scope)?;
        Ok(entry)
    }

    /// The note called `name`, in either scope.
    pub fn recall(&self, name: &str) -> Option<Entry> {
        let wanted = slug(name);
        self.all()
            .into_iter()
            .find(|entry| slug(&entry.name) == wanted)
    }

    /// Notes and deletions in the shape a sync sends. Calling this sends
    /// nothing: the caller sends them only when the person's sync choice
    /// allows it.
    pub fn sync_records(&self) -> Vec<SyncRecord> {
        let mut records = Vec::new();
        for scope in [Scope::User, Scope::Project] {
            let project = (scope == Scope::Project).then(|| self.project_key.clone());
            for entry in self.entries(scope) {
                records.push(SyncRecord {
                    id: entry.id,
                    scope,
                    project: project.clone(),
                    kind: Some(entry.kind),
                    name: Some(entry.name),
                    description: Some(entry.description),
                    body: Some(entry.body),
                    updated: entry.updated,
                    deleted: false,
                });
            }
            for stone in read_tombstones(&self.scope_dir(scope).join(TOMBSTONES)) {
                records.push(SyncRecord {
                    id: stone.id,
                    scope,
                    project: project.clone(),
                    kind: None,
                    name: None,
                    description: None,
                    body: None,
                    updated: stone.at,
                    deleted: true,
                });
            }
        }
        records
    }

    fn write_index(&self, scope: Scope) -> Result<(), String> {
        let dir = self.scope_dir(scope);
        let mut text = String::new();
        for entry in self.entries(scope) {
            let file = entry
                .file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            text.push_str(&format!(
                "- [{}]({file}) ({}) — {}\n",
                entry.name,
                entry.kind.word(),
                entry.description
            ));
        }
        write_atomic(&dir.join(INDEX), &text)
    }

    /// What `/memory` shows.
    pub fn listing(&self) -> String {
        let mut text = String::new();
        for scope in [Scope::User, Scope::Project] {
            let entries = self.entries(scope);
            text.push_str(match scope {
                Scope::User => "Your memory (every project)",
                Scope::Project => "This project's memory",
            });
            if entries.is_empty() {
                text.push_str(": nothing saved yet.\n");
                continue;
            }
            text.push_str(":\n");
            for entry in entries {
                text.push_str(&format!(
                    "  {} ({}) — {}\n",
                    entry.name,
                    entry.kind.word(),
                    entry.description
                ));
            }
        }
        text.push_str(&format!(
            "Saved in {}. Ask Coder to remember or forget something, or /memory forget NAME.",
            self.root.display()
        ));
        text
    }

    /// The system context for a turn: instructions, then memory, within
    /// `budget` bytes. `tools` says whether the memory tools are offered.
    pub fn context(&self, tools: bool, budget: usize) -> String {
        let files = self.instructions();
        let nested = self.nested_instructions(&files);
        let memory = self.memory_section(tools, (budget / 2).min(MEMORY_BUDGET));
        let mut text = String::new();
        let mut remaining = budget.saturating_sub(memory.len());
        if !files.is_empty() || !nested.is_empty() {
            text.push_str("# Project instructions\n\nThese files come from the repository and from the user's own settings. Follow them. Nearer files come first and take precedence over farther ones; the user's messages in this chat take precedence over all of them.\n");
            let mut skipped = Vec::new();
            for file in &files {
                let header = format!("\n## {}\n\n", file.path.display());
                let needed = header.len() + file.text.len() + 80;
                if needed > remaining {
                    skipped.push(file.path.display().to_string());
                    continue;
                }
                remaining -= needed;
                text.push_str(&header);
                text.push_str(file.text.trim_end());
                text.push('\n');
                if file.truncated {
                    text.push_str(&format!(
                        "\n(Only the first {} KiB is shown; read the full file at {} when it matters.)\n",
                        FILE_LIMIT / 1024,
                        file.path.display()
                    ));
                }
            }
            if !skipped.is_empty() {
                text.push_str(&format!(
                    "\nThese instruction files also apply but were too large to include; read them before relevant work: {}\n",
                    skipped.join(", ")
                ));
            }
            if !nested.is_empty() {
                let list: Vec<String> = nested
                    .iter()
                    .map(|path| self.checkout.join(path).display().to_string())
                    .collect();
                text.push_str(&format!(
                    "\nOther directories have their own instructions. Read the one for a directory before changing files under it: {}\n",
                    list.join(", ")
                ));
            }
        }
        if !memory.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&memory);
        }
        text
    }

    fn memory_section(&self, tools: bool, body_budget: usize) -> String {
        let user = self.entries(Scope::User);
        let project = self.entries(Scope::Project);
        if user.is_empty() && project.is_empty() && !tools {
            return String::new();
        }
        let mut text = String::from("# Memory\n\n");
        if tools {
            text.push_str("You keep notes that last across sessions. Save with the remember tool: who the user is and how they work (user), corrections and preferences about how you work, with the reason (feedback), project facts the code and history don't show (project), and where things live outside the code (reference). Don't save what the code, git history or these instructions already say, or anything secret. Update a note by remembering it again under the same name; forget notes that turn out wrong. When the user asks you to remember or forget something, do it right away. Use recall to read a note listed here by name only.\n");
        } else {
            text.push_str("Notes saved in earlier sessions. Treat them as the user's standing context; they may be out of date.\n");
        }
        let mut spent = 0usize;
        for (scope, entries) in [(Scope::User, user), (Scope::Project, project)] {
            if entries.is_empty() {
                continue;
            }
            text.push_str(match scope {
                Scope::User => "\n## The user's memory (every project)\n",
                Scope::Project => "\n## This project's memory\n",
            });
            for entry in entries {
                text.push_str(&format!(
                    "- {} ({}): {}\n",
                    entry.name,
                    entry.kind.word(),
                    entry.description
                ));
                if spent + entry.body.len() <= body_budget {
                    spent += entry.body.len();
                    for line in entry.body.lines() {
                        text.push_str("  ");
                        text.push_str(line);
                        text.push('\n');
                    }
                }
            }
        }
        text
    }

    /// The memory tools offered to the model.
    pub fn tool_definitions() -> Vec<Value> {
        vec![
            json!({"type":"function","function":{
                "name":"remember",
                "description":"Save a note that lasts across sessions, or update the note with the same name. Use for the user's preferences and role (user), corrections about how to work, with why (feedback), project facts the code doesn't show (project), and where things live outside the code (reference). Never save secrets.",
                "parameters":{"type":"object","additionalProperties":false,"required":["name","type","body"],"properties":{
                    "name":{"type":"string","description":"A short, unique title, such as \"Prefers small commits\"."},
                    "type":{"type":"string","enum":["user","feedback","project","reference"]},
                    "body":{"type":"string","description":"The note itself, in a few lines. For feedback, say why."},
                    "description":{"type":"string","description":"One line shown in the memory index. Defaults to the body's first line."},
                    "scope":{"type":"string","enum":["user","project"],"description":"user applies everywhere; project applies to this checkout. Defaults: user and feedback go to user, project and reference to project."}
                }}
            }}),
            json!({"type":"function","function":{
                "name":"forget",
                "description":"Delete a saved note by name. The delete reaches every device the memory syncs to.",
                "parameters":{"type":"object","additionalProperties":false,"required":["name"],"properties":{
                    "name":{"type":"string"},
                    "scope":{"type":"string","enum":["user","project"]}
                }}
            }}),
            json!({"type":"function","function":{
                "name":"recall",
                "description":"Read one saved note in full by name, or list every note when no name is given.",
                "parameters":{"type":"object","additionalProperties":false,"properties":{
                    "name":{"type":"string"}
                }}
            }}),
        ]
    }

    /// Whether `name` is one of the memory tools.
    pub fn is_tool(name: &str) -> bool {
        matches!(name, "remember" | "forget" | "recall")
    }

    /// Run one memory tool call.
    pub fn execute(&self, name: &str, arguments: Value) -> Result<Value, String> {
        match name {
            "remember" => {
                let note: Note = serde_json::from_value(arguments).map_err(
                    |_| "remember takes name, type and body, with optional description and scope.",
                )?;
                let entry = self.remember(&note)?;
                Ok(
                    json!({"saved": entry.name, "type": entry.kind.word(), "scope": entry.scope.word()}),
                )
            }
            "forget" => {
                let args: ForgetArguments = serde_json::from_value(arguments)
                    .map_err(|_| "forget takes a name and an optional scope.")?;
                let scope = match args.scope.as_deref() {
                    None => None,
                    Some(text) => {
                        Some(Scope::parse(text).ok_or("A memory's scope is user or project.")?)
                    }
                };
                let entry = self.forget(&args.name, scope)?;
                Ok(json!({"forgot": entry.name, "scope": entry.scope.word()}))
            }
            "recall" => {
                let args: RecallArguments = serde_json::from_value(arguments)
                    .map_err(|_| "recall takes an optional name.")?;
                match args.name {
                    Some(name) => {
                        let entry = self
                            .recall(&name)
                            .ok_or_else(|| format!("No memory is called {}.", one_line(&name)))?;
                        Ok(json!({
                            "name": entry.name,
                            "type": entry.kind.word(),
                            "scope": entry.scope.word(),
                            "description": entry.description,
                            "body": entry.body,
                        }))
                    }
                    None => Ok(json!({"memories": self.all().iter().map(|entry| json!({
                        "name": entry.name,
                        "type": entry.kind.word(),
                        "scope": entry.scope.word(),
                        "description": entry.description,
                    })).collect::<Vec<_>>()})),
                }
            }
            _ => Err("Unknown memory tool.".into()),
        }
    }
}

/// The checkout that owns `cwd`: the nearest directory with `.git`; for a
/// worktree, the main checkout. Without Git, `cwd` itself.
fn project_root(cwd: &Path) -> PathBuf {
    for dir in cwd.ancestors() {
        let git = dir.join(".git");
        if git.is_dir() {
            return dir.to_path_buf();
        }
        if git.is_file() {
            // `gitdir: /main/.git/worktrees/NAME`
            let main = fs::read_to_string(&git).ok().and_then(|text| {
                let gitdir = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
                let gitdir = if gitdir.is_absolute() {
                    gitdir
                } else {
                    dir.join(gitdir)
                };
                let dot_git = gitdir.parent()?.parent()?;
                (dot_git.file_name()? == ".git").then(|| dot_git.parent().map(Path::to_path_buf))?
            });
            return main
                .map(|path| path.canonicalize().unwrap_or(path))
                .unwrap_or_else(|| dir.to_path_buf());
        }
    }
    cwd.to_path_buf()
}

/// A folder name for a project: its path with separators as dashes.
fn project_key(root: &Path) -> String {
    let text: String = root
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if text.len() > 120 {
        text[text.len() - 120..].to_string()
    } else {
        text
    }
}

fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug: String = slug.chars().take(60).collect();
    if slug.is_empty() { "note".into() } else { slug }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cut(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_string(), false);
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn new_id() -> String {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        bytes = now().to_le_bytes();
    }
    format!(
        "mem-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

fn render_entry(entry: &Entry) -> String {
    format!(
        "---\nname: {}\ndescription: {}\ntype: {}\nid: {}\nupdated: {}\n---\n\n{}\n",
        entry.name,
        entry.description,
        entry.kind.word(),
        entry.id,
        entry.updated,
        entry.body
    )
}

fn parse_entry(path: &Path, scope: Scope) -> Option<Entry> {
    let text = fs::read_to_string(path).ok()?;
    let rest = text.strip_prefix("---\n")?;
    let (head, body) = rest.split_once("\n---\n")?;
    let mut name = None;
    let mut description = String::new();
    let mut kind = None;
    let mut id = None;
    let mut updated = 0;
    for line in head.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "name" => name = Some(value.to_string()),
            "description" => description = value.to_string(),
            "type" => kind = Kind::parse(value),
            "id" => id = Some(value.to_string()),
            "updated" => updated = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    let name = name.filter(|name| !name.is_empty())?;
    Some(Entry {
        // A note written by hand without an id gets a stable one from its name.
        id: id.unwrap_or_else(|| format!("mem-{}", slug(&name))),
        scope,
        kind: kind.unwrap_or(Kind::Project),
        name,
        description,
        body: body.trim().to_string(),
        updated,
        file: path.to_path_buf(),
    })
}

fn read_tombstones(path: &Path) -> Vec<Tombstone> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text)
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|_| {
            let _ = fs::remove_file(&temporary);
            format!("Cannot save {}.", path.display())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(name: &str, kind: &str, body: &str) -> Note {
        Note {
            name: name.into(),
            kind: kind.into(),
            body: body.into(),
            description: None,
            scope: None,
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        home: PathBuf,
        repo: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap().join("home");
        let repo = home.join("work").join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("crates/tool")).unwrap();
        fs::create_dir_all(home.join(".openagents")).unwrap();
        Fixture {
            _dir: dir,
            home,
            repo,
        }
    }

    fn memory(fixture: &Fixture, cwd: &Path) -> Memory {
        Memory::at(
            fixture.home.join(".openagents/memory"),
            Some(fixture.home.join(".openagents/AGENTS.md")),
            Some(fixture.home.clone()),
            cwd,
        )
    }

    #[test]
    fn instructions_load_nearest_first_up_to_home_then_the_users_file() {
        let f = fixture();
        fs::write(f.home.join("AGENTS.md"), "home file, never read").unwrap();
        fs::write(f.home.join("work/CLAUDE.md"), "workspace rule").unwrap();
        fs::write(f.repo.join("AGENTS.md"), "repo rule").unwrap();
        fs::write(f.repo.join("CLAUDE.md"), "repo rule").unwrap(); // same text: once
        fs::write(f.repo.join("crates/tool/AGENTS.md"), "tool rule").unwrap();
        fs::write(f.home.join(".openagents/AGENTS.md"), "user rule").unwrap();
        let files = memory(&f, &f.repo.join("crates/tool")).instructions();
        let texts: Vec<&str> = files.iter().map(|file| file.text.as_str()).collect();
        assert_eq!(
            texts,
            ["tool rule", "repo rule", "workspace rule", "user rule"]
        );
    }

    #[test]
    fn context_carries_instructions_and_memory_within_budget() {
        let f = fixture();
        fs::write(
            f.repo.join("AGENTS.md"),
            format!("Always answer in haiku.\n{}", "x".repeat(2000)),
        )
        .unwrap();
        let memory = memory(&f, &f.repo);
        memory
            .remember(&note(
                "Prefers tabs",
                "feedback",
                "Indent with tabs.\nWhy: the owner said so.",
            ))
            .unwrap();
        let context = memory.context(true, PROVIDER_BUDGET);
        assert!(context.contains("Always answer in haiku."));
        assert!(context.contains("Prefers tabs (feedback)"));
        assert!(context.contains("  Why: the owner said so."));
        assert!(context.contains("remember tool"));
        let tiny = memory.context(false, 600);
        assert!(
            !tiny.contains("haiku"),
            "over budget files are listed, not read"
        );
        assert!(tiny.contains("too large to include"));
        assert!(tiny.contains("Prefers tabs"));
    }

    #[test]
    fn remember_updates_by_name_forget_leaves_a_tombstone() {
        let f = fixture();
        let memory = memory(&f, &f.repo);
        let first = memory
            .remember(&note("Deploy target", "reference", "Cloud Run"))
            .unwrap();
        assert_eq!(first.scope, Scope::Project);
        let second = memory
            .remember(&note("deploy  target", "reference", "Workers"))
            .unwrap();
        assert_eq!(first.id, second.id);
        let entries = memory.entries(Scope::Project);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].body, "Workers");
        let index = fs::read_to_string(memory.scope_dir(Scope::Project).join(INDEX)).unwrap();
        assert!(index.contains("[deploy target](deploy-target.md) (reference) — Workers"));
        memory.forget("Deploy target", None).unwrap();
        assert!(memory.entries(Scope::Project).is_empty());
        let records = memory.sync_records();
        assert_eq!(records.len(), 1);
        assert!(records[0].deleted);
        assert_eq!(records[0].id, first.id);
        assert!(memory.forget("Deploy target", None).is_err());
    }

    #[test]
    fn user_notes_follow_the_user_and_project_notes_follow_worktrees() {
        let f = fixture();
        let main = memory(&f, &f.repo);
        main.remember(&note("Name", "user", "Chris")).unwrap();
        main.remember(&note("Board", "project", "Project 22"))
            .unwrap();
        // A worktree of the same repository.
        let worktree = f.home.join("worktrees/repo-wt");
        fs::create_dir_all(f.repo.join(".git/worktrees/wt")).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", f.repo.join(".git/worktrees/wt").display()),
        )
        .unwrap();
        let other = memory(&f, &worktree);
        assert_eq!(other.project_root(), f.repo.as_path());
        assert_eq!(other.all().len(), 2);
        // Another project sees only the user's notes.
        let elsewhere = f.home.join("work/other");
        fs::create_dir_all(elsewhere.join(".git")).unwrap();
        let names: Vec<String> = memory(&f, &elsewhere)
            .all()
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, ["Name"]);
    }

    #[test]
    fn credentials_and_bad_arguments_are_refused() {
        let f = fixture();
        let memory = memory(&f, &f.repo);
        let error = memory
            .remember(&note(
                "Key",
                "reference",
                "OPENAI key sk-proj-abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH",
            ))
            .unwrap_err();
        assert!(error.contains("credentials"), "{error}");
        assert!(memory.remember(&note("X", "mood", "y")).is_err());
        assert!(
            memory
                .execute(
                    "remember",
                    json!({"name":"X","type":"user","body":"y","extra":1})
                )
                .is_err()
        );
        let saved = memory
            .execute(
                "remember",
                json!({"name":"Likes short answers","type":"user","body":"Keep replies short."}),
            )
            .unwrap();
        assert_eq!(saved["scope"], "user");
        let recalled = memory
            .execute("recall", json!({"name":"likes short answers"}))
            .unwrap();
        assert_eq!(recalled["body"], "Keep replies short.");
        let listed = memory.execute("recall", json!({})).unwrap();
        assert_eq!(listed["memories"].as_array().unwrap().len(), 1);
        assert!(memory.listing().contains("Likes short answers (user)"));
    }

    #[test]
    fn a_worktree_inside_its_main_checkout_reads_its_own_instructions() {
        let f = fixture();
        fs::write(f.repo.join("AGENTS.md"), "main copy").unwrap();
        let worktree = f.repo.join(".claude/worktrees/wt");
        fs::create_dir_all(f.repo.join(".git/worktrees/wt")).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", f.repo.join(".git/worktrees/wt").display()),
        )
        .unwrap();
        fs::write(worktree.join("AGENTS.md"), "worktree copy").unwrap();
        fs::write(f.home.join("work/AGENTS.md"), "workspace rule").unwrap();
        let memory = memory(&f, &worktree);
        assert_eq!(memory.project_root(), f.repo.as_path());
        let texts: Vec<String> = memory
            .instructions()
            .into_iter()
            .map(|file| file.text)
            .collect();
        assert_eq!(texts, ["worktree copy", "workspace rule"]);
    }

    #[test]
    fn hand_written_notes_are_read() {
        let f = fixture();
        let memory = memory(&f, &f.repo);
        let dir = memory.scope_dir(Scope::User);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("tz.md"),
            "---\nname: Time zone\ntype: user\n---\n\nCentral time.\n",
        )
        .unwrap();
        let entry = memory.recall("Time zone").unwrap();
        assert_eq!(entry.body, "Central time.");
        assert_eq!(entry.id, "mem-time-zone");
    }
}
