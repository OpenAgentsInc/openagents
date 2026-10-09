//! Connected GitHub repositories and projects (docs/auth/github.md,
//! "Repository access"; docs/web/sidebar.md, "Projects and repositories").
//!
//! Sign-in keeps no GitHub token. Connecting repositories is a second trip
//! to GitHub through the same OAuth App and callback that asks for more
//! only then: `repo` and `read:org` to see private and organization
//! repositories, or nothing beyond sign-in for public ones
//! ([`crate::flow::Purpose::Repos`]). The token that trip returns is kept
//! here, encrypted with AES-256-GCM under the App file's
//! `token_encryption_key`, bound to the account and GitHub user it belongs
//! to, and never logged or shown.
//!
//! A project is an account's connected repository: an id, a name, the
//! repository's GitHub id and full name, its default branch, and when it
//! was added. Projects stay when the token is revoked; the status then
//! says to reconnect.
//!
//! One small file per account under `github-access/` beside the account
//! stores, written whole with a rename under a per-account lock. The
//! account service's routes call [`answer`].

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tenancy::Accounts;
use tenancy::accounts::identities::github_principal;

use crate::AuthError;
use crate::cache::Lists;
use crate::github::{Api, ApiFault, Github, Secret, Sso};
use crate::service::CodeRequest;

/// The directory beside `accounts.json` the per-account files live in.
pub const STORE_DIR: &str = "github-access";
const SCHEMA: &str = "openagents.github-access.v1";
/// The most projects one account keeps.
pub const MAX_PROJECTS: usize = 200;
const FILE_MAX: u64 = 1024 * 1024;

/// An account's project: one connected GitHub repository.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// `prj_` and 16 hex characters.
    pub id: String,
    /// Shown in the sidebar: the repository's name.
    pub name: String,
    /// GitHub's numeric repository id (stable across renames).
    pub repository_id: u64,
    /// `owner/name`.
    pub repository: String,
    pub default_branch: String,
    pub private: bool,
    pub created_unix: u64,
}

/// One repository the person can connect.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Repository {
    pub id: u64,
    pub full_name: String,
    pub default_branch: String,
    pub private: bool,
    /// Read-only on GitHub: it can be read, not pushed to.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
}

/// One page of the repositories the person can connect.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Listing {
    pub repositories: Vec<Repository>,
    /// GitHub names a next page (`Link: rel="next"`).
    #[serde(default)]
    pub more: bool,
    /// GitHub left out repositories of organizations that use single
    /// sign-on until the person authorizes OpenAgents for them.
    #[serde(default)]
    pub sso_hidden: bool,
}

/// Where the account's GitHub repository access stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Access {
    /// Never connected, or disconnected.
    None,
    /// Connected as `login`; `private` when private repositories are
    /// included (the token holds `repo`).
    Connected { login: String, private: bool },
    /// GitHub stopped accepting the token: connect again.
    Reconnect { login: String },
}

/// The account's access and projects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Status {
    pub access: Access,
    pub projects: Vec<Project>,
}

impl Status {
    /// The JSON the account service answers with.
    #[must_use]
    pub fn body(&self) -> Value {
        let github = match &self.access {
            Access::None => json!({"state": "none"}),
            Access::Connected { login, private } => {
                json!({"state": "connected", "login": login, "private": private})
            }
            Access::Reconnect { login } => json!({"state": "reconnect", "login": login}),
        };
        json!({"github": github, "projects": self.projects})
    }

    /// Read [`Status::body`] back (the web server's side).
    #[must_use]
    pub fn from_body(body: &Value) -> Option<Self> {
        let github = &body["github"];
        let login = || github["login"].as_str().map(str::to_string);
        let access = match github["state"].as_str()? {
            "none" => Access::None,
            "connected" => Access::Connected {
                login: login()?,
                private: github["private"].as_bool().unwrap_or(false),
            },
            "reconnect" => Access::Reconnect { login: login()? },
            _ => return None,
        };
        let projects: Vec<Project> = serde_json::from_value(body["projects"].clone()).ok()?;
        (projects.len() <= MAX_PROJECTS && projects.iter().all(|p| project_id(&p.id)))
            .then_some(Self { access, projects })
    }
}

