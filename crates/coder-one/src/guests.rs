//! `evidence.guests`: the snapshot-read Wasm guests of
//! `programs/evidence-guests.json`, run by code in the probe stage.
//!
//! The program has three `module` steps: `repo_map` maps the workspace,
//! `code_search` searches it for the issue's terms, and `test_report`
//! parses the test reports it holds. The manifest's `evidence.guests`
//! switch turns them on, and it is off by default. Code decides whether
//! each step runs and with what input: the search runs only when the issue
//! yields search terms, and the report parser only when a file's content
//! shows it is a test report. Nothing here is offered to a model as a
//! tool. Each output becomes a [`Probe`], which faces the probe keep
//! question like a command's output, so what reaches the briefing is still
//! Jev's choice.
//!
//! This is a host of its own, not `coder::runtime`: Coder One doesn't
//! depend on `crates/coder`. It runs the same program file, under the same
//! rule that a step's bounds narrow the host's ceilings and never widen
//! them, and it checks each step's inline bytes against the digest the
//! step's target pins. Its grant differs from the terminal's in one way
//! that matters for evidence: a workspace larger than the grant's bounds
//! is cut, and the run says how much, rather than refused.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::component::evidence::Probe;
use crate::record::Implementation;

/// The program the guests come from, built into the binary so a task
/// container needs no checkout of this repository.
pub const PROGRAM: &str = include_str!("../../../programs/evidence-guests.json");

/// The steps the program defines, in the order they run.
pub const STEPS: [&str; 3] = ["repo_map", "code_search", "test_report"];

/// The host's ceilings. A step's bounds may only narrow them.
pub const CEILING: plugin::Limits = plugin::Limits {
    fuel: 2_000_000_000,
    memory_bytes: 128 * 1024 * 1024,
    output_bytes: 32 * 1024 * 1024,
    read_bytes: 16 * 1024 * 1024,
    module_bytes: 2 * 1024 * 1024,
};

/// Directories the grant never walks into.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "__pycache__",
    "target",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
];
/// The most files the grant's walk looks at.
const WALK_FILES: usize = 100_000;
/// The most files one grant holds.
const GRANT_FILES: usize = 20_000;
/// The most bytes of one file the grant keeps.
const GRANT_FILE_BYTES: usize = 64 * 1024;
/// The most bytes one grant holds.
const GRANT_BYTES: usize = 64 * 1024 * 1024;
/// Path segments past which the grant ranks a file as deep.
const SHALLOW_SEGMENTS: usize = 6;
/// Extensions the grant ranks as source, ahead of documents and data.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "mjs", "ts", "tsx", "jsx", "go", "java", "kt", "c", "h", "cc", "cpp", "hpp",
    "cs", "rb", "php", "swift", "scala", "sh", "lua", "ex", "exs", "hs", "ml", "zig", "toml",
];
/// The most search terms one search sends.
const MAX_TERMS: usize = 8;
/// The most report files one parse reads.
const MAX_REPORTS: usize = 20;
/// Lines of one search result the rendered output keeps per file.
const RENDER_LINES: usize = 5;

/// What the manifest asks of the guests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The steps to run, by name, from [`STEPS`].
    #[serde(default = "all_steps")]
    pub steps: Vec<String>,
    /// Seconds all the steps together may run.
    #[serde(default = "default_seconds")]
    pub seconds: u64,
}

fn all_steps() -> Vec<String> {
    STEPS.iter().map(ToString::to_string).collect()
}

fn default_seconds() -> u64 {
    10
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            steps: all_steps(),
            seconds: default_seconds(),
        }
    }
}

impl Policy {
    /// Why this policy can't run, if it can't.
    ///
    /// # Errors
    ///
    /// Returns the first problem: no steps, a step the program doesn't
    /// define or names twice, or no time.
    pub fn validate(&self) -> Result<(), String> {
        if self.steps.is_empty() {
            return Err("steps names no step".to_string());
        }
        for (index, step) in self.steps.iter().enumerate() {
            if !STEPS.contains(&step.as_str()) {
                return Err(format!(
                    "steps names {step}, which evidence-guests doesn't define"
                ));
            }
            if self.steps[..index].contains(step) {
                return Err(format!("steps names {step} twice"));
            }
        }
        if !(1..=120).contains(&self.seconds) {
            return Err("seconds must be 1 to 120".to_string());
        }
        Ok(())
    }
}

