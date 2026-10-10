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
//! install Coder. The token is used for these reads only and never kept;
//! a read that fails leaves its part out, and when nothing could be read
//! the turn carries no repository.

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
    /// The repository as one bounded text, or `None` when GitHub answered
    /// none of the reads.
    pub(crate) async fn read(&self) -> Option<RepositorySnapshot> {
        if !oa_auth::repos::full_name(&self.repository) {
            return None;
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
        let tree = tree.and_then(|tree| serde_json::from_value::<Tree>(tree).ok());
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
                self.raw(&refs, branch).await
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
            key_files: keys
                .into_iter()
                .zip(key_texts)
                .filter_map(|(path, text)| Some((path, text?)))
                .collect(),
        };
        let snapshot = parts.text()?;
        Some(RepositorySnapshot {
            name: self.repository.clone(),
            branch: self.branch.clone(),
            snapshot,
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

    async fn get(&self, url: reqwest::Url, raw: bool) -> Option<Vec<u8>> {
        let client = client()?;
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
        let mut response = request.send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        Some(bytes)
    }

    async fn json(&self, path: &[&str], query: &[(&str, &str)]) -> Option<Value> {
        let bytes = self.get(self.url(path, query)?, false).await?;
        serde_json::from_slice(&bytes).ok()
    }

    async fn raw(&self, path: &[&str], branch: &str) -> Option<String> {
        let bytes = self.get(self.url(path, &[("ref", branch)])?, true).await?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        (!text.trim().is_empty()).then_some(text)
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
        if let Some(tree) = &self.tree {
            out.push_str(&tree_text(tree));
        }
        if let Some(readme) = &self.readme {
            out.push_str(&format!(
                "\nREADME:\n{}\n",
                cut(readme.trim(), README_BYTES)
            ));
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
    /// repository read as a project chat reads it, then "Summarize this
    /// repo." asked with it on the website's surface. The answer must come
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
        let snapshot = read.read().await.expect("GitHub answered");
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
                repository: Some(snapshot),
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
