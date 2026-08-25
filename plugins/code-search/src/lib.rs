//! `code_search`: bounded literal and regex search over the workspace, as a
//! `packet-v0` guest plugin (OpenAgentsInc/openagents#50).
//!
//! Mount 0 is the workspace — the manifest declares it as the literal
//! `${workspace}`, which the host resolves to its working directory at load
//! time, exactly as `repo_tree` and `repo_map` do. The coder shells out to
//! grep or rg for this today; sealing the search behind the plugin boundary
//! makes it replayable — the same tree and the same query always produce the
//! same output — and makes its bounds visible in the manifest instead of
//! buried in a shell flag.
//!
//! One tool, two matchers:
//!
//! - **Literal** (the default): a substring match of `pattern` against each
//!   line.
//! - **Regex** (`regex: true`): a documented subset of regular expressions,
//!   matched per line. The subset is: literal characters; `.` for any
//!   character; character classes `[...]` with ranges and leading-`^`
//!   negation; the escapes `\d`, `\w`, `\s` (and their `\D`/`\W`/`\S`
//!   negations outside classes), `\t`, `\n`, and escaped punctuation; the
//!   postfix quantifiers `*`, `+`, `?` on a single atom; `^` and `$`
//!   anchored to the line; and top-level alternation with `|`. Groups
//!   `(...)`, counted repetition `{n,m}`, and backreferences are refused
//!   with a reason, never silently misread.
//!
//! Both matchers are line-based: a line either matches or it does not, and
//! each matching line is one match, however many times the pattern occurs
//! on it. `case_sensitive: false` folds ASCII case on both sides, the same
//! folding `repo_tree`'s query mode uses.
//!
//! ## The walk
//!
//! Traversal honors the same documented gitignore subset as `repo_tree` —
//! comments and blanks skipped, trailing-slash directory patterns,
//! leading-slash anchoring, `*` within a segment, `**` across segments, and
//! no negation (`!` lines are counted as `ignored_negations`) — because a
//! search that reads what git ignores answers questions nobody asked.
//! `.git` is skipped unconditionally. The rules are reimplemented here
//! rather than imported because each guest is a sealed, self-contained
//! artifact; `repo_tree` owns the canonical statement of the subset.
//!
//! ## Bounds, and what truncation reports
//!
//! Every ceiling that fires is named in the output rather than swallowed:
//! `files_considered` counts the candidate files the walk produced,
//! `files_scanned` the ones whose bytes were actually searched, and
//! `files_unscanned` the considered files a budget left unread. Per file,
//! `matches_total` counts every matching line even when the per-file or
//! total match ceiling returned fewer, and `matches_dropped` sums what was
//! found in scanned files but not returned. Oversized, binary, and
//! unreadable files are counted, not hidden. `truncated` is true exactly
//! when any of that happened — silent truncation is a defect here.

use openagents_pdk::{
    list_mounted_dir, plugin_entry, read_mounted_file, MountDirListing, Refusal, RefusalCode,
};
use serde::{Deserialize, Serialize};

/// The workspace is the manifest's one mount.
const WORKSPACE_MOUNT: u32 = 0;
const DEFAULT_MAX_FILES: usize = 300;
const FILE_CAP: usize = 2_000;
const DEFAULT_MAX_MATCHES: usize = 20;
const MATCH_CAP: usize = 100;
const DEFAULT_PER_FILE: usize = 5;
const PER_FILE_CAP: usize = 20;
const DEFAULT_CONTEXT: usize = 2;
const CONTEXT_CAP: usize = 5;
/// Per-file byte bound; a larger file is counted oversized rather than read.
const MAX_FILE_BYTES: u64 = 524_288;
/// Characters kept of each returned line, match and context alike, so one
/// minified file cannot flood the output.
const LINE_CHAR_BOUND: usize = 200;
/// Most directory listings one invocation may ask the host for.
pub const LISTING_BOUND: usize = 2_000;
/// Most candidate files one invocation holds before the walk stops.
pub const HELD_BOUND: usize = 10_000;
/// Directory depth ceiling for the walk; deeper entries are cut, truncated.
const DEPTH_BOUND: usize = 16;
/// Backtracking steps the regex matcher may spend on one line before the
/// whole search is refused as too expensive. Quantifiers apply only to
/// single atoms in this subset, so real patterns never come near it.
const STEP_BUDGET: usize = 200_000;

