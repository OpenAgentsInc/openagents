//! An in-process fake of the WorkOS API calls we make, for tests and the
//! local fixture: users with passwords, email verification, Magic Auth
//! codes, password resets, provider sign-ins (GitHub, Google, SSO),
//! sessions with single-use refresh tokens and revocation, access tokens
//! signed with a test-only RSA key and published at `/sso/jwks/{id}`,
//! organizations, and memberships. The real SDK client talks to it over
//! HTTP, so the request and response shapes are the SDK's own.
//!
//! Every email WorkOS would send lands in [`FakeWorkos::inbox`] instead.
//! Nothing here ever reaches WorkOS.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};

use super::{Method, Mode, Settings, Workos};

/// The fake's client ID.
pub const CLIENT_ID: &str = "client_01FAKEWORKOS";
/// The API key the fake accepts. Not a real key.
pub const API_KEY: &str = "sk_test_fake_workos_key";
const KID: &str = "fake-workos-key-1";
/// The test-only RSA key the fake signs access tokens with (PKCS#1 DER).
const KEY_DER: &[u8] = include_bytes!("../../fixtures/workos-fake-key.der");
/// Its public modulus, base64url.
const MODULUS: &str = "zm6GRQBSYJHoZ2daNeNQ5H2Ae1DhD4TA5GLybAAOgbBnWVRU-kntVn2ZWnac4WVgrvMy9lR8AhX_vLrJZy_5Qj_Sx899YuG2rjDCowar8lchp53Ywfcl_tBbVUSF_Jq5DHAVYIz6TPljMAzcFrernRqKrP7KZVDDAK7XuXhP4JhR0BtDYJD2zYNPngmjKtXQPX0I6r-ERAqvEITkMLMJnDbRHgzeuWAGhQLjFWyxS4ZgSoJkhtC_5kbIuJEfObckLM2zmuWKDUM78kCqUEZCWT4XBq18VqL0QV4yvLrBEJpa77XeZYhLuuXrdCVR7LaI_RDzbqQSICjGLZnqU3wzaw";
const STAMP: &str = "2026-10-20T00:00:00.000Z";

/// One email the fake "sent".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    /// `magic_auth`, `email_verification`, or `password_reset`.
    pub kind: &'static str,
    /// The code, or for a reset the token.
    pub secret: String,
}

#[derive(Clone, Debug, Default)]
struct FakeUser {
    id: String,
    email: String,
    email_verified: bool,
    first_name: Option<String>,
    last_name: Option<String>,
    external_id: Option<String>,
    metadata: BTreeMap<String, String>,
    password: Option<String>,
    identities: Vec<(String, String)>,
}

impl FakeUser {
    fn json(&self) -> Value {
        json!({
            "object": "user",
            "id": self.id,
            "email": self.email,
            "email_verified": self.email_verified,
            "first_name": self.first_name,
            "last_name": self.last_name,
            "external_id": self.external_id,
            "metadata": self.metadata,
            "created_at": STAMP,
            "updated_at": STAMP,
        })
    }
}

#[derive(Clone, Debug)]
struct Profile {
    email: String,
    idp_id: String,
    first_name: Option<String>,
}

#[derive(Default)]
struct Inner {
    next: u64,
    users: BTreeMap<String, FakeUser>,
    magic: HashMap<String, String>,
    verifications: HashMap<String, (String, String)>,
    pending: HashMap<String, String>,
    oauth_codes: HashMap<String, (String, String)>,
    resets: HashMap<String, String>,
    /// Session ID to (user, revoked).
    sessions: HashMap<String, (String, bool)>,
    refresh: HashMap<String, String>,
    oauth_queue: HashMap<String, Vec<Profile>>,
    orgs: BTreeMap<String, Value>,
    memberships: BTreeMap<String, (String, String, String)>,
    inbox: Vec<Email>,
    /// Calls by path, for tests that count writes.
    calls: Vec<String>,
    /// Make every call answer 503 (a WorkOS outage).
    down: bool,
    /// The environment's role slugs.
    roles: Vec<String>,
}

