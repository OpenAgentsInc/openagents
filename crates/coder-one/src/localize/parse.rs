//! File and line references in compiler, test-runner, and traceback
//! output, read by one table of rules ([`RULES`]).
//!
//! Each rule is a pattern with a `path` and a `line` group, and says
//! whether the output it matches lists the innermost frame last (a Python
//! traceback) or first (a JVM, Node, or Go stack, and compiler errors,
//! whose first error matters most). [`parse`] returns the references in
//! the order a reader should see them: the innermost traceback frame
//! first, then the rest in the order the output printed them.
//! [`resolve`] maps a reference to a file of the workspace, and leaves out
//! the standard library, installed packages, and generated names.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// One row of the table.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    /// The rule's name, recorded with each reference it finds.
    pub name: &'static str,
    /// The output it reads, in words.
    pub reads: &'static str,
    /// The pattern, with named groups `path` and `line`, and optionally
    /// `col` and `qual` (a JVM frame's qualified class).
    pub pattern: &'static str,
    /// The output lists the innermost frame last.
    pub innermost_last: bool,
}

/// The table, in precedence order: where two rules match the same text,
/// the earlier one wins.
pub const RULES: &[Rule] = &[
    Rule {
        name: "file-line",
        reads: "Python tracebacks, and Coq, Rocq, and OCaml errors: File \"p\", line n",
        pattern: r#"File "(?P<path>[^"]+)", line (?P<line>\d+)"#,
        innermost_last: true,
    },
    Rule {
        name: "rust-arrow",
        reads: "rustc diagnostics: --> p:n:c",
        pattern: r"-->\s*(?P<path>[^\s:]+):(?P<line>\d+):(?P<col>\d+)",
        innermost_last: false,
    },
    Rule {
        name: "rust-panic",
        reads: "Rust panics: panicked at p:n:c",
        pattern: r"panicked at (?P<path>[^\s:]+):(?P<line>\d+):(?P<col>\d+)",
        innermost_last: false,
    },
    Rule {
        name: "jvm-frame",
        reads: "Java, Scala, and Kotlin stack frames: at pkg.Class.method(File.java:n)",
        pattern: r"\bat (?P<qual>[\w$.<>/]+)\((?P<path>[\w$.-]+\.(?:java|scala|kt|groovy)):(?P<line>\d+)\)",
        innermost_last: false,
    },
    Rule {
        name: "node-frame",
        reads: "Node stack frames: at f (p:n:c) or at p:n:c",
        pattern: r"\bat (?:[^\s(]+ \()?(?:file://)?(?P<path>[^\s():]+\.(?:js|mjs|cjs|ts|tsx|jsx)):(?P<line>\d+):(?P<col>\d+)\)?",
        innermost_last: false,
    },
    Rule {
        name: "go-frame",
        reads: "Go goroutine stacks: a tab, then p.go:n +0x..",
        pattern: r"^\s+(?P<path>[^\s:]+\.go):(?P<line>\d+)(?:\s+\+0x[0-9a-f]+)?\s*$",
        innermost_last: false,
    },
    Rule {
        name: "paren",
        reads: "TypeScript and MSVC diagnostics: p(n,c): error",
        pattern: r"(?P<path>[^\s():]+\.[A-Za-z][A-Za-z0-9_+]{0,7})\((?P<line>\d+)(?:,(?P<col>\d+))?\)\s*:",
        innermost_last: false,
    },
    Rule {
        name: "colon",
        reads: "GCC, Clang, Go, javac, scalac, Lean, and pytest: p:n: or p:n:c:",
        pattern: r#"(?:^|[\s(\['"])(?P<path>[^\s:()\['"]*[^\s:()\['".]\.[A-Za-z][A-Za-z0-9_+]{0,7}):(?P<line>\d+)(?::(?P<col>\d+))?(?::|\s|$|,)"#,
        innermost_last: false,
    },
];

fn compiled() -> &'static [Regex] {
    static SET: OnceLock<Vec<Regex>> = OnceLock::new();
    SET.get_or_init(|| {
        RULES
            .iter()
            .map(|rule| Regex::new(rule.pattern).expect("a localization rule compiles"))
            .collect()
    })
}

/// One file and line an output names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// The path as the output wrote it.
    pub path: String,
    /// The line, from 1.
    pub line: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
    /// A JVM frame's qualified class, which hints at the file's directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualified: Option<String>,
    /// The rule that found it.
    pub rule: String,
}

