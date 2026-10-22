//! The GitHub bridge (#12453): Kiln patches a maintainer opened on Nostr
//! become draft pull requests, and what happens on GitHub comes back.
//!
//! Each round the bridge:
//!
//! 1. reads `1630` (open) status events from the repository's maintainers
//!    and its listed openers (such as the Grid Foreman's key);
//! 2. for each newly opened patch, reviews it as `kiln-git apply` does (a
//!    passing check from a listed runner of exactly this text at its base,
//!    no secrets, only crate files), applies it on its base, pushes the
//!    branch `nostr/<event id>`, opens a **draft** pull request that links
//!    the event and credits the author, and comments the link on the patch;
//! 3. mirrors new Nostr comments on the patch to the pull request, and new
//!    pull-request comments back as NIP-22 comments it signs;
//! 4. when GitHub merges the pull request, publishes `1631` with the merge
//!    commit; when it is closed unmerged, `1632`.
//!
//! It never merges, never pushes to `main`, and never acts on a patch
//! nobody opened. Its GitHub token is read from a file for each call and
//! passed to git through the environment, never in a URL or a log line.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use kiln::identity::{Author, npub, short_npub};
use kiln::repo::Trust;
use nostr::domain::{Event, Tag};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::keys::Signer;
use crate::relay;
use crate::review::{self, Review};

/// The marker on every comment the bridge writes to GitHub.
pub const MARK: &str = "<!-- kiln-bridge -->";
/// The longest comment mirrored either way.
const MAX_COMMENT: usize = 4_000;

/// How the bridge runs.
pub struct Config {
    pub relay: String,
    /// The repository's announcement address, `30617:<key>:openagents`.
    pub address: String,
    /// `OWNER/NAME` on GitHub.
    pub github: String,
    /// GitHub's API, `https://api.github.com`.
    pub api: String,
    /// A file holding a fine-grained token for `github` with contents and
    /// pull requests read and write, and nothing else.
    pub token_file: PathBuf,
    /// The bridge's own clone, whose `origin` is the GitHub repository.
    pub checkout: PathBuf,
    /// Where the bridge keeps `bridge.json` and its worktrees.
    pub state: PathBuf,
    /// Keys besides the maintainers whose `1630` opens a pull request, one
    /// a line (the admission-list format); read every round.
    pub openers: Option<PathBuf>,
    /// The bridge's own key: signs its comments and `1631`/`1632`. List it
    /// as a maintainer so readers count its merge status.
    pub key: Signer,
}

/// One patch the bridge has acted on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracked {
    pub opener: String,
    #[serde(default)]
    pub number: u64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub head: String,
    /// `merged` or `closed` once GitHub says so.
    #[serde(default)]
    pub done: Option<String>,
    /// Why the bridge didn't open it, when it didn't.
    #[serde(default)]
    pub refused: Option<String>,
    /// When it was refused; a newer open retries it.
    #[serde(default)]
    pub refused_at: u64,
    /// `1617` for a patch, `1618` for a pull request.
    #[serde(default)]
    pub kind: u16,
    #[serde(default)]
    pub mirrored_nostr: BTreeSet<String>,
    #[serde(default)]
    pub mirrored_github: BTreeSet<u64>,
}

/// What the bridge remembers between rounds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Status events before this time were already read.
    pub since: u64,
    pub patches: BTreeMap<String, Tracked>,
}

impl State {
    /// Read `DIR/bridge.json`, or start empty.
    #[must_use]
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join("bridge.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Write `DIR/bridge.json` atomically.
    ///
    /// # Errors
    ///
    /// Why it couldn't be written.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let tmp = dir.join("bridge.json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dir.join("bridge.json")).map_err(|e| e.to_string())
    }
}

/// The patches newly opened by a counted opener: `(patch id, opener)`.
#[must_use]
pub fn openings(
    state: &State,
    trust: &Trust,
    openers: &BTreeSet<String>,
    statuses: &[Event],
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut sorted: Vec<&Event> = statuses.iter().collect();
    sorted.sort_by_key(|e| e.created_at);
    for event in sorted {
        if event.kind != 1_630 || event.validate_crypto().is_err() {
            continue;
        }
        if !(trust.maintainers.contains(&event.pubkey) || openers.contains(&event.pubkey)) {
            continue;
        }
        let Ok((_, target)) = nostr::domain::open_patch_status(event) else {
            continue;
        };
        let retry = state
            .patches
            .get(&target)
            .is_none_or(|t| t.refused.is_some() && event.created_at > t.refused_at);
        if !retry || out.iter().any(|(t, _)| *t == target) {
            continue;
        }
        out.push((target, event.pubkey.clone()));
    }
    out
}

/// The branch a patch is pushed to.
#[must_use]
pub fn branch(patch_id: &str) -> String {
    format!("nostr/{patch_id}")
}

