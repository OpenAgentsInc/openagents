//! Entry points: the task's own programs, found by code before any edit.
//!
//! `evidence.baseline` (issue #9633) runs these once on an untouched copy
//! of the workspace. Four kinds, in this order:
//!
//! - **`named`**: every command the instruction states, in a code span or
//!   a shell block, as [`extract::draft`] reads it. This is #9628's
//!   extractor, called as it is: an entry point is a draft item of kind
//!   `command` or `example` from the instruction that the draft could
//!   resolve, whatever its expectation.
//! - **`module`**: `python3 -m <package>` for each top-level package with a
//!   `__main__.py`.
//! - **`make`**: `make test`, or else `make check`, when the workspace's
//!   Makefile has that target.
//! - **`script`**: a Python or shell script the instruction names by path,
//!   when it is a program rather than a library.
//!
//! A module or a script gets the data files the instruction names as its
//! arguments only when their count fits the program's usage line or its
//! `argparse` positionals ([`usage_of`], [`fit`]); otherwise it runs with
//! none. The data files are the files the instruction names, and the
//! files directly inside the directories it names, that aren't code.
//!
//! [`Discovery::Wide`] (issue #9654) adds package entry points below the
//! top level and under `src/`, console scripts, more Makefile targets,
//! `package.json` scripts, and Cargo binaries, and matches a program to
//! the input files the task ships when the instruction names none. See
//! [`wide`].
//!
//! This module only adds to #9628's extractor. It doesn't change
//! [`extract::draft`] or [`extract::plan`], so the frozen contract plans
//! and their measurement stay as they were.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Kind as ItemKind;
use super::extract::{self, Pristine};
use super::host::{Host, Ran, Stat};

pub mod wide;

/// Entry points found for one task, at most.
pub const MAX_ENTRIES: usize = 6;

/// Entry points found for one task under [`Discovery::Wide`], at most.
pub const MAX_ENTRIES_WIDE: usize = 8;

/// Which entry points discovery looks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Discovery {
    /// Issue #9633's four kinds: named commands, top-level packages,
    /// `make test` or `make check`, and named scripts.
    #[default]
    Named,
    /// Those, plus the kinds in [`wide`] (issue #9654).
    Wide,
}

/// The largest program text read for a usage line.
const PROGRAM_MAX: u64 = 256 * 1024;

/// Extensions of files that are code or documentation, never data.
const CODE: &[&str] = &[
    "py", "pyc", "sh", "bash", "js", "mjs", "ts", "rs", "go", "c", "h", "cc", "cpp", "hpp", "java",
    "rb", "pl", "pm", "v", "md", "rst", "toml", "cfg", "ini", "lock", "bas", "frm", "cls",
];

/// Directories a package search never enters.
const SKIPPED: &[&str] = &[
    "node_modules",
    "__pycache__",
    "venv",
    ".venv",
    "site-packages",
    "dist",
    "build",
    "target",
];

/// What kind of entry point a command is. The words are the run card's
/// `kind` field (`docs/gym/run-card.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The instruction names the command.
    Named,
    /// `python3 -m <package>`.
    Module,
    /// `make test` or `make check`.
    Make,
    /// A script the instruction names by path.
    Script,
    /// A console script a `pyproject.toml`, `setup.cfg`, or `setup.py`
    /// declares.
    Console,
    /// A `package.json` script.
    Npm,
    /// A Cargo binary.
    Cargo,
}

impl EntryKind {
    /// The kind as the records spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            EntryKind::Named => "named",
            EntryKind::Module => "module",
            EntryKind::Make => "make",
            EntryKind::Script => "script",
            EntryKind::Console => "console",
            EntryKind::Npm => "npm",
            EntryKind::Cargo => "cargo",
        }
    }
}

/// One entry point: the command a session would type from the working
/// directory, and why code chose it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub kind: EntryKind,
    pub command: String,
    /// Where it came from and how its arguments were chosen.
    pub why: String,
    /// Why it must not run, when it must not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

/// A program's stated positional arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// The required positionals, by the name the program gives them.
    pub required: Vec<String>,
    /// Whether more of the last kind may follow.
    pub variadic: bool,
    /// `argparse` or `usage line`.
    pub source: String,
}

