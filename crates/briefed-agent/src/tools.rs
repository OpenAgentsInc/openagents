//! The briefed agent's other tier-1 tools (#11211,
//! `docs/inference/briefed-agent-tools.md`), each switched on by the config:
//!
//! - `outline {path}`: a file's items (fn, struct, enum, impl, mod, trait,
//!   const) with line numbers, no bodies.
//! - `read_symbol {path, symbol, context_lines}`: one item with its doc
//!   comment and attributes, found by name (`Type::method` looks inside
//!   `impl` blocks for `Type`).
//! - `related {path | symbol, limit}`: what else goes with it: files that
//!   changed together with the path in history before the base commit,
//!   files that use the symbol, and paired test files.
//! - `finish {summary, risk}`: runs the final plan
//!   ([`Verify::final_check`]: compile, every test the change adds, fmt;
//!   never cached); only on `pass` does it record the summary and the host
//!   end the run, on anything else it returns the verdict and the run goes
//!   on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use claude_agent_sdk::{SdkMcpServer, ToolResult};
use serde_json::{Value, json};
use tokio::process::Command;

use crate::verify::Verify;

/// Is this line a Rust item header?
fn item(line: &str) -> bool {
    let t = line.trim_start();
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub(super) "))
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    let t = t
        .strip_prefix("async ")
        .or_else(|| t.strip_prefix("unsafe "))
        .or_else(|| t.strip_prefix("const fn "))
        .map_or(t, |rest| rest);
    [
        "fn ",
        "struct ",
        "enum ",
        "impl ",
        "impl<",
        "mod ",
        "trait ",
        "const ",
        "static ",
        "type ",
        "macro_rules!",
    ]
    .iter()
    .any(|head| t.starts_with(head))
        || line.trim_start().starts_with("#[cfg(test)]")
}

fn inside(root: &Path, path: &str) -> Option<PathBuf> {
    let joined = root.join(path);
    let canonical = joined.canonicalize().ok()?;
    canonical.starts_with(root).then_some(canonical)
}

pub fn outline(root: &Path, path: &str) -> String {
    let Some(file) = inside(root, path) else {
        return format!("{path}: not a file in the worktree");
    };
    let Ok(text) = std::fs::read_to_string(&file) else {
        return format!("{path}: unreadable");
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut out = vec![format!("{path}: {} lines", lines.len())];
    let markdown = path.ends_with(".md");
    for (i, line) in lines.iter().enumerate() {
        if (markdown && line.starts_with('#')) || (!markdown && item(line)) {
            let shown: String = line.chars().take(140).collect();
            out.push(format!("{:>5}  {}", i + 1, shown.trim_end()));
        }
    }
    out.join("\n")
}

/// The end line (0-based, inclusive) of the item that starts at `start`.
fn item_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i64;
    let mut opened = false;
    for (i, line) in lines.iter().enumerate().skip(start) {
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                ';' if !opened && depth == 0 => return i,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return i;
        }
    }
    lines.len().saturating_sub(1)
}

fn defines(line: &str, name: &str) -> bool {
    if !item(line) {
        return false;
    }
    let rest = line.trim_start();
    for key in [
        "fn ", "struct ", "enum ", "mod ", "trait ", "const ", "static ", "type ",
    ] {
        if let Some(pos) = rest.find(key) {
            let after = &rest[pos + key.len()..];
            let ident: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if ident == name {
                return true;
            }
        }
    }
    false
}

pub fn read_symbol(root: &Path, path: &str, symbol: &str, context: usize) -> String {
    let Some(file) = inside(root, path) else {
        return format!("{path}: not a file in the worktree");
    };
    let Ok(text) = std::fs::read_to_string(&file) else {
        return format!("{path}: unreadable");
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut parts: Vec<&str> = symbol.split("::").collect();
    let name = parts.pop().unwrap_or(symbol);
    let owner = parts.pop();
    // Within an `impl ... Owner` block when an owner is named.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    if let Some(owner) = owner {
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            if (t.starts_with("impl") || t.starts_with("pub trait") || t.starts_with("trait"))
                && t.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|w| w == owner)
            {
                ranges.push((i, item_end(&lines, i)));
            }
        }
    }
    if ranges.is_empty() {
        ranges.push((0, lines.len().saturating_sub(1)));
    }
    for (lo, hi) in ranges {
        for i in lo..=hi.min(lines.len().saturating_sub(1)) {
            if defines(lines[i], name) {
                let mut start = i;
                while start > 0 {
                    let prev = lines[start - 1].trim_start();
                    if prev.starts_with("///") || prev.starts_with("#[") || prev.starts_with("//!")
                    {
                        start -= 1;
                    } else {
                        break;
                    }
                }
                let start = start.saturating_sub(context);
                let end = (item_end(&lines, i) + context).min(lines.len() - 1);
                let mut out = vec![format!("{path}:{}-{}", start + 1, end + 1)];
                for (n, line) in lines.iter().enumerate().take(end + 1).skip(start) {
                    out.push(format!("{:>5}  {line}", n + 1));
                }
                return out.join("\n");
            }
        }
    }
    format!("{symbol}: no definition found in {path}; try outline")
}

