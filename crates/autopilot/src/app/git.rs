use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::app::workspaces::WorkspaceInfo;
use git2::{DiffOptions, Repository, Status, StatusOptions, Tree};
use tokio::sync::mpsc;
use web_time::Instant;

const GIT_REFRESH_INTERVAL_SECS: u64 = 3;

#[derive(Clone, Debug, Default)]
pub(crate) struct GitFileStatus {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) additions: i64,
    pub(crate) deletions: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct GitStatusSnapshot {
    pub(crate) branch_name: String,
    pub(crate) files: Vec<GitFileStatus>,
    pub(crate) total_additions: i64,
    pub(crate) total_deletions: i64,
    pub(crate) error: Option<String>,
}

impl GitStatusSnapshot {
    pub(crate) fn empty() -> Self {
        Self {
            branch_name: "unknown".to_string(),
            files: Vec::new(),
            total_additions: 0,
            total_deletions: 0,
            error: None,
        }
    }

    fn with_error(message: String) -> Self {
        let mut snapshot = Self::empty();
        snapshot.error = Some(message);
        snapshot
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GitDiffItem {
    pub(crate) diff: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GitDiffSnapshot {
    pub(crate) diffs: HashMap<String, GitDiffItem>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct GitFileDiff {
    pub(crate) path: String,
    pub(crate) diff: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CenterMode {
    Chat,
    Diff,
}

#[derive(Debug)]
pub(crate) enum GitEvent {
    StatusUpdated {
        workspace_id: String,
        status: GitStatusSnapshot,
    },
    DiffsUpdated {
        workspace_id: String,
        diffs: Vec<GitFileDiff>,
        error: Option<String>,
    },
}

#[derive(Debug)]
pub(crate) enum GitCommand {
    RefreshStatus { workspace_id: String, path: PathBuf },
    RefreshDiffs { workspace_id: String, path: PathBuf },
}

pub(crate) struct GitRuntime {
    cmd_tx: mpsc::Sender<GitCommand>,
    pub(crate) event_rx: mpsc::Receiver<GitEvent>,
}

impl GitRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<GitCommand>(16);
        let (event_tx, event_rx) = mpsc::channel::<GitEvent>(32);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_git_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn refresh_status(&self, workspace_id: String, path: PathBuf) {
        let _ = self
            .cmd_tx
            .try_send(GitCommand::RefreshStatus { workspace_id, path });
    }

    pub(crate) fn refresh_diffs(&self, workspace_id: String, path: PathBuf) {
        let _ = self
            .cmd_tx
            .try_send(GitCommand::RefreshDiffs { workspace_id, path });
    }
}

impl Default for GitRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct GitState {
    pub(crate) runtime: GitRuntime,
    pub(crate) center_mode: CenterMode,
    pub(crate) selected_diff_path: Option<String>,
    pub(crate) diff_scroll_offset: f32,
    pub(crate) pending_scroll_to: Option<String>,
    pub(crate) back_button_hovered: bool,
    status_by_workspace: HashMap<String, GitStatusSnapshot>,
    diff_by_workspace: HashMap<String, GitDiffSnapshot>,
    status_in_flight: HashSet<String>,
    diff_in_flight: HashSet<String>,
    status_signature: HashMap<String, String>,
    diff_stale: HashSet<String>,
    status_refresh_at: HashMap<String, Instant>,
    diff_refresh_at: HashMap<String, Instant>,
}

impl GitState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: GitRuntime::new(),
            center_mode: CenterMode::Chat,
            selected_diff_path: None,
            diff_scroll_offset: 0.0,
            pending_scroll_to: None,
            back_button_hovered: false,
            status_by_workspace: HashMap::new(),
            diff_by_workspace: HashMap::new(),
            status_in_flight: HashSet::new(),
            diff_in_flight: HashSet::new(),
            status_signature: HashMap::new(),
            diff_stale: HashSet::new(),
            status_refresh_at: HashMap::new(),
            diff_refresh_at: HashMap::new(),
        }
    }

    pub(crate) fn set_active_workspace(&mut self, workspace_id: Option<&str>) {
        self.center_mode = CenterMode::Chat;
        self.selected_diff_path = None;
        self.pending_scroll_to = None;
        self.diff_scroll_offset = 0.0;
        self.back_button_hovered = false;
        if let Some(id) = workspace_id {
            self.status_refresh_at.remove(id);
            self.diff_refresh_at.remove(id);
            self.diff_stale.insert(id.to_string());
        }
    }

    pub(crate) fn select_diff_path(&mut self, path: String) {
        if self.selected_diff_path.as_deref() != Some(path.as_str()) {
            self.pending_scroll_to = Some(path.clone());
        }
        self.selected_diff_path = Some(path);
        self.center_mode = CenterMode::Diff;
    }

    pub(crate) fn exit_diff_view(&mut self) {
        self.center_mode = CenterMode::Chat;
        self.selected_diff_path = None;
        self.pending_scroll_to = None;
        self.diff_scroll_offset = 0.0;
        self.back_button_hovered = false;
    }

    pub(crate) fn force_refresh(&mut self, workspace_id: &str) {
        self.status_refresh_at.remove(workspace_id);
        self.diff_refresh_at.remove(workspace_id);
        self.diff_stale.insert(workspace_id.to_string());
    }

    pub(crate) fn status_for_workspace(&self, workspace_id: &str) -> Option<&GitStatusSnapshot> {
        self.status_by_workspace.get(workspace_id)
    }

    pub(crate) fn diff_snapshot_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Option<&GitDiffSnapshot> {
        self.diff_by_workspace.get(workspace_id)
    }

    pub(crate) fn is_diff_loading(&self, workspace_id: &str) -> bool {
        self.diff_in_flight.contains(workspace_id)
    }

    pub(crate) fn apply_status_update(&mut self, workspace_id: String, status: GitStatusSnapshot) {
        self.status_in_flight.remove(&workspace_id);
        let signature = signature_for_status(&status);
        let signature_changed = self
            .status_signature
            .get(&workspace_id)
            .map(|prev| prev != &signature)
            .unwrap_or(true);
        if signature_changed {
            self.status_signature
                .insert(workspace_id.clone(), signature);
            self.diff_stale.insert(workspace_id.clone());
        }
        if status.error.is_some() {
            self.diff_by_workspace.remove(&workspace_id);
        }
        self.status_by_workspace.insert(workspace_id, status);
    }

    pub(crate) fn apply_diff_update(
        &mut self,
        workspace_id: String,
        diffs: Vec<GitFileDiff>,
        error: Option<String>,
    ) {
        self.diff_in_flight.remove(&workspace_id);
        self.diff_stale.remove(&workspace_id);

        if let Some(message) = error {
            self.diff_by_workspace.insert(
                workspace_id,
                GitDiffSnapshot {
                    diffs: HashMap::new(),
                    error: Some(message),
                },
            );
            return;
        }

        let mut map = HashMap::new();
        for diff in diffs {
            map.insert(diff.path, GitDiffItem { diff: diff.diff });
        }
        self.diff_by_workspace.insert(
            workspace_id,
            GitDiffSnapshot {
                diffs: map,
                error: None,
            },
        );
    }

    pub(crate) fn refresh_if_needed(&mut self, workspace: Option<&WorkspaceInfo>) {
        let Some(workspace) = workspace else {
            return;
        };
        let now = Instant::now();
        let status_due = self
            .status_refresh_at
            .get(&workspace.id)
            .map(|last| now.duration_since(*last).as_secs() >= GIT_REFRESH_INTERVAL_SECS)
            .unwrap_or(true);
        if status_due && !self.status_in_flight.contains(&workspace.id) {
            self.status_in_flight.insert(workspace.id.clone());
            self.status_refresh_at.insert(workspace.id.clone(), now);
            self.runtime
                .refresh_status(workspace.id.clone(), PathBuf::from(&workspace.path));
        }

        if self.center_mode == CenterMode::Diff {
            let diff_due = self
                .diff_refresh_at
                .get(&workspace.id)
                .map(|last| now.duration_since(*last).as_secs() >= GIT_REFRESH_INTERVAL_SECS)
                .unwrap_or(true);
            let diff_stale = self.diff_stale.contains(&workspace.id);
            if (diff_due || diff_stale) && !self.diff_in_flight.contains(&workspace.id) {
                self.diff_in_flight.insert(workspace.id.clone());
                self.diff_refresh_at.insert(workspace.id.clone(), now);
                self.runtime
                    .refresh_diffs(workspace.id.clone(), PathBuf::from(&workspace.path));
            }
        }
    }
}

impl Default for GitState {
    fn default() -> Self {
        Self::new()
    }
}

async fn run_git_loop(mut cmd_rx: mpsc::Receiver<GitCommand>, event_tx: mpsc::Sender<GitEvent>) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            GitCommand::RefreshStatus { workspace_id, path } => {
                let status = match read_git_status(&path) {
                    Ok(snapshot) => snapshot,
                    Err(err) => GitStatusSnapshot::with_error(err),
                };
                let _ = event_tx
                    .send(GitEvent::StatusUpdated {
                        workspace_id,
                        status,
                    })
                    .await;
            }
            GitCommand::RefreshDiffs { workspace_id, path } => match read_git_diffs(&path) {
                Ok(diffs) => {
                    let _ = event_tx
                        .send(GitEvent::DiffsUpdated {
                            workspace_id,
                            diffs,
                            error: None,
                        })
                        .await;
                }
                Err(err) => {
                    let _ = event_tx
                        .send(GitEvent::DiffsUpdated {
                            workspace_id,
                            diffs: Vec::new(),
                            error: Some(err),
                        })
                        .await;
                }
            },
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn signature_for_status(status: &GitStatusSnapshot) -> String {
    let mut entries: Vec<String> = status
        .files
        .iter()
        .map(|file| {
            format!(
                "{}:{}:{}:{}",
                file.path, file.status, file.additions, file.deletions
            )
        })
        .collect();
    entries.sort();
    entries.join("|")
}

fn normalize_git_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn diff_stats_for_path(
    repo: &Repository,
    head_tree: Option<&Tree>,
    path: &str,
    include_index: bool,
    include_workdir: bool,
) -> Result<(i64, i64), git2::Error> {
    let mut additions = 0i64;
    let mut deletions = 0i64;

    if include_index {
        let mut options = DiffOptions::new();
        options.pathspec(path).include_untracked(true);
        let diff = repo.diff_tree_to_index(head_tree, None, Some(&mut options))?;
        let stats = diff.stats()?;
        additions += stats.insertions() as i64;
        deletions += stats.deletions() as i64;
    }

    if include_workdir {
        let mut options = DiffOptions::new();
        options
            .pathspec(path)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);
        let diff = repo.diff_index_to_workdir(None, Some(&mut options))?;
        let stats = diff.stats()?;
        additions += stats.insertions() as i64;
        deletions += stats.deletions() as i64;
    }

