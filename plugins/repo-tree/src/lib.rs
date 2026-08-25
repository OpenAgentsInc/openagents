//! `repo_tree`: the workspace's file tree and a fuzzy file-name lookup, as
//! a `packet-v0` guest plugin (OpenAgentsInc/openagents#43).
//!
//! Mount 0 is the workspace — the manifest declares it as the literal
//! `${workspace}`, which the host resolves to its working directory at load
//! time (OpenAgentsInc/openagents#44). One tool, two modes:
//!
//! - **Tree mode** (the default): walk the workspace depth-first and return
//!   its entries — path, kind, size — honoring `.gitignore` files at the
//!   root and in nested directories, bounded by depth and entry ceilings.
//! - **Query mode** (when `query` is present): a case-insensitive
//!   subsequence match of the query against each file path, ranked by match
//!   tightness, returned instead of the tree.
//!
//! ## The gitignore subset, honestly
//!
//! This is a deliberate subset of gitignore matching, not the whole
//! grammar:
//!
//! - Blank lines and `#` comments are skipped.
//! - A trailing `/` makes the pattern match directories only.
//! - A pattern containing a `/` (leading or interior) is anchored to the
//!   directory holding its `.gitignore`; a pattern without one matches the
//!   entry's basename at any depth below it.
//! - `*` matches within a path segment; a `**` segment matches zero or
//!   more whole segments. No other wildcard (`?`, `[...]`, escapes) is
//!   special — those characters match literally.
//! - Negation is not implemented: `!` lines are ignored and counted in the
//!   output as `ignored_negations`, so a caller can see when the subset
//!   fell short.
//! - An ignored directory is skipped wholesale, so nothing under it is
//!   walked, matched, or counted.
//!
//! `.git` is skipped unconditionally, gitignored or not. Everything else —
//! `node_modules`, `_build`, `deps`, `target` — is skipped purely by
//! gitignore rules.
//!
//! ## Bounds
//!
//! At most [`LISTING_BOUND`] directory listings per invocation and
//! [`HELD_BOUND`] entries held; hitting either fails soft and sets
//! `truncated`. An unreadable nested directory is skipped rather than
//! failing the walk. Symlinks are reported by the host but never followed,
//! and entries of kind `symlink` or `other` are left out of the tree.

use openagents_pdk::{list_mounted_dir, plugin_entry, read_mounted_file, MountDirListing, Refusal};
use serde::{Deserialize, Serialize};

/// The workspace is the manifest's one mount.
const WORKSPACE_MOUNT: u32 = 0;
/// Deepest entry level in tree mode when the input names none.
const DEFAULT_MAX_DEPTH: usize = 4;
/// Depth ceiling for both modes; query mode defaults to it.
const DEPTH_CAP: usize = 8;
const DEFAULT_MAX_ENTRIES: usize = 200;
const ENTRY_CAP: usize = 500;
const DEFAULT_QUERY_LIMIT: usize = 10;
const QUERY_LIMIT_CAP: usize = 25;
/// Most directory listings one invocation may ask the host for.
pub const LISTING_BOUND: usize = 2_000;
/// Most entries one invocation holds before the walk stops.
pub const HELD_BOUND: usize = 10_000;

