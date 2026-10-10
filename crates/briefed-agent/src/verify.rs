//! `verify`: the briefed agent's in-process check tool (#11211).
//!
//! One call runs exactly the issue's checks on the working copy, through
//! the bench's build host (`remote-exec`, one shared warm target dir):
//! `cargo check` of the touched crates and their tests, then their tests
//! (optionally filtered), then `cargo fmt` (applied; files it rewrote come
//! back to the worktree). It answers with a short structured verdict and no
//! raw log:
//!
//! - `status`: `pass`, `compile_error`, `test_failure`, or `error`;
//! - `compile_errors`: `{file, line, msg}`, new against the base commit's
//!   own errors (some bases already fail to build an unrelated target);
//! - `failing_tests`: `{name, file, line, assert}`;
//! - `fmt`: what formatting did;
//! - `untouched_but_implicated`: files the failure points at, or that
//!   changed together with a touched file in history, that the working
//!   copy has not changed, each with a reason;
//! - `done_when`: the issue's acceptance items, each mapped to the check
//!   that decides it and that check's result.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use claude_agent_sdk::{SdkMcpTool, ToolResult};
use serde_json::{Value, json};
use tokio::process::Command;

/// What `verify` needs, from the agent config's `verify` block.
pub struct Verify {
    pub root: PathBuf,
    /// The `remote-exec` script that runs a command on the build host.
    pub exec: PathBuf,
    /// Packages to check when the working copy touches no crate.
    pub crates: Vec<String>,
    /// The base commit's own compile errors, normalized (`file: msg`).
    pub baseline: BTreeSet<String>,
    pub done_when: Vec<String>,
    /// path -> [(partner, times changed together)].
    pub cochange: BTreeMap<String, Vec<(String, u64)>>,
    /// Appends one JSON line per call when set.
    pub log: Option<PathBuf>,
    /// The last test filter the agent gave, which `finish` reuses.
    pub last_filter: std::sync::Mutex<String>,
    /// The last call's key (mode, filter, working copy) and verdict.
    pub cache: std::sync::Mutex<Option<(String, Value)>>,
}

