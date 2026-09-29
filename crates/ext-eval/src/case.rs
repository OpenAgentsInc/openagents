//! One case: its prompt, its run configuration, and its graders.
//!
//! A case is a directory holding `prompt.md` and optionally `case.toml`,
//! `graders/`, and `fixtures/`. `case.toml` is the base document, the
//! `prompt.md` frontmatter overrides it key by key (the `[run]` table
//! merges key by key too), the `prompt.md` body is the prompt, and the
//! grader list is `case.toml`'s `graders` followed by `graders/*.md` in name
//! order. Every key set is closed, and an unknown key is an error that names
//! the allowed set. `docs/extensions/evaluation.md`, *Case format*, is the
//! specification.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use toml::{Table, Value as Toml};

use crate::grader::{Check, Focus, Grader};

/// The case schema, written as `v`.
pub const CASE_SCHEMA: &str = "openagents.eval-case.v1";
/// The prefix every version of the case schema shares.
const CASE_SCHEMA_PREFIX: &str = "openagents.eval-case.v";
/// The largest case file read: `prompt.md`, `case.toml`, or a grader.
pub const MAX_FILE_BYTES: u64 = 1 << 20;
/// The most graders a case may carry, and the most grader files.
pub const MAX_GRADERS: usize = 64;
/// The fewest and most runs per arm.
pub const RUNS: std::ops::RangeInclusive<u32> = 1..=10;
/// Runs per arm when the case names none.
pub const DEFAULT_RUNS: u32 = 3;
/// The longest wall-clock deadline a case may ask for.
pub const MAX_DEADLINE_SECONDS: u32 = 1800;
/// The deadline when the case names none.
pub const DEFAULT_DEADLINE_SECONDS: u32 = 300;
/// The prefix every `run.env` key must carry.
pub const ENV_PREFIX: &str = "OA_EVAL_";

/// The keys `prompt.md`'s frontmatter may carry.
pub const PROMPT_KEYS: [&str; 8] = [
    "v",
    "name",
    "description",
    "tags",
    "kind",
    "extensions",
    "runs",
    "run",
];
/// The keys `case.toml` may carry: the frontmatter's and `graders`.
pub const CASE_TOML_KEYS: [&str; 9] = [
    "v",
    "name",
    "description",
    "tags",
    "kind",
    "extensions",
    "runs",
    "run",
    "graders",
];
/// The keys of the `[run]` table.
pub const RUN_KEYS: [&str; 6] = [
    "prompt",
    "deadline_seconds",
    "door",
    "allowed_operations",
    "append_instructions",
    "env",
];

/// What went wrong reading a case or a suite.
#[derive(Debug, thiserror::Error)]
pub enum CaseError {
    /// A file or directory could not be read.
    #[error("{file}: {detail}")]
    Io {
        /// The file.
        file: String,
        /// The underlying error.
        detail: String,
    },
    /// A case file is over the size limit.
    #[error("{file} is {size} bytes; a case file is at most {MAX_FILE_BYTES} bytes (1 MiB)")]
    TooLarge {
        /// The file.
        file: String,
        /// Its size.
        size: u64,
    },
    /// Frontmatter or TOML that does not parse.
    #[error("{file}: {detail}")]
    Syntax {
        /// The file.
        file: String,
        /// What the parser said.
        detail: String,
    },
    /// A key outside the closed set.
    #[error("{file}: unknown key `{key}`; the allowed keys are {allowed}")]
    UnknownKey {
        /// The file.
        file: String,
        /// The key, with its table path.
        key: String,
        /// The allowed keys, comma-separated.
        allowed: String,
    },
    /// A case written for a newer schema.
    #[error("{file}: the case is `{found}`, and this build reads {CASE_SCHEMA}; update OpenAgents")]
    NewerVersion {
        /// The file.
        file: String,
        /// The version it declares.
        found: String,
    },
    /// A value that parsed and is not allowed.
    #[error("{file}: {detail}")]
    Invalid {
        /// The file.
        file: String,
        /// What is wrong.
        detail: String,
    },
    /// A case with a `TODO` line left in it.
    #[error("{file}:{line}: the case still has a TODO line; finish it before running it")]
    Todo {
        /// The file.
        file: String,
        /// The one-based line number.
        line: usize,
    },
    /// Two cases with one name.
    #[error("two cases are named `{name}`: {first} and {second}; case names are unique")]
    DuplicateCase {
        /// The name.
        name: String,
        /// The first case directory.
        first: String,
        /// The second.
        second: String,
    },
    /// An eval directory setting that is not plain directory names.
    #[error("the eval directory `{value}` {detail}")]
    EvalDir {
        /// The value given.
        value: String,
        /// What is wrong.
        detail: String,
    },
}