#[derive(Debug, Default, Deserialize)]
pub struct Input {
    /// Subtree to walk, relative to the workspace root. The root when absent.
    #[serde(default)]
    pub path: Option<String>,
    /// Deepest entry level to return. Default 4 (8 in query mode), capped at 8.
    #[serde(default)]
    pub max_depth: Option<usize>,
    /// Most tree entries to return. Default 200, capped at 500.
    #[serde(default)]
    pub max_entries: Option<usize>,
    /// Fuzzy file-name lookup; when present, matches replace the tree.
    #[serde(default)]
    pub query: Option<String>,
    /// Most query matches to return. Default 10, capped at 25.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Entry {
    /// Path relative to the workspace root.
    pub path: String,
    /// `"file"` or `"dir"`.
    pub kind: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct TreeOutput {
    pub entries: Vec<Entry>,
    /// Entries examined before the walk finished or stopped.
    pub total_seen: usize,
    /// True when a ceiling — entries, listings, held, or a host listing
    /// bound — cut the walk short.
    pub truncated: bool,
    /// `!` gitignore lines this subset ignored rather than honored.
    pub ignored_negations: usize,
    /// Entries the gitignore rules dropped (each skipped directory counts
    /// once; nothing under it is walked or counted).
    pub skipped_gitignored: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Match {
    /// Path relative to the workspace root.
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct QueryOutput {
    pub matches: Vec<Match>,
    /// Files tested against the query.
    pub searched: usize,
    /// True when the walk was cut short or more matches exist than `limit`.
    pub truncated: bool,
}

/// One output type, two shapes; the mode picks which.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Output {
    Tree(TreeOutput),
    Query(QueryOutput),
}

/// The host capabilities this plugin uses, as a seam the tests fake.
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

/// The whole tool, over any [`Host`].
pub fn repo_tree(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    let start = normalize_start(input.path.as_deref());
    match input.query.as_deref().map(str::trim) {
        Some(query) if !query.is_empty() => {
            Ok(Output::Query(query_mode(host, &start, query, input)?))
        }
        _ => Ok(Output::Tree(tree_mode(host, &start, input)?)),
    }
}

fn tree_mode(host: &dyn Host, start: &str, input: &Input) -> Result<TreeOutput, Refusal> {
    let max_entries = input
        .max_entries
        .unwrap_or(DEFAULT_MAX_ENTRIES)
        .clamp(1, ENTRY_CAP);
    let mut walk = Walk::new(host, input.max_depth.unwrap_or(DEFAULT_MAX_DEPTH));
    walk.load_ancestors(start);
    let mut entries: Vec<Entry> = Vec::new();
    walk.walk(start, 0, &mut |path, kind, size| {
        if entries.len() >= max_entries {
            return false;
        }
        entries.push(Entry {
            path,
            kind: kind.to_string(),
            size,
        });
        true
    })?;
    Ok(TreeOutput {
        entries,
        total_seen: walk.total_seen,
        truncated: walk.truncated,
        ignored_negations: walk.ignored_negations,
        skipped_gitignored: walk.skipped_gitignored,
    })
}

fn query_mode(
    host: &dyn Host,
    start: &str,
    query: &str,
    input: &Input,
) -> Result<QueryOutput, Refusal> {
    let limit = input
        .limit
        .unwrap_or(DEFAULT_QUERY_LIMIT)
        .clamp(1, QUERY_LIMIT_CAP);
    // Lookup wants the whole tree; without an explicit depth it walks to
    // the cap rather than tree mode's shallower default.
    let mut walk = Walk::new(host, input.max_depth.unwrap_or(DEPTH_CAP));
    walk.load_ancestors(start);
    let mut files: Vec<(String, u64)> = Vec::new();
    walk.walk(start, 0, &mut |path, kind, size| {
        if kind != "file" {
            return true;
        }
        if files.len() >= HELD_BOUND {
            return false;
        }
        files.push((path, size));
        true
    })?;

    let query_lower: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    let searched = files.len();
    let mut scored: Vec<(Score, Match)> = files
        .into_iter()
        .filter_map(|(path, size)| score(&query_lower, &path).map(|s| (s, Match { path, size })))
        .collect();
    scored.sort_by(|(a, am), (b, bm)| {
        a.span
            .cmp(&b.span)
            .then(b.boundaries.cmp(&a.boundaries))
            .then(am.path.len().cmp(&bm.path.len()))
            .then(am.path.cmp(&bm.path))
    });
    let found = scored.len();
    let matches: Vec<Match> = scored.into_iter().take(limit).map(|(_, m)| m).collect();
    Ok(QueryOutput {
        matches,
        searched,
        truncated: walk.truncated || found > limit,
    })
}

// ---------------------------------------------------------------------------
// The walk

/// A visitor: entry path, kind, size; `false` stops the whole walk.
type Sink<'s> = dyn FnMut(String, &str, u64) -> bool + 's;

struct Walk<'a> {
    host: &'a dyn Host,
    /// Deepest entry level to report; the walk's root's children are level 1.
    max_depth: usize,
    listings: usize,
    truncated: bool,
    total_seen: usize,
    skipped_gitignored: usize,
    ignored_negations: usize,
    rule_sets: Vec<RuleSet>,
}

impl<'a> Walk<'a> {
    fn new(host: &'a dyn Host, max_depth: usize) -> Self {
        Walk {
            host,
            max_depth: max_depth.clamp(1, DEPTH_CAP),
            listings: 0,
            truncated: false,
            total_seen: 0,
            skipped_gitignored: 0,
            ignored_negations: 0,
            rule_sets: Vec::new(),
        }
    }

