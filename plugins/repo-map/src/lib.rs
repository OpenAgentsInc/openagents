//! A bounded structural map of a repository, as a `packet-v0` guest plugin.
//!
//! The map half of OpenAgentsInc/openagents#44: given a read-only mount
//! over the session's workspace (mount 0, declared as `${workspace}` in the
//! manifest and resolved by the host at load time), walk the tree and answer
//! one of three questions:
//!
//! - **Outline** (the default): which symbols does each source file appear
//!   to define? `{files: [{path, language, symbols}], …}`
//! - **Definition lookup** (`symbol`): where is a symbol with exactly that
//!   name defined? `{definitions: [{path, kind, line}], …}`
//! - **References** (`symbol` + `count_references`): how often is that name
//!   used, per file, definitions excluded? `{references: [{path, count}], …}`
//!
//! Heuristic is the honest word for the extraction: symbols are found by
//! line pattern — `def`/`class`, `fn`/`struct`/`trait`/`impl`,
//! `defmodule`/`defp`, `func`/`type`, and so on — not by parsing. A string
//! or comment that looks like a definition is counted as one; a definition
//! split across lines or built by a macro is missed. A real parse
//! (tree-sitter compiled into the guest) is the recorded follow-up; the
//! C-runtime-on-`wasm32-unknown-unknown` toolchain fight it requires is
//! deliberately not taken on in this iteration.
//!
//! The walk skips a fixed list of directories by name — `.git`,
//! `node_modules`, `_build`, `deps`, `target`, `.elixir_ls`, `dist`,
//! `build` — and nothing else; it does not parse `.gitignore` (that is
//! `repo_tree`'s job). The posture is the plugin family's: read-only
//! through the host's confined capability imports, bounded everywhere
//! (directories listed, files visited, bytes per file, symbols per file),
//! fail-soft on unreadable files, and honest about truncation.

use std::collections::{BTreeSet, VecDeque};

use openagents_pdk::{
    list_mounted_dir, plugin_entry, read_mounted_file, MountDirListing, Refusal, RefusalCode,
};
use serde::{Deserialize, Serialize};

/// Mount index of the workspace, fixed by the manifest's mount order.
const WORKSPACE_MOUNT: u32 = 0;
/// Per-file byte bound; a larger file is listed with `symbols: null` and
/// counted oversized rather than read.
const MAX_FILE_BYTES: u64 = 262_144;
const DEFAULT_MAX_FILES: usize = 500;
const FILE_CAP: usize = 2_000;
const DEFAULT_MAX_SYMBOLS: usize = 100;
const SYMBOL_CAP: usize = 400;
/// Directory listings per invocation.
const MAX_DIR_LISTS: usize = 2_000;
/// Directories never descended into, by name. A fixed list, documented in
/// the crate doc; gitignore semantics live in `repo_tree`, not here.
const SKIP_DIRS: [&str; 8] = [
    ".git",
    "node_modules",
    "_build",
    "deps",
    "target",
    ".elixir_ls",
    "dist",
    "build",
];

#[derive(Deserialize)]
pub struct Input {
    /// Subtree to map, relative to the workspace root. The whole workspace
    /// when absent.
    #[serde(default)]
    pub path: Option<String>,
    /// Exact symbol name to look up. Alone: definition lookup. With
    /// `count_references`: per-file reference counts.
    #[serde(default)]
    pub symbol: Option<String>,
    /// With `symbol`: count whole-word uses per file instead of listing
    /// definitions.
    #[serde(default)]
    pub count_references: Option<bool>,
    /// Most source files to visit. Default 500, capped at 2000.
    #[serde(default)]
    pub max_files: Option<usize>,
    /// Most symbols to report per file. Default 100, capped at 400.
    #[serde(default)]
    pub max_symbols_per_file: Option<usize>,
}

/// One extracted symbol: what the line pattern says it is, and where.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Symbol {
    pub kind: &'static str,
    pub name: String,
    /// 1-based line number.
    pub line: usize,
    /// Best guess at the enclosing class, for indented Python methods.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileOutline {
    pub path: String,
    pub language: &'static str,
    /// `null` when the file was over the per-file byte bound, so its
    /// symbols are unknown rather than absent.
    pub symbols: Option<Vec<Symbol>>,
    /// True when the per-file symbol cap dropped later symbols.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub symbols_truncated: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Definition {
    pub path: String,
    pub kind: &'static str,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Reference {
    pub path: String,
    /// Whole-word occurrences, definition lines excluded.
    pub count: usize,
}

