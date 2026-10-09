//! A fake GitHub for tests and the local fixture: the authorize page, the
//! token endpoint (with real PKCE S256 checking and single-use codes),
//! `/user` and `/user/emails`, and for connecting repositories
//! `/user/repos` (paged as GitHub pages it, with `Link` headers) and
//! `/repos/{owner}/{name}` with whole, real-sized repository objects
//! (private ones only for a token granted `repo`). [`Fake::fail`] makes
//! reads fail the ways github.com does (rate limits, single sign-on,
//! organization restrictions, 5xx, an HTML page). Reads without a token
//! see public repositories and their branches
//! (`/repos/{owner}/{name}/branches[/{branch}]`) under GitHub's anonymous
//! limit of 60 an hour for the whole server ([`ANONYMOUS_LIMIT`]); reads
//! with a token get 5,000 ([`Fake::limit`] lowers it). A read naming an
//! `X-GitHub-Api-Version` GitHub doesn't know is refused with 400, as
//! GitHub does. Nothing here talks to github.com.
//!
//! `GET /login/oauth/authorize` shows a small page listing the fake people
//! (click one to approve as them) and a Cancel link. Tests skip the page
//! with `&login=<login>` (approve) or `&deny=1` (cancel).
//!
//! With [`Fake::with_app`] it is also a GitHub App ([`app`]): the App's
//! own OAuth client (expiring user tokens and refresh tokens), its install
//! page, `/user/installations` and their repositories, and installation
//! tokens minted only for a JWT the App's key signed.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::config::{Endpoints, GithubApp, GithubCredentials};

mod app;
pub use app::{FakeApp, FakeInstallation, app_credentials, app_key};

/// One fake GitHub person: the `/user` body, the `/user/emails` list
/// (`None` answers 404, as GitHub does without the email scope), and their
/// repositories (`id`, `full_name`, `default_branch`, `private`).
#[derive(Clone, Debug)]
pub struct FakeUser {
    pub user: Value,
    pub emails: Option<Value>,
    pub repos: Vec<Value>,
}

impl FakeUser {
    pub(crate) fn login(&self) -> &str {
        self.user["login"].as_str().unwrap_or_default()
    }
}

/// A full profile: name, verified primary email, company, bio, counts.
#[must_use]
pub fn octo() -> FakeUser {
    FakeUser {
        user: json!({
            "login": "octo-local", "id": 583231, "node_id": "MDQ6VXNlcjU4MzIzMQ==",
            "avatar_url": "https://avatars.githubusercontent.com/u/583231?v=4",
            "html_url": "https://github.com/octo-local", "type": "User", "site_admin": false,
            "name": "Octo Local", "company": "@openagents", "blog": "https://openagents.com",
            "location": "Austin, TX", "email": null, "hireable": true,
            "bio": "Builds agents.\nLikes Rust.", "twitter_username": "octolocal",
            "public_repos": 42, "public_gists": 3, "followers": 100, "following": 7,
            "created_at": "2011-01-25T18:44:36Z", "updated_at": "2026-10-01T12:00:00Z"
        }),
        emails: Some(json!([
            {"email": "octo@example.com", "verified": true, "primary": true, "visibility": "private"},
            {"email": "octo@users.noreply.github.com", "verified": true, "primary": false, "visibility": null},
            {"email": "old@example.com", "verified": false, "primary": false, "visibility": null}
        ])),
        repos: vec![
            json!({"id": 7001, "full_name": "octo-local/hello-world", "name": "hello-world", "default_branch": "main", "private": false}),
            json!({"id": 7002, "full_name": "octo-local/secret-plans", "name": "secret-plans", "default_branch": "trunk", "private": true}),
            json!({"id": 7003, "full_name": "acme/storefront", "name": "storefront", "default_branch": "main", "private": true}),
        ],
    }
}

/// A sparse profile: no name, no addresses at all.
#[must_use]
pub fn quiet() -> FakeUser {
    FakeUser {
        user: json!({
            "login": "quiet-local", "id": 9000001, "type": "User", "name": null,
            "email": null, "blog": "", "bio": null, "public_repos": 0,
            "created_at": "2024-02-02T00:00:00Z"
        }),
        emails: None,
        repos: Vec::new(),
    }
}

pub(crate) struct Grant {
    user: usize,
    challenge: String,
    redirect: String,
    scopes: Vec<String>,
    /// Issued to the GitHub App's client.
    app: bool,
}