    /// Rules from `.gitignore` files above the walk's start, so a subtree
    /// walk still honors the root's ignores. The start's own `.gitignore`
    /// is loaded by the walk itself.
    fn load_ancestors(&mut self, start: &str) {
        if start.is_empty() {
            return;
        }
        self.push_gitignore("");
        let mut base = String::new();
        let components: Vec<&str> = start.split('/').collect();
        for component in &components[..components.len() - 1] {
            base = join(&base, component);
            self.push_gitignore(&base);
        }
    }

    /// Read and push this directory's `.gitignore`, if it has one. Returns
    /// whether a rule set was pushed, so the caller can pop symmetrically.
    fn push_gitignore(&mut self, dir: &str) -> bool {
        match self.host.read(&join(dir, ".gitignore")) {
            Ok(bytes) => {
                let (rules, negations) = parse_gitignore(&bytes);
                self.ignored_negations += negations;
                self.rule_sets.push(RuleSet {
                    base: dir.to_string(),
                    rules,
                });
                true
            }
            // Absent or unreadable is the same answer: no rules here.
            Err(_) => false,
        }
    }

    fn ignored(&self, path: &str, is_dir: bool) -> bool {
        self.rule_sets.iter().any(|set| {
            let rel = if set.base.is_empty() {
                Some(path)
            } else {
                path.strip_prefix(set.base.as_str())
                    .and_then(|rest| rest.strip_prefix('/'))
            };
            rel.is_some_and(|rel| set.rules.iter().any(|rule| rule.matches(rel, is_dir)))
        })
    }

    /// Depth-first over `dir` (whose own depth is `depth`; its children are
    /// `depth + 1`). Returns whether the walk should keep going.
    fn walk(&mut self, dir: &str, depth: usize, sink: &mut Sink) -> Result<bool, Refusal> {
        if self.listings >= LISTING_BOUND {
            self.truncated = true;
            return Ok(true);
        }
        self.listings += 1;
        let listing = match self.host.list(WORKSPACE_MOUNT, dir) {
            Ok(listing) => listing,
            // The walk's own root must exist; a nested directory that
            // refuses to list is skipped, fail-soft.
            Err(refusal) if depth == 0 => return Err(refusal),
            Err(_) => return Ok(true),
        };
        if listing.truncated {
            self.truncated = true;
        }
        let pushed = self.push_gitignore(dir);
        let mut keep_going = true;
        for entry in &listing.entries {
            // `.git` is skipped unconditionally, never walked, never counted.
            if entry.name == ".git" {
                continue;
            }
            // Symlinks are reported but never followed; neither shape fits
            // the tree.
            if entry.kind != "file" && entry.kind != "dir" {
                continue;
            }
            self.total_seen += 1;
            let path = join(dir, &entry.name);
            if self.ignored(&path, entry.kind == "dir") {
                self.skipped_gitignored += 1;
                continue;
            }
            if !sink(path.clone(), &entry.kind, entry.size) {
                self.truncated = true;
                keep_going = false;
                break;
            }
            if entry.kind == "dir" && depth + 1 < self.max_depth {
                if !self.walk(&path, depth + 1, sink)? {
                    keep_going = false;
                    break;
                }
            }
        }
        if pushed {
            self.rule_sets.pop();
        }
        Ok(keep_going)
    }
}

fn normalize_start(path: Option<&str>) -> String {
    let mut trimmed = path.unwrap_or("").trim();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    let trimmed = trimmed.trim_matches('/');
    if trimmed == "." {
        String::new()
    } else {
        trimmed.to_string()
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

// ---------------------------------------------------------------------------
// The gitignore subset

struct RuleSet {
    /// Directory holding the `.gitignore`, relative to the workspace root.
    base: String,
    rules: Vec<Rule>,
}

struct Rule {
    /// Pattern split on `/`; a lone `**` segment crosses segments.
    segments: Vec<String>,
    /// Trailing `/`: the pattern matches directories only.
    dir_only: bool,
    /// The pattern contained a `/`, so it matches relative to its
    /// `.gitignore`'s directory; otherwise it matches basenames anywhere
    /// below it.
    anchored: bool,
}

impl Rule {
    /// Does this rule match `rel` (relative to the rule's base)?
    fn matches(&self, rel: &str, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }
        let path: Vec<&str> = rel.split('/').collect();
        if self.anchored {
            glob_path(&self.segments, &path)
        } else {
            path.last()
                .is_some_and(|name| glob_segment(&self.segments[0], name))
        }
    }
}

/// Parse one `.gitignore`'s bytes into the subset's rules, plus how many
/// `!` negation lines were ignored.
fn parse_gitignore(bytes: &[u8]) -> (Vec<Rule>, usize) {
    let text = String::from_utf8_lossy(bytes);
    let mut rules = Vec::new();
    let mut negations = 0usize;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('!') {
            negations += 1;
            continue;
        }
        let (body, dir_only) = match line.strip_suffix('/') {
            Some(body) => (body, true),
            None => (line, false),
        };
        let (body, leading_slash) = match body.strip_prefix('/') {
            Some(body) => (body, true),
            None => (body, false),
        };
        if body.is_empty() {
            continue;
        }
        let anchored = leading_slash || body.contains('/');
        let segments: Vec<String> = body
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if segments.is_empty() {
            continue;
        }
        rules.push(Rule {
            segments,
            dir_only,
            anchored,
        });
    }
    (rules, negations)
}

/// Match pattern segments against path segments; a `**` pattern segment
/// matches zero or more whole path segments.
fn glob_path(pattern: &[String], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((first, rest)) if first == "**" => {
            (0..=path.len()).any(|skip| glob_path(rest, &path[skip..]))
        }
        Some((first, rest)) => path
            .split_first()
            .is_some_and(|(segment, more)| glob_segment(first, segment) && glob_path(rest, more)),
    }
}