/// `related`'s deterministic sources.
pub struct Related {
    pub root: PathBuf,
    pub repo: PathBuf,
    pub rev: String,
}

impl Related {
    async fn git(&self, dir: &Path, args: &[&str]) -> String {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .await
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default()
    }

    pub async fn run(&self, path: Option<&str>, symbol: Option<&str>, limit: usize) -> Value {
        let mut found: BTreeMap<String, (f64, String, String)> = BTreeMap::new();
        let mut add = |file: String, score: f64, kind: &str, why: String| {
            let entry = found
                .entry(file)
                .or_insert((0.0, kind.to_owned(), why.clone()));
            if score > entry.0 {
                *entry = (score, kind.to_owned(), why);
            }
        };
        if let Some(path) = path {
            // Co-change before the base commit.
            let log = self
                .git(
                    &self.repo,
                    &[
                        "log",
                        &self.rev,
                        "-n",
                        "300",
                        "--no-merges",
                        "--format=@@",
                        "--name-only",
                        "--",
                        path,
                    ],
                )
                .await;
            let commits: Vec<Vec<&str>> = log
                .split("@@")
                .map(|chunk| chunk.lines().filter(|l| !l.trim().is_empty()).collect())
                .filter(|files: &Vec<&str>| !files.is_empty() && files.len() <= 40)
                .collect();
            let total = commits.len().max(1);
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for files in &commits {
                for file in files {
                    if *file != path {
                        *counts.entry(file).or_default() += 1;
                    }
                }
            }
            for (file, count) in counts {
                if count >= 2 && !file.ends_with("Cargo.lock") {
                    add(
                        file.to_owned(),
                        count as f64 / total as f64,
                        "co_change",
                        format!("changed together with {path} in {count}/{total} commits"),
                    );
                }
            }
            // Paired tests.
            if let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) {
                let dir = Path::new(path).parent().unwrap_or(Path::new(""));
                for candidate in [
                    dir.join(stem).join("tests.rs"),
                    dir.join(format!("{stem}_tests.rs")),
                    dir.join("tests.rs"),
                ] {
                    let candidate = candidate.to_string_lossy().into_owned();
                    if self.root.join(&candidate).is_file() {
                        add(candidate, 0.9, "tests", format!("tests for {path}"));
                    }
                }
            }
        }
        if let Some(symbol) = symbol {
            let name = symbol.rsplit("::").next().unwrap_or(symbol);
            let grep = self
                .git(&self.root, &["grep", "-n", "-w", "-F", "-I", "-e", name])
                .await;
            let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for line in grep.lines().take(2000) {
                let mut parts = line.splitn(3, ':');
                if let (Some(file), Some(row)) = (parts.next(), parts.next()) {
                    uses.entry(file.to_owned())
                        .or_default()
                        .push(row.to_owned());
                }
            }
            let files = uses.len().max(1) as f64;
            for (file, rows) in uses {
                let kind = if file.contains("/tests") || file.ends_with("tests.rs") {
                    "tests"
                } else if file.ends_with(".md") {
                    "docs"
                } else {
                    "uses"
                };
                let shown: Vec<String> = rows.iter().take(5).cloned().collect();
                add(
                    file,
                    0.5 + 0.5 / files,
                    kind,
                    format!("names `{name}` at lines {}", shown.join(", ")),
                );
            }
        }
        let mut ranked: Vec<(String, (f64, String, String))> = found.into_iter().collect();
        ranked.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
        json!(
            ranked
                .into_iter()
                .take(limit)
                .map(|(file, (score, kind, why))| json!({
                    "file": file, "kind": kind, "why": why,
                    "confidence": (score * 100.0).round() / 100.0,
                }))
                .collect::<Vec<_>>()
        )
    }
}