pub(crate) struct Inner {
    client_id: String,
    client_secret: String,
    redirect: String,
    users: Vec<FakeUser>,
    codes: BTreeMap<String, Grant>,
    tokens: BTreeMap<String, (usize, Vec<String>)>,
    exchanges: usize,
    faults: Vec<(String, Fault, usize, usize)>,
    sso_partial: bool,
    api_calls: usize,
    anonymous_calls: usize,
    token_limit: usize,
    app: Option<app::AppState>,
}

/// GitHub's hourly limit for reads without a token, per address.
pub const ANONYMOUS_LIMIT: usize = 60;

/// A running fake GitHub's shared state.
#[derive(Clone)]
pub struct Fake(Arc<Mutex<Inner>>);

impl Fake {
    /// A fake that accepts `client_id`/`client_secret` and exactly
    /// `redirect` as the callback.
    #[must_use]
    pub fn new(client_id: &str, client_secret: &str, redirect: &str, users: Vec<FakeUser>) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect: redirect.into(),
            users,
            codes: BTreeMap::new(),
            tokens: BTreeMap::new(),
            exchanges: 0,
            faults: Vec::new(),
            sso_partial: false,
            api_calls: 0,
            anonymous_calls: 0,
            token_limit: 5000,
            app: None,
        })))
    }

    /// Replace a person's `/user` body by login (a GitHub rename, a new bio).
    pub fn update(&self, login: &str, user: Value) {
        let mut inner = self.0.lock().expect("fake GitHub state");
        if let Some(found) = inner.users.iter_mut().find(|u| u.login() == login) {
            found.user = user;
        }
    }

    /// Revoke every token issued to `login`, as when the person removes
    /// the App's access on GitHub.
    pub fn revoke(&self, login: &str) {
        let mut inner = self.0.lock().expect("fake GitHub state");
        if let Some(index) = inner.users.iter().position(|u| u.login() == login) {
            inner.tokens.retain(|_, (user, _)| *user != index);
        }
    }

    /// How many token exchanges succeeded.
    #[must_use]
    pub fn exchanges(&self) -> usize {
        self.0.lock().expect("fake GitHub state").exchanges
    }

    /// Answer the next `times` API reads whose path starts with `path`
    /// (`/user/repos`, `/repos/acme/storefront`, `/user`) with `fault`,
    /// the way github.com does.
    pub fn fail(&self, path: &str, fault: Fault, times: usize) {
        self.fail_later(path, 0, fault, times);
    }

    /// As [`Fake::fail`], after `skip` matching reads succeed (fail the
    /// second page of a listing with `skip` 1).
    pub fn fail_later(&self, path: &str, skip: usize, fault: Fault, times: usize) {
        self.0
            .lock()
            .expect("fake GitHub state")
            .faults
            .push((path.into(), fault, skip, times));
    }

    /// Mark `/user/repos` answers `X-GitHub-SSO: partial-results`: an
    /// organization that uses single sign-on hid its repositories.
    pub fn sso_partial(&self) {
        self.0.lock().expect("fake GitHub state").sso_partial = true;
    }

    /// How many API reads (`/user...`, `/repos/...`) were answered.
    #[must_use]
    pub fn api_calls(&self) -> usize {
        self.0.lock().expect("fake GitHub state").api_calls
    }

    /// How many of those were read without a token.
    #[must_use]
    pub fn anonymous_calls(&self) -> usize {
        self.0.lock().expect("fake GitHub state").anonymous_calls
    }

    /// Set the hourly limit for reads with a token (`x-ratelimit-limit`;
    /// `x-ratelimit-remaining` counts down from it).
    pub fn limit(&self, limit: usize) {
        self.0.lock().expect("fake GitHub state").token_limit = limit;
    }

    /// The router (mount it at the origin's root).
    pub fn router(&self) -> Router {
        app::routes(Router::new())
            .route("/login/oauth/authorize", get(authorize))
            .route("/login/oauth/access_token", post(token))
            .route("/user", get(user))
            .route("/user/emails", get(emails))
            .route("/user/repos", get(repos))
            .route("/repos/{owner}/{name}", get(repo))
            .route("/repos/{owner}/{name}/branches", get(branches))
            .route("/repos/{owner}/{name}/branches/{*branch}", get(branch))
            .layer(axum::middleware::from_fn_with_state(
                self.clone(),
                api_layer,
            ))
            .with_state(self.clone())
    }

    /// Serve on `127.0.0.1:0`; answers the origin (`http://127.0.0.1:port`).
    pub async fn spawn(&self) -> std::io::Result<String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let router = self.router();
        tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });
        Ok(origin)
    }
}

/// Credentials that match a [`Fake`] at `origin`.
pub fn credentials(
    origin: &str,
    client_id: &str,
    client_secret: &str,
    redirect: &str,
) -> Result<GithubCredentials, String> {
    let app = GithubApp::new(client_id, redirect, Endpoints::at(origin))?;
    GithubCredentials::new(app, client_secret, [9; 32])
}

