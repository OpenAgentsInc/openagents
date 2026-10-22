//! NIP-34 pull requests (#12455): changes too large for a patch, published
//! as a `1618` that points at a commit in the author's own public clone, and
//! moved along by the author's `1619` updates.
//!
//! The bridge fetches the tip only over `https://`, checks that the change
//! touches only crate sources, and pushes it to GitHub as `nostr/<id>`; see
//! [`crate::bridge`]. No runner checks a pull request yet, so the draft pull
//! request says so and the land queue's tests are the check.

use std::path::Path;
use std::process::Command;

use kiln::identity::Author;
use kiln::repo::Trust;
use nostr::domain::Event;
use serde_json::json;

use crate::relay;
use crate::review::{path_problem, reader};

/// A pull request, read against its repository and its author's updates.
#[derive(Debug, Clone)]
pub struct PullReview {
    pub event: Event,
    pub trust: Trust,
    pub author: Author,
    pub subject: String,
    /// The newest tip: the request's own, or its author's latest `1619`.
    pub tip: String,
    pub clone: String,
    pub merge_base: String,
    pub crates: Vec<String>,
    pub status: Option<Event>,
}

fn https(url: &str) -> bool {
    url.starts_with("https://")
        && !url.contains(char::is_whitespace)
        && !url.contains("..")
        && url.len() < 300
}

/// Read pull request `event` with the replies to it.
///
/// # Errors
///
/// A plain reason it can't be considered.
pub fn assess(event: Event, trust: Trust, replies: &[Event]) -> Result<PullReview, String> {
    if event.kind != kiln::patch::PULL_REQUEST_KIND || event.validate_crypto().is_err() {
        return Err("that event is not a signed pull request".into());
    }
    let read = nostr::domain::open_pull_request(&event)
        .map_err(|_| "the pull request is malformed".to_string())?;
    if read.repository != trust.address {
        return Err(format!("the pull request is not to {}", trust.address));
    }
    let mut tip = read.tip.clone();
    let mut clone = read.clone.first().cloned().unwrap_or_default();
    let mut merge_base = read
        .merge_base
        .clone()
        .ok_or("the pull request names no merge base")?;
    // The author's newest update moves the tip.
    if let Some(update) = replies
        .iter()
        .filter(|r| r.kind == kiln::patch::PULL_UPDATE_KIND && r.pubkey == event.pubkey)
        .filter(|r| r.validate_crypto().is_ok())
        .filter(|r| r.tag_values("E").any(|e| e == event.id))
        .max_by_key(|r| r.created_at)
        && let Ok(moved) = nostr::domain::open_pull_request(update)
    {
        tip = moved.tip;
        clone = moved.clone.first().cloned().unwrap_or(clone);
        merge_base = moved.merge_base.unwrap_or(merge_base);
    }
    if !https(&clone) {
        return Err("the pull request's clone address must be https://".into());
    }
    let status = replies
        .iter()
        .filter(|r| r.validate_crypto().is_ok())
        .filter(|r| {
            nostr::domain::open_patch_status(r)
                .ok()
                .is_some_and(|(_, target)| target == event.id)
        })
        .filter(|r| {
            trust.maintainers.contains(&r.pubkey)
                || (r.pubkey == event.pubkey && matches!(r.kind, 1_632 | 1_633))
        })
        .max_by_key(|r| r.created_at)
        .cloned();
    Ok(PullReview {
        author: kiln::identity::author(&event, &trust.maintainers),
        subject: read
            .subject
            .unwrap_or_else(|| format!("Kiln pull request {}", &event.id[..12])),
        crates: event
            .tag_values("t")
            .filter_map(|t| t.strip_prefix("crate:").map(str::to_string))
            .collect(),
        event,
        trust,
        tip,
        clone,
        merge_base,
        status,
    })
}