impl Inner {
    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}_01FAKE{:020}", self.next)
    }

    fn secret(&mut self) -> String {
        self.next += 1;
        let mut bytes = [0u8; 16];
        getrandom_fill(&mut bytes);
        format!(
            "{:06}{}",
            (self.next * 7919) % 1_000_000,
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        )
    }

    fn code(&mut self) -> String {
        let secret = self.secret();
        secret[..6].to_string()
    }

    fn by_email(&self, email: &str) -> Option<&FakeUser> {
        let email = email.trim().to_ascii_lowercase();
        self.users.values().find(|user| user.email == email)
    }

    fn session(&mut self, user: &str, method: &str, org: Option<&str>) -> Value {
        let sid = self.id("session");
        self.sessions.insert(sid.clone(), (user.to_string(), false));
        let refresh = self.secret();
        self.refresh.insert(refresh.clone(), sid.clone());
        let access = mint(user, &sid, org, 300);
        json!({
            "user": self.users[user].json(),
            "organization_id": org,
            "access_token": access,
            "refresh_token": refresh,
            "authentication_method": method,
        })
    }
}

fn getrandom_fill(bytes: &mut [u8]) {
    use std::hash::{BuildHasher, Hasher};
    for chunk in bytes.chunks_mut(8) {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64),
        );
        let value = hasher.finish().to_le_bytes();
        chunk.copy_from_slice(&value[..chunk.len()]);
    }
}

/// Sign an access token for `user` in session `sid`, valid for `seconds`
/// (negative for one already expired).
#[must_use]
pub fn mint(user: &str, sid: &str, org: Option<&str>, seconds: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(KID.to_string());
    let mut claims = json!({
        "iss": format!("https://fake.workos/user_management/{CLIENT_ID}"),
        "sub": user,
        "sid": sid,
        "iat": now,
        "exp": now + seconds,
    });
    // WorkOS leaves `org_id` out when the session has no organization.
    if let Some(org) = org {
        claims["org_id"] = Value::String(org.to_string());
    }
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_rsa_der(KEY_DER),
    )
    .expect("the fake's key signs")
}

/// The fake's public key set, as WorkOS publishes one.
#[must_use]
pub fn jwks() -> Value {
    json!({"keys": [{"kty": "RSA", "kid": KID, "alg": "RS256", "use": "sig", "n": MODULUS, "e": "AQAB"}]})
}

type Shared = Arc<Mutex<Inner>>;

