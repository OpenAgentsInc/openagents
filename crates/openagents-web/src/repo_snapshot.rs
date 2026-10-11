//! A project chat's repository, read for one turn.
//!
//! When a web chat is in a project (a connected GitHub repository), each
//! turn reads the repository on the chosen branch with the person's GitHub
//! connection: its description and topics, languages, recent commits, file
//! tree, README, and a few key files (manifests at the root, chosen by
//! exact file name). The reads are bounded and run at once; what came back
//! becomes one text of at most
//! [`openagents_chat::router::MAX_REPOSITORY_SNAPSHOT_BYTES`] that the turn
//! carries as `context.repository`, so the chat worker answers "Summarize
//! this repo" from the repository itself instead of sending the person to
//! install Coder. The token is used for these reads only and never kept.
//!
//! The text says what the reads found, never only what they returned
//! (#11320): an empty repository (no commits, no files) is said to be
//! empty, a repository without a README is said to have none, a part that
//! couldn't be read is named as not read, and a repository that couldn't
//! be read at all ([`RepoRead::unreadable`]) is still carried, saying so,
//! so the answer says it couldn't look instead of describing the
//! repository from its name.

use std::sync::OnceLock;
use std::time::Duration;

use openagents_chat::router::{MAX_REPOSITORY_SNAPSHOT_BYTES, RepositorySnapshot};
use serde::Deserialize;
use serde_json::Value;

/// What reading a project's repository needs: where GitHub's API is, the
/// person's token for this turn, the repository, and the branch.
#[derive(Clone)]
pub(crate) struct RepoRead {
    pub base: String,
    pub token: Option<String>,
    /// `owner/name`.
    pub repository: String,
    pub branch: String,
    pub private: bool,
}

impl std::fmt::Debug for RepoRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoRead")
            .field("repository", &self.repository)
            .field("branch", &self.branch)
            .finish_non_exhaustive()
    }
}

/// The most bytes of one response read (a large repository's tree).
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
/// The README's share of the snapshot.
const README_BYTES: usize = 5 * 1024;
/// The file tree's share of the snapshot.
const TREE_BYTES: usize = 3 * 1024;
/// Each key file's share, and how many are read.
const KEY_FILE_BYTES: usize = 900;
const KEY_FILES: usize = 3;
/// How many recent commits are named.
const COMMITS: usize = 10;

/// Files at the repository's root that say what it is built with, by
/// exact name, in the order they are read.
const KEY_FILE_NAMES: [&str; 16] = [
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "requirements.txt",
    "Gemfile",
    "composer.json",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "mix.exs",
    "Package.swift",
    "setup.py",
    "Makefile",
    "Dockerfile",
    "docker-compose.yml",
];

