//! What a maintainer, the GitHub bridge, or the Grid Foreman reads before
//! acting on a Kiln patch (#12452): the patch itself, who wrote it, the
//! runner's check of exactly this text at its base, and the newest status
//! that counts.

use std::time::Duration;

use kiln::identity::Author;
use kiln::repo::Trust;
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::Serialize;
use serde_json::json;

use crate::relay;

/// A runner's check comment on a patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub id: String,
    pub runner: String,
    pub passed: bool,
    pub at: u64,
    pub summary: String,
}

/// A patch, read against its repository.
#[derive(Debug, Clone)]
pub struct Review {
    pub patch: Event,
    pub trust: Trust,
    pub author: Author,
    /// The commit the patch applies to (its `parent-commit` tag).
    pub base: String,
    /// The crates it changes.
    pub crates: Vec<String>,
    /// The newest counted check of exactly this text at `base`.
    pub check: Option<Check>,
    /// The newest status that counts: a maintainer's, or the author's own
    /// close or draft.
    pub status: Option<Event>,
}

fn hex40(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Why a path can't land through Kiln, if it can't.
#[must_use]
pub fn path_problem(path: &str) -> Option<String> {
    let plain = path
        .split('/')
        .all(|p| !p.is_empty() && p != "." && p != "..")
        && !path.starts_with('/');
    if !plain || !path.starts_with("crates/") {
        return Some(format!("{path} is outside crates/"));
    }
    if path.ends_with("/build.rs") || path.ends_with("Cargo.toml") {
        return Some(format!("{path} can't change through Kiln"));
    }
    None
}

/// Read `patch` against `trust` and the events that reply to it.
///
/// # Errors
///
/// A plain reason when the patch can't be considered at all.
pub fn assess(patch: Event, trust: Trust, replies: &[Event]) -> Result<Review, String> {
    if patch.kind != kiln::patch::PATCH_KIND || patch.validate_crypto().is_err() {
        return Err("that event is not a signed patch".into());
    }
    if nostr::domain::open_patch(&patch).ok().as_deref() != Some(trust.address.as_str()) {
        return Err(format!("the patch is not to {}", trust.address));
    }
    let base = patch
        .tag_values("parent-commit")
        .next()
        .filter(|b| hex40(b))
        .ok_or("the patch names no base commit")?
        .to_lowercase();
    let paths = kiln::remote::patch_paths(&patch.content);
    if paths.is_empty() {
        return Err("the patch changes no files".into());
    }
    if let Some(problem) = paths.iter().find_map(|p| path_problem(p)) {
        return Err(problem);
    }
    let found = kiln::patch::secret_findings(&patch.content);
    if !found.is_empty() {
        return Err(format!("the patch seems to hold {}", found.join(", ")));
    }
    let digest = kiln::sha256_hex(patch.content.as_bytes());
    let check = replies
        .iter()
        .filter(|r| r.validate_crypto().is_ok() && trust.counts_check(&r.pubkey))
        .filter(|r| r.tag_values("e").any(|e| e == patch.id))
        .filter(|r| r.tag_values("kiln-patch-sha256").any(|d| d == digest))
        .filter(|r| {
            r.tag_values("kiln-base")
                .any(|b| b.eq_ignore_ascii_case(&base))
        })
        .filter_map(|r| {
            kiln::patch::read_check(r).map(|(passed, text)| Check {
                id: r.id.clone(),
                runner: r.pubkey.clone(),
                passed,
                at: r.created_at,
                summary: text.lines().next().unwrap_or_default().to_string(),
            })
        })
        .max_by_key(|c| c.at);
    let status = replies
        .iter()
        .filter(|r| r.validate_crypto().is_ok())
        .filter(|r| {
            nostr::domain::open_patch_status(r)
                .ok()
                .is_some_and(|(_, target)| target == patch.id)
        })
        .filter(|r| {
            trust.maintainers.contains(&r.pubkey)
                || (r.pubkey == patch.pubkey && matches!(r.kind, 1_632 | 1_633))
        })
        .max_by_key(|r| r.created_at)
        .cloned();
    let author = kiln::identity::author(&patch, &trust.maintainers);
    Ok(Review {
        crates: kiln::remote::touched_crates(&paths),
        patch,
        trust,
        author,
        base,
        check,
        status,
    })
}

impl Review {
    /// Whether it may be applied or opened as a pull request: a passing
    /// counted check (unless `without_check`), and a vouched author (unless
    /// `allow_unverified`).
    ///
    /// # Errors
    ///
    /// What is missing.
    pub fn ready(&self, without_check: bool, allow_unverified: bool) -> Result<(), String> {
        if !without_check {
            match &self.check {
                Some(c) if c.passed => {}
                Some(_) => return Err("the runner's check of this patch failed".into()),
                None => {
                    return Err("no listed runner has checked this exact patch at its base".into());
                }
            }
        }
        if !allow_unverified && matches!(self.author, Author::Unverified(_)) {
            return Err(format!(
                "the patch is {}; pass --allow-unverified to take it anyway",
                self.author.label()
            ));
        }
        Ok(())
    }

    /// The status word of the newest counted status.
    #[must_use]
    pub fn status_word(&self) -> &'static str {
        match self.status.as_ref().map(|s| s.kind) {
            Some(1_631) => "applied",
            Some(1_632) => "closed",
            Some(1_633) => "draft",
            Some(_) => "open",
            None => "none",
        }
    }

    /// A summary for printing.
    #[must_use]
    pub fn summary(&self) -> serde_json::Value {
        json!({
            "id": self.patch.id,
            "subject": kiln::patch::subject(&self.patch.content),
            "author": self.author.label(),
            "base": self.base,
            "crates": self.crates,
            "check": self.check,
            "status": self.status_word(),
            "repository": self.trust.address,
        })
    }
}

