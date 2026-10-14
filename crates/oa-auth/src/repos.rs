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
//! account service's routes call [`answer_with`].
//!
//! When the service has a GitHub App ([`crate::app`]), a person can
//! install it on the repositories they pick instead ([`installed`]): the
//! App's user token (sealed the same way, refreshed when it expires) finds
//! their installations and lists the repositories in them, and Git gets
//! one-hour installation tokens for one repository at a time through the
//! credential broker ([`broker`]). An account with the App installed uses
//! it; one with only the OAuth grant keeps working as before.

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
use crate::app::AppClient;
use crate::cache::Lists;
use crate::github::{Api, ApiFault, Github, Secret, Sso};
use crate::service::CodeRequest;

pub mod broker;
mod installed;
pub mod members;

pub use installed::Installation;
pub use members::{Member, Role, Sharing};

/// The directory beside `accounts.json` the per-account files live in.
pub const STORE_DIR: &str = "github-access";
const SCHEMA: &str = "openagents.github-access.v1";
/// The most projects one account keeps.
pub const MAX_PROJECTS: usize = 200;
const FILE_MAX: u64 = 1024 * 1024;

/// An account's project: a named home for chats, instructions, files, and
/// zero or more connected GitHub repositories. When it has repositories,
/// exactly one is the primary (a chat started in the project works in it).
/// The same repository may be in any number of projects, and names may
/// repeat (projects have ids).
///
/// A project kept before 2026-10-13 had one `repository` and no
/// `repositories`; it reads back with that repository as its primary
/// ([`StoredProject`]), and is written in the new shape the next time the
/// account's record changes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(from = "StoredProject")]
pub struct Project {
    /// `prj_` and 16 hex characters.
    pub id: String,
    /// Shown in the sidebar.
    pub name: String,
    /// What a chat in the project starts from, as the person wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub created_unix: u64,
    /// Its repositories, in the order they were added.
    #[serde(default)]
    pub repositories: Vec<ProjectRepository>,
    /// Who is in it, as the account reading it sees (#11585): set on the
    /// projects an account service answers with, never kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharing: Option<Sharing>,
}

/// One repository in a project.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRepository {
    /// GitHub's numeric repository id (stable across renames).
    pub repository_id: u64,
    /// `owner/name`.
    pub repository: String,
    pub default_branch: String,
    pub private: bool,
    /// The project's primary repository.
    #[serde(default)]
    pub primary: bool,
    /// The GitHub App installation that reaches the repository, for one
    /// added through the App.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<u64>,
}

/// A project as kept or sent: the current shape, or the older one-repository
/// shape (`repository_id`, `repository`, `default_branch`, `private`,
/// `installation_id` at the top level).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProject {
    id: String,
    name: String,
    #[serde(default)]
    instructions: Option<String>,
    created_unix: u64,
    #[serde(default)]
    repositories: Vec<ProjectRepository>,
    #[serde(default)]
    repository_id: Option<u64>,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    private: Option<bool>,
    #[serde(default)]
    installation_id: Option<u64>,
    #[serde(default)]
    sharing: Option<Sharing>,
}

impl From<StoredProject> for Project {
    fn from(stored: StoredProject) -> Self {
        let mut repositories = stored.repositories;
        if repositories.is_empty()
            && let (Some(repository_id), Some(repository)) =
                (stored.repository_id, stored.repository)
        {
            repositories.push(ProjectRepository {
                repository_id,
                repository,
                default_branch: stored.default_branch.unwrap_or_else(|| "main".into()),
                private: stored.private.unwrap_or(false),
                primary: true,
                installation_id: stored.installation_id,
            });
        }
        let mut project = Self {
            id: stored.id,
            name: stored.name,
            instructions: stored.instructions,
            created_unix: stored.created_unix,
            repositories,
            sharing: stored.sharing,
        };
        project.settle_primary();
        project
    }
}

impl Project {
    /// The primary repository, when the project has any.
    #[must_use]
    pub fn primary(&self) -> Option<&ProjectRepository> {
        self.repositories.iter().find(|r| r.primary)
    }

    /// The primary repository's `owner/name`.
    #[must_use]
    pub fn repository(&self) -> Option<&str> {
        self.primary().map(|r| r.repository.as_str())
    }

    /// The project's entry for `repository` (`owner/name`, without regard
    /// to case, as GitHub treats it).
    #[must_use]
    pub fn holding(&self, repository: &str) -> Option<&ProjectRepository> {
        self.repositories
            .iter()
            .find(|r| r.repository.eq_ignore_ascii_case(repository))
    }

