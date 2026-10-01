//! Dependency-check guest.
//!
//! The `check` operation reads a project's manifests and lockfiles from a
//! granted snapshot, offline, and flags three things: packages a lockfile
//! holds at more than one version, dependencies a manifest leaves loose
//! (any version, no upper bound, or a Git source without a commit), and
//! licenses a declared policy doesn't allow. It is the Wasm in the
//! *Dependency check* plugin (`docs/plugins/examples/dependency-check.md`).
//!
//! What it reads, chosen by file name: Cargo (`Cargo.toml`, `Cargo.lock`),
//! npm, pnpm, Yarn, and Bun (`package.json`, `package-lock.json`,
//! `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`), Python
//! (`pyproject.toml`, `requirements*.txt`, `uv.lock`, `poetry.lock`), and
//! Go (`go.mod`). The policy is `dependency-policy.toml`, or `deny.toml`
//! (cargo-deny's) when there is none:
//!
//! ```toml
//! [licenses]
//! allow = ["MIT", "Apache-2.0", "BSD-3-Clause"]
//! deny = ["GPL-3.0"]
//!
//! [ranges]
//! allow_unbounded = false   # true: a lower bound alone is fine
//! exact = false             # true: manifests must pin exact versions
//! ```
//!
//! A license is checked only where a file records it: npm lockfiles and
//! every manifest's own `license`. Cargo, pnpm, Yarn, and Python lockfiles
//! don't record licenses, and the result counts those packages as not
//! checked rather than guessing. Every bound the guest hit (a file cut at
//! the read budget, a list cut short) is in the result.

use std::collections::BTreeMap;

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Map, Value, json};

plugin_pdk::export_guest!(handle);

/// The most bytes of one file the guest reads.
const FILE_BYTES: usize = 60 * 1024;
const MAX_LISTED: usize = 2_048;
/// The most files the guest reads.
const MAX_FILES: usize = 24;
/// The most findings of one kind the result lists.
const MAX_LISTED_FINDINGS: usize = 40;

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "check" => check(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// What a file is to the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Policy,
    Manifest,
    Lock,
}

fn role_of(path: &str) -> Option<(Role, &'static str)> {
    let name = path.rsplit('/').next().unwrap_or(path);
    Some(match name {
        "dependency-policy.toml" | "deny.toml" => (Role::Policy, "policy"),
        "Cargo.toml" => (Role::Manifest, "cargo"),
        "Cargo.lock" => (Role::Lock, "cargo"),
        "package.json" => (Role::Manifest, "npm"),
        "package-lock.json"
        | "npm-shrinkwrap.json"
        | "pnpm-lock.yaml"
        | "yarn.lock"
        | "bun.lock"
        | "bun.lockb" => (Role::Lock, "npm"),
        "pyproject.toml" => (Role::Manifest, "python"),
        "uv.lock" | "poetry.lock" => (Role::Lock, "python"),
        "go.mod" => (Role::Manifest, "go"),
        "go.sum" => (Role::Lock, "go"),
        _ if name.starts_with("requirements") && name.ends_with(".txt") => {
            (Role::Manifest, "python")
        }
        _ => return None,
    })
}

/// One thing the check flags.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    /// `duplicate`, `any-version`, `unbounded`, `unpinned-git`,
    /// `not-exact`, `two-majors`, `license-not-allowed`, `license-denied`,
    /// or `no-lockfile`.
    kind: &'static str,
    ecosystem: &'static str,
    package: String,
    file: String,
    line: Option<usize>,
    detail: String,
    versions: Vec<String>,
}

impl Finding {
    fn json(&self) -> Value {
        let mut object = Map::new();
        object.insert("kind".into(), json!(self.kind));
        object.insert("ecosystem".into(), json!(self.ecosystem));
        object.insert("package".into(), json!(self.package));
        object.insert("file".into(), json!(self.file));
        if let Some(line) = self.line {
            object.insert("line".into(), json!(line));
        }
        object.insert("detail".into(), json!(self.detail));
        if !self.versions.is_empty() {
            object.insert("versions".into(), json!(self.versions));
        }
        Value::Object(object)
    }
}

/// The declared policy.
#[derive(Debug, Clone, Default)]
struct Policy {
    source: Option<String>,
    allow: Vec<String>,
    deny: Vec<String>,
    allow_unbounded: bool,
    exact: bool,
}

/// One package a file records a license for.
#[derive(Debug, Clone)]
struct Licensed {
    package: String,
    license: String,
    file: String,
    ecosystem: &'static str,
}

/// What reading one file found.
#[derive(Debug, Default)]
struct Scan {
    findings: Vec<Finding>,
    licensed: Vec<Licensed>,
    /// Packages the file holds whose licenses it doesn't record.
    unlicensed: usize,
    /// Packages a lockfile holds.
    packages: usize,
}

