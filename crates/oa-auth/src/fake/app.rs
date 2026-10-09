//! The fake GitHub as a GitHub App (#11056): the App's own OAuth client
//! (user tokens that expire, refresh tokens that rotate), its install page,
//! `/user/installations` and `/user/installations/{id}/repositories` paged
//! with `Link`, installation tokens minted at
//! `POST /app/installations/{id}/access_tokens` only for a JWT the App's
//! key signed (RS256, `iss`, `iat`, `exp` checked), with `expires_at`, and
//! `/repos/{owner}/{name}` read with an installation token. Installations
//! can be suspended or removed, and tokens revoked early, the ways
//! github.com does it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::signature;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Exchange, Fake, FakeUser, Inner, bad, checked_grant, inflate, link_header, random_hex,
};
use crate::app::{AppCredentials, github_time_text};
use crate::config::{Endpoints, GithubApp, GithubCredentials};

/// The scope the fake reports for a GitHub App user token.
pub(crate) const USER_SCOPE: &str = "app-user";

/// The GitHub App the fake plays.
#[derive(Clone, Debug)]
pub struct FakeApp {
    pub app_id: u64,
    pub slug: String,
    pub client_id: String,
    pub client_secret: String,
    /// The App's public key (DER `RSAPublicKey`), to check JWTs with.
    pub public_key: Vec<u8>,
    /// Where the install page sends the browser back to.
    pub setup_url: String,
}

impl FakeApp {
    /// The fake App for `credentials` (from [`app_credentials`]).
    #[must_use]
    pub fn of(credentials: &AppCredentials, client_secret: &str, setup_url: &str) -> Self {
        Self {
            app_id: credentials.app_id,
            slug: credentials.slug.clone(),
            client_id: credentials.oauth.app.client_id.clone(),
            client_secret: client_secret.into(),
            public_key: credentials.public_key(),
            setup_url: setup_url.into(),
        }
    }
}

/// One installation of the fake App.
#[derive(Clone, Debug)]
pub struct FakeInstallation {
    pub id: u64,
    /// The user or organization it is installed on.
    pub account: String,
    pub organization: bool,
    /// Installed on every repository (GitHub's `repository_selection`).
    pub all: bool,
    pub suspended: bool,
    /// Short repository values (`id`, `full_name`, `private`).
    pub repositories: Vec<Value>,
    /// Who can see the installation in `/user/installations`.
    pub members: Vec<String>,
}

pub(super) struct AppState {
    pub app: FakeApp,
    installations: Vec<FakeInstallation>,
    tokens: BTreeMap<String, Minted>,
    token_ttl: u64,
    mints: usize,
    user_tokens: BTreeMap<String, (usize, u64)>,
    refresh_tokens: BTreeMap<String, usize>,
    user_ttl: u64,
    refreshes: usize,
    next_id: u64,
}

struct Minted {
    installation: u64,
    repositories: Option<Vec<u64>>,
    expires: u64,
}

fn now() -> u64 {
    crate::app::now()
}

impl Fake {
    /// Make this fake also the GitHub App `app`.
    pub fn with_app(&self, app: FakeApp) {
        self.0.lock().expect("fake GitHub state").app = Some(AppState {
            app,
            installations: Vec::new(),
            tokens: BTreeMap::new(),
            token_ttl: 3600,
            mints: 0,
            user_tokens: BTreeMap::new(),
            refresh_tokens: BTreeMap::new(),
            user_ttl: 8 * 3600,
            refreshes: 0,
            next_id: 50_001,
        });
    }

    fn with_state<T>(&self, change: impl FnOnce(&mut AppState) -> T) -> T {
        let mut inner = self.0.lock().expect("fake GitHub state");
        change(inner.app.as_mut().expect("Fake::with_app first"))
    }

    /// Install the App (or replace the installation with the same id).
    pub fn install(&self, installation: FakeInstallation) {
        self.with_state(|state| {
            state.installations.retain(|i| i.id != installation.id);
            state.installations.push(installation);
        });
    }