/// Shared walk accounting, flattened into every output shape.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct WalkStats {
    /// Source files with a mapped extension the walk visited.
    pub files_seen: usize,
    /// Files whose bytes were read and scanned.
    pub files_parsed: usize,
    /// Files past the per-file byte bound, listed but not read.
    pub oversized: usize,
    /// Files the host refused to read for any reason but size.
    pub unreadable: usize,
    /// True when any bound cut the picture short: the file budget, the
    /// listing budget, or a truncated directory listing.
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct OutlineOutput {
    pub files: Vec<FileOutline>,
    #[serde(flatten)]
    pub stats: WalkStats,
}

#[derive(Debug, Serialize)]
pub struct DefinitionsOutput {
    pub symbol: String,
    pub definitions: Vec<Definition>,
    #[serde(flatten)]
    pub stats: WalkStats,
}

#[derive(Debug, Serialize)]
pub struct ReferencesOutput {
    pub symbol: String,
    pub references: Vec<Reference>,
    pub total: usize,
    #[serde(flatten)]
    pub stats: WalkStats,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Output {
    Outline(OutlineOutput),
    Definitions(DefinitionsOutput),
    References(ReferencesOutput),
}

/// The two host capabilities the mapper uses, as a seam so the logic runs
/// under `cargo test` against a fake host as well as inside the sandbox.
pub trait Host {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal>;
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal>;
}

struct RealHost;

impl Host for RealHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        list_mounted_dir(mount_index, path)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        read_mounted_file(path)
    }
}

/// The languages the line patterns know, keyed by file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Python,
    TypeScript,
    JavaScript,
    Rust,
    Elixir,
    Go,
    Ruby,
}

impl Language {
    fn of(file_name: &str) -> Option<Language> {
        let (_, ext) = file_name.rsplit_once('.')?;
        match ext {
            "py" => Some(Language::Python),
            "ts" | "tsx" => Some(Language::TypeScript),
            "js" | "jsx" => Some(Language::JavaScript),
            "rs" => Some(Language::Rust),
            "ex" | "exs" => Some(Language::Elixir),
            "go" => Some(Language::Go),
            "rb" => Some(Language::Ruby),
            _ => None,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Rust => "rust",
            Language::Elixir => "elixir",
            Language::Go => "go",
            Language::Ruby => "ruby",
        }
    }
}

/// One file the walk found, before its bytes are inspected.
struct Candidate {
    path: String,
    language: Language,
    size: u64,
}

/// The whole map, over any [`Host`].
pub fn map(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    let max_files = input.max_files.unwrap_or(DEFAULT_MAX_FILES).clamp(1, FILE_CAP);
    let max_symbols = input
        .max_symbols_per_file
        .unwrap_or(DEFAULT_MAX_SYMBOLS)
        .clamp(1, SYMBOL_CAP);
    let symbol = match &input.symbol {
        Some(name) if name.trim().is_empty() => {
            return Err(Refusal::unsupported("an empty symbol names nothing to find"))
        }
        Some(name) => Some(name.trim().to_string()),
        None => None,
    };
    let count_references = input.count_references.unwrap_or(false);
    if count_references && symbol.is_none() {
        return Err(Refusal::unsupported(
            "count_references needs a symbol to count; pass one",
        ));
    }
    let root = input
        .path
        .as_deref()
        .unwrap_or("")
        .trim_matches('/')
        .to_string();

    let (candidates, mut stats) = walk(host, &root, max_files)?;

    match (symbol, count_references) {
        (None, _) => {
            let files = outline(host, &candidates, max_symbols, &mut stats);
            Ok(Output::Outline(OutlineOutput { files, stats }))
        }
        (Some(name), false) => {
            let definitions = definitions(host, &candidates, &name, max_symbols, &mut stats);
            Ok(Output::Definitions(DefinitionsOutput { symbol: name, definitions, stats }))
        }
        (Some(name), true) => {
            let (references, total) =
                references(host, &candidates, &name, max_symbols, &mut stats);
            Ok(Output::References(ReferencesOutput { symbol: name, references, total, stats }))
        }
    }
}