#[derive(Debug, Deserialize)]
pub struct Input {
    /// What to search for. Required; must be non-empty after trimming.
    pub pattern: String,
    /// Treat `pattern` as a regex in the documented subset. Literal when
    /// absent or false.
    #[serde(default)]
    pub regex: Option<bool>,
    /// Subtree to search, relative to the workspace root. The root when absent.
    #[serde(default)]
    pub path: Option<String>,
    /// Match case exactly. Default true; false folds ASCII case.
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    /// Most files whose bytes are searched. Default 300, capped at 2000.
    #[serde(default)]
    pub max_files: Option<usize>,
    /// Most matches returned across all files. Default 20, capped at 100.
    #[serde(default)]
    pub max_matches: Option<usize>,
    /// Most matches returned per file. Default 5, capped at 20.
    #[serde(default)]
    pub max_matches_per_file: Option<usize>,
    /// Context lines kept on each side of a match. Default 2, capped at 5.
    #[serde(default)]
    pub context_lines: Option<usize>,
}

/// One matching line, with its bounded context window.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Match {
    /// 1-based line number.
    pub line: usize,
    pub text: String,
    /// Up to `context_lines` lines immediately before the match.
    pub before: Vec<String>,
    /// Up to `context_lines` lines immediately after the match.
    pub after: Vec<String>,
}

/// All of one file's returned matches, grouped.
#[derive(Debug, Serialize)]
pub struct FileMatches {
    /// Path relative to the workspace root.
    pub path: String,
    pub matches: Vec<Match>,
    /// Every matching line in the file, counted even when the match
    /// ceilings returned fewer than this.
    pub matches_total: usize,
}

