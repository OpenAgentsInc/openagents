//! Discover Cargo outputs and registered Git worktrees without following links.

use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

const PRIVATE: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".config",
    ".codex",
    ".claude",
    "Documents",
    "Desktop",
    "Downloads",
    "Pictures",
    "Movies",
    "Videos",
    "Music",
    "Games",
    "Library",
];

pub fn safe(home: &Path, path: &Path) -> Result<(), String> {
    if path == home
        || !path.starts_with(home)
        || path
            .components()
            .any(|p| matches!(p, Component::ParentDir | Component::CurDir))
    {
        return Err("Outside the cleanup folders.".into());
    }
    let relative = path.strip_prefix(home).map_err(|e| e.to_string())?;
    if relative
        .components()
        .next()
        .is_some_and(|p| PRIVATE.iter().any(|name| p.as_os_str() == *name))
    {
        return Err("A personal folder.".into());
    }
    if path.starts_with(home.join(".local")) {
        let scratch = home.join(".local/share/openagents-desktop/scratch");
        if !path.starts_with(&scratch) && !scratch.starts_with(path) {
            return Err("Application data outside agent scratch.".into());
        }
    }
    for denied in [
        "work/.secrets",
        ".openagents/host",
        ".openagents/wallet",
        ".openagents/extensions",
        ".openagents/tasks",
        ".openagents/models",
        ".openagents/bin",
        ".openagents/background",
        ".openagents/nostr",
        ".local/share/keyrings",
    ] {
        if path.starts_with(home.join(denied)) {
            return Err("An application or account folder.".into());
        }
    }
    let device = std::fs::metadata(home).map_err(|e| e.to_string())?.dev();
    let mut current = home.to_owned();
    for part in relative.components() {
        current.push(part);
        let meta = std::fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() || meta.dev() != device {
            return Err("A symbolic link or another volume.".into());
        }
    }
    let keep = home.join(".openagents/disk-janitor.keep");
    if let Ok(body) = std::fs::read_to_string(keep) {
        for line in body.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let kept = line
                .strip_prefix("~/")
                .map_or_else(|| PathBuf::from(line), |p| home.join(p));
            if path.starts_with(&kept) || kept.starts_with(path) {
                return Err("Listed in disk-janitor.keep.".into());
            }
        }
    }
    Ok(())
}

pub fn cargo_profile(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("debug" | "release")
    ) && (path.join(".cargo-lock").is_file()
        || path.join(".fingerprint").is_dir()
        || path.join("deps").is_dir())
}

pub fn profiles(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_owned(), 0)];
    while let Some((path, depth)) = pending.pop() {
        if cargo_profile(&path) {
            found.push(path);
            continue;
        }
        if depth >= 12 {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) && entry.file_name() != ".git"
                {
                    pending.push((entry.path(), depth + 1));
                }
            }
        }
    }
    found.sort();
    found
}

#[derive(Default)]
pub struct Inventory {
    pub targets: Vec<PathBuf>,
    pub worktrees: Vec<PathBuf>,
    pub uncovered: Vec<PathBuf>,
}

/// The registry covers custom target paths; bounded discovery covers older builds.
pub fn discover(home: &Path) -> Inventory {
    let mut targets = BTreeSet::new();
    let mut repos = BTreeSet::new();
    let mut uncovered = BTreeSet::new();
    let mut pending = vec![(home.to_owned(), 0)];
    while let Some((path, depth)) = pending.pop() {
        if path != home && safe(home, &path).is_err() {
            continue;
        }
        if path.file_name().is_some_and(|name| {
            [
                "runs",
                "jobs",
                "public-replays",
                "bench",
                "assets",
                "node_modules",
                "venv",
                "site-packages",
                "logs",
                "evidence",
                "recordings",
                "toolchains",
            ]
            .iter()
            .any(|skip| name == *skip)
        }) {
            continue;
        }
        let checkout = path.join(".git").exists();
        if checkout {
            repos.insert(path.clone());
        }
        if cargo_profile(&path) {
            if let Some(root) = path.parent() {
                targets.insert(root.to_owned());
            }
            continue;
        }
        if [path.join("debug"), path.join("release")]
            .iter()
            .any(|profile| cargo_profile(profile))
        {
            targets.insert(path);
            continue;
        }
        let target_name = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().contains("target"));
        if target_name
            && (path.join("CACHEDIR.TAG").is_file()
                || path.join("debug/.cargo-lock").is_file()
                || path.join("release/.cargo-lock").is_file())
        {
            targets.insert(path.clone());
            continue;
        }
        if checkout || path.join("Cargo.toml").is_file() {
            // Source trees can contain millions of fixture directories. Cargo
            // uses workspace targets; custom targets also have the registry.
            for relative in [
                "target",
                "apps/openagents/target",
                "apps/openagents/web/target",
                "crates/psionic/target",
            ] {
                let target = path.join(relative);
                if target.is_dir() && safe(home, &target).is_ok() {
                    pending.push((target, 0));
                }
            }
            continue;
        }
        if depth >= 7 {
            if target_name {
                uncovered.insert(path);
            }
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let text = name.to_string_lossy();
                if !entry.file_type().is_ok_and(|kind| kind.is_dir()) || text == ".git" {
                    continue;
                }
                if text.starts_with('.')
                    && !matches!(text.as_ref(), ".openagents" | ".local" | ".cache")
                {
                    continue;
                }
                if path == home.join(".cache") && text != "openagents" {
                    continue;
                }
                if path == home.join(".local/share") && text != "openagents-desktop" {
                    continue;
                }
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(home.join(".openagents/build-targets")) {
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            if let Ok(body) = std::fs::read_to_string(entry.path()) {
                if let Some(line) = body.lines().next() {
                    let target = PathBuf::from(line);
                    if safe(home, &target).is_ok() && !profiles(&target).is_empty() {
                        targets.insert(target);
                    }
                }
            }
        }
    }
    let all_targets: Vec<_> = targets.iter().cloned().collect();
    targets.retain(|path| {
        !all_targets
            .iter()
            .any(|parent| path != parent && path.starts_with(parent))
    });
    let mut worktrees = BTreeSet::new();
    for repo in repos {
        if let Ok(text) = git(&repo, &["worktree", "list", "--porcelain", "-z"]) {
            for field in text.split('\0') {
                if let Some(path) = field.strip_prefix("worktree ") {
                    let path = PathBuf::from(path);
                    if path.join(".git").is_file() && safe(home, &path).is_ok() {
                        worktrees.insert(path);
                    }
                }
            }
        }
    }
    Inventory {
        targets: targets.into_iter().collect(),
        worktrees: worktrees.into_iter().collect(),
        uncovered: uncovered.into_iter().collect(),
    }
}

pub fn git(path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
