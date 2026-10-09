//! Repositories on GitHub: the signed-in person's list, a repository's
//! branches, and the exact commit a branch points at.
//!
//! A token is optional. Without one, public repositories still resolve
//! (GitHub's anonymous limits apply) and the list of your repositories is
//! not offered. The token is sent only to `api.github.com` and is never
//! retained or logged.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const API: &str = "https://api.github.com";

/// A repository named by the person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoName {
    pub owner: String,
    pub name: String,
}

impl RepoName {
    /// Accepts `owner/name`, `github.com/owner/name`, or an
    /// `https://github.com/owner/name(.git)` URL, with or without a
    /// trailing path such as `/tree/main`.
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim().trim_end_matches('/');
        let s = s
            .strip_prefix("https://")
            .or_else(|| s.strip_prefix("http://"))
            .unwrap_or(s);
        let s = s.strip_prefix("www.").unwrap_or(s);
        let s = s.strip_prefix("github.com/").unwrap_or(s);
        if s.contains("://") || s.contains('@') {
            return None;
        }
        let mut parts = s.split('/');
        let owner = parts.next()?.to_owned();
        let name = parts.next()?.trim_end_matches(".git").to_owned();
        let plain = |p: &str| {
            !p.is_empty()
                && p.len() <= 100
                && !p.starts_with('.')
                && p.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        };
        (plain(&owner) && plain(&name)).then_some(Self { owner, name })
    }

    /// `owner/name`.
    pub fn full(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// One of the person's repositories.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    pub full_name: String,
    pub default_branch: String,
    pub private: bool,
}

/// A repository resolved to an exact commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    pub repository: RepoName,
    pub branch: String,
    pub commit: String,
    pub private: bool,
}

/// A GitHub API client with an optional token.
#[derive(Clone)]
pub struct GitHub {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl std::fmt::Debug for GitHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHub")
            .field("base", &self.base)
            .field("token", &self.token.is_some())
            .finish()
    }
}

impl GitHub {
    pub fn new(token: Option<String>) -> Self {
        Self::at(API, token)
    }

    pub fn at(base: &str, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("openagents-environments")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            token: token.filter(|t| !t.is_empty()),
        }
    }

    /// Whether the person's own repositories can be listed.
    pub fn signed_in(&self) -> bool {
        self.token.is_some()
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let mut request = self
            .http
            .get(format!("{}{path}", self.base))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "GitHub didn't answer. Try again in a moment.".to_owned())?;
        match response.status().as_u16() {
            200 => response
                .json()
                .await
                .map_err(|_| "GitHub sent an answer we couldn't read.".to_owned()),
            404 => Err(
                "That repository or branch wasn't found. Private repositories need GitHub access."
                    .into(),
            ),
            401 => Err("GitHub didn't accept the access token.".into()),
            403 | 429 => Err("GitHub is limiting requests right now. Try again later.".into()),
            _ => Err("GitHub didn't answer. Try again in a moment.".into()),
        }
    }

    /// The person's repositories, most recently pushed first.
    pub async fn repositories(&self) -> Result<Vec<Repo>, String> {
        if self.token.is_none() {
            return Err("Sign in with GitHub to list your repositories.".into());
        }
        let v = self
            .get("/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member")
            .await?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some(Repo {
                    full_name: r["full_name"].as_str()?.to_owned(),
                    default_branch: r["default_branch"].as_str().unwrap_or("main").to_owned(),
                    private: r["private"].as_bool().unwrap_or(false),
                })
            })
            .collect())
    }

    /// A repository's default branch and visibility.
    pub async fn repository(&self, repo: &RepoName) -> Result<Repo, String> {
        let v = self.get(&format!("/repos/{}", repo.full())).await?;
        Ok(Repo {
            full_name: v["full_name"].as_str().unwrap_or(&repo.full()).to_owned(),
            default_branch: v["default_branch"].as_str().unwrap_or("main").to_owned(),
            private: v["private"].as_bool().unwrap_or(false),
        })
    }

    /// Up to 100 branch names.
    pub async fn branches(&self, repo: &RepoName) -> Result<Vec<String>, String> {
        let v = self
            .get(&format!("/repos/{}/branches?per_page=100", repo.full()))
            .await?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["name"].as_str().map(str::to_owned))
            .collect())
    }

    /// The exact commit `branch` points at (the default branch when
    /// `None`).
    pub async fn resolve(&self, repo: &RepoName, branch: Option<&str>) -> Result<Resolved, String> {
        let info = self.repository(repo).await?;
        let branch = branch
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .unwrap_or(&info.default_branch)
            .to_owned();
        if branch.contains("..") || branch.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err("That branch name isn't valid.".into());
        }
        let v = self
            .get(&format!(
                "/repos/{}/commits/{}",
                repo.full(),
                encode(&branch)
            ))
            .await?;
        let commit = v["sha"].as_str().unwrap_or_default().to_owned();
        if commit.len() != 40 || !commit.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("GitHub didn't name a commit for that branch.".into());
        }
        let name = RepoName::parse(&info.full_name).unwrap_or_else(|| repo.clone());
        Ok(Resolved {
            repository: name,
            branch,
            commit: commit.to_ascii_lowercase(),
            private: info.private,
        })
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_names_parse_from_what_people_paste() {
        for input in [
            "OpenAgentsInc/openagents",
            "github.com/OpenAgentsInc/openagents",
            "https://github.com/OpenAgentsInc/openagents",
            "https://github.com/OpenAgentsInc/openagents.git",
            "https://github.com/OpenAgentsInc/openagents/tree/main",
            " https://www.github.com/OpenAgentsInc/openagents/ ",
        ] {
            let r = RepoName::parse(input).unwrap_or_else(|| panic!("{input}"));
            assert_eq!(r.full(), "OpenAgentsInc/openagents", "{input}");
        }
        for bad in [
            "",
            "openagents",
            "https://user:pw@github.com/a/b",
            "git@github.com:a/b.git",
            "a/.b",
            "a b/c",
        ] {
            assert!(RepoName::parse(bad).is_none(), "{bad}");
        }
        assert_eq!(encode("feature/x y"), "feature%2Fx%20y");
    }
}