/// A workspace on this host that the instruction calls `alias`, such as a
/// copy under a scratch directory of a task whose instruction says
/// `/app`. Paths under `alias` read from `root`; with `alias` another
/// directory than `root`, every other path reads as missing, so an
/// offline run never reads the operator's own files. Commands don't run.
#[derive(Clone, Debug)]
pub struct Mapped {
    pub root: PathBuf,
    pub alias: String,
}

impl Mapped {
    /// Where `path` is on this host, if anywhere.
    #[must_use]
    pub fn local(&self, path: &str) -> Option<PathBuf> {
        let alias = self.alias.trim_end_matches('/');
        let rest = if path == alias {
            Some("")
        } else {
            path.strip_prefix(&format!("{alias}/"))
        };
        match rest {
            Some(rest) => Some(self.root.join(rest.trim_end_matches('/'))),
            None if Path::new(alias) == self.root => Some(PathBuf::from(path)),
            None => None,
        }
    }
}

impl Host for Mapped {
    fn describe(&self) -> Value {
        json!({ "host": "mapped", "root": self.root, "alias": self.alias })
    }

    async fn stat(&self, path: &str) -> Result<Stat, String> {
        let Some(local) = self.local(path) else {
            return Ok(Stat::Missing);
        };
        match std::fs::metadata(&local) {
            Ok(meta) if meta.is_dir() => Ok(Stat::Dir),
            Ok(meta) => Ok(Stat::File(meta.len())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Stat::Missing),
            Err(error) => Err(format!("{path}: {error}")),
        }
    }

    async fn read(&self, path: &str, max: usize) -> Result<Option<Vec<u8>>, String> {
        let Some(local) = self.local(path) else {
            return Ok(None);
        };
        if !local.is_file() {
            return Ok(None);
        }
        let mut bytes = std::fs::read(&local).map_err(|e| format!("{path}: {e}"))?;
        bytes.truncate(max);
        Ok(Some(bytes))
    }

    async fn run(&self, _command: &str, _wall: Duration) -> Ran {
        Ran {
            failed: Some("a mapped workspace doesn't run commands".to_string()),
            ..Ran::default()
        }
    }
}

/// Why a command needs the network, if it looks like it does.
#[must_use]
pub fn needs_network(command: &str) -> Option<&'static str> {
    const RULES: &[(&str, &str)] = &[
        (
            r"\b(pip3?|uv\s+pip|poetry|conda|mamba|gem|cpanm?|opam)\s+install\b",
            "it installs packages",
        ),
        (
            r"\b(npm|pnpm|bun|yarn)\s+(install|i|ci|add|update)\b",
            "it installs packages",
        ),
        (r"\buv\s+(sync|add|lock)\b", "it installs packages"),
        (r"\bcargo\s+(install|fetch|update)\b", "it fetches crates"),
        (
            r"\bgo\s+(get|install)\b|\bgo\s+mod\s+download\b",
            "it fetches modules",
        ),
        (
            r"\bgit\s+(clone|fetch|pull|push|ls-remote|submodule)\b",
            "it reaches a Git remote",
        ),
        (
            r"(^|[\s;&|(])(curl|wget|ssh|scp|sftp|ftp|telnet|nc|rsync)\s",
            "it reaches another host",
        ),
        (
            r"(^|[\s;&|(])(apt|apt-get|yum|dnf|apk|brew)\s",
            "it installs system packages",
        ),
        (
            r"\bdocker\s+(pull|push|run|build)\b",
            "it reaches a registry",
        ),
        (r"https?://", "it names a URL"),
    ];
    RULES.iter().find_map(|(pattern, why)| {
        Regex::new(pattern)
            .expect("a valid pattern")
            .is_match(command)
            .then_some(*why)
    })
}

/// Whether a command still holds a placeholder word such as `CONFIG_JSON`
/// or `OUTPUT_DIR` that the draft couldn't read as one.
fn has_placeholder(command: &str) -> bool {
    let word = Regex::new(r"^[A-Z][A-Z0-9]*(_[A-Z0-9]+)+$").expect("a valid pattern");
    command
        .split_whitespace()
        .skip(1)
        .any(|w| !w.contains('=') && word.is_match(w.trim_matches(['"', '\''])))
}

