//! NIP-34 git collaboration.
//!
//! Kind `30617` announces a repository — `d` is the repo id and
//! `name`, `description`, `web`, `clone`, `relays`, `maintainers`,
//! and `r`-marked-`euc` tags describe it. Kind `30618` announces
//! branch and tag state through `refs/<heads|tags>/<name>` tags and a
//! `HEAD` ref. Kind `1617` is a patch or cover letter, `1618` a pull
//! request, `1619` a pull-request update, `1621` an issue — all point
//! at the repo with an `a` tag. Kinds `1630`–`1633` set a root
//! patch's, PR's, or issue's status. Replies use NIP-22 comments.
//! NIP-34 is a draft, so the kinds are not added to the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const REPO_KIND: u16 = 30_617;
const REPO_STATE_KIND: u16 = 30_618;
const PATCH_KIND: u16 = 1_617;
const PULL_REQUEST_KIND: u16 = 1_618;
const PR_UPDATE_KIND: u16 = 1_619;
const ISSUE_KIND: u16 = 1_621;

/// A kind `30617` repository announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The `d` repo identifier.
    pub identifier: String,
    /// The human-readable project name.
    pub name: Option<String>,
    /// The project description.
    pub description: Option<String>,
    /// Web pages for browsing.
    pub web: Vec<String>,
    /// `git clone` URLs.
    pub clone: Vec<String>,
    /// Relays the repository monitors.
    pub relays: Vec<String>,
    /// Other recognized maintainer pubkeys.
    pub maintainers: Vec<String>,
    /// The earliest unique commit identifying the project across
    /// forks — the `r` tag marked `euc`.
    pub earliest_commit: Option<String>,
    /// Hashtag labels.
    pub labels: Vec<String>,
}

/// Read a kind `30617` repository announcement.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing or
/// empty `d`, or a malformed tag value.
pub fn open_repository(event: &Event) -> Result<Repository, DomainError> {
    if event.kind != REPO_KIND {
        return Err(invalid("a repository announcement is kind 30617"));
    }
    let identifier = event
        .distinct_parameter()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("a repository announcement needs a d identifier"))?
        .to_string();
    let text = |name: &str| event.tag_values(name).next().map(str::to_string);
    let all = |name: &str| event.tag_values(name).map(str::to_string).collect();
    let maintainers = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("maintainers"))
        .flat_map(|tag| tag.0.iter().skip(1))
        .map(|value| {
            decode_lower_hex::<32>(value, "maintainers")
                .map_err(|_| invalid("a maintainer is a pubkey"))?;
            Ok(value.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut earliest_commit = None;
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("r")) {
        if tag.0.get(2).is_some_and(|marker| marker == "euc") {
            let value = tag
                .value()
                .filter(|value| decode_lower_hex::<20>(value, "euc").is_ok())
                .ok_or_else(|| invalid("the euc commit id is 40-char lowercase hex"))?;
            earliest_commit = Some(value.to_string());
        }
    }
    Ok(Repository {
        identifier,
        name: text("name"),
        description: text("description"),
        web: all("web"),
        clone: all("clone"),
        relays: all("relays"),
        maintainers,
        earliest_commit,
        labels: all("t"),
    })
}

/// A kind `30618` repository state announcement: each
/// `refs/<heads|tags>/<name>` tag's commit id plus the `HEAD` ref.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryState {
    /// The repo identifier — matches the announcement's `d`.
    pub identifier: String,
    /// `(ref name, commit id)` pairs.
    pub refs: Vec<(String, String)>,
    /// The `HEAD` tag's `ref: refs/heads/<branch>` target.
    pub head: Option<String>,
}