    /// Exactly one primary when there are repositories: the first marked
    /// one, else the first added.
    fn settle_primary(&mut self) {
        let first = self
            .repositories
            .iter()
            .position(|r| r.primary)
            .unwrap_or(0);
        for (at, repository) in self.repositories.iter_mut().enumerate() {
            repository.primary = at == first;
        }
    }
}

/// The project a chat whose repository is `repository` files under
/// (#11541): the one project holding it; when several do, the one where it
/// is the primary; when that is still not one project, none.
#[must_use]
pub fn filing_project<'a>(projects: &'a [Project], repository: &str) -> Option<&'a Project> {
    let holding: Vec<&Project> = projects
        .iter()
        .filter(|p| p.holding(repository).is_some())
        .collect();
    if let [only] = holding.as_slice() {
        return Some(only);
    }
    let primary: Vec<&Project> = holding
        .into_iter()
        .filter(|p| p.holding(repository).is_some_and(|r| r.primary))
        .collect();
    match primary.as_slice() {
        [only] => Some(only),
        _ => None,
    }
}

/// The most characters a project's name keeps.
pub const MAX_NAME: usize = 100;
/// The most characters a project's instructions keep.
pub const MAX_INSTRUCTIONS: usize = 8_000;
/// The most repositories one project holds.
pub const MAX_PROJECT_REPOSITORIES: usize = 20;

/// A new project: a name (or the repository's name), optional
/// instructions, and the repositories it starts with (the first is the
/// primary).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct NewProject {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub repositories: Vec<String>,
}

/// Read a membership route's body into its call (#11585): `PATCH
/// /v1/projects/{id}/members/{member}` takes `{role}`, `POST
/// /v1/projects/{id}/invitations` takes `{role?, seconds?}` (an editor
/// for a week when empty), and the invitation routes take `{code}`.
pub mod bodies {
    use super::{Call, Role};
    use serde_json::Value;

    /// `{role}` for a member.
    #[must_use]
    pub fn set_role(id: String, member: String, body: &Value) -> Option<Call> {
        let role = Role::from_word(body.get("role")?.as_str()?)?;
        Some(Call::SetRole { id, member, role })
    }

    /// `{role?, seconds?}` for an invitation.
    #[must_use]
    pub fn invite(id: String, body: &Value) -> Option<Call> {
        let role = match body.get("role") {
            None | Some(Value::Null) => Role::Editor,
            Some(value) => Role::from_word(value.as_str()?)?,
        };
        let seconds = match body.get("seconds") {
            None | Some(Value::Null) => 0,
            Some(value) => value.as_u64()?,
        };
        Some(Call::Invite { id, role, seconds })
    }

    /// `{code}` for an invitation link.
    #[must_use]
    pub fn code(body: &Value) -> Option<String> {
        body.get("code")?
            .as_str()
            .map(str::trim)
            .filter(|code| !code.is_empty() && code.len() <= 256)
            .map(str::to_string)
    }
}

/// Read the account service's `POST /v1/projects` body: `{name?,
/// instructions?, repositories?: [owner/name]}`, or the older
/// `{repository}`. A name or at least one repository is needed.
#[must_use]
pub fn new_project(body: &Value) -> Option<NewProject> {
    let mut new: NewProject = serde_json::from_value(json!({
        "name": body.get("name").cloned().unwrap_or(Value::Null),
        "instructions": body.get("instructions").cloned().unwrap_or(Value::Null),
        "repositories": body.get("repositories").cloned().unwrap_or(json!([])),
    }))
    .ok()?;
    match body.get("repository") {
        None | Some(Value::Null) => {}
        Some(Value::String(repository)) => new.repositories.insert(0, repository.clone()),
        Some(_) => return None,
    }
    (new.name.is_some() || !new.repositories.is_empty()).then_some(new)
}

/// A change to a project. `instructions: Some(None)` clears them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectChange {
    pub name: Option<String>,
    pub instructions: Option<Option<String>>,
    /// Add this repository (`owner/name`); `primary` makes it the primary.
    pub add_repository: Option<String>,
    pub primary: bool,
    /// Remove this repository; when it was the primary, the first one left
    /// becomes the primary.
    pub remove_repository: Option<String>,
    /// Make this repository, already in the project, the primary.
    pub primary_repository: Option<String>,
}

