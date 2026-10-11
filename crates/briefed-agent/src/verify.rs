//! `verify`: the briefed agent's in-process check tool (#11211, #11229).
//!
//! One call runs the issue's checks on the working copy through the bench's
//! build host (`remote-exec`, one shared warm target dir) and answers with a
//! short structured verdict and no raw log.
//!
//! **What `pass` means.** `pass` is reported only when the checker ran, its
//! test command exited 0, at least one test ran and passed, and `cargo fmt`
//! exited 0 (formatting is part of the plan and is applied). Everything else
//! has its own status and is never `pass`:
//!
//! - `checker_error`: the checker could not start, timed out, exited nonzero
//!   with no recognized compiler or test failure, or printed output without
//!   its exit markers;
//! - `compile_error`: compile errors the base commit does not have, as
//!   `{file, line, msg}`;
//! - `test_failure`: failing tests as `{name, file, line, assert}`;
//! - `no_tests`: the command succeeded but ran no test;
//! - `fmt_failed`: the tests passed but `cargo fmt` failed;
//! - `error`: no crate to check.
//!
//! `fast` compiles only (`compiles` or one of the failures above); it is
//! feedback, never acceptance.
//!
//! **`finish`** ([`Verify::final_check`]) runs a declared plan, never the
//! cache and never the agent's last filter: the touched crates must compile
//! (no new errors), every test function the candidate adds must run and
//! pass, and fmt must succeed. A candidate that adds no test is `no_tests`.
//!
//! **Cache.** An unchanged candidate returns the last verdict at once. The
//! key is the git tree of the whole candidate (tracked, staged, unstaged,
//! untracked and removed content, from a scratch index), the mode, the
//! filter, the crates, and the checker's identity (path, size, mtime, and
//! the base's error set). When git cannot name the tree, nothing is cached.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use claude_agent_sdk::{SdkMcpTool, ToolResult};
use serde_json::{Value, json};
use tokio::process::Command;

/// The exit code `timeout` (and the build host's runner) report.
const TIMED_OUT: i32 = 124;

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
    /// The last call's key and verdict.
    pub cache: std::sync::Mutex<Option<(String, Value)>>,
    /// How the run's `verify` calls went, for the run summary.
    pub tally: Arc<std::sync::Mutex<Tally>>,
}

/// How a run's `verify` calls went (#11257).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tally {
    /// Calls that ran, cached answers included.
    pub calls: u64,
    /// Calls whose status was not `pass` or `compiles`.
    pub failures: u64,
    /// Whether any call returned `pass`.
    pub passed: bool,
    /// The last call's status.
    pub last: Option<String>,
}

impl Tally {
    pub fn record(&mut self, status: &str) {
        self.calls += 1;
        if !matches!(status, "pass" | "compiles") {
            self.failures += 1;
        }
        self.passed |= status == "pass";
        self.last = Some(status.to_owned());
    }

    pub fn to_json(&self) -> Value {
        json!({"calls": self.calls, "failures": self.failures,
               "passed": self.passed, "last": self.last})
    }
}

/// How one checker command ended.
#[derive(Debug)]
pub struct Outcome {
    /// `None`: it could not start, or it ended without an exit code.
    pub code: Option<i32>,
    pub text: String,
}

impl Outcome {
    fn failure(&self) -> Option<String> {
        match self.code {
            None => Some(format!(
                "the checker did not run: {}",
                self.text.lines().last().unwrap_or("no output")
            )),
            Some(TIMED_OUT) => Some("the checker timed out".to_owned()),
            _ => None,
        }
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Test filter words: test-name characters only.
fn words(filter: &str) -> Vec<String> {
    filter
        .split_whitespace()
        .filter(|word| {
            word.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
        })
        .take(12)
        .map(str::to_owned)
        .collect()
}

/// The test functions a unified diff adds (`#[test]` or `#[tokio::test]`
/// followed within a few lines by `fn NAME`).
pub fn added_tests(diff: &str) -> Vec<String> {
    let lines: Vec<&str> = diff.lines().collect();
    let mut names = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(body) = line.strip_prefix('+') else {
            continue;
        };
        let attr = body.trim_start();
        if !(attr.starts_with("#[test]") || attr.starts_with("#[tokio::test")) {
            continue;
        }
        for next in lines.iter().skip(i + 1).take(5) {
            let Some(next) = next.strip_prefix('+') else {
                break;
            };
            if let Some(pos) = next.find("fn ") {
                let name: String = next[pos + 3..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
                break;
            }
        }
    }
    names
}

impl Verify {
    pub fn from_config(root: &Path, config: &Value) -> Option<Self> {
        let block = config.get("verify")?;
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
            cache: std::sync::Mutex::new(None),
            tally: Arc::default(),
        })
    }

