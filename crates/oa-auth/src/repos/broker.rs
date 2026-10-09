//! The Git credential broker (#11056): how a machine that builds or sets
//! up an environment gets GitHub access to one project's repository
//! without a GitHub token ever reaching its environment, its files, or a
//! log.
//!
//! 1. Under the person's session, the web server asks for a ticket for one
//!    project ([`ticket`]): `ogb_<account digest>_<id>_<secret>`, good for
//!    two hours by default (five minutes to a day), kept here only as a
//!    SHA-256 digest. Disconnecting GitHub drops every ticket.
//! 2. The machine's Git credential helper (`coder_environment_setup::
//!    git_auth_env` with `OPENAGENTS_GIT_BROKER`) answers only `https` and
//!    `github.com`, and posts the ticket and the repository path Git asked
//!    about to [`PATH`] ([`credential`]).
//! 3. The broker checks the ticket, that the path is the ticket's project,
//!    and answers in Git's credential format with an installation token
//!    scoped to that one repository, from the token cache (reused while
//!    under 50 minutes old with 5 minutes left, minted again otherwise).
//!    Anything else gets an error and no token; the helper never falls
//!    back to an older one.

use std::path::Path;

use aes_gcm::aead::OsRng;
use aes_gcm::aead::rand_core::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::{RepoError, full_name, hex, load, load_by_digest, mutate, now};
use crate::app::AppClient;

/// The broker's route on the account service.
pub const PATH: &str = "/v1/github/git-credential";
/// Every ticket starts with this.
pub const PREFIX: &str = "ogb_";
/// The only host the broker answers for.
pub const HOST: &str = "github.com";
/// A ticket's lifetime when the caller names none.
pub const DEFAULT_SECONDS: u64 = 2 * 3600;
const MIN_SECONDS: u64 = 5 * 60;
const MAX_SECONDS: u64 = 24 * 3600;
const MAX_TICKETS: usize = 64;

/// A ticket, as kept: never the ticket itself.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ticket {
    id: String,
    /// SHA-256 of the whole ticket, hex.
    digest: String,
    repository_id: u64,
    expires_unix: u64,
}

/// The value a machine's `OPENAGENTS_GIT_BROKER` credential holds: the
/// broker's URL and the ticket, separated by a space.
#[must_use]
pub fn credential_value(service_origin: &str, ticket: &str) -> String {
    format!("{}{PATH} {ticket}", service_origin.trim_end_matches('/'))
}

/// Issue a ticket for the project holding `repository` (`owner/name`).
/// `seconds` 0 means [`DEFAULT_SECONDS`].
pub(super) fn ticket(
    dir: &Path,
    _app: &AppClient,
    account: &str,
    repository: &str,
    seconds: u64,
) -> Result<Value, RepoError> {
    if !full_name(repository) {
        return Err(RepoError::Invalid);
    }
    let seconds = if seconds == 0 {
        DEFAULT_SECONDS
    } else {
        seconds.clamp(MIN_SECONDS, MAX_SECONDS)
    };
    let mut id = [0u8; 8];
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut id);
    OsRng.fill_bytes(&mut secret);
    let id = hex(&id);
    let value = format!(
        "{PREFIX}{}_{id}_{}",
        hex(&Sha256::digest(account.as_bytes())),
        hex(&secret)
    );
    secret.fill(0);
    let now = now();
    let expires_unix = now + seconds;
    mutate(dir, account, |record| {
        let grant = record.app.as_ref().ok_or(RepoError::NotConnected)?;
        if grant.revoked_unix.is_some() {
            return Err(RepoError::Reconnect);
        }
        let project = record
            .projects
            .iter()
            .find(|p| p.repository.eq_ignore_ascii_case(repository))
            .ok_or(RepoError::NotFound)?;
        let repository_id = project.repository_id;
        record.tickets.retain(|t| t.expires_unix > now);
        if record.tickets.len() >= MAX_TICKETS {
            record.tickets.sort_by_key(|t| t.expires_unix);
            record.tickets.remove(0);
        }
        record.tickets.push(Ticket {
            id: id.clone(),
            digest: hex(&Sha256::digest(value.as_bytes())),
            repository_id,
            expires_unix,
        });
        Ok(())
    })?;
    Ok(json!({"ticket": value, "expires_unix": expires_unix, "path": PATH}))
}