/// The implementation record for the guests component.
#[must_use]
pub fn implementation(policy: &Policy) -> Implementation {
    let digests: Vec<Value> = program()
        .map(|steps| {
            steps
                .iter()
                .map(|step| json!({"step": step.name, "digest": step.digest}))
                .collect()
        })
        .unwrap_or_default();
    Implementation::new(
        "evidence.guests",
        "evidence-guests v1",
        &json!({
            "policy": policy,
            "guests": digests,
            "grant": {
                "skip_dirs": SKIP_DIRS,
                "walk": WALK_FILES,
                "files": GRANT_FILES,
                "file_bytes": GRANT_FILE_BYTES,
                "bytes": GRANT_BYTES,
                "source": SOURCE_EXTENSIONS,
                "shallow_segments": SHALLOW_SEGMENTS,
            },
            "max_terms": MAX_TERMS,
        }),
    )
}

/// One `module` step of the program.
#[derive(Debug, Clone)]
pub struct Step {
    pub name: String,
    pub operation: String,
    pub read: Vec<String>,
    pub input: Value,
    pub wasm: Vec<u8>,
    /// The digest the step's target pins, which the bytes match.
    pub digest: String,
    pub limits: plugin::Limits,
}

/// The program's steps, checked.
///
/// # Errors
///
/// Returns why the program isn't one this host runs: a step that isn't a
/// `snapshot-read` module, bytes that don't match the pinned digest, a
/// read scope that isn't plain and relative, or a bound wider than
/// [`CEILING`].
pub fn program() -> Result<Vec<Step>, String> {
    parse(PROGRAM)
}

fn parse(text: &str) -> Result<Vec<Step>, String> {
    let document: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let definition = document["definition"]["steps"]
        .as_array()
        .ok_or("the program has no steps")?;
    let binding = document["binding"]["steps"]
        .as_object()
        .ok_or("the program has no host binding")?;
    let mut steps = Vec::new();
    for step in definition {
        let name = step["name"].as_str().ok_or("a step has no name")?;
        if step["kind"] != "module" {
            return Err(format!("step {name} isn't a module step"));
        }
        let host = binding
            .get(name)
            .ok_or_else(|| format!("step {name} has no host binding"))?;
        let module = &host["module"];
        if module["profile"] != "snapshot-read" {
            return Err(format!("step {name} isn't a snapshot-read guest"));
        }
        let wasm = plugin::decode_base64(module["bytes_base64"].as_str().unwrap_or_default())
            .map_err(|_| format!("step {name} carries no guest bytes"))?;
        let digest = plugin::digest(&wasm);
        let pinned = &step["target"]["artifact"];
        if pinned["digest"].as_str() != Some(digest.as_str())
            || pinned["size"].as_u64() != Some(wasm.len() as u64)
        {
            return Err(format!(
                "step {name}'s bytes are {digest}, not the digest its target pins"
            ));
        }
        let read = module["read"]
            .as_array()
            .ok_or_else(|| format!("step {name} names no read scope"))?
            .iter()
            .map(|path| {
                path.as_str()
                    .filter(|path| plain(path))
                    .map(str::to_string)
                    .ok_or_else(|| format!("step {name}'s read scope names {path}"))
            })
            .collect::<Result<_, _>>()?;
        steps.push(Step {
            name: name.to_string(),
            operation: module["operation"].as_str().unwrap_or_default().to_string(),
            read,
            input: module["input"].clone(),
            wasm,
            digest,
            limits: limits(name, &host["bounds"])?,
        });
    }
    Ok(steps)
}

/// A read-scope path: `.`, or `/`-separated segments that are neither
/// empty, `.`, nor `..`.
fn plain(path: &str) -> bool {
    path == "."
        || (!path.contains('\\')
            && !path.contains('\0')
            && path
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != ".."))
}