/// The pull request's title and body.
#[must_use]
pub fn pull_text(review: &Review, opener: &str, relay: &str) -> (String, String) {
    let subject = kiln::patch::subject(&review.patch.content);
    let title = if subject.is_empty() {
        format!("Kiln patch {}", &review.patch.id[..12])
    } else {
        subject
    };
    let author = match &review.author {
        Author::Agent { agent, owner, .. } => format!(
            "an agent (`{}`) authorized by `{}`",
            npub(agent),
            npub(owner)
        ),
        Author::Maintainer(key) => format!("`{}` (maintainer)", npub(key)),
        Author::Unverified(key) => format!("`{}`, an unverified author", npub(key)),
    };
    let check = review.check.as_ref().map_or_else(
        || "No listed runner has checked it.".to_string(),
        |c| {
            format!(
                "Runner `{}` {}: {}",
                short_npub(&c.runner),
                if c.passed { "passed it" } else { "failed it" },
                c.summary
            )
        },
    );
    let body = format!(
        "{MARK}\nA Kiln patch from Nostr, opened for review by `{}`.\n\n\
         - **Author:** {author}\n\
         - **Event:** `{}` on {relay}\n\
         - **Base:** `{}`\n\
         - **Crates:** {}\n\
         - **Check:** {check}\n\n\
         This is a draft. A maintainer reviews and merges it on GitHub; the bridge \
         never merges. When it merges, the bridge marks the patch applied on Nostr.",
        npub(opener),
        review.patch.id,
        review.base,
        review.crates.join(", "),
    );
    (title, body)
}

/// A pull request as the bridge reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pull {
    pub open: bool,
    pub merged: bool,
    pub merge_commit: Option<String>,
    pub head: String,
}

/// One pull-request comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitComment {
    pub id: u64,
    pub login: String,
    pub body: String,
}

/// GitHub's REST API, as the bridge uses it.
pub struct GitHub {
    pub api: String,
    pub repo: String,
    pub token_file: PathBuf,
    client: reqwest::Client,
}

impl GitHub {
    #[must_use]
    pub fn new(api: &str, repo: &str, token_file: &Path) -> Self {
        Self {
            api: api.trim_end_matches('/').to_string(),
            repo: repo.to_string(),
            token_file: token_file.to_path_buf(),
            client: reqwest::Client::new(),
        }
    }

