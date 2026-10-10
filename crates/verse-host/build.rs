//! Records the local source revision without requiring a network or Git at runtime.
use std::process::Command;
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
fn main() {
    println!("cargo:rerun-if-env-changed=VERSE_SOURCE_REVISION");
    for path in [
        "src",
        "Cargo.toml",
        "../../Cargo.toml",
        "../../Cargo.lock",
        "../verse-world/src",
        "../verse-world/Cargo.toml",
        "../verse-engine/src",
        "../verse-engine/Cargo.toml",
        "../physics/src",
        "../physics/Cargo.toml",
        "../verse-content/src",
        "../verse-content/Cargo.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    if let Some(path) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={path}");
    }
    // Not the index: staging any file in the monorepo would rerun this
    // script. The source paths above already rerun it when a file that
    // feeds the `-modified` flag changes.
    if let Some(path) = git(&["rev-parse", "--git-path", "packed-refs"]) {
        println!("cargo:rerun-if-changed={path}");
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let revision = std::env::var("VERSE_SOURCE_REVISION")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .filter(|revision| revision.len() == 40 && revision.bytes().all(|c| c.is_ascii_hexdigit()));
    let modified = git(&[
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--",
        ".",
        "../verse-world",
        "../verse-engine",
        "../physics",
        "../verse-content",
        "../../Cargo.toml",
        "../../Cargo.lock",
    ])
    .is_some_and(|status| !status.is_empty());
    let revision = revision
        .map(|r| if modified { format!("{r}-modified") } else { r })
        .unwrap_or_else(|| "unrecorded".into());
    println!("cargo:rustc-env=VERSE_HOST_SOURCE_REVISION={revision}");
}