#[allow(clippy::too_many_lines)]
fn check(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let root = guest::root(request).ok_or_else(|| Refusal::refused("no granted snapshot"))?;
    let listing = guest::list(host, &root, MAX_LISTED)
        .map_err(|code| Refusal::refused(format!("list {code}")))?;
    let mut files: Vec<(Role, &'static str, String, String)> = listing
        .entries
        .iter()
        .filter(|entry| entry.kind == "file")
        .filter_map(|entry| {
            let path = guest::relative(&entry.name).to_string();
            role_of(&path).map(|(role, ecosystem)| (role, ecosystem, path, entry.handle.clone()))
        })
        .collect();
    // Policy first, then manifests, then lockfiles, so a spent read budget
    // costs the largest files.
    files.sort_by(|a, b| {
        (a.0, a.2.matches('/').count(), &a.2).cmp(&(b.0, b.2.matches('/').count(), &b.2))
    });
    let mut truncated = files.len() > MAX_FILES;
    files.truncate(MAX_FILES);

    let mut texts: Vec<(Role, &'static str, String, String, bool)> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    for (role, ecosystem, path, handle) in &files {
        // Bun's binary lockfile says it is there; its bytes aren't read.
        if path.ends_with("bun.lockb") {
            texts.push((*role, ecosystem, path.clone(), String::new(), true));
            continue;
        }
        match read_some(host, handle, FILE_BYTES) {
            Ok(read) => {
                if !read.complete {
                    truncated = true;
                }
                texts.push((
                    *role,
                    ecosystem,
                    path.clone(),
                    String::from_utf8_lossy(&read.bytes).into_owned(),
                    read.complete,
                ));
            }
            Err(_) => unread.push(path.clone()),
        }
    }

    // The policy: `dependency-policy.toml`, else `deny.toml`.
    let mut policy = Policy::default();
    for wanted in ["dependency-policy.toml", "deny.toml"] {
        if policy.source.is_some() {
            break;
        }
        if let Some((_, _, path, text, _)) = texts
            .iter()
            .find(|(role, _, path, _, _)| *role == Role::Policy && path.ends_with(wanted))
        {
            policy = read_policy(path, text);
        }
    }

    let mut read: Vec<Value> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut licensed: Vec<Licensed> = Vec::new();
    let mut unlicensed: BTreeMap<String, usize> = BTreeMap::new();
    let mut cut: Vec<String> = Vec::new();
    for (role, ecosystem, path, text, complete) in &texts {
        if !complete {
            cut.push(path.clone());
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        let scan = match (*role, name) {
            (Role::Policy, _) => Scan::default(),
            (_, "Cargo.toml") => cargo_manifest(path, text, &policy),
            (_, "Cargo.lock") => toml_lock(path, text, "cargo", *complete),
            (_, "uv.lock" | "poetry.lock") => toml_lock(path, text, "python", *complete),
            (_, "package.json") => npm_manifest(path, text, &policy),
            (_, "package-lock.json" | "npm-shrinkwrap.json") => {
                if *complete {
                    npm_lock(path, text)
                } else {
                    unread.push(path.clone());
                    continue;
                }
            }
            (_, "pnpm-lock.yaml") => pnpm_lock(path, text, *complete),
            (_, "bun.lock") => bun_lock(path, text, *complete),
            // Bun's binary lockfile: present, and nothing in it is read.
            (_, "bun.lockb") => Scan::default(),
            (_, "yarn.lock") => yarn_lock(path, text, *complete),
            (_, "pyproject.toml") => pyproject(path, text, &policy),
            (_, "go.mod") => go_mod(path, text),
            (_, "go.sum") => Scan::default(),
            _ => requirements(path, text, &policy),
        };
        let mut entry = Map::new();
        entry.insert("path".into(), json!(path));
        entry.insert("ecosystem".into(), json!(ecosystem));
        entry.insert(
            "role".into(),
            json!(match role {
                Role::Policy => "policy",
                Role::Manifest => "manifest",
                Role::Lock => "lockfile",
            }),
        );
        if *role == Role::Lock {
            entry.insert("packages".into(), json!(scan.packages));
        }
        entry.insert("complete".into(), json!(complete));
        read.push(Value::Object(entry));
        if scan.unlicensed > 0 {
            *unlicensed.entry(path.clone()).or_default() += scan.unlicensed;
        }
        findings.extend(scan.findings);
        licensed.extend(scan.licensed);
    }

    // A manifest whose ecosystem has no lockfile in the same directory, among
    // every file granted, read or not.
    for (role, ecosystem, path, _) in &files {
        if *role != Role::Manifest || *ecosystem == "python" && !path.ends_with("pyproject.toml") {
            continue;
        }
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        let locked = files.iter().any(|(other_role, other_ecosystem, other, _)| {
            *other_role == Role::Lock
                && other_ecosystem == ecosystem
                && other.rsplit_once('/').map_or("", |(dir, _)| dir) == dir
        });
        // A Cargo workspace member and a monorepo package share the root's
        // lockfile.
        let root_locked = files.iter().any(|(other_role, other_ecosystem, other, _)| {
            *other_role == Role::Lock && other_ecosystem == ecosystem && !other.contains('/')
        });
        if !locked && !root_locked {
            findings.push(Finding {
                kind: "no-lockfile",
                ecosystem,
                package: String::new(),
                file: path.clone(),
                line: None,
                detail: format!("{path} has no lockfile beside it, so installs can resolve to different versions over time"),
                versions: Vec::new(),
            });
        }
    }

    // Licenses against the policy.
    let checks_licenses = !policy.allow.is_empty() || !policy.deny.is_empty();
    if checks_licenses {
        for item in &licensed {
            match license_verdict(&item.license, &policy) {
                Verdict::Allowed => {}
                Verdict::Denied => findings.push(Finding {
                    kind: "license-denied",
                    ecosystem: item.ecosystem,
                    package: item.package.clone(),
                    file: item.file.clone(),
                    line: None,
                    detail: format!(
                        "{} is licensed {}, which the policy denies",
                        item.package, item.license
                    ),
                    versions: Vec::new(),
                }),
                Verdict::NotAllowed => findings.push(Finding {
                    kind: "license-not-allowed",
                    ecosystem: item.ecosystem,
                    package: item.package.clone(),
                    file: item.file.clone(),
                    line: None,
                    detail: format!(
                        "{} is licensed {}, which the policy doesn't allow",
                        item.package, item.license
                    ),
                    versions: Vec::new(),
                }),
            }
        }
    }

    let mut counts = Map::new();
    for finding in &findings {
        let count = counts.entry(finding.kind.to_string()).or_insert(json!(0));
        *count = json!(count.as_u64().unwrap_or(0) + 1);
    }
    let markdown = render(
        &read,
        &findings,
        &policy,
        checks_licenses,
        licensed.len(),
        &unlicensed,
        &cut,
        &unread,
    );
    let listed: Vec<Value> = {
        let mut per_kind: BTreeMap<&str, usize> = BTreeMap::new();
        findings
            .iter()
            .filter(|finding| {
                let seen = per_kind.entry(finding.kind).or_default();
                *seen += 1;
                *seen <= MAX_LISTED_FINDINGS
            })
            .map(Finding::json)
            .collect()
    };
    if listed.len() < findings.len() {
        truncated = true;
    }
    Ok(json!({
        "kind": "dependency-check",
        "files": read,
        "policy": {
            "source": policy.source,
            "allow": policy.allow,
            "deny": policy.deny,
            "allow_unbounded": policy.allow_unbounded,
            "exact": policy.exact,
        },
        "findings": listed,
        "findings_total": findings.len(),
        "counts": counts,
        "licenses": {
            "checked": if checks_licenses { licensed.len() } else { 0 },
            "not_recorded": unlicensed.values().sum::<usize>(),
        },
        "unread": unread,
        "truncated": truncated,
        "markdown": markdown,
    }))
}

/// The bytes one read call asks for: small, so a nearly spent read budget
/// still yields the part of a file that fits.
const CHUNK: usize = 4 * 1024;

/// Read at most `max_bytes` of a file from offset zero in [`CHUNK`]-sized
/// calls. A budget that runs out after some bytes were read returns those
/// bytes, marked incomplete.
fn read_some(host: &mut dyn Host, handle: &str, max_bytes: usize) -> Result<guest::Read, i32> {
    let total = guest::size(host, handle).map_or(usize::MAX, |size| {
        usize::try_from(size).unwrap_or(usize::MAX)
    });
    let mut read = guest::Read::default();
    if total == 0 {
        read.complete = true;
        return Ok(read);
    }
    loop {
        let chunk = CHUNK
            .min(max_bytes.saturating_sub(read.bytes.len()))
            .min(total.saturating_sub(read.bytes.len()));
        if chunk == 0 {
            read.complete = read.bytes.len() >= total;
            return Ok(read);
        }
        let answer = host.call(&json!({
            "v": 1,
            "handle": handle,
            "operation": "read",
            "args": {"offset": read.bytes.len(), "max_bytes": chunk}
        }));
        let answer = match answer {
            Ok(answer) => answer,
            Err(code) if read.bytes.is_empty() => return Err(code),
            Err(_) => return Ok(read),
        };
        let bytes = answer["bytes_base64"]
            .as_str()
            .map_or(Some(Vec::new()), guest::decode_base64)
            .ok_or(guest::MALFORMED)?;
        let eof = answer["eof"].as_bool().unwrap_or(true);
        let empty = bytes.is_empty();
        read.bytes.extend_from_slice(&bytes);
        if eof {
            read.complete = true;
            return Ok(read);
        }
        if empty {
            return Ok(read);
        }
    }
}

// ---------------------------------------------------------------------------
// A small TOML reader: tables, keys, strings, arrays, inline tables
// ---------------------------------------------------------------------------

/// A TOML value as this guest needs it.
#[derive(Debug, Clone, PartialEq)]
enum Toml {
    Text(String),
    Other(String),
    List(Vec<Toml>),
    Table(Vec<(String, Toml)>),
}

impl Toml {
    fn text(&self) -> Option<&str> {
        match self {
            Toml::Text(text) => Some(text),
            _ => None,
        }
    }

    fn get(&self, key: &str) -> Option<&Toml> {
        match self {
            Toml::Table(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn texts(&self) -> Vec<String> {
        match self {
            Toml::List(items) => items
                .iter()
                .filter_map(|item| item.text().map(str::to_string))
                .collect(),
            Toml::Text(text) => vec![text.clone()],
            _ => Vec::new(),
        }
    }
}

/// One `key = value` line, with the table it sits in and its line number.
#[derive(Debug, Clone)]
struct Pair {
    table: String,
    key: String,
    value: Toml,
    line: usize,
}

/// Every `key = value` in a TOML file, and the start line of each
/// `[[array]]` table. A value that spans lines (an array) is read whole.
fn toml_pairs(text: &str) -> (Vec<Pair>, Vec<(String, usize)>) {
    let mut pairs = Vec::new();
    let mut arrays = Vec::new();
    let mut table = String::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut at = 0;
    while at < lines.len() {
        let line = strip_comment(lines[at]).trim().to_string();
        let number = at + 1;
        at += 1;
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix("[[")
            .and_then(|rest| rest.strip_suffix("]]"))
        {
            table = unquote_key(name.trim());
            arrays.push((table.clone(), number));
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            table = unquote_key(name.trim());
            continue;
        }
        let Some((key, value)) = split_pair(&line) else {
            continue;
        };
        let mut value = value.to_string();
        while depth(&value) > 0 && at < lines.len() {
            value.push(' ');
            value.push_str(strip_comment(lines[at]).trim());
            at += 1;
        }
        let parsed = parse_value(&mut value.trim().chars().peekable());
        pairs.push(Pair {
            table: table.clone(),
            key: unquote_key(key.trim()),
            value: parsed,
            line: number,
        });
    }
    (pairs, arrays)
}

/// A table or key name without its quotes, dotted parts joined by `.`.
fn unquote_key(name: &str) -> String {
    name.split('.')
        .map(|part| part.trim().trim_matches('"').trim_matches('\''))
        .collect::<Vec<_>>()
        .join(".")
}

/// The line before a `#` that isn't inside a string.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (index, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            (None, '#') => return &line[..index],
            _ => {}
        }
    }
    line
}

/// `key = value` split at the first `=` outside quotes.
fn split_pair(line: &str) -> Option<(&str, &str)> {
    let mut quote: Option<char> = None;
    for (index, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            (None, '=') => return Some((&line[..index], &line[index + 1..])),
            _ => {}
        }
    }
    None
}

/// How many brackets and braces a value leaves open.
fn depth(value: &str) -> i32 {
    let mut quote: Option<char> = None;
    let mut open = 0;
    for c in value.chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '[' | '{') => open += 1,
            (None, ']' | '}') => open -= 1,
            _ => {}
        }
    }
    open
}