/// Breadth-first walk from `root`, collecting source files up to the file
/// budget. Directories on the skip list are never entered; symlinks are
/// never followed (the host would refuse anyway). Listing failures below
/// the root fail soft; a root that cannot be listed is a refusal, because
/// the caller named it.
fn walk(
    host: &dyn Host,
    root: &str,
    max_files: usize,
) -> Result<(Vec<Candidate>, WalkStats), Refusal> {
    let mut stats = WalkStats::default();
    let mut files: Vec<Candidate> = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::from([root.to_string()]);
    let mut lists = 0usize;
    let mut at_root = true;

    'walk: while let Some(dir) = queue.pop_front() {
        if lists >= MAX_DIR_LISTS {
            stats.truncated = true;
            break;
        }
        lists += 1;
        let listing = match host.list(WORKSPACE_MOUNT, &dir) {
            Ok(listing) => listing,
            Err(refusal) if at_root => {
                let shown = if dir.is_empty() { "." } else { dir.as_str() };
                return Err(Refusal::new(
                    RefusalCode::FileUnreadable,
                    format!("cannot list `{shown}` under the workspace mount: {}", refusal.reason),
                ));
            }
            Err(_) => continue,
        };
        at_root = false;
        if listing.truncated {
            stats.truncated = true;
        }
        for entry in &listing.entries {
            match entry.kind.as_str() {
                "dir" => {
                    if !SKIP_DIRS.contains(&entry.name.as_str()) {
                        queue.push_back(join(&dir, &entry.name));
                    }
                }
                "file" => {
                    let Some(language) = Language::of(&entry.name) else {
                        continue;
                    };
                    if files.len() >= max_files {
                        stats.truncated = true;
                        break 'walk;
                    }
                    stats.files_seen += 1;
                    files.push(Candidate {
                        path: join(&dir, &entry.name),
                        language,
                        size: entry.size,
                    });
                }
                _ => {}
            }
        }
    }
    Ok((files, stats))
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// A candidate's text, or `None` with the stats told why.
fn load(host: &dyn Host, candidate: &Candidate, stats: &mut WalkStats) -> Option<String> {
    if candidate.size > MAX_FILE_BYTES {
        stats.oversized += 1;
        return None;
    }
    match host.read(&candidate.path) {
        Ok(bytes) => {
            stats.files_parsed += 1;
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
        Err(refusal) if refusal.code == RefusalCode::FileTooLarge => {
            stats.oversized += 1;
            None
        }
        Err(_) => {
            stats.unreadable += 1;
            None
        }
    }
}

fn outline(
    host: &dyn Host,
    candidates: &[Candidate],
    max_symbols: usize,
    stats: &mut WalkStats,
) -> Vec<FileOutline> {
    let mut files = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let before_oversized = stats.oversized;
        let Some(text) = load(host, candidate, stats) else {
            // An oversized file is still part of the map, with its symbols
            // honestly unknown; an unreadable one is only counted.
            if stats.oversized > before_oversized {
                files.push(FileOutline {
                    path: candidate.path.clone(),
                    language: candidate.language.as_str(),
                    symbols: None,
                    symbols_truncated: false,
                });
            }
            continue;
        };
        let (symbols, symbols_truncated) = extract_symbols(candidate.language, &text, max_symbols);
        files.push(FileOutline {
            path: candidate.path.clone(),
            language: candidate.language.as_str(),
            symbols: Some(symbols),
            symbols_truncated,
        });
    }
    files
}

fn definitions(
    host: &dyn Host,
    candidates: &[Candidate],
    symbol: &str,
    max_symbols: usize,
    stats: &mut WalkStats,
) -> Vec<Definition> {
    let mut definitions = Vec::new();
    for candidate in candidates {
        let Some(text) = load(host, candidate, stats) else {
            continue;
        };
        let (symbols, _) = extract_symbols(candidate.language, &text, max_symbols);
        for found in symbols {
            if found.name == symbol {
                definitions.push(Definition {
                    path: candidate.path.clone(),
                    kind: found.kind,
                    line: found.line,
                    parent: found.parent,
                });
            }
        }
    }
    definitions
}

fn references(
    host: &dyn Host,
    candidates: &[Candidate],
    symbol: &str,
    max_symbols: usize,
    stats: &mut WalkStats,
) -> (Vec<Reference>, usize) {
    let mut references = Vec::new();
    let mut total = 0usize;
    for candidate in candidates {
        let Some(text) = load(host, candidate, stats) else {
            continue;
        };
        let (symbols, _) = extract_symbols(candidate.language, &text, max_symbols);
        let definition_lines: BTreeSet<usize> = symbols
            .iter()
            .filter(|s| s.name == symbol)
            .map(|s| s.line)
            .collect();
        let mut count = 0usize;
        for (index, line) in text.lines().enumerate() {
            if definition_lines.contains(&(index + 1)) {
                continue;
            }
            count += word_count(line, symbol);
        }
        if count > 0 {
            total += count;
            references.push(Reference { path: candidate.path.clone(), count });
        }
    }
    references.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    (references, total)
}