/// Whether the extension ought to be used on the task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The tool should help.
    ShouldFire,
    /// The tool should stay out of the way.
    ShouldNotFire,
}

impl Kind {
    /// The word a case file and a report write.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::ShouldFire => "should-fire",
            Self::ShouldNotFire => "should-not-fire",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

/// An operation class a case asks the run to be granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    /// Read the workspace. The only automatic grant.
    Read,
    /// Write the workspace.
    Write,
    /// Run programs.
    Exec,
    /// Reach the network beyond the pinned door.
    Network,
}

impl Grant {
    /// The word a case file writes.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Exec => "exec",
            Self::Network => "network",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "read" => Self::Read,
            "write" => Self::Write,
            "exec" => Self::Exec,
            "network" => Self::Network,
            _ => return None,
        })
    }
}

/// A run's configuration, from the `[run]` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RunConfig {
    /// Wall-clock cap, 1 to 1800 seconds.
    pub deadline_seconds: u32,
    /// The door the subject turn uses, as a name the runner's table knows.
    pub door: Option<String>,
    /// The operations the case asks for; `read` is always among them.
    pub allowed_operations: BTreeSet<Grant>,
    /// Appended to the child's instructions, in both arms.
    pub append_instructions: Option<String>,
    /// Extra environment, as written. [`Case::check_env`] refuses keys
    /// outside `OA_EVAL_*`.
    pub env: BTreeMap<String, String>,
}

/// The files a case was read from, relative to the eval directory.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CaseFiles {
    /// `prompt.md`'s exact bytes.
    #[serde(skip)]
    pub prompt: Vec<u8>,
    /// `case.toml`'s exact bytes, when present.
    #[serde(skip)]
    pub case_toml: Option<Vec<u8>>,
    /// Each grader file's name and exact bytes, in name order.
    #[serde(skip)]
    pub graders: Vec<(String, Vec<u8>)>,
    /// Each fixture's path under `fixtures/` and exact bytes, in path order.
    #[serde(skip)]
    pub fixtures: Vec<(String, Vec<u8>)>,
}

/// One case, parsed and validated.
#[derive(Clone, Debug, Serialize)]
pub struct Case {
    /// The name `--case` globs match; unique in the suite.
    pub name: String,
    /// The case directory relative to the eval directory, `/`-separated.
    pub path: String,
    /// For readers.
    pub description: Option<String>,
    /// `--tag` keeps a case when any tag matches.
    pub tags: Vec<String>,
    /// Whether the extension ought to be used.
    pub kind: Kind,
    /// The components under test; empty means the package at the root.
    pub extensions: Vec<String>,
    /// Runs per arm.
    pub runs: u32,
    /// The prompt.
    pub prompt: String,
    /// The run configuration.
    pub run: RunConfig,
    /// The graders, `case.toml`'s first and then the files in name order.
    pub graders: Vec<Grader>,
    /// The exact bytes the case was read from.
    #[serde(skip)]
    pub files: CaseFiles,
}

/// A `TODO` line left in a case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoLine {
    /// The file, relative to the case directory.
    pub file: String,
    /// The one-based line.
    pub line: usize,
}

/// What a run of a case failed with before it could be graded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunFailure {
    /// The deadline passed and the child was stopped.
    Timeout,
    /// The run was refused before it started.
    Refused,
    /// The spend ceiling stopped the run.
    CostCeiling,
    /// The door refused the credential.
    AuthFailed,
    /// `run.env` held a key outside `OA_EVAL_*`.
    EnvVarRejected,
    /// This host has no confinement backend.
    UnconfinedHost,
}