    /// The App's installations.
    #[must_use]
    pub fn installations(&self) -> Vec<FakeInstallation> {
        self.with_state(|state| state.installations.clone())
    }

    /// Suspend or resume an installation.
    pub fn set_suspended(&self, id: u64, suspended: bool) {
        self.with_state(|state| {
            for installation in &mut state.installations {
                if installation.id == id {
                    installation.suspended = suspended;
                }
            }
        });
    }

    /// Remove an installation: its tokens stop working.
    pub fn uninstall(&self, id: u64) {
        self.with_state(|state| {
            state.installations.retain(|i| i.id != id);
            state.tokens.retain(|_, minted| minted.installation != id);
        });
    }

    /// How long new installation tokens last (GitHub: one hour).
    pub fn set_token_ttl(&self, seconds: u64) {
        self.with_state(|state| state.token_ttl = seconds);
    }

    /// How long new App user tokens last (GitHub: eight hours).
    pub fn set_user_token_ttl(&self, seconds: u64) {
        self.with_state(|state| state.user_ttl = seconds);
    }

    /// Revoke every installation token issued so far: GitHub answers 401
    /// to them.
    pub fn revoke_installation_tokens(&self) {
        self.with_state(|state| state.tokens.clear());
    }

    /// Revoke every App user token (refresh tokens keep working).
    pub fn revoke_user_tokens(&self) {
        self.with_state(|state| state.user_tokens.clear());
    }

    /// Revoke the App's whole authorization: user and refresh tokens.
    pub fn revoke_app_authorization(&self) {
        self.with_state(|state| {
            state.user_tokens.clear();
            state.refresh_tokens.clear();
        });
    }

    /// How many installation tokens were minted.
    #[must_use]
    pub fn installation_mints(&self) -> usize {
        self.with_state(|state| state.mints)
    }

    /// How many refresh-token trades succeeded.
    #[must_use]
    pub fn refreshes(&self) -> usize {
        self.with_state(|state| state.refreshes)
    }

    /// Whether `token` is a live installation token that reaches
    /// `full_name` (what a Git fetch with it would do).
    #[must_use]
    pub fn reaches(&self, token: &str, full_name: &str) -> bool {
        self.with_state(|state| state.reaches(token, full_name).is_some())
    }
}

impl AppState {
    /// The repository `token` may read, if any.
    fn reaches(&self, token: &str, full_name: &str) -> Option<Value> {
        let minted = self.tokens.get(token).filter(|m| m.expires > now())?;
        let installation = self
            .installations
            .iter()
            .find(|i| i.id == minted.installation && !i.suspended)?;
        let found = installation
            .repositories
            .iter()
            .find(|r| r["full_name"].as_str() == Some(full_name))?;
        let id = found["id"].as_u64()?;
        if minted
            .repositories
            .as_ref()
            .is_some_and(|ids| !ids.contains(&id))
        {
            return None;
        }
        Some(found.clone())
    }
}

pub(super) fn routes(router: Router<Fake>) -> Router<Fake> {
    router
        .route("/apps/{slug}/installations/new", get(install_page))
        .route("/user/installations", get(user_installations))
        .route(
            "/user/installations/{id}/repositories",
            get(installation_repositories),
        )
        .route("/app/installations/{id}/access_tokens", post(access_tokens))
}