fn limits(name: &str, bounds: &Value) -> Result<plugin::Limits, String> {
    let bound = |key: &str, ceiling: u64| -> Result<u64, String> {
        match bounds.get(key).and_then(Value::as_u64) {
            None => Ok(ceiling),
            Some(value) if value <= ceiling && value > 0 => Ok(value),
            Some(value) => Err(format!(
                "step {name}'s {key} {value} is wider than this host's ceiling of {ceiling}"
            )),
        }
    };
    let bytes = |key: &str, ceiling: usize| -> Result<usize, String> {
        bound(key, ceiling as u64).map(|value| usize::try_from(value).unwrap_or(ceiling))
    };
    Ok(plugin::Limits {
        fuel: bound("fuel", CEILING.fuel)?,
        memory_bytes: bytes("memory_bytes", CEILING.memory_bytes)?,
        output_bytes: bytes("output_bytes", CEILING.output_bytes)?,
        read_bytes: bytes("read_bytes", CEILING.read_bytes)?,
        module_bytes: bytes("module_bytes", CEILING.module_bytes)?,
    })
}

/// The files one step may read, captured before the guest starts.
#[derive(Debug, Clone, Default)]
pub struct Grant {
    pub snapshot: plugin::Snapshot,
    pub handles: BTreeMap<String, String>,
    /// Captured files, by workspace-relative path, with their bytes.
    pub files: Vec<(String, usize)>,
    /// Files left out because the grant was full.
    pub omitted: usize,
    /// Files kept with fewer bytes than they have.
    pub cut: usize,
}

/// Capture the files under `scope` in `workdir`: one directory,
/// `workspace`, whose handle is `root`, listing each file as
/// `workspace/<relative path>`, as the terminal's runtime grants one.
/// Symlinks are left out and never followed, and so are the directories
/// in [`SKIP_DIRS`].
///
/// Code orders the grant, and a guest reads in listing order: shallow files
/// before deep ones, such as a checked-in trace archive; within each,
/// files whose path names one of `keywords`, then source files, then the
/// rest, each group by path. When the grant fills, the files it leaves out
/// are the last in that order, and [`Grant::omitted`] counts them.
#[must_use]
pub fn grant(workdir: &Path, scope: &[String], keywords: &[String]) -> Grant {
    let mut found: BTreeMap<String, (std::path::PathBuf, usize)> = BTreeMap::new();
    for path in scope {
        let start = if path == "." {
            workdir.to_path_buf()
        } else {
            workdir.join(path)
        };
        walk(workdir, &start, &mut found);
    }
    let terms: Vec<String> = keywords
        .iter()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| term.chars().count() >= 3)
        .collect();
    let mut ranked: Vec<(Rank, String, std::path::PathBuf, usize)> = found
        .into_iter()
        .map(|(relative, (path, size))| (rank(&relative, &terms), relative, path, size))
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut grant = Grant::default();
    let mut total = 0_usize;
    let mut children = Vec::new();
    for (_, relative, path, size) in ranked {
        let keep = size.min(GRANT_FILE_BYTES);
        if children.len() >= GRANT_FILES || total.saturating_add(keep) > GRANT_BYTES {
            grant.omitted += 1;
            continue;
        }
        let mut bytes = Vec::with_capacity(keep);
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        if std::io::Read::read_to_end(&mut std::io::Read::take(file, keep as u64), &mut bytes)
            .is_err()
        {
            continue;
        }
        total += bytes.len();
        let complete = bytes.len() == size;
        if !complete {
            grant.cut += 1;
        }
        let label = format!("workspace/{relative}");
        let version = plugin::digest(&bytes);
        let length = bytes.len();
        let entry = plugin::Entry::File {
            bytes,
            version,
            complete,
        };
        if grant.snapshot.insert(&label, entry).is_ok() {
            children.push(label);
            grant.files.push((relative, length));
        }
    }
    let _ = grant
        .snapshot
        .insert("workspace", plugin::Entry::Directory { children });
    grant.handles = BTreeMap::from([("workspace".to_string(), "root".to_string())]);
    grant
}