impl RunFailure {
    /// The reason word a report writes.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Refused => "refused",
            Self::CostCeiling => "cost_ceiling",
            Self::AuthFailed => "auth_failed",
            Self::EnvVarRejected => "env_var_rejected",
            Self::UnconfinedHost => "unconfined_host",
        }
    }

    /// Whether the run was refused before it ran (NIP-EVAL `refused`) rather
    /// than failing while it ran (`failed`).
    #[must_use]
    pub const fn refused(self) -> bool {
        matches!(
            self,
            Self::Refused | Self::UnconfinedHost | Self::EnvVarRejected
        )
    }
}

/// What a case's text is checked for at load.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadOptions {
    /// Accept `TODO` lines, for an author editing a draft. A run never sets
    /// this: a run refuses a case that still has one.
    pub allow_todo: bool,
}

impl Case {
    /// Reads and validates the case in `dir`.
    ///
    /// `path` is the case directory relative to the eval directory, which a
    /// report shows.
    ///
    /// # Errors
    ///
    /// Returns [`CaseError`] for an unreadable, oversized, malformed, or
    /// invalid case, and for a `TODO` line unless `options` allow it.
    pub fn load(dir: &Path, path: &str, options: LoadOptions) -> Result<Self, CaseError> {
        let prompt = read_limited(&dir.join("prompt.md"), &format!("{path}/prompt.md"))?
            .ok_or_else(|| CaseError::Io {
                file: format!("{path}/prompt.md"),
                detail: "a case directory needs a prompt.md".into(),
            })?;
        let case_toml = read_limited(&dir.join("case.toml"), &format!("{path}/case.toml"))?;
        let graders = read_graders(&dir.join("graders"), path)?;
        let fixtures = read_fixtures(&dir.join("fixtures"), path)?;
        let dir_name = path.rsplit('/').next().unwrap_or(path);
        let files = CaseFiles {
            prompt,
            case_toml,
            graders,
            fixtures,
        };
        let case = Self::parse(dir_name, path, files)?;
        if !options.allow_todo
            && let Some(todo) = case.todo_lines().into_iter().next()
        {
            return Err(CaseError::Todo {
                file: format!("{path}/{}", todo.file),
                line: todo.line,
            });
        }
        Ok(case)
    }

    /// Parses a case from its files' bytes. Pure: reads nothing from disk.
    ///
    /// `dir_name` is the default name; `path` prefixes file names in errors.
    ///
    /// # Errors
    ///
    /// Returns [`CaseError`] for a malformed or invalid case. `TODO` lines
    /// are not checked here; see [`Case::todo_lines`].
    pub fn parse(dir_name: &str, path: &str, mut files: CaseFiles) -> Result<Self, CaseError> {
        files.graders.sort_by(|left, right| left.0.cmp(&right.0));
        let prompt_file = format!("{path}/prompt.md");
        let toml_file = format!("{path}/case.toml");
        for (file, bytes) in std::iter::once((&prompt_file, &files.prompt))
            .chain(files.case_toml.as_ref().map(|bytes| (&toml_file, bytes)))
        {
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(CaseError::TooLarge {
                    file: file.clone(),
                    size: bytes.len() as u64,
                });
            }
        }
        if files.graders.len() > MAX_GRADERS {
            return Err(CaseError::Invalid {
                file: format!("{path}/graders"),
                detail: format!(
                    "holds {} grader files; a case has at most {MAX_GRADERS}",
                    files.graders.len()
                ),
            });
        }
        let prompt_text = utf8(&files.prompt, &prompt_file)?;
        let (front, body) = frontmatter(prompt_text, &prompt_file)?;
        let front = match front {
            Some(text) => parse_table(text, &prompt_file)?,
            None => Table::new(),
        };
        check_keys(&front, &PROMPT_KEYS, &prompt_file, "")?;
        let base = match &files.case_toml {
            Some(bytes) => parse_table(utf8(bytes, &toml_file)?, &toml_file)?,
            None => Table::new(),
        };
        check_keys(&base, &CASE_TOML_KEYS, &toml_file, "")?;
        for (table, file) in [(&front, &prompt_file), (&base, &toml_file)] {
            if let Some(run) = table.get("run") {
                let Toml::Table(run) = run else {
                    return Err(invalid(file, "`run` must be a table"));
                };
                check_keys(run, &RUN_KEYS, file, "run.")?;
            }
        }

        // Which file a merged key came from, for errors.
        let source = |key: &str| -> &str {
            if front.contains_key(key) {
                &prompt_file
            } else {
                &toml_file
            }
        };
        let merged = merge(&base, &front);