impl Verify {
    pub fn from_config(root: &Path, config: &Value) -> Option<Self> {
        let block = config.get("verify")?;
        let strings = |value: &Value| -> Vec<String> {
            value
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut cochange = BTreeMap::new();
        if let Some(map) = block["cochange"].as_object() {
            for (path, partners) in map {
                let list = partners
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| {
                                Some((item["path"].as_str()?.to_owned(), item["count"].as_u64()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                cochange.insert(path.clone(), list);
            }
        }
        Some(Self {
            root: root.to_path_buf(),
            exec: PathBuf::from(block["exec"].as_str()?),
            crates: strings(&block["crates"]),
            baseline: strings(&block["baseline_errors"]).into_iter().collect(),
            done_when: strings(&block["done_when"]),
            cochange,
            log: block["log"].as_str().map(PathBuf::from),
            last_filter: std::sync::Mutex::new(String::new()),
            cache: std::sync::Mutex::new(None),
        })
    }

    /// The `verify` tool for the in-process MCP server.
    pub fn tool(verify: Arc<Self>) -> SdkMcpTool {
        SdkMcpTool::new(
            "verify",
            "Run this issue's checks on your working copy: compile the touched crates and their \
             tests, run their tests (pass `tests`, space-separated test-name filters, to run \
             fewer), then format them; `fast` compiles only. An unchanged working copy returns \
             the last result at once. Returns a short structured verdict: status, compile \
             errors {file,line,msg}, failing tests {name,file,line,assert}, fmt, files \
             implicated but untouched, and the issue's acceptance items with their check's result.",
            json!({
                "type": "object",
                "properties": {
                    "tests": {"type": "string", "description": "test-name filter words (optional)"},
                    "fast": {"type": "boolean", "description": "compile only (cargo check), no tests or fmt"}
                }
            }),
            move |args| {
                let verify = verify.clone();
                async move {
                    let filter = args["tests"].as_str().unwrap_or("").to_owned();
                    if let Ok(mut last) = verify.last_filter.lock() {
                        last.clone_from(&filter);
                    }
                    let fast = args["fast"].as_bool().unwrap_or(false);
                    let verdict = verify.run(&filter, fast).await;
                    Ok(
                        ToolResult::text(serde_json::to_string(&verdict).unwrap_or_default())
                            .with_structured_content(verdict),
                    )
                }
            },
        )
    }

    async fn exec(&self, argv: &[&str]) -> (bool, String) {
        let output = Command::new(&self.exec)
            .args(argv)
            .current_dir(&self.root)
            .output()
            .await;
        match output {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                (out.status.success(), text)
            }
            Err(error) => (false, format!("could not run the check: {error}")),
        }
    }

    async fn git(&self, args: &[&str]) -> String {
        Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .await
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default()
    }

    /// Files the working copy changed (it cannot commit: no shell).
    async fn changed(&self) -> BTreeSet<String> {
        self.git(&["status", "--porcelain", "--untracked-files=all"])
            .await
            .lines()
            .filter_map(|line| line.get(3..))
            .map(|path| path.rsplit(" -> ").next().unwrap_or(path).to_owned())
            .collect()
    }

    /// The package that owns `path`, from the nearest manifest.
    fn package_of(&self, path: &str) -> Option<String> {
        let mut dir = Path::new(path).parent();
        while let Some(current) = dir {
            if current.as_os_str().is_empty() {
                break;
            }
            let manifest = self.root.join(current).join("Cargo.toml");
            if let Ok(text) = std::fs::read_to_string(&manifest)
                && let Some(section) = text.split("[package]").nth(1)
            {
                for line in section.lines() {
                    let line = line.trim();
                    if line.starts_with('[') {
                        break;
                    }
                    if let Some(rest) = line.strip_prefix("name")
                        && let Some(value) = rest.trim().strip_prefix('=')
                    {
                        return Some(value.trim().trim_matches('"').to_owned());
                    }
                }
            }
            dir = current.parent();
        }
        None
    }

    pub async fn run(&self, filter: &str, fast: bool) -> Value {
        let started = Instant::now();
        let digest = self.changed_digest().await;
        let key = format!("{fast}\u{0}{filter}\u{0}{digest}");
        let hit = self.cache.lock().ok().and_then(|cache| {
            cache
                .as_ref()
                .filter(|(last, _)| *last == key)
                .map(|(_, v)| v.clone())
        });
        if let Some(mut verdict) = hit {
            verdict["cached"] = json!(true);
            verdict["note"] = json!("nothing changed since the last verify; same result");
            return verdict;
        }
        let verdict = self.run_uncached(filter, fast, &digest, started).await;
        let after = self.changed_digest().await;
        if let Ok(mut cache) = self.cache.lock() {
            *cache = Some((format!("{fast}\u{0}{filter}\u{0}{after}"), verdict.clone()));
        }
        verdict
    }

    /// The working copy's change, as text, for the cache and for fmt.
    async fn changed_digest(&self) -> String {
        let mut out = self.git(&["diff"]).await;
        out.push_str(
            &self
                .git(&["status", "--porcelain", "--untracked-files=all"])
                .await,
        );
        out
    }

    fn implicate_errors(
        &self,
        errors: &[Value],
        changed: &BTreeSet<String>,
        implicated: &mut BTreeMap<String, String>,
    ) {
        for error in errors {
            let file = error["file"].as_str().unwrap_or("").to_owned();
            if !file.is_empty() && !changed.contains(&file) {
                implicated.entry(file).or_insert_with(|| {
                    format!(
                        "compile error here: {}",
                        error["msg"].as_str().unwrap_or("")
                    )
                });
            }
        }
    }

    async fn run_uncached(
        &self,
        filter: &str,
        fast: bool,
        digest: &str,
        started: Instant,
    ) -> Value {
        let changed = self.changed().await;
        let mut packages: Vec<String> = Vec::new();
        for path in changed.iter().filter(|path| path.ends_with(".rs")) {
            if let Some(package) = self.package_of(path)
                && !packages.contains(&package)
            {
                packages.push(package);
            }
        }
        if packages.is_empty() {
            packages = self.crates.clone();
        }
        let mut verdict = json!({
            "crates": packages,
            "changed": changed,
        });
        if packages.is_empty() {
            verdict["status"] = json!("error");
            verdict["note"] = json!("no crate to check");
            return verdict;
        }
        let selectors: Vec<String> = packages
            .iter()
            .flat_map(|package| ["-p".to_owned(), package.clone()])
            .collect();

        let words: Vec<&str> = filter
            .split_whitespace()
            .filter(|word| {
                word.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
            })
            .take(8)
            .collect();
        let mut implicated: BTreeMap<String, String> = BTreeMap::new();
        if fast {
            // Compile only (`cargo check`, tests included): the quick loop.
            let mut argv: Vec<&str> = vec!["cargo", "check"];
            argv.extend(selectors.iter().map(String::as_str));
            argv.extend(["--tests", "--keep-going", "--message-format", "short"]);
            let (_, text) = self.exec(&argv).await;
            let (errors, preexisting) = self.compile_errors(&text);
            verdict["preexisting_errors"] = json!(preexisting);
            self.implicate_errors(&errors, &changed, &mut implicated);
            verdict["status"] = json!(if errors.is_empty() {
                "compiles"
            } else {
                "compile_error"
            });
            if !errors.is_empty() {
                verdict["compile_errors"] = json!(errors.iter().take(20).collect::<Vec<_>>());
            }
        } else {
            // Tests and format in one build-host call: `cargo test` compiles
            // (its errors are parsed the same way), then `cargo fmt` applies.
            let (text, passed, failed, failures) = self.test_and_fmt(&selectors, &words, &[]).await;
            let (mut errors, preexisting) = self.compile_errors(&text);
            let (text, passed, failed, failures) =
                if errors.is_empty() && preexisting > 0 && passed == 0 && failed == 0 {
                    // A target the base already fails to build: test the
                    // library and binaries alone.
                    let again = self
                        .test_and_fmt(&selectors, &words, &["--lib", "--bins"])
                        .await;
                    errors = self.compile_errors(&again.0).0;
                    again
                } else {
                    (text, passed, failed, failures)
                };
            verdict["preexisting_errors"] = json!(preexisting);
            if !errors.is_empty() {
                self.implicate_errors(&errors, &changed, &mut implicated);
                verdict["status"] = json!("compile_error");
                verdict["compile_errors"] = json!(errors.iter().take(20).collect::<Vec<_>>());
            } else {
                for failure in &failures {
                    let file = failure["file"].as_str().unwrap_or("").to_owned();
                    if !file.is_empty() && !changed.contains(&file) {
                        implicated.entry(file).or_insert_with(|| {
                            format!(
                                "failing test {} asserts here",
                                failure["name"].as_str().unwrap_or("")
                            )
                        });
                    }
                }
                verdict["tests"] = json!({"passed": passed, "failed": failed});
                if !failures.is_empty() || failed > 0 {
                    verdict["status"] = json!("test_failure");
                    verdict["failing_tests"] = json!(failures.iter().take(10).collect::<Vec<_>>());
                } else {
                    verdict["status"] = json!("pass");
                    if passed == 0 {
                        verdict["note"] = json!("no test matched the filter");
                    }
                }
            }
            let fmt_ok = text.contains("@@fmt ok");
            let after = self.changed_digest().await;
            verdict["fmt"] = json!(if !fmt_ok {
                "cargo fmt failed"
            } else if after == digest {
                "already formatted"
            } else {
                "formatted (files rewritten in place)"
            });
        }
        for path in &changed {
            for (partner, count) in self.cochange.get(path).into_iter().flatten() {
                if *count >= 3 && !changed.contains(partner) && implicated.len() < 8 {
                    implicated.entry(partner.clone()).or_insert_with(|| {
                        format!("changed together with {path} in {count} past commits")
                    });
                }
            }
        }
        verdict["untouched_but_implicated"] = json!(
            implicated
                .iter()
                .map(|(file, reason)| json!({"file": file, "reason": reason}))
                .collect::<Vec<_>>()
        );
        let status = verdict["status"].as_str().unwrap_or("error").to_owned();
        let tests_ok = status == "pass";
        verdict["done_when"] = json!(
            self.done_when
                .iter()
                .map(|item| json!({
                    "item": item,
                    "check": "the touched crates compile and their tests pass",
                    "status": if tests_ok { "pass" } else { "fail" },
                }))
                .collect::<Vec<_>>()
        );
        verdict["secs"] = json!(started.elapsed().as_secs());
        if let Some(log) = &self.log {
            let line = json!({"filter": filter, "status": status, "secs": verdict["secs"]});
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
            {
                use std::io::Write as _;
                let _ = writeln!(file, "{line}");
            }
        }
        verdict
    }

    /// New compile errors as `{file, line, msg}`, and the count of the
    /// base's own errors that were left out.
    fn compile_errors(&self, text: &str) -> (Vec<Value>, usize) {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut preexisting = 0;
        for line in text.lines() {
            if line.starts_with("error: could not compile") || !line.contains("error") {
                continue;
            }
            // `path:line:col: error[E0xxx]: msg`
            let mut parts = line.splitn(4, ':');
            let (Some(file), Some(row), Some(_col), Some(rest)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                if line.starts_with("error") && seen.insert(line.to_owned()) {
                    out.push(json!({"file": "", "line": 0, "msg": line}));
                }
                continue;
            };
            let Ok(row) = row.trim().parse::<u64>() else {
                continue;
            };
            let rest = rest.trim();
            if !rest.starts_with("error") {
                continue;
            }
            let key = format!("{file}: {rest}");
            if self.baseline.contains(&key) {
                preexisting += 1;
                continue;
            }
            if seen.insert(key) {
                let msg = rest.split_once(": ").map_or(rest, |(_, msg)| msg);
                out.push(json!({"file": file, "line": row, "msg": msg}));
            }
        }
        (out, preexisting)
    }

    /// Runs the tests, then `cargo fmt`, in one build-host call; returns
    /// the output, passed, failed, and the failures.
    async fn test_and_fmt(
        &self,
        selectors: &[String],
        words: &[&str],
        targets: &[&str],
    ) -> (String, u64, u64, Vec<Value>) {
        let mut test = vec!["cargo".to_owned(), "test".to_owned()];
        test.extend(selectors.iter().cloned());
        test.extend(targets.iter().map(|t| (*t).to_owned()));
        test.extend(["--no-fail-fast", "--message-format", "short"].map(str::to_owned));
        if !words.is_empty() {
            test.push("--".to_owned());
            test.extend(words.iter().map(|w| (*w).to_owned()));
        }
        let mut fmt = vec!["cargo".to_owned(), "fmt".to_owned()];
        fmt.extend(selectors.iter().cloned());
        let script = format!(
            "{}; rc=$?; {} && echo @@fmt ok; exit $rc",
            test.join(" "),
            fmt.join(" ")
        );
        let (_, text) = self.exec(&["sh", "-c", &script]).await;
        let (mut passed, mut failed) = (0, 0);
        let mut failures = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if let Some(rest) = line.strip_prefix("test result: ") {
                for chunk in rest.split(';') {
                    let mut words = chunk.split_whitespace().rev();
                    let (Some(kind), Some(count)) = (words.next(), words.next()) else {
                        continue;
                    };
                    let count = count.parse::<u64>().unwrap_or(0);
                    match kind {
                        "passed" => passed += count,
                        "failed" => failed += count,
                        _ => {}
                    }
                }
            }
            if let Some(name) = line
                .strip_prefix("---- ")
                .and_then(|rest| rest.strip_suffix(" stdout ----"))
            {
                let mut file = String::new();
                let mut row = 0;
                let mut assert = Vec::new();
                for next in lines.iter().skip(i + 1).take(12) {
                    if next.starts_with("---- ") || next.trim().is_empty() && !assert.is_empty() {
                        break;
                    }
                    if let Some(at) = next.split(" panicked at ").nth(1) {
                        let mut parts = at.trim_end_matches(':').splitn(3, ':');
                        file = parts.next().unwrap_or("").to_owned();
                        row = parts
                            .next()
                            .and_then(|r| r.parse::<u64>().ok())
                            .unwrap_or(0);
                        continue;
                    }
                    if !file.is_empty() && assert.len() < 4 {
                        assert.push(next.trim().to_owned());
                    }
                }
                failures.push(json!({
                    "name": name, "file": file, "line": row, "assert": assert.join(" | "),
                }));
            }
        }
        (text, passed, failed, failures)
    }
}
