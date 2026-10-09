//! Repository access through the GitHub App ([`crate::app`]).
//!
//! The person authorizes the App (its own OAuth client, same callback),
//! which gives a user token that expires after eight hours and a refresh
//! token. Both are sealed like the OAuth grant's token, bound to the
//! account, the GitHub user and which of the two they are. The user token
//! finds the person's installations (`/user/installations`) and lists the
//! repositories in each that the person can reach
//! (`/user/installations/{id}/repositories`), so an organization's
//! installation shows a member only what they could open on GitHub.
//! Installation ids are never taken from a browser.
//!
//! The user token is renewed with its refresh token when it has less than
//! a minute left, or once when GitHub answers 401; renewals for one
//! account wait on each other, because GitHub rotates the refresh token.
//! A refresh token GitHub refuses marks the access ended (reconnect).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Listing, MAX_PAGE, PAGE_SIZE, Project, RepoError, SCHEMA, Status, answered, check_account,
    full_name, keep_project, listed, load, mutate, now, open_bound, same_person, seal_bound,
    status,
};
use crate::AuthError;
use crate::app::AppClient;
use crate::github::{Api, Secret};
use crate::service::CodeRequest;

/// The most installations kept for one person.
const MAX_INSTALLATIONS: usize = 100;
/// The most installations one page of repositories reads.
const LISTED_INSTALLATIONS: usize = 20;

/// One installation of the GitHub App the person can use.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub id: u64,
    /// The user or organization it's installed on.
    pub account: String,
    #[serde(default)]
    pub organization: bool,
    /// On every repository of the account, not chosen ones.
    #[serde(default)]
    pub all: bool,
    /// Suspended by the account's owner or by GitHub.
    #[serde(default)]
    pub suspended: bool,
}

/// The GitHub App's user authorization, as kept on disk.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AppGrant {
    pub github_id: u64,
    pub login: String,
    /// The user token, sealed (`v1.` and base64).
    pub sealed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_unix: Option<u64>,
    /// The refresh token, sealed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_refresh: Option<String>,
    #[serde(default)]
    pub installations: Vec<Installation>,
    pub granted_unix: u64,
    #[serde(default)]
    pub found_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_unix: Option<u64>,
}

impl AppGrant {
    /// The installation on `owner`'s account.
    pub fn installation_of(&self, owner: &str) -> Option<&Installation> {
        self.installations
            .iter()
            .find(|i| i.account.eq_ignore_ascii_case(owner))
    }
}

/// Whether this account has authorized the GitHub App.
pub(super) fn uses_app(dir: &Path, account: &str) -> Result<bool, RepoError> {
    Ok(load(dir, account)?.app.is_some())
}

fn bound(account: &str, github_id: u64, kind: &str) -> String {
    format!("{SCHEMA}\0app-{kind}\0{account}\0{github_id}")
}

/// Finish the App's user authorization: exchange the code, check the
/// GitHub user is the one this account signs in with, keep the tokens
/// sealed, and find the installations.
pub(super) async fn grant(
    dir: &Path,
    app: &AppClient,
    account: &str,
    request: &CodeRequest,
) -> Result<Status, RepoError> {
    check_account(account)?;
    let tokens = app
        .github()
        .exchange_tokens(&request.code, &request.code_verifier)
        .await?;
    let user = app.github().api(tokens.access.as_str(), "/user").await?;
    let user = match user.status {
        401 => return Err(RepoError::Denied),
        _ => answered(user)?,
    };
    let (github_id, login) = person(&user.body)?;
    same_person(dir, account, github_id)?;
    let now = now();
    let sealed = seal_bound(
        app.github(),
        &bound(account, github_id, "access"),
        tokens.access.as_str(),
    )?;
    let sealed_refresh = tokens
        .refresh
        .as_ref()
        .map(|r| {
            seal_bound(
                app.github(),
                &bound(account, github_id, "refresh"),
                r.as_str(),
            )
        })
        .transpose()?;
    mutate(dir, account, |record| {
        record.app = Some(AppGrant {
            github_id,
            login,
            sealed,
            expires_unix: tokens.expires_in.map(|s| now + s),
            sealed_refresh,
            installations: Vec::new(),
            granted_unix: now,
            found_unix: 0,
            revoked_unix: None,
        });
        Ok(())
    })?;
    refresh(dir, app, account).await
}

