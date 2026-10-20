//! A failed briefed run's saved diff and failing check (#12209).
//!
//! Before a run removes its worktrees after a failure, the engine
//! (`scripts/work/work_issue.py`) writes `OUT/saved/`: `change.patch` (the
//! diff against its base, new files included), `failure.json` (each failing
//! check's command and last 200 lines of output, and why the run stopped),
//! and `result.json`. A pool run uploads that folder to
//! `gs://openagentsgemini-coder-artifacts/runs/RUN/` from its host; an
//! Archil sandbox can't reach the bucket, so the engine prints the folder as
//! one `{"type": "saved"}` line and this computer uploads it. The run's
//! summary line and its issue comment name the folder, and
//! `openagents chat work --resume-from RUN` starts a new attempt from it.
//!
//! `OA_WORK_SAVED_RUNS` names the store (`gs://bucket/prefix` or a folder,
//! as for the landing queue); a run on this computer uploads only when it is
//! set, and keeps the folder under `~/.openagents/work/runs/RUN/saved/`.

use std::path::{Path, PathBuf};

use coder::task::land_queue::{self, Store};
use serde_json::{Value, json};

/// The variable naming where saved runs go.
pub(super) const STORE_ENV: &str = "OA_WORK_SAVED_RUNS";
/// Where pool and Archil runs save, and where `--resume-from` looks.
pub(super) const DEFAULT_STORE: &str = "gs://openagentsgemini-coder-artifacts/runs";

/// The store `OA_WORK_SAVED_RUNS` names, else the shared bucket.
pub(super) fn store() -> Result<Box<dyn Store>, String> {
    let named = std::env::var(STORE_ENV).ok();
    land_queue::open(named.as_deref().unwrap_or(DEFAULT_STORE))
}

/// Whether this process uploads failed runs (`OA_WORK_SAVED_RUNS` is set:
/// a pool host's run script sets it).
pub(super) fn uploads() -> bool {
    std::env::var_os(STORE_ENV).is_some_and(|v| !v.is_empty())
}

fn plain(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 96
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && !word.starts_with('.')
}

/// Stores `files` as `RUN/NAME` and returns the folder's location.
pub(super) fn upload(
    store: &dyn Store,
    run: &str,
    files: &[(String, Vec<u8>)],
) -> Result<String, String> {
    if !plain(run) {
        return Err(format!("`{run}` isn't a run ID."));
    }
    for (name, bytes) in files {
        if !plain(name) {
            return Err(format!("`{name}` isn't a file name."));
        }
        store.write(&format!("{run}/{name}"), bytes)?;
    }
    Ok(format!("{}/{run}/", store.location().trim_end_matches('/')))
}

/// The files of a saved folder on this computer.
pub(super) fn files_in(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            std::fs::read(entry.path()).ok().map(|bytes| (name, bytes))
        })
        .collect();
    files.sort();
    files
}

/// The files of the engine's `{"type": "saved"}` line.
pub(super) fn files_of(line: &Value) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = line["files"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, text)| text.as_str().map(|t| (name.clone(), t.as_bytes().to_vec())))
        .collect();
    files.sort();
    files
}

/// Uploads a failed result's saved files and records where in
/// `result["saved"]["url"]`; an upload that fails is said in
/// `result["saved"]["error"]`, never as the run's failure.
pub(super) fn record_upload(
    result: &mut Value,
    store: Result<Box<dyn Store>, String>,
    files: &[(String, Vec<u8>)],
) {
    if result["ok"] == true || files.is_empty() {
        return;
    }
    let Some(run) = result["run_id"].as_str().map(str::to_owned) else {
        return;
    };
    if !result["saved"].is_object() {
        result["saved"] = json!({});
    }
    match store.and_then(|store| upload(store.as_ref(), &run, files)) {
        Ok(url) => result["saved"]["url"] = json!(url),
        Err(error) => result["saved"]["error"] = json!(error),
    }
}

/// On a host that uploads, a failed result's `saved.dir` goes to the store.
pub(super) fn upload_here(result: &mut Value) {
    if !uploads() || result["ok"] == true {
        return;
    }
    let Some(dir) = result["saved"]["dir"].as_str().map(PathBuf::from) else {
        return;
    };
    let files = files_in(&dir);
    record_upload(result, store(), &files);
}

/// The saved folder an `issue` event's result names (`url`, else `dir`).
pub(super) fn saved_of(record: &Value) -> Option<String> {
    let saved = &record["result"]["saved"];
    saved["url"]
        .as_str()
        .or_else(|| saved["dir"].as_str())
        .map(str::to_owned)
}

/// The summary's words for a saved run, if it was saved.
pub(super) fn summary(result: &Value) -> Option<String> {
    let run = result["run_id"].as_str()?;
    let saved = &result["saved"];
    let at = saved["url"].as_str().or_else(|| saved["dir"].as_str())?;
    Some(format!(
        "The diff and the failing check are saved at {at}; continue with `openagents chat work --resume-from {run}`"
    ))
}

/// The issue comment's line for a saved run.
pub(super) fn comment_line(at: &str, run: Option<&str>) -> String {
    match run {
        Some(run) => format!(
            "- the failed run's diff and failing check: `{at}` (continue with `openagents chat work --resume-from {run}`)\n"
        ),
        None => format!("- the failed run's diff and failing check: `{at}`\n"),
    }
}

