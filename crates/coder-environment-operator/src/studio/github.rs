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
        self.page(path).await.map(|(value, _)| value)
    }

    /// One JSON read and the path of the next page GitHub names in its
    /// `Link` header (only one on this same API origin).
    async fn page(&self, path: &str) -> Result<(Value, Option<String>), String> {
        let (bytes, next) = self.read(path, "application/vnd.github+json").await?;
        let value = serde_json::from_slice(&bytes)
            .map_err(|_| "GitHub sent an answer we couldn't read.".to_owned())?;
        Ok((value, next))
    }

    async fn read(&self, path: &str, accept: &str) -> Result<(Vec<u8>, Option<String>), String> {
        let mut request = self
            .http
            .get(format!("{}{path}", self.base))
            .header("Accept", accept)
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| "GitHub didn't answer. Try again in a moment.".to_owned())?;
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let limited = status == 429
            || (status == 403
                && (header("x-ratelimit-remaining").as_deref() == Some("0")
                    || header("retry-after").is_some()));
        let sso = header("x-github-sso").is_some_and(|v| v.starts_with("required"));
        let next = header("link").and_then(|link| next_page(&link, &self.base));
        if limited {
            return Err(if self.token.is_some() {
                "GitHub is limiting requests right now. Try again in a few minutes.".into()
            } else {
                "GitHub is limiting requests without a sign-in. Connect GitHub, or try again later."
                    .into()
            });
        }
        match status {
            200 => {}
            404 => {
                return Err(
                    "That repository or branch wasn't found. Private repositories need GitHub access."
                        .into(),
                );
            }
            401 => return Err("GitHub didn't accept the access token.".into()),
            403 if sso => {
                return Err("That organization uses single sign-on. On GitHub, authorize OpenAgents for the organization, then try again.".into());
            }
            403 => {
                return Err("GitHub didn't allow access to that repository. An organization owner may need to approve OpenAgents on GitHub.".into());
            }
            500..=599 => {
                return Err("GitHub had a problem answering. Try again in a moment.".into());
            }
            _ => return Err("GitHub sent an answer we couldn't read.".into()),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "GitHub didn't answer. Try again in a moment.".to_owned())?
        {
            if bytes.len().saturating_add(chunk.len()) > BODY_MAX {
                return Err("GitHub's answer was too large to read.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((bytes, next))
    }

    /// The person's repositories, most recently pushed first: GitHub's
    /// pages followed by their `Link` header, up to 300.
    pub async fn repositories(&self) -> Result<Vec<Repo>, String> {
        if self.token.is_none() {
            return Err("Sign in with GitHub to list your repositories.".into());
        }
        let mut found = Vec::new();
        let mut next = Some(
            "/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member"
                .to_owned(),
        );
        for page in 0..3 {
            let Some(path) = next.take() else { break };
            let (v, more) = match self.page(&path).await {
                Ok(read) => read,
                // Keep the pages already read.
                Err(_) if page > 0 => break,
                Err(error) => return Err(error),
            };
            found.extend(v.as_array().into_iter().flatten().filter_map(|r| {
                if r["disabled"].as_bool() == Some(true) {
                    return None;
                }
                Some(Repo {
                    full_name: r["full_name"].as_str()?.to_owned(),
                    default_branch: r["default_branch"].as_str().unwrap_or("main").to_owned(),
                    private: r["private"].as_bool().unwrap_or(false),
                })
            }));
            next = more;
        }
        Ok(found)
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
        // Only the SHA: the full commit answer carries every changed file
        // and its patch, megabytes for a large merge.
        let (bytes, _) = self
            .read(
                &format!("/repos/{}/commits/{}", repo.full(), encode(&branch)),
                "application/vnd.github.sha",
            )
            .await?;
        let commit = String::from_utf8_lossy(&bytes).trim().to_owned();
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

/// The largest GitHub answer read (a page of 100 whole repositories is
/// about 600 KB).
const BODY_MAX: usize = 8 * 1024 * 1024;

/// The `rel="next"` target of a `Link` header as a path under `base`;
/// none on any other origin (the token would go with it).
fn next_page(link: &str, base: &str) -> Option<String> {
    link.split(',').find_map(|part| {
        let (target, params) = part.trim().split_once(';')?;
        let target = target.trim().strip_prefix('<')?.strip_suffix('>')?;
        if !params
            .split(';')
            .any(|p| matches!(p.trim(), "rel=\"next\"" | "rel=next"))
        {
            return None;
        }
        let rest = target.strip_prefix(base)?;
        (rest.starts_with('/') && !rest.chars().any(|c| c.is_whitespace() || c.is_control()))
            .then(|| rest.to_owned())
    })
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

    /// A tiny GitHub stand-in on a thread: `answer(path, accept)` gives
    /// the status line, extra headers, and body.
    fn serve(answer: fn(&str, &str, &str) -> (u16, Vec<(String, String)>, String)) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let base = origin.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut accept = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("accept:") {
                        accept = v.trim().to_owned();
                    }
                }
                let (status, headers, body) = answer(&base, &path, &accept);
                let mut text = format!(
                    "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                    body.len()
                );
                for (k, v) in headers {
                    text.push_str(&format!("{k}: {v}\r\n"));
                }
                text.push_str("\r\n");
                text.push_str(&body);
                stream.write_all(text.as_bytes()).ok();
            }
        });
        origin
    }

    fn rows(from: usize, to: usize) -> String {
        let rows: Vec<Value> = (from..to)
            .map(|n| {
                serde_json::json!({
                    "full_name": format!("o/r{n}"), "default_branch": "main",
                    "private": false, "disabled": n == 3,
                    "description": "x".repeat(5_000),
                })
            })
            .collect();
        serde_json::to_string(&rows).unwrap()
    }

    #[tokio::test]
    async fn listings_follow_link_pages_and_commits_read_only_the_sha() {
        let origin = serve(|base, path, accept| match path {
            p if p.starts_with("/user/repos") && !p.contains("page=2") => (
                200,
                vec![(
                    "link".into(),
                    format!("<{base}/user/repos?per_page=100&page=2>; rel=\"next\""),
                )],
                rows(0, 100),
            ),
            p if p.starts_with("/user/repos") => (200, vec![], rows(100, 130)),
            "/repos/o/r/commits/main" if accept == "application/vnd.github.sha" => {
                (200, vec![], "A".repeat(40))
            }
            "/repos/o/r/commits/main" => (200, vec![], "{\"files\": []}".into()),
            "/repos/o/r" => (
                200,
                vec![],
                "{\"full_name\":\"o/r\",\"default_branch\":\"main\",\"private\":true}".into(),
            ),
            _ => (404, vec![], "{}".into()),
        });
        let github = GitHub::at(&origin, Some("t".into()));
        let found = github.repositories().await.unwrap();
        assert_eq!(found.len(), 129);
        assert!(!found.iter().any(|r| r.full_name == "o/r3"));
        let resolved = github
            .resolve(&RepoName::parse("o/r").unwrap(), None)
            .await
            .unwrap();
        assert_eq!(resolved.commit, "a".repeat(40));
    }

    #[tokio::test]
    async fn refusals_say_what_github_said() {
        let origin = serve(|_, path, _| match path {
            "/repos/limited/r" => (
                403,
                vec![("x-ratelimit-remaining".into(), "0".into())],
                "{\"message\":\"API rate limit exceeded\"}".into(),
            ),
            "/repos/sso/r" => (
                403,
                vec![(
                    "x-github-sso".into(),
                    "required; url=https://github.com/orgs/sso/sso".into(),
                )],
                "{}".into(),
            ),
            "/repos/restricted/r" => (403, vec![], "{}".into()),
            "/repos/broken/r" => (502, vec![], "{}".into()),
            _ => (404, vec![], "{}".into()),
        });
        let github = GitHub::at(&origin, Some("t".into()));
        let error = |name: &'static str| {
            let github = github.clone();
            async move {
                github
                    .repository(&RepoName::parse(name).unwrap())
                    .await
                    .unwrap_err()
            }
        };
        assert!(error("limited/r").await.contains("limiting"));
        assert!(error("sso/r").await.contains("single sign-on"));
        assert!(error("restricted/r").await.contains("organization owner"));
        assert!(error("broken/r").await.contains("had a problem"));
        assert!(error("missing/r").await.contains("wasn't found"));
        let anonymous = GitHub::at(&origin, None);
        assert!(
            anonymous
                .repository(&RepoName::parse("limited/r").unwrap())
                .await
                .unwrap_err()
                .contains("Connect GitHub")
        );
    }

    #[test]
    fn next_pages_stay_on_the_api_origin() {
        let base = "https://api.github.com";
        assert_eq!(
            next_page(
                "<https://api.github.com/user/repos?page=2>; rel=\"next\"",
                base
            )
            .as_deref(),
            Some("/user/repos?page=2")
        );
        assert_eq!(
            next_page(
                "<https://evil.example/user/repos?page=2>; rel=\"next\"",
                base
            ),
            None
        );
        assert_eq!(
            next_page(
                "<https://api.github.com/user/repos?page=5>; rel=\"last\"",
                base
            ),
            None
        );
    }
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
