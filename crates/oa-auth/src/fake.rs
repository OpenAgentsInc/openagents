//! A fake GitHub for tests and the local fixture: the authorize page, the
//! token endpoint (with real PKCE S256 checking and single-use codes),
//! `/user` and `/user/emails`, and for connecting repositories
//! `/user/repos` and `/repos/{owner}/{name}` (private ones only for a
//! token granted `repo`). Nothing here talks to github.com.
//!
//! `GET /login/oauth/authorize` shows a small page listing the fake people
//! (click one to approve as them) and a Cancel link. Tests skip the page
//! with `&login=<login>` (approve) or `&deny=1` (cancel).

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
    fn login(&self) -> &str {
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

struct Grant {
    user: usize,
    challenge: String,
    redirect: String,
    scopes: Vec<String>,
}

struct Inner {
    client_id: String,
    client_secret: String,
    redirect: String,
    users: Vec<FakeUser>,
    codes: BTreeMap<String, Grant>,
    tokens: BTreeMap<String, (usize, Vec<String>)>,
    exchanges: usize,
}

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

    /// The router (mount it at the origin's root).
    pub fn router(&self) -> Router {
        Router::new()
            .route("/login/oauth/authorize", get(authorize))
            .route("/login/oauth/access_token", post(token))
            .route("/user", get(user))
            .route("/user/emails", get(emails))
            .route("/user/repos", get(repos))
            .route("/repos/{owner}/{name}", get(repo))
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

fn escape(value: &str) -> String {
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
    if query.client_id != inner.client_id || redirect != inner.redirect {
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
struct Exchange {
    client_id: Option<String>,
    client_secret: Option<String>,
    code: String,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    #[allow(dead_code)]
    grant_type: Option<String>,
}

fn bad(code: &str) -> Response {
    // GitHub answers token errors with 200 and an error body.
    axum::Json(json!({"error": code, "error_description": "Fake GitHub refused the exchange."}))
        .into_response()
}

async fn token(State(fake): State<Fake>, Form(form): Form<Exchange>) -> Response {
    let mut inner = fake.0.lock().expect("fake GitHub state");
    if form.client_id.as_deref() != Some(&inner.client_id)
        || form.client_secret.as_deref() != Some(&inner.client_secret)
    {
        return bad("incorrect_client_credentials");
    }
    // Single use: the code is gone whether or not the rest checks out.
    let Some(grant) = inner.codes.remove(&form.code) else {
        return bad("bad_verification_code");
    };
    if form
        .redirect_uri
        .as_deref()
        .is_some_and(|r| r != grant.redirect)
    {
        return bad("redirect_uri_mismatch");
    }
    let verifier = form.code_verifier.unwrap_or_default();
    if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return bad("bad_verification_code");
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
    page: Option<u32>,
}

async fn repos(State(fake): State<Fake>, headers: HeaderMap, Query(page): Query<Page>) -> Response {
    match holder(&fake, &headers) {
        Some(_) if page.page.unwrap_or(1) > 1 => axum::Json(json!([])).into_response(),
        Some((user, scopes)) => axum::Json(visible(&user, &scopes)).into_response(),
        None => bad_credentials(),
    }
}

async fn repo(
    State(fake): State<Fake>,
    headers: HeaderMap,
    axum::extract::Path((owner, name)): axum::extract::Path<(String, String)>,
) -> Response {
    let Some((user, scopes)) = holder(&fake, &headers) else {
        return bad_credentials();
    };
    let full = format!("{owner}/{name}");
    match visible(&user, &scopes)
        .into_iter()
        .find(|r| r["full_name"] == full.as_str())
    {
        Some(found) => axum::Json(found).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"message": "Not Found"})),
        )
            .into_response(),
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

fn random_hex(bytes: usize) -> String {
    let (_, verifier) = oauth2::PkceCodeChallenge::new_random_sha256_len(bytes.max(32) as u32);
    Sha256::digest(verifier.secret().as_bytes())[..bytes.min(32)]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