/// A running fake WorkOS.
pub struct FakeWorkos {
    pub url: String,
    inner: Shared,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeWorkos {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeWorkos {
    /// Start the fake on a free loopback port.
    pub async fn start() -> Self {
        let inner: Shared = Arc::new(Mutex::new(Inner {
            roles: ROLES.iter().map(|role| role.to_string()).collect(),
            ..Inner::default()
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the fake WorkOS");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let app = router(inner.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        Self { url, inner, task }
    }

    /// Settings pointing at this fake, with `methods` on.
    #[must_use]
    pub fn settings(&self, mode: Mode, methods: &[Method]) -> Settings {
        Settings {
            mode,
            client_id: CLIENT_ID.to_string(),
            api_key_env: None,
            api_key_file: None,
            base_url: Some(self.url.clone()),
            methods: methods.to_vec(),
            roles: super::Roles::default(),
        }
    }

    /// A client for this fake.
    #[must_use]
    pub fn client(&self, mode: Mode, methods: &[Method]) -> Workos {
        Workos::with_key(self.settings(mode, methods), API_KEY)
    }

    fn with<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> T {
        f(&mut self.inner.lock().expect("fake state"))
    }

    /// Add a User with a password, answering its ID.
    pub fn add_user(&self, email: &str, password: Option<&str>, verified: bool) -> String {
        self.with(|inner| {
            let id = inner.id("user");
            inner.users.insert(
                id.clone(),
                FakeUser {
                    id: id.clone(),
                    email: email.to_ascii_lowercase(),
                    email_verified: verified,
                    password: password.map(str::to_string),
                    ..FakeUser::default()
                },
            );
            id
        })
    }

    /// The next sign-in through `provider` (`GitHubOAuth`, `GoogleOAuth`,
    /// or an organization ID for SSO) comes back as this person.
    pub fn queue_sign_in(
        &self,
        provider: &str,
        email: &str,
        idp_id: &str,
        first_name: Option<&str>,
    ) {
        self.with(|inner| {
            inner
                .oauth_queue
                .entry(provider.to_string())
                .or_default()
                .push(Profile {
                    email: email.to_ascii_lowercase(),
                    idp_id: idp_id.to_string(),
                    first_name: first_name.map(str::to_string),
                });
        });
    }

    /// Every email "sent", oldest first.
    #[must_use]
    pub fn inbox(&self) -> Vec<Email> {
        self.with(|inner| inner.inbox.clone())
    }

    /// The newest email to `to` of `kind`.
    #[must_use]
    pub fn last_email(&self, to: &str, kind: &str) -> Option<String> {
        self.with(|inner| {
            inner
                .inbox
                .iter()
                .rev()
                .find(|email| email.to == to.to_ascii_lowercase() && email.kind == kind)
                .map(|email| email.secret.clone())
        })
    }

    /// The User with this email, as JSON.
    #[must_use]
    pub fn user_by_email(&self, email: &str) -> Option<Value> {
        self.with(|inner| inner.by_email(email).map(FakeUser::json))
    }

    /// The User with this ID, as JSON.
    #[must_use]
    pub fn user(&self, id: &str) -> Option<Value> {
        self.with(|inner| inner.users.get(id).map(FakeUser::json))
    }

    /// How many Users the fake holds.
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.with(|inner| inner.users.len())
    }

    /// Whether session `sid` was revoked.
    #[must_use]
    pub fn revoked(&self, sid: &str) -> bool {
        self.with(|inner| inner.sessions.get(sid).is_some_and(|(_, revoked)| *revoked))
    }

    /// Start a session for `user` directly, answering (access, refresh).
    pub fn sign_in(&self, user: &str) -> (String, String) {
        self.with(|inner| {
            let body = inner.session(user, "Password", None);
            (
                body["access_token"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                body["refresh_token"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
    }

    /// Every Organization, as JSON.
    #[must_use]
    pub fn organizations(&self) -> Vec<Value> {
        self.with(|inner| inner.orgs.values().cloned().collect())
    }

    /// Every membership as (user, organization, role).
    #[must_use]
    pub fn memberships(&self) -> Vec<(String, String, String)> {
        self.with(|inner| inner.memberships.values().cloned().collect())
    }

    /// Every call so far, as `METHOD path`.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        self.with(|inner| inner.calls.clone())
    }

    /// The environment's role slugs, as a production environment without
    /// an `owner` role has them.
    pub fn set_roles(&self, roles: &[&str]) {
        self.with(|inner| inner.roles = roles.iter().map(|role| role.to_string()).collect());
    }

    /// Make every call fail with 503 until set back.
    pub fn set_down(&self, down: bool) {
        self.with(|inner| inner.down = down);
    }
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"code": code, "message": message}))).into_response()
}

fn router(inner: Shared) -> Router {
    Router::new()
        .route("/sso/jwks/{client}", get(|| async { Json(jwks()) }))
        .route("/authorization/roles", get(roles))
        .route("/user_management/authorize", get(authorize))
        .route("/user_management/authenticate", post(authenticate))
        .route("/user_management/users", get(list_users).post(create_user))
        .route(
            "/user_management/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route(
            "/user_management/users/external_id/{external}",
            get(user_by_external),
        )
        .route("/user_management/users/{id}/identities", get(identities))
        .route("/user_management/magic_auth", post(magic_auth))
        .route(
            "/user_management/email_verification/{id}",
            get(email_verification),
        )
        .route("/user_management/password_reset", post(password_reset))
        .route(
            "/user_management/password_reset/confirm",
            post(password_reset_confirm),
        )
        .route("/user_management/sessions/revoke", post(revoke))
        .route("/organizations", get(list_orgs).post(create_org))
        .route(
            "/organizations/external_id/{external}",
            get(org_by_external),
        )
        .route(
            "/organizations/{id}",
            get(get_org).put(update_org).delete(delete_org),
        )
        .route(
            "/user_management/organization_memberships",
            get(list_memberships).post(create_membership),
        )
        .route(
            "/user_management/organization_memberships/{id}",
            axum::routing::put(update_membership).delete(delete_membership),
        )
        .layer(axum::middleware::from_fn_with_state(inner.clone(), gate))
        .with_state(inner)
}

async fn gate(
    State(inner): State<Shared>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    {
        let mut inner = inner.lock().expect("fake state");
        inner
            .calls
            .push(format!("{} {}", request.method(), request.uri().path()));
        if inner.down {
            return error(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "down");
        }
    }
    let path = request.uri().path().to_string();
    let public = path.starts_with("/sso/jwks/") || path == "/user_management/authorize";
    let keyed = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        == Some(&format!("Bearer {API_KEY}"));
    if !public && !keyed && path != "/user_management/authenticate" {
        return error(StatusCode::UNAUTHORIZED, "unauthorized", "no key");
    }
    next.run(request).await
}

async fn authorize(
    State(inner): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let Some(redirect) = query.get("redirect_uri") else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "no redirect_uri",
        );
    };
    let key = query
        .get("provider")
        .or_else(|| query.get("organization_id"))
        .cloned()
        .unwrap_or_default();
    let Some(profile) = inner
        .oauth_queue
        .get_mut(&key)
        .and_then(|queue| (!queue.is_empty()).then(|| queue.remove(0)))
    else {
        return error(StatusCode::NOT_FOUND, "not_found", "nothing queued");
    };
    // WorkOS links a provider sign-in to the User with that verified email.
    let user = match inner.by_email(&profile.email).map(|user| user.id.clone()) {
        Some(id) => id,
        None => {
            let id = inner.id("user");
            inner.users.insert(
                id.clone(),
                FakeUser {
                    id: id.clone(),
                    email: profile.email.clone(),
                    email_verified: true,
                    first_name: profile.first_name.clone(),
                    ..FakeUser::default()
                },
            );
            id
        }
    };
    let provider = match key.as_str() {
        "GitHubOAuth" => "GithubOAuth".to_string(),
        other => other.to_string(),
    };
    if let Some(record) = inner.users.get_mut(&user) {
        record.email_verified = true;
        if !record.identities.iter().any(|(p, _)| *p == provider) {
            record.identities.push((provider, profile.idp_id.clone()));
        }
    }
    let code = inner.secret();
    let method = match key.as_str() {
        "GitHubOAuth" => "GitHubOAuth",
        "GoogleOAuth" => "GoogleOAuth",
        _ => "SSO",
    };
    inner
        .oauth_codes
        .insert(code.clone(), (user, method.to_string()));
    let state = query.get("state").cloned().unwrap_or_default();
    let mut url = url::Url::parse(redirect).expect("a redirect URL");
    url.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &state);
    Redirect::to(url.as_str()).into_response()
}

async fn authenticate(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    if body["client_id"] != CLIENT_ID
        || (body["client_secret"] != API_KEY && body["code_verifier"].is_null())
    {
        return error(StatusCode::UNAUTHORIZED, "invalid_client", "bad client");
    }
    let text = |name: &str| body[name].as_str().unwrap_or_default().to_string();
    match body["grant_type"].as_str().unwrap_or_default() {
        "authorization_code" => match inner.oauth_codes.remove(&text("code")) {
            Some((user, method)) => Json(inner.session(&user, &method, None)).into_response(),
            None => error(StatusCode::BAD_REQUEST, "invalid_grant", "bad code"),
        },
        "password" => {
            let Some(user) = inner.by_email(&text("email")).cloned() else {
                return error(StatusCode::BAD_REQUEST, "invalid_credentials", "no");
            };
            if user.password.as_deref() != Some(text("password").as_str()) {
                return error(StatusCode::BAD_REQUEST, "invalid_credentials", "no");
            }
            if !user.email_verified {
                return pending_verification(&mut inner, &user);
            }
            Json(inner.session(&user.id, "Password", None)).into_response()
        }
        "urn:workos:oauth:grant-type:magic-auth:code" => {
            let email = text("email").to_ascii_lowercase();
            if inner.magic.get(&email) != Some(&text("code")) {
                return error(StatusCode::BAD_REQUEST, "invalid_one_time_code", "bad code");
            }
            inner.magic.remove(&email);
            let user = match inner.by_email(&email).map(|user| user.id.clone()) {
                Some(id) => id,
                None => {
                    let id = inner.id("user");
                    inner.users.insert(
                        id.clone(),
                        FakeUser {
                            id: id.clone(),
                            email: email.clone(),
                            ..FakeUser::default()
                        },
                    );
                    id
                }
            };
            if let Some(record) = inner.users.get_mut(&user) {
                record.email_verified = true;
            }
            Json(inner.session(&user, "MagicAuth", None)).into_response()
        }
        "urn:workos:oauth:grant-type:email-verification:code" => {
            let Some(user) = inner
                .pending
                .get(&text("pending_authentication_token"))
                .cloned()
            else {
                return error(StatusCode::BAD_REQUEST, "invalid_grant", "bad token");
            };
            let ok = inner
                .verifications
                .values()
                .any(|(owner, code)| *owner == user && *code == text("code"));
            if !ok {
                return error(
                    StatusCode::BAD_REQUEST,
                    "email_verification_code_invalid",
                    "bad code",
                );
            }
            inner.pending.remove(&text("pending_authentication_token"));
            if let Some(record) = inner.users.get_mut(&user) {
                record.email_verified = true;
            }
            Json(inner.session(&user, "Password", None)).into_response()
        }
        "refresh_token" => {
            let Some(sid) = inner.refresh.remove(&text("refresh_token")) else {
                return error(StatusCode::BAD_REQUEST, "invalid_grant", "used");
            };
            let Some((user, revoked)) = inner.sessions.get(&sid).cloned() else {
                return error(StatusCode::BAD_REQUEST, "invalid_grant", "gone");
            };
            if revoked {
                return error(StatusCode::BAD_REQUEST, "invalid_grant", "revoked");
            }
            let refresh = inner.secret();
            inner.refresh.insert(refresh.clone(), sid.clone());
            Json(json!({
                "user": inner.users[&user].json(),
                "access_token": mint(&user, &sid, None, 300),
                "refresh_token": refresh,
            }))
            .into_response()
        }
        _ => error(StatusCode::BAD_REQUEST, "unsupported_grant_type", "no"),
    }
}

fn pending_verification(inner: &mut Inner, user: &FakeUser) -> Response {
    let id = inner.id("email_verification");
    let code = inner.code();
    inner
        .verifications
        .insert(id.clone(), (user.id.clone(), code.clone()));
    let pending = inner.secret();
    inner.pending.insert(pending.clone(), user.id.clone());
    inner.inbox.push(Email {
        to: user.email.clone(),
        kind: "email_verification",
        secret: code,
    });
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "code": "email_verification_required",
            "message": "Email ownership must be verified before authentication.",
            "email": user.email,
            "pending_authentication_token": pending,
            "email_verification_id": id,
        })),
    )
        .into_response()
}