    Ok((additions, deletions))
}

fn diff_patch_to_string(patch: &mut git2::Patch) -> Result<String, git2::Error> {
    let buf = patch.to_buf()?;
    Ok(buf
        .as_str()
        .map(|value| value.to_string())
        .unwrap_or_else(|| String::from_utf8_lossy(&buf).to_string()))
}

fn read_git_status(path: &Path) -> Result<GitStatusSnapshot, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;

    let branch_name = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let mut status_options = StatusOptions::new();
    status_options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true)
        .include_ignored(false);

    let statuses = repo
        .statuses(Some(&mut status_options))
        .map_err(|e| e.to_string())?;

    let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());

    let mut files = Vec::new();
    let mut total_additions = 0i64;
    let mut total_deletions = 0i64;
    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        let status = entry.status();
        let status_str = if status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW) {
            "A"
        } else if status.contains(Status::WT_MODIFIED) || status.contains(Status::INDEX_MODIFIED) {
            "M"
        } else if status.contains(Status::WT_DELETED) || status.contains(Status::INDEX_DELETED) {
            "D"
        } else if status.contains(Status::WT_RENAMED) || status.contains(Status::INDEX_RENAMED) {
            "R"
        } else if status.contains(Status::WT_TYPECHANGE)
            || status.contains(Status::INDEX_TYPECHANGE)
        {
            "T"
        } else {
            "--"
        };

        let include_index = status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        );
        let include_workdir = status.intersects(
            Status::WT_NEW
                | Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE,
        );

        let (additions, deletions) = diff_stats_for_path(
            &repo,
            head_tree.as_ref(),
            path,
            include_index,
            include_workdir,
        )
        .map_err(|e| e.to_string())?;
        total_additions += additions;
        total_deletions += deletions;

        files.push(GitFileStatus {
            path: normalize_git_path(path),
            status: status_str.to_string(),
            additions,
            deletions,
        });
    }

    Ok(GitStatusSnapshot {
        branch_name,
        files,
        total_additions,
        total_deletions,
        error: None,
    })
}

fn read_git_diffs(path: &Path) -> Result<Vec<GitFileDiff>, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());

    let mut options = DiffOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true);

    let diff = match head_tree.as_ref() {
        Some(tree) => repo
            .diff_tree_to_workdir_with_index(Some(tree), Some(&mut options))
            .map_err(|e| e.to_string())?,
        None => repo
            .diff_tree_to_workdir_with_index(None, Some(&mut options))
            .map_err(|e| e.to_string())?,
    };

    let mut results = Vec::new();
    for (index, delta) in diff.deltas().enumerate() {
        let path = delta.new_file().path().or_else(|| delta.old_file().path());
        let Some(path) = path else {
            continue;
        };
        let patch = match git2::Patch::from_diff(&diff, index) {
            Ok(patch) => patch,
            Err(_) => continue,
        };
        let Some(mut patch) = patch else {
            continue;
        };
        let content = match diff_patch_to_string(&mut patch) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }
        results.push(GitFileDiff {
            path: normalize_git_path(path.to_string_lossy().as_ref()),
            diff: content,
        });
    }

    Ok(results)
}