/// Run `run`'s saved folder on this computer: the run's own folder when it
/// ran here, else a copy fetched from `store` under `home`. Returns the
/// folder and its `failure.json`.
pub(super) fn fetch(home: &Path, store: &dyn Store, run: &str) -> Result<(PathBuf, Value), String> {
    if !plain(run) {
        return Err(format!("`{run}` isn't a run ID."));
    }
    let work = home.join(".openagents").join("work");
    let local = work.join("runs").join(run).join("saved");
    let dir = if local.join("failure.json").is_file() {
        local
    } else {
        let dest = work.join("resume").join(run);
        let failure = store.read(&format!("{run}/failure.json"))?.ok_or_else(|| {
            format!(
                "No saved run `{run}` here or in {}.",
                store.location().trim_end_matches('/')
            )
        })?;
        std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        std::fs::write(dest.join("failure.json"), failure)
            .map_err(|e| format!("{}: {e}", dest.display()))?;
        for name in ["change.patch", "result.json"] {
            if let Some(bytes) = store.read(&format!("{run}/{name}"))? {
                std::fs::write(dest.join(name), bytes)
                    .map_err(|e| format!("{}: {e}", dest.display()))?;
            }
        }
        dest
    };
    let text = std::fs::read_to_string(dir.join("failure.json"))
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let failure: Value = serde_json::from_str(&text)
        .map_err(|e| format!("run `{run}`'s failure.json doesn't read: {e}"))?;
    if failure["issue"].as_u64().is_none() {
        return Err(format!("run `{run}`'s failure.json names no issue."));
    }
    Ok((dir, failure))
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder::task::land_queue::Dir;

    fn saved_folder(root: &Path) -> PathBuf {
        let dir = root.join("saved");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("change.patch"), "diff --git a/a b/a\n").unwrap();
        std::fs::write(
            dir.join("failure.json"),
            r#"{"run_id":"abc123","repo":"o/r","issue":7,"failing_checks":[{"command":"cargo test -p x","output":"boom"}]}"#,
        )
        .unwrap();
        std::fs::write(dir.join("result.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn a_failed_run_uploads_its_folder_as_run_slash_file() {
        let tmp = tempfile::tempdir().unwrap();
        let bucket = tmp.path().join("bucket");
        let dir = saved_folder(tmp.path());
        let mut result = json!({"ok": false, "run_id": "abc123",
            "saved": {"dir": dir.display().to_string()}});
        let store: Box<dyn Store> = Box::new(Dir(bucket.clone()));
        record_upload(&mut result, Ok(store), &files_in(&dir));
        assert_eq!(
            result["saved"]["url"].as_str().unwrap(),
            format!("{}/abc123/", bucket.display())
        );
        for name in ["change.patch", "failure.json", "result.json"] {
            assert_eq!(
                std::fs::read(bucket.join("abc123").join(name)).unwrap(),
                std::fs::read(dir.join(name)).unwrap(),
                "{name}"
            );
        }
        let line = summary(&result).unwrap();
        assert!(line.contains("--resume-from abc123"), "{line}");
    }

    #[test]
    fn a_passing_run_or_a_bad_name_uploads_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let bucket = tmp.path().join("bucket");
        let mut passed = json!({"ok": true, "run_id": "abc123"});
        let files = vec![("change.patch".to_owned(), b"x".to_vec())];
        record_upload(&mut passed, Ok(Box::new(Dir(bucket.clone()))), &files);
        assert!(!bucket.exists());
        assert!(upload(&Dir(bucket.clone()), "../up", &files).is_err());
        let bad = vec![("../x".to_owned(), b"x".to_vec())];
        assert!(upload(&Dir(bucket), "abc123", &bad).is_err());
    }

    #[test]
    fn the_saved_line_from_a_sandbox_uploads_the_same_files() {
        let line = json!({"type": "saved", "run_id": "r1",
            "files": {"failure.json": "{}", "change.patch": "diff"}});
        let files = files_of(&line);
        assert_eq!(
            files,
            vec![
                ("change.patch".to_owned(), b"diff".to_vec()),
                ("failure.json".to_owned(), b"{}".to_vec())
            ]
        );
    }

    #[test]
    fn resume_reads_the_local_folder_first_then_the_store() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let bucket = tmp.path().join("bucket");
        let dir = saved_folder(tmp.path());
        upload(&Dir(bucket.clone()), "abc123", &files_in(&dir)).unwrap();
        let (fetched, failure) = fetch(&home, &Dir(bucket.clone()), "abc123").unwrap();
        assert_eq!(fetched, home.join(".openagents/work/resume/abc123"));
        assert_eq!(failure["issue"], 7);
        assert_eq!(
            std::fs::read_to_string(fetched.join("change.patch")).unwrap(),
            "diff --git a/a b/a\n"
        );
        let local = home.join(".openagents/work/runs/local1/saved");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::copy(dir.join("failure.json"), local.join("failure.json")).unwrap();
        let (found, _) = fetch(&home, &Dir(bucket.clone()), "local1").unwrap();
        assert_eq!(found, local);
        assert!(fetch(&home, &Dir(bucket), "missing").is_err());
    }

    #[test]
    fn the_issue_event_names_the_saved_folder() {
        let record =
            json!({"event": "issue", "result": {"saved": {"dir": "/d", "url": "gs://b/runs/r/"}}});
        assert_eq!(saved_of(&record).as_deref(), Some("gs://b/runs/r/"));
        assert_eq!(saved_of(&json!({"event": "issue"})), None);
    }
}