async fn list_users(
    State(inner): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let inner = inner.lock().expect("fake state");
    let email = query.get("email").map(|e| e.to_ascii_lowercase());
    let data: Vec<Value> = inner
        .users
        .values()
        .filter(|user| email.as_ref().is_none_or(|email| user.email == *email))
        .map(FakeUser::json)
        .collect();
    Json(json!({"object": "list", "data": data, "list_metadata": {"before": null, "after": null}}))
        .into_response()
}

async fn create_user(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let email = body["email"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !email.contains('@') {
        return creation_error("email_invalid");
    }
    if inner.by_email(&email).is_some() {
        return creation_error("email_not_available");
    }
    let password = body["password"].as_str().map(str::to_string);
    if password
        .as_ref()
        .is_some_and(|p| p.len() < 10 || p == "password1234")
    {
        return creation_error("password_strength_error");
    }
    if let Some(external) = body["external_id"].as_str()
        && inner
            .users
            .values()
            .any(|user| user.external_id.as_deref() == Some(external))
    {
        return creation_error("external_id_not_available");
    }
    let id = inner.id("user");
    let user = FakeUser {
        id: id.clone(),
        email,
        email_verified: body["email_verified"].as_bool().unwrap_or(false),
        first_name: body["first_name"].as_str().map(str::to_string),
        last_name: body["last_name"].as_str().map(str::to_string),
        external_id: body["external_id"].as_str().map(str::to_string),
        metadata: serde_json::from_value(body["metadata"].clone()).unwrap_or_default(),
        password,
        identities: Vec::new(),
    };
    let answer = user.json();
    inner.users.insert(id, user);
    (StatusCode::CREATED, Json(answer)).into_response()
}

fn creation_error(code: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "code": "user_creation_error",
            "message": "Could not create user.",
            "errors": [{"code": code, "message": code}],
        })),
    )
        .into_response()
}