/// A file's place in the grant, lowest first: whether it is deeper than
/// [`SHALLOW_SEGMENTS`], whether its path misses every search term, and
/// whether it isn't a source file.
type Rank = (bool, bool, bool);

/// The [`Rank`] of one workspace-relative path.
fn rank(relative: &str, terms: &[String]) -> Rank {
    let lower = relative.to_ascii_lowercase();
    let deep = lower.split('/').count() > SHALLOW_SEGMENTS;
    let named = terms.iter().any(|term| lower.contains(term.as_str()));
    let extension = lower
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension);
    (deep, !named, !SOURCE_EXTENSIONS.contains(&extension))
}

/// Collects every regular file under `path` with its size, by
/// workspace-relative path, at most [`WALK_FILES`].
fn walk(root: &Path, path: &Path, found: &mut BTreeMap<String, (std::path::PathBuf, usize)>) {
    if found.len() >= WALK_FILES {
        return;
    }
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    let git = path
        .file_name()
        .is_some_and(|name| name == ".git" && path != root);
    if meta.file_type().is_symlink() || git {
        return;
    }
    if meta.is_dir() {
        let skipped = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| SKIP_DIRS.contains(&name));
        if skipped && path != root {
            return;
        }
        let Ok(read) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<_> = read.flatten().map(|entry| entry.path()).collect();
        children.sort();
        for child in children {
            walk(root, &child, found);
        }
        return;
    }
    if !meta.is_file() {
        return;
    }
    let Some(relative) = path.strip_prefix(root).ok().and_then(|relative| {
        relative
            .components()
            .map(|part| part.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()
            .map(|parts| parts.join("/"))
    }) else {
        return;
    };
    if relative.is_empty() {
        return;
    }
    let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
    found.entry(relative).or_insert((path.to_path_buf(), size));
}

/// What one step did.
#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub step: String,
    /// `ok`, `skipped`, or `refused`.
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub elapsed_ms: u64,
    /// The input code chose for the guest.
    #[serde(skip_serializing_if = "Value::is_null")]
    pub input: Value,
    /// What the guest returned.
    #[serde(skip_serializing_if = "Value::is_null")]
    pub value: Value,
    /// The value rendered for the probe keep question, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Probe>,
}

impl Run {
    fn skipped(step: &str, reason: impl Into<String>) -> Self {
        Run {
            step: step.to_string(),
            status: "skipped",
            reason: Some(reason.into()),
            elapsed_ms: 0,
            input: Value::Null,
            value: Value::Null,
            probe: None,
        }
    }
}

/// Run the steps `policy` names over `workdir`, within `limit`, with the
/// search terms `keywords`. A step that can't run says why; none of them
/// stops another.
pub async fn run(
    workdir: &Path,
    keywords: &[String],
    policy: &Policy,
    limit: Duration,
) -> Vec<Run> {
    let steps = match program() {
        Ok(steps) => steps,
        Err(reason) => {
            return policy
                .steps
                .iter()
                .map(|step| Run {
                    status: "refused",
                    ..Run::skipped(step, reason.clone())
                })
                .collect();
        }
    };
    let started = Instant::now();
    let mut grants: BTreeMap<Vec<String>, Arc<Grant>> = BTreeMap::new();
    let mut runs = Vec::new();
    for name in STEPS
        .iter()
        .filter(|step| policy.steps.iter().any(|s| s == *step))
    {
        let Some(step) = steps.iter().find(|step| step.name == *name) else {
            runs.push(Run::skipped(name, "the program has no such step"));
            continue;
        };
        let grant = Arc::clone(
            grants
                .entry(step.read.clone())
                .or_insert_with(|| Arc::new(grant(workdir, &step.read, keywords))),
        );
        let input = match choose(step, keywords, &grant) {
            Ok(input) => input,
            Err(reason) => {
                runs.push(Run::skipped(name, reason));
                continue;
            }
        };
        let Some(left) = limit
            .checked_sub(started.elapsed())
            .filter(|left| !left.is_zero())
        else {
            runs.push(Run::skipped(name, "the guests' time ran out"));
            continue;
        };
        runs.push(invoke(step, input, grant, left).await);
    }
    runs
}

