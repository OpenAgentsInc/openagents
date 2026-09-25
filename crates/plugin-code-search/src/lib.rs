//! Code search guest.
//!
//! The `search` operation reads the text files of a granted snapshot and
//! returns the lines that match any of up to 16 patterns, grouped by file.
//! A pattern is literal text, where `*` stands for any run of characters
//! within one line. Matching folds ASCII case unless `case_sensitive` is
//! true, and `whole_word` requires each match to start and end at a word
//! boundary.
//!
//! Files that match more distinct patterns come first, then files with more
//! matching lines, then by path, so the files an issue's terms point at
//! lead the result. Every bound that fires is counted rather than hidden:
//! `matches_total` counts every matching line, `files_skipped` counts
//! binary, lock, and generated files, and `truncated` is true when a
//! per-file, total, or read bound left anything out.
//!
//! This reimplements the pre-reset `code-search` plugin's purpose on the
//! `openagents.plugin-packet.v1` ABI, without its regular-expression subset
//! or gitignore walk: the host decides which files the grant holds.

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Value, json};

plugin_pdk::export_guest!(handle);

const MAX_PATTERNS: usize = 16;
const MIN_PATTERN_CHARS: usize = 2;
const MAX_PATTERN_CHARS: usize = 200;
const DEFAULT_MAX_FILES: usize = 5_000;
const MAX_FILES_CAP: usize = 20_000;
const DEFAULT_FILE_BYTES: usize = 256 * 1024;
const FILE_BYTES_CAP: usize = 1024 * 1024;
const DEFAULT_MAX_MATCHES: usize = 60;
const MAX_MATCHES_CAP: usize = 400;
const DEFAULT_PER_FILE: usize = 5;
const PER_FILE_CAP: usize = 40;
const DEFAULT_MAX_RESULT_FILES: usize = 20;
/// The most characters of one matching line the result keeps.
const LINE_CHARS: usize = 160;
/// Bytes checked for a NUL before a file counts as binary.
const BINARY_PROBE: usize = 1024;

/// Files that are never source: images, archives, binaries, and lock files.
const SKIPPED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "ico", "bmp", "webp", "pdf", "zip", "gz", "tgz", "xz", "bz2",
    "tar", "jar", "wasm", "so", "dylib", "dll", "exe", "bin", "o", "a", "class", "pyc", "lock",
    "woff", "woff2", "ttf", "otf", "mp3", "mp4", "mov", "sqlite", "db",
];
const SKIPPED_NAMES: &[&str] = &["package-lock.json", "yarn.lock", "pnpm-lock.yaml"];

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "search" => search(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// The search's parameters, read and bounded from the input.
struct Query {
    patterns: Vec<String>,
    /// Each pattern split on `*`, folded when the search folds case.
    parts: Vec<Vec<Vec<u8>>>,
    case_sensitive: bool,
    whole_word: bool,
    max_files: usize,
    file_bytes: usize,
    max_matches: usize,
    per_file: usize,
    result_files: usize,
}

fn count(input: &Value, key: &str, default: usize, cap: usize) -> usize {
    input[key]
        .as_u64()
        .map_or(default, |n| usize::try_from(n).unwrap_or(cap))
        .clamp(1, cap)
}

fn query(input: &Value) -> Result<Query, Refusal> {
    let patterns: Vec<String> = input["patterns"]
        .as_array()
        .ok_or_else(|| Refusal::unsupported("patterns is a list of strings"))?
        .iter()
        .map(|pattern| {
            pattern
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| Refusal::unsupported("patterns is a list of strings"))
        })
        .collect::<Result<_, _>>()?;
    if patterns.is_empty() || patterns.len() > MAX_PATTERNS {
        return Err(Refusal::unsupported("patterns holds one to 16 strings"));
    }
    let case_sensitive = input["case_sensitive"].as_bool().unwrap_or(false);
    let mut parts = Vec::with_capacity(patterns.len());
    for pattern in &patterns {
        let literal = pattern.chars().filter(|c| *c != '*').count();
        if literal < MIN_PATTERN_CHARS || pattern.chars().count() > MAX_PATTERN_CHARS {
            return Err(Refusal::unsupported(
                "each pattern has 2 to 200 characters besides its wildcards",
            ));
        }
        parts.push(
            pattern
                .split('*')
                .filter(|part| !part.is_empty())
                .map(|part| fold(part.as_bytes(), case_sensitive))
                .collect(),
        );
    }
    Ok(Query {
        patterns,
        parts,
        case_sensitive,
        whole_word: input["whole_word"].as_bool().unwrap_or(false),
        max_files: count(input, "max_files", DEFAULT_MAX_FILES, MAX_FILES_CAP),
        file_bytes: count(input, "max_file_bytes", DEFAULT_FILE_BYTES, FILE_BYTES_CAP),
        max_matches: count(input, "max_matches", DEFAULT_MAX_MATCHES, MAX_MATCHES_CAP),
        per_file: count(input, "per_file", DEFAULT_PER_FILE, PER_FILE_CAP),
        result_files: count(
            input,
            "max_result_files",
            DEFAULT_MAX_RESULT_FILES,
            MAX_FILES_CAP,
        ),
    })
}