impl RepoRead {
    /// The repository as one bounded text: what was read, or, when GitHub
    /// answered none of the reads, that it couldn't be read. `None` only
    /// for a name that isn't `owner/name`.
    pub(crate) async fn read(&self) -> Option<RepositorySnapshot> {
        if !oa_auth::repos::full_name(&self.repository) {
            return None;
        }
        if self.private && self.token.is_none() {
            return self.unreadable("the person's GitHub connection was not available");
        }
        let (owner, name) = self.repository.split_once('/')?;
        let branch = self.branch.as_str();
        let (about_path, languages_path, commits_path, tree_path, readme_path) = (
            ["repos", owner, name],
            ["repos", owner, name, "languages"],
            ["repos", owner, name, "commits"],
            ["repos", owner, name, "git", "trees", branch],
            ["repos", owner, name, "readme"],
        );
        let commits_query = [("sha", branch), ("per_page", "10")];
        let tree_query = [("recursive", "1")];
        let about = self.json(&about_path, &[]);
        let languages = self.json(&languages_path, &[]);
        let commits = self.json(&commits_path, &commits_query);
        let tree = self.json(&tree_path, &tree_query);
        let readme = self.raw(&readme_path, branch);
        let (about, languages, commits, tree, readme) =
            tokio::join!(about, languages, commits, tree, readme);
        // GitHub answers 409 for the commits and the tree of a repository
        // with no commits yet.
        let empty = commits.status() == Some(409) || tree.status() == Some(409);
        let no_readme = matches!(readme.status(), Some(404 | 204));
        let (about, languages, commits, readme) =
            (about.ok(), languages.ok(), commits.ok(), readme.ok());
        let tree = tree
            .ok()
            .and_then(|tree| serde_json::from_value::<Tree>(tree).ok());
        let keys: Vec<String> = tree
            .as_ref()
            .map(|tree| key_files(&tree.tree))
            .unwrap_or_default();
        let key_reads = keys.iter().map(|path| {
            let mut segments = vec!["repos", owner, name, "contents"];
            segments.extend(path.split('/'));
            let segments: Vec<String> = segments.into_iter().map(str::to_string).collect();
            async move {
                let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
                self.raw(&refs, branch).await.ok()
            }
        });
        let key_texts = futures_util::future::join_all(key_reads).await;
        let parts = Parts {
            repository: self.repository.clone(),
            branch: self.branch.clone(),
            private: self.private,
            about,
            languages,
            commits,
            tree,
            readme,
            empty,
            no_readme,
            key_files: keys
                .into_iter()
                .zip(key_texts)
                .filter_map(|(path, text)| Some((path, text?)))
                .collect(),
        };
        let snapshot = match parts.text() {
            Some(text) => text,
            // A public repository GitHub wouldn't show with the person's
            // token (expired or revoked): read it again without one.
            None if !self.private && self.token.is_some() => {
                let anonymous = RepoRead {
                    token: None,
                    ..self.clone()
                };
                return Box::pin(anonymous.read()).await;
            }
            None => {
                return self.unreadable("GitHub refused or did not answer every read");
            }
        };
        eprintln!(
            "openagents-web: chat read repository {} (empty {}, tree {}, readme {})",
            self.repository,
            parts.empty,
            parts.tree.is_some(),
            parts.readme.is_some()
        );
        Some(RepositorySnapshot {
            name: self.repository.clone(),
            branch: self.branch.clone(),
            snapshot,
        })
    }

    /// The turn's repository when it couldn't be read, saying so and why
    /// (`reason`, a fixed phrase, never GitHub's words), so the answer says
    /// it couldn't look rather than guess from the repository's name.
    pub(crate) fn unreadable(&self, reason: &str) -> Option<RepositorySnapshot> {
        if !oa_auth::repos::full_name(&self.repository) {
            return None;
        }
        eprintln!(
            "openagents-web: chat couldn't read repository {}: {reason}",
            self.repository
        );
        Some(RepositorySnapshot {
            name: self.repository.clone(),
            branch: self.branch.clone(),
            snapshot: format!(
                "Repository: {} ({}), branch {}\n\
                 NOT READ: OpenAgents could not read this repository for this message ({reason}). \
                 Nothing about its files, README, or commits is known. Say plainly that the \
                 repository couldn't be read just now and that reconnecting GitHub on the \
                 Projects page or asking again may help; do not describe the repository from \
                 its name.\n",
                self.repository,
                if self.private { "private" } else { "public" },
                self.branch
            ),
        })
    }