/// Why the broker gave no credential. The body says no more than this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not `https://github.com/<owner>/<name>`.
    WrongHost,
    /// No such ticket, or it expired.
    Ticket,
    /// The ticket is for another repository, or the project is gone.
    Repository,
    /// GitHub, the App, or the stores said no.
    Repo(RepoError),
}

impl Refused {
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::WrongHost | Self::Repository => 403,
            Self::Ticket => 401,
            Self::Repo(error) => error.status(),
        }
    }

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::WrongHost => "wrong_host",
            Self::Ticket => "ticket_refused",
            Self::Repository => "wrong_repository",
            Self::Repo(error) => error.code(),
        }
    }
}

impl From<RepoError> for Refused {
    fn from(error: RepoError) -> Self {
        Self::Repo(error)
    }
}

/// Answer one Git credential request: `form` is the helper's
/// `application/x-www-form-urlencoded` body with `ticket`, `protocol`,
/// `host`, and `path` (`owner/name` or `owner/name.git`). On success, the
/// lines Git reads: `username`, `password`, `password_expiry_utc`.
pub async fn credential(dir: &Path, app: Option<&AppClient>, form: &[u8]) -> (u16, String) {
    match answer(dir, app, form).await {
        Ok(text) => (200, text),
        Err(refused) => (refused.status(), format!("error={}\n", refused.code())),
    }
}

async fn answer(dir: &Path, app: Option<&AppClient>, form: &[u8]) -> Result<String, Refused> {
    let app = app.ok_or(RepoError::NotConfigured)?;
    if form.len() > 4096 {
        return Err(Refused::Ticket);
    }
    let field = |name: &str| {
        url::form_urlencoded::parse(form)
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    if field("protocol").as_deref() != Some("https") || field("host").as_deref() != Some(HOST) {
        return Err(Refused::WrongHost);
    }
    let path = field("path").unwrap_or_default();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if !full_name(path) {
        return Err(Refused::Repository);
    }
    let ticket = field("ticket").unwrap_or_default();
    let mut parts = ticket.strip_prefix(PREFIX).unwrap_or_default().split('_');
    let (Some(account_digest), Some(id), Some(_), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Refused::Ticket);
    };
    let record = load_by_digest(dir, account_digest)?.ok_or(Refused::Ticket)?;
    // Read again under the account's own name, as every other call does.
    let record = load(dir, &record.account)?;
    let digest = hex(&Sha256::digest(ticket.as_bytes()));
    let held = record
        .tickets
        .iter()
        .find(|t| t.id == id && bool::from(t.digest.as_bytes().ct_eq(digest.as_bytes())))
        .ok_or(Refused::Ticket)?;
    if held.expires_unix <= now() {
        return Err(Refused::Ticket);
    }
    let project = record
        .projects
        .iter()
        .find(|p| p.repository_id == held.repository_id)
        .ok_or(Refused::Repository)?;
    if !project.repository.eq_ignore_ascii_case(path) {
        return Err(Refused::Repository);
    }
    let grant = record.app.as_ref().ok_or(RepoError::NotConnected)?;
    if grant.revoked_unix.is_some() {
        return Err(RepoError::Reconnect.into());
    }
    let owner = project
        .repository
        .split_once('/')
        .map_or("", |(owner, _)| owner);
    let installation = project
        .installation_id
        .or_else(|| grant.installation_of(owner).map(|i| i.id))
        .ok_or(RepoError::NotInstalled)?;
    let minted = app
        .installation_token(installation, Some(project.repository_id), None)
        .await?;
    Ok(format!(
        "username=x-access-token\npassword={}\npassword_expiry_utc={}\n",
        minted.token.as_str(),
        minted.expires_unix
    ))
}