    fn token(&self) -> Result<String, String> {
        let token = std::fs::read_to_string(&self.token_file)
            .map_err(|e| format!("the GitHub token file: {e}"))?;
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("the GitHub token file is empty".into());
        }
        Ok(token)
    }

    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = self
            .client
            .request(method, format!("{}/repos/{}{path}", self.api, self.repo))
            .bearer_auth(self.token()?)
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28")
            .header("user-agent", "kiln-git-bridge");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|e| format!("GitHub: {e}"))?;
        let status = response.status();
        let value: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            Ok(value)
        } else {
            Err(format!(
                "GitHub answered {status}: {}",
                value["message"].as_str().unwrap_or("no message")
            ))
        }
    }

    /// Open a draft pull request from `head` into `main`.
    ///
    /// # Errors
    ///
    /// GitHub's refusal.
    pub async fn open_draft(
        &self,
        title: &str,
        head: &str,
        body: &str,
    ) -> Result<(u64, String), String> {
        let made = self
            .call(
                reqwest::Method::POST,
                "/pulls",
                Some(json!({"title": title, "head": head, "base": "main", "body": body, "draft": true})),
            )
            .await?;
        Ok((
            made["number"].as_u64().ok_or("GitHub returned no number")?,
            made["html_url"].as_str().unwrap_or_default().to_string(),
        ))
    }

    /// Recover a PR created before the local result was saved. The branch
    /// is derived from the exact patch event, not a caller-supplied URL.
    pub async fn for_patch(&self, patch: &Event) -> Result<Option<Tracked>, String> {
        let owner = self.repo.split('/').next().ok_or("repository owner")?;
        let branch = branch(&patch.id);
        let query = format!(
            "/pulls?state=all&base=main&head={owner}%3Anostr%2F{}&per_page=100",
            patch.id
        );
        let value = self.call(reqwest::Method::GET, &query, None).await?;
        let rows = value
            .as_array()
            .ok_or("GitHub returned no pull request list")?;
        if rows.len() > 1 {
            return Err("multiple pull requests use the patch branch".into());
        }
        let Some(pull) = rows.first() else {
            return Ok(None);
        };
        if pull["head"]["ref"] != branch
            || pull["head"]["repo"]["full_name"] != self.repo
            || !pull["body"]
                .as_str()
                .is_some_and(|s| s.contains(&patch.id) && s.contains(MARK))
        {
            return Err("the existing patch pull request does not match".into());
        }
        Ok(Some(Tracked {
            number: pull["number"].as_u64().ok_or("pull request number")?,
            url: pull["html_url"].as_str().ok_or("pull request URL")?.into(),
            branch,
            head: pull["head"]["sha"]
                .as_str()
                .ok_or("pull request head")?
                .into(),
            ..Tracked::default()
        }))
    }

    /// Read pull request `number`.
    ///
    /// # Errors
    ///
    /// GitHub's refusal.
    pub async fn pull(&self, number: u64) -> Result<Pull, String> {
        let pull = self
            .call(reqwest::Method::GET, &format!("/pulls/{number}"), None)
            .await?;
        Ok(Pull {
            open: pull["state"] == "open",
            merged: pull["merged"] == true,
            merge_commit: pull["merge_commit_sha"].as_str().map(str::to_string),
            head: pull["head"]["sha"].as_str().unwrap_or_default().to_string(),
        })
    }

    /// The comments on pull request `number`.
    ///
    /// # Errors
    ///
    /// GitHub's refusal.
    pub async fn comments(&self, number: u64) -> Result<Vec<GitComment>, String> {
        let list = self
            .call(
                reqwest::Method::GET,
                &format!("/issues/{number}/comments?per_page=100"),
                None,
            )
            .await?;
        Ok(list
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| {
                        Some(GitComment {
                            id: c["id"].as_u64()?,
                            login: c["user"]["login"].as_str().unwrap_or("someone").to_string(),
                            body: c["body"].as_str().unwrap_or_default().to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Comment on pull request `number`.
    ///
    /// # Errors
    ///
    /// GitHub's refusal.
    pub async fn comment(&self, number: u64, body: &str) -> Result<(), String> {
        self.call(
            reqwest::Method::POST,
            &format!("/issues/{number}/comments"),
            Some(json!({"body": body})),
        )
        .await
        .map(|_| ())
    }

    /// Push `commit` from `dir` to `branch` on GitHub, with the token in the
    /// environment only.
    ///
    /// # Errors
    ///
    /// Git's refusal.
    pub fn push(&self, dir: &Path, commit: &str, branch: &str) -> Result<(), String> {
        let token = self.token()?;
        let basic = base64(format!("x-access-token:{token}").as_bytes());
        let url = format!("https://github.com/{}.git", self.repo);
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "push",
                "--quiet",
                "--force",
                &url,
                &format!("{commit}:refs/heads/{branch}"),
            ])
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader")
            .env(
                "GIT_CONFIG_VALUE_0",
                format!("AUTHORIZATION: basic {basic}"),
            )
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git push to {branch}: {}",
                String::from_utf8_lossy(&out.stderr)
                    .replace(&token, "[token]")
                    .trim()
            ))
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn clip(text: &str) -> String {
    if text.len() <= MAX_COMMENT {
        return text.to_string();
    }
    let mut end = MAX_COMMENT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// The NIP-22 tags of a comment on `patch`.
#[must_use]
pub fn comment_tags(patch: &Event, relay: &str) -> Vec<Tag> {
    vec![
        Tag::new(vec![
            "E".into(),
            patch.id.clone(),
            relay.into(),
            patch.pubkey.clone(),
        ]),
        Tag::new(vec!["K".into(), patch.kind.to_string()]),
        Tag::new(vec!["P".into(), patch.pubkey.clone()]),
        Tag::new(vec![
            "e".into(),
            patch.id.clone(),
            relay.into(),
            patch.pubkey.clone(),
        ]),
        Tag::new(vec!["k".into(), patch.kind.to_string()]),
        Tag::new(vec!["p".into(), patch.pubkey.clone()]),
    ]
}

/// A Nostr comment as it reads on GitHub.
#[must_use]
pub fn to_github(comment: &Event, trust: &Trust) -> String {
    let who = if trust.maintainers.contains(&comment.pubkey) {
        format!("`{}` (maintainer)", short_npub(&comment.pubkey))
    } else if trust.runners.contains(&comment.pubkey) {
        format!("`{}` (runner)", short_npub(&comment.pubkey))
    } else {
        format!("`{}`", short_npub(&comment.pubkey))
    };
    let quoted: String = clip(&comment.content)
        .lines()
        .map(|l| format!("> {l}\n"))
        .collect();
    format!("{MARK}\n{who} on Nostr:\n\n{quoted}")
}

/// The Nostr comments on `patch` the bridge should copy to GitHub.
#[must_use]
pub fn nostr_to_mirror<'a>(
    patch: &Event,
    replies: &'a [Event],
    tracked: &Tracked,
    bridge: &str,
) -> Vec<&'a Event> {
    let mut out: Vec<&Event> = replies
        .iter()
        .filter(|r| r.kind == 1_111 && r.pubkey != bridge && r.validate_crypto().is_ok())
        .filter(|r| r.tag_values("e").any(|e| e == patch.id))
        .filter(|r| !tracked.mirrored_nostr.contains(&r.id))
        .collect();
    out.sort_by_key(|r| r.created_at);
    out
}

/// The GitHub comments the bridge should copy to Nostr.
#[must_use]
pub fn github_to_mirror<'a>(comments: &'a [GitComment], tracked: &Tracked) -> Vec<&'a GitComment> {
    comments
        .iter()
        .filter(|c| !c.body.contains(MARK) && !tracked.mirrored_github.contains(&c.id))
        .collect()
}

fn opener_keys(config: &Config) -> BTreeSet<String> {
    config
        .openers
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| kiln::admission::Roster::parse(&t).ok())
        .map(|r| r.keys.into_keys().collect())
        .unwrap_or_default()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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

fn verify_pull_tree(checkout: &Path, head: &str, expected: &str) -> Result<(), String> {
    if head.len() != 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the pull request has no valid head commit".into());
    }
    let commit = format!("{head}^{{commit}}");
    if git(checkout, &["cat-file", "-e", &commit]).is_err() {
        git(checkout, &["fetch", "--quiet", "origin", head])?;
    }
    let tree = git(checkout, &["rev-parse", &format!("{head}^{{tree}}")])?;
    if tree != expected {
        return Err("the pull request contents differ from the checked patch".into());
    }
    Ok(())
}

