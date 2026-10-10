//! Built-in file tools: `Read`, `Edit`, `Write`, `Grep` and `Glob` (#11168).
//!
//! The model reads and changes files through these instead of shell
//! commands. Every path stays inside the workspace: the checkout that holds
//! the working directory (the nearest folder with `.git`), or the working
//! directory itself outside a checkout. Symbolic links are followed before
//! that check, so a link cannot reach outside.
//!
//! Reads are free. A change (`Edit`, `Write`) goes through the same approval
//! gate as a shell command that changes something ([`crate::approval`]):
//! with no gate installed it runs, under a gate it asks the person, and a
//! tool-free charter refuses it.
//!
//! `Edit` replaces an exact string and refuses one that is missing or appears
//! more than once (unless `replace_all`). `Edit`, and `Write` over an existing
//! file, need a `Read` of the file first, and refuse when the file changed
//! after that read. Text files only: a file with NUL bytes or that is not
//! UTF-8 is refused, and so is one over [`MAX_FILE_BYTES`]. `Grep` and `Glob`
//! skip what `.gitignore` ignores, hidden files, and binary files.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The largest file the tools read or change.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Lines a `Read` returns when it names no limit.
pub const DEFAULT_LINES: usize = 2000;
/// A longer line is cut in `Read` and `Grep` output.
const MAX_LINE_CHARS: usize = 2000;
/// The most text one `Read` returns; a longer page ends early.
const MAX_READ_OUTPUT: usize = 256 * 1024;
/// The most `Grep` matches or `Glob` paths returned.
const DEFAULT_RESULTS: usize = 200;
const MAX_RESULTS: usize = 1000;
/// `Grep` skips files larger than this.
const MAX_GREP_FILE: u64 = 2 * 1024 * 1024;
/// Diff lines kept in a result.
const MAX_DIFF_LINES: usize = 400;

const NAMES: [&str; 5] = ["Read", "Edit", "Write", "Grep", "Glob"];

/// Whether `name` is one of these tools.
#[must_use]
pub fn is_tool(name: &str) -> bool {
    NAMES.contains(&name)
}

/// The tool definitions the model sees.
#[must_use]
pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{
            "name":"Read",
            "description":"Read a text file in the workspace. Returns numbered lines (\"N\\ttext\"), up to 2000 from `offset` (1-based). For a longer file, call again with the `next_offset` the result gives. Read a file before you Edit it or overwrite it with Write.",
            "parameters":{"type":"object","additionalProperties":false,"required":["path"],"properties":{
                "path":{"type":"string","description":"Relative to the working directory, or absolute inside the workspace."},
                "offset":{"type":"integer","minimum":1,"description":"The first line to return, from 1."},
                "limit":{"type":"integer","minimum":1,"maximum":10000,"description":"How many lines to return. Default 2000."}
            }}
        }}),
        json!({"type":"function","function":{
            "name":"Edit",
            "description":"Replace an exact string in a file you have read. `old_string` must match the file exactly, including whitespace and indentation, and appear once; include more surrounding lines to make it unique, or set replace_all to change every occurrence. Do not include the line numbers Read shows.",
            "parameters":{"type":"object","additionalProperties":false,"required":["path","old_string","new_string"],"properties":{
                "path":{"type":"string"},
                "old_string":{"type":"string","minLength":1},
                "new_string":{"type":"string"},
                "replace_all":{"type":"boolean","description":"Replace every occurrence. Default false."}
            }}
        }}),
        json!({"type":"function","function":{
            "name":"Write",
            "description":"Create a file, or replace a whole file you have read, with `content`. Missing folders are created. Prefer Edit for changes to part of a file.",
            "parameters":{"type":"object","additionalProperties":false,"required":["path","content"],"properties":{
                "path":{"type":"string"},
                "content":{"type":"string"}
            }}
        }}),
        json!({"type":"function","function":{
            "name":"Grep",
            "description":"Search file contents in the workspace with a regular expression (Rust regex syntax). Skips files .gitignore ignores, hidden files and binary files. Returns \"path:line:text\" matches.",
            "parameters":{"type":"object","additionalProperties":false,"required":["pattern"],"properties":{
                "pattern":{"type":"string","minLength":1},
                "path":{"type":"string","description":"A folder or file to search. Default: the working directory."},
                "glob":{"type":"string","description":"Only files whose path matches this glob, such as \"*.rs\" or \"src/**/*.toml\"."},
                "case_insensitive":{"type":"boolean"},
                "max_results":{"type":"integer","minimum":1,"maximum":1000,"description":"Default 200."}
            }}
        }}),
        json!({"type":"function","function":{
            "name":"Glob",
            "description":"List workspace files whose path matches a glob, such as \"**/*.rs\" or \"src/*.toml\", newest first. Skips files .gitignore ignores and hidden files.",
            "parameters":{"type":"object","additionalProperties":false,"required":["pattern"],"properties":{
                "pattern":{"type":"string","minLength":1},
                "path":{"type":"string","description":"The folder to search. Default: the working directory."},
                "max_results":{"type":"integer","minimum":1,"maximum":1000,"description":"Default 200."}
            }}
        }}),
    ]
}