fn fold(bytes: &[u8], case_sensitive: bool) -> Vec<u8> {
    if case_sensitive {
        bytes.to_vec()
    } else {
        bytes.to_ascii_lowercase()
    }
}

/// One matching line.
struct Hit {
    line: usize,
    pattern: usize,
    text: String,
}

/// The matches in one file.
struct FileHits {
    path: String,
    /// Which patterns matched anywhere in the file.
    patterns: Vec<bool>,
    /// Lines each pattern matched, including lines past the per-file cap.
    lines: Vec<usize>,
    matches: usize,
    hits: Vec<Hit>,
}

fn search(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let query = query(&request.input)?;
    let root = guest::root(request).ok_or_else(|| Refusal::refused("no granted snapshot"))?;
    let listing = guest::list(host, &root, query.max_files)
        .map_err(|code| Refusal::refused(format!("list {code}")))?;
    let mut truncated = !listing.complete;
    let mut scanned = 0_usize;
    let mut skipped = 0_usize;
    let mut unread = 0_usize;
    let mut per_pattern = vec![(0_usize, 0_usize); query.patterns.len()];
    let mut files: Vec<FileHits> = Vec::new();
    let mut budget_spent = false;
    for entry in listing.entries.iter().filter(|entry| entry.kind == "file") {
        let path = guest::relative(&entry.name);
        if skipped_name(path) {
            skipped += 1;
            continue;
        }
        if budget_spent {
            unread += 1;
            continue;
        }
        let read = match guest::read(host, &entry.handle, query.file_bytes) {
            Ok(read) => read,
            Err(guest::BUDGET) => {
                budget_spent = true;
                unread += 1;
                continue;
            }
            Err(_) => {
                unread += 1;
                continue;
            }
        };
        if !read.complete {
            truncated = true;
        }
        if read.bytes[..read.bytes.len().min(BINARY_PROBE)].contains(&0) {
            skipped += 1;
            continue;
        }
        scanned += 1;
        if let Some(found) = scan(&query, path, &read.bytes) {
            for (index, lines) in found.lines.iter().enumerate() {
                per_pattern[index].0 += lines;
                per_pattern[index].1 += usize::from(*lines > 0);
            }
            files.push(found);
        }
    }
    if unread > 0 {
        truncated = true;
    }
    files.sort_by(|a, b| {
        let distinct = |f: &FileHits| f.patterns.iter().filter(|p| **p).count();
        distinct(b)
            .cmp(&distinct(a))
            .then(b.matches.cmp(&a.matches))
            .then(a.path.cmp(&b.path))
    });
    let matches_total: usize = files.iter().map(|file| file.matches).sum();
    let mut left = query.max_matches;
    let mut result = Vec::new();
    for file in files.iter().take(query.result_files) {
        if left == 0 {
            break;
        }
        let lines: Vec<Value> = file
            .hits
            .iter()
            .take(left)
            .map(|hit| {
                json!({
                    "line": hit.line,
                    "pattern": query.patterns[hit.pattern],
                    "text": hit.text,
                })
            })
            .collect();
        left -= lines.len();
        result.push(json!({
            "path": file.path,
            "patterns": file.patterns.iter().filter(|p| **p).count(),
            "matches": file.matches,
            "lines": lines,
        }));
    }
    let returned: usize = result
        .iter()
        .map(|file| file["lines"].as_array().map_or(0, Vec::len))
        .sum();
    if returned < matches_total {
        truncated = true;
    }
    Ok(json!({
        "kind": "code-search",
        "patterns": query.patterns,
        "case_sensitive": query.case_sensitive,
        "whole_word": query.whole_word,
        "files_listed": listing.entries.len(),
        "files_scanned": scanned,
        "files_skipped": skipped,
        "files_unread": unread,
        "files_matched": files.len(),
        "matches_total": matches_total,
        "matches_returned": returned,
        "truncated": truncated,
        "per_pattern": query.patterns.iter().zip(&per_pattern).map(|(pattern, (lines, files))| json!({
            "pattern": pattern,
            "lines": lines,
            "files": files,
        })).collect::<Vec<_>>(),
        "files": result,
    }))
}