#[derive(Deserialize)]
struct Authorize {
    client_id: String,
    redirect_uri: Option<String>,
    state: String,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    login: Option<String>,
    deny: Option<String>,
    scope: Option<String>,
    #[allow(dead_code)]
    response_type: Option<String>,
}

pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn back(redirect: &str, pairs: &[(&str, &str)]) -> Response {
    let mut url = url::Url::parse(redirect).expect("configured redirect parses");
    url.query_pairs_mut().extend_pairs(pairs);
    Redirect::to(url.as_str()).into_response()
}

async fn authorize(
    State(fake): State<Fake>,
    uri: axum::http::Uri,
    Query(query): Query<Authorize>,
) -> Response {
    let mut inner = fake.0.lock().expect("fake GitHub state");
    let redirect = query
        .redirect_uri
        .clone()
        .unwrap_or_else(|| inner.redirect.clone());
    let for_app = inner
        .app
        .as_ref()
        .is_some_and(|app| app.app.client_id == query.client_id);
    if !(query.client_id == inner.client_id || for_app) || redirect != inner.redirect {
        return (
            StatusCode::BAD_REQUEST,
            "The redirect_uri is not associated with this application.",
        )
            .into_response();
    }
    let (Some(challenge), Some("S256")) = (
        query.code_challenge.clone(),
        query.code_challenge_method.as_deref(),
    ) else {
        return (
            StatusCode::BAD_REQUEST,
            "PKCE S256 is required by this fake.",
        )
            .into_response();
    };
    if query.deny.is_some() {
        return back(
            &redirect,
            &[
                ("error", "access_denied"),
                (
                    "error_description",
                    "The user has denied your application access.",
                ),
                ("state", &query.state),
            ],
        );
    }
    if let Some(login) = &query.login {
        let Some(index) = inner.users.iter().position(|u| u.login() == login) else {
            return (StatusCode::NOT_FOUND, "No such fake person.").into_response();
        };
        let code = random_hex(10);
        inner.codes.insert(
            code.clone(),
            Grant {
                user: index,
                challenge,
                redirect: redirect.clone(),
                scopes: query
                    .scope
                    .as_deref()
                    .unwrap_or_default()
                    .split([' ', ','])
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
                app: for_app,
            },
        );
        return back(&redirect, &[("code", &code), ("state", &query.state)]);
    }
    let base = uri.to_string();
    let people: String = inner
        .users
        .iter()
        .map(|u| {
            format!(
                "<li><a href=\"{}\">Continue as {}</a></li>",
                escape(&format!("{base}&login={}", u.login())),
                escape(u.login())
            )
        })
        .collect();
    Html(format!(
        "<!doctype html><meta charset=utf-8><title>Fake GitHub</title>\
         <h1>Fake GitHub</h1><p>This is the local test GitHub. Pick who to sign in as.</p>\
         <ul>{people}</ul><p><a href=\"{}\">Cancel</a></p>",
        escape(&format!("{base}&deny=1"))
    ))
    .into_response()
}

#[derive(Deserialize)]
pub(crate) struct Exchange {
    client_id: Option<String>,
    client_secret: Option<String>,
    #[serde(default)]
    code: String,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    grant_type: Option<String>,
    refresh_token: Option<String>,
}

/// The redirect and PKCE verifier checks every code exchange makes.
pub(crate) fn checked_grant(grant: &Grant, form: &Exchange) -> Option<Response> {
    if form
        .redirect_uri
        .as_deref()
        .is_some_and(|r| r != grant.redirect)
    {
        return Some(bad("redirect_uri_mismatch"));
    }
    let verifier = form.code_verifier.clone().unwrap_or_default();
    if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return Some(bad("bad_verification_code"));
    }
    None
}

pub(crate) fn bad(code: &str) -> Response {
    // GitHub answers token errors with 200 and an error body.
    axum::Json(json!({"error": code, "error_description": "Fake GitHub refused the exchange."}))
        .into_response()
}

