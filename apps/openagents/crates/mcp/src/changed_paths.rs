//! `changed_paths`: which files each chat changed against a base ref, and
//! which chats overlap. Foreman serializes pushes only between chats whose
//! changed paths overlap (#11972), so this is read-only: it runs `git diff`
//! and `git ls-files` in each chat's folder and never fetches or writes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

/// The ref a chat's changes are measured against when the caller names none.
pub const DEFAULT_BASE: &str = "origin/main";

const GIT_TIMEOUT: Duration = Duration::from_secs(20);

/// Files changed in the checkout at `dir` relative to `base`: committed
/// changes since the merge base, uncommitted changes, and untracked files,
/// as repository-relative paths, sorted and deduplicated.
pub async fn changed_files(dir: &Path, base: &str) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(
        !base.starts_with('-') && !base.is_empty(),
        "base must be a ref name"
    );
    anyhow::ensure!(dir.is_dir(), "{} is not a folder", dir.display());
    let committed = git(dir, &["diff", "--name-only", &format!("{base}...HEAD")]).await?;
    let uncommitted = git(dir, &["diff", "--name-only", "HEAD"]).await?;
    let untracked = git(dir, &["ls-files", "--others", "--exclude-standard"]).await?;
    let mut files = BTreeSet::new();
    for out in [committed, uncommitted, untracked] {
        files.extend(
            out.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned),
        );
    }
    Ok(files.into_iter().collect())
}

async fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let run = tokio::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(GIT_TIMEOUT, run)
        .await
        .map_err(|_| anyhow::anyhow!("git {} timed out", args[0]))??;
    anyhow::ensure!(
        out.status.success(),
        "git {} failed: {}",
        args[0],
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Pairs of chats that change at least one same path, and the groups of
/// chats linked through such pairs. A chat alone in its group can push as
/// soon as its own tests pass; chats in one group push in order.
///
/// Input order is kept: pairs and groups list chats in the order given.
pub fn overlaps(chats: &[(String, Vec<String>)]) -> Value {
    let sets: Vec<BTreeSet<&str>> = chats
        .iter()
        .map(|(_, paths)| paths.iter().map(String::as_str).collect())
        .collect();
    let mut parent: Vec<usize> = (0..chats.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut pairs = Vec::new();
    for a in 0..chats.len() {
        for b in a + 1..chats.len() {
            let shared: Vec<&str> = sets[a].intersection(&sets[b]).copied().collect();
            if shared.is_empty() {
                continue;
            }
            pairs.push(json!({ "chats": [chats[a].0, chats[b].0], "paths": shared }));
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for i in 0..chats.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(&chats[i].0);
    }
    let (shared, independent): (Vec<_>, Vec<_>) =
        groups.into_values().partition(|group| group.len() > 1);
    json!({
        "overlaps": pairs,
        "pushInOrder": shared,
        "pushFreely": independent.into_iter().flatten().collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: &str, paths: &[&str]) -> (String, Vec<String>) {
        (id.into(), paths.iter().map(|p| p.to_string()).collect())
    }

    #[test]
    fn disjoint_chats_push_freely() {
        let out = overlaps(&[
            chat("a", &["crates/mcp/src/tools.rs"]),
            chat("b", &["crates/ui/src/shell.rs"]),
            chat("c", &[]),
        ]);
        assert_eq!(out["overlaps"], json!([]));
        assert_eq!(out["pushInOrder"], json!([]));
        assert_eq!(out["pushFreely"], json!(["a", "b", "c"]));
    }

    #[test]
    fn shared_paths_group_transitively_and_name_the_files() {
        let out = overlaps(&[
            chat("a", &["shell.rs", "x.rs"]),
            chat("b", &["sidebar.rs"]),
            chat("c", &["shell.rs", "sidebar.rs"]),
            chat("d", &["docs/readme.md"]),
        ]);
        assert_eq!(
            out["overlaps"],
            json!([
                {"chats": ["a", "c"], "paths": ["shell.rs"]},
                {"chats": ["b", "c"], "paths": ["sidebar.rs"]},
            ])
        );
        assert_eq!(out["pushInOrder"], json!([["a", "b", "c"]]));
        assert_eq!(out["pushFreely"], json!(["d"]));
    }

    #[tokio::test]
    async fn reads_committed_uncommitted_and_untracked_changes() {
        let dir = std::env::temp_dir().join(format!("oa-changed-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let sh = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        sh(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("base.txt"), "1").unwrap();
        sh(&["add", "."]);
        sh(&["commit", "-qm", "base"]);
        sh(&["branch", "base"]);
        std::fs::write(dir.join("committed.txt"), "1").unwrap();
        sh(&["add", "."]);
        sh(&["commit", "-qm", "work"]);
        std::fs::write(dir.join("base.txt"), "2").unwrap();
        std::fs::write(dir.join("new.txt"), "1").unwrap();
        let files = changed_files(&dir, "base").await.unwrap();
        assert_eq!(files, ["base.txt", "committed.txt", "new.txt"]);
        assert!(changed_files(&dir, "--output=x").await.is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