/// Why a repository call did not complete. Carries no token or provider
/// detail. Each says what actually went wrong: "isn't answering" is only
/// for GitHub not answering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoError {
    /// No repository access yet.
    NotConnected,
    /// GitHub no longer accepts the stored token.
    Reconnect,
    /// The GitHub account that granted access isn't the one this account
    /// signs in with.
    OtherGithub,
    /// No such repository, or the access doesn't reach it.
    NotFound,
    /// The account has [`MAX_PROJECTS`] projects.
    Full,
    /// GitHub refused the code (expired, reused, canceled).
    Denied,
    /// A malformed request.
    Invalid,
    /// GitHub could not be reached or didn't answer in time.
    Unavailable,
    /// GitHub is over a rate limit for this token (403 or 429).
    RateLimited,
    /// The repository's organization requires single sign-on and the
    /// token isn't authorized for it.
    SsoRequired,
    /// GitHub refused access for another reason (an organization's
    /// OAuth App access restrictions, a disabled repository).
    Forbidden,
    /// GitHub answered with an error of its own (5xx).
    GithubError,
    /// GitHub's answer wasn't what its API sends, or was too large.
    BadAnswer,
    /// This server has no GitHub App or token key set up.
    NotConfigured,
    /// The saved access on this server couldn't be read or written.
    Storage,
}

impl RepoError {
    const ALL: [Self; 15] = [
        Self::NotConnected,
        Self::Reconnect,
        Self::OtherGithub,
        Self::NotFound,
        Self::Full,
        Self::Denied,
        Self::Invalid,
        Self::Unavailable,
        Self::RateLimited,
        Self::SsoRequired,
        Self::Forbidden,
        Self::GithubError,
        Self::BadAnswer,
        Self::NotConfigured,
        Self::Storage,
    ];

    /// A stable code for the HTTP error envelope.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::NotConnected => "github_not_connected",
            Self::Reconnect => "github_reconnect",
            Self::OtherGithub => "github_other_account",
            Self::NotFound => "repository_not_found",
            Self::Full => "too_many_projects",
            Self::Denied => "github_denied",
            Self::Invalid => "invalid_request",
            Self::Unavailable => "github_unavailable",
            Self::RateLimited => "github_rate_limited",
            Self::SsoRequired => "github_sso_required",
            Self::Forbidden => "github_forbidden",
            Self::GithubError => "github_error",
            Self::BadAnswer => "github_bad_answer",
            Self::NotConfigured => "github_not_configured",
            Self::Storage => "github_access_storage",
        }
    }

    /// The HTTP status an account-service route answers with.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::NotConnected | Self::Reconnect | Self::OtherGithub | Self::Full => 409,
            Self::NotFound => 404,
            Self::Denied => 401,
            Self::Invalid => 400,
            Self::SsoRequired | Self::Forbidden => 403,
            Self::RateLimited => 429,
            Self::GithubError | Self::BadAnswer => 502,
            Self::Unavailable | Self::NotConfigured => 503,
            Self::Storage => 500,
        }
    }

    /// Read back from an account-service error code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|error| error.code() == code)
    }
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotConnected => "Connect GitHub to see your repositories.",
            Self::Reconnect => "GitHub access ended. Connect GitHub again.",
            Self::OtherGithub => "Use the GitHub account you sign in with.",
            Self::NotFound => "That repository wasn't found.",
            Self::Full => "You have as many projects as you can add. Remove one first.",
            Self::Denied => "GitHub didn't connect. Try again.",
            Self::Invalid => "That didn't work. Try again.",
            Self::Unavailable => "GitHub isn't answering right now. Try again in a minute.",
            Self::RateLimited => {
                "GitHub is limiting how often OpenAgents can read your repositories. Try again in a few minutes."
            }
            Self::SsoRequired => {
                "That organization uses single sign-on. On GitHub, authorize OpenAgents for the organization, then try again."
            }
            Self::Forbidden => {
                "GitHub didn't allow access to that repository. An organization owner may need to approve OpenAgents on GitHub."
            }
            Self::GithubError => "GitHub had a problem answering. Try again in a minute.",
            Self::BadAnswer => "GitHub sent an answer OpenAgents couldn't read. Try again in a minute.",
            Self::NotConfigured => "GitHub repositories aren't set up on this server.",
            Self::Storage => "Your saved GitHub access couldn't be read. Try again in a minute.",
        })
    }
}