async fn token(State(fake): State<Fake>, Form(form): Form<Exchange>) -> Response {
    let mut inner = fake.0.lock().expect("fake GitHub state");
    if let Some(answer) = app::token(&mut inner, &form) {
        return answer;
    }
    if form.client_id.as_deref() != Some(&inner.client_id)
        || form.client_secret.as_deref() != Some(&inner.client_secret)
    {
        return bad("incorrect_client_credentials");
    }
    // Single use: the code is gone whether or not the rest checks out.
    // A code issued to the GitHub App's client is only for that client.
    let Some(grant) = inner.codes.remove(&form.code).filter(|g| !g.app) else {
        return bad("bad_verification_code");
    };
    if let Some(refused) = checked_grant(&grant, &form) {
        return refused;
    }
    let access = format!("gho_{}", random_hex(18));
    let scope = grant.scopes.join(",");
    inner
        .tokens
        .insert(access.clone(), (grant.user, grant.scopes));
    inner.exchanges += 1;
    axum::Json(json!({"access_token": access, "token_type": "bearer", "scope": scope}))
        .into_response()
}

fn holder(fake: &Fake, headers: &HeaderMap) -> Option<(FakeUser, Vec<String>)> {
    let token = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    let inner = fake.0.lock().expect("fake GitHub state");
    if let Some(user) = app::user_of(&inner, token) {
        return Some((inner.users[user].clone(), vec![app::USER_SCOPE.to_string()]));
    }
    inner
        .tokens
        .get(token)
        .map(|(i, scopes)| (inner.users[*i].clone(), scopes.clone()))
}

fn person(fake: &Fake, headers: &HeaderMap) -> Option<FakeUser> {
    holder(fake, headers).map(|(user, _)| user)
}

fn bad_credentials() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(json!({"message": "Bad credentials"})),
    )
        .into_response()
}

/// The person's repositories the token can see: private ones need `repo`.
fn visible(user: &FakeUser, scopes: &[String]) -> Vec<Value> {
    let private = scopes.iter().any(|s| s == "repo");
    user.repos
        .iter()
        .filter(|r| private || !r["private"].as_bool().unwrap_or(false))
        .cloned()
        .collect()
}

#[derive(Deserialize)]
struct Page {
    page: Option<usize>,
    per_page: Option<usize>,
}

/// `/user/repos` as GitHub pages it: `per_page` (default 30, at most
/// 100), `page` from 1, and a `Link` header naming the next and last
/// pages with the rest of the query kept.
async fn repos(
    State(fake): State<Fake>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(page): Query<Page>,
) -> Response {
    let Some((user, scopes)) = holder(&fake, &headers) else {
        return bad_credentials();
    };
    let all = visible(&user, &scopes);
    let per = page.per_page.unwrap_or(30).clamp(1, 100);
    let number = page.page.unwrap_or(1).max(1);
    let last = all.len().div_ceil(per).max(1);
    let rows: Vec<Value> = all
        .iter()
        .skip((number - 1).saturating_mul(per))
        .take(per)
        .map(inflate)
        .collect();
    let mut response = axum::Json(rows).into_response();
    if let Some(value) = link_header(&headers, &uri, number, last) {
        response.headers_mut().insert(header::LINK, value);
    }
    if fake.0.lock().expect("fake GitHub state").sso_partial {
        response.headers_mut().insert(
            "x-github-sso",
            axum::http::HeaderValue::from_static(
                "partial-results; organizations=21955855,20582480",
            ),
        );
    }
    response
}

/// GitHub's `Link` header for page `number` of `last`, keeping the rest of
/// the request's query.
pub(crate) fn link_header(
    headers: &HeaderMap,
    uri: &axum::http::Uri,
    number: usize,
    last: usize,
) -> Option<axum::http::HeaderValue> {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1");
    let link = |n: usize| {
        let mut url = url::Url::parse(&format!("http://{host}{uri}")).expect("request URL");
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| k != "page")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        url.query_pairs_mut()
            .clear()
            .extend_pairs(kept)
            .append_pair("page", &n.to_string());
        url.to_string()
    };
    let mut parts = Vec::new();
    if number > 1 {
        parts.push(format!("<{}>; rel=\"prev\"", link(number - 1)));
    }
    if number < last {
        parts.push(format!("<{}>; rel=\"next\"", link(number + 1)));
        parts.push(format!("<{}>; rel=\"last\"", link(last)));
    }
    if number > 1 {
        parts.push(format!("<{}>; rel=\"first\"", link(1)));
    }
    if parts.is_empty() {
        return None;
    }
    axum::http::HeaderValue::from_str(&parts.join(", ")).ok()
}

