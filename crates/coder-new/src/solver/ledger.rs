//! The landed-work ledger: what every finished solver run under a state
//! directory touched, whether it landed on `main`, and which other
//! unlanded runs touch the same files. Read from each run's `summary.json`
//! and `change.patch`, so a later queue pass reads fact, not inference.

use std::path::Path;

use serde_json::{Value, json};

use super::hands::landed_commit;
use crate::issue_run::setup::diff_files;

/// One finished run's row in the ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub run_id: String,
    pub issue: u64,
    pub status: String,
    pub pull_request: Option<String>,
    pub files: Vec<String>,
    /// The `main` commit that landed this issue, when one has.
    pub merged: Option<String>,
    /// Run ids of other unlanded runs that touch one of the same files.
    pub conflicts_with: Vec<String>,
}

impl Row {
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "run_id": self.run_id,
            "issue": self.issue,
            "status": self.status,
            "pull_request": self.pull_request,
            "files": self.files,
            "merged": self.merged,
            "conflicts_with": self.conflicts_with,
        })
    }
}

/// One run's row before `main` is consulted: the facts its folder holds.
fn row_of(folder: &Path) -> Option<Row> {
    let summary: Value =
        serde_json::from_slice(&std::fs::read(folder.join("summary.json")).ok()?).ok()?;
    let mut files: Vec<String> = summary["changed"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        if let Ok(patch) = std::fs::read_to_string(folder.join("change.patch")) {
            files = diff_files(&patch);
        }
    }
    Some(Row {
        run_id: summary["run_id"].as_str()?.to_owned(),
        issue: summary["issue"].as_u64()?,
        status: summary["outcome"]["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
        pull_request: summary["solver"]["pull_request"]
            .as_str()
            .map(str::to_owned),
        files,
        merged: None,
        conflicts_with: Vec::new(),
    })
}

/// Marks which unlanded rows touch the same files, oldest first by run id.
pub fn cross(rows: &mut [Row]) {
    rows.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    let open: Vec<(String, Vec<String>)> = rows
        .iter()
        .filter(|row| row.merged.is_none())
        .map(|row| (row.run_id.clone(), row.files.clone()))
        .collect();
    for row in rows.iter_mut() {
        if row.merged.is_some() {
            continue;
        }
        row.conflicts_with = open
            .iter()
            .filter(|(id, files)| {
                *id != row.run_id && files.iter().any(|file| row.files.contains(file))
            })
            .map(|(id, _)| id.clone())
            .collect();
    }
}

/// Every finished run under `state`, with `main` in `repo` consulted for
/// what landed.
pub async fn read(state: &Path, repo: &Path) -> Vec<Row> {
    let mut rows: Vec<Row> = std::fs::read_dir(state)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| row_of(&entry.path()))
                .collect()
        })
        .unwrap_or_default();
    for row in &mut rows {
        row.merged = landed_commit(repo, row.issue).await;
    }
    cross(&mut rows);
    rows
}

/// `coder solver ledger [--state DIR] [--repo PATH]`: prints one JSON line
/// per finished run.
///
/// # Errors
/// An option is unknown or lacks its value.
pub async fn command(args: &[String]) -> Result<String, String> {
    let mut state = super::default_state();
    let mut repo = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => state = args.next().ok_or("--state needs a directory")?.into(),
            "--repo" => repo = args.next().ok_or("--repo needs a path")?.into(),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver ledger takes --state DIR and --repo PATH"
                ));
            }
        }
    }
    let mut out = String::new();
    for row in read(&state, &repo).await {
        out.push_str(&row.json().to_string());
        out.push('\n');
    }
    Ok(out)
}

/// Where a run stands among the unlanded runs that touch one of its
/// files: `{"position": K, "ahead": [run ids]}`, position 1 when no
/// earlier unlanded run shares a file. `changed` is the run's own
/// `summary.json["changed"]`; the run itself is left out by id.
pub async fn queue_position(state: &Path, repo: &Path, run_id: &str, changed: &Value) -> Value {
    let files: Vec<&str> = changed
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let ahead: Vec<String> = read(state, repo)
        .await
        .into_iter()
        .filter(|row| {
            row.merged.is_none()
                && row.run_id.as_str() < run_id
                && row.files.iter().any(|file| files.contains(&file.as_str()))
        })
        .map(|row| row.run_id)
        .collect();
    json!({"position": ahead.len() + 1, "ahead": ahead})
}
