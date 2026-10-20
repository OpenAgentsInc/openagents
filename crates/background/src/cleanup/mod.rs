//! Reviewable cleanup of old build outputs and saved, merged worktrees.

pub mod command;
pub mod inventory;

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::inuse::{Processes, Snapshot, System};
use crate::paths::Layout;
use crate::plan::Facts;
use crate::records::Undo;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub builds: bool,
    pub worktrees: bool,
    pub minimum_age_secs: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            builds: true,
            worktrees: true,
            minimum_age_secs: 86_400,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    BuildOutputs,
    Worktree,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub kind: Kind,
    pub path: PathBuf,
    pub target: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub fingerprint: String,
    pub undo: Option<Undo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Kept {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub home: PathBuf,
    pub created: u64,
    pub options: Options,
    pub entries: Vec<Entry>,
    pub kept: Vec<Kept>,
    pub uncovered: Vec<PathBuf>,
    pub digest: String,
}

impl Plan {
    fn seal(&mut self) -> Result<(), String> {
        self.digest.clear();
        self.digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).map_err(|e| e.to_string())?)
        );
        Ok(())
    }
    pub fn verify(&self, home: &Path) -> Result<(), String> {
        if self.schema != "openagents.cleanup.plan.v1"
            || self.home != home
            || self.options.minimum_age_secs < 86_400
            || self.created > crate::paths::now()
        {
            return Err("The cleanup plan has an unsupported scope, age, or home folder.".into());
        }
        let mut copy = self.clone();
        copy.seal()?;
        if copy.digest != self.digest {
            return Err("The cleanup plan changed. Scan again.".into());
        }
        Ok(())
    }
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.bytes).sum()
    }
}

#[derive(Debug, Serialize)]
pub struct Applied {
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub reason: String,
    pub undo: Option<Undo>,
}

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub plan_digest: String,
    pub actions: Vec<Applied>,
}

#[derive(Clone)]
struct Output {
    path: PathBuf,
    dev: u64,
    ino: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
    len: u64,
    bytes: u64,
}

fn protected(name: &str) -> bool {
    name.ends_with(".lock")
        || (name.starts_with(".cargo-") && name.ends_with("lock"))
        || name.starts_with(".openagents-")
        || matches!(
            name,
            "CACHEDIR.TAG" | ".rustc_info.json" | ".future-incompat-report.json"
        )
}

fn output_file(profile: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(profile) else {
        return false;
    };
    let Some(first) = relative.components().next() else {
        return false;
    };
    if [
        "deps",
        "build",
        "incremental",
        "incremental.kache-auto",
        ".fingerprint",
        "examples",
    ]
    .iter()
    .any(|part| first.as_os_str() == *part)
    {
        return true;
    }
    if relative.components().count() != 1 {
        return false;
    }
    if path.extension().is_some_and(|ext| {
        [
            "d", "rlib", "rmeta", "so", "a", "o", "dylib", "dll", "exe", "pdb", "wasm",
        ]
        .iter()
        .any(|known| ext == *known)
    }) {
        return true;
    }
    path.with_extension("d").is_file()
}