/// Whether a command is a compiler call with no input file, such as the
/// flags a task states for its own build.
fn no_input(command: &str) -> bool {
    const COMPILERS: &[&str] = &[
        "gcc", "g++", "cc", "c++", "clang", "clang++", "rustc", "javac",
    ];
    let mut words = command.split_whitespace();
    words.next().is_some_and(|first| COMPILERS.contains(&first))
        && !words.any(|w| !w.starts_with('-') && extension(w).is_some())
}

/// The program's stated positionals: its `argparse` positionals when it
/// uses `argparse`, else the first usage line that names `program`. `None`
/// when it states neither.
#[must_use]
pub fn usage_of(text: &str, program: &str) -> Option<Usage> {
    let argument =
        Regex::new(r#"add_argument\(\s*["']([^"']+)["']([^)]*)\)"#).expect("a valid pattern");
    let nargs = Regex::new(r#"nargs\s*=\s*["']([+*?])["']"#).expect("a valid pattern");
    if text.contains("argparse") && argument.is_match(text) {
        let mut usage = Usage {
            required: Vec::new(),
            variadic: false,
            source: "argparse".to_string(),
        };
        for captures in argument.captures_iter(text) {
            let name = &captures[1];
            if name.starts_with('-') {
                continue;
            }
            match nargs.captures(&captures[2]).map(|c| c[1].to_string()) {
                Some(n) if n == "+" => {
                    usage.required.push(name.to_string());
                    usage.variadic = true;
                }
                Some(n) if n == "*" => usage.variadic = true,
                Some(_) => {}
                None => usage.required.push(name.to_string()),
            }
        }
        return Some(usage);
    }
    let head = Regex::new(r"(?i)\b(?:usage|cli|synopsis)\s*:\s*(.*)$").expect("a valid pattern");
    let invoked = Regex::new(&format!(
        r"\bpython3?\s+(?:-m\s+)?\S*\b{}\b",
        regex::escape(program)
    ))
    .expect("a valid pattern");
    let lines: Vec<&str> = text.lines().collect();
    let mut candidates: Vec<String> = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let Some(captures) = head.captures(line) else {
            continue;
        };
        let rest = captures[1].trim();
        if rest.is_empty() {
            candidates.extend(
                lines[n + 1..]
                    .iter()
                    .take_while(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_string()),
            );
        } else {
            candidates.push(rest.to_string());
        }
    }
    // Then any line that shows the program invoked.
    candidates.extend(
        lines
            .iter()
            .filter_map(|l| invoked.find(l).map(|m| l[m.start()..].to_string())),
    );
    candidates
        .iter()
        .find_map(|candidate| usage_line(candidate, program))
}

/// Reads one usage line: the words after `program` are `<placeholders>`,
/// file names, flags, or an optional `[...]` group that repeats.
fn usage_line(line: &str, program: &str) -> Option<Usage> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = words.iter().position(|w| {
        let w = w.trim_matches(['"', '\'', '`']);
        w == program || w.ends_with(&format!("/{program}"))
    })?;
    // A usage line inside a string literal ends where the literal does.
    let mut rest: Vec<&str> = Vec::new();
    for word in &words[at + 1..] {
        match word.find(['"', '\'', '`']) {
            Some(0) => rest.push(word),
            Some(cut) => {
                rest.push(&word[..cut]);
                break;
            }
            None => rest.push(word),
        }
    }
    let mut usage = Usage {
        required: Vec::new(),
        variadic: false,
        source: "usage line".to_string(),
    };
    let mut optional = 0usize;
    for word in rest {
        let word = word.trim_matches(['"', '\'', '`']);
        if word.is_empty() {
            continue;
        }
        if optional > 0 || word.starts_with('[') {
            optional += word.matches('[').count();
            if word.contains("...") {
                usage.variadic = true;
            }
            optional = optional.saturating_sub(word.matches(']').count());
            continue;
        }
        if word.starts_with('-') {
            continue;
        }
        if word.contains("...") {
            usage.variadic = true;
            continue;
        }
        if let Some(inner) = word.strip_prefix('<').and_then(|w| w.strip_suffix('>')) {
            usage.required.push(inner.to_string());
        } else if extract::as_path(word, "/").is_some() && word.contains('.') {
            usage.required.push(word.to_string());
        } else {
            // A subcommand or a word this reading doesn't know.
            return None;
        }
    }
    Some(usage)
}

fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, e)| e.to_lowercase())
}

/// The data files for `usage`, in order, when their count fits: exactly
/// the required count, or at least it when the program takes more. Each
/// placeholder takes the file whose name shares the most words with it,
/// and the rest follow in the order the instruction gives them. `None`
/// when the count doesn't fit.
#[must_use]
pub fn fit(usage: &Usage, files: &[String]) -> Option<Vec<String>> {
    let wanted: BTreeSet<String> = usage.required.iter().filter_map(|p| extension(p)).collect();
    let files: Vec<&String> = files
        .iter()
        .filter(|f| wanted.is_empty() || extension(f).is_some_and(|e| wanted.contains(&e)))
        .collect();
    let count = usage.required.len();
    let fits = if usage.variadic {
        files.len() >= count.max(1)
    } else {
        files.len() == count
    };
    if !fits || files.is_empty() {
        return None;
    }
    let mut left: Vec<&String> = files.clone();
    let mut out = Vec::new();
    for placeholder in &usage.required {
        let want = extract::stem_tokens(placeholder);
        let best = left
            .iter()
            .enumerate()
            .max_by_key(|(i, f)| {
                (
                    extract::stem_tokens(f).intersection(&want).count(),
                    std::cmp::Reverse(*i),
                )
            })
            .map(|(i, _)| i)?;
        out.push(left.remove(best).clone());
    }
    out.extend(left.into_iter().cloned());
    Some(out)
}

/// `word` as one shell word, quoted only when it has to be, so the command
/// reads as a session would type it.
#[must_use]
pub fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-+=:,@%".contains(c));
    if plain {
        word.to_string()
    } else {
        crate::accept::runner::sh_quote(word)
    }
}

/// A path as the session would type it from the working directory.
fn relative(path: &str, alias: &str) -> String {
    let alias = alias.trim_end_matches('/');
    path.strip_prefix(&format!("{alias}/"))
        .map_or_else(|| path.to_string(), str::to_string)
}

/// The data files the instruction names, by the name the instruction
/// uses, in its order: named files that aren't code, and the files
/// directly inside named directories.
fn data_files(instruction: &str, alias: &str, host: &Mapped) -> Vec<String> {
    let (units, _) = extract::segment(instruction);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut add = |path: String, out: &mut Vec<String>| {
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let code = extension(&path).is_some_and(|e| CODE.contains(&e.as_str()))
            || name.starts_with('.')
            || name.to_lowercase().starts_with("requirements")
            || name.eq_ignore_ascii_case("makefile");
        if !code && seen.insert(path.clone()) {
            out.push(path);
        }
    };
    for unit in &units {
        for (_, span) in extract::spans(&unit.text) {
            if extract::is_command(&span) {
                continue;
            }
            let Some(path) = extract::as_path(&span, alias) else {
                continue;
            };
            let path = path.trim_end_matches('/').to_string();
            let Some(local) = host.local(&path) else {
                continue;
            };
            if local.is_file() {
                add(path, &mut out);
            } else if local.is_dir() {
                let mut names: Vec<String> = std::fs::read_dir(&local)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                for name in names {
                    add(format!("{path}/{name}"), &mut out);
                }
            }
        }
    }
    out
}

fn read_text(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > PROGRAM_MAX {
        return None;
    }
    String::from_utf8(std::fs::read(path).ok()?).ok()
}