async fn get_user(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let inner = inner.lock().expect("fake state");
    match inner.users.get(&id) {
        Some(user) => Json(user.json()).into_response(),
        None => error(StatusCode::NOT_FOUND, "entity_not_found", "no user"),
    }
}

async fn user_by_external(State(inner): State<Shared>, Path(external): Path<String>) -> Response {
    let inner = inner.lock().expect("fake state");
    match inner
        .users
        .values()
        .find(|user| user.external_id.as_deref() == Some(external.as_str()))
    {
        Some(user) => Json(user.json()).into_response(),
        None => error(StatusCode::NOT_FOUND, "entity_not_found", "no user"),
    }
}

async fn update_user(
    State(inner): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut inner = inner.lock().expect("fake state");
    if let Some(external) = body["external_id"].as_str()
        && inner
            .users
            .values()
            .any(|user| user.id != id && user.external_id.as_deref() == Some(external))
    {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "external_id_not_available",
            "taken",
        );
    }
    let Some(user) = inner.users.get_mut(&id) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no user");
    };
    if let Some(email) = body["email"].as_str() {
        user.email = email.to_ascii_lowercase();
    }
    if let Some(verified) = body["email_verified"].as_bool() {
        user.email_verified = verified;
    }
    if let Some(external) = body["external_id"].as_str() {
        user.external_id = Some(external.to_string());
    }
    if let Ok(metadata) =
        serde_json::from_value::<BTreeMap<String, String>>(body["metadata"].clone())
    {
        user.metadata = metadata;
    }
    Json(user.json()).into_response()
}