/// The repository `owner/name` the caller can see: any public one without
/// a token, the token holder's visible ones with one. `Err` is the answer
/// GitHub gives instead (401 for a token it doesn't know, else 404).
fn reachable(
    fake: &Fake,
    headers: &HeaderMap,
    owner: &str,
    name: &str,
) -> Result<Value, Box<Response>> {
    let full = format!("{owner}/{name}");
    let found = if headers.contains_key(header::AUTHORIZATION) {
        let Some((user, scopes)) = holder(fake, headers) else {
            return Err(Box::new(bad_credentials()));
        };
        let shown = if scopes.iter().any(|s| s == app::USER_SCOPE) {
            app::reachable(fake, &user)
        } else {
            visible(&user, &scopes)
        };
        shown.into_iter().find(|r| r["full_name"] == full.as_str())
    } else {
        let inner = fake.0.lock().expect("fake GitHub state");
        inner
            .users
            .iter()
            .flat_map(|u| u.repos.iter())
            .find(|r| r["full_name"] == full.as_str() && !r["private"].as_bool().unwrap_or(false))
            .cloned()
    };
    found.ok_or_else(|| {
        Box::new(
            (
                StatusCode::NOT_FOUND,
                axum::Json(json!({
                    "message": "Not Found",
                    "documentation_url": "https://docs.github.com/rest/repos/repos#get-a-repository",
                    "status": "404"
                })),
            )
                .into_response(),
        )
    })
}

async fn repo(
    State(fake): State<Fake>,
    headers: HeaderMap,
    axum::extract::Path((owner, name)): axum::extract::Path<(String, String)>,
) -> Response {
    let full = format!("{owner}/{name}");
    if let Some(answer) = app::repo_as_installation(&fake, &headers, &full) {
        return answer;
    }
    match reachable(&fake, &headers, &owner, &name) {
        Ok(found) => axum::Json(inflate(&found)).into_response(),
        Err(response) => *response,
    }
}

/// A repository's branches: its default branch, then any it names under
/// `"branches"`, each with a commit sha made from its names.
fn branch_rows(repo: &Value) -> Vec<Value> {
    let full = repo["full_name"].as_str().unwrap_or_default();
    let mut names = vec![
        repo["default_branch"]
            .as_str()
            .unwrap_or("main")
            .to_string(),
    ];
    if let Some(more) = repo["branches"].as_array() {
        names.extend(more.iter().filter_map(Value::as_str).map(str::to_string));
    }
    names
        .into_iter()
        .map(|name| {
            let sha: String = Sha256::digest(format!("{full}/{name}").as_bytes())[..20]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            json!({
                "name": name,
                "commit": {"sha": sha, "url": format!("https://api.github.com/repos/{full}/commits/{sha}")},
                "protected": false
            })
        })
        .collect()
}

async fn branches(
    State(fake): State<Fake>,
    headers: HeaderMap,
    axum::extract::Path((owner, name)): axum::extract::Path<(String, String)>,
) -> Response {
    match reachable(&fake, &headers, &owner, &name) {
        Ok(found) => axum::Json(branch_rows(&found)).into_response(),
        Err(response) => *response,
    }
}

async fn branch(
    State(fake): State<Fake>,
    headers: HeaderMap,
    axum::extract::Path((owner, name, wanted)): axum::extract::Path<(String, String, String)>,
) -> Response {
    let found = match reachable(&fake, &headers, &owner, &name) {
        Ok(found) => found,
        Err(response) => return *response,
    };
    match branch_rows(&found)
        .into_iter()
        .find(|b| b["name"] == wanted.as_str())
    {
        Some(row) => axum::Json(row).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"message": "Branch not found", "status": "404"})),
        )
            .into_response(),
    }
}

/// How the fake can fail an API read, each as github.com answers it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    /// The hourly limit is spent: 403, `x-ratelimit-remaining: 0`.
    RateLimited,
    /// A secondary (abuse) limit: 403 with `retry-after: 60`.
    SecondaryRateLimit,
    /// 429 with `retry-after`.
    TooManyRequests,
    /// A 5xx with GitHub's own message (502 is common for slow listings).
    ServerError(u16),
    /// The organization requires single sign-on: 403 and
    /// `X-GitHub-SSO: required; url=...`.
    SsoRequired,
    /// The organization restricts OAuth App access: 403.
    OrgRestricted,
    /// 200 with an HTML page (a captive proxy, an outage page).
    NotJson,
}

