//! A fake GitHub for tests and the local fixture: the authorize page, the
//! token endpoint (with real PKCE S256 checking and single-use codes), and
//! `/user` and `/user/emails`. Nothing here talks to github.com.
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

/// One fake GitHub person: the `/user` body and the `/user/emails` list
/// (`None` answers 404, as GitHub does without the email scope).
#[derive(Clone, Debug)]
pub struct FakeUser {
    pub user: Value,
    pub emails: Option<Value>,
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
    }
}

struct Grant {
    user: usize,
    challenge: String,
    redirect: String,
}

struct Inner {
    client_id: String,
    client_secret: String,
    redirect: String,
    users: Vec<FakeUser>,
    codes: BTreeMap<String, Grant>,
    tokens: BTreeMap<String, usize>,
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
    #[allow(dead_code)]
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
    inner.tokens.insert(access.clone(), grant.user);
    inner.exchanges += 1;
    axum::Json(
        json!({"access_token": access, "token_type": "bearer", "scope": "read:user,user:email"}),
    )
    .into_response()
}

fn person(fake: &Fake, headers: &HeaderMap) -> Option<FakeUser> {
    let token = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    let inner = fake.0.lock().expect("fake GitHub state");
    inner.tokens.get(token).map(|i| inner.users[*i].clone())
}

async fn user(State(fake): State<Fake>, headers: HeaderMap) -> Response {
    match person(&fake, &headers) {
        Some(found) => axum::Json(found.user).into_response(),
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