async fn delete_user(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    if inner.users.remove(&id).is_none() {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no user");
    }
    inner.memberships.retain(|_, (user, _, _)| *user != id);
    StatusCode::ACCEPTED.into_response()
}

async fn identities(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let inner = inner.lock().expect("fake state");
    let Some(user) = inner.users.get(&id) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no user");
    };
    let items: Vec<Value> = user
        .identities
        .iter()
        .map(|(provider, idp)| json!({"idp_id": idp, "type": "OAuth", "provider": provider}))
        .collect();
    Json(Value::Array(items)).into_response()
}

async fn magic_auth(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let email = body["email"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !email.contains('@') {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "email_invalid",
            "bad email",
        );
    }
    let code = inner.code();
    inner.magic.insert(email.clone(), code.clone());
    inner.inbox.push(Email {
        to: email.clone(),
        kind: "magic_auth",
        secret: code.clone(),
    });
    let id = inner.id("magic_auth");
    let user = inner
        .by_email(&email)
        .map(|user| user.id.clone())
        .unwrap_or_default();
    Json(json!({
        "object": "magic_auth", "id": id, "user_id": user, "email": email,
        "expires_at": STAMP, "created_at": STAMP, "updated_at": STAMP, "code": code,
    }))
    .into_response()
}

async fn email_verification(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let inner = inner.lock().expect("fake state");
    let Some((user, code)) = inner.verifications.get(&id) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "none");
    };
    let email = inner
        .users
        .get(user)
        .map(|u| u.email.clone())
        .unwrap_or_default();
    Json(json!({
        "object": "email_verification", "id": id, "user_id": user, "email": email,
        "expires_at": STAMP, "created_at": STAMP, "updated_at": STAMP, "code": code,
    }))
    .into_response()
}