fn parse_value(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Toml {
    while chars.peek().is_some_and(|c| c.is_whitespace()) {
        chars.next();
    }
    match chars.peek().copied() {
        Some(quote @ ('"' | '\'')) => {
            chars.next();
            let mut text = String::new();
            while let Some(c) = chars.next() {
                if c == quote {
                    break;
                }
                if c == '\\' && quote == '"' {
                    if let Some(escaped) = chars.next() {
                        text.push(escaped);
                    }
                    continue;
                }
                text.push(c);
            }
            Toml::Text(text)
        }
        Some('[') => {
            chars.next();
            let mut items = Vec::new();
            loop {
                while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
                    chars.next();
                }
                match chars.peek() {
                    None => break,
                    Some(']') => {
                        chars.next();
                        break;
                    }
                    _ => items.push(parse_value(chars)),
                }
            }
            Toml::List(items)
        }
        Some('{') => {
            chars.next();
            let mut entries = Vec::new();
            loop {
                while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
                    chars.next();
                }
                match chars.peek() {
                    None => break,
                    Some('}') => {
                        chars.next();
                        break;
                    }
                    _ => {
                        let mut key = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '=' {
                                chars.next();
                                break;
                            }
                            key.push(c);
                            chars.next();
                        }
                        let value = parse_value(chars);
                        entries.push((unquote_key(key.trim()), value));
                    }
                }
            }
            Toml::Table(entries)
        }
        _ => {
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if matches!(c, ',' | ']' | '}') {
                    break;
                }
                text.push(c);
                chars.next();
            }
            Toml::Other(text.trim().to_string())
        }
    }
}

fn read_policy(path: &str, text: &str) -> Policy {
    let (pairs, _) = toml_pairs(text);
    let mut policy = Policy {
        source: Some(path.to_string()),
        ..Policy::default()
    };
    for pair in &pairs {
        match (pair.table.as_str(), pair.key.as_str()) {
            ("licenses", "allow") => policy.allow = pair.value.texts(),
            ("licenses", "deny") => policy.deny = pair.value.texts(),
            ("ranges", "allow_unbounded") => {
                policy.allow_unbounded = pair.value == Toml::Other("true".into());
            }
            ("ranges", "exact") => policy.exact = pair.value == Toml::Other("true".into()),
            _ => {}
        }
    }
    policy
}

// ---------------------------------------------------------------------------
// Version requirements
// ---------------------------------------------------------------------------

/// How loose a version requirement is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Looseness {
    /// Any version at all.
    Any,
    /// A lower bound and no upper bound.
    Unbounded,
    /// Bounded, but not one exact version.
    Range,
    /// Exactly one version.
    Exact,
}

