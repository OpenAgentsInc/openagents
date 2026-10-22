//! Apply a reviewed Kiln patch on a branch of its own (#12452): a new
//! worktree at the patch's base, `git am -3`, and trailers that credit the
//! author and name the event. Nothing here pushes, merges, or touches
//! `main`; landing stays a maintainer's action.

use std::path::{Path, PathBuf};
use std::process::Command;

use kiln::identity::{Author, npub};

use crate::review::Review;

/// Where an applied patch is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub dir: PathBuf,
    pub branch: String,
    pub commit: String,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The branch a patch lands on: `kiln/<first 12 of the event id>`.
#[must_use]
pub fn branch_name(review: &Review) -> String {
    format!("kiln/{}", &review.patch.id[..12])
}

/// The commit trailers that credit a patch.
#[must_use]
pub fn trailers(review: &Review, relay: &str) -> Vec<String> {
    let mut out = vec![format!("Nostr-Patch: {} {relay}", review.patch.id)];
    match &review.author {
        Author::Agent { agent, owner, .. } => {
            out.push(format!("Kiln-Agent: {}", npub(agent)));
            out.push(format!("Authorized-By: {}", npub(owner)));
        }
        Author::Maintainer(key) | Author::Unverified(key) => {
            out.push(format!("Kiln-Author: {}", npub(key)));
        }
    }
    if let Some(check) = &review.check {
        out.push(format!("Kiln-Check: {} {}", check.id, npub(&check.runner)));
    }
    out
}

/// Apply `review`'s patch from `checkout` into a new worktree at `into`, on
/// a new branch, then rebase onto `onto` when given.
///
/// # Errors
///
/// The git step that failed; the worktree is left for inspection.
pub fn apply(
    review: &Review,
    checkout: &Path,
    into: &Path,
    onto: Option<&str>,
    relay: &str,
) -> Result<Applied, String> {
    let base = &review.base;
    if git(checkout, &["cat-file", "-e", &format!("{base}^{{commit}}")]).is_err() {
        git(checkout, &["fetch", "--quiet", "origin", base])?;
    }
    if into.exists() {
        return Err(format!("{} already exists", into.display()));
    }
    let branch = branch_name(review);
    git(
        checkout,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            &branch,
            &into.to_string_lossy(),
            base,
        ],
    )?;
    let mbox = into.with_extension("patch");
    std::fs::write(&mbox, &review.patch.content).map_err(|e| e.to_string())?;
    let applied = git(into, &["am", "-3", "--quiet", &mbox.to_string_lossy()]);
    let _ = std::fs::remove_file(&mbox);
    if let Err(e) = applied {
        let _ = git(into, &["am", "--abort"]);
        return Err(e);
    }
    let mut amend = vec!["commit", "--amend", "--no-edit", "--quiet"];
    let lines = trailers(review, relay);
    for line in &lines {
        amend.push("--trailer");
        amend.push(line);
    }
    git(into, &amend)?;
    if let Some(onto) = onto {
        git(into, &["rebase", "--quiet", onto])?;
    }
    let commit = git(into, &["rev-parse", "HEAD"])?;
    Ok(Applied {
        dir: into.to_path_buf(),
        branch,
        commit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{assess, tests as fixture};

    fn run(dir: &Path, args: &[&str]) -> String {
        git(dir, args).unwrap()
    }

    #[test]
    fn a_checked_patch_applies_on_its_own_branch_with_credit() {
        let root = std::env::temp_dir().join(format!("kiln-git-apply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let checkout = root.join("repo");
        std::fs::create_dir_all(checkout.join("crates/demo/src")).unwrap();
        run(&checkout, &["init", "--quiet", "-b", "main"]);
        run(
            &checkout,
            &["config", "user.email", "maintainer@example.com"],
        );
        run(&checkout, &["config", "user.name", "Maintainer"]);
        run(&checkout, &["config", "commit.gpgsign", "false"]);
        std::fs::write(checkout.join("crates/demo/src/lib.rs"), fixture::BASE_FILE).unwrap();
        run(&checkout, &["add", "."]);
        run(&checkout, &["commit", "--quiet", "-m", "base"]);
        let base = run(&checkout, &["rev-parse", "HEAD"]);
        let w = fixture::world();
        let patch = fixture::patch(&w, &base, true);
        let check = fixture::check(&w, &patch, &w.runner, true, &base);
        let review = assess(patch, w.trust.clone(), &[check]).unwrap();
        review.ready(false, false).unwrap();
        let into = root.join("applied");
        let applied = apply(&review, &checkout, &into, None, "wss://r").unwrap();
        assert_eq!(applied.branch, branch_name(&review));
        assert_eq!(
            std::fs::read_to_string(into.join("crates/demo/src/lib.rs")).unwrap(),
            fixture::NEW_FILE
        );
        let message = run(&into, &["log", "-1", "--format=%B"]);
        assert!(message.starts_with("Return 2"), "{message}");
        assert!(message.contains(&format!("Nostr-Patch: {}", review.patch.id)));
        assert!(message.contains("Authorized-By: npub1"), "{message}");
        let author = run(&into, &["log", "-1", "--format=%an <%ae>"]);
        assert_eq!(author, "Kiln agent <npub1agent@nostr>");
        assert_eq!(
            run(&checkout, &["rev-parse", "main"]),
            base,
            "main never moves"
        );
        assert!(apply(&review, &checkout, &into, None, "wss://r").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
