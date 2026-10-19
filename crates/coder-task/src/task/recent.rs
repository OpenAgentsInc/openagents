//! What Coder runs did lately, for the background rules
//! (docs/background): the daily usage summary (what ended, what it cost;
//! information, never a limit) and the flake watch (which checks failed).
//! A task's time is its file's last change in the store.

use std::path::Path;
use std::time::UNIX_EPOCH;

use background::services::{Failure, Usage};

use super::{Execution, Status, Store, TASK_DIR, Task};

fn changed(store: &Path, id: &str) -> u64 {
    std::fs::metadata(store.join(TASK_DIR).join(format!("{id}.json")))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs())
}

fn recent(store: &Path, since: u64) -> Result<Vec<(Task, u64)>, String> {
    if !super::present(store) {
        return Ok(Vec::new());
    }
    let tasks = Store::open(store)
        .and_then(|store| store.list())
        .map_err(|error| error.to_string())?;
    Ok(tasks
        .into_iter()
        .map(|task| {
            let at = changed(store, &task.task_id);
            (task, at)
        })
        .filter(|(_, at)| *at >= since)
        .collect())
}

/// The directories the store's newest tasks ran in, newest first, each
/// once, at most `max`. A directory that no longer exists is skipped.
#[must_use]
pub fn checkouts(store: &Path, max: usize) -> Vec<std::path::PathBuf> {
    let mut tasks = recent(store, 0).unwrap_or_default();
    tasks.sort_by(|a, b| b.1.cmp(&a.1));
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for (task, _) in tasks {
        let path = std::path::PathBuf::from(&task.intent.workspace.path);
        if path.as_os_str().is_empty() || !path.is_dir() || paths.contains(&path) {
            continue;
        }
        paths.push(path);
        if paths.len() == max {
            break;
        }
    }
    paths
}

/// The Coder runs that ended since `since`, and what the priced ones cost.
///
/// # Errors
/// The store cannot be read.
pub fn usage(store: &Path, since: u64) -> Result<Usage, String> {
    let mut usage = Usage::default();
    let mut cost: Option<u64> = None;
    for (task, _) in recent(store, since)? {
        if !matches!(task.status, Status::Finished | Status::Cancelled) {
            continue;
        }
        usage.ended += 1;
        if task.execution == Execution::Finished && task.checks != super::Checks::Failed {
            usage.succeeded += 1;
        } else {
            usage.failed += 1;
        }
        match task
            .run
            .as_ref()
            .and_then(|run| run.result.as_ref())
            .and_then(|result| result.cost_microusd)
        {
            Some(micro) => cost = Some(cost.unwrap_or(0) + micro),
            None => usage.unpriced += 1,
        }
    }
    usage.cost_microusd = cost;
    Ok(usage)
}

/// The checks that failed in Coder runs since `since`: each failed check
/// of each run's check report, with its reason.
///
/// # Errors
/// The store cannot be read.
pub fn failures(store: &Path, since: u64) -> Result<Vec<Failure>, String> {
    let mut found = Vec::new();
    for (task, at) in recent(store, since)? {
        for run in task.earlier.iter().chain(task.run.as_ref()) {
            let Some(report) = &run.check_report else {
                continue;
            };
            let Some(checks) = report
                .evidence
                .as_ref()
                .and_then(|evidence| evidence["checks"].as_array())
            else {
                continue;
            };
            for check in checks {
                let failed = check["verdict"]
                    .as_str()
                    .is_some_and(|verdict| verdict.to_ascii_lowercase().starts_with("fail"));
                if failed {
                    found.push(Failure {
                        test: check["id"].as_str().unwrap_or("check").to_owned(),
                        output: check["reason"].as_str().unwrap_or_default().to_owned(),
                        task: task.task_id.clone(),
                        at,
                    });
                }
            }
        }
    }
    found.sort_by_key(|failure| failure.at);
    Ok(found)
}