impl ProjectChange {
    /// Read the account service's `PATCH /v1/projects/{id}` body:
    /// `{name?, instructions? (null clears), add_repository?, primary?,
    /// remove_repository?, primary_repository?}`.
    #[must_use]
    pub fn from_body(body: &Value) -> Option<Self> {
        let text = |key: &str| -> Option<Option<String>> {
            match body.get(key) {
                None => Some(None),
                Some(Value::String(value)) => Some(Some(value.clone())),
                Some(_) => None,
            }
        };
        let instructions = match body.get("instructions") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::String(value)) => Some(Some(value.clone())),
            Some(_) => return None,
        };
        let change = Self {
            name: text("name")?,
            instructions,
            add_repository: text("add_repository")?,
            primary: match body.get("primary") {
                None => false,
                Some(value) => value.as_bool()?,
            },
            remove_repository: text("remove_repository")?,
            primary_repository: text("primary_repository")?,
        };
        Some(change)
    }

    /// The body [`Self::from_body`] reads.
    #[must_use]
    pub fn body(&self) -> Value {
        let mut body = json!({});
        if let Some(name) = &self.name {
            body["name"] = json!(name);
        }
        if let Some(instructions) = &self.instructions {
            body["instructions"] = json!(instructions);
        }
        if let Some(repository) = &self.add_repository {
            body["add_repository"] = json!(repository);
            body["primary"] = json!(self.primary);
        }
        if let Some(repository) = &self.remove_repository {
            body["remove_repository"] = json!(repository);
        }
        if let Some(repository) = &self.primary_repository {
            body["primary_repository"] = json!(repository);
        }
        body
    }
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
    /// The GitHub App installation it was listed through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation: Option<u64>,
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
    /// The GitHub App is authorized as `login` and installed on these
    /// accounts (none yet when the person hasn't picked repositories).
    Installed {
        login: String,
        installations: Vec<Installation>,
    },
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
            Access::Installed {
                login,
                installations,
            } => json!({"state": "installed", "login": login, "installations": installations}),
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
            "installed" => Access::Installed {
                login: login()?,
                installations: serde_json::from_value(github["installations"].clone()).ok()?,
            },
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
    /// The GitHub App isn't installed on that repository (or its account
    /// removed it).
    NotInstalled,
    /// The GitHub App's installation on that account is suspended.
    Suspended,
    /// The person connected GitHub for public repositories only, so their
    /// token can't change code (#11572): connect private repositories.
    NeedsRepoAccess,
    /// The person's GitHub account can't push to that repository.
    CannotPush,
    /// The caller's role in the project doesn't allow that (#11585).
    NotAllowed,
    /// The owner can't leave or step down without giving the project to
    /// another member.
    OwnerStays,
    /// The invitation was used, expired, or never existed.
    InvitationGone,
    /// The project has as many members as it holds.
    ProjectFull,
}

impl RepoError {
    const ALL: [Self; 23] = [
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
        Self::NotInstalled,
        Self::Suspended,
        Self::NeedsRepoAccess,
        Self::CannotPush,
        Self::NotAllowed,
        Self::OwnerStays,
        Self::InvitationGone,
        Self::ProjectFull,
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
            Self::NotInstalled => "github_app_not_installed",
            Self::Suspended => "github_app_suspended",
            Self::NeedsRepoAccess => "github_needs_repo_access",
            Self::CannotPush => "github_cannot_push",
            Self::NotAllowed => "project_role",
            Self::OwnerStays => "project_owner_stays",
            Self::InvitationGone => "invitation_not_found",
            Self::ProjectFull => "project_full",
        }
    }

    /// The HTTP status an account-service route answers with.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::NotConnected
            | Self::Reconnect
            | Self::OtherGithub
            | Self::Full
            | Self::NeedsRepoAccess
            | Self::OwnerStays
            | Self::ProjectFull => 409,
            Self::NotFound | Self::NotInstalled | Self::InvitationGone => 404,
            Self::Denied => 401,
            Self::Invalid => 400,
            Self::SsoRequired
            | Self::Forbidden
            | Self::Suspended
            | Self::CannotPush
            | Self::NotAllowed => 403,
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
            Self::NotInstalled => {
                "OpenAgents isn't installed on that repository. Choose it on GitHub, then try again."
            }
            Self::Suspended => {
                "OpenAgents is suspended on that GitHub account. An owner can turn it back on in the account's GitHub settings."
            }
            Self::NeedsRepoAccess => {
                "Your GitHub connection reaches public repositories only, so OpenAgents can't push for you. Connect private repositories, then try again."
            }
            Self::CannotPush => {
                "Your GitHub account can't push to that repository. Ask its owner for write access, then try again."
            }
            Self::NotAllowed => "Your role in this project doesn't allow that.",
            Self::OwnerStays => {
                "You own this project. Make another member the owner first, or delete the project."
            }
            Self::InvitationGone => {
                "That invitation link has expired or was already used. Ask for a new one."
            }
            Self::ProjectFull => "This project has as many members as it can hold.",
        })
    }
}