/// What the model reads about these tools.
pub const INSTRUCTIONS: &str = "Use Read, Edit, Write, Grep and Glob for files instead of shell commands such as cat, sed, grep or find. Read a file before you Edit or Write it. Edit takes an exact string that appears once; give more surrounding lines when it is not unique. Paths stay inside the workspace. Reads never ask; under an approval policy, Edit and Write ask like any other change.\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArguments {
    path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditArguments {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteArguments {
    path: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrepArguments {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    max_results: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobArguments {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

/// Runs one tool call in `cwd`.
///
/// # Errors
/// The arguments are invalid, the path leaves the workspace, the file is
/// missing, binary or too large, an edit does not match exactly once, the
/// file changed since it was read, or the change was refused.
pub fn execute(name: &str, arguments: Value, cwd: &Path) -> Result<Value, String> {
    let workspace = Workspace::new(cwd)?;
    match name {
        "Read" => {
            let args: ReadArguments = serde_json::from_value(arguments)
                .map_err(|_| "Read takes a path, and optionally offset and limit.")?;
            workspace.read(&args)
        }
        "Edit" => {
            let args: EditArguments = serde_json::from_value(arguments).map_err(
                |_| "Edit takes path, old_string and new_string, and optionally replace_all.",
            )?;
            workspace.edit(&args)
        }
        "Write" => {
            let args: WriteArguments =
                serde_json::from_value(arguments).map_err(|_| "Write takes a path and content.")?;
            workspace.write(&args)
        }
        "Grep" => {
            let args: GrepArguments = serde_json::from_value(arguments).map_err(|_| {
                "Grep takes a pattern, and optionally path, glob, case_insensitive and max_results."
            })?;
            workspace.grep(&args)
        }
        "Glob" => {
            let args: GlobArguments = serde_json::from_value(arguments)
                .map_err(|_| "Glob takes a pattern, and optionally path and max_results.")?;
            workspace.glob(&args)
        }
        _ => Err("This file tool is unknown.".into()),
    }
}

/// The content each file had when this process last read or wrote it.
fn ledger() -> &'static Mutex<HashMap<PathBuf, [u8; 32]>> {
    static LEDGER: OnceLock<Mutex<HashMap<PathBuf, [u8; 32]>>> = OnceLock::new();
    LEDGER.get_or_init(Default::default)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn remember(path: &Path, bytes: &[u8]) {
    ledger()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(path.to_owned(), digest(bytes));
}

/// Whether `bytes` is what the last read of `path` saw.
fn seen(path: &Path, bytes: &[u8]) -> Result<(), String> {
    match ledger()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(path)
    {
        None => Err("Read the file before changing it.".into()),
        Some(stamp) if *stamp != digest(bytes) => {
            Err("The file changed since you read it. Read it again, then retry.".into())
        }
        Some(_) => Ok(()),
    }
}

struct Workspace {
    root: PathBuf,
    cwd: PathBuf,
}

impl Workspace {
    fn new(cwd: &Path) -> Result<Self, String> {
        let cwd = cwd
            .canonicalize()
            .map_err(|_| "The working directory is unavailable.".to_string())?;
        let root = cwd
            .ancestors()
            .find(|folder| folder.join(".git").exists())
            .unwrap_or(&cwd)
            .to_owned();
        Ok(Self { root, cwd })
    }

    /// `path` resolved inside the workspace. A path that does not exist yet
    /// resolves through its nearest existing folder.
    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let path = path.trim();
        if path.is_empty() {
            return Err("Name a path.".into());
        }
        let path = Path::new(path);
        let joined = if path.is_absolute() {
            path.to_owned()
        } else {
            self.cwd.join(path)
        };
        let mut existing = joined.as_path();
        let mut rest = Vec::new();
        let resolved = loop {
            if let Ok(found) = existing.canonicalize() {
                break found;
            }
            let (Some(parent), Some(name)) = (existing.parent(), existing.file_name()) else {
                return Err(
                    "Write the path without `..` past a folder that does not exist.".into(),
                );
            };
            rest.push(name.to_owned());
            existing = parent;
        };
        if Path::new(&joined).strip_prefix(existing).is_ok_and(|tail| {
            tail.components()
                .any(|c| !matches!(c, Component::Normal(_)))
        }) {
            return Err("Write the path without `..` past a folder that does not exist.".into());
        }
        let mut full = resolved;
        for name in rest.into_iter().rev() {
            full.push(name);
        }
        if !full.starts_with(&self.root) {
            return Err(format!(
                "{} is outside the workspace ({}).",
                full.display(),
                self.root.display()
            ));
        }
        Ok(full)
    }

    /// `path` as the result shows it: relative to the workspace.
    fn shown(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .ok()
            .filter(|relative| !relative.as_os_str().is_empty())
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// A text file's bytes and text.
    fn text(&self, path: &Path) -> Result<(Vec<u8>, String), String> {
        let shown = self.shown(path);
        let meta = std::fs::metadata(path).map_err(|_| format!("{shown} does not exist."))?;
        if meta.is_dir() {
            return Err(format!("{shown} is a folder. Use Glob to list its files."));
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(format!(
                "{shown} is {} MB, over the {} MB limit. Use Grep to find the part you need.",
                meta.len() / (1024 * 1024),
                MAX_FILE_BYTES / (1024 * 1024)
            ));
        }
        let bytes = std::fs::read(path).map_err(|error| format!("{shown}: {error}"))?;
        if bytes[..bytes.len().min(8192)].contains(&0) {
            return Err(format!("{shown} is a binary file, not text."));
        }
        let text =
            String::from_utf8(bytes.clone()).map_err(|_| format!("{shown} is not UTF-8 text."))?;
        Ok((bytes, text))
    }

    fn read(&self, args: &ReadArguments) -> Result<Value, String> {
        let path = self.resolve(&args.path)?;
        let (bytes, text) = self.text(&path)?;
        remember(&path, &bytes);
        let offset = args.offset.unwrap_or(1).max(1);
        let limit = args.limit.unwrap_or(DEFAULT_LINES).clamp(1, 10_000);
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        if total > 0 && offset > total {
            return Err(format!(
                "The file has {total} lines; offset {offset} is past the end."
            ));
        }
        let mut content = String::new();
        let mut next = None;
        let mut cut = false;
        for (index, line) in lines.iter().enumerate().skip(offset - 1).take(limit) {
            let number = index + 1;
            if content.len() > MAX_READ_OUTPUT {
                next = Some(number);
                break;
            }
            let line = if line.chars().count() > MAX_LINE_CHARS {
                cut = true;
                let kept: String = line.chars().take(MAX_LINE_CHARS).collect();
                format!("{kept}…")
            } else {
                (*line).to_owned()
            };
            content.push_str(&format!("{number:>6}\t{line}\n"));
        }
        let end = offset - 1 + limit;
        if next.is_none() && end < total {
            next = Some(end + 1);
        }
        let mut result = json!({
            "path": self.shown(&path),
            "content": content,
            "total_lines": total,
        });
        if total == 0 {
            result["content"] = json!("(empty file)");
        }
        if let Some(next) = next {
            result["next_offset"] = json!(next);
        }
        if cut {
            result["note"] = json!(format!("Lines over {MAX_LINE_CHARS} characters are cut."));
        }
        Ok(result)
    }

    fn edit(&self, args: &EditArguments) -> Result<Value, String> {
        let path = self.resolve(&args.path)?;
        if args.old_string.is_empty() {
            return Err("old_string is empty. To create a file, use Write.".into());
        }
        if args.old_string == args.new_string {
            return Err("old_string and new_string are the same; nothing would change.".into());
        }
        let (bytes, text) = self.text(&path)?;
        seen(&path, &bytes)?;
        let count = text.matches(&args.old_string).count();
        if count == 0 {
            return Err("old_string is not in the file. Match the file exactly, including whitespace; Read it again if unsure.".into());
        }
        if count > 1 && !args.replace_all {
            return Err(format!(
                "old_string appears {count} times. Include more surrounding lines so it is unique, or set replace_all."
            ));
        }
        let updated = if args.replace_all {
            text.replace(&args.old_string, &args.new_string)
        } else {
            text.replacen(&args.old_string, &args.new_string, 1)
        };
        let shown = self.shown(&path);
        approve(&format!("Edit {shown}"))?;
        std::fs::write(&path, &updated).map_err(|error| format!("{shown}: {error}"))?;
        remember(&path, updated.as_bytes());
        let diff = unified(&text, &updated);
        Ok(json!({
            "path": shown,
            "replacements": count.min(if args.replace_all { usize::MAX } else { 1 }),
            "added": diff.added,
            "removed": diff.removed,
            "diff": diff.text,
        }))
    }

    fn write(&self, args: &WriteArguments) -> Result<Value, String> {
        let path = self.resolve(&args.path)?;
        let shown = self.shown(&path);
        if path
            .strip_prefix(&self.root)
            .is_ok_and(|relative| relative.components().any(|c| c.as_os_str() == ".git"))
        {
            return Err("Files inside .git are not changed with Write.".into());
        }
        if u64::try_from(args.content.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
            return Err(format!(
                "The content is over the {} MB limit.",
                MAX_FILE_BYTES / (1024 * 1024)
            ));
        }
        let previous = if path.exists() {
            let (bytes, text) = self.text(&path)?;
            seen(&path, &bytes)?;
            Some(text)
        } else {
            None
        };
        approve(&format!(
            "{} {shown}",
            if previous.is_some() {
                "Overwrite"
            } else {
                "Create"
            }
        ))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("{shown}: {error}"))?;
        }
        std::fs::write(&path, &args.content).map_err(|error| format!("{shown}: {error}"))?;
        remember(&path, args.content.as_bytes());
        let diff = unified(previous.as_deref().unwrap_or(""), &args.content);
        Ok(json!({
            "path": shown,
            "created": previous.is_none(),
            "bytes": args.content.len(),
            "added": diff.added,
            "removed": diff.removed,
            "diff": diff.text,
        }))
    }

    fn walker(&self, path: Option<&str>) -> Result<(PathBuf, ignore::Walk), String> {
        let start = match path {
            Some(path) => self.resolve(path)?,
            None => self.cwd.clone(),
        };
        if !start.exists() {
            return Err(format!("{} does not exist.", self.shown(&start)));
        }
        let walk = ignore::WalkBuilder::new(&start)
            .standard_filters(true)
            .require_git(false)
            .follow_links(false)
            .build();
        Ok((start, walk))
    }

    fn grep(&self, args: &GrepArguments) -> Result<Value, String> {
        let pattern = regex::RegexBuilder::new(&args.pattern)
            .case_insensitive(args.case_insensitive)
            .size_limit(10 * 1024 * 1024)
            .build()
            .map_err(|error| format!("The pattern is not a valid regular expression: {error}"))?;
        let filter = args.glob.as_deref().map(glob_matcher).transpose()?;
        let limit = args
            .max_results
            .unwrap_or(DEFAULT_RESULTS)
            .clamp(1, MAX_RESULTS);
        let (start, walk) = self.walker(args.path.as_deref())?;
        let mut matches = Vec::new();
        let mut files = 0usize;
        let mut truncated = false;
        'files: for entry in walk.flatten() {
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            let path = entry.path();
            if let Some(filter) = &filter
                && !matches_glob(filter, path, &start)
            {
                continue;
            }
            if entry
                .metadata()
                .map_or(true, |meta| meta.len() > MAX_GREP_FILE)
            {
                continue;
            }
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            if bytes[..bytes.len().min(8192)].contains(&0) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let mut hit = false;
            for (index, line) in text.lines().enumerate() {
                if pattern.is_match(line) {
                    if matches.len() >= limit {
                        truncated = true;
                        break 'files;
                    }
                    hit = true;
                    let line: String = line.chars().take(MAX_LINE_CHARS).collect();
                    matches.push(format!("{}:{}:{line}", self.shown(path), index + 1));
                }
            }
            files += usize::from(hit);
        }
        let mut result = json!({
            "matches": matches.join("\n"),
            "count": matches.len(),
            "files": files,
        });
        if matches.is_empty() {
            result["matches"] = json!("No matches.");
        }
        if truncated {
            result["truncated"] = json!(true);
            result["note"] = json!(format!(
                "Stopped at {limit} matches. Narrow the pattern, path or glob to see the rest."
            ));
        }
        Ok(result)
    }

    fn glob(&self, args: &GlobArguments) -> Result<Value, String> {
        let matcher = glob_matcher(&args.pattern)?;
        let limit = args
            .max_results
            .unwrap_or(DEFAULT_RESULTS)
            .clamp(1, MAX_RESULTS);
        let (start, walk) = self.walker(args.path.as_deref())?;
        let mut found = Vec::new();
        for entry in walk.flatten() {
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            if !matches_glob(&matcher, entry.path(), &start) {
                continue;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            found.push((modified, self.shown(entry.path())));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let total = found.len();
        found.truncate(limit);
        let paths: Vec<String> = found.into_iter().map(|(_, path)| path).collect();
        let mut result = json!({
            "files": if paths.is_empty() { "No files match.".to_string() } else { paths.join("\n") },
            "count": total,
        });
        if total > limit {
            result["truncated"] = json!(true);
            result["note"] = json!(format!("Showing the newest {limit} of {total}."));
        }
        Ok(result)
    }
}

fn glob_matcher(pattern: &str) -> Result<globset::GlobMatcher, String> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|error| format!("The glob is not valid: {error}"))
}

/// A pattern without `/` matches a file name anywhere, as in .gitignore;
/// one with `/` matches the path from the search folder.
fn matches_glob(matcher: &globset::GlobMatcher, path: &Path, start: &Path) -> bool {
    let relative = path.strip_prefix(start).unwrap_or(path);
    if matcher.glob().glob().contains('/') {
        matcher.is_match(relative)
    } else {
        path.file_name().is_some_and(|name| matcher.is_match(name))
    }
}

/// Asks the approval gate about a change, as for a shell command that
/// changes something.
fn approve(action: &str) -> Result<(), String> {
    match crate::approval::check_change(action) {
        crate::approval::Verdict::Run => Ok(()),
        crate::approval::Verdict::Refused(why) => Err(why),
    }
}

/// A unified diff of a change, hunks only, with its line counts.
pub struct Diff {
    pub text: String,
    pub added: usize,
    pub removed: usize,
}

/// The `@@` hunks between `old` and `new`, three lines of context, at most
/// [`MAX_DIFF_LINES`] lines.
#[must_use]
pub fn unified(old: &str, new: &str) -> Diff {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (mid_a, mid_b) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    // Each operation: (tag, old index, new index).
    let mut ops: Vec<(char, usize, usize)> = (0..prefix).map(|i| (' ', i, i)).collect();
    if mid_a.len().saturating_mul(mid_b.len()) <= 4_000_000 {
        // Longest common subsequence over the changed middle.
        let (n, m) = (mid_a.len(), mid_b.len());
        let mut table = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                table[i * (m + 1) + j] = if mid_a[i] == mid_b[j] {
                    table[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    table[(i + 1) * (m + 1) + j].max(table[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && mid_a[i] == mid_b[j] {
                ops.push((' ', prefix + i, prefix + j));
                i += 1;
                j += 1;
            } else if i < n
                && (j == m || table[(i + 1) * (m + 1) + j] >= table[i * (m + 1) + j + 1])
            {
                ops.push(('-', prefix + i, prefix + j));
                i += 1;
            } else {
                ops.push(('+', prefix + i, prefix + j));
                j += 1;
            }
        }
    } else {
        ops.extend((0..mid_a.len()).map(|i| ('-', prefix + i, prefix)));
        ops.extend((0..mid_b.len()).map(|j| ('+', prefix + mid_a.len(), prefix + j)));
    }
    ops.extend((0..suffix).map(|k| (' ', a.len() - suffix + k, b.len() - suffix + k)));

    let added = ops.iter().filter(|op| op.0 == '+').count();
    let removed = ops.iter().filter(|op| op.0 == '-').count();
    // Group changes with three lines of context into hunks.
    const CONTEXT: usize = 3;
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k].0 != ' ').collect();
    let mut text = String::new();
    let mut lines = 0usize;
    let mut k = 0;
    while k < changed.len() && lines < MAX_DIFF_LINES {
        let start = changed[k].saturating_sub(CONTEXT);
        let mut end = changed[k] + 1;
        while k + 1 < changed.len() && changed[k + 1] <= end + 2 * CONTEXT {
            k += 1;
            end = changed[k] + 1;
        }
        let end = (end + CONTEXT).min(ops.len());
        let hunk = &ops[start..end.min(start + MAX_DIFF_LINES - lines)];
        let old_count = hunk.iter().filter(|op| op.0 != '+').count();
        let new_count = hunk.iter().filter(|op| op.0 != '-').count();
        text.push_str(&format!(
            "@@ -{},{old_count} +{},{new_count} @@\n",
            hunk[0].1 + usize::from(old_count > 0),
            hunk[0].2 + usize::from(new_count > 0),
        ));
        for &(tag, i, j) in hunk {
            let line = if tag == '+' { b[j] } else { a[i] };
            text.push(tag);
            text.push_str(line);
            text.push('\n');
        }
        lines += hunk.len();
        k += 1;
    }
    Diff {
        text,
        added,
        removed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        (dir, root)
    }

    fn run(name: &str, arguments: Value, cwd: &Path) -> Result<Value, String> {
        let _lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        execute(name, arguments, cwd)
    }

    #[test]
    fn read_numbers_lines_and_pages_a_big_file() {
        let (_dir, root) = workspace();
        let body: String = (1..=5000).map(|n| format!("line {n}\n")).collect();
        std::fs::write(root.join("big.txt"), body).unwrap();
        let first = run("Read", json!({"path":"big.txt"}), &root).unwrap();
        assert_eq!(first["total_lines"], 5000);
        assert_eq!(first["next_offset"], 2001);
        let content = first["content"].as_str().unwrap();
        assert!(content.starts_with("     1\tline 1\n"));
        assert_eq!(content.lines().count(), DEFAULT_LINES);
        let last = run("Read", json!({"path":"big.txt","offset":4001}), &root).unwrap();
        assert!(last.get("next_offset").is_none());
        assert!(
            last["content"]
                .as_str()
                .unwrap()
                .ends_with("  5000\tline 5000\n")
        );
        let window = run(
            "Read",
            json!({"path":"big.txt","offset":10,"limit":2}),
            &root,
        )
        .unwrap();
        assert_eq!(window["content"], "    10\tline 10\n    11\tline 11\n");
        assert_eq!(window["next_offset"], 12);
        assert!(run("Read", json!({"path":"big.txt","offset":6000}), &root).is_err());
    }

    #[test]
    fn edit_needs_a_unique_exact_match_after_a_read() {
        let (_dir, root) = workspace();
        let file = root.join("a.rs");
        std::fs::write(&file, "fn a() {}\nfn b() {}\nfn a() {}\n").unwrap();
        let edit = |old: &str, new: &str, all: bool| {
            run(
                "Edit",
                json!({"path":"a.rs","old_string":old,"new_string":new,"replace_all":all}),
                &root,
            )
        };
        assert!(
            edit("fn b", "fn c", false)
                .unwrap_err()
                .contains("Read the file")
        );
        run("Read", json!({"path":"a.rs"}), &root).unwrap();
        let twice = edit("fn a() {}", "fn z() {}", false).unwrap_err();
        assert!(twice.contains("appears 2 times"), "{twice}");
        assert!(
            edit("fn q", "fn r", false)
                .unwrap_err()
                .contains("not in the file")
        );
        assert!(edit("fn b", "fn b", false).is_err());
        let done = edit("fn b() {}", "fn c() {}", false).unwrap();
        assert_eq!(done["replacements"], 1);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "fn a() {}\nfn c() {}\nfn a() {}\n"
        );
        assert!(
            done["diff"]
                .as_str()
                .unwrap()
                .contains("-fn b() {}\n+fn c() {}\n")
        );
        assert_eq!(
            (done["added"].as_u64(), done["removed"].as_u64()),
            (Some(1), Some(1))
        );
        // An edit counts as a read of the new content, so another edit works.
        let all = edit("fn a() {}", "fn y() {}", true).unwrap();
        assert_eq!(all["replacements"], 2);
        // A change made outside the tools needs a fresh read.
        std::fs::write(&file, "changed\n").unwrap();
        assert!(
            edit("changed", "x", false)
                .unwrap_err()
                .contains("changed since")
        );
    }

    #[test]
    fn write_creates_files_and_refuses_to_overwrite_an_unread_one() {
        let (_dir, root) = workspace();
        let made = run(
            "Write",
            json!({"path":"src/new/mod.rs","content":"one\ntwo\n"}),
            &root,
        )
        .unwrap();
        assert_eq!(made["created"], true);
        assert_eq!(made["added"], 2);
        assert_eq!(
            std::fs::read_to_string(root.join("src/new/mod.rs")).unwrap(),
            "one\ntwo\n"
        );
        std::fs::write(root.join("old.txt"), "keep\n").unwrap();
        let refused = run("Write", json!({"path":"old.txt","content":"gone\n"}), &root);
        assert!(refused.unwrap_err().contains("Read the file"));
        assert_eq!(
            std::fs::read_to_string(root.join("old.txt")).unwrap(),
            "keep\n"
        );
        run("Read", json!({"path":"old.txt"}), &root).unwrap();
        let replaced = run("Write", json!({"path":"old.txt","content":"new\n"}), &root).unwrap();
        assert_eq!(replaced["created"], false);
        assert!(replaced["diff"].as_str().unwrap().contains("-keep\n+new\n"));
        assert!(run("Write", json!({"path":".git/config","content":"x"}), &root).is_err());
    }

    #[test]
    fn paths_stay_inside_the_workspace() {
        let (dir, root) = workspace();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "no").unwrap();
        let sub = root.join("sub");
        std::fs::create_dir(&sub).unwrap();
        for path in [
            "../../etc/passwd".to_string(),
            outside.path().join("secret.txt").display().to_string(),
            "/etc/hosts".into(),
            "missing/../../../x".into(),
        ] {
            let error = run("Read", json!({"path":path}), &sub).unwrap_err();
            assert!(
                error.contains("outside the workspace") || error.contains("`..`"),
                "{path}: {error}"
            );
            assert!(run("Write", json!({"path":path,"content":"x"}), &sub).is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
            let error = run("Read", json!({"path":"link/secret.txt"}), &root).unwrap_err();
            assert!(error.contains("outside the workspace"), "{error}");
        }
        // The checkout root is the workspace, so a subfolder can reach it.
        std::fs::write(root.join("top.txt"), "top\n").unwrap();
        assert!(run("Read", json!({"path":"../top.txt"}), &sub).is_ok());
        drop(dir);
    }

    #[test]
    fn binary_large_and_folder_paths_are_refused() {
        let (_dir, root) = workspace();
        std::fs::write(root.join("image.png"), [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
        assert!(
            run("Read", json!({"path":"image.png"}), &root)
                .unwrap_err()
                .contains("binary")
        );
        std::fs::write(root.join("latin.txt"), [0xff, 0xfe, b'a']).unwrap();
        assert!(
            run("Read", json!({"path":"latin.txt"}), &root)
                .unwrap_err()
                .contains("UTF-8")
        );
        let big = std::fs::File::create(root.join("huge.log")).unwrap();
        big.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(
            run("Read", json!({"path":"huge.log"}), &root)
                .unwrap_err()
                .contains("limit")
        );
        assert!(
            run("Read", json!({"path":"."}), &root)
                .unwrap_err()
                .contains("folder")
        );
        assert!(
            run("Read", json!({"path":"nope.txt"}), &root)
                .unwrap_err()
                .contains("does not exist")
        );
        assert!(run("Read", json!({"path":"x","extra":1}), &root).is_err());
    }

    #[test]
    fn grep_and_glob_respect_gitignore_and_skip_binaries() {
        let (_dir, root) = workspace();
        std::fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "fn needle() {}\n").unwrap();
        std::fs::write(root.join("src/deep/mod.rs"), "// Needle here\nother\n").unwrap();
        std::fs::write(root.join("target/out.rs"), "fn needle() {}\n").unwrap();
        std::fs::write(root.join("run.log"), "needle\n").unwrap();
        std::fs::write(root.join("blob.bin"), b"needle\0\0").unwrap();
        let found = run("Grep", json!({"pattern":"needle"}), &root).unwrap();
        assert_eq!(found["matches"], "src/lib.rs:1:fn needle() {}");
        let any_case = run(
            "Grep",
            json!({"pattern":"needle","case_insensitive":true,"glob":"*.rs"}),
            &root,
        )
        .unwrap();
        assert_eq!(any_case["count"], 2);
        let scoped = run("Grep", json!({"pattern":"other","path":"src/deep"}), &root).unwrap();
        assert_eq!(scoped["matches"], "src/deep/mod.rs:2:other");
        let capped = run("Grep", json!({"pattern":"e","max_results":1}), &root).unwrap();
        assert_eq!(capped["truncated"], true);
        assert!(run("Grep", json!({"pattern":"("}), &root).is_err());

        let rust = run("Glob", json!({"pattern":"**/*.rs"}), &root).unwrap();
        let mut files: Vec<&str> = rust["files"].as_str().unwrap().lines().collect();
        files.sort_unstable();
        assert_eq!(files, ["src/deep/mod.rs", "src/lib.rs"]);
        let named = run("Glob", json!({"pattern":"mod.rs"}), &root).unwrap();
        assert_eq!(named["files"], "src/deep/mod.rs");
        let top = run("Glob", json!({"pattern":"src/*.rs"}), &root).unwrap();
        assert_eq!(top["files"], "src/lib.rs");
        assert_eq!(
            run("Glob", json!({"pattern":"*.log"}), &root).unwrap()["count"],
            0
        );
        assert!(run("Glob", json!({"pattern":"*","path":"/etc"}), &root).is_err());
    }

    #[test]
    fn a_gated_change_asks_and_a_rejection_leaves_the_file() {
        let (_dir, root) = workspace();
        std::fs::write(root.join("notes.txt"), "draft\n").unwrap();
        let _lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let desk = crate::approval::Desk::new();
        crate::approval::install(Some(crate::approval::Gate {
            desk: desk.clone(),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }));
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                crate::approval::install(None);
            }
        }
        let _reset = Reset;
        // Reads never ask.
        execute("Read", json!({"path":"notes.txt"}), &root).unwrap();
        execute("Grep", json!({"pattern":"draft"}), &root).unwrap();
        assert!(desk.drain().is_empty());
        let answer = |decision: &'static str| {
            let desk = desk.clone();
            std::thread::spawn(move || {
                loop {
                    if let Some(event) = desk
                        .drain()
                        .into_iter()
                        .find(|event| event["event"] == "approval")
                    {
                        assert_eq!(event["command"], "Edit notes.txt");
                        desk.answer(&format!("{decision} {}", event["id"])).unwrap();
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })
        };
        let edit = json!({"path":"notes.txt","old_string":"draft","new_string":"final"});
        let rejecting = answer("reject");
        let refused = execute("Edit", edit.clone(), &root).unwrap_err();
        rejecting.join().unwrap();
        assert!(refused.contains("rejected"), "{refused}");
        assert_eq!(
            std::fs::read_to_string(root.join("notes.txt")).unwrap(),
            "draft\n"
        );
        let confirming = answer("confirm");
        execute("Edit", edit, &root).unwrap();
        confirming.join().unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("notes.txt")).unwrap(),
            "final\n"
        );
    }

    #[test]
    fn diffs_group_changes_into_hunks_with_context() {
        let old: String = (1..=30).map(|n| format!("{n}\n")).collect();
        let new: String = (1..=30)
            .map(|n| match n {
                5 => "five\n".to_string(),
                25 => "twenty-five\n".to_string(),
                n => format!("{n}\n"),
            })
            .collect();
        let diff = unified(&old, &new);
        assert_eq!((diff.added, diff.removed), (2, 2));
        let hunks = coder_terminal::components::diff::hunks(&diff.text);
        assert_eq!(hunks.len(), 2, "{}", diff.text);
        assert!(
            diff.text
                .starts_with("@@ -2,7 +2,7 @@\n 2\n 3\n 4\n-5\n+five\n")
        );
        let created = unified("", "a\nb\n");
        assert_eq!(created.text, "@@ -0,0 +1,2 @@\n+a\n+b\n");
    }
}