    /// The `verify` tool for the in-process MCP server.
    pub fn tool(verify: Arc<Self>) -> SdkMcpTool {
        SdkMcpTool::new(
            "verify",
            "Run this issue's checks on your working copy: run the touched crates' tests (pass \
             `tests`, space-separated test-name filters, to run fewer), then format them; `fast` \
             compiles only. `pass` means the tests ran and passed and fmt succeeded; anything else \
             says what failed or could not run. An unchanged working copy returns the last result. \
             The verdict: status, compile errors {file,line,msg}, failing tests \
             {name,file,line,assert}, fmt, files implicated but untouched, and the issue's \
             acceptance items with their check's result.",
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

    async fn exec(&self, argv: &[&str]) -> Outcome {
        let output = Command::new(&self.exec)
            .args(argv)
            .current_dir(&self.root)
            .output()
            .await;
        match output {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                Outcome {
                    code: out.status.code(),
                    text,
                }
            }
            Err(error) => Outcome {
                code: None,
                text: format!("could not run the check: {error}"),
            },
        }
    }

    async fn git_env(&self, args: &[&str], index: Option<&Path>) -> Option<String> {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.root).args(args);
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        let out = command.output().await.ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    async fn git(&self, args: &[&str]) -> String {
        self.git_env(args, None).await.unwrap_or_default()
    }

    /// The whole candidate as a scratch index (a copy of the real one, so
    /// sparse-checkout bits hold, with every change added), and the diff of
    /// that index against HEAD. `None` when git cannot do either.
    async fn candidate(&self) -> Option<(String, String)> {
        let index = self
            .git_env(
                &["rev-parse", "--path-format=absolute", "--git-path", "index"],
                None,
            )
            .await?;
        let scratch = tempfile::NamedTempFile::new().ok()?;
        std::fs::copy(index.trim(), scratch.path()).ok()?;
        self.git_env(&["add", "-A"], Some(scratch.path())).await?;
        let tree = self.git_env(&["write-tree"], Some(scratch.path())).await?;
        let diff = self
            .git_env(&["diff", "--cached", "HEAD"], Some(scratch.path()))
            .await?;
        Some((tree.trim().to_owned(), diff))
    }

    /// The checker's identity: its path, size and modification time, and
    /// the base's error set.
    fn checker_identity(&self) -> String {
        let meta = std::fs::metadata(&self.exec).ok();
        let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
        let mtime = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        format!(
            "{}\u{0}{size}\u{0}{mtime}\u{0}{}",
            self.exec.display(),
            self.baseline
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join("\u{1}")
        )
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

    fn packages(&self, changed: &BTreeSet<String>) -> Vec<String> {
        let mut packages: Vec<String> = Vec::new();
        for path in changed.iter().filter(|path| path.ends_with(".rs")) {
            if let Some(package) = self.package_of(path)
                && !packages.contains(&package)
            {
                packages.push(package);
            }
        }
        if packages.is_empty() {
            packages.clone_from(&self.crates);
        }
        packages
    }

    /// `verify`: the cached result for an unchanged candidate, otherwise a
    /// fresh run. Either way the call counts in the tally.
    pub async fn run(&self, filter: &str, fast: bool) -> Value {
        let verdict = self.answer(filter, fast).await;
        if let Ok(mut tally) = self.tally.lock() {
            tally.record(verdict["status"].as_str().unwrap_or("error"));
        }
        verdict
    }

    async fn answer(&self, filter: &str, fast: bool) -> Value {
        let candidate = self.candidate().await;
        let changed = self.changed().await;
        let key = candidate.as_ref().map(|(tree, _)| {
            format!(
                "{fast}\u{0}{filter}\u{0}{tree}\u{0}{}\u{0}{}",
                self.packages(&changed).join(","),
                self.checker_identity()
            )
        });
        if let Some(key) = &key {
            let hit = self.cache.lock().ok().and_then(|cache| {
                cache
                    .as_ref()
                    .filter(|(last, _)| last == key)
                    .map(|(_, v)| v.clone())
            });
            if let Some(mut verdict) = hit {
                verdict["cached"] = json!(true);
                verdict["note"] = json!("nothing changed since the last verify; same result");
                return verdict;
            }
        }
        let verdict = self.run_uncached(&words(filter), fast, None).await;
        // Cache under the candidate as it is now (fmt may have rewritten it).
        if key.is_some()
            && let Some((tree, _)) = self.candidate().await
        {
            let changed = self.changed().await;
            let after = format!(
                "{fast}\u{0}{filter}\u{0}{tree}\u{0}{}\u{0}{}",
                self.packages(&changed).join(","),
                self.checker_identity()
            );
            if let Ok(mut cache) = self.cache.lock() {
                *cache = Some((after, verdict.clone()));
            }
        }
        verdict
    }

    /// `finish`: the declared final plan, never cached and never the
    /// agent's filter. Every test the candidate adds must run and pass.
    pub async fn final_check(&self) -> Value {
        let Some((_, diff)) = self.candidate().await else {
            return json!({"status": "checker_error", "note": "git could not read the candidate"});
        };
        let required = added_tests(&diff);
        if required.is_empty() {
            return json!({
                "status": "no_tests",
                "note": "the change adds no test; add one that pins the new behavior",
            });
        }
        let mut verdict = self
            .run_uncached(&required, false, Some(required.len()))
            .await;
        verdict["required_tests"] = json!(required);
        verdict
    }

    /// One run. `required`: the number of tests that must run and pass.
    async fn run_uncached(&self, words: &[String], fast: bool, required: Option<usize>) -> Value {
        let started = Instant::now();
        let changed = self.changed().await;
        let packages = self.packages(&changed);
        let mut verdict = json!({"crates": packages, "changed": changed});
        if packages.is_empty() {
            verdict["status"] = json!("error");
            verdict["note"] = json!("no crate to check");
            return verdict;
        }
        let selectors: Vec<String> = packages
            .iter()
            .flat_map(|package| ["-p".to_owned(), package.clone()])
            .collect();
        let mut implicated: BTreeMap<String, String> = BTreeMap::new();
        if fast {
            let mut argv: Vec<&str> = vec!["cargo", "check"];
            argv.extend(selectors.iter().map(String::as_str));
            argv.extend(["--tests", "--keep-going", "--message-format", "short"]);
            let outcome = self.exec(&argv).await;
            let (errors, preexisting) = self.compile_errors(&outcome.text);
            verdict["preexisting_errors"] = json!(preexisting);
            verdict["exit"] = json!(outcome.code);
            let status = if let Some(why) = outcome.failure() {
                verdict["note"] = json!(why);
                "checker_error"
            } else if !errors.is_empty() {
                self.implicate_errors(&errors, &changed, &mut implicated);
                verdict["compile_errors"] = json!(errors.iter().take(20).collect::<Vec<_>>());
                "compile_error"
            } else if outcome.code == Some(0) || preexisting > 0 {
                "compiles"
            } else {
                verdict["note"] = json!(format!(
                    "cargo check exited {:?} with no compile error it could name",
                    outcome.code
                ));
                "checker_error"
            };
            verdict["status"] = json!(status);
        } else {
            let mut run = self.test_and_fmt(&selectors, words, &[]).await;
            let (mut errors, preexisting) = self.compile_errors(&run.text);
            if run.test_code != Some(0) && errors.is_empty() && preexisting > 0 && run.passed == 0 {
                // A target the base already fails to build: test the
                // library and binaries alone.
                run = self
                    .test_and_fmt(&selectors, words, &["--lib", "--bins"])
                    .await;
                errors = self.compile_errors(&run.text).0;
            }
            verdict["preexisting_errors"] = json!(preexisting);
            verdict["exit"] =
                json!({"tests": run.test_code, "fmt": run.fmt_code, "checker": run.outcome.code});
            verdict["tests"] = json!({"passed": run.passed, "failed": run.failed});
            for failure in &run.failures {
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
            let status = if let Some(why) = run.outcome.failure() {
                verdict["note"] = json!(why);
                "checker_error"
            } else if run.test_code.is_none() || run.fmt_code.is_none() {
                verdict["note"] = json!("the checker's output had no exit markers");
                "checker_error"
            } else if !errors.is_empty() {
                self.implicate_errors(&errors, &changed, &mut implicated);
                verdict["compile_errors"] = json!(errors.iter().take(20).collect::<Vec<_>>());
                "compile_error"
            } else if !run.failures.is_empty() || run.failed > 0 {
                verdict["failing_tests"] = json!(run.failures.iter().take(10).collect::<Vec<_>>());
                "test_failure"
            } else if run.test_code == Some(TIMED_OUT) {
                verdict["note"] = json!("the tests timed out");
                "checker_error"
            } else if run.test_code != Some(0) {
                verdict["note"] = json!(format!(
                    "cargo test exited {:?} with no failure it could name",
                    run.test_code
                ));
                "checker_error"
            } else if run.passed == 0 || required.is_some_and(|n| run.passed < n as u64) {
                verdict["note"] = json!(if run.passed == 0 {
                    "no test ran".to_owned()
                } else {
                    format!(
                        "{} of {} required tests ran",
                        run.passed,
                        required.unwrap_or(0)
                    )
                });
                "no_tests"
            } else if run.fmt_code != Some(0) {
                "fmt_failed"
            } else {
                "pass"
            };
            verdict["status"] = json!(status);
            verdict["fmt"] = json!(match run.fmt_code {
                Some(0) => "ok (applied)",
                Some(_) => "cargo fmt failed",
                None => "did not run",
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
        verdict["done_when"] = json!(
            self.done_when
                .iter()
                .map(|item| json!({
                    "item": item,
                    "check": "the touched crates' tests ran and passed",
                    "status": if status == "pass" { "pass" } else { "not shown" },
                }))
                .collect::<Vec<_>>()
        );
        verdict["secs"] = json!(started.elapsed().as_secs());
        if let Some(log) = &self.log {
            let line = json!({"filter": words.join(" "), "fast": fast, "final": required.is_some(),
                              "status": status, "secs": verdict["secs"]});
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

    /// New compile errors as `{file, line, msg}`, and the count of the
    /// base's own errors that were left out.
    fn compile_errors(&self, text: &str) -> (Vec<Value>, usize) {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut preexisting = 0;
        for line in text.lines() {
            if line.starts_with("error: could not compile")
                || line.starts_with("error: test failed")
                || line.starts_with("error: 1 target failed")
                || line.contains("target failed:")
                || !line.contains("error")
            {
                continue;
            }
            // `path:line:col: error[E0xxx]: msg`
            let mut parts = line.splitn(4, ':');
            let (Some(file), Some(row), Some(_col), Some(rest)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
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

    /// Runs the tests, then `cargo fmt`, in one build-host call, and reads
    /// both exit codes from markers the script prints.
    async fn test_and_fmt(
        &self,
        selectors: &[String],
        words: &[String],
        targets: &[&str],
    ) -> TestRun {
        let mut test = vec!["cargo".to_owned(), "test".to_owned()];
        test.extend(selectors.iter().cloned());
        test.extend(targets.iter().map(|t| (*t).to_owned()));
        test.extend(["--no-fail-fast", "--message-format", "short"].map(str::to_owned));
        if !words.is_empty() {
            test.push("--".to_owned());
            test.extend(words.iter().cloned());
        }
        let mut fmt = vec!["cargo".to_owned(), "fmt".to_owned()];
        fmt.extend(selectors.iter().cloned());
        let script = format!(
            "{}; t=$?; {}; f=$?; echo \"@@test_exit=$t\"; echo \"@@fmt_exit=$f\"; exit $t",
            test.join(" "),
            fmt.join(" ")
        );
        let outcome = self.exec(&["sh", "-c", &script]).await;
        parse_test_run(outcome)
    }
}

/// A test-and-fmt run, parsed.
pub struct TestRun {
    pub outcome: Outcome,
    pub test_code: Option<i32>,
    pub fmt_code: Option<i32>,
    pub passed: u64,
    pub failed: u64,
    pub failures: Vec<Value>,
    pub text: String,
}

fn marker(text: &str, name: &str) -> Option<i32> {
    text.lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(name))
        .and_then(|value| value.trim().parse().ok())
}

fn parse_test_run(outcome: Outcome) -> TestRun {
    let text = outcome.text.clone();
    let test_code = marker(&text, "@@test_exit=");
    let fmt_code = marker(&text, "@@fmt_exit=");
    let (mut passed, mut failed) = (0, 0);
    let mut failures = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix("test result: ") {
            for chunk in rest.split(';') {
                let mut parts = chunk.split_whitespace().rev();
                let (Some(kind), Some(count)) = (parts.next(), parts.next()) else {
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
    TestRun {
        outcome,
        test_code,
        fmt_code,
        passed,
        failed,
        failures,
        text,
    }
}

#[cfg(test)]
mod tests;