impl std::error::Error for RepoError {}

impl From<AuthError> for RepoError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Denied | AuthError::InviteOnly => Self::Denied,
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
    /// `POST /v1/projects` `{name?, instructions?, repositories?}` (or
    /// the older `{repository}`): always a new project.
    CreateProject(NewProject),
    /// `PATCH /v1/projects/{id}` ([`ProjectChange::from_body`]).
    EditProject { id: String, change: ProjectChange },
    /// `DELETE /v1/account/projects/{id}`
    RemoveProject(String),
    /// `POST /v1/account/github/app/grant` `{code, code_verifier}`: the
    /// GitHub App's user authorization; finds the person's installations.
    AppGrant(CodeRequest),
    /// `POST /v1/account/github/app/refresh`: find the installations again
    /// (after the person installed the App or changed its repositories).
    AppRefresh,
    /// `POST /v1/account/github/broker` `{repository, seconds}`: a ticket
    /// that lets one machine fetch and push one project's repository
    /// through the credential broker ([`broker`]).
    BrokerTicket { repository: String, seconds: u64 },
    /// `GET /v1/projects/{id}/members` (#11585).
    Members(String),
    /// `PATCH /v1/projects/{id}/members/{member}` `{role}`.
    SetRole {
        id: String,
        member: String,
        role: Role,
    },
    /// `DELETE /v1/projects/{id}/members/{member}`.
    RemoveMember { id: String, member: String },
    /// `POST /v1/projects/{id}/leave`.
    Leave(String),
    /// `POST /v1/projects/{id}/invitations` `{role?, seconds?}`.
    Invite {
        id: String,
        role: Role,
        seconds: u64,
    },
    /// `POST /v1/projects/invitations/preview` `{code}`.
    Invitation(String),
    /// `POST /v1/projects/invitations/accept` `{code}`.
    Accept(String),
    /// `GET /v1/projects/{id}/access` (#11587): whether the caller's own
    /// GitHub access reaches each of the project's repositories.
    Access(String),
}

/// Answer `call` for `account` with the OAuth App only (see
/// [`answer_with`]).
pub async fn answer(
    dir: &Path,
    github: Option<&Github>,
    account: &str,
    call: Call,
) -> (u16, Value) {
    answer_with(dir, github, None, account, call).await
}