/// The arguments for a program with `text`, called `program` in its usage
/// line, and a phrase that says how they were chosen.
fn arguments(text: &str, program: &str, files: &[String], alias: &str) -> (Vec<String>, String) {
    match usage_of(text, program) {
        None => (
            Vec::new(),
            "no usage line or argparse positionals, so no arguments".to_string(),
        ),
        Some(usage) if usage.required.is_empty() && !usage.variadic => (
            Vec::new(),
            format!("its {} states no positional argument", usage.source),
        ),
        Some(usage) => match fit(&usage, files) {
            Some(chosen) => (
                chosen
                    .iter()
                    .map(|f| shell_word(&relative(f, alias)))
                    .collect(),
                format!(
                    "the instruction's {} data files fit its {} ({} required{})",
                    chosen.len(),
                    usage.source,
                    usage.required.len(),
                    if usage.variadic { ", more allowed" } else { "" }
                ),
            ),
            None => (
                Vec::new(),
                format!(
                    "the instruction's {} data files don't fit its {} ({} required{}), so no \
                     arguments",
                    files.len(),
                    usage.source,
                    usage.required.len(),
                    if usage.variadic { ", more allowed" } else { "" }
                ),
            ),
        },
    }
}

/// Top-level packages of `root` with a `__main__.py`, sorted.
fn packages(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            !name.starts_with('.')
                && !SKIPPED.contains(&name.as_str())
                && extract::is_identifier(name)
                && root.join(name).join("__main__.py").is_file()
        })
        .collect();
    out.sort();
    out
}

/// The text a package's usage is read from: `__main__.py`, then its other
/// modules, `__main__.py` first.
fn package_text(dir: &Path) -> String {
    let mut text = read_text(&dir.join("__main__.py")).unwrap_or_default();
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "py")
                && p.file_name().is_some_and(|n| n != "__main__.py")
        })
        .collect();
    names.sort();
    if !text.contains("argparse") {
        for path in names {
            if let Some(more) = read_text(&path).filter(|t| t.contains("argparse")) {
                text.push('\n');
                text.push_str(&more);
                break;
            }
        }
    }
    text
}

/// The `test` or else `check` target of the workspace's Makefile.
fn make_target(root: &Path) -> Option<&'static str> {
    let text = ["Makefile", "makefile", "GNUmakefile"]
        .iter()
        .find_map(|name| read_text(&root.join(name)))?;
    ["test", "check"].into_iter().find(|target| {
        Regex::new(&format!(r"(?m)^{target}\s*:([^=]|$)"))
            .expect("a valid pattern")
            .is_match(&text)
    })
}

/// Whether a Python file is a program: it has a `__main__` guard or reads
/// its arguments.
fn is_program(text: &str) -> bool {
    text.contains("__name__") && text.contains("__main__")
        || text.contains("sys.argv")
        || text.contains("argparse")
}

/// Every entry point of the task, in kind order, deduplicated by command,
/// at most [`MAX_ENTRIES`]. `root` is the untouched workspace on this host
/// and `alias` the directory the instruction calls it, the same as `root`
/// outside an offline replay.
pub async fn find(instruction: &str, root: &Path, alias: &str) -> Vec<Entry> {
    find_with(instruction, root, alias, Discovery::Named).await
}