/// The input code gives a step, or why the step shouldn't run.
fn choose(step: &Step, keywords: &[String], grant: &Grant) -> Result<Value, String> {
    let mut input = step.input.as_object().cloned().unwrap_or_else(Map::new);
    match step.name.as_str() {
        "code_search" => {
            let terms: Vec<Value> = keywords
                .iter()
                .map(|term| term.trim())
                .filter(|term| (3..=200).contains(&term.chars().count()))
                .take(MAX_TERMS)
                .map(|term| Value::String(term.to_string()))
                .collect();
            if terms.is_empty() {
                return Err("the issue names no search terms".to_string());
            }
            input.insert("patterns".into(), Value::Array(terms));
        }
        "test_report" => {
            let reports = reports(grant);
            if reports.is_empty() {
                return Err("the workspace holds no test report".to_string());
            }
            input.insert(
                "paths".into(),
                Value::Array(reports.into_iter().map(Value::String).collect()),
            );
        }
        _ => {}
    }
    Ok(Value::Object(input))
}

/// The granted files whose content shows a test report, at most
/// [`MAX_REPORTS`].
fn reports(grant: &Grant) -> Vec<String> {
    const MARKERS: &[&str] = &[
        "<testsuite",
        "<testcase",
        "test result:",
        "short test summary info",
        "= FAILURES =",
    ];
    grant
        .files
        .iter()
        .filter(|(path, _)| {
            let extension = path.rsplit_once('.').map_or("", |(_, extension)| extension);
            matches!(extension, "xml" | "txt" | "log" | "out")
        })
        .filter(|(path, _)| {
            let Some(plugin::Entry::File { bytes, .. }) =
                grant.snapshot.get(&format!("workspace/{path}"))
            else {
                return false;
            };
            let text = String::from_utf8_lossy(bytes);
            MARKERS.iter().any(|marker| text.contains(marker))
        })
        .map(|(path, _)| path.clone())
        .take(MAX_REPORTS)
        .collect()
}

/// Run one guest on a blocking thread, stopping it when `left` runs out.
async fn invoke(step: &Step, input: Value, grant: Arc<Grant>, left: Duration) -> Run {
    let started = Instant::now();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    let wasm = step.wasm.clone();
    let operation = step.operation.clone();
    let limits = step.limits;
    let invocation = format!("evidence-guests/{}", step.name);
    let call_input = input.clone();
    let mut guest = tokio::task::spawn_blocking(move || {
        plugin::invoke(plugin::Call {
            wasm: &wasm,
            profile: plugin::Profile::SnapshotRead,
            invocation: &invocation,
            operation: &operation,
            input: &call_input,
            snapshot: &grant.snapshot,
            handles: &grant.handles,
            limits,
            cancelled: flag,
            required: true,
        })
        .map(|value| (value, grant))
    });
    let outcome = match tokio::time::timeout(left, &mut guest).await {
        Ok(joined) => joined.map_err(|error| error.to_string()),
        Err(_) => {
            cancelled.store(true, Ordering::SeqCst);
            let _ = guest.await;
            Err("the guests' time ran out".to_string())
        }
    };
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let refused = |reason: String| Run {
        step: step.name.clone(),
        status: "refused",
        reason: Some(reason),
        elapsed_ms,
        input: input.clone(),
        value: Value::Null,
        probe: None,
    };
    match outcome {
        Ok(Ok((value, grant))) => {
            let output = render(&step.name, &value.value, &grant);
            Run {
                step: step.name.clone(),
                status: "ok",
                reason: None,
                elapsed_ms,
                input: input.clone(),
                probe: output.map(|output| Probe {
                    command: command(&step.name, &input),
                    output,
                }),
                value: value.value,
            }
        }
        Ok(Err(error)) => refused(error.to_string()),
        Err(reason) => refused(reason),
    }
}