async fn checked_draft(
    config: &Config,
    hub: &GitHub,
    review: &Review,
    opener: &str,
    applied: &crate::apply::Applied,
    push: impl FnOnce(&crate::apply::Applied, &str) -> Result<(), String>,
) -> Result<(Tracked, bool), String> {
    // Reapplying can change commit metadata; the checked contents are the Git tree.
    let expected = git(
        &applied.dir,
        &["rev-parse", &format!("{}^{{tree}}", applied.commit)],
    )?;
    if let Some(mut existing) = hub.for_patch(&review.patch).await? {
        verify_pull_tree(&config.checkout, &existing.head, &expected)?;
        existing.opener = opener.into();
        return Ok((existing, false));
    }
    let branch = branch(&review.patch.id);
    push(applied, &branch)?;
    let (title, body) = pull_text(review, opener, &config.relay);
    let (number, url) = hub.open_draft(&title, &branch, &body).await?;
    let pull = hub.pull(number).await?;
    verify_pull_tree(&config.checkout, &pull.head, &expected)?;
    Ok((
        Tracked {
            opener: opener.to_string(),
            number,
            url,
            branch,
            head: pull.head,
            ..Tracked::default()
        },
        true,
    ))
}

/// Open a draft pull request for `review`, opened by `opener`. The Grid
/// Foreman can call this directly for a patch it opened.
///
/// # Errors
///
/// Why the pull request could not be opened or verified.
pub async fn open(
    config: &Config,
    hub: &GitHub,
    review: &Review,
    opener: &str,
) -> Result<Tracked, String> {
    // A maintainer chose to open it, so an unverified author is allowed;
    // the pull request says so.
    review.ready(false, true)?;
    let worktree = config.state.join("worktrees").join(&review.patch.id[..12]);
    if worktree.exists() {
        let _ = git(
            &config.checkout,
            &["worktree", "remove", "--force", &worktree.to_string_lossy()],
        );
        let _ = std::fs::remove_dir_all(&worktree);
    }
    let _ = git(
        &config.checkout,
        &["branch", "-D", &crate::apply::branch_name(review)],
    );
    std::fs::create_dir_all(config.state.join("worktrees")).map_err(|e| e.to_string())?;
    let applied = crate::apply::apply(review, &config.checkout, &worktree, None, &config.relay)?;
    let (tracked, created) =
        checked_draft(config, hub, review, opener, &applied, |applied, branch| {
            hub.push(&applied.dir, &applied.commit, branch)
        })
        .await?;
    if created {
        let note = config.key.key.sign(
            1_111,
            comment_tags(&review.patch, &config.relay),
            format!(
                "Opened draft pull request #{} on GitHub: {}",
                tracked.number, tracked.url
            ),
            now(),
        );
        relay::publish(&config.relay, &config.key.secret, &note).await?;
    }
    let _ = git(
        &config.checkout,
        &["worktree", "remove", "--force", &worktree.to_string_lossy()],
    );
    Ok(tracked)
}

/// The draft pull request's title and body for a Nostr pull request.
#[must_use]
pub fn pull_request_text(
    read: &crate::pull::PullReview,
    opener: &str,
    relay: &str,
) -> (String, String) {
    let author = match &read.author {
        Author::Agent { agent, owner, .. } => format!(
            "an agent (`{}`) authorized by `{}`",
            npub(agent),
            npub(owner)
        ),
        Author::Maintainer(key) => format!("`{}` (maintainer)", npub(key)),
        Author::Unverified(key) => format!("`{}`, an unverified author", npub(key)),
    };
    let body = format!(
        "{MARK}\nA pull request from Nostr, opened for review by `{}`.\n\n\
         - **Author:** {author}\n\
         - **Event:** `{}` on {relay}\n\
         - **Tip:** `{}` from {}\n\
         - **Merge base:** `{}`\n\
         - **Check:** no runner checks pull requests yet; the land queue's tests are the check.\n\n\
         {}\n\n\
         This is a draft. A maintainer reviews and merges it on GitHub; the bridge \
         never merges. New tips the author publishes on Nostr update this branch.",
        npub(opener),
        read.event.id,
        read.tip,
        read.clone,
        read.merge_base,
        clip(&read.event.content),
    );
    (read.subject.clone(), body)
}

/// Open a draft pull request for a Nostr pull request: fetch its tip over
/// https, check it changes only crate sources, and push it.
///
/// # Errors
///
/// Why the pull request wasn't opened.
pub async fn open_pull(
    config: &Config,
    hub: &GitHub,
    read: &crate::pull::PullReview,
    opener: &str,
) -> Result<Tracked, String> {
    crate::pull::bring(read, &config.checkout)?;
    let branch = branch(&read.event.id);
    hub.push(&config.checkout, &read.tip, &branch)?;
    let (title, body) = pull_request_text(read, opener, &config.relay);
    let (number, url) = hub.open_draft(&title, &branch, &body).await?;
    let note = config.key.key.sign(
        1_111,
        comment_tags(&read.event, &config.relay),
        format!("Opened draft pull request #{number} on GitHub: {url}"),
        now(),
    );
    relay::publish(&config.relay, &config.key.secret, &note).await?;
    Ok(Tracked {
        opener: opener.to_string(),
        number,
        url,
        branch,
        head: read.tip.clone(),
        kind: kiln::patch::PULL_REQUEST_KIND,
        ..Tracked::default()
    })
}