fn outputs(profile: &Path, cutoff: u64) -> Result<Vec<Output>, String> {
    let mut pending = vec![profile.to_owned()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_name() != ".git" {
                    pending.push(entry.path());
                }
            }
        } else if (meta.is_file() || meta.file_type().is_symlink())
            && output_file(profile, &path)
            && !protected(&path.file_name().unwrap_or_default().to_string_lossy())
            && meta.mtime().max(meta.ctime()) >= 0
            && (meta.mtime().max(meta.ctime()) as u64) < cutoff
        {
            files.push(Output {
                path,
                dev: meta.dev(),
                ino: meta.ino(),
                mtime: (meta.mtime(), meta.mtime_nsec()),
                ctime: (meta.ctime(), meta.ctime_nsec()),
                len: meta.len(),
                bytes: meta.blocks() * 512,
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn fingerprint(files: &[Output]) -> String {
    let mut hash = Sha256::new();
    for f in files {
        hash.update(f.path.as_os_str().as_encoded_bytes());
        hash.update([0]);
        for n in [
            f.dev,
            f.ino,
            f.len,
            f.mtime.0 as u64,
            f.mtime.1 as u64,
            f.ctime.0 as u64,
            f.ctime.1 as u64,
        ] {
            hash.update(n.to_le_bytes());
        }
    }
    format!("{hash:x}", hash = hash.finalize())
}

fn allocated(files: &[Output]) -> u64 {
    let mut seen = BTreeSet::new();
    files
        .iter()
        .filter(|f| seen.insert((f.dev, f.ino)))
        .map(|f| f.bytes)
        .sum()
}

fn profile_scope(target: &Path, profile: &Path) -> PathBuf {
    if let Ok(relative) = profile.strip_prefix(target.join("worktrees")) {
        let parts: Vec<_> = relative.components().collect();
        if parts.len() >= 3 {
            return target
                .join("worktrees")
                .join(parts[0].as_os_str())
                .join(parts[1].as_os_str());
        }
    }
    target.to_owned()
}

fn newest_files(path: &Path, cutoff: u64) -> Result<u64, String> {
    let mut newest = 0;
    let mut pending = vec![path.to_owned()];
    while let Some(path) = pending.pop() {
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            for e in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                pending.push(e.map_err(|e| e.to_string())?.path());
            }
        } else {
            newest = newest.max(meta.mtime().max(meta.ctime()).max(0) as u64);
            if newest >= cutoff {
                return Ok(newest);
            }
        }
    }
    if let Some(dir) = crate::paths::git_dir(path) {
        for marker in ["HEAD", "index", "logs/HEAD"] {
            if let Ok(meta) = std::fs::metadata(dir.join(marker)) {
                newest = newest.max(meta.mtime().max(meta.ctime()).max(0) as u64);
            }
        }
    }
    Ok(newest)
}

fn worktree(
    layout: &Layout,
    path: &Path,
    snapshot: &Snapshot,
    facts: Option<&dyn Facts>,
    cutoff: u64,
) -> Result<(Undo, u64), String> {
    inventory::safe(&layout.home, path)?;
    if snapshot.inside(path) > 0 {
        return Err("In use by a process.".into());
    }
    if let Some(reason) = crate::git::locked(path) {
        return Err(format!("Git worktree locked: {reason}"));
    }
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().contains(".spare-"))
    {
        return Err("A project's spare checkout.".into());
    }
    match facts {
        Some(facts) => {
            let tasks = facts.tasks()?;
            if tasks.iter().any(|task| {
                !task.ended && (task.worktree == path || task.worktree.starts_with(path))
            }) {
                return Err("A task still uses this worktree.".into());
            }
        }
        None if path.starts_with(layout.worktrees()) || path.starts_with(layout.leases()) => {
            return Err(
                "Task status is unavailable; use the task-aware host to archive it.".into(),
            );
        }
        None => {}
    }
    let undo = crate::git::removable(path)?;
    inventory::git(
        path,
        &[
            "merge-base",
            "--is-ancestor",
            "HEAD",
            "refs/remotes/origin/main",
        ],
    )
    .map_err(|_| "Not fully merged into origin/main.".to_owned())?;
    let newest = newest_files(path, cutoff)?;
    if newest >= cutoff {
        return Err("Used within the minimum age.".into());
    }
    Ok((undo, newest))
}

/// Scan only build outputs and linked worktrees. Nothing is deleted or saved.
pub fn scan(layout: &Layout, options: Options, facts: Option<&dyn Facts>) -> Result<Plan, String> {
    scan_at(layout, options, facts, &System, crate::paths::now())
}