impl std::error::Error for RepoError {}

impl From<AuthError> for RepoError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Denied => Self::Denied,
            AuthError::Taken | AuthError::AlreadyLinked => Self::OtherGithub,
            AuthError::Unavailable => Self::Unavailable,
        }
    }
}

impl From<ApiFault> for RepoError {
    fn from(fault: ApiFault) -> Self {
        match fault {
            ApiFault::Unreachable => Self::Unavailable,
            ApiFault::TooLarge => Self::BadAnswer,
        }
    }
}

/// One account-service call.
#[derive(Debug)]
pub enum Call {
    /// `GET /v1/account/github`
    Status,
    /// `POST /v1/account/github/grant` `{code, code_verifier}`
    Grant(CodeRequest),
    /// `DELETE /v1/account/github/grant`
    Disconnect,
    /// `GET /v1/account/github/repositories?page=N`: one page, most
    /// recently pushed first.
    Repositories(u32),
    /// `POST /v1/account/github/token`: the token, for this deployment's
    /// own web server to read GitHub as the person.
    Token,
    /// `POST /v1/account/projects` `{repository}`
    AddProject(String),
    /// `DELETE /v1/account/projects/{id}`
    RemoveProject(String),
}

/// Answer `call` for `account`: an HTTP status and a JSON body (an error
/// envelope on failure). `github` is `None` when the service has no
/// GitHub App; only [`Call::Status`] and removals work then.
pub async fn answer(
    dir: &Path,
    github: Option<&Github>,
    account: &str,
    call: Call,
) -> (u16, Value) {
    let result = async {
        let github = || github.ok_or(RepoError::NotConfigured);
        Ok::<Value, RepoError>(match call {
            Call::Status => status(dir, account)?.body(),
            Call::Grant(request) => grant(dir, github()?, account, &request).await?.body(),
            Call::Disconnect => disconnect(dir, account)?.body(),
            Call::Repositories(page) => {
                serde_json::to_value(repositories(dir, github()?, account, page).await?)
                    .map_err(|_| RepoError::BadAnswer)?
            }
            Call::Token => {
                let (token, private) = token(dir, github()?, account)?;
                json!({"token": token.as_str(), "private": private})
            }
            Call::AddProject(repository) => {
                json!({"project": add_project(dir, github()?, account, &repository).await?})
            }
            Call::RemoveProject(id) => {
                remove_project(dir, account, &id)?;
                json!({"removed": id})
            }
        })
    }
    .await;
    match result {
        Ok(body) => (200, body),
        Err(error) => (
            error.status(),
            json!({"error": {"code": error.code(), "message": error.to_string()}}),
        ),
    }
}

/// The account's access and projects.
pub fn status(dir: &Path, account: &str) -> Result<Status, RepoError> {
    Ok(load(dir, account)?.status())
}