/// Read a kind `30618` repository state event.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing
/// `d`, or a malformed ref.
pub fn open_repository_state(event: &Event) -> Result<RepositoryState, DomainError> {
    if event.kind != REPO_STATE_KIND {
        return Err(invalid("a repository state announcement is kind 30618"));
    }
    let identifier = event
        .distinct_parameter()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("a repository state needs a d identifier"))?
        .to_string();
    let mut refs = Vec::new();
    for tag in &event.tags {
        let Some(name) = tag.name() else { continue };
        if !(name.starts_with("refs/heads/") || name.starts_with("refs/tags/")) {
            continue;
        }
        let commit = tag
            .value()
            .filter(|value| decode_lower_hex::<20>(value, "commit").is_ok())
            .ok_or_else(|| invalid("a ref names a 40-char commit id"))?;
        refs.push((name.to_string(), commit.to_string()));
    }
    let head = event
        .tag_values("HEAD")
        .next()
        .map(|value| {
            value
                .strip_prefix("ref: refs/heads/")
                .filter(|branch| !branch.is_empty())
                .map(str::to_string)
                .ok_or_else(|| invalid("a HEAD tag is ref: refs/heads/<branch>"))
        })
        .transpose()?;
    Ok(RepositoryState {
        identifier,
        refs,
        head,
    })
}

/// The `30617:<pubkey>:<identifier>` address a collaboration event's
/// `a` tag must carry.
fn repository_address(event: &Event) -> Result<String, DomainError> {
    let value = event
        .tag_values("a")
        .find(|value| value.starts_with("30617:"))
        .ok_or_else(|| invalid("the event names its repository in a 30617 a tag"))?;
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() == 3
        && decode_lower_hex::<32>(parts[1], "a pubkey").is_ok()
        && !parts[2].is_empty()
    {
        Ok(value.to_string())
    } else {
        Err(invalid("an a tag is 30617:pubkey:identifier"))
    }
}

/// The `t` markers a patch may carry: `root` for a series root,
/// `root-revision` for the first patch of a revision.
#[must_use]
pub fn patch_markers(event: &Event) -> Vec<&str> {
    event
        .tag_values("t")
        .filter(|value| matches!(*value, "root" | "root-revision"))
        .collect()
}

/// Read a kind `1617` patch: the repository `a` tag, the optional
/// `t` series markers, and the commit identity fields when the author
/// commits to a stable id.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or no
/// repository `a` tag.
pub fn open_patch(event: &Event) -> Result<String, DomainError> {
    if event.kind != PATCH_KIND {
        return Err(invalid("a patch is kind 1617"));
    }
    repository_address(event)
}

/// A kind `1618` pull request or `1619` pull-request update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequest {
    /// The base repository's `a` address.
    pub repository: String,
    /// The tip commit — the `c` tag.
    pub tip: String,
    /// Clone URLs the commit can be fetched from — at least one.
    pub clone: Vec<String>,
    /// The most recent common ancestor — the `merge-base` tag.
    pub merge_base: Option<String>,
    /// The PR subject (`1618` only, when present).
    pub subject: Option<String>,
    /// The root patch this PR revises (`e` tag, when present).
    pub revises: Option<String>,
}

/// Read a kind `1618` pull request or `1619` update: a repository
/// `a`, the tip `c`, and at least one `clone` URL.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing
/// `a`, `c`, or clone URL.
pub fn open_pull_request(event: &Event) -> Result<PullRequest, DomainError> {
    if event.kind != PULL_REQUEST_KIND && event.kind != PR_UPDATE_KIND {
        return Err(invalid("a pull request is kind 1618 or 1619"));
    }
    let repository = repository_address(event)?;
    let tip = commit_field(event, "c")?;
    let clone: Vec<String> = event.tag_values("clone").map(str::to_string).collect();
    if clone.is_empty() {
        return Err(invalid("a pull request needs at least one clone url"));
    }
    let merge_base = event
        .tag_values("merge-base")
        .next()
        .map(|value| {
            decode_lower_hex::<20>(value, "merge-base")
                .map_err(|_| invalid("a merge-base is a 40-char commit id"))?;
            Ok(value.to_string())
        })
        .transpose()?;
    Ok(PullRequest {
        repository,
        tip,
        clone,
        merge_base,
        subject: event
            .tag_values("subject")
            .next()
            .map(str::to_string)
            .filter(|_| event.kind == PULL_REQUEST_KIND),
        revises: event
            .tag_values("e")
            .next()
            .filter(|value| decode_lower_hex::<32>(value, "e").is_ok())
            .map(str::to_string),
    })
}