/// The label a guest's output carries in the probe keep question and the
/// briefing.
fn command(step: &str, input: &Value) -> String {
    match step {
        "code_search" => {
            let terms: Vec<&str> = input["patterns"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            format!("guest code-search: {}", terms.join(", "))
        }
        "test_report" => "guest test-report".to_string(),
        _ => "guest repo-map".to_string(),
    }
}

/// A guest's value as text for the briefing, or `None` when it found
/// nothing worth showing.
#[must_use]
pub fn render(step: &str, value: &Value, grant: &Grant) -> Option<String> {
    let mut out = String::new();
    let omitted = if grant.omitted > 0 || grant.cut > 0 {
        format!(
            " (the grant left out {} files and cut {} to {} KiB)",
            grant.omitted,
            grant.cut,
            GRANT_FILE_BYTES / 1024
        )
    } else {
        String::new()
    };
    match step {
        "repo_map" => {
            if value["files"].as_u64().unwrap_or(0) == 0 {
                return None;
            }
            out.push_str(&format!(
                "{} files, {} bytes{}{omitted}\n",
                value["files"],
                value["bytes"],
                if value["complete"] == true {
                    ""
                } else {
                    ", listing incomplete"
                }
            ));
            let languages: Vec<String> = rows(&value["languages"])
                .map(|row| {
                    format!(
                        "{} {} files ({} bytes)",
                        text(&row["language"]),
                        row["files"],
                        row["bytes"]
                    )
                })
                .collect();
            out.push_str(&format!("languages: {}\n", languages.join(", ")));
            let top: Vec<String> = rows(&value["top"])
                .map(|row| {
                    if row["kind"] == "dir" {
                        format!("{}/ ({} files)", text(&row["path"]), row["files"])
                    } else {
                        text(&row["path"]).to_string()
                    }
                })
                .collect();
            out.push_str(&format!("top level: {}\n", top.join(", ")));
            let dirs: Vec<String> = rows(&value["dirs"])
                .take(15)
                .map(|row| format!("{}/ ({} files)", text(&row["path"]), row["files"]))
                .collect();
            if !dirs.is_empty() {
                out.push_str(&format!("largest directories: {}\n", dirs.join(", ")));
            }
            let manifests: Vec<&str> = rows(&value["manifests"]).map(text).collect();
            if !manifests.is_empty() {
                out.push_str(&format!("build manifests: {}\n", manifests.join(", ")));
            }
            let tests: Vec<String> = rows(&value["tests"]["dirs"])
                .map(|row| format!("{} ({})", text(&row["path"]), row["files"]))
                .collect();
            out.push_str(&format!(
                "test files: {}{}\n",
                value["tests"]["files"],
                if tests.is_empty() {
                    String::new()
                } else {
                    format!(" in {}", tests.join(", "))
                }
            ));
            let largest: Vec<String> = rows(&value["largest"])
                .take(5)
                .map(|row| format!("{} ({} bytes)", text(&row["path"]), row["bytes"]))
                .collect();
            out.push_str(&format!("largest files: {}\n", largest.join(", ")));
        }
        "code_search" => {
            if value["files_matched"].as_u64().unwrap_or(0) == 0 {
                return None;
            }
            out.push_str(&format!(
                "{} files matched, {} lines{}{omitted}\n",
                value["files_matched"],
                value["matches_total"],
                if value["truncated"] == true {
                    "; some left out"
                } else {
                    ""
                }
            ));
            for file in rows(&value["files"]) {
                out.push_str(&format!(
                    "{} ({} terms, {} lines)\n",
                    text(&file["path"]),
                    file["patterns"],
                    file["matches"]
                ));
                for line in rows(&file["lines"]).take(RENDER_LINES) {
                    out.push_str(&format!("  {}: {}\n", line["line"], text(&line["text"])));
                }
            }
        }
        "test_report" => {
            if value["failures_total"].as_u64().unwrap_or(0) == 0 {
                return None;
            }
            for report in rows(&value["reports"]) {
                let counts: Vec<String> = ["tests", "passed", "failed", "errors", "skipped"]
                    .iter()
                    .filter_map(|key| report[*key].as_u64().map(|n| format!("{n} {key}")))
                    .collect();
                out.push_str(&format!(
                    "{} ({}): {}\n",
                    text(&report["path"]),
                    text(&report["format"]),
                    counts.join(", ")
                ));
                for failure in rows(&report["failures"]) {
                    let place = match (failure["file"].as_str(), failure["line"].as_u64()) {
                        (Some(file), Some(line)) => format!(" at {file}:{line}"),
                        (Some(file), None) => format!(" in {file}"),
                        _ => String::new(),
                    };
                    let message = failure["message"]
                        .as_str()
                        .map(|message| format!(": {message}"))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "  {} {}{place}{message}\n",
                        if failure["kind"] == "error" {
                            "ERROR"
                        } else {
                            "FAILED"
                        },
                        text(&failure["test"])
                    ));
                }
            }
        }
        _ => return None,
    }
    Some(out)
}