/// Match one pattern segment against one name; `*` matches any run of
/// characters within the segment, everything else is literal.
fn glob_segment(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    segment_match(&pattern, &name)
}

fn segment_match(pattern: &[char], name: &[char]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some(('*', rest)) => {
            // Collapse star runs so `**` inside a segment is one `*`.
            let rest = if rest.first() == Some(&'*') {
                &rest[1..]
            } else {
                rest
            };
            (0..=name.len()).any(|skip| segment_match(rest, &name[skip..]))
        }
        Some((ch, rest)) => name
            .split_first()
            .is_some_and(|(first, more)| first == ch && segment_match(rest, more)),
    }
}

// ---------------------------------------------------------------------------
// Query scoring

/// How well a path matched: a shorter span is tighter, and more matched
/// characters sitting on segment boundaries (start of the path, or after
/// `/`, `.`, `_`, `-`) is better on a tie. That is the whole formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub span: usize,
    pub boundaries: usize,
}

/// The best case-insensitive subsequence match of `query` in `path`, or
/// `None`. Each occurrence of the query's first character starts one greedy
/// forward match; the tightest wins. Greedy interior choices are not
/// globally optimal, which is a documented simplification.
pub fn score(query_lower: &[char], path: &str) -> Option<Score> {
    let lower: Vec<char> = path.chars().map(|c| c.to_ascii_lowercase()).collect();
    let is_boundary = |at: usize| at == 0 || matches!(lower[at - 1], '/' | '.' | '_' | '-');
    let first = *query_lower.first()?;
    let mut best: Option<Score> = None;
    for start in 0..lower.len() {
        if lower[start] != first {
            continue;
        }
        let mut boundaries = usize::from(is_boundary(start));
        let mut at = start;
        let mut complete = true;
        for &wanted in &query_lower[1..] {
            match (at + 1..lower.len()).find(|&i| lower[i] == wanted) {
                Some(found) => {
                    at = found;
                    boundaries += usize::from(is_boundary(found));
                }
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if !complete {
            // Greedy from here used the earliest possible positions; a
            // later start has strictly less to work with.
            break;
        }
        let candidate = Score {
            span: at - start + 1,
            boundaries,
        };
        best = Some(match best {
            Some(held)
                if held.span < candidate.span
                    || (held.span == candidate.span && held.boundaries >= candidate.boundaries) =>
            {
                held
            }
            _ => candidate,
        });
    }
    best
}

fn handle(input: Input) -> Result<Output, Refusal> {
    repo_tree(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
