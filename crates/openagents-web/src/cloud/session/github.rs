//! The signed-in person's connected GitHub repositories and projects, read
//! and changed at the account service under their session (docs/auth,
//! "Repository access"). The account service keeps the GitHub token
//! encrypted; [`CloudSession::github_token`] reads it only to call GitHub
//! as the person, and it is never stored or logged here.

use axum::http::HeaderMap;
use oa_auth::repos::{Listing, Project, RepoError, Status};
use reqwest::Method;
use serde_json::{Value, json};
use std::time::Duration;

use super::{CloudSession, SESSION_COOKIE, SessionError, session_token, value};

/// Why a repository call didn't complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RepoCallError {
    /// Signing in, the session, or the account service.
    Session(SessionError),
    /// What the account service said about GitHub or the request.
    Repo(RepoError),
}

impl std::fmt::Display for RepoCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Session(error) => std::fmt::Display::fmt(error, f),
            Self::Repo(error) => std::fmt::Display::fmt(error, f),
        }
    }
}

impl From<SessionError> for RepoCallError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

type Result<T> = std::result::Result<T, RepoCallError>;

/// The largest answer read from the account service.
const BODY_MAX: usize = 1024 * 1024;

impl CloudSession {
    /// The account's GitHub access and projects.
    pub(crate) async fn github_status(&self, headers: &HeaderMap) -> Result<Status> {
        let body = self
            .account_call(headers, Method::GET, "/v1/account/github", None)
            .await?;
        Status::from_body(&body).ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// The same, under a signed-in app's own token (`coder export
    /// --account`, #11134).
    pub(crate) async fn github_status_as(&self, token: &str) -> Result<Status> {
        let body = self
            .account_call_as(token, Method::GET, "/v1/account/github", None, None)
            .await?;
        Status::from_body(&body).ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// Finish connecting repositories with GitHub's code and its verifier.
    pub(crate) async fn github_grant(
        &self,
        headers: &HeaderMap,
        code: &str,
        verifier: &str,
    ) -> Result<Status> {
        let body = self
            .account_call(
                headers,
                Method::POST,
                "/v1/account/github/grant",
                Some(json!({"code": code, "code_verifier": verifier})),
            )
            .await?;
        Status::from_body(&body).ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// Finish authorizing the GitHub App with GitHub's code and its
    /// verifier; the account service finds where it is installed.
    pub(crate) async fn github_app_grant(
        &self,
        headers: &HeaderMap,
        code: &str,
        verifier: &str,
    ) -> Result<Status> {
        let body = self
            .account_call(
                headers,
                Method::POST,
                "/v1/account/github/app/grant",
                Some(json!({"code": code, "code_verifier": verifier})),
            )
            .await?;
        Status::from_body(&body).ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// Find where the GitHub App is installed again (after GitHub's
    /// install page sent the person back).
    pub(crate) async fn github_app_refresh(&self, headers: &HeaderMap) -> Result<Status> {
        let body = self
            .account_call(
                headers,
                Method::POST,
                "/v1/account/github/app/refresh",
                None,
            )
            .await?;
        Status::from_body(&body).ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// Forget the stored GitHub token; projects stay.
    pub(crate) async fn github_disconnect(&self, headers: &HeaderMap) -> Result<Status> {
        let body = self
            .account_call(headers, Method::DELETE, "/v1/account/github/grant", None)
            .await?;
        Status::from_body(&body).ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// One page of the repositories the person can connect, most recently
    /// pushed first, whether there are more, and whether single sign-on
    /// hid some.
    pub(crate) async fn github_repositories(
        &self,
        headers: &HeaderMap,
        page: u32,
    ) -> Result<Listing> {
        let body = self
            .account_call(
                headers,
                Method::GET,
                &format!("/v1/account/github/repositories?page={page}"),
                None,
            )
            .await?;
        serde_json::from_value(body).map_err(|_| RepoCallError::Session(SessionError::Unavailable))
    }

    /// The person's GitHub token, to read GitHub as them for this request.
    pub(crate) async fn github_token(&self, headers: &HeaderMap) -> Result<String> {
        let body = self
            .account_call(headers, Method::POST, "/v1/account/github/token", None)
            .await?;
        body["token"]
            .as_str()
            .filter(|token| !token.is_empty() && token.len() <= 1024)
            .map(str::to_string)
            .ok_or(RepoCallError::Session(SessionError::Unavailable))
    }

    /// Add `repository` (`owner/name`) as a project.
    pub(crate) async fn add_project(
        &self,
        headers: &HeaderMap,
        repository: &str,
    ) -> Result<Project> {
        if !oa_auth::repos::full_name(repository) {
            return Err(RepoCallError::Repo(RepoError::Invalid));
        }
        let body = self
            .account_call_or(
                headers,
                Method::POST,
                "/v1/projects",
                Some("/v1/account/projects"),
                Some(json!({"repository": repository})),
            )
            .await?;
        serde_json::from_value(body["project"].clone())
            .map_err(|_| RepoCallError::Session(SessionError::Unavailable))
    }

    /// Remove a project.
    pub(crate) async fn remove_project(&self, headers: &HeaderMap, id: &str) -> Result<()> {
        if !oa_auth::repos::project_id(id) {
            return Err(RepoCallError::Repo(RepoError::Invalid));
        }
        self.account_call_or(
            headers,
            Method::DELETE,
            &format!("/v1/projects/{id}"),
            Some(&format!("/v1/account/projects/{id}")),
            None,
        )
        .await
        .map(|_| ())
    }

    /// One account-service call under the request's session cookie.
    async fn account_call(
        &self,
        headers: &HeaderMap,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        self.account_call_or(headers, method, path, None, body)
            .await
    }

    /// [`Self::account_call`] at `path`, then at `older` when the account
    /// service doesn't serve `path` (a `404` without an error of its own):
    /// a service from before #11158 has projects only under
    /// `/v1/account/projects`.
    async fn account_call_or(
        &self,
        headers: &HeaderMap,
        method: Method,
        path: &str,
        older: Option<&str>,
        body: Option<Value>,
    ) -> Result<Value> {
        self.request_host(headers)?;
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        self.account_call_as(&token, method, path, older, body)
            .await
    }

    /// [`Self::account_call_or`] under a user session token: the browser's
    /// cookie, or a signed-in app's own token (#11134).
    async fn account_call_as(
        &self,
        token: &str,
        method: Method,
        path: &str,
        older: Option<&str>,
        body: Option<Value>,
    ) -> Result<Value> {
        if !session_token(token) {
            return Err(SessionError::Unauthenticated.into());
        }
        self.ready()?;
        let mut path = path;
        let mut older = older;
        let (status, body) = loop {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{path}", self.account_service))
                .bearer_auth(token)
                // A call may read GitHub up to three times.
                .timeout(Duration::from_secs(30));
            if let Some(body) = &body {
                request = request.json(body);
            }
            let mut response = request
                .send()
                .await
                .map_err(|_| SessionError::Unavailable)?;
            let status = response.status();
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| SessionError::Unavailable)?
            {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > BODY_MAX {
                    return Err(SessionError::Unavailable.into());
                }
            }
            let answered: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            if status.as_u16() == 404
                && answered["error"]["code"]
                    .as_str()
                    .and_then(RepoError::from_code)
                    .is_none()
                && let Some(next) = older.take()
            {
                path = next;
                continue;
            }
            break (status, answered);
        };
        if status.is_success() {
            return Ok(body);
        }
        if let Some(error) = body["error"]["code"]
            .as_str()
            .and_then(RepoError::from_code)
        {
            return Err(RepoCallError::Repo(error));
        }
        Err(RepoCallError::Session(match status.as_u16() {
            401 => SessionError::Unauthenticated,
            403 => SessionError::Forbidden,
            _ => SessionError::Unavailable,
        }))
    }
}