fn scan_at(
    layout: &Layout,
    options: Options,
    facts: Option<&dyn Facts>,
    processes: &dyn Processes,
    now: u64,
) -> Result<Plan, String> {
    if options.minimum_age_secs < 86_400 {
        return Err("The minimum cleanup age is 24 hours.".into());
    }
    let cutoff = now.saturating_sub(options.minimum_age_secs);
    let snapshot = processes.snapshot()?;
    let found = inventory::discover(&layout.home);
    let mut plan = Plan {
        schema: "openagents.cleanup.plan.v1".into(),
        home: layout.home.clone(),
        created: now,
        options,
        entries: Vec::new(),
        kept: Vec::new(),
        uncovered: found.uncovered,
        digest: String::new(),
    };
    if plan.options.builds {
        for target in found.targets {
            for profile in inventory::profiles(&target) {
                let scope = profile_scope(&target, &profile);
                if snapshot.inside(&scope) > 0 {
                    plan.kept.push(Kept {
                        path: profile,
                        reason: "In use by a process.".into(),
                    });
                    continue;
                }
                let files = match outputs(&profile, cutoff) {
                    Ok(files) => files,
                    Err(reason) => {
                        plan.kept.push(Kept {
                            path: profile,
                            reason,
                        });
                        continue;
                    }
                };
                if files.is_empty() {
                    plan.kept.push(Kept {
                        path: profile,
                        reason: "No build outputs older than the minimum age.".into(),
                    });
                    continue;
                }
                if let Err(reason) = crate::inuse::Held::take(&crate::inuse::locks_of(&profile)) {
                    plan.kept.push(Kept {
                        path: profile,
                        reason,
                    });
                    continue;
                }
                plan.entries.push(Entry {
                    kind: Kind::BuildOutputs,
                    path: profile,
                    target: scope,
                    files: files.len(),
                    bytes: allocated(&files),
                    fingerprint: fingerprint(&files),
                    undo: None,
                });
            }
        }
    }
    if plan.options.worktrees {
        let task_data = facts.map(|facts| facts.tasks());
        let cached_tasks = || task_data.clone().unwrap_or_else(|| Ok(Vec::new()));
        let cached = facts.map(|_| &cached_tasks as &dyn Facts);
        let checked = crate::pool::map(&found.worktrees, |path| {
            match worktree(layout, path, &snapshot, cached, cutoff) {
                Ok((undo, newest)) => {
                    let fingerprint = format!("{}:{newest}", undo.commit);
                    let bytes = crate::paths::measure(path, &layout.home)
                        .map_err(|e| e.to_string())?
                        .bytes;
                    Ok(Entry {
                        kind: Kind::Worktree,
                        path: path.clone(),
                        target: path.clone(),
                        files: 1,
                        bytes,
                        fingerprint,
                        undo: Some(undo),
                    })
                }
                Err(reason) => Err(reason),
            }
        });
        for (path, result) in found.worktrees.into_iter().zip(checked) {
            match result {
                Ok(entry) => plan.entries.push(entry),
                Err(reason) => plan.kept.push(Kept { path, reason }),
            }
        }
    }
    let trees: Vec<_> = plan
        .entries
        .iter()
        .filter(|entry| entry.kind == Kind::Worktree)
        .map(|entry| entry.path.clone())
        .collect();
    plan.entries.retain(|entry| {
        entry.kind == Kind::Worktree || !trees.iter().any(|tree| entry.path.starts_with(tree))
    });
    plan.entries.sort_by(|a, b| a.path.cmp(&b.path));
    plan.seal()?;
    Ok(plan)
}

fn lock_profile(profile: &Path) -> Result<Vec<File>, String> {
    let mut held = Vec::new();
    for name in [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"] {
        let path = profile.join(name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("An unsafe Cargo lock file.".into());
        }
        file.try_lock()
            .map_err(|_| "Cargo is using this build folder.".to_owned())?;
        held.push(file);
    }
    Ok(held)
}

/// Apply a saved plan, rechecking identity, age, processes, locks, and Git.
pub fn apply(layout: &Layout, plan: &Plan, facts: Option<&dyn Facts>) -> Result<Receipt, String> {
    apply_selected(layout, plan, facts, &[])
}