/// Whole-word occurrences of `needle` in `text`: a match whose neighbors
/// are not identifier characters, so `foo` never matches inside `foobar`
/// or `foo_bar`.
fn word_count(text: &str, needle: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0usize;
    for (position, _) in text.match_indices(needle) {
        let before_ok = position == 0 || !is_word_byte(bytes[position - 1]);
        let end = position + needle.len();
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            count += 1;
        }
    }
    count
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The heuristic line-based extraction: at most `max` symbols, plus a flag
/// saying whether more existed.
pub fn extract_symbols(language: Language, text: &str, max: usize) -> (Vec<Symbol>, bool) {
    let mut symbols = Vec::new();
    let mut truncated = false;
    // Python only: (indent, class name) of every enclosing class candidate.
    let mut class_stack: Vec<(usize, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let found = match language {
            Language::Python => python_symbol(raw, &mut class_stack),
            Language::TypeScript | Language::JavaScript => ts_symbol(raw),
            Language::Rust => rust_symbol(raw),
            Language::Elixir => elixir_symbol(raw),
            Language::Go => go_symbol(raw),
            Language::Ruby => ruby_symbol(raw),
        };
        if let Some((kind, name, parent)) = found {
            if symbols.len() >= max {
                truncated = true;
                break;
            }
            symbols.push(Symbol { kind, name, line: index + 1, parent });
        }
    }
    (symbols, truncated)
}

type Found = Option<(&'static str, String, Option<String>)>;

/// A leading identifier: `[A-Za-z_][A-Za-z0-9_]*` plus any `extra` characters.
fn ident_with(text: &str, extra: &[char]) -> Option<String> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut name = String::new();
    name.push(first);
    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' || extra.contains(&c) {
            name.push(c);
        } else {
            break;
        }
    }
    Some(name)
}

fn ident(text: &str) -> Option<String> {
    ident_with(text, &[])
}

fn python_symbol(raw: &str, class_stack: &mut Vec<(usize, String)>) -> Found {
    let trimmed = raw.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let indent = raw.len() - trimmed.len();
    if let Some(rest) = trimmed.strip_prefix("class ") {
        while class_stack.last().is_some_and(|(depth, _)| *depth >= indent) {
            class_stack.pop();
        }
        let name = ident(rest)?;
        let parent = class_stack.last().map(|(_, parent)| parent.clone());
        class_stack.push((indent, name.clone()));
        return Some(("class", name, parent));
    }
    let def = trimmed
        .strip_prefix("def ")
        .or_else(|| trimmed.strip_prefix("async def "));
    if let Some(rest) = def {
        while class_stack.last().is_some_and(|(depth, _)| *depth >= indent) {
            class_stack.pop();
        }
        let name = ident(rest)?;
        return match class_stack.last() {
            Some((_, parent)) => Some(("method", name, Some(parent.clone()))),
            None => Some(("function", name, None)),
        };
    }
    None
}

fn ts_symbol(raw: &str) -> Found {
    let mut s = raw.trim_start();
    loop {
        let mut stripped = false;
        for prefix in ["export ", "default ", "declare ", "abstract "] {
            if let Some(rest) = s.strip_prefix(prefix) {
                s = rest.trim_start();
                stripped = true;
            }
        }
        if !stripped {
            break;
        }
    }
    if let Some(rest) = s.strip_prefix("async ") {
        let rest = rest.trim_start();
        if rest.starts_with("function") {
            s = rest;
        }
    }
    if let Some(rest) = s.strip_prefix("function") {
        if !(rest.starts_with(' ') || rest.starts_with('*')) {
            return None;
        }
        let rest = rest.trim_start().trim_start_matches('*').trim_start();
        return ident(rest).map(|name| ("function", name, None));
    }
    if let Some(rest) = s.strip_prefix("class ") {
        return ident(rest).map(|name| ("class", name, None));
    }
    if let Some(rest) = s.strip_prefix("interface ") {
        return ident(rest).map(|name| ("interface", name, None));
    }
    if let Some(rest) = s
        .strip_prefix("const enum ")
        .or_else(|| s.strip_prefix("enum "))
    {
        return ident(rest).map(|name| ("enum", name, None));
    }
    if let Some(rest) = s.strip_prefix("type ") {
        let name = ident(rest)?;
        let after = rest[name.len()..].trim_start();
        if after.starts_with('=') || after.starts_with('<') {
            return Some(("type", name, None));
        }
        return None;
    }
    for keyword in ["const ", "let ", "var "] {
        if let Some(rest) = s.strip_prefix(keyword) {
            let name = ident(rest)?;
            let after = &rest[name.len()..];
            // Only bindings that hold a function: an arrow or a function
            // expression on the same line. Plain values are data, not map.
            if after.contains('=') && (after.contains("=>") || after.contains("function")) {
                return Some(("function", name, None));
            }
            return None;
        }
    }
    None
}