        let version = match merged.get("v") {
            Some(Toml::String(version)) => version.clone(),
            Some(_) => return Err(invalid(source("v"), "`v` must be a string")),
            None => {
                return Err(invalid(
                    &prompt_file,
                    &format!("the case declares no `v`; write v = \"{CASE_SCHEMA}\""),
                ));
            }
        };
        if version != CASE_SCHEMA {
            let newer = version
                .strip_prefix(CASE_SCHEMA_PREFIX)
                .and_then(|major| major.parse::<u32>().ok())
                .is_some_and(|major| major > 1);
            return Err(if newer {
                CaseError::NewerVersion {
                    file: source("v").to_string(),
                    found: version,
                }
            } else {
                invalid(
                    source("v"),
                    &format!("`v` is `{version}`; it must be \"{CASE_SCHEMA}\""),
                )
            });
        }

        let name = match merged.get("name") {
            Some(Toml::String(name)) => name.clone(),
            Some(_) => return Err(invalid(source("name"), "`name` must be a string")),
            None => dir_name.to_string(),
        };
        check_name(&name, source("name"), "case name")?;
        let description = optional_string(&merged, "description", source("description"))?;
        let tags = strings(&merged, "tags", source("tags"))?;
        let extensions = strings(&merged, "extensions", source("extensions"))?;
        let kind = match merged.get("kind") {
            None => Kind::ShouldFire,
            Some(Toml::String(word)) if word == "should-fire" => Kind::ShouldFire,
            Some(Toml::String(word)) if word == "should-not-fire" => Kind::ShouldNotFire,
            Some(_) => {
                return Err(invalid(
                    source("kind"),
                    "`kind` must be \"should-fire\" or \"should-not-fire\"",
                ));
            }
        };
        let runs = match merged.get("runs") {
            None => DEFAULT_RUNS,
            Some(Toml::Integer(runs)) if (1..=10).contains(runs) => {
                u32::try_from(*runs).unwrap_or(DEFAULT_RUNS)
            }
            Some(_) => {
                return Err(invalid(
                    source("runs"),
                    &format!(
                        "`runs` must be a whole number from {} to {}",
                        RUNS.start(),
                        RUNS.end()
                    ),
                ));
            }
        };