/// Builds the `oa` server with `verify` and the tools the config names.
pub fn server(
    root: &Path,
    config: &Value,
    names: &[String],
    finished: Arc<AtomicBool>,
) -> Option<SdkMcpServer> {
    let has = |name: &str| names.iter().any(|n| n == name);
    let verify = Verify::from_config(root, config).map(Arc::new);
    let mut server = SdkMcpServer::new("oa", "1.0.0").timeout_ms(1_800_000);
    let mut any = false;
    if has("verify")
        && let Some(verify) = verify.clone()
    {
        server = server.add_tool(Verify::tool(verify));
        any = true;
    }
    if has("outline") {
        let root = root.to_path_buf();
        server = server.tool(
            "outline",
            "A file's items (fn, struct, enum, impl, mod, trait, const) with line numbers, no bodies.",
            json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
            move |args| {
                let root = root.clone();
                async move {
                    Ok(ToolResult::text(outline(
                        &root,
                        args["path"].as_str().unwrap_or(""),
                    )))
                }
            },
        );
        any = true;
    }
    if has("read_symbol") {
        let root = root.to_path_buf();
        server = server.tool(
            "read_symbol",
            "One item of a file (`name` or `Type::method`) with its doc comment, and nothing else.",
            json!({"type": "object", "properties": {
                "path": {"type": "string"}, "symbol": {"type": "string"},
                "context_lines": {"type": "integer"}}, "required": ["path", "symbol"]}),
            move |args| {
                let root = root.clone();
                async move {
                    Ok(ToolResult::text(read_symbol(
                        &root,
                        args["path"].as_str().unwrap_or(""),
                        args["symbol"].as_str().unwrap_or(""),
                        usize::try_from(args["context_lines"].as_u64().unwrap_or(0).min(20))
                            .unwrap_or(0),
                    )))
                }
            },
        );
        any = true;
    }
    if has("related")
        && let Some(block) = config.get("related")
    {
        let related = Arc::new(Related {
            root: root.to_path_buf(),
            repo: PathBuf::from(block["repo"].as_str().unwrap_or(".")),
            rev: block["rev"].as_str().unwrap_or("HEAD").to_owned(),
        });
        server = server.tool(
            "related",
            "What else goes with a file or symbol: files changed together with it in history, \
             files that use the symbol, and its tests. Give `path` or `symbol`.",
            json!({"type": "object", "properties": {
                "path": {"type": "string"}, "symbol": {"type": "string"},
                "limit": {"type": "integer"}}}),
            move |args| {
                let related = related.clone();
                async move {
                    let limit =
                        usize::try_from(args["limit"].as_u64().unwrap_or(10).min(30)).unwrap_or(10);
                    let value = related
                        .run(args["path"].as_str(), args["symbol"].as_str(), limit)
                        .await;
                    Ok(ToolResult::text(value.to_string()))
                }
            },
        );
        any = true;
    }
    if has("finish")
        && let Some(verify) = verify
    {
        let summary_path = config["finish_path"].as_str().map(PathBuf::from);
        server = server.tool(
            "finish",
            "Say you are done. Runs the final checks (compile, every test your change adds, fmt), \
             never a cached result: on a pass the run ends and your summary becomes the pull \
             request description; otherwise you get the verdict and go on.",
            json!({"type": "object", "properties": {
                "summary": {"type": "string"},
                "risk": {"type": "string", "enum": ["low", "medium", "high"]}},
                "required": ["summary"]}),
            move |args| {
                let verify = verify.clone();
                let finished = finished.clone();
                let summary_path = summary_path.clone();
                async move {
                    // The declared final plan, never the cache or the
                    // agent's last filter (#11229).
                    let verdict = verify.final_check().await;
                    if verdict["status"].as_str() == Some("pass") {
                        finished.store(true, Ordering::SeqCst);
                        if let Some(path) = summary_path {
                            let _ = std::fs::write(
                                path,
                                json!({"summary": args["summary"], "risk": args["risk"], "verdict": verdict})
                                    .to_string(),
                            );
                        }
                        Ok(ToolResult::text("Verified. The run ends here."))
                    } else {
                        Ok(ToolResult::text(
                            serde_json::to_string(&verdict).unwrap_or_default(),
                        ))
                    }
                }
            },
        );
        any = true;
    }
    any.then_some(server)
}