fn rows(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("tests")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::fs::write(
            root.join("src/ledger.py"),
            "class Ledger:\n    RETRY_LIMIT = 3\n\n    def post(self, entry):\n        pass\n",
        )
        .unwrap();
        std::fs::write(
            root.join("tests/test_ledger.py"),
            "from ledger import Ledger\n\ndef test_post():\n    Ledger().post(1)\n",
        )
        .unwrap();
        std::fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"ledger\"\n",
        )
        .unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(
            root.join("out/pytest.log"),
            "=== test session starts ===\n\
             ___ test_post ___\n\
             tests/test_ledger.py:4: AttributeError\n\
             === short test summary info ===\n\
             FAILED tests/test_ledger.py::test_post - AttributeError: no post\n\
             === 1 failed in 0.01s ===\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn the_program_pins_every_guest_it_carries() {
        let steps = program().expect("the built-in program checks");
        let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
        assert_eq!(names, STEPS);
        for step in &steps {
            assert_eq!(step.read, ["."]);
            assert!(step.limits.fuel <= CEILING.fuel);
            let receipt: Value = serde_json::from_str(
                &std::fs::read_to_string(format!(
                    "{}/../plugin/fixtures/{}.receipt.json",
                    env!("CARGO_MANIFEST_DIR"),
                    step.name.replace('_', "-")
                ))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(
                receipt["guest_digest"],
                step.digest.as_str(),
                "{}",
                step.name
            );
        }
    }

    #[test]
    fn a_tampered_or_widened_step_is_refused() {
        let mut document: Value = serde_json::from_str(PROGRAM).unwrap();
        document["binding"]["steps"]["repo_map"]["module"]["bytes_base64"] =
            json!(plugin::encode_base64(b"not the guest"));
        let error = parse(&document.to_string()).unwrap_err();
        assert!(error.contains("not the digest its target pins"), "{error}");

        let mut document: Value = serde_json::from_str(PROGRAM).unwrap();
        document["binding"]["steps"]["code_search"]["bounds"]["fuel"] = json!(u64::MAX);
        let error = parse(&document.to_string()).unwrap_err();
        assert!(error.contains("wider than this host's ceiling"), "{error}");

        let mut document: Value = serde_json::from_str(PROGRAM).unwrap();
        document["binding"]["steps"]["test_report"]["module"]["read"] = json!(["../elsewhere"]);
        assert!(parse(&document.to_string()).is_err());
    }

    #[test]
    fn the_grant_skips_git_and_holds_the_files() {
        let dir = workspace();
        let named = grant(dir.path(), &[".".to_string()], &["pytest".to_string()]);
        let paths: Vec<&str> = named.files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "out/pytest.log",
                "pyproject.toml",
                "src/ledger.py",
                "tests/test_ledger.py"
            ],
            "a path naming a term first, then source, each by path"
        );
        let grant = grant(dir.path(), &[".".to_string()], &[]);
        let paths: Vec<&str> = grant.files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "pyproject.toml",
                "src/ledger.py",
                "tests/test_ledger.py",
                "out/pytest.log"
            ]
        );
        assert_eq!(grant.omitted, 0);
        assert_eq!(reports(&grant), ["out/pytest.log"]);
    }

    #[tokio::test]
    async fn every_guest_runs_and_becomes_a_probe() {
        let dir = workspace();
        let keywords = vec!["Ledger".to_string(), "retry_limit".to_string()];
        let runs = run(
            dir.path(),
            &keywords,
            &Policy::default(),
            Duration::from_secs(60),
        )
        .await;
        let statuses: Vec<(&str, &str)> = runs
            .iter()
            .map(|run| (run.step.as_str(), run.status))
            .collect();
        assert_eq!(
            statuses,
            [
                ("repo_map", "ok"),
                ("code_search", "ok"),
                ("test_report", "ok")
            ],
            "{runs:?}"
        );
        let map = runs[0].probe.as_ref().unwrap();
        assert_eq!(map.command, "guest repo-map");
        assert!(map.output.starts_with("4 files"), "{}", map.output);
        assert!(map.output.contains("build manifests: pyproject.toml"));
        let search = runs[1].probe.as_ref().unwrap();
        assert_eq!(search.command, "guest code-search: Ledger, retry_limit");
        assert!(
            search.output.contains("src/ledger.py (2 terms"),
            "{}",
            search.output
        );
        let report = runs[2].probe.as_ref().unwrap();
        assert!(
            report
                .output
                .contains("FAILED tests/test_ledger.py::test_post at tests/test_ledger.py:4"),
            "{}",
            report.output
        );
    }

    #[tokio::test]
    async fn code_decides_which_guests_run() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.md"), "nothing to see\n").unwrap();
        let runs = run(dir.path(), &[], &Policy::default(), Duration::from_secs(60)).await;
        assert_eq!(runs[0].status, "ok");
        assert_eq!(runs[1].status, "skipped");
        assert_eq!(
            runs[1].reason.as_deref(),
            Some("the issue names no search terms")
        );
        assert_eq!(runs[2].status, "skipped");
        assert_eq!(
            runs[2].reason.as_deref(),
            Some("the workspace holds no test report")
        );
        let only = Policy {
            steps: vec!["repo_map".to_string()],
            ..Policy::default()
        };
        let runs = run(dir.path(), &[], &only, Duration::from_secs(60)).await;
        assert_eq!(runs.len(), 1);
    }

    /// How long the guests take on this repository's own checkout. Run with
    /// `cargo test -p coder-one --lib guests_on_this_checkout -- --ignored
    /// --nocapture`.
    #[tokio::test]
    #[ignore = "reads the whole checkout; a timing check, not a unit test"]
    async fn guests_on_this_checkout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let started = Instant::now();
        let grant = grant(&root, &[".".to_string()], &[]);
        println!(
            "grant: {} files, {} omitted, {} cut, in {} ms",
            grant.files.len(),
            grant.omitted,
            grant.cut,
            started.elapsed().as_millis()
        );
        let keywords = vec!["snapshot".to_string(), "read_scope".to_string()];
        let runs = run(
            &root,
            &keywords,
            &Policy::default(),
            Duration::from_secs(120),
        )
        .await;
        for run in &runs {
            println!(
                "{} {} {} ms {:?} {} chars",
                run.step,
                run.status,
                run.elapsed_ms,
                run.reason,
                run.probe.as_ref().map_or(0, |probe| probe.output.len())
            );
            if let Some(probe) = &run.probe {
                println!(
                    "$ {}\n{}",
                    probe.command,
                    crate::judge::clip(&probe.output, 800)
                );
            }
        }
        assert!(runs.iter().any(|run| run.status == "ok"));
    }

    #[test]
    fn a_policy_names_only_the_programs_steps() {
        assert!(Policy::default().validate().is_ok());
        let unknown = Policy {
            steps: vec!["git_facts".to_string()],
            ..Policy::default()
        };
        assert!(unknown.validate().is_err());
        let twice = Policy {
            steps: vec!["repo_map".to_string(), "repo_map".to_string()],
            ..Policy::default()
        };
        assert!(twice.validate().is_err());
        let parsed: Policy = serde_json::from_value(json!({})).unwrap();
        assert_eq!(parsed, Policy::default());
    }
}