    fn url(&self, path: &[&str], query: &[(&str, &str)]) -> Option<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base).ok()?;
        url.path_segments_mut()
            .ok()?
            .pop_if_empty()
            .extend(path.iter().copied());
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        Some(url)
    }

    async fn get(&self, url: reqwest::Url, raw: bool) -> Fetched {
        let Some(client) = client() else {
            return Fetched::Failed;
        };
        let mut request = client
            .get(url)
            .header(
                "Accept",
                if raw {
                    "application/vnd.github.raw"
                } else {
                    "application/vnd.github+json"
                },
            )
            .header("X-GitHub-Api-Version", oa_auth::github::API_VERSION);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let Ok(mut response) = request.send().await else {
            return Fetched::Failed;
        };
        let status = response.status();
        if !status.is_success() {
            return Fetched::Status(status.as_u16());
        }
        let mut bytes = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                        return Fetched::Failed;
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(None) => return Fetched::Body(bytes),
                Err(_) => return Fetched::Failed,
            }
        }
    }

    async fn json(&self, path: &[&str], query: &[(&str, &str)]) -> Got<Value> {
        let Some(url) = self.url(path, query) else {
            return Got::Failed;
        };
        match self.get(url, false).await {
            Fetched::Body(bytes) => serde_json::from_slice(&bytes).map_or(Got::Failed, Got::Read),
            Fetched::Status(status) => Got::Status(status),
            Fetched::Failed => Got::Failed,
        }
    }

    async fn raw(&self, path: &[&str], branch: &str) -> Got<String> {
        let Some(url) = self.url(path, &[("ref", branch)]) else {
            return Got::Failed;
        };
        match self.get(url, true).await {
            Fetched::Body(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                if text.trim().is_empty() {
                    Got::Status(204)
                } else {
                    Got::Read(text)
                }
            }
            Fetched::Status(status) => Got::Status(status),
            Fetched::Failed => Got::Failed,
        }
    }
}

/// What one GitHub request returned.
enum Fetched {
    Body(Vec<u8>),
    /// GitHub answered with this unsuccessful status.
    Status(u16),
    /// No answer, or one too large or cut off.
    Failed,
}

/// One part of the repository as read: its value, GitHub's unsuccessful
/// status (404 for a missing README, 409 for an empty repository), or
/// no answer.
#[derive(Debug)]
enum Got<T> {
    Read(T),
    Status(u16),
    Failed,
}

impl<T> Got<T> {
    fn ok(self) -> Option<T> {
        match self {
            Got::Read(value) => Some(value),
            _ => None,
        }
    }

    fn status(&self) -> Option<u16> {
        match self {
            Got::Status(status) => Some(*status),
            _ => None,
        }
    }
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(6))
                .redirect(reqwest::redirect::Policy::limited(2))
                .user_agent("OpenAgents-chat-repository")
                .build()
                .ok()
        })
        .as_ref()
}