#[derive(Debug, Serialize)]
pub struct Output {
    /// Matching files in walk order, each with its matches grouped.
    pub files: Vec<FileMatches>,
    /// Candidate files the walk produced (gitignored files excluded).
    pub files_considered: usize,
    /// Files whose bytes were searched.
    pub files_scanned: usize,
    /// Considered files never searched because the file or match budget
    /// stopped the scan first.
    pub files_unscanned: usize,
    /// Scanned files with at least one match.
    pub files_matched: usize,
    /// Matches returned across all files.
    pub matches_returned: usize,
    /// Matching lines found in scanned files but dropped by the per-file
    /// or total match ceiling.
    pub matches_dropped: usize,
    /// Entries the gitignore rules dropped (each skipped directory counts
    /// once; nothing under it is walked or counted).
    pub skipped_gitignored: usize,
    /// `!` gitignore lines this subset ignored rather than honored.
    pub ignored_negations: usize,
    /// Files skipped because a NUL byte marked them binary.
    pub skipped_binary: usize,
    /// Files past the per-file byte bound, never read.
    pub skipped_oversized: usize,
    /// Files the host refused to read for any reason but size.
    pub skipped_unreadable: usize,
    /// True exactly when something was dropped: files unscanned, matches
    /// dropped, or the walk itself cut short by a listing or held bound.
    pub truncated: bool,
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
pub fn code_search(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    let pattern = input.pattern.trim();
    if pattern.is_empty() {
        return Err(Refusal::unsupported(
            "the pattern is empty; pass a non-empty literal or regex in `pattern`",
        ));
    }
    let case_sensitive = input.case_sensitive.unwrap_or(true);
    let matcher = if input.regex.unwrap_or(false) {
        Matcher::Regex(parse_pattern(pattern, case_sensitive).map_err(|why| {
            Refusal::unsupported(format!(
                "the pattern is not a valid regex in this subset: {why}"
            ))
        })?)
    } else {
        Matcher::Literal {
            needle: if case_sensitive {
                pattern.to_string()
            } else {
                pattern.to_ascii_lowercase()
            },
            case_sensitive,
        }
    };
    let max_files = input
        .max_files
        .unwrap_or(DEFAULT_MAX_FILES)
        .clamp(1, FILE_CAP);
    let max_matches = input
        .max_matches
        .unwrap_or(DEFAULT_MAX_MATCHES)
        .clamp(1, MATCH_CAP);
    let per_file = input
        .max_matches_per_file
        .unwrap_or(DEFAULT_PER_FILE)
        .clamp(1, PER_FILE_CAP);
    let context = input
        .context_lines
        .unwrap_or(DEFAULT_CONTEXT)
        .min(CONTEXT_CAP);

    // Phase 1: the gitignore-aware walk collects candidate files in a fixed
    // depth-first order, so the scan below is deterministic and the
    // unscanned remainder is countable instead of unknown.
    let start = normalize_start(input.path.as_deref());
    let mut walk = Walk::new(host);
    walk.load_ancestors(&start);
    let mut candidates: Vec<(String, u64)> = Vec::new();
    walk.walk(&start, 0, &mut |path, kind, size| {
        if kind != "file" {
            return true;
        }
        if candidates.len() >= HELD_BOUND {
            return false;
        }
        candidates.push((path, size));
        true
    })?;

    // Phase 2: scan candidates in walk order until a budget stops it.
    let mut files: Vec<FileMatches> = Vec::new();
    let mut files_scanned = 0usize;
    let mut files_unscanned = 0usize;
    let mut files_matched = 0usize;
    let mut matches_returned = 0usize;
    let mut matches_dropped = 0usize;
    let mut skipped_binary = 0usize;
    let mut skipped_oversized = 0usize;
    let mut skipped_unreadable = 0usize;
    for (at, (path, size)) in candidates.iter().enumerate() {
        if files_scanned >= max_files || matches_returned >= max_matches {
            files_unscanned = candidates.len() - at;
            break;
        }
        if *size > MAX_FILE_BYTES {
            skipped_oversized += 1;
            continue;
        }
        let bytes = match host.read(path) {
            Ok(bytes) => bytes,
            Err(refusal) if refusal.code == RefusalCode::FileTooLarge => {
                skipped_oversized += 1;
                continue;
            }
            Err(_) => {
                skipped_unreadable += 1;
                continue;
            }
        };
        if bytes.contains(&0) {
            skipped_binary += 1;
            continue;
        }
        files_scanned += 1;
        let text = String::from_utf8_lossy(&bytes);
        // `lines()` leaves the `\r` of CRLF endings on the line; strip it so
        // Windows-authored files match and render the same as Unix ones.
        let lines: Vec<&str> = text
            .lines()
            .map(|line| line.trim_end_matches('\r'))
            .collect();
        let mut hit_lines: Vec<usize> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let matched = matcher.matches(line).map_err(|_| {
                Refusal::unsupported(format!(
                    "the regex exceeded the matching budget on {path}:{}; simplify the pattern",
                    index + 1
                ))
            })?;
            if matched {
                hit_lines.push(index);
            }
        }
        if hit_lines.is_empty() {
            continue;
        }
        files_matched += 1;
        let take = hit_lines
            .len()
            .min(per_file)
            .min(max_matches - matches_returned);
        let matches: Vec<Match> = hit_lines
            .iter()
            .take(take)
            .map(|&index| Match {
                line: index + 1,
                text: bound_line(lines[index]),
                before: lines[index.saturating_sub(context)..index]
                    .iter()
                    .map(|line| bound_line(line))
                    .collect(),
                after: lines[(index + 1).min(lines.len())..(index + 1 + context).min(lines.len())]
                    .iter()
                    .map(|line| bound_line(line))
                    .collect(),
            })
            .collect();
        matches_returned += take;
        matches_dropped += hit_lines.len() - take;
        files.push(FileMatches {
            path: path.clone(),
            matches,
            matches_total: hit_lines.len(),
        });
    }