fn skipped_name(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if SKIPPED_NAMES.contains(&name) || name.ends_with(".min.js") || name.ends_with(".min.css") {
        return true;
    }
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        SKIPPED_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// The matches in one file's bytes, or `None` when nothing matched.
fn scan(query: &Query, path: &str, bytes: &[u8]) -> Option<FileHits> {
    let mut found = FileHits {
        path: path.to_string(),
        patterns: vec![false; query.patterns.len()],
        lines: vec![0; query.patterns.len()],
        matches: 0,
        hits: Vec::new(),
    };
    // Fold the file once, and skip it when no pattern's first part occurs
    // anywhere in it, which is most files.
    let folded = fold(bytes, query.case_sensitive);
    if !query.parts.iter().any(|parts| {
        parts
            .first()
            .is_some_and(|first| find(&folded, first).is_some())
    }) {
        return None;
    }
    let mut start = 0;
    for (number, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let folded = &folded[start..start + line.len()];
        start += line.len() + 1;
        let Some(pattern) = query
            .parts
            .iter()
            .position(|parts| matches(folded, parts, query.whole_word))
        else {
            continue;
        };
        for (index, parts) in query.parts.iter().enumerate() {
            if index == pattern || matches(folded, parts, query.whole_word) {
                found.patterns[index] = true;
                found.lines[index] += 1;
            }
        }
        found.matches += 1;
        if found.hits.len() < query.per_file {
            let text = String::from_utf8_lossy(line);
            found.hits.push(Hit {
                line: number + 1,
                pattern,
                text: text.trim().chars().take(LINE_CHARS).collect(),
            });
        }
    }
    (found.matches > 0).then_some(found)
}

/// Whether `line` holds the pattern's parts in order. With `whole_word`,
/// the match starts and ends at a word boundary.
fn matches(line: &[u8], parts: &[Vec<u8>], whole_word: bool) -> bool {
    let Some(first) = parts.first() else {
        return false;
    };
    let mut from = 0;
    while let Some(start) = find(&line[from..], first).map(|at| at + from) {
        if let Some(end) = rest(line, start + first.len(), &parts[1..])
            && (!whole_word || (boundary(line, start) && boundary(line, end)))
        {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Where the remaining parts end when each follows the one before.
fn rest(line: &[u8], mut at: usize, parts: &[Vec<u8>]) -> Option<usize> {
    for part in parts {
        at += find(&line[at..], part)? + part.len();
    }
    Some(at)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let (&first, rest) = needle.split_first()?;
    let last = haystack.len().checked_sub(needle.len())?;
    let mut at = 0;
    while at <= last {
        at += haystack[at..=last].iter().position(|byte| *byte == first)?;
        if haystack[at + 1..at + needle.len()] == *rest {
            return Some(at);
        }
        at += 1;
    }
    None
}

/// Whether offset `at` sits between a word character and a non-word one,
/// counting the ends of the line as non-word.
fn boundary(line: &[u8], at: usize) -> bool {
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let before = at.checked_sub(1).and_then(|i| line.get(i)).copied();
    let after = line.get(at).copied();
    before.is_some_and(word) != after.is_some_and(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn fixture() -> MemoryHost {
        MemoryHost::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tree"))
    }

    fn run(host: &mut MemoryHost, input: Value) -> Result<Value, Refusal> {
        handle(&MemoryHost::request("search", input), host)
    }

    #[test]
    fn files_matching_more_patterns_come_first() {
        let mut host = fixture();
        let value = run(&mut host, json!({"patterns": ["ledger", "retry_limit"]})).unwrap();
        assert_eq!(value["kind"], "code-search");
        assert_eq!(value["files"][0]["path"], "src/billing.py");
        assert_eq!(value["files"][0]["patterns"], 2);
        assert_eq!(value["files"][0]["lines"][0]["line"], 1);
        assert_eq!(value["files_skipped"], 1, "the lock file is skipped");
        assert_eq!(value["truncated"], false);
    }

    #[test]
    fn case_whole_words_and_wildcards() {
        let mut host = fixture();
        let folded = run(&mut host, json!({"patterns": ["LEDGER"]})).unwrap();
        let exact = run(
            &mut host,
            json!({"patterns": ["LEDGER"], "case_sensitive": true}),
        )
        .unwrap();
        assert!(folded["matches_total"].as_u64().unwrap() > 0);
        assert_eq!(exact["matches_total"], 0);
        let word = run(&mut host, json!({"patterns": ["post"], "whole_word": true})).unwrap();
        let any = run(&mut host, json!({"patterns": ["post"]})).unwrap();
        assert!(word["matches_total"].as_u64() < any["matches_total"].as_u64());
        let wild = run(&mut host, json!({"patterns": ["def *(self"]})).unwrap();
        assert_eq!(wild["files"][0]["path"], "src/billing.py");
    }

    #[test]
    fn bounds_are_reported() {
        let mut host = fixture();
        let value = run(&mut host, json!({"patterns": ["e"], "max_matches": 2})).unwrap_err();
        assert_eq!(
            value.status, "unsupported_input",
            "one-letter patterns refuse"
        );
        let value = run(&mut host, json!({"patterns": ["se"], "max_matches": 2})).unwrap();
        assert_eq!(value["matches_returned"], 2);
        assert_eq!(value["truncated"], true);
        let mut starved = fixture().with_read_budget(8);
        let value = run(&mut starved, json!({"patterns": ["ledger"]})).unwrap();
        assert!(value["files_unread"].as_u64().unwrap() > 0);
        assert_eq!(value["truncated"], true);
    }

    #[test]
    fn a_binary_file_is_skipped() {
        let mut host = MemoryHost::new([
            ("blob.dat", b"ledger\0ledger".to_vec()),
            ("a.txt", b"ledger".to_vec()),
        ]);
        let value = run(&mut host, json!({"patterns": ["ledger"]})).unwrap();
        assert_eq!(value["files_skipped"], 1);
        assert_eq!(value["files_matched"], 1);
    }
}