        let run_table = match merged.get("run") {
            Some(Toml::Table(run)) => run.clone(),
            _ => Table::new(),
        };
        let run_source = |key: &str| -> &str {
            let in_front = front
                .get("run")
                .and_then(Toml::as_table)
                .is_some_and(|run| run.contains_key(key));
            if in_front { &prompt_file } else { &toml_file }
        };
        let body = body.trim();
        let prompt = if body.is_empty() {
            match run_table.get("prompt") {
                Some(Toml::String(prompt)) if !prompt.trim().is_empty() => {
                    prompt.trim().to_string()
                }
                Some(Toml::String(_)) | None => {
                    return Err(invalid(
                        &prompt_file,
                        "the case has no prompt; write the task as prompt.md's body",
                    ));
                }
                Some(_) => {
                    return Err(invalid(
                        run_source("prompt"),
                        "`run.prompt` must be a string",
                    ));
                }
            }
        } else {
            body.to_string()
        };
        let deadline_seconds = match run_table.get("deadline_seconds") {
            None => DEFAULT_DEADLINE_SECONDS,
            Some(Toml::Integer(seconds))
                if (1..=i64::from(MAX_DEADLINE_SECONDS)).contains(seconds) =>
            {
                u32::try_from(*seconds).unwrap_or(DEFAULT_DEADLINE_SECONDS)
            }
            Some(_) => {
                return Err(invalid(
                    run_source("deadline_seconds"),
                    &format!(
                        "`run.deadline_seconds` must be a whole number from 1 to \
                         {MAX_DEADLINE_SECONDS}"
                    ),
                ));
            }
        };
        let door = optional_string(&run_table, "door", run_source("door"))?;
        let append_instructions = optional_string(
            &run_table,
            "append_instructions",
            run_source("append_instructions"),
        )?;
        let mut allowed_operations = BTreeSet::from([Grant::Read]);
        if run_table.contains_key("allowed_operations") {
            allowed_operations.clear();
            for word in strings(
                &run_table,
                "allowed_operations",
                run_source("allowed_operations"),
            )? {
                let grant = Grant::parse(&word).ok_or_else(|| {
                    invalid(
                        run_source("allowed_operations"),
                        &format!(
                            "`run.allowed_operations` names `{word}`; the operations are read, \
                             write, exec, and network"
                        ),
                    )
                })?;
                allowed_operations.insert(grant);
            }
            allowed_operations.insert(Grant::Read);
        }
        let env = match run_table.get("env") {
            None => BTreeMap::new(),
            Some(Toml::Table(env)) => env
                .iter()
                .map(|(key, value)| match value {
                    Toml::String(value) => Ok((key.clone(), value.clone())),
                    _ => Err(invalid(
                        run_source("env"),
                        &format!("`run.env.{key}` must be a string"),
                    )),
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(invalid(run_source("env"), "`run.env` must be a table")),
        };

        let mut graders = Vec::new();
        match base.get("graders") {
            None => {}
            Some(Toml::Array(entries)) => {
                for (index, entry) in entries.iter().enumerate() {
                    let origin = format!("{toml_file}#graders[{index}]");
                    let Toml::Table(table) = entry else {
                        return Err(invalid(&origin, "a grader entry must be a table"));
                    };
                    graders.push(Grader::parse(table, None, None, &origin)?);
                }
            }
            Some(_) => {
                return Err(invalid(&toml_file, "`graders` must be an array of tables"));
            }
        }
        for (file_name, bytes) in &files.graders {
            let origin = format!("{path}/graders/{file_name}");
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(CaseError::TooLarge {
                    file: origin,
                    size: bytes.len() as u64,
                });
            }
            let text = utf8(bytes, &origin)?;
            let (front, body) = frontmatter(text, &origin)?;
            let Some(front) = front else {
                return Err(invalid(
                    &origin,
                    "a grader file starts with TOML frontmatter between +++ lines",
                ));
            };
            let table = parse_table(front, &origin)?;
            let stem = file_name.strip_suffix(".md").unwrap_or(file_name);
            graders.push(Grader::parse(&table, Some(body), Some(stem), &origin)?);
        }
        if graders.is_empty() {
            return Err(invalid(
                &prompt_file,
                "the case has no graders; add graders/<name>.md or a `graders` list in case.toml",
            ));
        }
        if graders.len() > MAX_GRADERS {
            return Err(invalid(
                path,
                &format!(
                    "has {} graders; a case has at most {MAX_GRADERS}",
                    graders.len()
                ),
            ));
        }
        let mut seen = BTreeSet::new();
        for grader in &graders {
            if !seen.insert(grader.name.as_str()) {
                return Err(invalid(
                    &grader.origin,
                    &format!(
                        "a second grader is named `{}`; grader names are unique",
                        grader.name
                    ),
                ));
            }
        }

        Ok(Self {
            name,
            path: path.to_string(),
            description,
            tags,
            kind,
            extensions,
            runs,
            prompt,
            run: RunConfig {
                deadline_seconds,
                door,
                allowed_operations,
                append_instructions,
                env,
            },
            graders,
            files,
        })
    }

    /// Every `TODO` line in the case's files: a line whose text starts with
    /// `TODO` once leading whitespace is dropped.
    #[must_use]
    pub fn todo_lines(&self) -> Vec<TodoLine> {
        let mut found = Vec::new();
        let mut scan = |file: &str, bytes: &[u8]| {
            let text = String::from_utf8_lossy(bytes);
            for (index, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("TODO") {
                    found.push(TodoLine {
                        file: file.to_string(),
                        line: index + 1,
                    });
                }
            }
        };
        scan("prompt.md", &self.files.prompt);
        if let Some(bytes) = &self.files.case_toml {
            scan("case.toml", bytes);
        }
        for (name, bytes) in &self.files.graders {
            scan(&format!("graders/{name}"), bytes);
        }
        found
    }

    /// Refuses a run whose `run.env` holds a key outside `OA_EVAL_*`.
    ///
    /// # Errors
    ///
    /// Returns [`RunFailure::EnvVarRejected`] and the first such key.
    pub fn check_env(&self) -> Result<(), (RunFailure, String)> {
        for key in self.run.env.keys() {
            let valid = key.strip_prefix(ENV_PREFIX).is_some_and(|rest| {
                rest.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            });
            if !valid {
                return Err((RunFailure::EnvVarRejected, key.clone()));
            }
        }
        Ok(())
    }