/// Finish a repository-access trip: exchange the code, check the GitHub
/// user is the one this account signs in with, and keep the token
/// encrypted.
pub async fn grant(
    dir: &Path,
    github: &Github,
    account: &str,
    request: &CodeRequest,
) -> Result<Status, RepoError> {
    check_account(account)?;
    let token = github
        .exchange(&request.code, &request.code_verifier)
        .await?;
    let user = github.api(token.as_str(), "/user").await?;
    let user = match user.status {
        // The token was just issued: GitHub refusing it is a failed
        // connect, not a stored token to mark revoked.
        401 => return Err(RepoError::Denied),
        _ => answered(user)?,
    };
    let github_id = user.body["id"].as_u64().ok_or(RepoError::BadAnswer)?;
    let login = user.body["login"]
        .as_str()
        .filter(|login| {
            !login.is_empty()
                && login.len() <= 64
                && login
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .ok_or(RepoError::BadAnswer)?
        .to_string();
    same_person(dir, account, github_id)?;
    let scopes = user.scopes.unwrap_or_default();
    let sealed = seal(github, account, github_id, token.as_str())?;
    mutate(dir, account, |record| {
        record.grant = Some(Grant {
            github_id,
            login,
            scopes,
            sealed,
            granted_unix: now(),
            revoked_unix: None,
        });
        Ok(())
    })?;
    forget(dir, account);
    status(dir, account)
}

/// Forget the stored token. Projects stay.
pub fn disconnect(dir: &Path, account: &str) -> Result<Status, RepoError> {
    mutate(dir, account, |record| {
        record.grant = None;
        Ok(())
    })?;
    forget(dir, account);
    status(dir, account)
}

/// Repositories listed per page: one quick GitHub call.
pub const PAGE_SIZE: usize = 30;

/// The last page offered ([`PAGE_SIZE`] × this many repositories in all).
pub const MAX_PAGE: u32 = 20;

/// One page (1-based) of the repositories the person can connect, most
/// recently pushed first, and whether GitHub names a page after it (its
/// `Link: rel="next"`). One call to GitHub, so the page shows quickly; the
/// next page loads on request.
///
/// Pages are cached per account and grant ([`crate::cache`]: fresh for 5
/// minutes, kept for an hour and served while one read refreshes them).
/// Connecting, disconnecting, adding or removing a project, and a GitHub
/// 401 drop the account's pages. While the token's hourly budget is nearly
/// spent, kept pages are served without reading GitHub again.
pub async fn repositories(
    dir: &Path,
    github: &Github,
    account: &str,
    page: u32,
) -> Result<Listing, RepoError> {
    let page = page.clamp(1, MAX_PAGE);
    let grant = current(dir, account)?;
    let group = (dir.to_path_buf(), account.to_string());
    let item = (grant.github_id, grant.granted_unix, page);
    let may_refresh = !budget_low(dir, account);
    let (github, dir, account) = (github.clone(), dir.to_path_buf(), account.to_string());
    LISTS
        .get(
            group,
            item,
            may_refresh,
            move || async move { read_page(&dir, &github, &account, page).await },
            RepoError::Unavailable,
        )
        .await
}

/// One page straight from GitHub (no cache).
async fn read_page(
    dir: &Path,
    github: &Github,
    account: &str,
    page: u32,
) -> Result<Listing, RepoError> {
    let (token, _) = token(dir, github, account)?;
    let answer = github
        .api(
            token.as_str(),
            &format!(
                "/user/repos?per_page={PAGE_SIZE}&page={page}&sort=pushed&affiliation=owner,collaborator,organization_member"
            ),
        )
        .await?;
    note_budget(dir, account, &answer);
    let answer = checked(dir, account, answer)?;
    let rows = answer.body.as_array().ok_or(RepoError::BadAnswer)?;
    Ok(Listing {
        repositories: rows.iter().filter_map(listed).collect(),
        more: answer.next.is_some() && page < MAX_PAGE,
        sso_hidden: answer.sso == Some(Sso::Partial),
    })
}

/// Pages by account (the stores directory and account id), then by grant
/// (GitHub user id and when it was granted) and page.
type PageCache = Lists<(PathBuf, String), (u64, u64, u32), Listing>;

static LISTS: LazyLock<PageCache> = LazyLock::new(Lists::new);

/// Below this many reads left in the hour, kept pages are served instead
/// of reading GitHub again.
const LOW_BUDGET: u64 = 100;

/// The last `x-ratelimit-remaining` and `x-ratelimit-reset` per account.
static BUDGETS: LazyLock<Mutex<HashMap<(PathBuf, String), (u64, u64)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn budgets() -> std::sync::MutexGuard<'static, HashMap<(PathBuf, String), (u64, u64)>> {
    BUDGETS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn note_budget(dir: &Path, account: &str, answer: &Api) {
    if let (Some(remaining), Some(reset)) = (answer.remaining, answer.reset) {
        let mut budgets = budgets();
        if budgets.len() >= 4096 {
            let now = now();
            budgets.retain(|_, (_, reset)| *reset > now);
        }
        budgets.insert((dir.to_path_buf(), account.to_string()), (remaining, reset));
    }
}

fn budget_low(dir: &Path, account: &str) -> bool {
    budgets()
        .get(&(dir.to_path_buf(), account.to_string()))
        .is_some_and(|(remaining, reset)| *remaining < LOW_BUDGET && *reset > now())
}

/// Drop the account's cached pages.
fn forget(dir: &Path, account: &str) {
    LISTS.remove(&(dir.to_path_buf(), account.to_string()));
}

/// Make the account's cached pages `by` older (tests: step past the
/// freshness window or the hour they are kept without waiting).
#[doc(hidden)]
pub fn age_cached_lists(dir: &Path, account: Option<&str>, by: Duration) {
    LISTS.age_where(by, |(stores, owner)| {
        stores == dir && account.is_none_or(|account| owner == account)
    });
}

/// The account's grant, when it is connected and not revoked.
fn current(dir: &Path, account: &str) -> Result<Grant, RepoError> {
    let record = load(dir, account)?;
    let grant = record.grant.ok_or(RepoError::NotConnected)?;
    if grant.revoked_unix.is_some() {
        return Err(RepoError::Reconnect);
    }
    Ok(grant)
}

/// The stored token and whether it reaches private repositories.
pub(crate) fn token(
    dir: &Path,
    github: &Github,
    account: &str,
) -> Result<(Secret, bool), RepoError> {
    let grant = current(dir, account)?;
    let token = open(github, account, grant.github_id, &grant.sealed)?;
    Ok((token, grant.private()))
}

/// Add `repository` (`owner/name`) as a project, or return the project
/// that already holds it.
pub async fn add_project(
    dir: &Path,
    github: &Github,
    account: &str,
    repository: &str,
) -> Result<Project, RepoError> {
    if !full_name(repository) {
        return Err(RepoError::Invalid);
    }
    let (token, _) = token(dir, github, account)?;
    let answer = github
        .api(token.as_str(), &format!("/repos/{repository}"))
        .await?;
    let body = checked(dir, account, answer)?.body;
    if body["disabled"].as_bool() == Some(true) {
        return Err(RepoError::Forbidden);
    }
    let found = listed(&body).ok_or(RepoError::BadAnswer)?;
    let mut id = [0u8; 8];
    OsRng.fill_bytes(&mut id);
    let fresh = Project {
        id: format!("prj_{}", hex(&id)),
        name: found
            .full_name
            .split_once('/')
            .map_or(found.full_name.as_str(), |(_, name)| name)
            .to_string(),
        repository_id: found.id,
        repository: found.full_name.clone(),
        default_branch: found.default_branch.clone(),
        private: found.private,
        created_unix: now(),
    };
    forget(dir, account);
    mutate(dir, account, |record| {
        if let Some(existing) = record
            .projects
            .iter_mut()
            .find(|p| p.repository_id == found.id)
        {
            // A rename or a new default branch on GitHub follows.
            existing.repository.clone_from(&fresh.repository);
            existing.name.clone_from(&fresh.name);
            existing.default_branch.clone_from(&fresh.default_branch);
            existing.private = fresh.private;
            return Ok(existing.clone());
        }
        if record.projects.len() >= MAX_PROJECTS {
            return Err(RepoError::Full);
        }
        record.projects.push(fresh.clone());
        Ok(fresh.clone())
    })
}

/// Remove a project. Removing one that isn't there is not an error.
pub fn remove_project(dir: &Path, account: &str, id: &str) -> Result<(), RepoError> {
    if !project_id(id) {
        return Err(RepoError::Invalid);
    }
    forget(dir, account);
    mutate(dir, account, |record| {
        record.projects.retain(|p| p.id != id);
        Ok(())
    })
}

/// Whether `value` looks like a project id.
#[must_use]
pub fn project_id(value: &str) -> bool {
    value
        .strip_prefix("prj_")
        .is_some_and(|hex| hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether `value` is a GitHub `owner/name`.
#[must_use]
pub fn full_name(value: &str) -> bool {
    let Some((owner, name)) = value.split_once('/') else {
        return false;
    };
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && !p.starts_with('.')
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    part(owner) && part(name)
}

/// A GitHub answer with a 2xx status, or the error it means. A 401 marks
/// the stored token revoked.
fn checked(dir: &Path, account: &str, answer: Api) -> Result<Api, RepoError> {
    if answer.status == 401 {
        mutate(dir, account, |record| {
            if let Some(grant) = record.grant.as_mut()
                && grant.revoked_unix.is_none()
            {
                grant.revoked_unix = Some(now());
            }
            Ok(())
        })?;
        forget(dir, account);
        return Err(RepoError::Reconnect);
    }
    answered(answer)
}

/// What a GitHub status means, for any status but 401 (which depends on
/// whose token it was). Rate limits and single sign-on come first: GitHub
/// answers both with 403.
fn answered(answer: Api) -> Result<Api, RepoError> {
    if answer.rate_limited {
        return Err(RepoError::RateLimited);
    }
    match answer.status {
        200..=299 if answer.body.is_null() => Err(RepoError::BadAnswer),
        200..=299 => Ok(answer),
        403 if answer.sso == Some(Sso::Required) => Err(RepoError::SsoRequired),
        403 | 451 => Err(RepoError::Forbidden),
        404 => Err(RepoError::NotFound),
        500..=599 => Err(RepoError::GithubError),
        _ => Err(RepoError::BadAnswer),
    }
}

/// A repository from GitHub's JSON. Disabled repositories (GitHub blocks
/// every read of them) are left out; `default_branch` falls back to
/// `main` when GitHub has none to name.
fn listed(value: &Value) -> Option<Repository> {
    if value["disabled"].as_bool() == Some(true) {
        return None;
    }
    let full_name = value["full_name"].as_str().filter(|n| full_name(n))?;
    Some(Repository {
        id: value["id"].as_u64()?,
        full_name: full_name.to_string(),
        default_branch: value["default_branch"]
            .as_str()
            .filter(|b| !b.is_empty() && b.len() <= 255 && !b.chars().any(char::is_control))
            .unwrap_or("main")
            .to_string(),
        private: value["private"].as_bool().unwrap_or(false),
        archived: value["archived"].as_bool().unwrap_or(false),
    })
}

/// The GitHub user who granted access must be the one this account signs
/// in with, when it signs in with GitHub at all.
fn same_person(dir: &Path, account: &str, github_id: u64) -> Result<(), RepoError> {
    let accounts = Accounts::open(dir).map_err(|_| RepoError::Storage)?;
    if let Some(identity) = accounts
        .github_identity(github_id)
        .map_err(|_| RepoError::Storage)?
    {
        return if identity.account == account {
            Ok(())
        } else {
            Err(RepoError::OtherGithub)
        };
    }
    let store = accounts.store().map_err(|_| RepoError::Storage)?;
    let record = store.accounts.get(account).ok_or(RepoError::Invalid)?;
    let principal = github_principal(github_id);
    if record
        .principals
        .iter()
        .any(|p| p.starts_with("github:") && *p != principal)
    {
        return Err(RepoError::OtherGithub);
    }
    Ok(())
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    github_id: u64,
    login: String,
    /// What GitHub said the token holds (`X-OAuth-Scopes`).
    scopes: Vec<String>,
    /// `v1.` and base64 of the 12-byte nonce and the AES-256-GCM
    /// ciphertext. The associated data binds it to the account and the
    /// GitHub user.
    sealed: String,
    granted_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revoked_unix: Option<u64>,
}

impl Grant {
    fn private(&self) -> bool {
        self.scopes.iter().any(|s| s == "repo")
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    v: String,
    account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grant: Option<Grant>,
    #[serde(default)]
    projects: Vec<Project>,
}

impl Record {
    fn new(account: &str) -> Self {
        Self {
            v: SCHEMA.into(),
            account: account.into(),
            grant: None,
            projects: Vec::new(),
        }
    }

    fn status(&self) -> Status {
        let access = match &self.grant {
            None => Access::None,
            Some(grant) if grant.revoked_unix.is_some() => Access::Reconnect {
                login: grant.login.clone(),
            },
            Some(grant) => Access::Connected {
                login: grant.login.clone(),
                private: grant.private(),
            },
        };
        Status {
            access,
            projects: self.projects.clone(),
        }
    }
}

fn associated(account: &str, github_id: u64) -> String {
    format!("{SCHEMA}\0{account}\0{github_id}")
}

fn cipher(github: &Github) -> Result<Aes256Gcm, RepoError> {
    let credentials = github.credentials();
    if !credentials.has_token_key() {
        return Err(RepoError::NotConfigured);
    }
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(
        credentials.token_key(),
    )))
}

fn seal(github: &Github, account: &str, github_id: u64, token: &str) -> Result<String, RepoError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let aad = associated(account, github_id);
    let sealed = cipher(github)?
        .encrypt(
            &nonce,
            Payload {
                msg: token.as_bytes(),
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| RepoError::Storage)?;
    let mut bytes = nonce.to_vec();
    bytes.extend_from_slice(&sealed);
    Ok(format!("v1.{}", STANDARD.encode(bytes)))
}

fn open(github: &Github, account: &str, github_id: u64, sealed: &str) -> Result<Secret, RepoError> {
    let bytes = sealed
        .strip_prefix("v1.")
        .and_then(|b64| STANDARD.decode(b64).ok())
        .filter(|bytes| bytes.len() > 12 + 16)
        .ok_or(RepoError::Storage)?;
    let (nonce, body) = bytes.split_at(12);
    let aad = associated(account, github_id);
    let plain = cipher(github)?
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: body,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| RepoError::Storage)?;
    String::from_utf8(plain)
        .map(Secret::new)
        .map_err(|_| RepoError::Storage)
}

fn check_account(account: &str) -> Result<(), RepoError> {
    if account.is_empty()
        || account.len() > 128
        || account.chars().any(|c| c.is_control() || c == '\0')
    {
        return Err(RepoError::Invalid);
    }
    Ok(())
}

fn file(dir: &Path, account: &str, extension: &str) -> PathBuf {
    let digest = Sha256::digest(account.as_bytes());
    dir.join(STORE_DIR)
        .join(format!("{}.{extension}", hex(&digest)))
}

fn load(dir: &Path, account: &str) -> Result<Record, RepoError> {
    check_account(account)?;
    let path = file(dir, account, "json");
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Record::new(account));
        }
        Err(_) => return Err(RepoError::Storage),
    };
    if !meta.is_file() || meta.len() > FILE_MAX {
        return Err(RepoError::Storage);
    }
    let bytes = std::fs::read(&path).map_err(|_| RepoError::Storage)?;
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| RepoError::Storage)?;
    if record.v != SCHEMA || record.account != account || record.projects.len() > MAX_PROJECTS {
        return Err(RepoError::Storage);
    }
    Ok(record)
}