/// A Cargo, npm, or PEP 440 requirement's looseness. Alternatives
/// (`||`) are as loose as the loosest.
fn looseness(requirement: &str) -> Looseness {
    let requirement = requirement.trim();
    if requirement.contains("||") {
        return requirement
            .split("||")
            .map(looseness)
            .min_by_key(|l| match l {
                Looseness::Any => 0,
                Looseness::Unbounded => 1,
                Looseness::Range => 2,
                Looseness::Exact => 3,
            })
            .unwrap_or(Looseness::Any);
    }
    if matches!(requirement, "" | "*" | "x" | "X" | "latest" | "next") {
        return Looseness::Any;
    }
    // npm's hyphen range `1.2.3 - 2.0.0` is bounded.
    if requirement.contains(" - ") {
        return Looseness::Range;
    }
    let parts: Vec<&str> = requirement
        .split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let lower_only = parts
        .iter()
        .all(|part| part.starts_with(">=") || (part.starts_with('>') && !part.starts_with(">=")));
    if lower_only {
        return Looseness::Unbounded;
    }
    // A bare `1.2.3` is exact to npm and a caret range to Cargo; the
    // caller decides for Cargo.
    if parts.len() == 1 {
        let part = parts[0];
        let exact = part.starts_with("==") && !part.contains('*')
            || part.starts_with("===")
            || (part.starts_with('=') && !part.starts_with("=="))
            || part.chars().next().is_some_and(|c| c.is_ascii_digit())
                && part.matches('.').count() == 2
                && !part.contains(['*', 'x', 'X']);
        if exact {
            return Looseness::Exact;
        }
    }
    Looseness::Range
}

// ---------------------------------------------------------------------------
// Cargo
// ---------------------------------------------------------------------------

fn is_dependency_table(table: &str) -> Option<&str> {
    let last = table.rsplit('.').next().unwrap_or(table);
    if matches!(
        last,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    ) {
        return Some(table);
    }
    None
}

fn cargo_manifest(path: &str, text: &str, policy: &Policy) -> Scan {
    let (pairs, _) = toml_pairs(text);
    let mut scan = Scan::default();
    let mut dotted: BTreeMap<String, (usize, Vec<(String, Toml)>)> = BTreeMap::new();
    for pair in &pairs {
        if pair.table == "package"
            && pair.key == "license"
            && let Some(license) = pair.value.text()
        {
            let name = pairs
                .iter()
                .find(|p| p.table == "package" && p.key == "name")
                .and_then(|p| p.value.text())
                .unwrap_or("this package");
            scan.licensed.push(Licensed {
                package: name.to_string(),
                license: license.to_string(),
                file: path.to_string(),
                ecosystem: "cargo",
            });
        }
        if is_dependency_table(&pair.table).is_some() {
            cargo_dependency(path, &pair.key, &pair.value, pair.line, policy, &mut scan);
            continue;
        }
        // `[dependencies.serde]` with `version = "..."` inside.
        if let Some((table, name)) = pair.table.rsplit_once('.')
            && is_dependency_table(table).is_some()
        {
            let entry = dotted
                .entry(name.to_string())
                .or_insert((pair.line, Vec::new()));
            entry.1.push((pair.key.clone(), pair.value.clone()));
        }
    }
    for (name, (line, entries)) in dotted {
        cargo_dependency(path, &name, &Toml::Table(entries), line, policy, &mut scan);
    }
    scan
}