/// One round: open newly opened patches, mirror comments, and report
/// merges and closes. Returns a line per thing it did.
///
/// # Errors
///
/// Why the round couldn't start (the relay or the announcement); a failure
/// on one patch is recorded on it and the round goes on.
pub async fn round(
    config: &Config,
    hub: &GitHub,
    state: &mut State,
) -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    let trust = review::trust(&config.relay, &config.address).await?;
    let openers = opener_keys(config);
    let authors: Vec<String> = trust.maintainers.iter().chain(&openers).cloned().collect();
    let started = now();
    let statuses = relay::query(
        &config.relay,
        &review::reader(),
        json!({"kinds": [1630], "authors": authors, "since": state.since.saturating_sub(600), "limit": 200}),
    )
    .await?;
    for (id, opener) in openings(state, &trust, &openers, &statuses) {
        let kind = relay::query(
            &config.relay,
            &review::reader(),
            json!({"ids": [id], "limit": 1}),
        )
        .await?
        .first()
        .map_or(0, |e| e.kind);
        let outcome = if kind == kiln::patch::PULL_REQUEST_KIND {
            match crate::pull::fetch(&config.relay, &id, &config.address).await {
                Ok(read) => open_pull(config, hub, &read, &opener).await,
                Err(e) => Err(e),
            }
        } else {
            match review::fetch(&config.relay, &id, Some(&config.address)).await {
                Ok(review) => open(config, hub, &review, &opener).await,
                Err(e) => Err(e),
            }
        };
        match outcome {
            Ok(tracked) => {
                said.push(format!("opened #{} for {}", tracked.number, &id[..12]));
                state.patches.insert(id, tracked);
            }
            Err(e) => {
                said.push(format!("not opened {}: {e}", &id[..12]));
                state.patches.insert(
                    id,
                    Tracked {
                        opener,
                        refused: Some(e),
                        refused_at: now(),
                        ..Tracked::default()
                    },
                );
            }
        }
        state.save(&config.state)?;
    }
    state.since = started;
    let live: Vec<String> = state
        .patches
        .iter()
        .filter(|(_, t)| t.number > 0 && t.done.is_none())
        .map(|(id, _)| id.clone())
        .collect();
    for id in live {
        if let Err(e) = follow(config, hub, &trust, state, &id, &mut said).await {
            said.push(format!("{}: {e}", &id[..12]));
        }
        state.save(&config.state)?;
    }
    state.save(&config.state)?;
    Ok(said)
}