/// Fetch pull request `id` and its updates and statuses.
///
/// # Errors
///
/// Why it can't be read or considered.
pub async fn fetch(relay_url: &str, id: &str, address: &str) -> Result<PullReview, String> {
    let key = reader();
    let event = relay::query(relay_url, &key, json!({"ids": [id], "limit": 1}))
        .await?
        .into_iter()
        .next()
        .ok_or("the relay doesn't have that pull request")?;
    let trust = crate::review::trust(relay_url, address).await?;
    let mut replies = relay::query(
        relay_url,
        &key,
        json!({"kinds": [1619], "#E": [id], "limit": 100}),
    )
    .await?;
    replies.extend(
        relay::query(
            relay_url,
            &key,
            json!({"kinds": [1630, 1631, 1632, 1633], "#e": [id], "limit": 100}),
        )
        .await?,
    );
    assess(event, trust, &replies)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Fetch the tip and the merge base into `checkout` and check the change:
/// only crate sources, no `Cargo.toml` or `build.rs`. Returns the paths.
///
/// # Errors
///
/// A fetch that failed, or the first path that can't change through Kiln.
pub fn bring(review: &PullReview, checkout: &Path) -> Result<Vec<String>, String> {
    let has = |sha: &str| git(checkout, &["cat-file", "-e", &format!("{sha}^{{commit}}")]).is_ok();
    if !has(&review.merge_base) {
        git(
            checkout,
            &[
                "fetch",
                "--quiet",
                "--depth",
                "1",
                "origin",
                &review.merge_base,
            ],
        )?;
    }
    if !has(&review.tip) {
        git(
            checkout,
            &[
                "fetch",
                "--quiet",
                "--depth",
                "100",
                "--no-tags",
                &review.clone,
                &review.tip,
            ],
        )?;
    }
    let paths: Vec<String> = git(
        checkout,
        &["diff", "--name-only", &review.merge_base, &review.tip],
    )?
    .lines()
    .map(str::to_string)
    .collect();
    if paths.is_empty() {
        return Err("the pull request changes nothing".into());
    }
    if let Some(problem) = paths.iter().find_map(|p| path_problem(p)) {
        return Err(problem);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests as fixture;

    fn pull(w: &fixture::World, clone: &str) -> Event {
        let tags = kiln::patch::pull_tags(&kiln::patch::Pull {
            repo: &w.trust.address,
            subject: "Big change",
            tip: &"c".repeat(40),
            clone,
            branch: "big",
            merge_base: &"b".repeat(40),
            crates: &["demo".to_string()],
        });
        w.agent.sign(1_618, tags, "Why.".into(), 10)
    }

    #[test]
    fn the_authors_newest_update_moves_the_tip() {
        let w = fixture::world();
        let pr = pull(&w, "https://github.com/someone/openagents.git");
        let update = |tip: &str, at: u64, by: &kiln::cj::Key| {
            by.sign(
                1_619,
                kiln::patch::pull_update_tags(
                    &w.trust.address,
                    &pr.id,
                    &pr.pubkey,
                    tip,
                    "https://github.com/someone/openagents.git",
                    &"b".repeat(40),
                ),
                String::new(),
                at,
            )
        };
        let replies = [
            update(&"d".repeat(40), 20, &w.agent),
            update(&"e".repeat(40), 30, &w.agent),
            update(&"f".repeat(40), 40, &w.runner),
        ];
        let read = assess(pr, w.trust.clone(), &replies).unwrap();
        assert_eq!(read.tip, "e".repeat(40), "only the author moves it");
        assert_eq!(read.subject, "Big change");
        assert_eq!(read.crates, vec!["demo".to_string()]);
    }

    #[test]
    fn only_https_clones_are_fetched() {
        let w = fixture::world();
        for bad in [
            "file:///etc",
            "ssh://host/repo",
            "/home/kiln/.ssh",
            "https://x/../y",
        ] {
            assert!(
                assess(pull(&w, bad), w.trust.clone(), &[]).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_pull_request_brings_only_crate_changes() {
        let root = std::env::temp_dir().join(format!("kiln-git-pull-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let run = |dir: &Path, args: &[&str]| git(dir, args).unwrap();
        let checkout = root.join("repo");
        std::fs::create_dir_all(checkout.join("crates/demo/src")).unwrap();
        run(&checkout, &["init", "--quiet", "-b", "main"]);
        run(&checkout, &["config", "user.email", "m@example.com"]);
        run(&checkout, &["config", "user.name", "M"]);
        run(&checkout, &["config", "commit.gpgsign", "false"]);
        std::fs::write(checkout.join("crates/demo/src/lib.rs"), "a\n").unwrap();
        run(&checkout, &["add", "."]);
        run(&checkout, &["commit", "--quiet", "-m", "base"]);
        let base = run(&checkout, &["rev-parse", "HEAD"]);
        std::fs::write(checkout.join("crates/demo/src/lib.rs"), "b\n").unwrap();
        run(&checkout, &["commit", "--quiet", "-am", "change"]);
        let good = run(&checkout, &["rev-parse", "HEAD"]);
        std::fs::write(checkout.join("crates/demo/Cargo.toml"), "x\n").unwrap();
        run(&checkout, &["add", "."]);
        run(&checkout, &["commit", "--quiet", "-m", "manifest"]);
        let bad = run(&checkout, &["rev-parse", "HEAD"]);
        let w = fixture::world();
        let mut read = assess(
            pull(&w, "https://github.com/someone/openagents.git"),
            w.trust.clone(),
            &[],
        )
        .unwrap();
        read.merge_base = base;
        read.tip = good;
        assert_eq!(
            bring(&read, &checkout).unwrap(),
            vec!["crates/demo/src/lib.rs"]
        );
        read.tip = bad;
        assert!(bring(&read, &checkout).unwrap_err().contains("Cargo.toml"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