    Ok(Output {
        files,
        files_considered: candidates.len(),
        files_scanned,
        files_unscanned,
        files_matched,
        matches_returned,
        matches_dropped,
        skipped_gitignored: walk.skipped_gitignored,
        ignored_negations: walk.ignored_negations,
        skipped_binary,
        skipped_oversized,
        skipped_unreadable,
        truncated: files_unscanned > 0 || matches_dropped > 0 || walk.truncated,
    })
}

/// Keep at most [`LINE_CHAR_BOUND`] characters of a line.
fn bound_line(line: &str) -> String {
    line.chars().take(LINE_CHAR_BOUND).collect()
}

// ---------------------------------------------------------------------------
// The matchers

enum Matcher {
    Literal {
        needle: String,
        case_sensitive: bool,
    },
    Regex(Pattern),
}

impl Matcher {
    /// Does the pattern match this line? `Err` means the regex step budget
    /// ran out — reported, never silently folded into "no match".
    fn matches(&self, line: &str) -> Result<bool, ()> {
        match self {
            Matcher::Literal {
                needle,
                case_sensitive,
            } => {
                if *case_sensitive {
                    Ok(line.contains(needle.as_str()))
                } else {
                    Ok(line.to_ascii_lowercase().contains(needle.as_str()))
                }
            }
            Matcher::Regex(pattern) => pattern.matches_line(line),
        }
    }
}

// ---------------------------------------------------------------------------
// The regex subset

/// A parsed pattern: top-level alternation over branches.
pub struct Pattern {
    branches: Vec<Branch>,
    /// ASCII-fold the line before matching; branch atoms were folded at
    /// parse time.
    fold_case: bool,
}

struct Branch {
    start_anchor: bool,
    end_anchor: bool,
    pieces: Vec<Piece>,
}

struct Piece {
    atom: Atom,
    quant: Quant,
}

enum Atom {
    /// `.`: any character on the line.
    Any,
    Char(char),
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
}

enum ClassItem {
    Char(char),
    Range(char, char),
    Digit,
    Word,
    Space,
}

enum Quant {
    One,
    Star,
    Plus,
    Opt,
}

/// Parse the subset, folding case at parse time when asked. Errors are
/// prose for the refusal reason.
pub fn parse_pattern(text: &str, case_sensitive: bool) -> Result<Pattern, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut branches = Vec::new();
    let mut at = 0usize;
    loop {
        let (branch, next) = parse_branch(&chars, at, case_sensitive)?;
        branches.push(branch);
        if next >= chars.len() {
            break;
        }
        // `parse_branch` stops only at `|` or the end.
        at = next + 1;
        if at >= chars.len() {
            return Err("a trailing `|` leaves an empty branch".to_string());
        }
    }
    Ok(Pattern {
        branches,
        fold_case: !case_sensitive,
    })
}

fn parse_branch(
    chars: &[char],
    mut at: usize,
    case_sensitive: bool,
) -> Result<(Branch, usize), String> {
    let mut start_anchor = false;
    if chars.get(at) == Some(&'^') {
        start_anchor = true;
        at += 1;
    }
    let mut pieces = Vec::new();
    let mut end_anchor = false;
    while at < chars.len() {
        let c = chars[at];
        if c == '|' {
            break;
        }
        if c == '$' {
            if at + 1 == chars.len() || chars[at + 1] == '|' {
                end_anchor = true;
                at += 1;
                break;
            }
            return Err("`$` is only supported at the end of a pattern or branch".to_string());
        }
        if c == '(' || c == ')' {
            return Err("groups `(...)` are not supported by the regex subset".to_string());
        }
        if c == '{' || c == '}' {
            return Err(
                "counted repetition `{n,m}` is not supported by the regex subset".to_string(),
            );
        }
        if c == '*' || c == '+' || c == '?' {
            return Err(format!("`{c}` has nothing to repeat"));
        }
        let (atom, next) = parse_atom(chars, at, case_sensitive)?;
        at = next;
        let quant = match chars.get(at) {
            Some('*') => {
                at += 1;
                Quant::Star
            }
            Some('+') => {
                at += 1;
                Quant::Plus
            }
            Some('?') => {
                at += 1;
                Quant::Opt
            }
            _ => Quant::One,
        };
        pieces.push(Piece { atom, quant });
    }
    Ok((
        Branch {
            start_anchor,
            end_anchor,
            pieces,
        },
        at,
    ))
}