/// [`find`] under `discovery`: with [`Discovery::Wide`], also the kinds
/// [`wide`] finds, at most [`MAX_ENTRIES_WIDE`].
pub async fn find_with(
    instruction: &str,
    root: &Path,
    alias: &str,
    discovery: Discovery,
) -> Vec<Entry> {
    let is_wide = discovery == Discovery::Wide;
    let host = Mapped {
        root: root.to_path_buf(),
        alias: alias.to_string(),
    };
    let pristine: Pristine = extract::gather(&host, instruction, alias).await;
    let draft = extract::draft(instruction, alias, &pristine);
    let mut out: Vec<Entry> = Vec::new();
    let mut commands: BTreeSet<String> = BTreeSet::new();
    let mut push = |entry: Entry, out: &mut Vec<Entry>| {
        if commands.insert(entry.command.clone()) {
            out.push(entry);
        }
    };
    // Named: the draft's commands and examples from the instruction.
    for item in &draft.items {
        let Some(command) = &item.command else {
            continue;
        };
        if !matches!(item.kind, ItemKind::Command | ItemKind::Example)
            || item.source != "instruction"
            || item.not_executable.is_some()
        {
            continue;
        }
        let refused = if let Some(why) = needs_network(command) {
            Some(format!("it needs the network: {why}"))
        } else if has_placeholder(command) {
            Some("it is a usage pattern with placeholder words".to_string())
        } else if no_input(command) {
            Some("it is a compiler call that names no input file".to_string())
        } else {
            None
        };
        push(
            Entry {
                kind: EntryKind::Named,
                command: command.clone(),
                why: format!("the instruction names it: {}", item.span),
                refused,
            },
            &mut out,
        );
    }
    let named_text: String = out
        .iter()
        .map(|e| e.command.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut files = data_files(instruction, alias, &host);
    if is_wide && files.is_empty() {
        files = wide::shipped_inputs(root, alias);
    }
    if is_wide {
        for entry in wide::modules(root, alias, &files, &named_text) {
            push(entry, &mut out);
        }
        scripts(
            instruction,
            alias,
            &host,
            &files,
            &named_text,
            true,
            &mut |e| {
                push(e, &mut out);
            },
        );
        for entry in wide::others(root, alias, &files, &named_text) {
            push(entry, &mut out);
        }
        out.truncate(MAX_ENTRIES_WIDE);
        return out;
    }
    // Module: python3 -m <package>.
    for package in packages(root) {
        if named_text.contains(&format!("-m {package}")) {
            continue;
        }
        let text = package_text(&root.join(&package));
        let (args, how) = arguments(&text, &package, &files, alias);
        let mut command = format!("python3 -m {package}");
        for arg in &args {
            command.push(' ');
            command.push_str(arg);
        }
        push(
            Entry {
                kind: EntryKind::Module,
                command,
                why: format!("{package}/__main__.py; {how}"),
                refused: None,
            },
            &mut out,
        );
    }
    // Make: the test or check target.
    if let Some(target) = make_target(root) {
        let command = format!("make {target}");
        if !named_text.contains(&command) {
            push(
                Entry {
                    kind: EntryKind::Make,
                    command,
                    why: format!("the Makefile has a {target} target"),
                    refused: None,
                },
                &mut out,
            );
        }
    }
    // Script: a program the instruction names by path.
    scripts(
        instruction,
        alias,
        &host,
        &files,
        &named_text,
        false,
        &mut |e| {
            push(e, &mut out);
        },
    );
    out.truncate(MAX_ENTRIES);
    out
}

/// Script: each Python or shell program the instruction names by path.
/// With `sweep`, a program whose usage the files don't fit runs once per
/// file ([`wide::argument_sets`]).
fn scripts(
    instruction: &str,
    alias: &str,
    host: &Mapped,
    files: &[String],
    named_text: &str,
    sweep: bool,
    push: &mut dyn FnMut(Entry),
) {
    let (units, _) = extract::segment(instruction);
    for unit in &units {
        for (_, span) in extract::spans(&unit.text) {
            if extract::is_command(&span) {
                continue;
            }
            let Some(path) = extract::as_path(&span, alias) else {
                continue;
            };
            let Some(ext) = extension(&path).filter(|e| e == "py" || e == "sh") else {
                continue;
            };
            let Some(text) = host.local(&path).and_then(|local| read_text(&local)) else {
                continue;
            };
            let shown = relative(&path, alias);
            if named_text.contains(&path) || named_text.contains(&shown) {
                continue;
            }
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            let interpreter = if ext == "py" {
                if !is_program(&text) {
                    continue;
                }
                "python3"
            } else if text.lines().next().is_some_and(|l| l.contains("bash")) {
                "bash"
            } else {
                "sh"
            };
            let sets = if sweep {
                wide::argument_sets(&text, &name, files, alias)
            } else {
                vec![arguments(&text, &name, files, alias)]
            };
            for (args, how) in sets {
                let mut command = format!("{interpreter} {}", shell_word(&shown));
                for arg in &args {
                    command.push(' ');
                    command.push_str(arg);
                }
                push(Entry {
                    kind: EntryKind::Script,
                    command,
                    why: format!("the instruction names {shown}; {how}"),
                    refused: None,
                });
            }
        }
    }
}