/// A key to read the relay with: made for the call, never saved.
#[must_use]
pub fn reader() -> SecretKey {
    loop {
        if let Ok(key) = SecretKey::from_byte_array(secp256k1::rand::random()) {
            return key;
        }
    }
}

/// Fetch the announcement for `address`, or a trust list of its owner alone.
///
/// # Errors
///
/// Why the relay couldn't be read.
pub async fn trust(relay_url: &str, address: &str) -> Result<Trust, String> {
    let mut parts = address.split(':');
    let (Some("30617"), Some(owner), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
        return Err("a repository address is 30617:<key>:<id>".into());
    };
    let found = relay::query(
        relay_url,
        &reader(),
        json!({"kinds": [30617], "authors": [owner], "#d": [id], "limit": 1}),
    )
    .await?;
    Ok(found
        .iter()
        .max_by_key(|e| e.created_at)
        .and_then(kiln::repo::read)
        .unwrap_or_else(|| Trust::of_address(address)))
}

/// Fetch patch `id` and everything needed to judge it. `address` is the
/// repository it must be to; `None` takes the patch's own `a` tag.
///
/// # Errors
///
/// Why the patch can't be read or considered.
pub async fn fetch(relay_url: &str, id: &str, address: Option<&str>) -> Result<Review, String> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("an event id is 64 hex characters".into());
    }
    let key = reader();
    let patch = relay::query(relay_url, &key, json!({"ids": [id], "limit": 1}))
        .await?
        .into_iter()
        .next()
        .ok_or("the relay doesn't have that patch")?;
    let address = match address {
        Some(a) => a.to_string(),
        None => nostr::domain::open_patch(&patch).map_err(|_| "the patch names no repository")?,
    };
    let trust = trust(relay_url, &address).await?;
    let replies = relay::query(
        relay_url,
        &key,
        json!({"kinds": [1111, 1630, 1631, 1632, 1633], "#e": [id], "limit": 200}),
    )
    .await?;
    assess(patch, trust, &replies)
}