fn parse_atom(chars: &[char], at: usize, case_sensitive: bool) -> Result<(Atom, usize), String> {
    let fold = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_ascii_lowercase()
        }
    };
    match chars[at] {
        '.' => Ok((Atom::Any, at + 1)),
        '[' => parse_class(chars, at, case_sensitive),
        '^' => Err("`^` is only supported at the start of a pattern or branch".to_string()),
        '\\' => {
            let escaped = *chars
                .get(at + 1)
                .ok_or_else(|| "a trailing `\\` escapes nothing".to_string())?;
            let atom = match escaped {
                'd' => class_of(false, ClassItem::Digit),
                'D' => class_of(true, ClassItem::Digit),
                'w' => class_of(false, ClassItem::Word),
                'W' => class_of(true, ClassItem::Word),
                's' => class_of(false, ClassItem::Space),
                'S' => class_of(true, ClassItem::Space),
                't' => Atom::Char('\t'),
                'n' => Atom::Char('\n'),
                other if other.is_ascii_alphanumeric() => {
                    return Err(format!("the escape `\\{other}` is not in the subset"))
                }
                other => Atom::Char(fold(other)),
            };
            Ok((atom, at + 2))
        }
        c => Ok((Atom::Char(fold(c)), at + 1)),
    }
}

fn class_of(negated: bool, item: ClassItem) -> Atom {
    Atom::Class {
        negated,
        items: vec![item],
    }
}

fn parse_class(chars: &[char], at: usize, case_sensitive: bool) -> Result<(Atom, usize), String> {
    let fold = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_ascii_lowercase()
        }
    };
    let mut cursor = at + 1;
    let negated = chars.get(cursor) == Some(&'^');
    if negated {
        cursor += 1;
    }
    let mut items: Vec<ClassItem> = Vec::new();
    loop {
        let c = *chars
            .get(cursor)
            .ok_or_else(|| "the character class `[` is never closed".to_string())?;
        if c == ']' {
            if items.is_empty() {
                return Err("the character class `[]` is empty".to_string());
            }
            return Ok((Atom::Class { negated, items }, cursor + 1));
        }
        let low = if c == '\\' {
            let escaped = *chars
                .get(cursor + 1)
                .ok_or_else(|| "a trailing `\\` escapes nothing".to_string())?;
            cursor += 2;
            match escaped {
                'd' => {
                    items.push(ClassItem::Digit);
                    continue;
                }
                'w' => {
                    items.push(ClassItem::Word);
                    continue;
                }
                's' => {
                    items.push(ClassItem::Space);
                    continue;
                }
                't' => '\t',
                'n' => '\n',
                other if other.is_ascii_alphanumeric() => {
                    return Err(format!("the class escape `\\{other}` is not in the subset"))
                }
                other => other,
            }
        } else {
            cursor += 1;
            c
        };
        // A `-` between two characters is a range; anywhere else it is the
        // literal dash, matching what grep users expect of `[a-z-]`.
        if chars.get(cursor) == Some(&'-') && chars.get(cursor + 1).is_some_and(|&next| next != ']')
        {
            let high = chars[cursor + 1];
            if high == '\\' {
                return Err("an escape cannot end a class range".to_string());
            }
            if (low as u32) > (high as u32) {
                return Err(format!("the class range `{low}-{high}` runs backwards"));
            }
            items.push(ClassItem::Range(fold(low), fold(high)));
            cursor += 2;
        } else {
            items.push(ClassItem::Char(fold(low)));
        }
    }
}

