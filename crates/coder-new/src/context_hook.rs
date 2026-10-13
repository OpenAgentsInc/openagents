//! System One hook 5 (#11490): instruction files added on the first touch
//! of a directory.
//!
//! What Claude Code loads on its own, from its documented behavior (not
//! re-tested against 2.1.29x here): `CLAUDE.md` files in the working
//! directory and its ancestors at start, `CLAUDE.md` files in subdirectories
//! lazily when it reads a file there, and never `AGENTS.md`. So the only
//! file this hook adds is an `AGENTS.md` (in the touched directory or an
//! ancestor up to the repo root) that is not the briefing's, and not a twin
//! of a `CLAUDE.md` beside it (the same file by link or by text, which
//! Claude Code loads itself).
//!
//! A `PostToolUse` hook on `Read|Edit|Write|MultiEdit|Grep|Glob` watches the
//! directories the agent touches. The first touch of a directory in a session
//! finds the candidates by rule. One small candidate is added as it is. More
//! than one, or a large one, goes to one Decisions API call with a question
//! per candidate; the files with p >= 0.5 are added as `additionalContext`
//! (at the tail, so the cached prefix stays cached), up to 3 KB. A failed
//! call adds nothing unless the rule alone settles it. A file is added or
//! refused once per session. Each decision is a JSON line in
//! `~/.openagents/decisions/context.jsonl` (`CODER_CONTEXT_LOG`).
//! `CODER_CONTEXT=off` turns it off.
//!
//! `UserPromptSubmit` records the session's first message, the task the
//! questions are asked against. Not built: re-adding the briefing's key facts
//! after a compaction (`PostCompact` carries no context output; see #11490).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use claude_agent_sdk::{HookMatcher, SyncHookJSONOutput, hook_fn};
use serde_json::{Value, json};
use squeeze::Judge;

/// The tools whose use counts as touching a directory.
pub const TOOLS: &str = "Read|Edit|Write|MultiEdit|Grep|Glob";
/// Where decisions are logged.
pub const LOG_VAR: &str = "CODER_CONTEXT_LOG";
/// A candidate larger than this is judged, not added on the rule's say-so.
const LARGE: usize = 4 * 1024;
/// The most one touch adds.
const BUDGET: usize = 3 * 1024;
/// What the judge sees of each candidate.
const SNIPPET: usize = 1_500;
/// Probability that keeps a candidate.
const KEEP_AT: f64 = 0.5;
/// The task text the judge sees.
const TASK_CHARS: usize = 2_000;

#[derive(Default)]
struct Session {
    task: Option<String>,
    dirs: HashSet<PathBuf>,
    /// Files added or refused; never asked about again.
    decided: HashSet<PathBuf>,
}

static SESSIONS: LazyLock<Mutex<HashMap<String, Session>>> = LazyLock::new(Mutex::default);
/// Files the briefing already carries, by working directory.
static BRIEFED: LazyLock<Mutex<HashMap<PathBuf, Vec<PathBuf>>>> = LazyLock::new(Mutex::default);

/// Records the instruction files `cwd`'s briefing carries, so the hook skips them.
pub fn note_briefed(cwd: &Path, files: &[PathBuf]) {
    if let Ok(mut map) = BRIEFED.lock() {
        map.insert(cwd.to_owned(), files.to_vec());
    }
}

fn with_session<T>(id: &str, f: impl FnOnce(&mut Session) -> T) -> T {
    let mut map = SESSIONS.lock().unwrap_or_else(|e| e.into_inner());
    if map.len() > 64 && !map.contains_key(id) {
        map.clear();
    }
    f(map.entry(id.to_owned()).or_default())
}

/// The hooks for a session in `cwd`.
#[must_use]
pub fn hooks(cwd: PathBuf) -> Vec<(claude_agent_sdk::HookEvent, HookMatcher)> {
    let judge: Arc<dyn Judge + Send + Sync> = Arc::new(squeeze::Decisions);
    let post = hook_fn(move |input: Value, _| {
        let cwd = cwd.clone();
        let judge = Arc::clone(&judge);
        async move {
            let output = tokio::task::spawn_blocking(move || on_tool(&input, &cwd, &*judge))
                .await
                .ok()
                .flatten();
            Ok(match output {
                Some(specific) => SyncHookJSONOutput {
                    hook_specific_output: Some(specific),
                    ..SyncHookJSONOutput::default()
                }
                .into(),
                None => SyncHookJSONOutput::continue_execution().into(),
            })
        }
    });
    let prompt = hook_fn(|input: Value, _| async move {
        if let (Some(id), Some(text)) = (input["session_id"].as_str(), input["prompt"].as_str()) {
            with_session(id, |s| {
                s.task
                    .get_or_insert_with(|| text.chars().take(TASK_CHARS).collect());
            });
        }
        Ok(SyncHookJSONOutput::continue_execution().into())
    });
    vec![
        (
            claude_agent_sdk::HookEvent::PostToolUse,
            HookMatcher::new(Some(TOOLS), post),
        ),
        (
            claude_agent_sdk::HookEvent::UserPromptSubmit,
            HookMatcher::new(None, prompt),
        ),
    ]
}