/// Answer `call` for `account`: an HTTP status and a JSON body (an error
/// envelope on failure). `github` is the OAuth App, `app` the GitHub App;
/// either may be missing. An account that installed the GitHub App uses
/// it when the service has it.
pub async fn answer_with(
    dir: &Path,
    github: Option<&Github>,
    app: Option<&AppClient>,
    account: &str,
    call: Call,
) -> (u16, Value) {
    let result = async {
        let oauth = github;
        let github = || github.ok_or(RepoError::NotConfigured);
        let app_ready = || app.ok_or(RepoError::NotConfigured);
        let via_app = match app {
            Some(app) if installed::uses_app(dir, account)? => Some(app),
            _ => None,
        };
        Ok::<Value, RepoError>(match call {
            Call::Status => status(dir, account)?.body(),
            Call::Grant(request) => grant(dir, github()?, account, &request).await?.body(),
            Call::Disconnect => disconnect(dir, account)?.body(),
            Call::Repositories(page) => {
                let listing = match via_app {
                    Some(app) => installed::repositories(dir, app, account, page).await?,
                    None => repositories(dir, github()?, account, page).await?,
                };
                let more = listing.more;
                let mut body = serde_json::to_value(listing).map_err(|_| RepoError::BadAnswer)?;
                // The shared list shape (#11156): `data` and `next` (the
                // cursor for `?after=`) beside `repositories` and `more`.
                body["data"] = body["repositories"].clone();
                body["next"] = if more {
                    json!(page.clamp(1, MAX_PAGE).saturating_add(1).to_string())
                } else {
                    Value::Null
                };
                body
            }
            Call::Token => {
                let (token, private) = match via_app {
                    Some(app) => (installed::user_token(dir, app, account).await?, true),
                    None => token(dir, github()?, account)?,
                };
                json!({"token": token.as_str(), "private": private})
            }
            Call::CreateProject(new) => {
                let mut found = Vec::with_capacity(new.repositories.len());
                for repository in &new.repositories {
                    found.push(look_up(dir, oauth, via_app, account, repository).await?);
                }
                json!({"project": create_project(dir, account, &new, &found)?})
            }
            Call::EditProject { id, change } => {
                let found = match &change.add_repository {
                    Some(repository) => {
                        Some(look_up(dir, oauth, via_app, account, repository).await?)
                    }
                    None => None,
                };
                json!({"project": edit_project(dir, account, &id, &change, found.as_ref())?})
            }
            Call::AppGrant(request) => installed::grant(dir, app_ready()?, account, &request)
                .await?
                .body(),
            Call::AppRefresh => installed::refresh(dir, app_ready()?, account).await?.body(),
            Call::BrokerTicket {
                repository,
                seconds,
            } => match via_app {
                // The App, when the account installed it: tokens for one
                // repository from its installation. In a project shared
                // with the account, its own user token must push there
                // first (#11587).
                Some(app) => {
                    if !broker::owns_repository(dir, account, &repository)? {
                        installed::can_push(dir, app, account, &repository).await?;
                    }
                    broker::ticket(dir, true, account, &repository, seconds)?
                }
                // Otherwise the person's own OAuth token (#11572): it must
                // reach private repositories (`repo`) and push to this one.
                None => {
                    can_push(dir, github()?, account, &repository).await?;
                    broker::ticket(dir, false, account, &repository, seconds)?
                }
            },
            Call::RemoveProject(id) => {
                remove_project(dir, account, &id)?;
                json!({"removed": id})
            }
            Call::Members(id) => members::members(dir, account, &id)?,
            Call::SetRole { id, member, role } => {
                members::set_role(dir, account, &id, &member, role)?
            }
            Call::RemoveMember { id, member } => {
                members::remove_member(dir, account, &id, &member)?
            }
            Call::Leave(id) => members::leave(dir, account, &id)?,
            Call::Invite { id, role, seconds } => {
                members::invite(dir, account, &id, role, seconds)?
            }
            Call::Invitation(code) => members::preview(dir, account, &code)?,
            Call::Accept(code) => members::accept(dir, account, &code)?,
            Call::Access(id) => {
                let held = members::held(dir, &id)?.ok_or(RepoError::NotFound)?;
                if held.role_of(account).is_none() {
                    return Err(RepoError::NotFound);
                }
                let mut rows = Vec::with_capacity(held.project.repositories.len());
                for repository in &held.project.repositories {
                    // The caller's own token or App grant, never the
                    // project owner's.
                    let reached =
                        look_up(dir, oauth, via_app, account, &repository.repository).await;
                    rows.push(match reached {
                        Ok(_) => json!({"repository": repository.repository, "access": true}),
                        Err(error) => json!({
                            "repository": repository.repository,
                            "access": false,
                            "code": error.code(),
                            "message": no_access_message(error),
                        }),
                    });
                }
                json!({"repositories": rows})
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

/// What a member is told about a repository their own GitHub access
/// doesn't reach (#11587).
fn no_access_message(error: RepoError) -> String {
    match error {
        RepoError::NotFound | RepoError::NotInstalled | RepoError::Forbidden => {
            "You don't have access to this repository on GitHub.".into()
        }
        other => other.to_string(),
    }
}

/// The account's access and projects: its own and those shared with it
/// (#11585), each with the account's role.
pub fn status(dir: &Path, account: &str) -> Result<Status, RepoError> {
    let access = load(dir, account)?.status().access;
    Ok(Status {
        access,
        projects: members::projects_of(dir, account)?,
    })
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

/// Forget the stored tokens (the OAuth grant, the GitHub App's user
/// token, and every broker ticket). Projects stay.
pub fn disconnect(dir: &Path, account: &str) -> Result<Status, RepoError> {
    mutate(dir, account, |record| {
        record.grant = None;
        record.app = None;
        record.tickets.clear();
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

/// `repository` (`owner/name`) as GitHub shows it to the person, and the
/// GitHub App installation that reaches it when the account uses the App.
async fn look_up(
    dir: &Path,
    github: Option<&Github>,
    app: Option<&AppClient>,
    account: &str,
    repository: &str,
) -> Result<(Repository, Option<u64>), RepoError> {
    match app {
        Some(app) => installed::look_up(dir, app, account, repository).await,
        None => Ok((
            found_repository(
                dir,
                github.ok_or(RepoError::NotConfigured)?,
                account,
                repository,
            )
            .await?,
            None,
        )),
    }
}

/// The account's OAuth token has `repo` and pushes to `repository`
/// (`owner/name`), as GitHub says now (#11572).
async fn can_push(
    dir: &Path,
    github: &Github,
    account: &str,
    repository: &str,
) -> Result<(), RepoError> {
    if !full_name(repository) {
        return Err(RepoError::Invalid);
    }
    let (token, private) = token(dir, github, account)?;
    if !private {
        return Err(RepoError::NeedsRepoAccess);
    }
    let answer = github
        .api(token.as_str(), &format!("/repos/{repository}"))
        .await?;
    let body = checked(dir, account, answer)?.body;
    if body["disabled"].as_bool() == Some(true) || body["archived"].as_bool() == Some(true) {
        return Err(RepoError::CannotPush);
    }
    if body["permissions"]["push"].as_bool() != Some(true) {
        return Err(RepoError::CannotPush);
    }
    Ok(())
}

/// `repository` (`owner/name`) read with the account's OAuth token.
pub async fn found_repository(
    dir: &Path,
    github: &Github,
    account: &str,
    repository: &str,
) -> Result<Repository, RepoError> {
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
    listed(&body).ok_or(RepoError::BadAnswer)
}

/// Add `repository` (`owner/name`) as a new project named after it (the
/// older one-step form; there is no reuse of a project that holds it).
pub async fn add_project(
    dir: &Path,
    github: &Github,
    account: &str,
    repository: &str,
) -> Result<Project, RepoError> {
    let found = found_repository(dir, github, account, repository).await?;
    create_project(
        dir,
        account,
        &NewProject {
            repositories: vec![repository.to_string()],
            ..NewProject::default()
        },
        &[(found, None)],
    )
}

/// A name or instructions as kept: trimmed, bounded, without control
/// characters (instructions keep their line breaks).
fn clean(value: &str, max: usize, lines: bool) -> Result<String, RepoError> {
    let value = value.trim();
    if value.chars().count() > max
        || value
            .chars()
            .any(|c| c.is_control() && !(lines && matches!(c, '\n' | '\t' | '\r')))
    {
        return Err(RepoError::Invalid);
    }
    Ok(value.to_string())
}

fn entry(found: &Repository, installation: Option<u64>) -> ProjectRepository {
    ProjectRepository {
        repository_id: found.id,
        repository: found.full_name.clone(),
        default_branch: found.default_branch.clone(),
        private: found.private,
        primary: false,
        installation_id: installation,
    }
}

/// A rename, a new default branch or visibility on GitHub follows into
/// every project the account owns that holds the repository, except
/// `skip` (changed by the caller itself).
fn follow(
    dir: &Path,
    account: &str,
    found: &Repository,
    installation: Option<u64>,
    skip: Option<&str>,
) -> Result<(), RepoError> {
    for held in members::held_by(dir, account)? {
        if held.role_of(account) != Some(Role::Owner)
            || skip == Some(held.project.id.as_str())
            || !held.project.repositories.iter().any(|r| {
                r.repository_id == found.id
                    && (r.repository != found.full_name
                        || r.default_branch != found.default_branch
                        || r.private != found.private
                        || (installation.is_some() && r.installation_id != installation))
            })
        {
            continue;
        }
        members::edit(
            dir,
            account,
            &held.project.id,
            |_| false,
            |project| {
                follow_into(project, found, installation);
                Ok(())
            },
        )?;
    }
    Ok(())
}

/// [`follow`] for one project.
fn follow_into(project: &mut Project, found: &Repository, installation: Option<u64>) {
    for held in project
        .repositories
        .iter_mut()
        .filter(|r| r.repository_id == found.id)
    {
        held.repository.clone_from(&found.full_name);
        held.default_branch.clone_from(&found.default_branch);
        held.private = found.private;
        if installation.is_some() {
            held.installation_id = installation;
        }
    }
}

/// Make a new project: always new, even when another project has the same
/// name or repositories. `found` are `new.repositories` as GitHub showed
/// them, in order; the first is the primary.
pub fn create_project(
    dir: &Path,
    account: &str,
    new: &NewProject,
    found: &[(Repository, Option<u64>)],
) -> Result<Project, RepoError> {
    if found.len() > MAX_PROJECT_REPOSITORIES {
        return Err(RepoError::Invalid);
    }
    let name = match new.name.as_deref().map(|n| clean(n, MAX_NAME, false)) {
        Some(name) => name?,
        None => found
            .first()
            .map(|(r, _)| {
                r.full_name
                    .split_once('/')
                    .map_or(r.full_name.as_str(), |(_, name)| name)
                    .to_string()
            })
            .unwrap_or_default(),
    };
    if name.is_empty() {
        return Err(RepoError::Invalid);
    }
    let instructions = match new.instructions.as_deref() {
        Some(text) => Some(clean(text, MAX_INSTRUCTIONS, true)?).filter(|t| !t.is_empty()),
        None => None,
    };
    let mut repositories: Vec<ProjectRepository> = Vec::new();
    for (repository, installation) in found {
        if !repositories
            .iter()
            .any(|r| r.repository_id == repository.id)
        {
            repositories.push(entry(repository, *installation));
        }
    }
    let mut id = [0u8; 8];
    OsRng.fill_bytes(&mut id);
    let mut fresh = Project {
        id: format!("prj_{}", hex(&id)),
        name,
        instructions,
        created_unix: now(),
        repositories,
        sharing: None,
    };
    fresh.settle_primary();
    forget(dir, account);
    let created = members::create(dir, account, fresh)?;
    for (repository, installation) in found {
        follow(dir, account, repository, *installation, Some(&created.id))?;
    }
    Ok(created)
}

/// Change a project: its name, instructions, and repositories. `found` is
/// `change.add_repository` as GitHub showed it. A repository to remove or
/// make primary that isn't in the project is [`RepoError::NotFound`].
pub fn edit_project(
    dir: &Path,
    account: &str,
    id: &str,
    change: &ProjectChange,
    found: Option<&(Repository, Option<u64>)>,
) -> Result<Project, RepoError> {
    if !project_id(id) {
        return Err(RepoError::Invalid);
    }
    let name = match change.name.as_deref() {
        Some(name) => Some(clean(name, MAX_NAME, false)?).filter(|n| !n.is_empty()),
        None => None,
    };
    if change.name.is_some() && name.is_none() {
        return Err(RepoError::Invalid);
    }
    let instructions = match &change.instructions {
        Some(Some(text)) => {
            Some(Some(clean(text, MAX_INSTRUCTIONS, true)?).filter(|t| !t.is_empty()))
        }
        Some(None) => Some(None),
        None => None,
    };
    if change.add_repository.is_some() != found.is_some() {
        return Err(RepoError::Invalid);
    }
    forget(dir, account);
    // Renaming is the owner's; instructions and repositories an editor's.
    let renaming = |kept: &Project| name.as_ref().is_some_and(|name| *name != kept.name);
    let edited = members::edit(dir, account, id, renaming, |project| {
        if let Some(name) = &name {
            project.name.clone_from(name);
        }
        if let Some(instructions) = &instructions {
            project.instructions.clone_from(instructions);
        }
        if let Some((repository, installation)) = found {
            follow_into(project, repository, *installation);
            let at = match project
                .repositories
                .iter()
                .position(|r| r.repository_id == repository.id)
            {
                Some(at) => at,
                None => {
                    if project.repositories.len() >= MAX_PROJECT_REPOSITORIES {
                        return Err(RepoError::Full);
                    }
                    project.repositories.push(entry(repository, *installation));
                    project.repositories.len() - 1
                }
            };
            if change.primary || project.repositories.len() == 1 {
                for (n, held) in project.repositories.iter_mut().enumerate() {
                    held.primary = n == at;
                }
            }
        }
        if let Some(repository) = &change.remove_repository {
            let before = project.repositories.len();
            project
                .repositories
                .retain(|r| !r.repository.eq_ignore_ascii_case(repository));
            if project.repositories.len() == before {
                return Err(RepoError::NotFound);
            }
        }
        if let Some(repository) = &change.primary_repository {
            let at = project
                .repositories
                .iter()
                .position(|r| r.repository.eq_ignore_ascii_case(repository))
                .ok_or(RepoError::NotFound)?;
            for (n, held) in project.repositories.iter_mut().enumerate() {
                held.primary = n == at;
            }
        }
        project.settle_primary();
        Ok(())
    })?;
    if let Some((repository, installation)) = found {
        follow(dir, account, repository, *installation, Some(id))?;
    }
    Ok(edited)
}

/// Remove a project. Removing one that isn't there is not an error.
pub fn remove_project(dir: &Path, account: &str, id: &str) -> Result<(), RepoError> {
    if !project_id(id) {
        return Err(RepoError::Invalid);
    }
    forget(dir, account);
    members::remove(dir, account, id)
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
pub(crate) fn answered(answer: Api) -> Result<Api, RepoError> {
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
        installation: None,
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
    /// The GitHub App's user authorization and installations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app: Option<installed::AppGrant>,
    /// Credential-broker tickets, by digest ([`broker`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tickets: Vec<broker::Ticket>,
    /// The account's projects before #11585. Projects now live in their
    /// own store with members ([`members`]); this copy is read only to
    /// fill a file store that doesn't exist yet, and is never changed.
    #[serde(default)]
    projects: Vec<Project>,
}

impl Record {
    fn new(account: &str) -> Self {
        Self {
            v: SCHEMA.into(),
            account: account.into(),
            grant: None,
            app: None,
            tickets: Vec::new(),
            projects: Vec::new(),
        }
    }

    fn status(&self) -> Status {
        if let Some(app) = &self.app {
            let access = if app.revoked_unix.is_some() {
                Access::Reconnect {
                    login: app.login.clone(),
                }
            } else {
                Access::Installed {
                    login: app.login.clone(),
                    installations: app.installations.clone(),
                }
            };
            return Status {
                access,
                projects: self.projects.clone(),
            };
        }
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
    seal_bound(github, &associated(account, github_id), token)
}

/// Seal `token` under the App file's key, bound to `aad`.
fn seal_bound(github: &Github, aad: &str, token: &str) -> Result<String, RepoError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
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
    open_bound(github, &associated(account, github_id), sealed)
}

fn open_bound(github: &Github, aad: &str, sealed: &str) -> Result<Secret, RepoError> {
    let bytes = sealed
        .strip_prefix("v1.")
        .and_then(|b64| STANDARD.decode(b64).ok())
        .filter(|bytes| bytes.len() > 12 + 16)
        .ok_or(RepoError::Storage)?;
    let (nonce, body) = bytes.split_at(12);
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

/// A record read from the account database, checked as a file is.
#[cfg(feature = "postgres")]
fn from_row(value: serde_json::Value) -> Result<Record, RepoError> {
    let record: Record = serde_json::from_value(value).map_err(|_| RepoError::Storage)?;
    if record.v != SCHEMA || record.projects.len() > MAX_PROJECTS {
        return Err(RepoError::Storage);
    }
    Ok(record)
}

fn load(dir: &Path, account: &str) -> Result<Record, RepoError> {
    check_account(account)?;
    #[cfg(feature = "postgres")]
    if let Some(database) = tenancy::db::bound(dir) {
        let record = match tenancy::db::records::github_access(&database, account)
            .map_err(|_| RepoError::Storage)?
        {
            Some(value) => from_row(value)?,
            None => Record::new(account),
        };
        if record.account != account {
            return Err(RepoError::Storage);
        }
        return Ok(record);
    }
    let record = read_record(&file(dir, account, "json"))?.unwrap_or_else(|| Record::new(account));
    if record.account != account {
        return Err(RepoError::Storage);
    }
    Ok(record)
}

/// The record in the file named by its account's digest (a broker ticket
/// carries the digest, not the account).
fn load_by_digest(dir: &Path, digest: &str) -> Result<Option<Record>, RepoError> {
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(None);
    }
    #[cfg(feature = "postgres")]
    if let Some(database) = tenancy::db::bound(dir) {
        return match tenancy::db::records::github_access_by_digest(&database, digest)
            .map_err(|_| RepoError::Storage)?
        {
            Some(value) => Ok(Some(from_row(value)?).filter(|record| {
                hex(&Sha256::digest(record.account.as_bytes())) == digest.to_ascii_lowercase()
            })),
            None => Ok(None),
        };
    }
    let path = dir.join(STORE_DIR).join(format!("{digest}.json"));
    Ok(read_record(&path)?.filter(|record| {
        hex(&Sha256::digest(record.account.as_bytes())) == digest.to_ascii_lowercase()
    }))
}

fn read_record(path: &Path) -> Result<Option<Record>, RepoError> {
    let path = path.to_path_buf();
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(_) => return Err(RepoError::Storage),
    };
    if !meta.is_file() || meta.len() > FILE_MAX {
        return Err(RepoError::Storage);
    }
    let bytes = std::fs::read(&path).map_err(|_| RepoError::Storage)?;
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| RepoError::Storage)?;
    if record.v != SCHEMA || record.projects.len() > MAX_PROJECTS {
        return Err(RepoError::Storage);
    }
    Ok(Some(record))
}

/// Run `change` on the account's record under its lock and write it back.
fn mutate<T>(
    dir: &Path,
    account: &str,
    change: impl FnOnce(&mut Record) -> Result<T, RepoError>,
) -> Result<T, RepoError> {
    check_account(account)?;
    #[cfg(feature = "postgres")]
    if let Some(database) = tenancy::db::bound(dir) {
        return tenancy::db::records::update_github_access(&database, account, |current| {
            let mut record = match current {
                Some(value) => from_row(value)?,
                None => Record::new(account),
            };
            if record.account != account {
                return Err(RepoError::Storage);
            }
            let out = change(&mut record)?;
            let value = serde_json::to_value(&record).map_err(|_| RepoError::Storage)?;
            Ok((value, out))
        })
        .map_err(|_| RepoError::Storage)?;
    }
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