fn cargo_dependency(
    path: &str,
    name: &str,
    value: &Toml,
    line: usize,
    policy: &Policy,
    scan: &mut Scan,
) {
    let (requirement, table) = match value {
        Toml::Text(requirement) => (Some(requirement.as_str()), None),
        Toml::Table(_) => (value.get("version").and_then(Toml::text), Some(value)),
        _ => return,
    };
    if let Some(table) = table {
        if table.get("path").is_some() || table.get("workspace").is_some() {
            return;
        }
        if let Some(git) = table.get("git").and_then(Toml::text) {
            if table.get("rev").is_none() && table.get("tag").is_none() {
                scan.findings.push(Finding {
                    kind: "unpinned-git",
                    ecosystem: "cargo",
                    package: name.to_string(),
                    file: path.to_string(),
                    line: Some(line),
                    detail: format!(
                        "{name} comes from {git}{} without a `rev` or `tag`, so it follows whatever the branch holds",
                        table.get("branch").and_then(Toml::text).map(|b| format!(" (branch {b})")).unwrap_or_default()
                    ),
                    versions: Vec::new(),
                });
            }
            return;
        }
    }
    let Some(requirement) = requirement else {
        return;
    };
    // Cargo reads `1.2.3` as `^1.2.3`; only `=1.2.3` is exact.
    let looseness = match looseness(requirement) {
        Looseness::Exact if !requirement.trim().starts_with('=') => Looseness::Range,
        other => other,
    };
    push_looseness(
        scan,
        "cargo",
        path,
        name,
        requirement,
        Some(line),
        looseness,
        policy,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_looseness(
    scan: &mut Scan,
    ecosystem: &'static str,
    path: &str,
    name: &str,
    requirement: &str,
    line: Option<usize>,
    looseness: Looseness,
    policy: &Policy,
) {
    let shown = if requirement.trim().is_empty() {
        "no version".to_string()
    } else {
        format!("`{}`", requirement.trim())
    };
    let (kind, detail) = match looseness {
        Looseness::Any => (
            "any-version",
            format!("{name} asks for {shown}: any version can arrive"),
        ),
        Looseness::Unbounded if !policy.allow_unbounded => (
            "unbounded",
            format!(
                "{name} asks for {shown}: a lower bound with no upper bound, so the next major version can arrive"
            ),
        ),
        Looseness::Range if policy.exact => (
            "not-exact",
            format!("{name} asks for {shown}, and the policy wants exact versions"),
        ),
        _ => return,
    };
    scan.findings.push(Finding {
        kind,
        ecosystem,
        package: name.to_string(),
        file: path.to_string(),
        line,
        detail,
        versions: Vec::new(),
    });
}

/// `[[package]]` tables with `name` and `version`: Cargo, uv, and Poetry
/// lockfiles. A file cut short loses its last, partial package.
fn toml_lock(path: &str, text: &str, ecosystem: &'static str, complete: bool) -> Scan {
    let (pairs, arrays) = toml_pairs(text);
    let mut packages: Vec<(String, String)> = Vec::new();
    for (index, (table, start)) in arrays.iter().enumerate() {
        if table != "package" {
            continue;
        }
        let end = arrays.get(index + 1).map_or(usize::MAX, |(_, next)| *next);
        if !complete && end == usize::MAX {
            break;
        }
        let field = |key: &str| {
            pairs
                .iter()
                .find(|pair| {
                    pair.line > *start
                        && pair.line < end
                        && pair.table == "package"
                        && pair.key == key
                })
                .and_then(|pair| pair.value.text())
                .map(str::to_string)
        };
        if let (Some(name), Some(version)) = (field("name"), field("version")) {
            packages.push((name, version));
        }
    }
    let mut scan = duplicates(path, ecosystem, &packages);
    scan.unlicensed = packages.len();
    scan
}

/// The packages a lockfile holds at more than one version.
fn duplicates(path: &str, ecosystem: &'static str, packages: &[(String, String)]) -> Scan {
    let mut versions: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, version) in packages {
        let held = versions.entry(name.as_str()).or_default();
        if !held.contains(&version.as_str()) {
            held.push(version.as_str());
        }
    }
    let mut findings: Vec<Finding> = versions
        .into_iter()
        .filter(|(_, held)| held.len() > 1)
        .map(|(name, held)| {
            let mut held: Vec<String> = held.into_iter().map(str::to_string).collect();
            held.sort_by_key(|version| version_key(version));
            Finding {
                kind: "duplicate",
                ecosystem,
                package: name.to_string(),
                file: path.to_string(),
                line: None,
                detail: format!(
                    "{name} is in the lockfile at {} versions: {}",
                    held.len(),
                    held.join(", ")
                ),
                versions: held,
            }
        })
        .collect();
    findings.sort_by(|a, b| {
        b.versions
            .len()
            .cmp(&a.versions.len())
            .then(a.package.cmp(&b.package))
    });
    Scan {
        findings,
        packages: packages.len(),
        ..Scan::default()
    }
}

/// A version's numeric parts, for ordering.
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

// ---------------------------------------------------------------------------
// npm, pnpm, Yarn
// ---------------------------------------------------------------------------

fn npm_manifest(path: &str, text: &str, policy: &Policy) -> Scan {
    let mut scan = Scan::default();
    let Ok(manifest) = serde_json::from_str::<Value>(text) else {
        return scan;
    };
    if let Some(license) = manifest["license"].as_str() {
        scan.licensed.push(Licensed {
            package: manifest["name"]
                .as_str()
                .unwrap_or("this package")
                .to_string(),
            license: license.to_string(),
            file: path.to_string(),
            ecosystem: "npm",
        });
    }
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
        let Some(dependencies) = manifest[section].as_object() else {
            continue;
        };
        for (name, requirement) in dependencies {
            let Some(requirement) = requirement.as_str() else {
                continue;
            };
            let line = text
                .lines()
                .position(|line| line.trim_start().starts_with(&format!("\"{name}\"")))
                .map(|index| index + 1);
            let requirement = requirement.trim();
            if ["file:", "link:", "workspace:", "portal:"]
                .iter()
                .any(|prefix| requirement.starts_with(prefix))
            {
                continue;
            }
            let source = requirement.starts_with("git")
                || requirement.starts_with("http")
                || requirement.starts_with("github:")
                || (requirement.contains('/')
                    && !requirement.starts_with('@')
                    && !requirement.starts_with("npm:"));
            if source {
                let pinned = requirement.rsplit_once('#').is_some_and(|(_, fragment)| {
                    fragment.len() >= 7 && fragment.chars().all(|c| c.is_ascii_hexdigit())
                });
                if !pinned {
                    scan.findings.push(Finding {
                        kind: "unpinned-git",
                        ecosystem: "npm",
                        package: name.clone(),
                        file: path.to_string(),
                        line,
                        detail: format!("{name} comes from `{requirement}` without a commit hash, so it follows whatever the branch holds"),
                        versions: Vec::new(),
                    });
                }
                continue;
            }
            let range = requirement
                .strip_prefix("npm:")
                .and_then(|alias| alias.rsplit_once('@').map(|(_, range)| range))
                .unwrap_or(requirement);
            push_looseness(
                &mut scan,
                "npm",
                path,
                name,
                range,
                line,
                looseness(range),
                policy,
            );
        }
    }
    scan
}

/// The package name in a `node_modules/...` path.
fn module_name(key: &str) -> Option<String> {
    let rest = key.rsplit("node_modules/").next()?;
    if rest.is_empty() || rest == key && !key.contains("node_modules/") {
        return None;
    }
    Some(rest.to_string())
}

fn npm_lock(path: &str, text: &str) -> Scan {
    let Ok(lock) = serde_json::from_str::<Value>(text) else {
        return Scan::default();
    };
    let mut packages: Vec<(String, String)> = Vec::new();
    let mut licensed: Vec<Licensed> = Vec::new();
    let mut unlicensed = 0;
    if let Some(entries) = lock["packages"].as_object() {
        for (key, entry) in entries {
            if key.is_empty() || entry["link"].as_bool() == Some(true) {
                continue;
            }
            let Some(name) = entry["name"]
                .as_str()
                .map(str::to_string)
                .or_else(|| module_name(key))
            else {
                continue;
            };
            let Some(version) = entry["version"].as_str() else {
                continue;
            };
            packages.push((name.clone(), version.to_string()));
            match entry["license"]
                .as_str()
                .or_else(|| entry["license"]["type"].as_str())
            {
                Some(license) => licensed.push(Licensed {
                    package: format!("{name}@{version}"),
                    license: license.to_string(),
                    file: path.to_string(),
                    ecosystem: "npm",
                }),
                None => unlicensed += 1,
            }
        }
    } else if let Some(dependencies) = lock["dependencies"].as_object() {
        fn walk(dependencies: &Map<String, Value>, out: &mut Vec<(String, String)>) {
            for (name, entry) in dependencies {
                if let Some(version) = entry["version"].as_str() {
                    out.push((name.clone(), version.to_string()));
                }
                if let Some(nested) = entry["dependencies"].as_object() {
                    walk(nested, out);
                }
            }
        }
        walk(dependencies, &mut packages);
        unlicensed = packages.len();
    }
    let mut scan = duplicates(path, "npm", &packages);
    scan.licensed = licensed;
    scan.unlicensed = unlicensed;
    scan
}

/// `name@version` with a leading `@` scope kept together.
fn split_at_version(spec: &str) -> Option<(String, String)> {
    let at = spec
        .char_indices()
        .skip(1)
        .filter(|(_, c)| *c == '@')
        .map(|(i, _)| i)
        .last()?;
    let (name, version) = (&spec[..at], &spec[at + 1..]);
    (!name.is_empty() && !version.is_empty()).then(|| (name.to_string(), version.to_string()))
}