/// The directory a tool call touched.
fn touched(input: &Value, cwd: &Path) -> Option<PathBuf> {
    let args = &input["tool_input"];
    let base = input["cwd"]
        .as_str()
        .map_or_else(|| cwd.to_owned(), PathBuf::from);
    let raw = args["file_path"]
        .as_str()
        .or_else(|| args["path"].as_str())
        .map(PathBuf::from);
    let path = match raw {
        Some(p) if p.is_absolute() => p,
        Some(p) => base.join(p),
        None => return Some(base),
    };
    if path.is_dir() || matches!(input["tool_name"].as_str(), Some("Grep" | "Glob")) {
        Some(path)
    } else {
        path.parent().map(Path::to_owned)
    }
}

/// The checkout's root: the nearest ancestor of `dir` with `.git`, else `dir`.
fn repo_root(dir: &Path) -> PathBuf {
    dir.ancestors()
        .find(|a| a.join(".git").exists())
        .unwrap_or(dir)
        .to_owned()
}

/// Whether Claude Code loads `agents` itself: a `CLAUDE.md` beside it that is
/// the same file or has the same text.
fn twin_of_claude_md(agents: &Path) -> bool {
    let Some(claude) = agents.parent().map(|d| d.join("CLAUDE.md")) else {
        return false;
    };
    if !claude.exists() {
        return false;
    }
    if let (Ok(a), Ok(c)) = (agents.canonicalize(), claude.canonicalize())
        && a == c
    {
        return true;
    }
    matches!((std::fs::read(agents), std::fs::read(&claude)), (Ok(a), Ok(c)) if a == c)
}

/// The candidates for `dir`: `AGENTS.md` files from `dir` up to the repo
/// root that nothing has decided about and the briefing does not carry.
fn candidates(dir: &Path, cwd: &Path, decided: &HashSet<PathBuf>) -> Vec<(PathBuf, String)> {
    let root = repo_root(dir);
    let briefed: Vec<PathBuf> = BRIEFED
        .lock()
        .ok()
        .and_then(|m| m.get(cwd).cloned())
        .unwrap_or_default();
    let mut out = Vec::new();
    for ancestor in dir.ancestors() {
        let file = ancestor.join("AGENTS.md");
        let real = file.canonicalize().unwrap_or_else(|_| file.clone());
        if file.is_file()
            && !decided.contains(&real)
            && !briefed.iter().any(|b| *b == file || *b == real)
            && !twin_of_claude_md(&file)
            && let Ok(text) = std::fs::read_to_string(&file)
            && !text.trim().is_empty()
        {
            out.push((real, text));
        }
        if ancestor == root {
            break;
        }
    }
    out
}