impl Pattern {
    /// Does any branch match anywhere on the line? `Err` means the step
    /// budget ran out.
    pub fn matches_line(&self, line: &str) -> Result<bool, ()> {
        let chars: Vec<char> = if self.fold_case {
            line.chars().map(|c| c.to_ascii_lowercase()).collect()
        } else {
            line.chars().collect()
        };
        let mut steps = STEP_BUDGET;
        for branch in &self.branches {
            let starts: Vec<usize> = if branch.start_anchor {
                vec![0]
            } else {
                (0..=chars.len()).collect()
            };
            for start in starts {
                if match_here(&branch.pieces, &chars, start, branch.end_anchor, &mut steps)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

/// Match the remaining pieces at `pos`, backtracking over quantifiers.
/// Quantifiers apply to single atoms only, so the recursion depth is the
/// piece count and the budget bounds total work.
fn match_here(
    pieces: &[Piece],
    line: &[char],
    pos: usize,
    end_anchor: bool,
    steps: &mut usize,
) -> Result<bool, ()> {
    if *steps == 0 {
        return Err(());
    }
    *steps -= 1;
    let Some((piece, rest)) = pieces.split_first() else {
        return Ok(!end_anchor || pos == line.len());
    };
    let hits = |at: usize| line.get(at).is_some_and(|&c| atom_matches(&piece.atom, c));
    match piece.quant {
        Quant::One => {
            if hits(pos) {
                match_here(rest, line, pos + 1, end_anchor, steps)
            } else {
                Ok(false)
            }
        }
        Quant::Opt => {
            if hits(pos) && match_here(rest, line, pos + 1, end_anchor, steps)? {
                return Ok(true);
            }
            match_here(rest, line, pos, end_anchor, steps)
        }
        Quant::Star | Quant::Plus => {
            let least = if matches!(piece.quant, Quant::Plus) {
                1
            } else {
                0
            };
            let mut most = pos;
            while hits(most) {
                most += 1;
            }
            // Greedy: the longest run first, giving back one character at a
            // time, exactly the order a reader expects of `*`.
            let mut take = most;
            loop {
                if take < pos + least {
                    return Ok(false);
                }
                if match_here(rest, line, take, end_anchor, steps)? {
                    return Ok(true);
                }
                if take == pos {
                    return Ok(false);
                }
                take -= 1;
            }
        }
    }
}

fn atom_matches(atom: &Atom, c: char) -> bool {
    match atom {
        Atom::Any => true,
        Atom::Char(wanted) => c == *wanted,
        Atom::Class { negated, items } => {
            let inside = items.iter().any(|item| class_item_matches(item, c));
            inside != *negated
        }
    }
}

fn class_item_matches(item: &ClassItem, c: char) -> bool {
    match item {
        ClassItem::Char(wanted) => c == *wanted,
        ClassItem::Range(low, high) => (*low..=*high).contains(&c),
        ClassItem::Digit => c.is_ascii_digit(),
        ClassItem::Word => c.is_ascii_alphanumeric() || c == '_',
        ClassItem::Space => c.is_whitespace(),
    }
}

// ---------------------------------------------------------------------------
// The walk — the same documented gitignore subset as `repo_tree`, which owns
// its canonical statement; reimplemented because each guest is sealed.

/// A visitor: entry path, kind, size; `false` stops the whole walk.
type Sink<'s> = dyn FnMut(String, &str, u64) -> bool + 's;

struct Walk<'a> {
    host: &'a dyn Host,
    listings: usize,
    truncated: bool,
    skipped_gitignored: usize,
    ignored_negations: usize,
    rule_sets: Vec<RuleSet>,
}

impl<'a> Walk<'a> {
    fn new(host: &'a dyn Host) -> Self {
        Walk {
            host,
            listings: 0,
            truncated: false,
            skipped_gitignored: 0,
            ignored_negations: 0,
            rule_sets: Vec::new(),
        }
    }

    /// Rules from `.gitignore` files above the walk's start, so a subtree
    /// search still honors the root's ignores. The start's own `.gitignore`
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
            // Symlinks are reported but never followed; neither shape can
            // be searched.
            if entry.kind != "file" && entry.kind != "dir" {
                continue;
            }
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
            if entry.kind == "dir" {
                if depth + 1 >= DEPTH_BOUND {
                    self.truncated = true;
                } else if !self.walk(&path, depth + 1, sink)? {
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

fn handle(input: Input) -> Result<Output, Refusal> {
    code_search(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