impl Fault {
    fn response(self) -> Response {
        let reset = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            + 1800)
            .to_string();
        let message = |status: u16, body: Value| {
            (
                StatusCode::from_u16(status).expect("fault status"),
                axum::Json(body),
            )
                .into_response()
        };
        let mut response = match self {
            Self::RateLimited => message(
                403,
                json!({"message": "API rate limit exceeded for user ID 583231. If you reach out to GitHub Support for help, please include the request ID.", "documentation_url": "https://docs.github.com/rest/overview/rate-limits-for-the-rest-api", "status": "403"}),
            ),
            Self::SecondaryRateLimit => message(
                403,
                json!({"message": "You have exceeded a secondary rate limit. Please wait a few minutes before you try again.", "documentation_url": "https://docs.github.com/free-pro-team@latest/rest/overview/rate-limits-for-the-rest-api#about-secondary-rate-limits", "status": "403"}),
            ),
            Self::TooManyRequests => message(
                429,
                json!({"message": "Too Many Requests", "status": "429"}),
            ),
            Self::ServerError(status) => message(
                status,
                json!({"message": "Server Error", "status": status.to_string()}),
            ),
            Self::SsoRequired => message(
                403,
                json!({"message": "Resource protected by organization SAML enforcement. You must grant your OAuth token access to this organization.", "documentation_url": "https://docs.github.com/articles/authenticating-to-a-github-organization-with-saml-single-sign-on/", "status": "403"}),
            ),
            Self::OrgRestricted => message(
                403,
                json!({"message": "Although you appear to have the correct authorization credentials, the `acme` organization has enabled OAuth App access restrictions, meaning that data access to third-parties is limited.", "documentation_url": "https://docs.github.com/articles/restricting-access-to-your-organization-s-data/", "status": "403"}),
            ),
            Self::NotJson => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                "<!DOCTYPE html><html><head><title>Unicorn!</title></head><body>No server is currently available to service your request.</body></html>",
            )
                .into_response(),
        };
        let headers = response.headers_mut();
        let set = |headers: &mut HeaderMap, name: &'static str, value: &str| {
            if let Ok(value) = axum::http::HeaderValue::from_str(value) {
                headers.insert(name, value);
            }
        };
        match self {
            Self::RateLimited => {
                set(headers, "x-ratelimit-remaining", "0");
                set(headers, "x-ratelimit-used", "5000");
                set(headers, "x-ratelimit-reset", &reset);
            }
            Self::SecondaryRateLimit | Self::TooManyRequests => {
                set(headers, "retry-after", "60");
            }
            Self::SsoRequired => set(
                headers,
                "x-github-sso",
                "required; url=https://github.com/orgs/acme/sso?authorization_request=AZSEXAMPLE",
            ),
            _ => {}
        }
        response
    }
}

/// Every API read: counted, refused when it names an API version GitHub
/// doesn't know, failed when a [`Fault`] is due or the caller's hourly
/// limit is spent, and answered with GitHub's rate-limit headers.
async fn api_layer(
    State(fake): State<Fake>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_string();
    if !(path.starts_with("/user") || path.starts_with("/repos/") || path.starts_with("/app/")) {
        return next.run(request).await;
    }
    if let Some(version) = request.headers().get("x-github-api-version")
        && version.as_bytes() != crate::github::API_VERSION.as_bytes()
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({
                "message": "Unsupported 'X-GitHub-Api-Version' header. The fake GitHub supports 2022-11-28.",
                "status": "400"
            })),
        )
            .into_response();
    }
    let anonymous = !request.headers().contains_key(header::AUTHORIZATION);
    let (fault, used, limit) = {
        let mut inner = fake.0.lock().expect("fake GitHub state");
        inner.api_calls += 1;
        if anonymous {
            inner.anonymous_calls += 1;
        }
        let (used, limit) = if anonymous {
            (inner.anonymous_calls, ANONYMOUS_LIMIT)
        } else {
            (inner.api_calls - inner.anonymous_calls, inner.token_limit)
        };
        let mut fault = None;
        for (prefix, due, skip, times) in &mut inner.faults {
            if *times == 0 || !path.starts_with(prefix.as_str()) {
                continue;
            }
            if *skip > 0 {
                *skip -= 1;
            } else {
                *times -= 1;
                fault = Some(*due);
            }
            break;
        }
        (fault, used, limit)
    };
    let reset = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        + 3600)
        .to_string();
    if used > limit {
        let mut response = if anonymous {
            (
                StatusCode::FORBIDDEN,
                axum::Json(json!({
                    "message": "API rate limit exceeded for 127.0.0.1. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)",
                    "documentation_url": "https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"
                })),
            )
                .into_response()
        } else {
            Fault::RateLimited.response()
        };
        for (name, value) in [
            ("x-ratelimit-limit", limit.to_string()),
            ("x-ratelimit-remaining", "0".to_string()),
            ("x-ratelimit-used", limit.to_string()),
            ("x-ratelimit-reset", reset),
        ] {
            if let Ok(value) = axum::http::HeaderValue::from_str(&value) {
                response.headers_mut().insert(name, value);
            }
        }
        return response;
    }
    let mut response = match fault {
        Some(fault) => return fault.response(),
        None => next.run(request).await,
    };
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-ratelimit-limit", limit.to_string()),
        (
            "x-ratelimit-remaining",
            limit.saturating_sub(used).to_string(),
        ),
        ("x-ratelimit-used", used.to_string()),
        ("x-ratelimit-reset", reset),
        ("x-ratelimit-resource", "core".to_string()),
        ("x-github-media-type", "github.v3; format=json".to_string()),
    ] {
        if let Ok(value) = axum::http::HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    }
    response
}