fn pnpm_lock(path: &str, text: &str, complete: bool) -> Scan {
    let mut packages: Vec<(String, String)> = Vec::new();
    let mut in_packages = false;
    let lines: Vec<&str> = text.lines().collect();
    let usable = if complete {
        lines.len()
    } else {
        lines.len().saturating_sub(1)
    };
    for line in &lines[..usable] {
        if !line.starts_with(' ') && !line.is_empty() {
            in_packages = line.trim_end() == "packages:";
            continue;
        }
        if !in_packages || !line.starts_with("  ") || line.starts_with("   ") {
            continue;
        }
        let key = line.trim().trim_end_matches(':').trim_matches(['\'', '"']);
        let key = key.strip_prefix('/').unwrap_or(key);
        let key = key.split('(').next().unwrap_or(key);
        // pnpm 5: `/name/1.2.3_peer`; pnpm 6 and later: `name@1.2.3`.
        let parsed = split_at_version(key).or_else(|| {
            key.rsplit_once('/')
                .filter(|(_, version)| version.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .map(|(name, version)| {
                    (
                        name.to_string(),
                        version.split('_').next().unwrap_or(version).to_string(),
                    )
                })
        });
        if let Some((name, version)) = parsed {
            packages.push((name, version));
        }
    }
    let mut scan = duplicates(path, "npm", &packages);
    scan.unlicensed = packages.len();
    scan
}

/// Bun's text lockfile: under `"packages"`, each entry's array starts with
/// `"name@version"`.
fn bun_lock(path: &str, text: &str, complete: bool) -> Scan {
    let mut packages: Vec<(String, String)> = Vec::new();
    let mut in_packages = false;
    let lines: Vec<&str> = text.lines().collect();
    let usable = if complete {
        lines.len()
    } else {
        lines.len().saturating_sub(1)
    };
    for line in &lines[..usable] {
        let trimmed = line.trim();
        if trimmed.starts_with("\"packages\"") {
            in_packages = true;
            continue;
        }
        if !in_packages {
            continue;
        }
        let Some((_, array)) = trimmed.split_once("\": [\"") else {
            continue;
        };
        let Some(spec) = array.split('"').next() else {
            continue;
        };
        if let Some((name, version)) = split_at_version(spec) {
            packages.push((name, version));
        }
    }
    let mut scan = duplicates(path, "npm", &packages);
    scan.unlicensed = packages.len();
    scan
}

fn yarn_lock(path: &str, text: &str, complete: bool) -> Scan {
    let mut packages: Vec<(String, String)> = Vec::new();
    let mut name: Option<String> = None;
    let lines: Vec<&str> = text.lines().collect();
    let usable = if complete {
        lines.len()
    } else {
        lines.len().saturating_sub(1)
    };
    for line in &lines[..usable] {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            let first = line
                .trim_end_matches(':')
                .split(", ")
                .next()
                .unwrap_or_default()
                .trim_matches('"');
            name = split_at_version(first).map(|(name, _)| name);
            continue;
        }
        let trimmed = line.trim();
        if let Some(version) = trimmed
            .strip_prefix("version ")
            .or_else(|| trimmed.strip_prefix("version: "))
            && let Some(name) = &name
        {
            packages.push((name.clone(), version.trim_matches('"').to_string()));
        }
    }
    let mut scan = duplicates(path, "npm", &packages);
    scan.unlicensed = packages.len();
    scan
}

// ---------------------------------------------------------------------------
// Python and Go
// ---------------------------------------------------------------------------

/// A PEP 508 requirement's name and version part.
fn pep508(requirement: &str) -> Option<(String, String)> {
    let requirement = requirement.split(';').next()?.trim();
    if requirement.is_empty() || requirement.starts_with(['-', '#']) {
        return None;
    }
    let end = requirement
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(requirement.len());
    let name = &requirement[..end];
    let rest = requirement[end..].trim();
    let rest = if rest.starts_with('[') {
        rest.split_once(']').map_or("", |(_, after)| after.trim())
    } else {
        rest
    };
    let rest = rest.trim_start_matches('(').trim_end_matches(')').trim();
    (!name.is_empty()).then(|| (name.to_string(), rest.to_string()))
}

fn python_requirement(
    scan: &mut Scan,
    path: &str,
    requirement: &str,
    line: Option<usize>,
    policy: &Policy,
) {
    if let Some((name, url)) = requirement.split_once(" @ ") {
        let pinned = url
            .rsplit_once('@')
            .is_some_and(|(_, rev)| rev.len() >= 7 && rev.chars().all(|c| c.is_ascii_hexdigit()));
        if url.contains("git") && !pinned {
            scan.findings.push(Finding {
                kind: "unpinned-git",
                ecosystem: "python",
                package: name.trim().to_string(),
                file: path.to_string(),
                line,
                detail: format!(
                    "{} comes from `{}` without a commit hash",
                    name.trim(),
                    url.trim()
                ),
                versions: Vec::new(),
            });
        }
        return;
    }
    let Some((name, spec)) = pep508(requirement) else {
        return;
    };
    push_looseness(
        scan,
        "python",
        path,
        &name,
        &spec,
        line,
        looseness(&spec),
        policy,
    );
}

fn requirements(path: &str, text: &str, policy: &Policy) -> Scan {
    let mut scan = Scan::default();
    for (index, line) in text.lines().enumerate() {
        let line = strip_comment(line).trim();
        if line.is_empty() || line.starts_with('-') {
            continue;
        }
        if line.starts_with("git+") {
            if !line.contains('@') {
                scan.findings.push(Finding {
                    kind: "unpinned-git",
                    ecosystem: "python",
                    package: line.rsplit('/').next().unwrap_or(line).to_string(),
                    file: path.to_string(),
                    line: Some(index + 1),
                    detail: format!("`{line}` names no commit"),
                    versions: Vec::new(),
                });
            }
            continue;
        }
        python_requirement(&mut scan, path, line, Some(index + 1), policy);
    }
    scan
}

fn pyproject(path: &str, text: &str, policy: &Policy) -> Scan {
    let (pairs, _) = toml_pairs(text);
    let mut scan = Scan::default();
    for pair in &pairs {
        match (pair.table.as_str(), pair.key.as_str()) {
            ("project", "license") | ("tool.poetry", "license") => {
                let license = pair.value.text().map(str::to_string).or_else(|| {
                    pair.value
                        .get("text")
                        .and_then(Toml::text)
                        .map(str::to_string)
                });
                if let Some(license) = license {
                    let name = pairs
                        .iter()
                        .find(|p| {
                            (p.table == "project" || p.table == "tool.poetry") && p.key == "name"
                        })
                        .and_then(|p| p.value.text())
                        .unwrap_or("this project");
                    scan.licensed.push(Licensed {
                        package: name.to_string(),
                        license,
                        file: path.to_string(),
                        ecosystem: "python",
                    });
                }
            }
            ("project", "dependencies") => {
                for requirement in pair.value.texts() {
                    python_requirement(&mut scan, path, &requirement, Some(pair.line), policy);
                }
            }
            ("project.optional-dependencies" | "dependency-groups", _) => {
                for requirement in pair.value.texts() {
                    python_requirement(&mut scan, path, &requirement, Some(pair.line), policy);
                }
            }
            ("tool.poetry.dependencies" | "tool.poetry.dev-dependencies", name)
                if name != "python" =>
            {
                let requirement = pair.value.text().map(str::to_string).or_else(|| {
                    pair.value
                        .get("version")
                        .and_then(Toml::text)
                        .map(str::to_string)
                });
                if let Some(requirement) = requirement {
                    // Poetry's `^1.2` and `~1.2` are bounded.
                    push_looseness(
                        &mut scan,
                        "python",
                        path,
                        name,
                        &requirement,
                        Some(pair.line),
                        looseness(&requirement),
                        policy,
                    );
                }
            }
            _ => {}
        }
    }
    scan
}

