//! Forge repository management, git execution, and the git credential helper.
//!
//! Every command here either reads the server's own answer or refuses. There is
//! no default repository, no assumed visibility, and no invented clone URL: a
//! repository the API did not describe is one this CLI cannot describe either.

use crate::auth::{api_error_detail, AuthError, Secret};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::process::Command as SyncCommand;
use std::time::{Duration, Instant};
use tokio::process::Command;

// ---------------------------------------------------------------------------
// contract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryOwner {
    pub id: serde_json::Value,
    pub login: String,
    #[serde(default)]
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryPermissions {
    pub admin: bool,
    pub push: bool,
    pub pull: bool,
}

/// The repository as `openagents.repositories.v1` describes it.
///
/// Nothing here has a serde default. A response missing `lifecycle_state` or
/// `clone_url` is a response this CLI cannot report on, and saying so is the
/// point: the alternative is printing `Provisioning: ready` about a repository
/// whose state the server never sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub owner: RepositoryOwner,
    pub private: bool,
    pub visibility: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub lifecycle_state: String,
    pub provision_error_code: Option<String>,
    pub clone_url: String,
    pub html_url: String,
    pub permissions: RepositoryPermissions,
    pub created_at: String,
    pub updated_at: String,
}

impl Repository {
    /// The block `oa repo view` prints, field for field with the TypeScript CLI.
    pub fn human_lines(&self) -> Vec<String> {
        vec![
            self.full_name.clone(),
            format!(
                "Visibility: {}",
                if self.private { "private" } else { "public" }
            ),
            format!("Default branch: {}", self.default_branch),
            format!("Provisioning: {}", self.lifecycle_state),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryImport {
    pub id: String,
    pub provider: String,
    pub source_full_name: String,
    pub state: String,
    #[serde(default)]
    pub attempt_count: i64,
    #[serde(default)]
    pub lfs_warning: bool,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryList {
    pub repositories: Vec<Repository>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedNamespace {
    pub id: serde_json::Value,
    pub login: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: i64,
    pub login: String,
    pub token_expires_at: String,
    pub namespaces: Vec<AuthenticatedNamespace>,
}

// ---------------------------------------------------------------------------
// name validation
// ---------------------------------------------------------------------------

/// `[a-z0-9](?:[a-z0-9_-]|\.(?=[a-z0-9])){0,63}` written out, because a dot has
/// to be followed by an alphanumeric and Rust's regex engine has no lookahead.
pub fn validate_repository_name(name: &str) -> Result<String, AuthError> {
    let normalized = name.trim().to_ascii_lowercase();
    let bytes = normalized.as_bytes();
    let mut valid = (1..=64).contains(&bytes.len()) && bytes[0].is_ascii_alphanumeric();
    let mut index = 1;
    while valid && index < bytes.len() {
        let byte = bytes[index];
        valid = byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || byte == b'_'
            || byte == b'-'
            || (byte == b'.'
                && bytes
                    .get(index + 1)
                    .is_some_and(|next| next.is_ascii_lowercase() || next.is_ascii_digit()));
        index += 1;
    }
    if !valid {
        return Err(AuthError::new(format!(
            "invalid repository name {name}. Names must match \
             [a-z0-9](?:[a-z0-9_-]|\\.(?=[a-z0-9])){{0,63}}"
        )));
    }
    Ok(normalized)
}

/// `[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})`, the GitHub namespace shape.
pub fn validate_owner(owner: &str) -> Result<String, AuthError> {
    let normalized = owner.trim().to_string();
    let bytes = normalized.as_bytes();
    let valid = (1..=39).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-');
    if !valid {
        return Err(AuthError::new(format!(
            "invalid GitHub-backed namespace: {owner}"
        )));
    }
    Ok(normalized)
}

pub fn parse_repository_target(full_name: &str) -> Result<(String, String), AuthError> {
    let parts: Vec<&str> = full_name.trim().split('/').collect();
    if parts.len() != 2 {
        return Err(AuthError::new("use the repository format OWNER/REPO"));
    }
    Ok((
        validate_owner(parts[0])?,
        validate_repository_name(parts[1])?,
    ))
}

// ---------------------------------------------------------------------------
// client
// ---------------------------------------------------------------------------

pub struct RepoClient {
    origin: String,
    token: Option<Secret>,
    http: reqwest::Client,
}

impl RepoClient {
    /// `origin` is a bare API origin, such as `https://openagents.com`.
    pub fn new(origin: &str, token: Option<Secret>) -> Self {
        Self {
            origin: origin.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    fn headers(&self, idempotency_key: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(token) = &self.token {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", token.expose())) {
                map.insert(AUTHORIZATION, value);
            }
        }
        if let Some(key) = idempotency_key {
            if let Ok(value) = HeaderValue::from_str(key) {
                map.insert("idempotency-key", value);
            }
        }
        map
    }

    /// Issue one request and refuse on anything the caller did not admit.
    ///
    /// The refusal carries the server's own status, code, and request id. A
    /// caller that fell back to an empty list here would report "no
    /// repositories" for a token the server rejected.
    async fn request(
        &self,
        operation: &str,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        idempotency_key: Option<&str>,
        admitted: &[u16],
    ) -> Result<serde_json::Value, AuthError> {
        let url = format!("{}{}", self.origin, path);
        let mut builder = self
            .http
            .request(method, &url)
            .headers(self.headers(idempotency_key));
        if let Some(value) = body {
            builder = builder.json(&value);
        }
        let response = builder.send().await.map_err(|error| {
            AuthError::new(format!("could not {operation} at {}: {error}", self.origin))
        })?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        if !admitted.contains(&status) {
            return Err(AuthError::new(format!(
                "could not {operation} ({status}{})",
                api_error_detail(&value)
            )));
        }
        Ok(value)
    }

    fn decode<T: serde::de::DeserializeOwned>(
        operation: &str,
        value: serde_json::Value,
    ) -> Result<T, AuthError> {
        serde_json::from_value(value).map_err(|error| {
            AuthError::new(format!(
                "the API response did not match the {operation} contract: {error}"
            ))
        })
    }

    pub async fn authenticated_user(&self) -> Result<AuthenticatedUser, AuthError> {
        let value = self
            .request(
                "read the authenticated user",
                reqwest::Method::GET,
                "/api/v1/user",
                None,
                None,
                &[200],
            )
            .await?;
        Self::decode("read authenticated user", value)
    }

    pub async fn list(
        &self,
        namespace: Option<&str>,
        limit: u32,
        after: Option<&str>,
    ) -> Result<RepositoryList, AuthError> {
        if !(1..=100).contains(&limit) {
            return Err(AuthError::new("--limit must be between 1 and 100"));
        }
        let mut query = format!("per_page={limit}");
        if let Some(namespace) = namespace {
            query.push_str(&format!("&namespace={}", validate_owner(namespace)?));
        }
        if let Some(after) = after {
            query.push_str(&format!("&after={}", urlencode(after)));
        }
        let value = self
            .request(
                "list repositories",
                reqwest::Method::GET,
                &format!("/api/v1/user/repos?{query}"),
                None,
                None,
                &[200],
            )
            .await?;
        Self::decode("list repositories", value)
    }

    pub async fn view(&self, owner: &str, repo: &str) -> Result<Repository, AuthError> {
        let owner = validate_owner(owner)?;
        let repo = validate_repository_name(repo)?;
        let value = self
            .request(
                &format!("view {owner}/{repo}"),
                reqwest::Method::GET,
                &format!("/api/v1/repos/{}/{}", urlencode(&owner), urlencode(&repo)),
                None,
                None,
                &[200],
            )
            .await?;
        Self::decode("view repository", value)
    }

    pub async fn remove(&self, owner: &str, repo: &str) -> Result<(), AuthError> {
        let owner = validate_owner(owner)?;
        let repo = validate_repository_name(repo)?;
        self.request(
            &format!("delete {owner}/{repo}"),
            reqwest::Method::DELETE,
            &format!("/api/v1/repos/{}/{}", urlencode(&owner), urlencode(&repo)),
            None,
            None,
            &[200, 202, 204],
        )
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        owner: Option<&str>,
        name: &str,
        private: bool,
        description: Option<&str>,
        default_branch: &str,
        wait: Duration,
    ) -> Result<Repository, AuthError> {
        let name = validate_repository_name(name)?;
        let owner = owner.map(validate_owner).transpose()?;
        let mut body = serde_json::json!({
            "name": name,
            "private": private,
            "default_branch": default_branch,
        });
        if let Some(description) = description {
            body["description"] = serde_json::Value::String(description.to_string());
        }
        let path = match &owner {
            None => "/api/v1/user/repos".to_string(),
            Some(owner) => format!("/api/v1/orgs/{}/repos", urlencode(owner)),
        };
        let value = self
            .request(
                "create the repository",
                reqwest::Method::POST,
                &path,
                Some(body),
                Some(&idempotency_key()),
                &[200, 201, 202],
            )
            .await?;
        let repository: Repository = Self::decode("create repository", value)?;
        if repository.lifecycle_state == "ready" || wait.is_zero() {
            return Ok(repository);
        }
        self.wait_for_repository(&repository.owner.login, &repository.name, wait)
            .await
    }

    async fn wait_for_repository(
        &self,
        owner: &str,
        repo: &str,
        wait: Duration,
    ) -> Result<Repository, AuthError> {
        let started = Instant::now();
        loop {
            let repository = self.view(owner, repo).await?;
            match repository.lifecycle_state.as_str() {
                "ready" => return Ok(repository),
                "failed" => {
                    return Err(AuthError::new(format!(
                        "provisioning failed for {owner}/{repo}{}",
                        repository
                            .provision_error_code
                            .map(|code| format!(": {code}"))
                            .unwrap_or_default()
                    )))
                }
                _ => {}
            }
            if started.elapsed() >= wait {
                return Err(AuthError::new(format!(
                    "{owner}/{repo} is still provisioning after {} s. Provisioning continues on the server",
                    wait.as_secs()
                )));
            }
            eprintln!(
                "Repository provisioning: {} ({}s elapsed).",
                repository.lifecycle_state,
                started.elapsed().as_secs()
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn import(
        &self,
        owner: Option<&str>,
        source: &str,
        name: Option<&str>,
        private: Option<bool>,
        wait: Duration,
    ) -> Result<(Repository, RepositoryImport), AuthError> {
        let (source_owner, source_repo) = parse_repository_target(source)?;
        let owner = owner.map(validate_owner).transpose()?;
        let name = name.map(validate_repository_name).transpose()?;
        let mut body = serde_json::json!({
            "source": { "provider": "github", "repository": format!("{source_owner}/{source_repo}") },
        });
        if let Some(private) = private {
            body["private"] = serde_json::Value::Bool(private);
        }
        if let Some(name) = &name {
            body["name"] = serde_json::Value::String(name.clone());
        }
        let path = match &owner {
            None => "/api/v1/user/repos/imports".to_string(),
            Some(owner) => format!("/api/v1/orgs/{}/repos/imports", urlencode(owner)),
        };
        let value = self
            .request(
                "import the repository",
                reqwest::Method::POST,
                &path,
                Some(body),
                Some(&idempotency_key()),
                &[200, 201, 202],
            )
            .await?;
        let repository: Repository = Self::decode("import repository", value.clone())?;
        let repository_import: RepositoryImport = serde_json::from_value(
            value
                .get("import")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
        .map_err(|error| {
            AuthError::new(format!(
                "the API response did not match the import repository contract: {error}"
            ))
        })?;
        if repository_import.state == "completed" || wait.is_zero() {
            return Ok((repository, repository_import));
        }
        self.wait_for_import(&repository_import.id, wait).await
    }

    async fn wait_for_import(
        &self,
        import_id: &str,
        wait: Duration,
    ) -> Result<(Repository, RepositoryImport), AuthError> {
        let started = Instant::now();
        loop {
            let value = self
                .request(
                    "read the repository import",
                    reqwest::Method::GET,
                    &format!("/api/v1/repository-imports/{}", urlencode(import_id)),
                    None,
                    None,
                    &[200],
                )
                .await?;
            let repository: Repository = Self::decode(
                "read repository import",
                value
                    .get("repository")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )?;
            let repository_import: RepositoryImport = Self::decode(
                "read repository import",
                value
                    .get("import")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )?;
            match repository_import.state.as_str() {
                "completed" => return Ok((repository, repository_import)),
                "failed" => {
                    return Err(AuthError::new(format!(
                        "repository import {import_id} failed{}",
                        repository_import
                            .error_code
                            .map(|code| format!(": {code}"))
                            .unwrap_or_default()
                    )))
                }
                _ => {}
            }
            if started.elapsed() >= wait {
                return Err(AuthError::new(format!(
                    "repository import {import_id} is still running after {} s. The import continues on the server",
                    wait.as_secs()
                )));
            }
            eprintln!(
                "Repository import: {} (shallow snapshot, attempt {}, {}s elapsed).",
                repository_import.state,
                repository_import.attempt_count,
                started.elapsed().as_secs()
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// The repository and the URL to clone it from, after checking that the URL
    /// the API returned is on the origin this invocation is talking to. A clone
    /// URL pointing elsewhere would send the credential helper's token to
    /// whatever host the response named.
    pub async fn clone_info(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<(Repository, String), AuthError> {
        let repository = self.view(owner, repo).await?;
        let url = reqwest::Url::parse(&repository.clone_url).map_err(|error| {
            AuthError::new(format!("the API returned an invalid clone URL: {error}"))
        })?;
        let expected = format!(
            "/{}/{}.git",
            urlencode(&repository.owner.login),
            urlencode(&repository.name)
        );
        let origin_matches = url_origin(&url)
            .map(|value| value == self.origin)
            .unwrap_or(false);
        if !origin_matches
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != expected
        {
            return Err(AuthError::new(
                "the API returned a clone URL outside the selected OpenAgents origin",
            ));
        }
        Ok((repository, url.to_string()))
    }
}

fn url_origin(url: &reqwest::Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
        None => format!("{}://{}", url.scheme(), host),
    })
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A fresh idempotency key, so a retried mutation does not create a second
/// repository. Derived from the clock and the process, not from a constant: a
/// hardcoded key would make every machine's create collide with every other's.
fn idempotency_key() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("oa-{:x}-{:x}", std::process::id(), nanos)
}

// ---------------------------------------------------------------------------
// git
// ---------------------------------------------------------------------------

/// Quote a value for the shell git runs a `!`-prefixed helper through.
fn shell_argument(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./:@=-".contains(&byte));
    if plain {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

/// The path of the running binary, which is the program the credential helper
/// names.
///
/// A bare `oa` would be resolved by the shell against `PATH`, and on a machine
/// that also has an older `oa` installed — the common case while this port
/// lands — git would run that one instead, which does not understand
/// `--api-url` and answers nothing. Naming the path makes the helper this CLI.
pub fn cli_program_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.canonicalize().ok())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "oa".to_string())
}

/// The git credential helper line this CLI installs.
///
/// The `!` makes git run it as a shell command with the operation appended, so
/// `credential.<origin>.helper` resolves to `<oa> --api-url <origin> auth
/// git-credential get`.
pub fn credential_helper_command(origin: &str) -> String {
    format!(
        "!{} --api-url {origin} auth git-credential",
        shell_argument(&cli_program_path())
    )
}

pub fn credential_helper_key(origin: &str) -> String {
    format!("credential.{origin}.helper")
}

fn run_git_sync(args: &[&str], directory: Option<&Path>) -> Result<(i32, String), AuthError> {
    let mut command = SyncCommand::new("git");
    if let Some(directory) = directory {
        command.arg("-C").arg(directory);
    }
    command.args(args);
    let output = command
        .output()
        .map_err(|error| AuthError::new(format!("could not run git: {error}")))?;
    Ok((
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
    ))
}

/// Write `credential.<origin>.helper` into the local or global git config.
///
/// `directory` selects the checkout for `--local`; `None` means the working
/// directory, which is what `oa auth setup-git --local` wants.
pub fn configure_credential_helper(
    origin: &str,
    scope: &str,
    directory: Option<&Path>,
) -> Result<(), AuthError> {
    let scope_flag = if scope == "local" {
        "--local"
    } else {
        "--global"
    };
    let key = credential_helper_key(origin);
    let (reset, _) = run_git_sync(
        &["config", scope_flag, "--replace-all", &key, ""],
        directory,
    )?;
    if reset != 0 {
        return Err(AuthError::new(format!(
            "git config exited with status {reset}. Run oa auth setup-git --local inside a git repository"
        )));
    }
    let helper = credential_helper_command(origin);
    let (added, _) = run_git_sync(&["config", scope_flag, "--add", &key, &helper], directory)?;
    if added != 0 {
        return Err(AuthError::new(format!(
            "git config exited with status {added}"
        )));
    }
    Ok(())
}

/// Whether the helper is configured locally, globally, or not at all.
pub fn credential_helper_state(origin: &str, directory: Option<&Path>) -> (bool, bool) {
    let expected = credential_helper_command(origin);
    let key = credential_helper_key(origin);
    let configured = |scope: &str| {
        run_git_sync(&["config", scope, "--get-all", &key], directory)
            .map(|(code, out)| code == 0 && out.lines().any(|line| line == expected))
            .unwrap_or(false)
    };
    (configured("--local"), configured("--global"))
}

/// `git clone` with this CLI wired in as the only credential helper for the
/// origin, so a private repository clones without any other credential present.
pub fn git_clone_argv(url: &str, directory: Option<&str>) -> Vec<String> {
    let origin = reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| url_origin(&parsed))
        .unwrap_or_default();
    let mut argv = vec![
        "-c".to_string(),
        "credential.helper=".to_string(),
        "-c".to_string(),
        format!(
            "{}={}",
            credential_helper_key(&origin),
            credential_helper_command(&origin)
        ),
        "clone".to_string(),
        "--".to_string(),
        url.to_string(),
    ];
    if let Some(directory) = directory {
        argv.push(directory.to_string());
    }
    argv
}

pub async fn git_clone(url: &str, directory: Option<&str>) -> Result<(), AuthError> {
    let status = Command::new("git")
        .args(git_clone_argv(url, directory))
        .status()
        .await
        .map_err(|error| AuthError::new(format!("could not run git: {error}")))?;
    if !status.success() {
        return Err(AuthError::new(format!(
            "git clone exited with status {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

pub fn parse_git_remotes(output: &str) -> Vec<(String, String)> {
    let mut remotes: Vec<(String, String)> = Vec::new();
    for line in output.lines() {
        let mut fields = line.split_whitespace();
        let (Some(name), Some(url)) = (fields.next(), fields.next()) else {
            continue;
        };
        if !remotes.iter().any(|(existing, _)| existing == name) {
            remotes.push((name.to_string(), url.to_string()));
        }
    }
    remotes
}

/// The `OWNER/REPO` a remote URL names, when the URL is a repository on `origin`.
///
/// A remote's *name* is a local convention — this project names the forge
/// `openagents` and reserves `origin` for the GitHub mirror, other checkouts do
/// the reverse — so the URL is what decides. A mirror is never inferred.
pub fn repository_from_remote_url(origin: &str, remote_url: &str) -> Result<String, AuthError> {
    let url = reqwest::Url::parse(remote_url)
        .map_err(|_| AuthError::new("that git remote URL is not an OpenAgents repository URL"))?;
    let parts: Vec<&str> = url.path().split('/').collect();
    let matches_origin = url_origin(&url)
        .map(|value| value == origin)
        .unwrap_or(false);
    if !matches_origin
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || parts.len() != 3
        || !parts[2].ends_with(".git")
    {
        return Err(AuthError::new(
            "that git remote URL is not an OpenAgents repository URL",
        ));
    }
    let owner = parts[1];
    let repo = &parts[2][..parts[2].len() - 4];
    if owner.is_empty() || repo.is_empty() {
        return Err(AuthError::new(
            "that git remote URL is not an OpenAgents repository URL",
        ));
    }
    Ok(format!("{owner}/{repo}"))
}

/// The repository this checkout belongs to, or a refusal that names what it
/// looked at. Never a guess.
pub fn infer_repository(origin: &str, directory: Option<&Path>) -> Result<String, AuthError> {
    let (code, listed) = run_git_sync(&["remote", "-v"], directory)?;
    if code != 0 {
        return Err(AuthError::new(
            "could not read the git remotes of this directory. Pass OWNER/REPO instead",
        ));
    }
    let remotes = parse_git_remotes(&listed);
    if remotes.is_empty() {
        return Err(AuthError::new(format!(
            "this checkout has no git remotes. Pass OWNER/REPO, or add a remote for {origin}"
        )));
    }
    // Prefer the forge remote by name only as a tie-break among admitted URLs.
    let mut ordered: Vec<&(String, String)> = Vec::new();
    for preferred in ["openagents", "origin", "upstream"] {
        if let Some(remote) = remotes.iter().find(|(name, _)| name == preferred) {
            ordered.push(remote);
        }
    }
    for remote in &remotes {
        if !ordered.iter().any(|(name, _)| *name == remote.0) {
            ordered.push(remote);
        }
    }
    let mut rejected: Vec<String> = Vec::new();
    for (name, url) in ordered {
        match repository_from_remote_url(origin, url) {
            Ok(repository) => return Ok(repository),
            Err(_) => rejected.push(format!("{name} {url}")),
        }
    }
    Err(AuthError::new(format!(
        "no git remote of this checkout is a repository on {origin}: {}. \
         A remote's name does not decide this; its URL does. Pass OWNER/REPO instead",
        rejected.join("; ")
    )))
}

// ---------------------------------------------------------------------------
// git credential helper protocol
// ---------------------------------------------------------------------------

/// Parse git's `key=value` credential request, keeping only the fields that
/// decide admission.
pub fn parse_git_credential_request(input: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    for line in input.split(['\n', '\r']) {
        let Some(separator) = line.find('=') else {
            continue;
        };
        if separator == 0 {
            continue;
        }
        let key = &line[..separator];
        let value = &line[separator + 1..];
        if matches!(key, "protocol" | "host" | "path") {
            fields.push((key.to_string(), value.to_string()));
        }
    }
    fields
}

/// Whether this request is for the endpoint the CLI holds a token for.
///
/// git asks every configured helper for every host. Answering one for
/// `github.com` would hand an OpenAgents token to GitHub.
pub fn admitted_credential_request(origin: &str, fields: &[(String, String)]) -> bool {
    let Ok(url) = reqwest::Url::parse(origin) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let authority = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    let field = |key: &str| {
        fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    field("protocol") == Some(url.scheme()) && field("host") == Some(authority.as_str())
}

/// The answer written on stdout for an admitted `get`. The username is ignored
/// by the forge; the token travels as the password.
pub fn credential_answer(token: &Secret) -> String {
    format!("username=openagents\npassword={}\n\n", token.expose())
}

/// Run the helper protocol against a store.
///
/// Returns the bytes to write on stdout, which is empty for every case that is
/// not an admitted `get` holding a token. Silence is the protocol's way of
/// saying "I have nothing", and it is the only honest answer when there is no
/// credential: an invented one would make git retry against the server with a
/// password that was never issued.
pub fn run_git_credential_helper(
    origin: &str,
    operation: &str,
    input: &str,
    store: &crate::auth::CredentialStore,
) -> Result<String, AuthError> {
    if input.len() > 8_192 {
        return Err(AuthError::new(
            "the git credential request exceeded 8192 bytes",
        ));
    }
    let fields = parse_git_credential_request(input);
    if !admitted_credential_request(origin, &fields) {
        return Ok(String::new());
    }
    match operation {
        "erase" => {
            store.remove()?;
            Ok(String::new())
        }
        "store" => Ok(String::new()),
        "get" => match store.find_token()? {
            Some(stored) => Ok(credential_answer(&stored.token)),
            None => Ok(String::new()),
        },
        other => Err(AuthError::new(format!(
            "unknown git credential operation {other}. Use get, store, or erase"
        ))),
    }
}

/// Read git's request from stdin, bounded.
pub fn read_credential_stdin() -> Result<String, AuthError> {
    let mut buffer = Vec::new();
    std::io::stdin()
        .take(8_193)
        .read_to_end(&mut buffer)
        .map_err(|error| {
            AuthError::new(format!(
                "the credential helper could not read git input: {error}"
            ))
        })?;
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_targets_are_validated() {
        assert_eq!(
            validate_repository_name("OpenAgents").unwrap(),
            "openagents"
        );
        assert_eq!(
            validate_repository_name("open.agents").unwrap(),
            "open.agents"
        );
        assert!(validate_repository_name("open.").is_err());
        assert!(validate_repository_name("-open").is_err());
        assert!(validate_repository_name("").is_err());
        assert_eq!(
            parse_repository_target("OpenAgentsInc/openagents").unwrap(),
            ("OpenAgentsInc".to_string(), "openagents".to_string())
        );
        assert!(parse_repository_target("openagents").is_err());
        assert!(parse_repository_target("a/b/c").is_err());
    }

    #[test]
    fn only_the_selected_origin_is_admitted() {
        let fields = parse_git_credential_request("protocol=https\nhost=openagents.com\n\n");
        assert!(admitted_credential_request(
            "https://openagents.com",
            &fields
        ));
        assert!(!admitted_credential_request(
            "https://staging.openagents.com",
            &fields
        ));
        let github = parse_git_credential_request("protocol=https\nhost=github.com\n");
        assert!(!admitted_credential_request(
            "https://openagents.com",
            &github
        ));
    }

    #[test]
    fn remote_urls_decide_the_repository_not_remote_names() {
        assert_eq!(
            repository_from_remote_url(
                "https://openagents.com",
                "https://openagents.com/OpenAgentsInc/openagents.git"
            )
            .unwrap(),
            "OpenAgentsInc/openagents"
        );
        assert!(repository_from_remote_url(
            "https://openagents.com",
            "https://github.com/OpenAgentsInc/openagents.git"
        )
        .is_err());
        assert!(repository_from_remote_url(
            "https://openagents.com",
            "https://openagents.com/OpenAgentsInc/openagents"
        )
        .is_err());
    }

    #[test]
    fn clone_argv_pins_this_cli_as_the_only_helper() {
        let argv = git_clone_argv("https://openagents.com/a/b.git", Some("dest"));
        assert_eq!(argv[0], "-c");
        assert_eq!(argv[1], "credential.helper=");
        assert_eq!(
            argv[3],
            format!(
                "credential.https://openagents.com.helper={}",
                credential_helper_command("https://openagents.com")
            )
        );
        // The helper names this binary, not a bare `oa` the shell would resolve
        // against PATH — where an older install would answer instead.
        assert!(argv[3].contains(&cli_program_path()), "{}", argv[3]);
        assert_eq!(argv[argv.len() - 1], "dest");
    }
}
