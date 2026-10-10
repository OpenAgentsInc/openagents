//! Stamps the build with the commit it came from and whether the tree was
//! dirty, so `coder --version` says which Coder is running.
//!
//! `scripts/install-coder.sh` and the release scripts set
//! `CODER_BUILD_COMMIT` and `CODER_BUILD_DIRTY` from the checkout they
//! build, which is exact. A plain `cargo build` asks Git for the commit and
//! reruns only when `HEAD` or the branch it points at moves. It does not
//! watch the index or run `git status`: staging a file anywhere in the
//! monorepo must not rebuild this crate and its dependents, so without
//! `CODER_BUILD_DIRTY` the tree is reported as `unknown`.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CODER_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=CODER_BUILD_DIRTY");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let git = |args: &[&str]| -> Option<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    for path in ["HEAD", "packed-refs"] {
        if let Some(found) = git(&["rev-parse", "--git-path", path]) {
            watch(&dir, &found);
        }
    }
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(found) = git(&["rev-parse", "--git-path", &branch])
    {
        watch(&dir, &found);
    }

    // A short or symbolic `CODER_BUILD_COMMIT` is widened to the full
    // object name when Git can resolve it, so the stamp is always exact.
    let commit = std::env::var("CODER_BUILD_COMMIT")
        .ok()
        .map(|commit| commit.trim().to_string())
        .filter(|commit| !commit.is_empty())
        .map(|commit| {
            git(&[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{commit}^{{commit}}"),
            ])
            .filter(|full| is_object_name(full))
            .unwrap_or(commit)
        })
        .or_else(|| git(&["rev-parse", "HEAD"]).filter(|full| is_object_name(full)))
        .or_else(|| read_head(Path::new(&dir)))
        .unwrap_or_else(|| "unknown".to_string());
    let dirty = match std::env::var("CODER_BUILD_DIRTY").ok().as_deref() {
        Some("1" | "true" | "dirty") => "dirty".to_string(),
        Some("0" | "false" | "clean") => "clean".to_string(),
        _ => "unknown".to_string(),
    };
    println!("cargo:rustc-env=CODER_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=CODER_GIT_TREE={dirty}");
}

/// Reruns this script when `path`, relative to `dir` unless absolute,
/// changes.
fn watch(dir: &str, path: &str) {
    let path = Path::new(path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(dir).join(path)
    };
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// A full SHA-1 or SHA-256 object name.
fn is_object_name(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The commit `HEAD` names, read straight from the repository files, for
/// builds where the `git` command is missing or refuses the checkout (a
/// linked worktree whose common directory it cannot use, for one). Follows
/// a `.git` file to its worktree directory and its `commondir`, and a
/// symbolic `HEAD` through loose refs and then `packed-refs`.
fn read_head(start: &Path) -> Option<String> {
    let dot_git = start
        .ancestors()
        .map(|dir| dir.join(".git"))
        .find(|path| path.exists())?;
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else {
        let pointer = std::fs::read_to_string(&dot_git).ok()?;
        let target = Path::new(pointer.trim().strip_prefix("gitdir:")?.trim());
        dot_git.parent()?.join(target)
    };
    let common = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(common) => git_dir.join(common.trim()),
        Err(_) => git_dir.clone(),
    };
    let head_path = git_dir.join("HEAD");
    watch(".", &head_path.to_string_lossy());
    let head = std::fs::read_to_string(head_path).ok()?;
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref:").map(str::trim) else {
        return is_object_name(head).then(|| head.to_string());
    };
    for dir in [&git_dir, &common] {
        let loose = dir.join(reference);
        if let Ok(found) = std::fs::read_to_string(&loose) {
            watch(".", &loose.to_string_lossy());
            let found = found.trim();
            if is_object_name(found) {
                return Some(found.to_string());
            }
        }
    }
    let packed_path = common.join("packed-refs");
    watch(".", &packed_path.to_string_lossy());
    let packed = std::fs::read_to_string(packed_path).ok()?;
    packed.lines().find_map(|line| {
        let (name, found) = line.split_once(' ').map(|(sha, name)| (name, sha))?;
        (name.trim() == reference && is_object_name(found)).then(|| found.to_string())
    })
}