/// Read a kind `1621` issue: the repository `a` tag and optional
/// `subject` and `t` labels.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or no `a`.
pub fn open_issue(event: &Event) -> Result<String, DomainError> {
    if event.kind != ISSUE_KIND {
        return Err(invalid("an issue is kind 1621"));
    }
    repository_address(event)
}

/// The status a kind `1630`–`1633` event declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchStatus {
    /// `1630`: open — the default.
    Open,
    /// `1631`: applied or merged for patches, resolved for issues.
    Applied,
    /// `1632`: closed without applying.
    Closed,
    /// `1633`: draft.
    Draft,
}

/// Read a kind `1630`–`1633` status event: the root target's `e` tag
/// marked `root` and, for `1631`, the applied or merged evidence.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or no `e`
/// root tag.
pub fn open_patch_status(event: &Event) -> Result<(PatchStatus, String), DomainError> {
    let status = match event.kind {
        1_630 => PatchStatus::Open,
        1_631 => PatchStatus::Applied,
        1_632 => PatchStatus::Closed,
        1_633 => PatchStatus::Draft,
        _ => return Err(invalid("a status is kind 1630 through 1633")),
    };
    let target = event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("e") && tag.0.get(3).is_some_and(|marker| marker == "root"))
        .and_then(|tag| tag.value())
        .filter(|value| decode_lower_hex::<32>(value, "e").is_ok())
        .ok_or_else(|| invalid("a status names its target in an e root tag"))?;
    Ok((status, target.to_string()))
}

/// The applied commits a `1631` carries: `merge-commit`, or each
/// `applied-as-commits` value.
#[must_use]
pub fn applied_commits(event: &Event) -> Vec<String> {
    let mut commits: Vec<String> = event
        .tag_values("applied-as-commits")
        .flat_map(|tag_line| tag_line.split_whitespace().map(str::to_string))
        .collect();
    if let Some(merge) = event.tag_values("merge-commit").next() {
        commits.push(merge.to_string());
    }
    commits
}