/// A repository as `/user/repos` and `/repos/{owner}/{name}` answer it:
/// every field GitHub sends, about 6 KB, with synthetic values.
#[must_use]
pub fn repository(id: u64, full_name: &str, private: bool) -> Value {
    let (owner, name) = full_name
        .split_once('/')
        .unwrap_or(("octo-local", full_name));
    let api = format!("https://api.github.com/repos/{full_name}");
    let owner_id = 10_000 + u64::from(owner.bytes().map(u32::from).sum::<u32>());
    let org = owner != "octo-local" && owner != "quiet-local";
    let mut repo = json!({
        "id": id,
        "node_id": format!("R_kgDO{id:08}"),
        "name": name,
        "full_name": full_name,
        "private": private,
        "owner": {
            "login": owner, "id": owner_id, "node_id": format!("MDQ6VXNlcj{owner_id}"),
            "avatar_url": format!("https://avatars.githubusercontent.com/u/{owner_id}?v=4"),
            "gravatar_id": "", "url": format!("https://api.github.com/users/{owner}"),
            "html_url": format!("https://github.com/{owner}"),
            "followers_url": format!("https://api.github.com/users/{owner}/followers"),
            "following_url": format!("https://api.github.com/users/{owner}/following{{/other_user}}"),
            "gists_url": format!("https://api.github.com/users/{owner}/gists{{/gist_id}}"),
            "starred_url": format!("https://api.github.com/users/{owner}/starred{{/owner}}{{/repo}}"),
            "subscriptions_url": format!("https://api.github.com/users/{owner}/subscriptions"),
            "organizations_url": format!("https://api.github.com/users/{owner}/orgs"),
            "repos_url": format!("https://api.github.com/users/{owner}/repos"),
            "events_url": format!("https://api.github.com/users/{owner}/events{{/privacy}}"),
            "received_events_url": format!("https://api.github.com/users/{owner}/received_events"),
            "type": if org { "Organization" } else { "User" },
            "user_view_type": "public", "site_admin": false
        },
        "html_url": format!("https://github.com/{full_name}"),
        "description": format!("Synthetic repository {name} for tests: a service, its command-line tools, deployment scripts, and documentation. Nothing here is real data."),
        "fork": false,
    });
    // Two macros: one would pass serde_json's macro recursion limit.
    let more = json!({
        "created_at": "2021-03-04T05:06:07Z",
        "updated_at": "2026-09-30T10:11:12Z",
        "pushed_at": "2026-10-01T08:09:10Z",
        "git_url": format!("git://github.com/{full_name}.git"),
        "ssh_url": format!("git@github.com:{full_name}.git"),
        "clone_url": format!("https://github.com/{full_name}.git"),
        "svn_url": format!("https://github.com/{full_name}"),
        "homepage": null,
        "size": 48213,
        "stargazers_count": 12,
        "watchers_count": 12,
        "language": "Rust",
        "has_issues": true,
        "has_projects": true,
        "has_downloads": true,
        "has_wiki": false,
        "has_pages": false,
        "has_discussions": false,
        "forks_count": 3,
        "mirror_url": null,
        "archived": false,
        "disabled": false,
        "open_issues_count": 7,
        "license": if private { Value::Null } else { json!({"key": "mit", "name": "MIT License", "spdx_id": "MIT", "url": "https://api.github.com/licenses/mit", "node_id": "MDc6TGljZW5zZTEz"}) },
        "allow_forking": true,
        "is_template": false,
        "web_commit_signoff_required": false,
        "topics": ["agents", "rust", "synthetic"],
        "visibility": if private { "private" } else { "public" },
        "forks": 3,
        "open_issues": 7,
        "watchers": 12,
        "default_branch": "main",
        "permissions": {"admin": !org, "maintain": !org, "push": true, "triage": true, "pull": true}
    });
    if let (Some(fields), Value::Object(more)) = (repo.as_object_mut(), more) {
        fields.extend(more);
        fields.insert("url".into(), json!(api));
        for (key, suffix) in [
            ("forks_url", "/forks"),
            ("keys_url", "/keys{/key_id}"),
            ("collaborators_url", "/collaborators{/collaborator}"),
            ("teams_url", "/teams"),
            ("hooks_url", "/hooks"),
            ("issue_events_url", "/issues/events{/number}"),
            ("events_url", "/events"),
            ("assignees_url", "/assignees{/user}"),
            ("branches_url", "/branches{/branch}"),
            ("tags_url", "/tags"),
            ("blobs_url", "/git/blobs{/sha}"),
            ("git_tags_url", "/git/tags{/sha}"),
            ("git_refs_url", "/git/refs{/sha}"),
            ("trees_url", "/git/trees{/sha}"),
            ("statuses_url", "/statuses/{sha}"),
            ("languages_url", "/languages"),
            ("stargazers_url", "/stargazers"),
            ("contributors_url", "/contributors"),
            ("subscribers_url", "/subscribers"),
            ("subscription_url", "/subscription"),
            ("commits_url", "/commits{/sha}"),
            ("git_commits_url", "/git/commits{/sha}"),
            ("comments_url", "/comments{/number}"),
            ("issue_comment_url", "/issues/comments{/number}"),
            ("contents_url", "/contents/{+path}"),
            ("compare_url", "/compare/{base}...{head}"),
            ("merges_url", "/merges"),
            ("archive_url", "/{archive_format}{/ref}"),
            ("downloads_url", "/downloads"),
            ("issues_url", "/issues{/number}"),
            ("pulls_url", "/pulls{/number}"),
            ("milestones_url", "/milestones{/number}"),
            (
                "notifications_url",
                "/notifications{?since,all,participating}",
            ),
            ("labels_url", "/labels{/name}"),
            ("releases_url", "/releases{/id}"),
            ("deployments_url", "/deployments"),
        ] {
            fields.insert(key.into(), json!(format!("{api}{suffix}")));
        }
    }
    repo
}