fn person(body: &Value) -> Result<(u64, String), RepoError> {
    let id = body["id"].as_u64().ok_or(RepoError::BadAnswer)?;
    let login = body["login"]
        .as_str()
        .filter(|login| {
            !login.is_empty()
                && login.len() <= 64
                && login
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .ok_or(RepoError::BadAnswer)?;
    Ok((id, login.to_string()))
}

/// Find the person's installations again with their user token.
pub(super) async fn refresh(
    dir: &Path,
    app: &AppClient,
    account: &str,
) -> Result<Status, RepoError> {
    let mut found: Vec<Installation> = Vec::new();
    let mut path = Some("/user/installations?per_page=100".to_string());
    for _ in 0..10 {
        let Some(next) = path.take() else { break };
        let answer = answered(user_api(dir, app, account, &next).await?)?;
        let rows = answer.body["installations"]
            .as_array()
            .ok_or(RepoError::BadAnswer)?;
        for row in rows {
            if let Some(installation) = installation(row)
                && !found.iter().any(|i| i.id == installation.id)
            {
                found.push(installation);
            }
        }
        path = answer.next;
    }
    found.truncate(MAX_INSTALLATIONS);
    mutate(dir, account, |record| {
        if let Some(grant) = record.app.as_mut() {
            grant.installations = found;
            grant.found_unix = now();
        }
        Ok(())
    })?;
    status(dir, account)
}

fn installation(row: &Value) -> Option<Installation> {
    let account = row["account"]["login"]
        .as_str()
        .filter(|l| !l.is_empty() && l.len() <= 100 && full_name(&format!("{l}/x")))?;
    Some(Installation {
        id: row["id"].as_u64().filter(|id| *id > 0)?,
        account: account.to_string(),
        organization: row["account"]["type"].as_str() == Some("Organization")
            || row["target_type"].as_str() == Some("Organization"),
        all: row["repository_selection"].as_str() == Some("all"),
        suspended: !row["suspended_at"].is_null(),
    })
}

/// One page (1-based) of the repositories the person can reach in each of
/// their installations; `more` when any installation has a next page.
pub(super) async fn repositories(
    dir: &Path,
    app: &AppClient,
    account: &str,
    page: u32,
) -> Result<Listing, RepoError> {
    let page = page.clamp(1, MAX_PAGE);
    let grant = load(dir, account)?.app.ok_or(RepoError::NotConnected)?;
    if grant.revoked_unix.is_some() {
        return Err(RepoError::Reconnect);
    }
    let mut listing = Listing::default();
    for installation in grant
        .installations
        .iter()
        .filter(|i| !i.suspended)
        .take(LISTED_INSTALLATIONS)
    {
        let answer = user_api(
            dir,
            app,
            account,
            &format!(
                "/user/installations/{}/repositories?per_page={PAGE_SIZE}&page={page}",
                installation.id
            ),
        )
        .await?;
        // Removed from that account since it was found.
        if answer.status == 404 {
            continue;
        }
        let answer = answered(answer)?;
        let rows = answer.body["repositories"]
            .as_array()
            .ok_or(RepoError::BadAnswer)?;
        for row in rows {
            if let Some(mut repository) = listed(row)
                && !listing.repositories.iter().any(|r| r.id == repository.id)
            {
                repository.installation = Some(installation.id);
                listing.repositories.push(repository);
            }
        }
        listing.more |= answer.next.is_some() && page < MAX_PAGE;
    }
    Ok(listing)
}

/// Add `repository` (`owner/name`) as a project through the App: the
/// person's user token must reach it, and the App must be installed on
/// its account.
pub(super) async fn add_project(
    dir: &Path,
    app: &AppClient,
    account: &str,
    repository: &str,
) -> Result<Project, RepoError> {
    if !full_name(repository) {
        return Err(RepoError::Invalid);
    }
    let answer = user_api(dir, app, account, &format!("/repos/{repository}")).await?;
    // A user token sees only repositories the App is installed on.
    if answer.status == 404 {
        return Err(RepoError::NotInstalled);
    }
    let body = answered(answer)?.body;
    if body["disabled"].as_bool() == Some(true) {
        return Err(RepoError::Forbidden);
    }
    let found = listed(&body).ok_or(RepoError::BadAnswer)?;
    let owner = found
        .full_name
        .split_once('/')
        .map_or("", |(owner, _)| owner)
        .to_string();
    let installation = match installation_for(dir, account, &owner)? {
        Some(found) => found,
        None => {
            // Installed since the installations were last read.
            refresh(dir, app, account).await?;
            installation_for(dir, account, &owner)?.ok_or(RepoError::NotInstalled)?
        }
    };
    if installation.suspended {
        return Err(RepoError::Suspended);
    }
    keep_project(dir, account, &found, Some(installation.id))
}

fn installation_for(
    dir: &Path,
    account: &str,
    owner: &str,
) -> Result<Option<Installation>, RepoError> {
    Ok(load(dir, account)?
        .app
        .and_then(|grant| grant.installation_of(owner).cloned()))
}

/// The person's App user token, renewed when it has under a minute left.
pub(super) async fn user_token(
    dir: &Path,
    app: &AppClient,
    account: &str,
) -> Result<Secret, RepoError> {
    token_unless(dir, app, account, None).await
}

/// A GitHub read with the person's App user token, renewed and tried once
/// more when GitHub answers 401; a second 401 ends the access.
async fn user_api(
    dir: &Path,
    app: &AppClient,
    account: &str,
    path: &str,
) -> Result<Api, RepoError> {
    let token = token_unless(dir, app, account, None).await?;
    let answer = app.github().api(token.as_str(), path).await?;
    if answer.status != 401 {
        return Ok(answer);
    }
    let again = token_unless(dir, app, account, Some(token.as_str())).await?;
    let answer = app.github().api(again.as_str(), path).await?;
    if answer.status == 401 {
        end(dir, account)?;
        return Err(RepoError::Reconnect);
    }
    Ok(answer)
}

/// The user token, renewed when it is about to expire or is `refused`
/// (GitHub just answered 401 to it). One renewal per account at a time.
async fn token_unless(
    dir: &Path,
    app: &AppClient,
    account: &str,
    refused: Option<&str>,
) -> Result<Secret, RepoError> {
    let lock = account_lock(dir, account);
    let _held = lock.lock().await;
    let grant = load(dir, account)?.app.ok_or(RepoError::NotConnected)?;
    if grant.revoked_unix.is_some() {
        return Err(RepoError::Reconnect);
    }
    let current = open_bound(
        app.github(),
        &bound(account, grant.github_id, "access"),
        &grant.sealed,
    )?;
    let expiring = grant.expires_unix.is_some_and(|at| at <= now() + 60);
    let was_refused = refused.is_some_and(|r| r == current.as_str());
    if !expiring && !was_refused {
        return Ok(current);
    }
    let Some(sealed_refresh) = &grant.sealed_refresh else {
        // No refresh token: a refused token can't be renewed.
        if was_refused {
            end(dir, account)?;
            return Err(RepoError::Reconnect);
        }
        return Ok(current);
    };
    let refresh = open_bound(
        app.github(),
        &bound(account, grant.github_id, "refresh"),
        sealed_refresh,
    )?;
    let tokens = match app.github().refresh(&refresh).await {
        Ok(tokens) => tokens,
        Err(AuthError::Denied) => {
            end(dir, account)?;
            return Err(RepoError::Reconnect);
        }
        Err(_) => return Err(RepoError::Unavailable),
    };
    let sealed = seal_bound(
        app.github(),
        &bound(account, grant.github_id, "access"),
        tokens.access.as_str(),
    )?;
    let sealed_refresh = tokens
        .refresh
        .as_ref()
        .map(|r| {
            seal_bound(
                app.github(),
                &bound(account, grant.github_id, "refresh"),
                r.as_str(),
            )
        })
        .transpose()?;
    let now = now();
    mutate(dir, account, |record| {
        if let Some(kept) = record.app.as_mut()
            && kept.github_id == grant.github_id
        {
            kept.sealed = sealed;
            kept.expires_unix = tokens.expires_in.map(|s| now + s);
            if sealed_refresh.is_some() {
                kept.sealed_refresh = sealed_refresh;
            }
        }
        Ok(())
    })?;
    Ok(tokens.access)
}

/// GitHub stopped accepting the App's user authorization.
fn end(dir: &Path, account: &str) -> Result<(), RepoError> {
    mutate(dir, account, |record| {
        if let Some(grant) = record.app.as_mut()
            && grant.revoked_unix.is_none()
        {
            grant.revoked_unix = Some(now());
        }
        Ok(())
    })
}

fn account_lock(dir: &Path, account: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<
        Mutex<HashMap<(std::path::PathBuf, String), Arc<tokio::sync::Mutex<()>>>>,
    > = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("account locks");
    if locks.len() > 4096 {
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    }
    locks
        .entry((dir.to_path_buf(), account.to_string()))
        .or_default()
        .clone()
}
