//! Task worktrees as a person sees them (#10296, #10294): how many each
//! project has and how much room they take, and archiving an ended task's
//! worktree. Archiving removes the checkout only when nothing in it is
//! unsaved ([`background::git::removable`]: no uncommitted or unpushed
//! work, no ignored file outside a build cache, no stash made on it) and
//! keeps what recreates it, so [`restore`] puts it back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use background::git::{self, Undo};

use super::Store;
use super::retire::Retired;

/// One task's worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub task: String,
    pub path: PathBuf,
    pub bytes: u64,
    /// The task is over for good ([`super::Task::ended`]), so archiving
    /// its worktree cannot take it from a running turn.
    pub ended: bool,
}

/// One project's task worktrees, largest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The project's folder, as the task names it.
    pub project: String,
    pub bytes: u64,
    pub worktrees: Vec<Worktree>,
}

/// The task worktrees on this computer that still exist, by project.
///
/// # Errors
/// The task store cannot be read.
pub fn list(store: &Path) -> Result<Vec<Project>, String> {
    if !super::present(store) {
        return Ok(Vec::new());
    }
    let tasks = Store::open_waiting(store, std::time::Duration::from_secs(30))
        .and_then(|store| store.list())
        .map_err(|error| error.to_string())?;
    let found: Vec<(String, Worktree)> = tasks
        .iter()
        .filter_map(|task| {
            let record = super::local::record(store, &task.task_id)?;
            let path = PathBuf::from(&record.worktree);
            path.is_dir().then(|| {
                (
                    task.intent.workspace.path.clone(),
                    Worktree {
                        task: task.task_id.clone(),
                        bytes: 0,
                        path,
                        ended: task.ended(),
                    },
                )
            })
        })
        .collect();
    // Sizing walks every file: several worktrees at a time (#10304).
    let sizes = background::pool::map(&found, |(_, worktree)| size(&worktree.path));
    let mut projects: BTreeMap<String, Vec<Worktree>> = BTreeMap::new();
    for ((project, mut worktree), bytes) in found.into_iter().zip(sizes) {
        worktree.bytes = bytes;
        projects.entry(project).or_default().push(worktree);
    }
    let mut out: Vec<Project> = projects
        .into_iter()
        .map(|(project, mut worktrees)| {
            worktrees.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.task.cmp(&b.task)));
            Project {
                project,
                bytes: worktrees.iter().map(|w| w.bytes).sum(),
                worktrees,
            }
        })
        .collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.project.cmp(&b.project)));
    Ok(out)
}

/// Archive the worktree of the ended task `task`: remove it when nothing in
/// it is unsaved, and keep what recreates it for [`restore`]. The words to
/// show.
///
/// # Errors
/// The task is not over, has no worktree here, or holds something unsaved,
/// in words for the person.
pub fn archive(store: &Path, task: &str) -> Result<String, String> {
    let found = Store::open_waiting(store, std::time::Duration::from_secs(30))
        .and_then(|store| store.list())
        .map_err(|error| error.to_string())?
        .into_iter()
        .any(|found| found.task_id == task);
    if !found {
        return Err("There is no such task here.".into());
    }
    if super::local::record(store, task).is_none() {
        return Err("This task has no worktree on this computer.".into());
    }
    // The same removal a task's end makes (#10291): the repository's
    // teardown, then the worktree, with what recreates it kept in the
    // run's record, so a follow-up brings it back by itself.
    match super::retire::retire(store, task) {
        Retired::Removed => Ok(format!(
            "Archived the worktree of task {}; what brings it back is kept.",
            &task[..task.len().min(8)]
        )),
        Retired::NotEnded => Err("This task is still going, so its worktree stays.".into()),
        Retired::Absent => Err("This task's worktree is already gone.".into()),
        Retired::Kept(why) => Err(format!("Kept the worktree: {why}.")),
    }
}