fn commit_field(event: &Event, name: &str) -> Result<String, DomainError> {
    let value = event
        .tag_values(name)
        .next()
        .ok_or_else(|| invalid("a pull request names its tip commit in a c tag"))?;
    decode_lower_hex::<20>(value, "commit").map_err(|_| invalid("a commit id is 40-char hex"))?;
    Ok(value.to_string())
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    const COMMIT: &str = "3093509d1e0bc604ff60cb9286f4cd7c781553bc";

    #[test]
    fn a_repository_announces_itself_and_its_state() {
        let owner = "ab".repeat(32);
        let repo = sign(
            REPO_KIND,
            vec![
                Tag::new(vec!["d".into(), "ngit".into()]),
                Tag::new(vec!["name".into(), "ngit".into()]),
                Tag::new(vec!["description".into(), "git over nostr".into()]),
                Tag::new(vec!["clone".into(), "https://git.example/ngit.git".into()]),
                Tag::new(vec!["relays".into(), "wss://relay.ngit.dev".into()]),
                Tag::new(vec!["maintainers".into(), owner.clone()]),
                Tag::new(vec!["r".into(), COMMIT.into(), "euc".into()]),
                Tag::new(vec!["t".into(), "git".into()]),
            ],
            "",
        );
        let opened = open_repository(&repo).unwrap();
        assert_eq!(opened.identifier, "ngit");
        assert_eq!(opened.name.as_deref(), Some("ngit"));
        assert_eq!(opened.earliest_commit.as_deref(), Some(COMMIT));
        assert_eq!(opened.maintainers, vec![owner.clone()]);

        let state = sign(
            REPO_STATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "ngit".into()]),
                Tag::new(vec!["refs/heads/main".into(), COMMIT.into()]),
                Tag::new(vec!["refs/tags/v1".into(), COMMIT.into()]),
                Tag::new(vec!["HEAD".into(), "ref: refs/heads/main".into()]),
            ],
            "",
        );
        let state = open_repository_state(&state).unwrap();
        assert_eq!(state.refs.len(), 2);
        assert_eq!(state.head.as_deref(), Some("main"));

        let repo_addr = format!("30617:{}:ngit", owner);
        let patch = sign(
            PATCH_KIND,
            vec![
                Tag::new(vec!["a".into(), repo_addr.clone()]),
                Tag::new(vec!["r".into(), COMMIT.into()]),
                Tag::new(vec!["p".into(), owner.clone()]),
                Tag::new(vec!["t".into(), "root".into()]),
            ],
            "From abc123 Mon Sep 17 00:00:00 2001\nSubject: [PATCH] fix",
        );
        assert_eq!(open_patch(&patch).unwrap(), repo_addr);
        assert_eq!(patch_markers(&patch), vec!["root"]);

        let pr = sign(
            PULL_REQUEST_KIND,
            vec![
                Tag::new(vec!["a".into(), repo_addr.clone()]),
                Tag::new(vec!["subject".into(), "add feature".into()]),
                Tag::new(vec!["c".into(), COMMIT.into()]),
                Tag::new(vec!["clone".into(), "https://git.example/fork.git".into()]),
            ],
            "please review",
        );
        let pr = open_pull_request(&pr).unwrap();
        assert_eq!(pr.tip, COMMIT);
        assert_eq!(pr.subject.as_deref(), Some("add feature"));

        let issue = sign(
            ISSUE_KIND,
            vec![
                Tag::new(vec!["a".into(), repo_addr.clone()]),
                Tag::new(vec!["subject".into(), "it broke".into()]),
            ],
            "steps to reproduce",
        );
        assert_eq!(open_issue(&issue).unwrap(), repo_addr);

        let merged = sign(
            1_631,
            vec![
                Tag::new(vec!["e".into(), "cd".repeat(32), "".into(), "root".into()]),
                Tag::new(vec!["merge-commit".into(), COMMIT.into()]),
            ],
            "merged",
        );
        let (status, target) = open_patch_status(&merged).unwrap();
        assert_eq!(status, PatchStatus::Applied);
        assert_eq!(target, "cd".repeat(32));
        assert_eq!(applied_commits(&merged), vec![COMMIT]);
    }

    #[test]
    fn malformed_git_events_are_refused() {
        assert!(open_repository(&sign(REPO_KIND, Vec::new(), "")).is_err());
        let bad_euc = sign(
            REPO_KIND,
            vec![
                Tag::new(vec!["d".into(), "r".into()]),
                Tag::new(vec!["r".into(), "nothex".into(), "euc".into()]),
            ],
            "",
        );
        assert!(open_repository(&bad_euc).is_err());
        let bad_ref = sign(
            REPO_STATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "r".into()]),
                Tag::new(vec!["refs/heads/main".into(), "short".into()]),
            ],
            "",
        );
        assert!(open_repository_state(&bad_ref).is_err());
        assert!(open_patch(&sign(PATCH_KIND, Vec::new(), "patch")).is_err());
        let no_clone = sign(
            PULL_REQUEST_KIND,
            vec![
                Tag::new(vec!["a".into(), format!("30617:{}:r", "ab".repeat(32))]),
                Tag::new(vec!["c".into(), COMMIT.into()]),
            ],
            "pr",
        );
        assert!(open_pull_request(&no_clone).is_err());
        assert!(open_patch_status(&sign(1_631, Vec::new(), "")).is_err());
    }
}