async fn follow(
    config: &Config,
    hub: &GitHub,
    trust: &Trust,
    state: &mut State,
    id: &str,
    said: &mut Vec<String>,
) -> Result<(), String> {
    let Some(tracked) = state.patches.get_mut(id) else {
        return Ok(());
    };
    let reader = review::reader();
    let patch = relay::query(&config.relay, &reader, json!({"ids": [id], "limit": 1}))
        .await?
        .into_iter()
        .next()
        .ok_or("the relay no longer has the patch")?;
    if patch.kind == kiln::patch::PULL_REQUEST_KIND {
        // The author moved the tip with a 1619: bring it and push it.
        let read = crate::pull::fetch(&config.relay, id, &config.address).await?;
        if read.tip != tracked.head {
            crate::pull::bring(&read, &config.checkout)?;
            hub.push(&config.checkout, &read.tip, &tracked.branch)?;
            hub.comment(
                tracked.number,
                &format!(
                    "{MARK}\nThe author moved this pull request to `{}` on Nostr.",
                    read.tip
                ),
            )
            .await?;
            tracked.head = read.tip;
            said.push(format!("updated #{} to a new tip", tracked.number));
        }
    }
    let replies = relay::query(
        &config.relay,
        &reader,
        json!({"kinds": [1111], "#e": [id], "limit": 200}),
    )
    .await?;
    for comment in nostr_to_mirror(&patch, &replies, tracked, config.key.key.pubkey()) {
        hub.comment(tracked.number, &to_github(comment, trust))
            .await?;
        tracked.mirrored_nostr.insert(comment.id.clone());
        said.push(format!("copied a Nostr comment to #{}", tracked.number));
    }
    let comments = hub.comments(tracked.number).await?;
    for comment in github_to_mirror(&comments, tracked) {
        let event = config.key.key.sign(
            1_111,
            comment_tags(&patch, &config.relay),
            format!("{} on GitHub: {}", comment.login, clip(&comment.body)),
            now(),
        );
        relay::publish(&config.relay, &config.key.secret, &event).await?;
        tracked.mirrored_github.insert(comment.id);
        said.push(format!("copied a GitHub comment from #{}", tracked.number));
    }
    let pull = hub.pull(tracked.number).await?;
    if pull.merged || !pull.open {
        let (kind, word) = if pull.merged {
            (1_631, "merged")
        } else {
            (1_632, "closed")
        };
        let applied = if pull.merged {
            vec![pull.head.clone()]
        } else {
            Vec::new()
        };
        let tags = kiln::patch::status_tags(
            id,
            &patch.pubkey,
            &config.address,
            &config.relay,
            &applied,
            pull.merge_commit.as_deref().filter(|_| pull.merged),
        );
        let event = config.key.key.sign(kind, tags, String::new(), now());
        relay::publish(&config.relay, &config.key.secret, &event).await?;
        tracked.done = Some(word.to_string());
        said.push(format!(
            "#{} {word}; marked the patch {word}",
            tracked.number
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{assess, tests as fixture};
    use kiln::cj::Key;

    struct DeliveryFixture {
        root: PathBuf,
        config: Config,
        review: Review,
        applied: crate::apply::Applied,
    }

    impl DeliveryFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "kiln-checked-pr-{}-{}",
                std::process::id(),
                secp256k1::rand::random::<u64>()
            ));
            let checkout = root.join("repo");
            let origin = root.join("origin.git");
            std::fs::create_dir_all(checkout.join("crates/demo/src")).unwrap();
            git(&checkout, &["init", "--quiet", "-b", "main"]).unwrap();
            git(&checkout, &["config", "user.name", "Fixture"]).unwrap();
            git(&checkout, &["config", "user.email", "fixture@example.com"]).unwrap();
            git(&checkout, &["config", "commit.gpgsign", "false"]).unwrap();
            std::fs::write(checkout.join("crates/demo/src/lib.rs"), fixture::BASE_FILE).unwrap();
            git(&checkout, &["add", "."]).unwrap();
            git(&checkout, &["commit", "--quiet", "-m", "base"]).unwrap();
            let base = git(&checkout, &["rev-parse", "HEAD"]).unwrap();
            git(
                &checkout,
                &["init", "--quiet", "--bare", origin.to_str().unwrap()],
            )
            .unwrap();
            git(
                &checkout,
                &["remote", "add", "origin", origin.to_str().unwrap()],
            )
            .unwrap();
            git(&checkout, &["push", "--quiet", "origin", "main"]).unwrap();
            let w = fixture::world();
            let patch = fixture::patch(&w, &base, true);
            let check = fixture::check(&w, &patch, &w.runner, true, &base);
            let review = assess(patch, w.trust.clone(), &[check]).unwrap();
            review.ready(false, false).unwrap();
            let applied =
                crate::apply::apply(&review, &checkout, &root.join("applied"), None, "wss://r")
                    .unwrap();
            std::fs::write(root.join("token"), "fixture").unwrap();
            let config = Config {
                relay: "wss://r".into(),
                address: w.trust.address,
                github: "OpenAgentsInc/openagents".into(),
                api: "unused".into(),
                token_file: root.join("token"),
                checkout,
                state: root.join("state"),
                openers: None,
                key: Signer {
                    key: w.repo,
                    secret: secp256k1::SecretKey::from_byte_array([5; 32]).unwrap(),
                },
            };
            Self {
                root,
                config,
                review,
                applied,
            }
        }

        fn push_checked(&self) {
            git(
                &self.applied.dir,
                &[
                    "push",
                    "--quiet",
                    "--force",
                    "origin",
                    &format!(
                        "{}:refs/heads/{}",
                        self.applied.commit,
                        branch(&self.review.patch.id)
                    ),
                ],
            )
            .unwrap();
        }

        fn change_branch(&self, force: bool) -> String {
            let writer = self.root.join("writer");
            git(
                &self.config.checkout,
                &[
                    "clone",
                    "--quiet",
                    self.root.join("origin.git").to_str().unwrap(),
                    writer.to_str().unwrap(),
                ],
            )
            .unwrap();
            let from = if force {
                &self.review.base
            } else {
                &self.applied.commit
            };
            git(&writer, &["checkout", "--quiet", "--detach", from]).unwrap();
            git(&writer, &["config", "user.name", "Fixture"]).unwrap();
            git(&writer, &["config", "user.email", "fixture@example.com"]).unwrap();
            git(&writer, &["config", "commit.gpgsign", "false"]).unwrap();
            std::fs::write(
                writer.join("crates/demo/src/lib.rs"),
                "pub fn a() -> u32 { 3 }\n",
            )
            .unwrap();
            git(&writer, &["commit", "--quiet", "-am", "unchecked contents"]).unwrap();
            self.push_writer();
            git(&writer, &["rev-parse", "HEAD"]).unwrap()
        }

        fn push_writer(&self) {
            git(
                &self.root.join("writer"),
                &[
                    "push",
                    "--quiet",
                    "--force",
                    "origin",
                    &format!("HEAD:refs/heads/{}", branch(&self.review.patch.id)),
                ],
            )
            .unwrap();
        }

        fn remote_head(&self) -> String {
            git(
                &self.root.join("origin.git"),
                &[
                    "rev-parse",
                    &format!("refs/heads/{}", branch(&self.review.patch.id)),
                ],
            )
            .unwrap()
        }

        fn api(
            &self,
            replies: Vec<(&'static str, Value)>,
        ) -> (GitHub, std::thread::JoinHandle<()>) {
            use std::io::{Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                for (prefix, reply) in replies {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    loop {
                        let mut buf = [0; 4096];
                        let n = stream.read(&mut buf).unwrap();
                        assert_ne!(n, 0);
                        bytes.extend_from_slice(&buf[..n]);
                        let request = String::from_utf8_lossy(&bytes);
                        if let Some(end) = request.find("\r\n\r\n") {
                            let body_len = request[..end]
                                .lines()
                                .find_map(|l| {
                                    l.to_ascii_lowercase()
                                        .strip_prefix("content-length: ")
                                        .and_then(|n| n.parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + body_len {
                                break;
                            }
                        }
                    }
                    assert!(String::from_utf8_lossy(&bytes).starts_with(prefix));
                    let body = reply.to_string();
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
            });
            (
                GitHub::new(
                    &format!("http://{addr}"),
                    "OpenAgentsInc/openagents",
                    &self.config.token_file,
                ),
                server,
            )
        }

        fn recovered(&self, head: &str) -> Value {
            json!([{"number":42,"html_url":"https://example.test/pull/42","head":{"ref":branch(&self.review.patch.id),"sha":head,"repo":{"full_name":"OpenAgentsInc/openagents"}},"body":format!("{MARK} patch {}",self.review.patch.id)}])
        }
    }

    impl Drop for DeliveryFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn recovered_pr_contents_must_match_the_checked_patch() {
        for changed in [None, Some(false), Some(true)] {
            let f = DeliveryFixture::new();
            f.push_checked();
            if let Some(force) = changed {
                f.change_branch(force);
            }
            let head = f.remote_head();
            let (hub, server) = f.api(vec![(
                "GET /repos/OpenAgentsInc/openagents/pulls?",
                f.recovered(&head),
            )]);
            let result =
                checked_draft(&f.config, &hub, &f.review, "fixture", &f.applied, |_, _| {
                    panic!("recovery must not push")
                })
                .await;
            if changed.is_some() {
                assert!(
                    result
                        .unwrap_err()
                        .contains("contents differ from the checked patch")
                );
            } else {
                let (tracked, created) = result.unwrap();
                assert!(!created);
                assert_eq!(tracked.head, f.applied.commit);
            }
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn recovered_pr_allows_the_checked_tree_with_different_commit_metadata() {
        let f = DeliveryFixture::new();
        f.push_checked();
        git(
            &f.applied.dir,
            &[
                "commit",
                "--quiet",
                "--amend",
                "-m",
                "same checked contents",
            ],
        )
        .unwrap();
        let head = git(&f.applied.dir, &["rev-parse", "HEAD"]).unwrap();
        assert_ne!(head, f.applied.commit);
        git(
            &f.applied.dir,
            &[
                "push",
                "--quiet",
                "--force",
                "origin",
                &format!("HEAD:refs/heads/{}", branch(&f.review.patch.id)),
            ],
        )
        .unwrap();
        let (hub, server) = f.api(vec![(
            "GET /repos/OpenAgentsInc/openagents/pulls?",
            f.recovered(&f.remote_head()),
        )]);
        let (tracked, created) =
            checked_draft(&f.config, &hub, &f.review, "fixture", &f.applied, |_, _| {
                panic!("recovery must not push")
            })
            .await
            .unwrap();
        assert!(!created);
        assert_eq!(tracked.head, head);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn new_pr_contents_are_checked_after_creation() {
        for changed in [false, true] {
            let f = DeliveryFixture::new();
            f.push_checked();
            let head = if changed {
                f.change_branch(false)
            } else {
                f.applied.commit.clone()
            };
            let (hub, server) = f.api(vec![
                ("GET /repos/OpenAgentsInc/openagents/pulls?", json!([])),
                (
                    "POST /repos/OpenAgentsInc/openagents/pulls ",
                    json!({"number":42,"html_url":"https://example.test/pull/42"}),
                ),
                (
                    "GET /repos/OpenAgentsInc/openagents/pulls/42 ",
                    json!({"state":"open","head":{"sha":head}}),
                ),
            ]);
            let result =
                checked_draft(&f.config, &hub, &f.review, "fixture", &f.applied, |_, _| {
                    f.push_checked();
                    if changed {
                        f.push_writer();
                    }
                    assert_eq!(f.remote_head(), head);
                    Ok(())
                })
                .await;
            if changed {
                assert!(
                    result
                        .unwrap_err()
                        .contains("contents differ from the checked patch")
                );
            } else {
                let (tracked, created) = result.unwrap();
                assert!(created);
                assert_eq!(tracked.head, f.remote_head());
            }
            server.join().unwrap();
        }
    }

    #[test]
    fn only_a_counted_opener_opens_and_only_once() {
        let w = fixture::world();
        let base = "a".repeat(40);
        let p = fixture::patch(&w, &base, true);
        let tags =
            kiln::patch::status_tags(&p.id, &p.pubkey, &w.trust.address, "wss://r", &[], None);
        let stranger = Key::from_bytes([12; 32]).unwrap();
        let foreman = Key::from_bytes([13; 32]).unwrap();
        let by_stranger = stranger.sign(1_630, tags.clone(), String::new(), 1);
        let by_repo = w.repo.sign(1_630, tags.clone(), String::new(), 2);
        let by_foreman = foreman.sign(1_630, tags.clone(), String::new(), 3);
        let closed = w.repo.sign(1_632, tags, String::new(), 4);
        let mut state = State::default();
        let none = BTreeSet::new();
        assert!(
            openings(
                &state,
                &w.trust,
                &none,
                &[by_stranger.clone(), closed.clone()]
            )
            .is_empty()
        );
        let found = openings(
            &state,
            &w.trust,
            &none,
            &[by_foreman.clone(), by_repo.clone()],
        );
        assert_eq!(found, vec![(p.id.clone(), w.repo.pubkey().to_string())]);
        let listed = BTreeSet::from([foreman.pubkey().to_string()]);
        let found = openings(&state, &w.trust, &listed, &[by_foreman]);
        assert_eq!(found, vec![(p.id.clone(), foreman.pubkey().to_string())]);
        state.patches.insert(
            p.id.clone(),
            Tracked {
                number: 7,
                ..Tracked::default()
            },
        );
        assert!(openings(&state, &w.trust, &none, &[by_repo.clone()]).is_empty());
        state.patches.insert(
            p.id.clone(),
            Tracked {
                refused: Some("no check yet".into()),
                refused_at: 1,
                ..Tracked::default()
            },
        );
        assert_eq!(
            openings(&state, &w.trust, &none, &[by_repo]).len(),
            1,
            "a newer open retries"
        );
    }

    #[test]
    fn a_pull_request_credits_the_author_and_says_it_is_a_draft() {
        let w = fixture::world();
        let base = "a".repeat(40);
        let p = fixture::patch(&w, &base, true);
        let check = fixture::check(&w, &p, &w.runner, true, &base);
        let review = assess(p, w.trust.clone(), &[check]).unwrap();
        let (title, body) = pull_text(&review, w.repo.pubkey(), "wss://relay.openagents.com");
        assert_eq!(title, "Return 2");
        assert!(body.starts_with(MARK));
        assert!(body.contains("authorized by `npub1"));
        assert!(body.contains(&review.patch.id));
        assert!(body.contains("never merges"));
        assert_eq!(
            branch(&review.patch.id),
            format!("nostr/{}", review.patch.id)
        );
    }

    #[test]
    fn comments_cross_once_and_never_echo() {
        let w = fixture::world();
        let p = fixture::patch(&w, &"a".repeat(40), true);
        let bridge = Key::from_bytes([14; 32]).unwrap();
        let mine = bridge.sign(
            1_111,
            comment_tags(&p, "wss://r"),
            "x on GitHub: hi".into(),
            5,
        );
        let theirs = w
            .repo
            .sign(1_111, comment_tags(&p, "wss://r"), "Looks good".into(), 6);
        let mut tracked = Tracked::default();
        let replies = [mine, theirs.clone()];
        let picked = nostr_to_mirror(&p, &replies, &tracked, bridge.pubkey());
        assert_eq!(picked.len(), 1);
        assert!(to_github(picked[0], &w.trust).contains("(maintainer) on Nostr"));
        tracked.mirrored_nostr.insert(theirs.id.clone());
        assert!(nostr_to_mirror(&p, &replies, &tracked, bridge.pubkey()).is_empty());
        let comments = vec![
            GitComment {
                id: 1,
                login: "bot".into(),
                body: format!("{MARK}\nmirrored"),
            },
            GitComment {
                id: 2,
                login: "chris".into(),
                body: "Ship it".into(),
            },
        ];
        let back = github_to_mirror(&comments, &tracked);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].id, 2);
        tracked.mirrored_github.insert(2);
        assert!(github_to_mirror(&comments, &tracked).is_empty());
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b"x-access-token:abc"), "eC1hY2Nlc3MtdG9rZW46YWJj");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"a"), "YQ==");
    }

    #[test]
    fn state_round_trips() {
        let dir = std::env::temp_dir().join(format!("kiln-bridge-state-{}", std::process::id()));
        let mut state = State {
            since: 9,
            ..State::default()
        };
        state.patches.insert(
            "e".repeat(64),
            Tracked {
                number: 3,
                ..Tracked::default()
            },
        );
        state.save(&dir).unwrap();
        assert_eq!(State::load(&dir), state);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_lost_pr_reply_is_recovered_from_the_exact_patch_branch() {
        use std::io::{Read, Write};
        let key = kiln::cj::Key::from_bytes([7; 32]).unwrap();
        let patch = key.sign(1617, vec![], "fixture".into(), 100);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body=json!([{"number":42,"html_url":"https://github.com/OpenAgentsInc/openagents/pull/42","head":{"ref":branch(&patch.id),"sha":"22".repeat(20),"repo":{"full_name":"OpenAgentsInc/openagents"}},"body":format!("{MARK} patch {}",patch.id)}]).to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]);
            assert!(request.starts_with("GET /repos/OpenAgentsInc/openagents/pulls?state=all&base=main&head=OpenAgentsInc%3Anostr%2F"));
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let dir = std::env::temp_dir().join(format!("kiln-pr-recovery-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "fixture").unwrap();
        let hub = GitHub::new(
            &format!("http://{addr}"),
            "OpenAgentsInc/openagents",
            &dir.join("token"),
        );
        let found = hub.for_patch(&patch).await.unwrap().unwrap();
        assert_eq!(found.number, 42);
        assert_eq!(found.branch, branch(&patch.id));
        server.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