async fn password_reset(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let email = body["email"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let Some(user) = inner.by_email(&email).map(|user| user.id.clone()) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no user");
    };
    let token = inner.secret();
    inner.resets.insert(token.clone(), user.clone());
    inner.inbox.push(Email {
        to: email.clone(),
        kind: "password_reset",
        secret: token.clone(),
    });
    let id = inner.id("password_reset");
    Json(json!({
        "object": "password_reset", "id": id, "user_id": user, "email": email,
        "expires_at": STAMP, "created_at": STAMP,
        "password_reset_token": token, "password_reset_url": "https://fake.workos/reset",
    }))
    .into_response()
}

async fn password_reset_confirm(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let token = body["token"].as_str().unwrap_or_default().to_string();
    let password = body["new_password"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if password.len() < 10 {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "password_strength_error",
            "weak",
        );
    }
    let Some(user) = inner.resets.remove(&token) else {
        return error(
            StatusCode::BAD_REQUEST,
            "password_reset_token_invalid",
            "bad token",
        );
    };
    let Some(record) = inner.users.get_mut(&user) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no user");
    };
    record.password = Some(password);
    record.email_verified = true;
    Json(json!({"user": record.json()})).into_response()
}

async fn revoke(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    match inner
        .sessions
        .get_mut(body["session_id"].as_str().unwrap_or_default())
    {
        Some(session) => {
            session.1 = true;
            StatusCode::NO_CONTENT.into_response()
        }
        None => error(StatusCode::NOT_FOUND, "entity_not_found", "no session"),
    }
}

async fn list_orgs(
    State(inner): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let inner = inner.lock().expect("fake state");
    let domain = query.get("domains").cloned();
    let data: Vec<Value> = inner
        .orgs
        .values()
        .filter(|org| {
            domain.as_ref().is_none_or(|domain| {
                org["domains"]
                    .as_array()
                    .is_some_and(|domains| domains.iter().any(|d| d["domain"] == domain.as_str()))
            })
        })
        .cloned()
        .collect();
    Json(json!({"object": "list", "data": data, "list_metadata": {"before": null, "after": null}}))
        .into_response()
}

fn org_json(id: &str, name: &str, external: Option<&str>, domain: Option<&str>) -> Value {
    let domains: Vec<Value> = domain
        .map(|domain| {
            vec![json!({
                "object": "organization_domain", "id": "org_domain_01FAKE", "organization_id": id,
                "domain": domain, "state": "verified", "verification_strategy": "manual",
                "created_at": STAMP, "updated_at": STAMP,
            })]
        })
        .unwrap_or_default();
    json!({
        "object": "organization", "id": id, "name": name, "domains": domains,
        "metadata": {}, "external_id": external, "created_at": STAMP, "updated_at": STAMP,
    })
}

impl FakeWorkos {
    /// Add an Organization with an SSO connection on `domain`, answering
    /// its ID. A sign-in through it comes from [`Self::queue_sign_in`]
    /// with the organization ID as the provider.
    pub fn add_sso_organization(&self, name: &str, domain: &str) -> String {
        self.with(|inner| {
            let id = inner.id("org");
            inner
                .orgs
                .insert(id.clone(), org_json(&id, name, None, Some(domain)));
            id
        })
    }
}

async fn create_org(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let external = body["external_id"].as_str().map(str::to_string);
    if external.as_ref().is_some_and(|external| {
        inner
            .orgs
            .values()
            .any(|org| org["external_id"] == external.as_str())
    }) {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "external_id_not_available",
            "taken",
        );
    }
    let id = inner.id("org");
    let org = org_json(
        &id,
        body["name"].as_str().unwrap_or_default(),
        external.as_deref(),
        None,
    );
    inner.orgs.insert(id, org.clone());
    (StatusCode::CREATED, Json(org)).into_response()
}

async fn org_by_external(State(inner): State<Shared>, Path(external): Path<String>) -> Response {
    let inner = inner.lock().expect("fake state");
    match inner
        .orgs
        .values()
        .find(|org| org["external_id"] == external.as_str())
    {
        Some(org) => Json(org.clone()).into_response(),
        None => error(StatusCode::NOT_FOUND, "entity_not_found", "no org"),
    }
}

