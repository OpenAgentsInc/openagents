//! Stamps the build with the commit it came from and whether the tree was
//! dirty, so `coder --version` says which Coder is running.
//!
//! `scripts/install-coder.sh` sets `CODER_BUILD_COMMIT` and
//! `CODER_BUILD_DIRTY` from the checkout it builds, which is exact. A
//! plain `cargo build` asks Git itself, and reruns when `HEAD`, the branch
//! it points at, or the index moves. An edit to a file outside this crate
//! that is never staged does not rerun the script, so such a build can say
//! `clean` for a tree that is not; the install script is the exact path.

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
    for path in ["HEAD", "index", "packed-refs"] {
        if let Some(found) = git(&["rev-parse", "--git-path", path]) {
            watch(&dir, &found);
        }
    }
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(found) = git(&["rev-parse", "--git-path", &branch])
    {
        watch(&dir, &found);
    }

    let commit = std::env::var("CODER_BUILD_COMMIT")
        .ok()
        .filter(|commit| !commit.trim().is_empty())
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let dirty = match std::env::var("CODER_BUILD_DIRTY").ok().as_deref() {
        Some("1" | "true" | "dirty") => "dirty".to_string(),
        Some("0" | "false" | "clean") => "clean".to_string(),
        _ => match git(&["status", "--porcelain", "--untracked-files=no"]) {
            Some(status) if status.is_empty() => "clean".to_string(),
            Some(_) => "dirty".to_string(),
            None => "unknown".to_string(),
        },
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