/// Recreate the worktree [`archive`] (or the task's end) removed for
/// `task`. The words to show.
///
/// # Errors
/// It was not archived here, or Git refused.
pub fn restore(store: &Path, task: &str) -> Result<String, String> {
    // An archive made before the run's record kept it.
    let path = undo_path(store, task);
    if let Ok(bytes) = std::fs::read(&path) {
        let undo: Undo = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        git::restore(&undo)?;
        let _ = std::fs::remove_file(&path);
        return Ok(format!("Restored the worktree at {}.", undo.path.display()));
    }
    let archived =
        super::local::record(store, task).is_some_and(|record| record.archived.is_some());
    if !archived {
        return Err("This task's worktree was not archived here.".into());
    }
    let record = super::retire::ensure(store, task)?
        .ok_or_else(|| "This task's worktree was not archived here.".to_owned())?;
    Ok(format!("Restored the worktree at {}.", record.worktree))
}

/// Throw away what [`archive`] would keep the worktree for: its
/// uncommitted changes and every untracked or ignored file, which are
/// deleted for good. Commits not on a remote, and a stash made on it,
/// still keep it. Only the person's typed confirmation should lead here
/// (`openagents worktree archive --force`, #10294); [`archive`] follows.
///
/// # Errors
/// The task is still going or has no worktree here, or Git refused.
pub fn discard_unsaved(store: &Path, task: &str) -> Result<(), String> {
    let ended = Store::open_waiting(store, std::time::Duration::from_secs(30))
        .and_then(|store| store.list())
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|found| found.task_id == task)
        .map(|found| found.ended())
        .ok_or_else(|| "There is no such task here.".to_owned())?;
    if !ended {
        return Err("This task is still going, so its worktree stays.".into());
    }
    let record = super::local::record(store, task)
        .ok_or_else(|| "This task has no worktree on this computer.".to_owned())?;
    let path = PathBuf::from(&record.worktree);
    if !path.join(".git").is_file() {
        return Err("This task's worktree is already gone.".into());
    }
    for args in [&["reset", "--hard", "--quiet"][..], &["clean", "-fdxq"][..]] {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| format!("git: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Git kept the worktree: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

/// A task worktree [`archive`] or the task's end removed, which
/// [`restore`] recreates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archived {
    pub task: String,
    pub path: PathBuf,
    pub commit: String,
    /// When it was removed, in Unix seconds (0 when not recorded).
    pub at: u64,
}

/// Every removed task worktree [`restore`] can bring back, newest first:
/// the runs' records (#10291) and archives made before them.
#[must_use]
pub fn archived(store: &Path) -> Vec<Archived> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(store.join("local")) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(task) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
                continue;
            };
            let Some(record) = super::local::record(store, task) else {
                continue;
            };
            if let Some(gone) = &record.archived {
                out.push(Archived {
                    task: record.task.clone(),
                    path: PathBuf::from(&record.worktree),
                    commit: gone.commit.clone(),
                    at: gone.removed_at,
                });
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(store.join("archived-worktrees")) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(task) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
                continue;
            };
            let Some(undo) = std::fs::read(entry.path())
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Undo>(&bytes).ok())
            else {
                continue;
            };
            let at = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_secs());
            out.push(Archived {
                task: task.to_owned(),
                path: undo.path,
                commit: undo.commit,
                at,
            });
        }
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.task.cmp(&b.task)));
    out
}

fn undo_path(store: &Path, task: &str) -> PathBuf {
    store
        .join("archived-worktrees")
        .join(format!("{task}.json"))
}

/// The bytes the files under `dir` take, not following links.
fn size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.file_type() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total += entry.metadata().map_or(0, |meta| meta.len());
            }
        }
    }
    total
}