fn go_mod(path: &str, text: &str) -> Scan {
    let mut scan = Scan::default();
    let mut required: Vec<(String, String, usize)> = Vec::new();
    let mut in_block = false;
    for (index, line) in text.lines().enumerate() {
        let line = line.split("//").next().unwrap_or(line).trim();
        if line == "require (" {
            in_block = true;
            continue;
        }
        if in_block && line == ")" {
            in_block = false;
            continue;
        }
        let entry = if in_block {
            Some(line)
        } else {
            line.strip_prefix("require ")
        };
        if let Some(entry) = entry {
            let mut parts = entry.split_whitespace();
            if let (Some(module), Some(version)) = (parts.next(), parts.next()) {
                required.push((module.to_string(), version.to_string(), index + 1));
            }
        }
    }
    let base = |module: &str| -> String {
        match module.rsplit_once('/') {
            Some((base, last))
                if last.len() > 1
                    && last.starts_with('v')
                    && last[1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                base.to_string()
            }
            _ => module.to_string(),
        }
    };
    let mut by_base: BTreeMap<String, Vec<(String, String, usize)>> = BTreeMap::new();
    for (module, version, line) in required {
        by_base
            .entry(base(&module))
            .or_default()
            .push((module, version, line));
    }
    for (base, modules) in by_base {
        if modules.len() > 1 {
            scan.findings.push(Finding {
                kind: "two-majors",
                ecosystem: "go",
                package: base.clone(),
                file: path.to_string(),
                line: modules.first().map(|(_, _, line)| *line),
                detail: format!(
                    "{base} is required at {} major versions: {}",
                    modules.len(),
                    modules
                        .iter()
                        .map(|(module, version, _)| format!("{module} {version}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                versions: modules
                    .iter()
                    .map(|(_, version, _)| version.clone())
                    .collect(),
            });
        }
    }
    scan
}

// ---------------------------------------------------------------------------
// Licenses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Allowed,
    NotAllowed,
    Denied,
}

/// An SPDX expression against the policy: `OR` needs one acceptable
/// side, `AND` needs both, `WITH` reads as its license.
fn license_verdict(expression: &str, policy: &Policy) -> Verdict {
    let tokens: Vec<String> = expression
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let mut at = 0;
    let verdict = or_expression(&tokens, &mut at, policy);
    verdict.unwrap_or(Verdict::NotAllowed)
}

fn rank(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::Allowed => 2,
        Verdict::NotAllowed => 1,
        Verdict::Denied => 0,
    }
}

fn or_expression(tokens: &[String], at: &mut usize, policy: &Policy) -> Option<Verdict> {
    let mut best = and_expression(tokens, at, policy)?;
    while tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("OR"))
    {
        *at += 1;
        let next = and_expression(tokens, at, policy)?;
        if rank(next) > rank(best) {
            best = next;
        }
    }
    Some(best)
}

fn and_expression(tokens: &[String], at: &mut usize, policy: &Policy) -> Option<Verdict> {
    let mut worst = atom(tokens, at, policy)?;
    while tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("AND"))
    {
        *at += 1;
        let next = atom(tokens, at, policy)?;
        if rank(next) < rank(worst) {
            worst = next;
        }
    }
    Some(worst)
}