/// Strip a Rust visibility prefix: `pub `, `pub(crate) `, `pub(in …) `.
fn strip_rust_visibility(s: &str) -> &str {
    let Some(rest) = s.strip_prefix("pub") else {
        return s;
    };
    if let Some(after_paren) = rest.strip_prefix('(') {
        match after_paren.find(')') {
            Some(close) => return after_paren[close + 1..].trim_start(),
            None => return s,
        }
    }
    if rest.starts_with(' ') {
        return rest.trim_start();
    }
    s
}

fn rust_symbol(raw: &str) -> Found {
    let mut s = strip_rust_visibility(raw.trim_start());
    loop {
        let mut stripped = false;
        for prefix in ["async ", "unsafe ", "default "] {
            if let Some(rest) = s.strip_prefix(prefix) {
                s = rest.trim_start();
                stripped = true;
            }
        }
        if !stripped {
            break;
        }
    }
    if let Some(rest) = s
        .strip_prefix("fn ")
        .or_else(|| s.strip_prefix("const fn "))
    {
        return ident(rest).map(|name| ("function", name, None));
    }
    if let Some(rest) = s.strip_prefix("struct ") {
        return ident(rest).map(|name| ("struct", name, None));
    }
    if let Some(rest) = s.strip_prefix("enum ") {
        return ident(rest).map(|name| ("enum", name, None));
    }
    if let Some(rest) = s.strip_prefix("trait ") {
        return ident(rest).map(|name| ("trait", name, None));
    }
    if let Some(rest) = s.strip_prefix("mod ") {
        return ident(rest).map(|name| ("module", name, None));
    }
    if let Some(rest) = s.strip_prefix("impl") {
        if !(rest.starts_with(' ') || rest.starts_with('<')) {
            return None;
        }
        let rest = if rest.starts_with('<') {
            let mut depth = 0i32;
            let mut close = None;
            for (i, c) in rest.char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            &rest[close? + 1..]
        } else {
            rest
        };
        let name = rest.split('{').next().unwrap_or(rest);
        let name = name.split(" where").next().unwrap_or(name).trim();
        if name.is_empty() {
            return None;
        }
        return Some(("impl", name.to_string(), None));
    }
    if let Some(rest) = s
        .strip_prefix("const ")
        .or_else(|| s.strip_prefix("static "))
    {
        return ident(rest).map(|name| ("const", name, None));
    }
    None
}

fn elixir_symbol(raw: &str) -> Found {
    let s = raw.trim_start();
    if let Some(rest) = s.strip_prefix("defmodule ") {
        return ident_with(rest, &['.']).map(|name| ("module", name, None));
    }
    if let Some(rest) = s
        .strip_prefix("defmacrop ")
        .or_else(|| s.strip_prefix("defmacro "))
    {
        return ident_with(rest, &['?', '!']).map(|name| ("macro", name, None));
    }
    if let Some(rest) = s.strip_prefix("defp ") {
        return ident_with(rest, &['?', '!']).map(|name| ("private_function", name, None));
    }
    if let Some(rest) = s.strip_prefix("def ") {
        return ident_with(rest, &['?', '!']).map(|name| ("function", name, None));
    }
    None
}

fn go_symbol(raw: &str) -> Found {
    let s = raw.trim_start();
    if let Some(rest) = s.strip_prefix("func (") {
        let (_, after) = rest.split_once(')')?;
        return ident(after.trim_start()).map(|name| ("method", name, None));
    }
    if let Some(rest) = s.strip_prefix("func ") {
        return ident(rest).map(|name| ("function", name, None));
    }
    if let Some(rest) = s.strip_prefix("type ") {
        return ident(rest).map(|name| ("type", name, None));
    }
    None
}

fn ruby_symbol(raw: &str) -> Found {
    let s = raw.trim_start();
    if let Some(rest) = s.strip_prefix("def ") {
        let rest = rest.strip_prefix("self.").unwrap_or(rest);
        return ident_with(rest, &['?', '!']).map(|name| ("function", name, None));
    }
    if let Some(rest) = s.strip_prefix("class ") {
        if rest.starts_with("<<") {
            return None;
        }
        return ident(rest).map(|name| ("class", name, None));
    }
    if let Some(rest) = s.strip_prefix("module ") {
        return ident(rest).map(|name| ("module", name, None));
    }
    None
}

fn handle(input: Input) -> Result<Output, Refusal> {
    map(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