async fn get_org(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let inner = inner.lock().expect("fake state");
    match inner.orgs.get(&id) {
        Some(org) => Json(org.clone()).into_response(),
        None => error(StatusCode::NOT_FOUND, "entity_not_found", "no org"),
    }
}

async fn update_org(
    State(inner): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let Some(org) = inner.orgs.get_mut(&id) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no org");
    };
    if let Some(name) = body["name"].as_str() {
        org["name"] = Value::String(name.to_string());
    }
    Json(org.clone()).into_response()
}

async fn delete_org(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    if inner.orgs.remove(&id).is_none() {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no org");
    }
    inner.memberships.retain(|_, (_, org, _)| *org != id);
    StatusCode::ACCEPTED.into_response()
}

fn membership_json(inner: &Inner, id: &str, user: &str, org: &str, role: &str) -> Value {
    let user_json = inner
        .users
        .get(user)
        .map(FakeUser::json)
        .unwrap_or(Value::Null);
    json!({
        "object": "organization_membership", "id": id, "user_id": user, "organization_id": org,
        "status": "active", "directory_managed": false, "created_at": STAMP, "updated_at": STAMP,
        "role": {"slug": role}, "roles": [{"slug": role}], "user": user_json,
    })
}

async fn list_memberships(
    State(inner): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let inner = inner.lock().expect("fake state");
    let org = query.get("organization_id").cloned().unwrap_or_default();
    let data: Vec<Value> = inner
        .memberships
        .iter()
        .filter(|(_, (_, o, _))| *o == org)
        .map(|(id, (user, o, role))| membership_json(&inner, id, user, o, role))
        .collect();
    Json(json!({"object": "list", "data": data, "list_metadata": {"before": null, "after": null}}))
        .into_response()
}

/// The role slugs the fake environment has, as a default WorkOS
/// environment with `owner` and `admin` added.
const ROLES: &[&str] = &["owner", "admin", "member"];

async fn roles(State(inner): State<Shared>) -> Response {
    let inner = inner.lock().expect("fake state");
    let data: Vec<Value> = inner
        .roles
        .iter()
        .enumerate()
        .map(|(priority, slug)| {
            json!({
                "object": "role", "id": format!("role_01FAKE{priority:020}"), "slug": slug, "name": slug,
                "description": null, "type": "EnvironmentRole", "resource_type_slug": "environment", "permissions": [],
                "created_at": STAMP, "updated_at": STAMP,
            })
        })
        .collect();
    Json(json!({"object": "list", "data": data})).into_response()
}

async fn create_membership(State(inner): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let user = body["user_id"].as_str().unwrap_or_default().to_string();
    let org = body["organization_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let role = body["role_slug"].as_str().unwrap_or("member").to_string();
    if !inner.roles.contains(&role) {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_role",
            "no such role",
        );
    }
    if !inner.users.contains_key(&user) || !inner.orgs.contains_key(&org) {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "no user or org");
    }
    if inner
        .memberships
        .values()
        .any(|(u, o, _)| *u == user && *o == org)
    {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "membership_already_exists",
            "exists",
        );
    }
    let id = inner.id("om");
    inner
        .memberships
        .insert(id.clone(), (user.clone(), org.clone(), role.clone()));
    (
        StatusCode::CREATED,
        Json(membership_json(&inner, &id, &user, &org, &role)),
    )
        .into_response()
}

async fn update_membership(
    State(inner): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut inner = inner.lock().expect("fake state");
    let role = body["role_slug"].as_str().unwrap_or("member").to_string();
    if !inner.roles.contains(&role) {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_role",
            "no such role",
        );
    }
    let Some(entry) = inner.memberships.get_mut(&id) else {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "none");
    };
    entry.2 = role;
    let (user, org, role) = entry.clone();
    Json(membership_json(&inner, &id, &user, &org, &role)).into_response()
}

async fn delete_membership(State(inner): State<Shared>, Path(id): Path<String>) -> Response {
    let mut inner = inner.lock().expect("fake state");
    if inner.memberships.remove(&id).is_none() {
        return error(StatusCode::NOT_FOUND, "entity_not_found", "none");
    }
    StatusCode::ACCEPTED.into_response()
}