    /// Whether every grader of the case is subject-only, so the case scores
    /// the subject arm alone and is excluded from the change.
    #[must_use]
    pub fn subject_only(&self, extension_operations: &BTreeSet<String>) -> bool {
        self.graders
            .iter()
            .all(|grader| grader.subject_only(extension_operations))
    }

    /// Reasons the case can't pass with the operations it asked for. The
    /// harness shows them; they don't stop a run.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        let granted = &self.run.allowed_operations;
        let mut warnings = Vec::new();
        for grader in &self.graders {
            let needs_write = match &grader.check {
                Check::FileExists { exists, .. } => *exists,
                Check::Regex { target, .. } => matches!(target, Focus::File(_) | Focus::Files),
                Check::Decision { focus, .. } | Check::Judge { focus, .. } => {
                    matches!(focus, Focus::File(_) | Focus::Files)
                }
                _ => false,
            };
            if needs_write && !granted.contains(&Grant::Write) {
                warnings.push(format!(
                    "{}: grader `{}` reads files the run creates, and the case does not ask for \
                     `write`",
                    self.name, grader.name
                ));
            }
            if let Check::OperationUsed { operation, min, .. } = &grader.check
                && operation == "shell"
                && *min > 0
                && !granted.contains(&Grant::Exec)
            {
                warnings.push(format!(
                    "{}: grader `{}` needs a shell command, and the case does not ask for `exec`",
                    self.name, grader.name
                ));
            }
        }
        warnings
    }
}

/// Splits `+++` TOML frontmatter from a Markdown body. A file that doesn't
/// start with a `+++` line has no frontmatter.
///
/// # Errors
///
/// Returns [`CaseError::Syntax`] for an opening fence with no closing one.
pub fn frontmatter<'a>(text: &'a str, file: &str) -> Result<(Option<&'a str>, &'a str), CaseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Ok((None, text));
    };
    if first.trim_end() != "+++" {
        return Ok((None, text));
    }
    let start = first.len();
    let mut offset = start;
    for line in lines {
        if line.trim_end() == "+++" {
            return Ok((Some(&text[start..offset]), &text[offset + line.len()..]));
        }
        offset += line.len();
    }
    Err(CaseError::Syntax {
        file: file.to_string(),
        detail: "the frontmatter opens with +++ and never closes".into(),
    })
}

/// Refuses keys outside `allowed`, naming the allowed set.
///
/// # Errors
///
/// Returns [`CaseError::UnknownKey`].
pub fn check_keys(
    table: &Table,
    allowed: &[&str],
    file: &str,
    within: &str,
) -> Result<(), CaseError> {
    match table.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(CaseError::UnknownKey {
            file: file.to_string(),
            key: format!("{within}{key}"),
            allowed: allowed.join(", "),
        }),
        None => Ok(()),
    }
}

/// Checks a case or grader name: 1 to 64 of `A-Z a-z 0-9 . _ -`, not
/// starting with `.`. Names become report keys and result paths.
///
/// # Errors
///
/// Returns [`CaseError::Invalid`].
pub fn check_name(name: &str, file: &str, what: &str) -> Result<(), CaseError> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(invalid(
            file,
            &format!(
                "the {what} `{name}` must be 1 to 64 letters, digits, `.`, `_`, or `-`, and \
                 not start with `.`"
            ),
        ))
    }
}

fn invalid(file: &str, detail: &str) -> CaseError {
    CaseError::Invalid {
        file: file.to_string(),
        detail: detail.to_string(),
    }
}

fn utf8<'a>(bytes: &'a [u8], file: &str) -> Result<&'a str, CaseError> {
    std::str::from_utf8(bytes).map_err(|_| CaseError::Syntax {
        file: file.to_string(),
        detail: "the file is not UTF-8 text".into(),
    })
}

fn parse_table(text: &str, file: &str) -> Result<Table, CaseError> {
    toml::from_str::<Table>(text).map_err(|error| CaseError::Syntax {
        file: file.to_string(),
        detail: format!("TOML does not parse: {}", error.message()),
    })
}