fn cut(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// One `PostToolUse` call: the `hookSpecificOutput` to return, if any.
pub fn on_tool(input: &Value, cwd: &Path, judge: &dyn Judge) -> Option<Value> {
    if std::env::var("CODER_CONTEXT").is_ok_and(|v| v.trim().eq_ignore_ascii_case("off")) {
        return None;
    }
    let id = input["session_id"].as_str().unwrap_or("").to_owned();
    let dir = touched(input, cwd)?;
    let (first, task, decided) = with_session(&id, |s| {
        (
            s.dirs.insert(dir.clone()),
            s.task.clone(),
            s.decided.clone(),
        )
    });
    if !first {
        return None;
    }
    let found = candidates(&dir, cwd, &decided);
    if found.is_empty() {
        return None;
    }
    let mut how = "rule";
    let mut error = None;
    let mut meta = None;
    let mut rated: Vec<(String, f64)> = Vec::new();
    let mut refused: Vec<usize> = Vec::new();
    let keep: Vec<usize> = if found.len() == 1 && found[0].1.len() <= LARGE {
        vec![0]
    } else {
        let mut input_text = format!(
            "A coding agent is working in {}.\nIts task: {}\n\nInstruction files it has not read:\n",
            dir.display(),
            task.as_deref().unwrap_or("(not known)")
        );
        let mut questions = Vec::new();
        for (i, (path, text)) in found.iter().enumerate() {
            input_text.push_str(&format!(
                "\n[file {i}] {}\n{}\n",
                path.display(),
                cut(text, SNIPPET)
            ));
            questions.push((
                format!("f{i}"),
                format!(
                    "Does [file {i}] hold guidance the agent needs for the change it is making in {}?",
                    dir.display()
                ),
            ));
        }
        match judge.rate(&input_text, &questions) {
            Ok((model, door, latency, probs)) => {
                how = "decision";
                meta = Some((model, door, latency));
                let mut picked: Vec<(usize, f64)> = probs
                    .iter()
                    .filter_map(|(n, p)| Some((n.strip_prefix('f')?.parse().ok()?, *p)))
                    .filter(|(i, _)| *i < found.len())
                    .collect();
                rated = probs;
                picked.sort_by(|a, b| b.1.total_cmp(&a.1));
                refused = picked
                    .iter()
                    .filter(|(_, p)| *p < KEEP_AT)
                    .map(|(i, _)| *i)
                    .collect();
                picked
                    .into_iter()
                    .filter(|(_, p)| *p >= KEEP_AT)
                    .map(|(i, _)| i)
                    .collect()
            }
            Err(e) => {
                how = "rule (decision unavailable)";
                error = Some(e);
                Vec::new()
            }
        }
    };
    let mut text = String::new();
    let mut added = Vec::new();
    for &i in &keep {
        let (path, body) = &found[i];
        let head = format!("Instructions from {}:\n", path.display());
        let room = BUDGET.saturating_sub(text.len() + head.len() + 2);
        if room < 200 {
            break;
        }
        let body = if body.len() > room {
            cut(body, room)
        } else {
            body.clone()
        };
        text.push_str(&format!("{head}{}\n\n", body.trim_end()));
        added.push(path.clone());
    }
    with_session(&id, |s| {
        s.decided.extend(added.iter().cloned());
        s.decided
            .extend(refused.iter().map(|&i| found[i].0.clone()));
    });
    let mut line = json!({
        "hook": "context",
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        "dir": dir.display().to_string(),
        "candidates": found.iter().map(|(p, t)| json!({"path": p.display().to_string(), "bytes": t.len()})).collect::<Vec<_>>(),
        "added": added.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "added_bytes": text.len(),
        "how": how,
    });
    if let Some((model, door, latency)) = meta {
        line["model"] = json!(model);
        line["door"] = json!(door);
        line["latency_ms"] = json!(latency);
        line["threshold"] = json!(KEEP_AT);
        line["rated"] = json!(
            rated
                .iter()
                .map(|(n, p)| json!({"file": n, "p": p}))
                .collect::<Vec<_>>()
        );
    }
    if let Some(e) = error {
        line["error"] = json!(e);
    }
    append_log(&line);
    if text.is_empty() {
        return None;
    }
    Some(json!({"hookEventName": "PostToolUse", "additionalContext": text.trim_end()}))
}

fn log_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(LOG_VAR)
        && !path.trim().is_empty()
    {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".openagents/decisions/context.jsonl"))
}

