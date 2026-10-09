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

use std::io::Write;
use std::path::{Path, PathBuf};

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
use crate::github::{Github, Secret};
use crate::service::CodeRequest;

/// The directory beside `accounts.json` the per-account files live in.
pub const STORE_DIR: &str = "github-access";
const SCHEMA: &str = "openagents.github-access.v1";
/// The most projects one account keeps.
pub const MAX_PROJECTS: usize = 200;
/// The most repositories one listing returns (three GitHub pages).
const MAX_LISTED: usize = 300;
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
}

/// Why a repository call did not complete. Carries no token or provider
/// detail.
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
    /// GitHub, the key, or the store could not be reached.
    Unavailable,
}

impl RepoError {
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
            Self::Unavailable => 503,
        }
    }

    /// Read back from an account-service error code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::NotConnected,
            Self::Reconnect,
            Self::OtherGithub,
            Self::NotFound,
            Self::Full,
            Self::Denied,
            Self::Invalid,
            Self::Unavailable,
        ]
        .into_iter()
        .find(|error| error.code() == code)
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

/// One account-service call.
#[derive(Debug)]
pub enum Call {
    /// `GET /v1/account/github`
    Status,
    /// `POST /v1/account/github/grant` `{code, code_verifier}`
    Grant(CodeRequest),
    /// `DELETE /v1/account/github/grant`
    Disconnect,
    /// `GET /v1/account/github/repositories`
    Repositories,
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
        let github = || github.ok_or(RepoError::Unavailable);
        Ok::<Value, RepoError>(match call {
            Call::Status => status(dir, account)?.body(),
            Call::Grant(request) => grant(dir, github()?, account, &request).await?.body(),
            Call::Disconnect => disconnect(dir, account)?.body(),
            Call::Repositories => {
                json!({"repositories": repositories(dir, github()?, account).await?})
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
    if user.status != 200 {
        return Err(RepoError::Unavailable);
    }
    let github_id = user.body["id"].as_u64().ok_or(RepoError::Unavailable)?;
    let login = user.body["login"]
        .as_str()
        .filter(|login| {
            !login.is_empty()
                && login.len() <= 64
                && login
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .ok_or(RepoError::Unavailable)?
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
    status(dir, account)
}

/// Forget the stored token. Projects stay.
pub fn disconnect(dir: &Path, account: &str) -> Result<Status, RepoError> {
    mutate(dir, account, |record| {
        record.grant = None;
        Ok(())
    })?;
    status(dir, account)
}

/// The repositories the person can connect, most recently pushed first.
pub async fn repositories(
    dir: &Path,
    github: &Github,
    account: &str,
) -> Result<Vec<Repository>, RepoError> {
    let (token, _) = token(dir, github, account)?;
    let mut found = Vec::new();
    for page in 1..=MAX_LISTED / 100 {
        let answer = github
            .api(
                token.as_str(),
                &format!(
                    "/user/repos?per_page=100&page={page}&sort=pushed&affiliation=owner,collaborator,organization_member"
                ),
            )
            .await?;
        let list = checked(dir, account, answer.status, answer.body)?;
        let rows = list.as_array().map_or(&[][..], Vec::as_slice);
        found.extend(rows.iter().filter_map(listed));
        if rows.len() < 100 {
            break;
        }
    }
    found.truncate(MAX_LISTED);
    Ok(found)
}

/// The stored token and whether it reaches private repositories.
pub(crate) fn token(
    dir: &Path,
    github: &Github,
    account: &str,
) -> Result<(Secret, bool), RepoError> {
    let record = load(dir, account)?;
    let grant = record.grant.as_ref().ok_or(RepoError::NotConnected)?;
    if grant.revoked_unix.is_some() {
        return Err(RepoError::Reconnect);
    }
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
    let found = listed(&checked(dir, account, answer.status, answer.body)?)
        .ok_or(RepoError::Unavailable)?;
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

/// A GitHub answer's body, or the error it means. A 401 marks the stored
/// token revoked.
fn checked(dir: &Path, account: &str, status: u16, body: Value) -> Result<Value, RepoError> {
    match status {
        200..=299 => Ok(body),
        401 => {
            mutate(dir, account, |record| {
                if let Some(grant) = record.grant.as_mut()
                    && grant.revoked_unix.is_none()
                {
                    grant.revoked_unix = Some(now());
                }
                Ok(())
            })?;
            Err(RepoError::Reconnect)
        }
        404 => Err(RepoError::NotFound),
        _ => Err(RepoError::Unavailable),
    }
}

fn listed(value: &Value) -> Option<Repository> {
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
    })
}

/// The GitHub user who granted access must be the one this account signs
/// in with, when it signs in with GitHub at all.
fn same_person(dir: &Path, account: &str, github_id: u64) -> Result<(), RepoError> {
    let accounts = Accounts::open(dir).map_err(|_| RepoError::Unavailable)?;
    if let Some(identity) = accounts
        .github_identity(github_id)
        .map_err(|_| RepoError::Unavailable)?
    {
        return if identity.account == account {
            Ok(())
        } else {
            Err(RepoError::OtherGithub)
        };
    }
    let store = accounts.store().map_err(|_| RepoError::Unavailable)?;
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
        return Err(RepoError::Unavailable);
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
        .map_err(|_| RepoError::Unavailable)?;
    let mut bytes = nonce.to_vec();
    bytes.extend_from_slice(&sealed);
    Ok(format!("v1.{}", STANDARD.encode(bytes)))
}

fn open(github: &Github, account: &str, github_id: u64, sealed: &str) -> Result<Secret, RepoError> {
    let bytes = sealed
        .strip_prefix("v1.")
        .and_then(|b64| STANDARD.decode(b64).ok())
        .filter(|bytes| bytes.len() > 12 + 16)
        .ok_or(RepoError::Unavailable)?;
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
        .map_err(|_| RepoError::Unavailable)?;
    String::from_utf8(plain)
        .map(Secret::new)
        .map_err(|_| RepoError::Unavailable)
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
        Err(_) => return Err(RepoError::Unavailable),
    };
    if !meta.is_file() || meta.len() > FILE_MAX {
        return Err(RepoError::Unavailable);
    }
    let bytes = std::fs::read(&path).map_err(|_| RepoError::Unavailable)?;
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| RepoError::Unavailable)?;
    if record.v != SCHEMA || record.account != account || record.projects.len() > MAX_PROJECTS {
        return Err(RepoError::Unavailable);
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
    let text = serde_json::to_vec_pretty(&record).map_err(|_| RepoError::Unavailable)?;
    let temporary = file(dir, account, "tmp");
    std::fs::remove_file(&temporary).ok();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut handle = options
        .open(&temporary)
        .map_err(|_| RepoError::Unavailable)?;
    handle
        .write_all(&text)
        .and_then(|()| handle.sync_all())
        .map_err(|_| RepoError::Unavailable)?;
    std::fs::rename(&temporary, file(dir, account, "json")).map_err(|_| RepoError::Unavailable)?;
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
    builder.create(path).map_err(|_| RepoError::Unavailable)
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
                Err(_) => return Err(RepoError::Unavailable),
            }
        }
        Err(RepoError::Unavailable)
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