/// `front` over `base`, with the `run` tables merged key by key.
fn merge(base: &Table, front: &Table) -> Table {
    let mut merged = base.clone();
    merged.remove("graders");
    for (key, value) in front {
        match (key.as_str(), merged.get_mut(key), value) {
            ("run", Some(Toml::Table(run)), Toml::Table(over)) => {
                for (key, value) in over {
                    run.insert(key.clone(), value.clone());
                }
            }
            _ => {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    merged
}

fn optional_string(table: &Table, key: &str, file: &str) -> Result<Option<String>, CaseError> {
    match table.get(key) {
        None => Ok(None),
        Some(Toml::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(invalid(file, &format!("`{key}` must be a string"))),
    }
}

fn strings(table: &Table, key: &str, file: &str) -> Result<Vec<String>, CaseError> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(Toml::Array(items)) => items
            .iter()
            .map(|item| match item {
                Toml::String(text) if !text.trim().is_empty() => Ok(text.clone()),
                _ => Err(invalid(file, &format!("`{key}` holds non-empty strings"))),
            })
            .collect(),
        Some(_) => Err(invalid(
            file,
            &format!("`{key}` must be an array of strings"),
        )),
    }
}

/// Reads a file of at most [`MAX_FILE_BYTES`], refusing a symbolic link.
/// `None` when it does not exist.
fn read_limited(path: &Path, file: &str) -> Result<Option<Vec<u8>>, CaseError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CaseError::Io {
                file: file.to_string(),
                detail: error.to_string(),
            });
        }
    };
    if !metadata.is_file() {
        return Err(CaseError::Io {
            file: file.to_string(),
            detail: "is not a regular file (a symbolic link or a directory)".into(),
        });
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(CaseError::TooLarge {
            file: file.to_string(),
            size: metadata.len(),
        });
    }
    std::fs::read(path)
        .map(Some)
        .map_err(|error| CaseError::Io {
            file: file.to_string(),
            detail: error.to_string(),
        })
}

fn read_graders(dir: &Path, path: &str) -> Result<Vec<(String, Vec<u8>)>, CaseError> {
    let label = format!("{path}/graders");
    let mut names = Vec::new();
    for name in list_dir(dir, &label)? {
        if name.ends_with(".md") {
            names.push(name);
        }
    }
    if names.len() > MAX_GRADERS {
        return Err(invalid(
            &label,
            &format!(
                "holds {} grader files; a case has at most {MAX_GRADERS}",
                names.len()
            ),
        ));
    }
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let file = format!("{label}/{name}");
            let bytes = read_limited(&dir.join(&name), &file)?.ok_or_else(|| CaseError::Io {
                file,
                detail: "disappeared while the case was read".into(),
            })?;
            Ok((name, bytes))
        })
        .collect()
}

fn read_fixtures(dir: &Path, path: &str) -> Result<Vec<(String, Vec<u8>)>, CaseError> {
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, String)> = vec![(dir.to_path_buf(), String::new())];
    while let Some((at, prefix)) = stack.pop() {
        for name in list_dir(&at, &format!("{path}/fixtures/{prefix}"))? {
            let full = at.join(&name);
            let relative = format!("{prefix}{name}");
            let label = format!("{path}/fixtures/{relative}");
            let metadata = std::fs::symlink_metadata(&full).map_err(|error| CaseError::Io {
                file: label.clone(),
                detail: error.to_string(),
            })?;
            if metadata.is_dir() {
                stack.push((full, format!("{relative}/")));
            } else if metadata.is_file() {
                let bytes = std::fs::read(&full).map_err(|error| CaseError::Io {
                    file: label,
                    detail: error.to_string(),
                })?;
                out.push((relative, bytes));
            } else {
                return Err(CaseError::Io {
                    file: label,
                    detail: "a fixture must be a regular file or a directory, not a link".into(),
                });
            }
        }
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(out)
}

/// The entry names of a directory; empty when it does not exist.
pub(crate) fn list_dir(dir: &Path, label: &str) -> Result<Vec<String>, CaseError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(CaseError::Io {
                file: label.to_string(),
                detail: error.to_string(),
            });
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| CaseError::Io {
            file: label.to_string(),
            detail: error.to_string(),
        })?;
        let name = entry.file_name().into_string().map_err(|_| CaseError::Io {
            file: label.to_string(),
            detail: "holds a file name that is not UTF-8".into(),
        })?;
        names.push(name);
    }
    names.sort();
    Ok(names)
}