/// How long to wait between polls.
pub const POLL: Duration = Duration::from_secs(20);

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use kiln::cj::Key;
    use nostr::domain::Tag;
    use std::collections::BTreeMap;

    pub(crate) const BASE_FILE: &str = "pub fn a() -> u32 {\n    1\n}\n";
    pub(crate) const NEW_FILE: &str = "pub fn a() -> u32 {\n    2\n}\n";

    pub(crate) fn patch_text() -> String {
        let base = BTreeMap::from([("crates/demo/src/lib.rs".to_string(), BASE_FILE.to_string())]);
        let current =
            BTreeMap::from([("crates/demo/src/lib.rs".to_string(), NEW_FILE.to_string())]);
        kiln::patch::format_patch(
            &kiln::patch::Header {
                author: "Kiln agent",
                email: "npub1agent@nostr",
                subject: "Return 2",
                body: "Because.",
                date: 1_792_540_800,
            },
            &kiln::patch::changes(&base, &current),
        )
    }

    pub(crate) struct World {
        pub repo: Key,
        pub runner: Key,
        pub agent: Key,
        pub trust: Trust,
    }

    pub(crate) fn world() -> World {
        let repo = Key::from_bytes([5; 32]).unwrap();
        let runner = Key::from_bytes([6; 32]).unwrap();
        let agent = Key::from_bytes([8; 32]).unwrap();
        let mut trust = Trust::of_address(&format!("30617:{}:openagents", repo.pubkey()));
        trust.runners.insert(runner.pubkey().to_string());
        World {
            repo,
            runner,
            agent,
            trust,
        }
    }

    pub(crate) fn patch(w: &World, base: &str, credential: bool) -> Event {
        let mut tags = kiln::patch::patch_tags(
            &w.trust.address,
            w.repo.pubkey(),
            base,
            &["demo".to_string()],
        );
        if credential {
            let owner = secp256k1::SecretKey::from_byte_array([5; 32]).unwrap();
            let c = kiln::identity::mint(
                &owner,
                w.agent.pubkey(),
                &[kiln::identity::GIT_PROPOSE],
                4_000_000_000,
            )
            .unwrap();
            tags.extend(c.tags());
        }
        w.agent
            .sign(kiln::patch::PATCH_KIND, tags, patch_text(), 1_792_540_800)
    }

    pub(crate) fn check(w: &World, patch: &Event, by: &Key, passed: bool, base: &str) -> Event {
        let mut tags = kiln::patch::check_tags(&patch.id, &patch.pubkey, "wss://r", passed);
        tags.push(Tag::new(vec!["kiln-base".into(), base.into()]));
        tags.push(Tag::new(vec![
            "kiln-patch-sha256".into(),
            kiln::sha256_hex(patch.content.as_bytes()),
        ]));
        let _ = w;
        by.sign(1_111, tags, "Passed `cargo test`".into(), 1_792_540_900)
    }

    #[test]
    fn only_a_listed_runners_check_of_this_text_counts() {
        let w = world();
        let base = "a".repeat(40);
        let p = patch(&w, &base, true);
        let stranger = Key::from_bytes([11; 32]).unwrap();
        let review = assess(
            p.clone(),
            w.trust.clone(),
            &[check(&w, &p, &stranger, true, &base)],
        )
        .unwrap();
        assert!(review.check.is_none());
        assert!(review.ready(false, false).is_err());
        let other_base = check(&w, &p, &w.runner, true, &"b".repeat(40));
        let review = assess(p.clone(), w.trust.clone(), &[other_base]).unwrap();
        assert!(review.check.is_none(), "a check at another base");
        let good = check(&w, &p, &w.runner, true, &base);
        let review = assess(p.clone(), w.trust.clone(), &[good]).unwrap();
        assert!(review.check.as_ref().is_some_and(|c| c.passed));
        assert!(
            review.ready(false, false).is_ok(),
            "the owner is a maintainer"
        );
        assert_eq!(review.crates, vec!["demo".to_string()]);
    }

    #[test]
    fn an_unvouched_author_and_odd_paths_are_held_back() {
        let w = world();
        let base = "a".repeat(40);
        let p = patch(&w, &base, false);
        let good = check(&w, &p, &w.runner, true, &base);
        let review = assess(p, w.trust.clone(), &[good]).unwrap();
        assert!(
            review
                .ready(false, false)
                .unwrap_err()
                .contains("unverified")
        );
        assert!(review.ready(false, true).is_ok());
        assert!(path_problem("crates/demo/build.rs").is_some());
        assert!(path_problem("scripts/x.sh").is_some());
        assert!(path_problem("crates/demo/src/lib.rs").is_none());
    }

    #[test]
    fn only_a_maintainers_status_counts() {
        let w = world();
        let base = "a".repeat(40);
        let p = patch(&w, &base, true);
        let tags = |kind_word: &str| {
            let _ = kind_word;
            kiln::patch::status_tags(&p.id, &p.pubkey, &w.trust.address, "wss://r", &[], None)
        };
        let by_agent = w.agent.sign(1_631, tags("applied"), String::new(), 10);
        let review = assess(p.clone(), w.trust.clone(), &[by_agent]).unwrap();
        assert_eq!(review.status_word(), "none");
        let by_repo = w.repo.sign(1_630, tags("open"), String::new(), 11);
        let review = assess(p.clone(), w.trust.clone(), &[by_repo]).unwrap();
        assert_eq!(review.status_word(), "open");
    }
}