/// Every reference in `output`, the innermost traceback frame first,
/// without repeats of the same path and line.
#[must_use]
pub fn parse(output: &str) -> Vec<Location> {
    // (line index, start byte, innermost_last, location)
    let mut found: Vec<(usize, usize, bool, Location)> = Vec::new();
    for (index, text) in output.lines().enumerate() {
        let mut taken: Vec<(usize, usize)> = Vec::new();
        for (rule, pattern) in RULES.iter().zip(compiled()) {
            for caps in pattern.captures_iter(text) {
                let Some(path) = caps.name("path") else {
                    continue;
                };
                let whole = caps.get(0).map_or((0, 0), |m| (m.start(), m.end()));
                if taken.iter().any(|(s, e)| whole.0 < *e && *s < whole.1) {
                    continue;
                }
                let Some(line) = caps
                    .name("line")
                    .and_then(|m| m.as_str().parse::<u64>().ok())
                    .filter(|n| *n > 0)
                else {
                    continue;
                };
                let path = path.as_str();
                if !plausible(path) {
                    continue;
                }
                taken.push(whole);
                found.push((
                    index,
                    whole.0,
                    rule.innermost_last,
                    Location {
                        path: path.to_string(),
                        line,
                        column: caps.name("col").and_then(|m| m.as_str().parse().ok()),
                        qualified: caps.name("qual").map(|m| m.as_str().to_string()),
                        rule: rule.name.to_string(),
                    },
                ));
            }
        }
    }
    found.sort_by_key(|(line, start, _, _)| (*line, *start));
    let (mut last, first): (Vec<_>, Vec<_>) = found
        .into_iter()
        .partition(|(_, _, innermost_last, _)| *innermost_last);
    last.reverse();
    let mut seen = BTreeSet::new();
    last.into_iter()
        .chain(first)
        .map(|(_, _, _, location)| location)
        .filter(|l| seen.insert((l.path.clone(), l.line)))
        .collect()
}

/// Whether a matched path could be a file: not a URL, a version, a time,
/// or an address.
fn plausible(path: &str) -> bool {
    if path.contains("://") || path.starts_with("//") {
        return false;
    }
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && ext.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        // `1.2.3`, `0x1f.x`, and IP-like names aren't files.
        && !stem.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Path parts that mean the file isn't the task's own: the standard
/// library, installed packages, toolchains, and generated names.
pub const OUTSIDE: &[&str] = &[
    "site-packages/",
    "dist-packages/",
    "/usr/lib/",
    "/usr/local/lib/",
    "/usr/include/",
    "/lib/python",
    "/rustc/",
    "/.rustup/",
    ".cargo/registry/",
    "node_modules/",
    "/nix/store/",
    "/go/pkg/mod/",
    "/usr/local/go/src/",
    "/opt/conda/",
    "<",
];

/// Whether `path` is a library's or a generated file's, never the task's.
#[must_use]
pub fn outside(path: &str) -> bool {
    OUTSIDE.iter().any(|part| path.contains(part))
}

/// The workspace file, relative to its root, that `location` names, from
/// `files`, the workspace's relative paths; `root` is the workspace as
/// commands saw it, such as `/app`. An absolute path under `root` is taken
/// relative to it; otherwise the longest trailing part of the path that is
/// a workspace file wins, and a bare file name must match exactly one file
/// (a JVM frame's package breaks a tie).
#[must_use]
pub fn resolve(location: &Location, files: &BTreeSet<String>, root: &str) -> Option<String> {
    let path = location.path.trim_start_matches("file://");
    if outside(path) {
        return None;
    }
    let root = root.trim_end_matches('/');
    let path = if !root.is_empty() {
        path.strip_prefix(&format!("{root}/")).unwrap_or(path)
    } else {
        path
    };
    let path = path.trim_start_matches("./").replace('\\', "/");
    let parts: Vec<&str> = path
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    // The longest trailing part with a directory in it that is a file.
    for start in 0..parts.len().saturating_sub(1) {
        let tail = parts[start..].join("/");
        if files.contains(&tail) {
            return Some(tail);
        }
    }
    let name = parts.last()?;
    let mut matches: Vec<&String> = files
        .iter()
        .filter(|f| f.as_str() == *name || f.ends_with(&format!("/{name}")))
        .collect();
    if matches.len() > 1
        && let Some(qualified) = &location.qualified
    {
        // `com.acme.Bar.run` → `com/acme/`.
        let dirs: Vec<&str> = qualified.split('.').collect();
        let package = dirs[..dirs.len().saturating_sub(2)].join("/");
        if !package.is_empty() {
            matches.retain(|f| f.contains(&format!("{package}/")));
        }
    }
    (matches.len() == 1).then(|| matches[0].clone())
}