#[derive(Debug, Deserialize)]
struct Tree {
    #[serde(default)]
    tree: Vec<Entry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct Entry {
    path: String,
    #[serde(rename = "type", default)]
    kind: String,
}

/// The root files of [`KEY_FILE_NAMES`] the tree has, in that order.
fn key_files(tree: &[Entry]) -> Vec<String> {
    KEY_FILE_NAMES
        .iter()
        .filter(|name| {
            tree.iter()
                .any(|entry| entry.kind == "blob" && entry.path == **name)
        })
        .take(KEY_FILES)
        .map(|name| (*name).to_string())
        .collect()
}

/// What the reads returned, each `None` when GitHub didn't answer it.
#[derive(Debug, Default)]
struct Parts {
    repository: String,
    branch: String,
    private: bool,
    about: Option<Value>,
    languages: Option<Value>,
    commits: Option<Value>,
    tree: Option<Tree>,
    readme: Option<String>,
    /// GitHub says the repository has no commits (and so no files).
    empty: bool,
    /// GitHub says the branch has no README.
    no_readme: bool,
    key_files: Vec<(String, String)>,
}

impl Parts {
    /// The snapshot text, at most [`MAX_REPOSITORY_SNAPSHOT_BYTES`]; `None`
    /// when nothing was read.
    fn text(&self) -> Option<String> {
        if self.about.is_none()
            && self.languages.is_none()
            && self.commits.is_none()
            && self.tree.is_none()
            && self.readme.is_none()
            && !self.empty
        {
            return None;
        }
        let mut out = format!(
            "Repository: {} ({}), branch {}\n",
            self.repository,
            if self.private { "private" } else { "public" },
            self.branch
        );
        if let Some(about) = &self.about {
            if let Some(description) = about["description"].as_str().filter(|d| !d.is_empty()) {
                out.push_str(&format!("Description: {}\n", cut(description, 400)));
            }
            if let Some(home) = about["homepage"].as_str().filter(|h| !h.is_empty()) {
                out.push_str(&format!("Homepage: {}\n", cut(home, 200)));
            }
            let topics: Vec<&str> = about["topics"]
                .as_array()
                .map(|topics| topics.iter().filter_map(Value::as_str).take(12).collect())
                .unwrap_or_default();
            if !topics.is_empty() {
                out.push_str(&format!("Topics: {}\n", topics.join(", ")));
            }
            if let Some(license) = about["license"]["spdx_id"].as_str() {
                out.push_str(&format!("License: {license}\n"));
            }
            if let Some(pushed) = about["pushed_at"].as_str() {
                out.push_str(&format!("Last pushed: {}\n", cut(pushed, 10)));
            }
        }
        if let Some(languages) = self.languages.as_ref().and_then(Value::as_object) {
            let total: f64 = languages.values().filter_map(Value::as_f64).sum();
            if total > 0.0 {
                let mut shares: Vec<(&String, f64)> = languages
                    .iter()
                    .filter_map(|(name, bytes)| Some((name, bytes.as_f64()? / total * 100.0)))
                    .collect();
                shares.sort_by(|a, b| b.1.total_cmp(&a.1));
                let line: Vec<String> = shares
                    .iter()
                    .take(8)
                    .map(|(name, share)| format!("{name} {share:.0}%"))
                    .collect();
                out.push_str(&format!("Languages: {}\n", line.join(", ")));
            }
        }
        if let Some(commits) = self.commits.as_ref().and_then(Value::as_array) {
            if !commits.is_empty() {
                out.push_str(&format!("\nRecent commits on {}:\n", self.branch));
            }
            for commit in commits.iter().take(COMMITS) {
                let sha = commit["sha"].as_str().map_or("", |sha| cut(sha, 7));
                let date = commit["commit"]["author"]["date"]
                    .as_str()
                    .map_or("", |date| cut(date, 10));
                let message = commit["commit"]["message"]
                    .as_str()
                    .and_then(|m| m.lines().next())
                    .unwrap_or_default();
                out.push_str(&format!("- {date} {sha} {}\n", cut(message, 120)));
            }
        }
        if self.empty {
            out.push_str(
                "\nEMPTY: GitHub says this repository has no commits yet, so it has no files, \
                 no README, and no code. Say plainly that it is empty; do not describe what it \
                 might contain from its name.\n",
            );
        }
        if let Some(tree) = &self.tree {
            out.push_str(&tree_text(tree));
        }
        if let Some(readme) = &self.readme {
            out.push_str(&format!(
                "\nREADME:\n{}\n",
                cut(readme.trim(), README_BYTES)
            ));
        } else if self.no_readme && !self.empty {
            out.push_str("\nREADME: none (the branch has no README file).\n");
        }
        if !self.empty {
            let unread: Vec<&str> = [
                (self.tree.is_none(), "the file tree"),
                (self.readme.is_none() && !self.no_readme, "the README"),
                (self.commits.is_none(), "the recent commits"),
            ]
            .into_iter()
            .filter_map(|(missing, part)| missing.then_some(part))
            .collect();
            if !unread.is_empty() {
                out.push_str(&format!(
                    "\nNOT READ: {} could not be read just now; say so if asked about it rather \
                     than guess.\n",
                    unread.join(", ")
                ));
            }
        }
        for (path, text) in &self.key_files {
            out.push_str(&format!(
                "\n{path}:\n{}\n",
                cut(text.trim(), KEY_FILE_BYTES)
            ));
        }
        Some(cut(&out, MAX_REPOSITORY_SNAPSHOT_BYTES).to_string())
    }
}

/// The file tree within [`TREE_BYTES`]: the shallowest paths first, then
/// listed in order, with how many there are in all.
fn tree_text(tree: &Tree) -> String {
    let mut paths: Vec<(&str, bool)> = tree
        .tree
        .iter()
        .filter(|entry| entry.kind == "blob" || entry.kind == "tree")
        .map(|entry| (entry.path.as_str(), entry.kind == "tree"))
        .collect();
    let files = paths.iter().filter(|(_, dir)| !dir).count();
    paths.sort_by_key(|(path, _)| (path.matches('/').count(), *path));
    let mut shown = Vec::new();
    let mut bytes = 0;
    for (path, dir) in &paths {
        let line = if *dir {
            format!("{path}/")
        } else {
            (*path).to_string()
        };
        if bytes + line.len() + 1 > TREE_BYTES {
            break;
        }
        bytes += line.len() + 1;
        shown.push(line);
    }
    shown.sort();
    let mut out = format!(
        "\nFiles ({files} files{}; {} of {} paths shown, shallowest first):\n",
        if tree.truncated {
            ", more than GitHub lists at once"
        } else {
            ""
        },
        shown.len(),
        paths.len()
    );
    for line in shown {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// `text` cut to at most `bytes` bytes on a character boundary.
fn cut(text: &str, bytes: usize) -> &str {
    if text.len() <= bytes {
        return text;
    }
    let mut end = bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(path: &str, kind: &str) -> Entry {
        Entry {
            path: path.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn a_snapshot_says_what_the_repository_is_within_its_bound() {
        let parts = Parts {
            repository: "AtlantisPleb/finances".into(),
            branch: "main".into(),
            private: true,
            about: Some(json!({
                "description": "Household ledger and tax scripts",
                "topics": ["finance", "ledger"],
                "license": {"spdx_id": "MIT"},
                "pushed_at": "2026-10-09T12:00:00Z"
            })),
            languages: Some(json!({"Python": 750, "Shell": 250})),
            commits: Some(json!([
                {"sha": "abcdef1234567", "commit": {"message": "Add 2026 budget\n\nbody", "author": {"date": "2026-10-09T10:00:00Z"}}}
            ])),
            tree: Some(Tree {
                tree: vec![
                    entry("src", "tree"),
                    entry("src/ledger.py", "blob"),
                    entry("README.md", "blob"),
                    entry("pyproject.toml", "blob"),
                ],
                truncated: false,
            }),
            readme: Some("# Finances\n\n".to_string() + &"x".repeat(20_000)),
            empty: false,
            no_readme: false,
            key_files: vec![(
                "pyproject.toml".into(),
                "[project]\nname = \"finances\"".into(),
            )],
        };
        let text = parts.text().unwrap();
        assert!(text.len() <= MAX_REPOSITORY_SNAPSHOT_BYTES);
        assert!(text.starts_with("Repository: AtlantisPleb/finances (private), branch main"));
        assert!(text.contains("Description: Household ledger and tax scripts"));
        assert!(text.contains("Languages: Python 75%, Shell 25%"));
        assert!(text.contains("- 2026-10-09 abcdef1 Add 2026 budget"));
        assert!(!text.contains("body"));
        assert!(text.contains("src/ledger.py"));
        assert!(text.contains("# Finances"));
        assert!(text.contains("pyproject.toml:\n[project]"));
    }

    /// Reads a public repository from GitHub without a token:
    /// `cargo test -p openagents-web --lib repo_snapshot -- --ignored`.
    #[tokio::test]
    #[ignore = "reads github.com"]
    async fn reads_a_public_repository() {
        let read = RepoRead {
            base: "https://api.github.com".into(),
            token: None,
            repository: "rust-lang/log".into(),
            branch: "master".into(),
            private: false,
        };
        let snapshot = read.read().await.expect("GitHub answered");
        println!("{}", snapshot.snapshot);
        assert!(snapshot.snapshot.contains("Languages: Rust"));
        assert!(snapshot.snapshot.contains("README:"));
        assert!(snapshot.snapshot.contains("Cargo.toml:"));
        assert!(snapshot.snapshot.len() <= MAX_REPOSITORY_SNAPSHOT_BYTES);
    }

    /// End to end against the production chat worker: a public
    /// repository read as a project chat reads it (empty, missing, or
    /// unreadable ones say so), then "Summarize this repo." asked with it
    /// on the website's surface. The answer must come
    /// from the repository, never the install text:
    /// `OPENAGENTS_LIVE_REPO=owner/name OPENAGENTS_LIVE_BRANCH=main cargo test
    /// -p openagents-web --lib repo_snapshot -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "asks the production chat worker"]
    async fn the_production_worker_summarizes_a_project_repository() {
        use crate::ask::Chat;
        use openagents_chat::basic_coder::{Reply, Turn};
        use openagents_chat::router::{Context, Surface};
        use std::sync::{Arc, Mutex};
        let repository =
            std::env::var("OPENAGENTS_LIVE_REPO").unwrap_or_else(|_| "rust-lang/log".into());
        let branch = std::env::var("OPENAGENTS_LIVE_BRANCH").unwrap_or_else(|_| "master".into());
        let read = RepoRead {
            base: "https://api.github.com".into(),
            token: None,
            repository,
            branch,
            private: false,
        };
        // OPENAGENTS_LIVE_NO_REPO=1 sends the turn with no repository, as
        // a turn whose read failed was sent before #11320.
        let snapshot = if std::env::var_os("OPENAGENTS_LIVE_NO_REPO").is_some() {
            None
        } else {
            Some(read.read().await.expect("a valid repository name"))
        };
        let sent = snapshot.as_ref().map(|s| s.snapshot.clone());
        let door = crate::ask::Worker::default()
            .door(secp256k1::SecretKey::new(&mut secp256k1::rand::rng()))
            .unwrap();
        let reply = Arc::new(Mutex::new(Reply::default()));
        let message = std::env::var("OPENAGENTS_LIVE_MESSAGE")
            .unwrap_or_else(|_| "Summarize this repo.".into());
        door.ask(
            vec![Turn::user(message)],
            Context {
                surface: Surface::Web,
                repository: snapshot,
                ..Context::default()
            },
            reply.clone(),
        )
        .await;
        let reply = openagents_chat::basic_coder::lock(&reply);
        println!(
            "route {:?} tier {:?} answer {:?} model {:?}\n---\n{}",
            reply.meta.route, reply.meta.tier, reply.meta.answer, reply.model, reply.text
        );
        assert!(
            reply.done,
            "{:?}",
            reply.failure.as_ref().map(|f| f.describe())
        );
        assert!(!reply.text.contains("coder login"));
        assert!(!reply.text.contains("install.sh"));
        // The answer comes from what was read (#11320): a repository with
        // files is described by naming at least one of its real files; an
        // empty one is said to be empty; an unread one, unread.
        let text = reply.text.to_lowercase();
        let Some(sent) = sent else { return };
        if sent.contains("\nNOT READ: OpenAgents could not read") {
            assert!(
                ["could not", "couldn't", "can't", "cannot", "unable"]
                    .iter()
                    .any(|word| text.contains(word)),
                "{text}"
            );
        } else if sent.contains("\nEMPTY:") {
            assert!(text.contains("empty"), "{text}");
        } else {
            let files: Vec<&str> = sent
                .split("shallowest first):\n")
                .nth(1)
                .unwrap_or_default()
                .lines()
                .take_while(|line| !line.is_empty())
                .filter(|line| !line.ends_with('/'))
                .collect();
            assert!(
                files.iter().any(|path| {
                    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
                    text.contains(&name)
                }),
                "the answer names none of {files:?}: {text}"
            );
        }
    }

    /// How the fixture GitHub answers: a repository with files, one with
    /// no commits yet, or one that refuses every read.
    #[derive(Clone, Copy)]
    enum Fixture {
        Files,
        Empty,
        Refused,
        /// Refuses a token (expired), answers without one as `Files`.
        RefusesTokens,
    }

    /// A stand-in for GitHub's API serving `fixture` for `fixture/finances`,
    /// as GitHub answers those reads; its base URL.
    async fn github(fixture: Fixture) -> String {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        let answer = move |uri: axum::http::Uri, headers: axum::http::HeaderMap| async move {
            let token = headers.contains_key("authorization");
            let fixture = match fixture {
                Fixture::RefusesTokens if !token => Fixture::Files,
                fixture => fixture,
            };
            let path = uri.path().trim_start_matches("/repos/fixture/finances");
            let json = |value: Value| axum::Json(value).into_response();
            let status = |code: u16, message: &str| {
                (
                    StatusCode::from_u16(code).unwrap(),
                    axum::Json(json!({"message": message})),
                )
                    .into_response()
            };
            match (fixture, path) {
                (Fixture::Refused, _) => status(401, "Bad credentials"),
                (Fixture::RefusesTokens, _) if token => status(401, "Bad credentials"),
                (_, "") => json(json!({
                    "description": null,
                    "pushed_at": "2026-10-11T12:00:00Z",
                    "size": if matches!(fixture, Fixture::Empty) { 0 } else { 42 }
                })),
                (Fixture::Empty, "/languages") => json(json!({})),
                (Fixture::Empty, "/commits" | "/git/trees/main") => {
                    status(409, "Git Repository is empty.")
                }
                (Fixture::Empty, _) => status(404, "This repository is empty."),
                (Fixture::Files, "/languages") => json(json!({"Python": 900, "Shell": 100})),
                (Fixture::Files, "/commits") => json(json!([
                    {"sha": "3f9a2c1aaaaaaa", "commit": {"message": "Add 2026 budget categories", "author": {"date": "2026-10-08T10:00:00Z"}}}
                ])),
                (Fixture::Files, "/git/trees/main") => json(json!({"tree": [
                    {"path": "README.md", "type": "blob"},
                    {"path": "finances", "type": "tree"},
                    {"path": "finances/importer.py", "type": "blob"},
                    {"path": "finances/ledger.py", "type": "blob"},
                    {"path": "pyproject.toml", "type": "blob"},
                    {"path": "scripts/monthly.sh", "type": "blob"}
                ], "truncated": false})),
                (Fixture::Files, "/readme") => {
                    "# finances\n\nImports bank CSV exports into a SQLite ledger.".into_response()
                }
                (Fixture::Files, "/contents/pyproject.toml") => {
                    "[project]\nname = \"finances\"".into_response()
                }
                _ => status(404, "Not Found"),
            }
        };
        let router = axum::Router::new().fallback(answer);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{address}")
    }

    fn fixture_read(base: String, token: Option<&str>, private: bool) -> RepoRead {
        RepoRead {
            base,
            token: token.map(str::to_string),
            repository: "fixture/finances".into(),
            branch: "main".into(),
            private,
        }
    }

    /// A repository with files: the snapshot carries its real paths,
    /// README, manifest, and commits, and says nothing is missing (#11320).
    #[tokio::test]
    async fn a_fixture_repository_is_read_from_its_files() {
        let read = fixture_read(github(Fixture::Files).await, Some("t"), true);
        let snapshot = read.read().await.unwrap().snapshot;
        for real in [
            "Repository: fixture/finances (private), branch main",
            "Languages: Python 90%, Shell 10%",
            "- 2026-10-08 3f9a2c1 Add 2026 budget categories",
            "finances/importer.py",
            "finances/ledger.py",
            "scripts/monthly.sh",
            "README:\n# finances\n\nImports bank CSV exports into a SQLite ledger.",
            "pyproject.toml:\n[project]",
        ] {
            assert!(snapshot.contains(real), "{real:?} missing from {snapshot}");
        }
        assert!(!snapshot.contains("EMPTY"), "{snapshot}");
        assert!(!snapshot.contains("NOT READ"), "{snapshot}");
    }

    /// AtlantisPleb/finances as it was (#11320): a repository with no
    /// commits. GitHub answers its metadata only, so the snapshot used to
    /// be a name and a date, and the answer described a finance app from
    /// the name. It now says the repository is empty.
    #[tokio::test]
    async fn an_empty_repository_is_said_to_be_empty() {
        let read = fixture_read(github(Fixture::Empty).await, Some("t"), false);
        let snapshot = read.read().await.unwrap().snapshot;
        assert!(snapshot.contains("EMPTY: GitHub says this repository has no commits yet"));
        assert!(snapshot.contains("do not describe what it might contain from its name"));
        assert!(!snapshot.contains("NOT READ"), "{snapshot}");
        assert!(!snapshot.contains("README:"), "{snapshot}");
    }

    /// A repository GitHub won't show this connection: carried anyway,
    /// saying it couldn't be read, instead of the turn carrying nothing
    /// and the model answering from nothing.
    #[tokio::test]
    async fn a_refused_repository_is_said_to_be_unread() {
        let read = fixture_read(github(Fixture::Refused).await, Some("t"), true);
        let snapshot = read.read().await.unwrap();
        assert_eq!(snapshot.name, "fixture/finances");
        assert!(
            snapshot
                .snapshot
                .contains("NOT READ: OpenAgents could not read this repository"),
            "{}",
            snapshot.snapshot
        );
        assert!(snapshot.snapshot.len() <= MAX_REPOSITORY_SNAPSHOT_BYTES);
    }

    /// A public repository whose reads the person's token can't make (an
    /// expired connection) is read without the token.
    #[tokio::test]
    async fn a_public_repository_is_read_without_a_refused_token() {
        let read = fixture_read(github(Fixture::RefusesTokens).await, Some("t"), false);
        let snapshot = read.read().await.unwrap().snapshot;
        assert!(snapshot.contains("finances/ledger.py"), "{snapshot}");
        assert!(!snapshot.contains("NOT READ"), "{snapshot}");
    }

    /// A private repository with no GitHub connection is never asked for
    /// and reads as unread.
    #[tokio::test]
    async fn a_private_repository_without_a_connection_is_unread() {
        let read = fixture_read("http://127.0.0.1:9".into(), None, true);
        let snapshot = read.read().await.unwrap().snapshot;
        assert!(
            snapshot.contains("GitHub connection was not available"),
            "{snapshot}"
        );
    }

    /// One part GitHub didn't answer is named as not read.
    #[test]
    fn a_part_not_read_is_named() {
        let parts = Parts {
            repository: "o/r".into(),
            branch: "main".into(),
            about: Some(json!({"description": "x"})),
            ..Parts::default()
        };
        let text = parts.text().unwrap();
        assert!(
            text.contains(
                "NOT READ: the file tree, the README, the recent commits could not be read"
            ),
            "{text}"
        );
        let parts = Parts {
            no_readme: true,
            ..parts
        };
        assert!(parts.text().unwrap().contains("README: none"));
    }

    #[test]
    fn nothing_read_is_no_snapshot() {
        assert!(Parts::default().text().is_none());
    }

    #[test]
    fn key_files_are_root_manifests_by_exact_name() {
        let tree = vec![
            entry("web/package.json", "blob"),
            entry("Cargo.toml", "blob"),
            entry("package.json", "blob"),
            entry("Makefile", "tree"),
        ];
        assert_eq!(key_files(&tree), ["package.json", "Cargo.toml"]);
    }

    #[test]
    fn a_long_cut_keeps_whole_characters() {
        assert_eq!(cut("héllo", 2), "h");
        assert_eq!(cut("abc", 10), "abc");
    }
}