/// The token endpoint for the App's client: a code (PKCE-checked) or a
/// refresh token for a user token that expires and a new refresh token.
/// `None` when the request isn't for the App's client.
pub(super) fn token(inner: &mut Inner, form: &Exchange) -> Option<Response> {
    let app_client = inner.app.as_ref()?.app.client_id.clone();
    if form.client_id.as_deref() != Some(app_client.as_str()) {
        return None;
    }
    let secret_ok = inner
        .app
        .as_ref()
        .is_some_and(|s| form.client_secret.as_deref() == Some(s.app.client_secret.as_str()));
    if !secret_ok {
        return Some(bad("incorrect_client_credentials"));
    }
    let user = if form.grant_type.as_deref() == Some("refresh_token") {
        let refresh = form.refresh_token.clone().unwrap_or_default();
        let state = inner.app.as_mut()?;
        let Some(user) = state.refresh_tokens.remove(&refresh) else {
            return Some(bad("bad_refresh_token"));
        };
        state.refreshes += 1;
        user
    } else {
        let Some(grant) = inner.codes.remove(&form.code).filter(|g| g.app) else {
            return Some(bad("bad_verification_code"));
        };
        if let Some(refused) = checked_grant(&grant, form) {
            return Some(refused);
        }
        inner.exchanges += 1;
        grant.user
    };
    let state = inner.app.as_mut()?;
    let access = format!("ghu_{}", random_hex(18));
    let refresh = format!("ghr_{}", random_hex(30));
    state
        .user_tokens
        .insert(access.clone(), (user, now() + state.user_ttl));
    state.refresh_tokens.insert(refresh.clone(), user);
    Some(
        axum::Json(json!({
            "access_token": access,
            "expires_in": state.user_ttl,
            "refresh_token": refresh,
            "refresh_token_expires_in": 15_897_600,
            "token_type": "bearer",
            "scope": "",
        }))
        .into_response(),
    )
}