/// A short repository value (`id`, `full_name`, `private`, and any field
/// it sets) grown to GitHub's whole shape.
pub(crate) fn inflate(short: &Value) -> Value {
    let mut full = repository(
        short["id"].as_u64().unwrap_or_default(),
        short["full_name"].as_str().unwrap_or("octo-local/unnamed"),
        short["private"].as_bool().unwrap_or(false),
    );
    if let (Some(full), Some(short)) = (full.as_object_mut(), short.as_object()) {
        for (key, value) in short {
            full.insert(key.clone(), value.clone());
        }
    }
    full
}

/// A busy person: `count` repositories (ids from 80001) across their own
/// account and two organizations, every seventh private, every eleventh
/// archived, the fifth disabled by GitHub, and some with `null` default
/// branches the way empty repositories come back.
#[must_use]
pub fn busy(count: usize) -> FakeUser {
    FakeUser {
        user: json!({
            "login": "busy-local", "id": 7_700_001, "type": "User", "name": "Busy Local",
            "email": null, "public_repos": count, "created_at": "2012-02-02T00:00:00Z"
        }),
        emails: Some(json!([
            {"email": "busy@example.com", "verified": true, "primary": true, "visibility": "private"}
        ])),
        repos: (1..=count)
            .map(|n| {
                let owner = match n % 3 {
                    0 => "busy-local",
                    1 => "acme-corp",
                    _ => "example-labs",
                };
                let mut repo = json!({
                    "id": 80_000 + n as u64,
                    "full_name": format!("{owner}/project-{n:04}"),
                    "private": n % 7 == 0,
                });
                if n % 11 == 0 {
                    repo["archived"] = json!(true);
                }
                if n == 5 {
                    repo["disabled"] = json!(true);
                }
                if n % 50 == 13 {
                    repo["default_branch"] = Value::Null;
                }
                repo
            })
            .collect(),
    }
}

async fn user(State(fake): State<Fake>, headers: HeaderMap) -> Response {
    match holder(&fake, &headers) {
        Some((found, scopes)) => {
            let mut response = axum::Json(found.user).into_response();
            if let Ok(value) = axum::http::HeaderValue::from_str(&scopes.join(", ")) {
                response.headers_mut().insert("x-oauth-scopes", value);
            }
            response
        }
        None => (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"message": "Bad credentials"})),
        )
            .into_response(),
    }
}

async fn emails(State(fake): State<Fake>, headers: HeaderMap) -> Response {
    match person(&fake, &headers) {
        Some(FakeUser {
            emails: Some(list), ..
        }) => axum::Json(list).into_response(),
        Some(_) => (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"message": "Not Found"})),
        )
            .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"message": "Bad credentials"})),
        )
            .into_response(),
    }
}

pub(crate) fn random_hex(bytes: usize) -> String {
    let (_, verifier) = oauth2::PkceCodeChallenge::new_random_sha256_len(bytes.max(32) as u32);
    Sha256::digest(verifier.secret().as_bytes())[..bytes.min(32)]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