/// Bytes in the plain words the screens use: "1.2 GB", "340 MB".
#[must_use]
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    value /= 1000.0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_plain_units() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1_234_000), "1.2 MB");
        assert_eq!(human(340_000_000), "340 MB");
        assert_eq!(human(12_500_000_000), "12 GB");
    }

    #[test]
    fn size_counts_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/f"), vec![0u8; 300]).unwrap();
        std::fs::write(dir.path().join("g"), vec![0u8; 200]).unwrap();
        assert_eq!(size(dir.path()), 500);
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?}: {status:?}");
    }

    fn command(store: &Path, value: serde_json::Value) {
        let mut inbox = Store::open(store).unwrap();
        inbox.apply(&serde_json::to_vec(&value).unwrap()).unwrap();
    }

    /// A task's worktree is listed under its project, kept while the task
    /// runs, archived once it ended, and restored at its commit.
    #[test]
    fn an_ended_tasks_worktree_archives_and_restores() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let origin = root.join("origin.git");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q", "--bare", "-b", "main"]);
        let checkout = root.join("app");
        git(root, &["clone", "-q", origin.to_str().unwrap(), "app"]);
        std::fs::write(checkout.join("README"), "hi").unwrap();
        git(&checkout, &["add", "README"]);
        git(&checkout, &["commit", "-q", "-m", "one"]);
        git(&checkout, &["push", "-q", "origin", "HEAD:main"]);
        let worktree = root.join("worktrees/t1");
        git(
            &checkout,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                worktree.to_str().unwrap(),
            ],
        );

        let store = root.join("tasks");
        command(
            &store,
            serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"submit", "task_id":"t1", "expected_revision":null, "action":{"type":"submit", "intent":{"title":"test", "prompt":"test", "workspace":{"path":checkout,"source_revision":null}, "configuration":{"adapter":"test", "model":null}}}}),
        );
        std::fs::create_dir_all(store.join("local")).unwrap();
        std::fs::write(
            store.join("local/t1.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema": super::super::local::RECORD_SCHEMA,
                "task": "t1",
                "project": checkout,
                "checkout": checkout,
                "worktree": worktree,
                "base": "main",
                "turns": [],
            }))
            .unwrap(),
        )
        .unwrap();

        let projects = list(&store).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project, checkout.to_string_lossy());
        assert_eq!(projects[0].worktrees[0].task, "t1");
        assert!(!projects[0].worktrees[0].ended);
        assert_eq!(
            archive(&store, "t1").unwrap_err(),
            "This task is still going, so its worktree stays."
        );

        command(
            &store,
            serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"cancel", "task_id":"t1", "expected_revision":1, "action":{"type":"cancel", "reason":"done"}}),
        );
        // Unsaved work keeps it.
        std::fs::write(worktree.join("notes"), "mine").unwrap();
        assert!(
            archive(&store, "t1")
                .unwrap_err()
                .starts_with("Kept the worktree")
        );
        std::fs::remove_file(worktree.join("notes")).unwrap();

        assert!(
            archive(&store, "t1")
                .unwrap()
                .starts_with("Archived the worktree")
        );
        assert!(!worktree.exists());
        assert!(list(&store).unwrap().is_empty());
        let gone = archived(&store);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].task, "t1");
        assert_eq!(gone[0].path, worktree);
        assert!(
            restore(&store, "t1")
                .unwrap()
                .starts_with("Restored the worktree")
        );
        assert!(worktree.join("README").exists());
        assert!(archived(&store).is_empty());

        // With the person's confirmation, unsaved files go for good and
        // the worktree archives; the committed state comes back.
        std::fs::write(worktree.join("notes"), "mine").unwrap();
        std::fs::write(worktree.join("README"), "changed").unwrap();
        discard_unsaved(&store, "t1").unwrap();
        assert!(!worktree.join("notes").exists());
        archive(&store, "t1").unwrap();
        restore(&store, "t1").unwrap();
        assert_eq!(
            std::fs::read_to_string(worktree.join("README")).unwrap(),
            "hi"
        );
    }

    #[test]
    fn an_empty_store_lists_nothing_and_restore_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        assert_eq!(list(&store).unwrap(), Vec::new());
        assert_eq!(
            restore(&store, "t1").unwrap_err(),
            "This task's worktree was not archived here."
        );
    }
}