/// The person a live App user token belongs to.
pub(super) fn user_of(inner: &Inner, token: &str) -> Option<usize> {
    let (user, expires) = inner.app.as_ref()?.user_tokens.get(token)?;
    (*expires > now()).then_some(*user)
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn message(status: u16, text: &str) -> Response {
    (
        StatusCode::from_u16(status).expect("status"),
        axum::Json(json!({"message": text, "documentation_url": "https://docs.github.com/rest", "status": status.to_string()})),
    )
        .into_response()
}

/// Whether `user` may open `repository` on GitHub: their own account's, or
/// one their list holds.
fn can_open(user: &FakeUser, repository: &Value) -> bool {
    let name = repository["full_name"].as_str().unwrap_or_default();
    name.split_once('/')
        .is_some_and(|(owner, _)| owner == user.login())
        || user
            .repos
            .iter()
            .any(|r| r["full_name"].as_str() == Some(name))
}

/// The person behind an App user token, and the installations they see.
fn caller(fake: &Fake, headers: &HeaderMap) -> Option<(FakeUser, Vec<FakeInstallation>)> {
    let inner = fake.0.lock().expect("fake GitHub state");
    let user = user_of(&inner, bearer(headers)?)?;
    let person = inner.users[user].clone();
    let seen = inner
        .app
        .as_ref()?
        .installations
        .iter()
        .filter(|i| i.members.iter().any(|m| m == person.login()))
        .cloned()
        .collect();
    Some((person, seen))
}

/// Repositories the App user token reaches: in the person's installations
/// (not suspended) and ones they can open.
pub(super) fn reachable(fake: &Fake, user: &FakeUser) -> Vec<Value> {
    let inner = fake.0.lock().expect("fake GitHub state");
    let Some(state) = inner.app.as_ref() else {
        return Vec::new();
    };
    state
        .installations
        .iter()
        .filter(|i| !i.suspended && i.members.iter().any(|m| m == user.login()))
        .flat_map(|i| i.repositories.iter())
        .filter(|r| can_open(user, r))
        .cloned()
        .collect()
}

#[derive(Deserialize)]
struct Paging {
    page: Option<usize>,
    per_page: Option<usize>,
}

/// One page of `all` as GitHub wraps it (`{total_count, <key>: [...]}`)
/// with its `Link` header.
fn paged(
    headers: &HeaderMap,
    uri: &axum::http::Uri,
    paging: &Paging,
    all: &[Value],
    key: &str,
    extra: Value,
) -> Response {
    let per = paging.per_page.unwrap_or(30).clamp(1, 100);
    let number = paging.page.unwrap_or(1).max(1);
    let last = all.len().div_ceil(per).max(1);
    let rows: Vec<Value> = all
        .iter()
        .skip((number - 1).saturating_mul(per))
        .take(per)
        .cloned()
        .collect();
    let mut body = json!({"total_count": all.len()});
    body[key] = Value::Array(rows);
    if let (Some(body), Value::Object(extra)) = (body.as_object_mut(), extra) {
        body.extend(extra);
    }
    let mut response = axum::Json(body).into_response();
    if let Some(value) = link_header(headers, uri, number, last) {
        response.headers_mut().insert(header::LINK, value);
    }
    response
}

fn installation_json(app: &FakeApp, installation: &FakeInstallation) -> Value {
    let kind = if installation.organization {
        "Organization"
    } else {
        "User"
    };
    json!({
        "id": installation.id,
        "account": {
            "login": installation.account,
            "id": 20_000 + installation.id,
            "type": kind,
            "html_url": format!("https://github.com/{}", installation.account),
        },
        "repository_selection": if installation.all { "all" } else { "selected" },
        "access_tokens_url": format!("https://api.github.com/app/installations/{}/access_tokens", installation.id),
        "repositories_url": "https://api.github.com/installation/repositories",
        "html_url": format!("https://github.com/settings/installations/{}", installation.id),
        "app_id": app.app_id,
        "app_slug": app.slug,
        "target_id": 20_000 + installation.id,
        "target_type": kind,
        "permissions": {"contents": "write", "metadata": "read", "pull_requests": "write"},
        "events": [],
        "created_at": "2026-10-01T12:00:00Z",
        "updated_at": "2026-10-01T12:00:00Z",
        "single_file_name": null,
        "suspended_by": if installation.suspended { json!({"login": installation.account}) } else { Value::Null },
        "suspended_at": if installation.suspended { json!("2026-10-02T12:00:00Z") } else { Value::Null },
    })
}

async fn user_installations(
    State(fake): State<Fake>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(paging): Query<Paging>,
) -> Response {
    let Some((_, seen)) = caller(&fake, &headers) else {
        return message(401, "Bad credentials");
    };
    let app = fake.with_state(|state| state.app.clone());
    let all: Vec<Value> = seen.iter().map(|i| installation_json(&app, i)).collect();
    paged(&headers, &uri, &paging, &all, "installations", json!({}))
}

async fn installation_repositories(
    State(fake): State<Fake>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Path(id): Path<u64>,
    Query(paging): Query<Paging>,
) -> Response {
    let Some((person, seen)) = caller(&fake, &headers) else {
        return message(401, "Bad credentials");
    };
    let Some(installation) = seen.iter().find(|i| i.id == id) else {
        return message(404, "Not Found");
    };
    if installation.suspended {
        return message(403, "This installation has been suspended");
    }
    let all: Vec<Value> = installation
        .repositories
        .iter()
        .filter(|r| can_open(&person, r))
        .map(inflate)
        .collect();
    let selection = if installation.all { "all" } else { "selected" };
    paged(
        &headers,
        &uri,
        &paging,
        &all,
        "repositories",
        json!({"repository_selection": selection}),
    )
}

/// Whether `jwt` is one the App's key signed, current, and short-lived.
fn jwt_ok(app: &FakeApp, jwt: &str) -> bool {
    let mut parts = jwt.split('.');
    let (Some(head), Some(claims), Some(signed), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let decode = |part: &str| {
        URL_SAFE_NO_PAD
            .decode(part)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    };
    let (Some(head_json), Some(claims_json), Ok(signature_bytes)) =
        (decode(head), decode(claims), URL_SAFE_NO_PAD.decode(signed))
    else {
        return false;
    };
    if head_json["alg"] != "RS256" {
        return false;
    }
    let verified =
        signature::UnparsedPublicKey::new(&signature::RSA_PKCS1_2048_8192_SHA256, &app.public_key)
            .verify(format!("{head}.{claims}").as_bytes(), &signature_bytes)
            .is_ok();
    let iss = match &claims_json["iss"] {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return false,
    };
    let (Some(iat), Some(exp)) = (claims_json["iat"].as_u64(), claims_json["exp"].as_u64()) else {
        return false;
    };
    let now = now();
    verified
        && iss == app.app_id.to_string()
        && iat <= now + 5
        && exp > now
        && exp <= now + 600 + 5
        && exp - iat <= 660
}

async fn access_tokens(
    State(fake): State<Fake>,
    headers: HeaderMap,
    Path(id): Path<u64>,
    body: Option<axum::Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or(Value::Null);
    let mut inner = fake.0.lock().expect("fake GitHub state");
    let Some(state) = inner.app.as_mut() else {
        return message(404, "Not Found");
    };
    if !bearer(&headers).is_some_and(|jwt| jwt_ok(&state.app, jwt)) {
        return message(401, "A JSON web token could not be decoded or has expired");
    }
    let Some(installation) = state.installations.iter().find(|i| i.id == id) else {
        return message(404, "Not Found");
    };
    if installation.suspended {
        return message(403, "This installation has been suspended");
    }
    let ids: Option<Vec<u64>> = body["repository_ids"]
        .as_array()
        .map(|ids| ids.iter().filter_map(Value::as_u64).collect());
    if let Some(ids) = &ids
        && ids.iter().any(|wanted| {
            !installation
                .repositories
                .iter()
                .any(|r| r["id"].as_u64() == Some(*wanted))
        })
    {
        return message(
            422,
            "There is at least one repository that does not exist or is not accessible to the parent installation.",
        );
    }
    let token = format!("ghs_{}", random_hex(18));
    let expires = now() + state.token_ttl;
    state.mints += 1;
    state.tokens.insert(
        token.clone(),
        Minted {
            installation: id,
            repositories: ids,
            expires,
        },
    );
    (
        StatusCode::CREATED,
        axum::Json(json!({
            "token": token,
            "expires_at": github_time_text(expires),
            "permissions": body["permissions"].clone(),
            "repository_selection": "selected",
        })),
    )
        .into_response()
}

/// `/repos/{owner}/{name}` read with an installation token: `None` when
/// the bearer isn't one.
pub(super) fn repo_as_installation(
    fake: &Fake,
    headers: &HeaderMap,
    full: &str,
) -> Option<Response> {
    let token = bearer(headers)?;
    if !token.starts_with("ghs_") {
        return None;
    }
    let inner = fake.0.lock().expect("fake GitHub state");
    let state = inner.app.as_ref()?;
    if !state.tokens.get(token).is_some_and(|m| m.expires > now()) {
        return Some(message(401, "Bad credentials"));
    }
    Some(match state.reaches(token, full) {
        Some(found) => axum::Json(inflate(&found)).into_response(),
        None => message(404, "Not Found"),
    })
}

#[derive(Deserialize)]
struct Install {
    login: Option<String>,
    /// Comma-separated `owner/name` to install on.
    repos: Option<String>,
    /// Install on this organization instead of the person's account.
    org: Option<String>,
    state: Option<String>,
}

/// GitHub's install page: pick an account and repositories. Tests pass
/// `login` and `repos` to skip the page.
async fn install_page(
    State(fake): State<Fake>,
    Path(slug): Path<String>,
    uri: axum::http::Uri,
    Query(query): Query<Install>,
) -> Response {
    let mut inner = fake.0.lock().expect("fake GitHub state");
    let users = inner.users.clone();
    let Some(state) = inner.app.as_mut().filter(|s| s.app.slug == slug) else {
        return message(404, "Not Found");
    };
    let Some(login) = query.login.filter(|l| users.iter().any(|u| u.login() == l)) else {
        let base = uri.to_string();
        let join = if base.contains('?') { '&' } else { '?' };
        let people: String = users
            .iter()
            .map(|u| {
                let repos: Vec<&str> = u
                    .repos
                    .iter()
                    .filter_map(|r| r["full_name"].as_str())
                    .filter(|n| n.starts_with(&format!("{}/", u.login())))
                    .collect();
                format!(
                    "<li><a href=\"{}\">Install for {} on {} repositories</a></li>",
                    super::escape(&format!(
                        "{base}{join}login={}&repos={}",
                        u.login(),
                        repos.join(",")
                    )),
                    super::escape(u.login()),
                    repos.len()
                )
            })
            .collect();
        return Html(format!(
            "<!doctype html><meta charset=utf-8><title>Fake GitHub</title>\
             <h1>Install {}</h1><p>This is the local test GitHub. Pick where to install.</p><ul>{people}</ul>",
            super::escape(&slug)
        ))
        .into_response();
    };
    let account = query.org.unwrap_or_else(|| login.clone());
    let short = |name: &str| {
        users
            .iter()
            .flat_map(|u| u.repos.iter())
            .find(|r| r["full_name"].as_str() == Some(name))
            .cloned()
            .unwrap_or_else(|| {
                let id = 90_000 + name.bytes().map(u64::from).sum::<u64>();
                json!({"id": id, "full_name": name, "private": false})
            })
    };
    let repositories: Vec<Value> = query
        .repos
        .unwrap_or_default()
        .split(',')
        .filter(|n| crate::repos::full_name(n))
        .map(short)
        .collect();
    let id = match state
        .installations
        .iter_mut()
        .find(|i| i.account == account)
    {
        Some(existing) => {
            existing.repositories = repositories;
            if !existing.members.contains(&login) {
                existing.members.push(login.clone());
            }
            existing.id
        }
        None => {
            let id = state.next_id;
            state.next_id += 1;
            state.installations.push(FakeInstallation {
                id,
                organization: account != login,
                account,
                all: false,
                suspended: false,
                repositories,
                members: vec![login],
            });
            id
        }
    };
    let mut back = url::Url::parse(&state.app.setup_url).expect("setup URL");
    back.query_pairs_mut()
        .append_pair("installation_id", &id.to_string())
        .append_pair("setup_action", "install");
    if let Some(value) = query.state {
        back.query_pairs_mut().append_pair("state", &value);
    }
    Redirect::to(back.as_str()).into_response()
}

/// A 2048-bit RSA key in PEM for the fake App, made once per process with
/// the `openssl` command (PKCS #1, as GitHub hands App keys out).
#[must_use]
pub fn app_key() -> Vec<u8> {
    static KEY: OnceLock<Vec<u8>> = OnceLock::new();
    KEY.get_or_init(|| {
        let run = |args: &[&str]| {
            std::process::Command::new("openssl")
                .args(args)
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|out| out.status.success() && !out.stdout.is_empty())
                .map(|out| out.stdout)
        };
        run(&["genrsa", "-traditional", "2048"])
            .or_else(|| run(&["genrsa", "2048"]))
            .expect("the fake GitHub App needs the openssl command to make its key")
    })
    .clone()
}

/// App credentials that match a [`Fake`] at `origin` with
/// [`Fake::with_app`]`(`[`FakeApp::of`]`(..))`.
pub fn app_credentials(
    origin: &str,
    app_id: u64,
    slug: &str,
    client_id: &str,
    client_secret: &str,
    redirect: &str,
) -> Result<AppCredentials, String> {
    let app = GithubApp::new(client_id, redirect, Endpoints::at(origin))?;
    let oauth = GithubCredentials::new(app, client_secret, [11; 32])?;
    AppCredentials::new(app_id, slug, oauth, &app_key())
}