fn atom(tokens: &[String], at: &mut usize, policy: &Policy) -> Option<Verdict> {
    let token = tokens.get(*at)?.clone();
    *at += 1;
    if token == "(" {
        let inner = or_expression(tokens, at, policy)?;
        if tokens.get(*at).map(String::as_str) == Some(")") {
            *at += 1;
        }
        return Some(inner);
    }
    if tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("WITH"))
    {
        *at += 2;
    }
    let id = token.trim_end_matches('+');
    let named = |list: &[String]| {
        list.iter().any(|known| {
            let known = known.trim_end_matches('+');
            known.eq_ignore_ascii_case(id)
                || known.eq_ignore_ascii_case(&format!("{id}-only"))
                || id.eq_ignore_ascii_case(&format!("{known}-only"))
                || id.eq_ignore_ascii_case(&format!("{known}-or-later"))
        })
    };
    Some(if named(&policy.deny) {
        Verdict::Denied
    } else if policy.allow.is_empty() || named(&policy.allow) {
        Verdict::Allowed
    } else {
        Verdict::NotAllowed
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render(
    read: &[Value],
    findings: &[Finding],
    policy: &Policy,
    checks_licenses: bool,
    licensed: usize,
    unlicensed: &BTreeMap<String, usize>,
    cut: &[String],
    unread: &[String],
) -> String {
    let mut out = String::from("## Dependency check\n\n");
    if read.iter().all(|file| file["role"] == "policy") {
        out.push_str("I found no manifest or lockfile to check here. I read Cargo, npm, pnpm, Yarn, Python, and Go files by their usual names, at the project root or wherever the request names them.\n");
        return out;
    }
    let files: Vec<String> = read
        .iter()
        .filter(|file| file["role"] != "policy")
        .map(|file| match file["packages"].as_u64() {
            Some(count) => format!(
                "`{}` ({count} packages)",
                file["path"].as_str().unwrap_or_default()
            ),
            None => format!("`{}`", file["path"].as_str().unwrap_or_default()),
        })
        .collect();
    out.push_str(&format!("Read {}.", files.join(", ")));
    match &policy.source {
        Some(source) => out.push_str(&format!(
            " Policy: `{source}` (allows {} license{}, denies {}); {licensed} license{} checked.\n",
            policy.allow.len(),
            if policy.allow.len() == 1 { "" } else { "s" },
            policy.deny.len(),
            if licensed == 1 { "" } else { "s" }
        )),
        None => out
            .push_str(" No `dependency-policy.toml` or `deny.toml`, so licenses aren't checked.\n"),
    }
    let sections: [(&str, &[&str]); 4] = [
        (
            "Licenses the policy doesn't allow",
            &["license-denied", "license-not-allowed"],
        ),
        (
            "Loose or unpinned versions",
            &["any-version", "unbounded", "unpinned-git", "not-exact"],
        ),
        ("Duplicate versions", &["duplicate", "two-majors"]),
        ("No lockfile", &["no-lockfile"]),
    ];
    let mut any = false;
    for (title, kinds) in sections {
        let members: Vec<&Finding> = findings
            .iter()
            .filter(|f| kinds.contains(&f.kind))
            .collect();
        if members.is_empty() {
            continue;
        }
        any = true;
        out.push_str(&format!("\n### {title} ({})\n\n", members.len()));
        for finding in members.iter().take(MAX_LISTED_FINDINGS) {
            let place = match finding.line {
                Some(line) => format!("`{}` line {line}", finding.file),
                None => format!("`{}`", finding.file),
            };
            out.push_str(&format!("- {} ({place})\n", finding.detail));
        }
        if members.len() > MAX_LISTED_FINDINGS {
            out.push_str(&format!(
                "- and {} more\n",
                members.len() - MAX_LISTED_FINDINGS
            ));
        }
    }
    if !any {
        out.push_str("\nNothing to flag: no duplicate versions, no loose or unpinned versions");
        out.push_str(if checks_licenses {
            ", and no license the policy rejects.\n"
        } else {
            ".\n"
        });
    }
    let mut not_checked: Vec<String> = Vec::new();
    if checks_licenses {
        for (file, count) in unlicensed {
            not_checked.push(format!(
                "`{file}` doesn't record licenses, so its {count} packages weren't checked."
            ));
        }
    }
    for file in cut {
        not_checked.push(format!(
            "`{file}` was cut at the read limit; what's past the cut isn't counted."
        ));
    }
    for file in unread {
        not_checked.push(format!("`{file}` couldn't be read whole."));
    }
    if !not_checked.is_empty() {
        out.push_str("\n### Not checked\n\n");
        for line in not_checked {
            out.push_str(&format!("- {line}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn run(host: &mut MemoryHost) -> Value {
        check(&MemoryHost::request("check", json!({})), host).unwrap()
    }

    fn kinds(value: &Value, kind: &str) -> Vec<String> {
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["kind"] == kind)
            .map(|finding| finding["package"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_mixed_project_is_flagged_against_its_policy() {
        let mut host =
            MemoryHost::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tree"));
        let value = run(&mut host);
        assert_eq!(value["policy"]["source"], "dependency-policy.toml");
        assert_eq!(kinds(&value, "duplicate"), ["syn", "left-pad"]);
        assert_eq!(kinds(&value, "any-version"), ["rand", "chalk", "flask"]);
        assert_eq!(kinds(&value, "unbounded"), ["anyhow", "lodash", "requests"]);
        assert_eq!(kinds(&value, "unpinned-git"), ["ledger-core", "acme-ui"]);
        assert_eq!(kinds(&value, "license-denied"), ["gpl-widget@2.0.0"]);
        assert_eq!(kinds(&value, "license-not-allowed"), ["wtf-lib@1.0.0"]);
        assert_eq!(value["licenses"]["not_recorded"], 4);
        let markdown = value["markdown"].as_str().unwrap();
        assert!(
            markdown.contains("syn is in the lockfile at 2 versions: 1.0.109, 2.0.77"),
            "{markdown}"
        );
        assert!(
            markdown.contains("gpl-widget@2.0.0 is licensed GPL-3.0-only, which the policy denies"),
            "{markdown}"
        );
        assert!(
            markdown.contains("`Cargo.lock` doesn't record licenses"),
            "{markdown}"
        );
    }

    #[test]
    fn pnpm_yarn_and_go_files_are_read() {
        let mut host = MemoryHost::new([
            (
                "pnpm-lock.yaml",
                "lockfileVersion: '9.0'\n\nimporters:\n  .:\n    dependencies: {}\n\npackages:\n\n  '@types/node@20.1.0':\n    resolution: {integrity: x}\n\n  '@types/node@22.4.1':\n    resolution: {integrity: y}\n\n  react@18.3.1(react-dom@18.3.1):\n    resolution: {integrity: z}\n\nsnapshots:\n\n  react@17.0.0:\n    dependencies: {}\n",
            ),
            (
                "yarn.lock",
                "# yarn lockfile v1\n\n\"debug@^2.6.9\":\n  version \"2.6.9\"\n\ndebug@^4.3.4, debug@^4.1.0:\n  version \"4.3.4\"\n",
            ),
            (
                "bun.lock",
                "{\n  \"lockfileVersion\": 1,\n  \"packages\": {\n    \"zod\": [\"zod@3.23.8\", \"\", {}, \"sha512-a\"],\n    \"ai/zod\": [\"zod@3.22.4\", \"\", {}, \"sha512-b\"],\n  }\n}\n",
            ),
            (
                "go.mod",
                "module example.com/app\n\ngo 1.22\n\nrequire (\n\tgithub.com/acme/kit v1.4.0\n\tgithub.com/acme/kit/v2 v2.1.0 // indirect\n)\n",
            ),
        ]);
        let value = run(&mut host);
        assert_eq!(kinds(&value, "duplicate"), ["zod", "@types/node", "debug"]);
        assert_eq!(kinds(&value, "two-majors"), ["github.com/acme/kit"]);
        assert_eq!(value["policy"]["source"], Value::Null);
        assert!(
            value["markdown"]
                .as_str()
                .unwrap()
                .contains("licenses aren't checked")
        );
    }

    #[test]
    fn requirements_and_licenses_read_their_usual_shapes() {
        assert_eq!(looseness("*"), Looseness::Any);
        assert_eq!(looseness(">=1.2"), Looseness::Unbounded);
        assert_eq!(looseness(">=1.2, <2"), Looseness::Range);
        assert_eq!(looseness("^1.2.3"), Looseness::Range);
        assert_eq!(looseness("1.2.3"), Looseness::Exact);
        assert_eq!(looseness("==2.31.0"), Looseness::Exact);
        assert_eq!(looseness("^1 || >=3"), Looseness::Unbounded);
        let policy = Policy {
            allow: vec!["MIT".into(), "Apache-2.0".into()],
            deny: vec!["GPL-3.0".into()],
            ..Policy::default()
        };
        assert_eq!(license_verdict("MIT OR GPL-3.0", &policy), Verdict::Allowed);
        assert_eq!(
            license_verdict("(MIT AND BSD-3-Clause)", &policy),
            Verdict::NotAllowed
        );
        assert_eq!(license_verdict("GPL-3.0-only", &policy), Verdict::Denied);
        assert_eq!(
            license_verdict("Apache-2.0 WITH LLVM-exception", &policy),
            Verdict::Allowed
        );
        assert_eq!(
            pep508("requests[security] >=2.0 ; python_version > '3'"),
            Some(("requests".into(), ">=2.0".into()))
        );
    }

    #[test]
    fn nothing_to_check_says_so() {
        let mut host = MemoryHost::new([("README.md", "hello")]);
        let value = run(&mut host);
        assert!(
            value["markdown"]
                .as_str()
                .unwrap()
                .contains("no manifest or lockfile")
        );
    }
}