fn append_log(line: &Value) {
    // One shared format for every hook's decisions (#11492).
    if let Some(path) = log_path() {
        hook_log::append(&path, &hook_log::from_line(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake(Result<Vec<(String, f64)>, String>, AtomicUsize);
    impl Fake {
        fn new(r: Result<Vec<(String, f64)>, String>) -> Self {
            Self(r, AtomicUsize::new(0))
        }
        fn calls(&self) -> usize {
            self.1.load(Ordering::Relaxed)
        }
    }
    impl Judge for Fake {
        #[allow(clippy::type_complexity)]
        fn rate(
            &self,
            _: &str,
            _: &[(String, String)],
        ) -> Result<(String, Option<String>, u64, Vec<(String, f64)>), String> {
            self.1.fetch_add(1, Ordering::Relaxed);
            self.0.clone().map(|p| ("fake".into(), None, 1, p))
        }
    }

    fn repo() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join(".git")).unwrap();
        // Keep the tests' log out of the person's home.
        // SAFETY: every test sets the same value.
        unsafe {
            std::env::set_var(
                LOG_VAR,
                std::env::temp_dir().join("coder-context-test.jsonl"),
            );
        }
        t
    }

    fn read(sid: &str, file: &Path) -> Value {
        json!({"session_id": sid, "tool_name": "Read", "tool_input": {"file_path": file}})
    }

    fn text(out: Option<Value>) -> String {
        out.map(|v| v["additionalContext"].as_str().unwrap().to_owned())
            .unwrap_or_default()
    }

    #[test]
    fn one_small_file_is_added_without_a_call_once_per_session() {
        let t = repo();
        let sub = t.path().join("a");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("AGENTS.md"), "use the foo macro").unwrap();
        let judge = Fake::new(Err("no".into()));
        let out = text(on_tool(&read("s1", &sub.join("x.rs")), t.path(), &judge));
        assert!(out.contains("use the foo macro") && out.contains("AGENTS.md"));
        assert_eq!(judge.calls(), 0);
        // Same directory again, and a new directory below: nothing added twice.
        assert!(on_tool(&read("s1", &sub.join("x.rs")), t.path(), &judge).is_none());
        std::fs::create_dir(sub.join("b")).unwrap();
        assert!(on_tool(&read("s1", &sub.join("b/y.rs")), t.path(), &judge).is_none());
        // Another session gets it again.
        assert!(!text(on_tool(&read("s2", &sub.join("x.rs")), t.path(), &judge)).is_empty());
    }

    #[test]
    fn claude_md_twins_and_briefed_files_are_skipped() {
        let t = repo();
        std::fs::write(t.path().join("CLAUDE.md"), "same").unwrap();
        std::fs::write(t.path().join("AGENTS.md"), "same").unwrap();
        let sub = t.path().join("c");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("AGENTS.md"), "briefed").unwrap();
        note_briefed(t.path(), &[sub.join("AGENTS.md")]);
        let judge = Fake::new(Err("no".into()));
        assert!(on_tool(&read("s3", &sub.join("x.rs")), t.path(), &judge).is_none());
    }

    #[test]
    fn several_candidates_are_judged_and_p_below_half_is_left_out() {
        let t = repo();
        let sub = t.path().join("d");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(t.path().join("AGENTS.md"), "root rule").unwrap();
        std::fs::write(sub.join("AGENTS.md"), "dir rule").unwrap();
        // The directory's own file comes first: f0 = d, f1 = root.
        let judge = Fake::new(Ok(vec![("f0".into(), 0.9), ("f1".into(), 0.2)]));
        let out = text(on_tool(&read("s4", &sub.join("x.rs")), t.path(), &judge));
        assert!(out.contains("dir rule") && !out.contains("root rule"));
        assert_eq!(judge.calls(), 1);
        // The refused root file is not asked about again from another directory.
        let other = t.path().join("e");
        std::fs::create_dir(&other).unwrap();
        assert!(on_tool(&read("s4", &other.join("z.rs")), t.path(), &judge).is_none());
        assert_eq!(judge.calls(), 1);
    }

    #[test]
    fn a_failed_call_adds_nothing_and_the_cap_holds() {
        let t = repo();
        let sub = t.path().join("f");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(t.path().join("AGENTS.md"), "r").unwrap();
        std::fs::write(sub.join("AGENTS.md"), "d").unwrap();
        let down = Fake::new(Err("down".into()));
        assert!(on_tool(&read("s5", &sub.join("x.rs")), t.path(), &down).is_none());
        // A large single file is judged and then cut to the cap.
        let big = t.path().join("g");
        std::fs::create_dir(&big).unwrap();
        std::fs::remove_file(t.path().join("AGENTS.md")).unwrap();
        std::fs::write(big.join("AGENTS.md"), "x".repeat(10_000)).unwrap();
        let up = Fake::new(Ok(vec![("f0".into(), 0.8)]));
        let out = text(on_tool(&read("s5", &big.join("x.rs")), t.path(), &up));
        assert!(!out.is_empty() && out.len() <= BUDGET);
    }

    #[test]
    fn grep_with_a_directory_path_counts() {
        let t = repo();
        let sub = t.path().join("h");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("AGENTS.md"), "grep rule").unwrap();
        let judge = Fake::new(Err("no".into()));
        let input = json!({"session_id": "s6", "tool_name": "Grep", "tool_input": {"path": sub}});
        assert!(text(on_tool(&input, t.path(), &judge)).contains("grep rule"));
    }
}