/// Apply only the listed one-based entries, or all entries when empty.
pub fn apply_selected(
    layout: &Layout,
    plan: &Plan,
    facts: Option<&dyn Facts>,
    selection: &[usize],
) -> Result<Receipt, String> {
    plan.verify(&layout.home)?;
    let mut selected = plan.clone();
    if !selection.is_empty() {
        if selection
            .iter()
            .any(|index| *index == 0 || *index > plan.entries.len())
        {
            return Err("An entry number is outside the saved plan.".into());
        }
        selected.entries = plan
            .entries
            .iter()
            .enumerate()
            .filter(|(index, _)| selection.contains(&(index + 1)))
            .map(|(_, entry)| entry.clone())
            .collect();
    }
    apply_with(layout, &selected, facts, &System)
}

fn apply_with(
    layout: &Layout,
    plan: &Plan,
    facts: Option<&dyn Facts>,
    processes: &dyn Processes,
) -> Result<Receipt, String> {
    let cutoff = plan.created.saturating_sub(plan.options.minimum_age_secs);
    let mut receipt = Receipt {
        plan_digest: plan.digest.clone(),
        actions: Vec::new(),
    };
    let mut changed = BTreeSet::new();
    for entry in &plan.entries {
        let mut removed_count = 0;
        let mut removed_bytes = 0;
        let result = (|| -> Result<(usize, u64), String> {
            inventory::safe(&layout.home, &entry.path)?;
            if entry.kind == Kind::Worktree {
                if !plan.options.worktrees {
                    return Err("Worktrees are outside this plan's scope.".into());
                }
                let snapshot = processes.snapshot()?;
                let (undo, newest) = worktree(layout, &entry.path, &snapshot, facts, cutoff)?;
                if entry.undo.as_ref() != Some(&undo)
                    || entry.fingerprint != format!("{}:{newest}", undo.commit)
                {
                    return Err("Worktree changed since the scan.".into());
                }
                crate::git::remove(&undo)?;
                return Ok((1, entry.bytes));
            }
            if !inventory::cargo_profile(&entry.path)
                || !entry.path.starts_with(&entry.target)
                || !plan.options.builds
            {
                return Err("Not a Cargo output folder in this plan.".into());
            }
            let _held = lock_profile(&entry.path)?;
            if processes.snapshot()?.inside(&entry.target) > 0 {
                return Err("A process now uses this build folder.".into());
            }
            let files = outputs(&entry.path, cutoff)?;
            if fingerprint(&files) != entry.fingerprint {
                return Err("Build outputs changed since the scan.".into());
            }
            let mut removed = 0;
            let mut bytes = 0;
            let mut seen = BTreeSet::new();
            for file in files {
                inventory::safe(
                    &layout.home,
                    file.path.parent().ok_or("Missing parent folder.")?,
                )?;
                let meta = std::fs::symlink_metadata(&file.path).map_err(|e| e.to_string())?;
                let identity = (meta.dev(), meta.ino());
                if identity != (file.dev, file.ino)
                    || (meta.mtime(), meta.mtime_nsec()) != file.mtime
                    || meta.len() != file.len
                    || (!changed.contains(&identity)
                        && (meta.ctime(), meta.ctime_nsec()) != file.ctime)
                {
                    return Err("A build output changed during cleanup.".into());
                }
                std::fs::remove_file(&file.path).map_err(|e| e.to_string())?;
                changed.insert(identity);
                removed += 1;
                if seen.insert(identity) {
                    bytes += file.bytes;
                }
                removed_count = removed;
                removed_bytes = bytes;
            }
            Ok((removed, bytes))
        })();
        let (files, bytes, reason) = match result {
            Ok((files, bytes)) => (files, bytes, "Removed.".into()),
            Err(reason) => (removed_count, removed_bytes, reason),
        };
        receipt.actions.push(Applied {
            path: entry.path.clone(),
            files,
            bytes,
            reason,
            undo: entry.undo.clone(),
        });
    }
    Ok(receipt)
}

/// Save without replacing an existing plan or following a link.
pub fn save(path: &Path, value: &impl Serialize) -> Result<(), String> {
    use std::io::Write;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

/// Remove recognized outputs after the background runner holds their locks.
/// Shared folders, lock inodes, state markers, and unknown files stay.
pub(crate) fn clear_outputs(target: &Path) -> Result<(), String> {
    for profile in inventory::profiles(target) {
        for output in outputs(&profile, u64::MAX)? {
            std::fs::remove_file(output.path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