/// Run `change` on the account's record under its lock and write it back.
fn mutate<T>(
    dir: &Path,
    account: &str,
    change: impl FnOnce(&mut Record) -> Result<T, RepoError>,
) -> Result<T, RepoError> {
    check_account(account)?;
    let store = dir.join(STORE_DIR);
    create_private_dir(&store)?;
    let _lock = Lock::acquire(&file(dir, account, "lock"))?;
    let mut record = load(dir, account)?;
    let out = change(&mut record)?;
    let text = serde_json::to_vec_pretty(&record).map_err(|_| RepoError::Storage)?;
    let temporary = file(dir, account, "tmp");
    std::fs::remove_file(&temporary).ok();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut handle = options.open(&temporary).map_err(|_| RepoError::Storage)?;
    handle
        .write_all(&text)
        .and_then(|()| handle.sync_all())
        .map_err(|_| RepoError::Storage)?;
    std::fs::rename(&temporary, file(dir, account, "json")).map_err(|_| RepoError::Storage)?;
    Ok(out)
}

fn create_private_dir(path: &Path) -> Result<(), RepoError> {
    if path.is_dir() {
        return Ok(());
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| RepoError::Storage)
}

/// A per-account writer lock: `create_new` takes it, dropping removes it.
struct Lock(PathBuf);

impl Lock {
    fn acquire(path: &Path) -> Result<Self, RepoError> {
        for attempt in 0..200 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(_) => return Ok(Self(path.to_path_buf())),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    // A lock older than a minute was left by a crash.
                    if attempt == 0
                        && std::fs::metadata(path)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.elapsed().ok())
                            .is_some_and(|age| age.as_secs() > 60)
                    {
                        std::fs::remove_file(path).ok();
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => return Err(RepoError::Storage),
            }
        }
        Err(RepoError::Storage)
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests;
