//! Browser sessions project current native account authority through the SDK.

use super::private::ProtectedFile;
use axum::http::{HeaderMap, HeaderValue, header};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SESSION_COOKIE: &str = "oa_cloud_session";
const WORKSPACE_COOKIE: &str = "oa_cloud_workspace";
const LOGIN_COOKIE: &str = "oa_cloud_login";
const TICKET_SECONDS: u64 = 600;

tokio::task_local! {
    /// The views [`CloudSession::authenticate`] already read during this
    /// request, so the account menu and a Cloud page check sign-in once.
    static VIEWS: std::cell::RefCell<Vec<(String, Option<String>, Viewer)>>;
}

/// Run one request's work with a shared sign-in check: every
/// [`CloudSession::authenticate`] inside `work` for the same cookies reads
/// the account service once. The cache lives only as long as `work`.
pub async fn shared<F: std::future::Future>(work: F) -> F::Output {
    VIEWS.scope(std::cell::RefCell::new(Vec::new()), work).await
}

fn cached(token: &str, selected: Option<&str>) -> Option<Viewer> {
    VIEWS
        .try_with(|views| {
            views
                .borrow()
                .iter()
                .find(|(t, s, v)| t == token && s.as_deref() == selected && v.expires_at > now())
                .map(|(_, _, viewer)| viewer.clone())
        })
        .ok()
        .flatten()
}

fn remember(token: &str, selected: Option<&str>, viewer: &Viewer) {
    let _ = VIEWS.try_with(|views| {
        views
            .borrow_mut()
            .push((token.into(), selected.map(Into::into), viewer.clone()));
    });
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    public_origin: String,
    account_service: String,
    csrf_secret: PathBuf,
}

/// Runtime failures contain no credential, private response, or transport details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    Unauthenticated,
    Forbidden,
    Unavailable,
    InvalidRequest,
    Csrf,
    Conflict,
}

impl SessionError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unauthenticated => "sign_in_required",
            Self::Forbidden => "workspace_access_denied",
            Self::Unavailable => "account_service_unavailable",
            Self::InvalidRequest => "invalid_cloud_request",
            Self::Csrf => "cloud_request_not_authorized",
            Self::Conflict => "native_account_changed",
        }
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Unauthenticated => "Sign in to continue.",
            Self::Forbidden => "Your account can't open this.",
            Self::Unavailable => "Sign-in isn't working right now. Try again later.",
            Self::InvalidRequest => "That didn't work. Check what you entered and try again.",
            Self::Csrf => "This page expired. Reload it and try again.",
            Self::Conflict => "Your account changed. Reload the page.",
        })
    }
}
impl std::error::Error for SessionError {}

type Result<T> = std::result::Result<T, SessionError>;

/// One sanitized workspace from the account owner's current membership list.
#[derive(Clone, Debug, Serialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub role: String,
}

/// A selected workspace read separately under its current native authority.
#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceSelection {
    pub id: String,
    pub name: String,
    pub role: String,
    pub members_epoch: u64,
}

/// A request's current native view; its client never enters serialized HTML.
#[derive(Clone)]
pub struct Viewer {
    pub account_id: String,
    pub account_label: String,
    /// The verified email from the linked GitHub profile.
    pub email: Option<String>,
    /// The linked GitHub profile's picture, on GitHub's avatar host.
    pub avatar_url: Option<String>,
    pub workspaces: Vec<Workspace>,
    pub workspace: Option<WorkspaceSelection>,
    pub session_id: String,
    pub expires_at: u64,
    client: jev::Client,
    credential_digest: String,
}

impl Viewer {
    pub fn client(&self) -> &jev::Client {
        &self.client
    }
}

/// A once-issued native token is retained only until it becomes an HttpOnly cookie.
pub struct SessionGrant {
    pub viewer: Viewer,
    token: jev::ApiKey,
    secure: bool,
}

impl SessionGrant {
    pub fn cookies(&self) -> Result<Vec<HeaderValue>> {
        let remaining = self.viewer.expires_at.saturating_sub(now());
        if remaining == 0 {
            return Err(SessionError::Unauthenticated);
        }
        let mut cookies = legacy_clears(self.secure);
        cookies.extend([
            cookie(SESSION_COOKIE, self.token.expose(), remaining, self.secure)?,
            cookie(WORKSPACE_COOKIE, "", 0, self.secure)?,
            cookie(LOGIN_COOKIE, "", 0, self.secure)?,
        ]);
        Ok(cookies)
    }
}

/// A redeemed recovery: the native replacement key, shown once.
pub struct Recovered {
    pub account: Option<String>,
    pub key: jev::ApiKey,
}

/// A public form ticket and the private nonce cookies that bind it.
pub struct CsrfForm {
    pub token: String,
    pub cookie: Option<HeaderValue>,
    pub legacy_cookies: Vec<HeaderValue>,
}

/// Explicit transport settings and pinned CSRF custody, without an account book.
pub struct CloudSession {
    config: ProtectedFile,
    secret_file: ProtectedFile,
    origin: String,
    host: String,
    account_service: String,
    secure: bool,
    csrf_key: [u8; 32],
    http: reqwest::Client,
}

impl Drop for CloudSession {
    fn drop(&mut self) {
        self.csrf_key.fill(0);
    }
}

impl CloudSession {
    pub fn load(path: &Path) -> std::result::Result<Self, String> {
        let (config, bytes) = ProtectedFile::open(path, 16 * 1024)?;
        let declared: Configuration = serde_json::from_slice(&bytes)
            .map_err(|_| "Private Cloud configuration is malformed.")?;
        if declared.schema != "openagents.cloud.web-config.v1" {
            return Err("Private Cloud configuration has an unsupported schema.".into());
        }
        let origin = endpoint(&declared.public_origin, true)?;
        let service = endpoint(&declared.account_service, false)?;
        let secure = origin.scheme() == "https";
        let host = match origin.port() {
            Some(port) => format!(
                "{}:{port}",
                origin.host_str().ok_or("Cloud origin has no host.")?
            ),
            None => origin.host_str().ok_or("Cloud origin has no host.")?.into(),
        };
        let (secret_file, mut secret) = ProtectedFile::open(&declared.csrf_secret, 128)?;
        let key: [u8; 32] = match secret.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => {
                let trimmed = std::str::from_utf8(&secret)
                    .map_err(|_| "The Cloud CSRF secret must contain 32 bytes or 64 hexadecimal characters.")?
                    .trim();
                if trimmed.len() != 64 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(
                        "The Cloud CSRF secret must contain 32 bytes or 64 hexadecimal characters."
                            .into(),
                    );
                }
                let mut key = [0; 32];
                for (index, byte) in key.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&trimmed[index * 2..index * 2 + 2], 16)
                        .map_err(|_| "The Cloud CSRF secret is malformed.")?;
                }
                key
            }
        };
        secret.fill(0);
        if key == [0; 32] {
            return Err("The Cloud CSRF secret must be privately generated.".into());
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| "The Cloud account transport is unavailable.")?;
        let loaded = Self {
            config,
            secret_file,
            origin: origin.as_str().trim_end_matches('/').into(),
            host,
            account_service: service.as_str().trim_end_matches('/').into(),
            secure,
            csrf_key: key,
            http,
        };
        loaded.health()?;
        Ok(loaded)
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn secure(&self) -> bool {
        self.secure
    }

    /// Replacement, altered bytes, or loss of private modes fences this instance.
    pub fn health(&self) -> std::result::Result<(), String> {
        self.config.check()?;
        self.secret_file.check()
    }

    fn ready(&self) -> Result<()> {
        self.health().map_err(|_| SessionError::Unavailable)
    }

    fn client(&self, credential: &str) -> Result<jev::Client> {
        self.ready()?;
        jev::Client::new(
            jev::Config::new()
                .base_url(&self.account_service)
                .api_key(jev::ApiKey::new(credential))
                .default_model("jev-latest")
                .timeout(Duration::from_secs(5))
                .retry(jev::RetryPolicy {
                    max_retries: 0,
                    budget: Some(Duration::from_secs(5)),
                    ..Default::default()
                })
                .http_client(self.http.clone()),
        )
        .map_err(|_| SessionError::Unavailable)
    }

    pub async fn sign_in(&self, credential: &str) -> Result<SessionGrant> {
        if !credential.starts_with("oak_")
            || credential.len() > 512
            || !credential
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_- .".contains(&b))
            || credential.contains(' ')
        {
            return Err(SessionError::InvalidRequest);
        }
        let issued = self
            .client(credential)?
            .account()
            .sign_in()
            .await
            .map_err(native_error)?;
        if !session_token(issued.token.expose()) {
            return Err(SessionError::Unavailable);
        }
        let viewer = self.read_view(issued.token.expose(), None).await?;
        if viewer.session_id != issued.session.id
            || issued.session.kind != "user"
            || issued.session.account.as_deref() != Some(&viewer.account_id)
            || issued.session.expires_at != viewer.expires_at
        {
            return Err(SessionError::Conflict);
        }
        Ok(SessionGrant {
            viewer,
            token: issued.token,
            secure: self.secure,
        })
    }

    /// Finish a GitHub sign-in: hand the authorization code and its PKCE
    /// verifier to the account service, which exchanges them with GitHub,
    /// finds or creates the account, and issues a session (docs/auth). The
    /// web server never sees the GitHub token.
    pub async fn sign_in_github(&self, code: &str, verifier: &str) -> Result<SessionGrant> {
        if code.is_empty()
            || code.len() > 256
            || !code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || !(43..=128).contains(&verifier.len())
            || !verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.~".contains(&b))
        {
            return Err(SessionError::InvalidRequest);
        }
        self.ready()?;
        let response = self
            .http
            .post(format!("{}/v1/sessions/github", self.account_service))
            // The account service talks to GitHub twice or three times.
            .timeout(Duration::from_secs(20))
            .json(&serde_json::json!({"code": code, "code_verifier": verifier}))
            .send()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        if !status.is_success() {
            return Err(match status.as_u16() {
                400 | 401 | 403 => SessionError::Unauthenticated,
                409 => SessionError::Conflict,
                _ => SessionError::Unavailable,
            });
        }
        #[derive(Deserialize)]
        struct WireSession {
            id: String,
            kind: String,
            account: Option<String>,
            expires_at: u64,
        }
        #[derive(Deserialize)]
        struct Wire {
            session: WireSession,
            token: String,
        }
        let wire: Wire = (bytes.len() <= 64 * 1024)
            .then(|| serde_json::from_slice(&bytes).ok())
            .flatten()
            .ok_or(SessionError::Unavailable)?;
        if !session_token(&wire.token) {
            return Err(SessionError::Unavailable);
        }
        let viewer = self.read_view(&wire.token, None).await?;
        if viewer.session_id != wire.session.id
            || wire.session.kind != "user"
            || wire.session.account.as_deref() != Some(&viewer.account_id)
            || wire.session.expires_at != viewer.expires_at
        {
            return Err(SessionError::Conflict);
        }
        Ok(SessionGrant {
            viewer,
            token: jev::ApiKey::new(wire.token),
            secure: self.secure,
        })
    }

    /// Redeem one recovery token at the native account owner. The token is
    /// sent once, in the body only, with no retry; the replacement account
    /// key is returned once and never stored here.
    pub async fn recover(&self, token: &str) -> Result<Recovered> {
        if !token.starts_with("rcv_")
            || token.len() > 256
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
        {
            return Err(SessionError::InvalidRequest);
        }
        self.ready()?;
        let response = self
            .http
            .post(format!("{}/v1/recovery/redeem", self.account_service))
            .json(&serde_json::json!({"token":token}))
            .send()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        if !status.is_success() {
            return Err(if status.is_client_error() {
                SessionError::Forbidden
            } else {
                SessionError::Unavailable
            });
        }
        #[derive(Deserialize)]
        struct Wire {
            account: Option<String>,
            key_token: String,
        }
        let wire: Wire = (bytes.len() <= 16 * 1024)
            .then(|| serde_json::from_slice(&bytes).ok())
            .flatten()
            .ok_or(SessionError::Unavailable)?;
        if !wire.key_token.starts_with("oak_")
            || wire.key_token.len() > 512
            || wire.account.as_deref().is_some_and(|a| !identifier(a))
        {
            return Err(SessionError::Unavailable);
        }
        self.ready()?;
        Ok(Recovered {
            account: wire.account,
            key: jev::ApiKey::new(wire.key_token),
        })
    }

    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<Viewer> {
        self.request_host(headers)?;
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        if !session_token(&token) {
            return Err(SessionError::Unauthenticated);
        }
        let selected = value(headers, WORKSPACE_COOKIE)?;
        if selected.as_ref().is_some_and(|id| !identifier(id)) {
            return Err(SessionError::InvalidRequest);
        }
        if let Some(viewer) = cached(&token, selected.as_deref()) {
            self.ready()?;
            return Ok(viewer);
        }
        let viewer = self.read_view(&token, selected.as_deref()).await?;
        remember(&token, selected.as_deref(), &viewer);
        Ok(viewer)
    }

    async fn read_view(&self, token: &str, selected: Option<&str>) -> Result<Viewer> {
        let client = self.client(token)?;
        let session = client
            .account()
            .session()
            .await
            .map_err(native_error)?
            .session;
        let expiry = session
            .expires_at
            .as_deref()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(SessionError::Unauthenticated)?;
        let account = session
            .account
            .filter(|id| identifier(id))
            .ok_or(SessionError::Unauthenticated)?;
        if session.kind != "user"
            || session.state.as_deref() != Some("active")
            || !identifier(&session.id)
            || expiry <= now()
        {
            return Err(SessionError::Unauthenticated);
        }
        let details = client.account().details().await.map_err(native_error)?;
        if details.account.id != account || details.workspaces.len() > 128 {
            return Err(SessionError::Conflict);
        }
        let mut seen = BTreeSet::new();
        let mut workspaces = Vec::new();
        for workspace in details.workspaces {
            if !identifier(&workspace.id) || !seen.insert(workspace.id.clone()) {
                return Err(SessionError::Conflict);
            }
            let role = workspace
                .role
                .filter(|role| supported_role(role))
                .ok_or(SessionError::Conflict)?;
            workspaces.push(Workspace {
                name: label(workspace.name.as_deref().unwrap_or(&workspace.id))?,
                id: workspace.id,
                role,
            });
        }
        let selection = match selected {
            Some(id) => {
                let listed = workspaces
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .ok_or(SessionError::Forbidden)?;
                let original = client.account().workspace(id).await.map_err(native_error)?;
                if original.workspace.id != id
                    || !supported_role(&original.role)
                    || original.role != listed.role
                {
                    return Err(SessionError::Conflict);
                }
                Some(WorkspaceSelection {
                    id: id.into(),
                    name: listed.name.clone(),
                    role: original.role,
                    members_epoch: original.workspace.members_epoch,
                })
            }
            None => None,
        };
        // Standing is checked after the account reads as well; these reads do not
        // turn a session revoked during the projection into a current view.
        let final_session = client
            .account()
            .session()
            .await
            .map_err(native_error)?
            .session;
        if final_session.id != session.id
            || final_session.account.as_deref() != Some(&account)
            || final_session.kind != "user"
            || final_session.state.as_deref() != Some("active")
            || final_session
                .expires_at
                .as_deref()
                .and_then(|v| v.parse::<u64>().ok())
                != Some(expiry)
            || expiry <= now()
        {
            return Err(SessionError::Unauthenticated);
        }
        self.ready()?;
        Ok(Viewer {
            account_id: account.clone(),
            account_label: label(details.account.label.as_deref().unwrap_or(&account))?,
            email: details.account.email.clone(),
            avatar_url: details
                .account
                .avatar_url
                .clone()
                .filter(|url| url.starts_with("https://avatars.githubusercontent.com/")),
            workspaces,
            workspace: selection,
            session_id: session.id,
            expires_at: expiry,
            client,
            credential_digest: hex(&Sha256::digest(token.as_bytes())),
        })
    }

    pub async fn select_workspace(&self, headers: &HeaderMap, target: &str) -> Result<Viewer> {
        if !identifier(target) {
            return Err(SessionError::InvalidRequest);
        }
        // Recheck the current selection first; a revoked selection cannot be
        // silently replaced with another workspace by a stale form.
        self.authenticate(headers).await?;
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        self.read_view(&token, Some(target)).await
    }

    pub fn workspace_cookies(&self, workspace: &str) -> Result<Vec<HeaderValue>> {
        self.ready()?;
        if !identifier(workspace) {
            return Err(SessionError::InvalidRequest);
        }
        Ok(vec![
            clear_legacy(WORKSPACE_COOKIE, self.secure),
            cookie(WORKSPACE_COOKIE, workspace, 2_592_000, self.secure)?,
        ])
    }

    /// Reissue current standing at the root path and retire its legacy cookies.
    pub fn refresh_cookies(
        &self,
        headers: &HeaderMap,
        viewer: &Viewer,
    ) -> Result<Vec<HeaderValue>> {
        self.ready()?;
        self.viewer_cookie(headers, viewer)?;
        let remaining = viewer.expires_at.saturating_sub(now());
        if remaining == 0 {
            return Err(SessionError::Unauthenticated);
        }
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        let mut cookies = legacy_clears(self.secure);
        cookies.push(cookie(SESSION_COOKIE, &token, remaining, self.secure)?);
        cookies.push(match &viewer.workspace {
            Some(workspace) => cookie(WORKSPACE_COOKIE, &workspace.id, 2_592_000, self.secure)?,
            None => cookie(WORKSPACE_COOKIE, "", 0, self.secure)?,
        });
        cookies.push(cookie(LOGIN_COOKIE, "", 0, self.secure)?);
        Ok(cookies)
    }

    pub fn clear_cookies(&self) -> Vec<HeaderValue> {
        let mut cookies = legacy_clears(self.secure);
        cookies.extend(
            [SESSION_COOKIE, WORKSPACE_COOKIE, LOGIN_COOKIE]
                .map(|name| cookie(name, "", 0, self.secure).expect("static cookie is valid")),
        );
        cookies
    }

    /// End the cookie's native session without depending on workspace membership.
    pub async fn sign_out_current(&self, headers: &HeaderMap) -> Result<()> {
        self.request_origin(headers)?;
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        if !session_token(&token) {
            return Err(SessionError::Unauthenticated);
        }
        self.client(&token)?
            .account()
            .sign_out()
            .await
            .map_err(native_error)?;
        self.ready()
    }

    pub fn login_csrf(&self, headers: &HeaderMap, scope: &str, target: &str) -> Result<CsrfForm> {
        self.ready()?;
        self.request_host(headers)?;
        let nonce = match value(headers, LOGIN_COOKIE)? {
            Some(nonce) if nonce.len() == 64 && nonce.bytes().all(|b| b.is_ascii_hexdigit()) => {
                nonce
            }
            Some(_) => return Err(SessionError::InvalidRequest),
            None => hex(&secp256k1::rand::random::<[u8; 32]>()),
        };
        Ok(CsrfForm {
            token: self.ticket(&hex(&Sha256::digest(nonce.as_bytes())), None, scope, target)?,
            cookie: Some(cookie(LOGIN_COOKIE, &nonce, TICKET_SECONDS, self.secure)?),
            legacy_cookies: vec![clear_legacy(LOGIN_COOKIE, self.secure)],
        })
    }

    pub fn csrf(
        &self,
        headers: &HeaderMap,
        viewer: &Viewer,
        scope: &str,
        target: &str,
    ) -> Result<String> {
        self.ready()?;
        self.request_host(headers)?;
        self.viewer_cookie(headers, viewer)?;
        self.ticket(&viewer.credential_digest, Some(viewer), scope, target)
    }

    /// A logout review carries no account, workspace, session ID, or label.
    pub fn logout_csrf(&self, headers: &HeaderMap, viewer: &Viewer) -> Result<String> {
        self.ready()?;
        self.request_host(headers)?;
        self.viewer_cookie(headers, viewer)?;
        let mut ticket =
            self.new_ticket(&viewer.credential_digest, Some(viewer), "sign-out", "")?;
        ticket.viewer = None;
        self.seal_ticket(&ticket)
    }

    pub fn verify_csrf(
        &self,
        headers: &HeaderMap,
        viewer: Option<&Viewer>,
        scope: &str,
        target: &str,
        token: &str,
    ) -> Result<()> {
        self.ready()?;
        let ticket = self.checked_ticket(headers, scope, target, token)?;
        let binding = match viewer {
            Some(viewer) => {
                self.viewer_cookie(headers, viewer)?;
                viewer.credential_digest.clone()
            }
            None => hex(&Sha256::digest(
                value(headers, LOGIN_COOKIE)?
                    .ok_or(SessionError::Csrf)?
                    .as_bytes(),
            )),
        };
        if ticket.binding != binding || ticket.viewer != identity(viewer) {
            return Err(SessionError::Csrf);
        }
        Ok(())
    }

    /// A reviewed logout can clear cookies after native standing becomes unavailable.
    pub fn verify_logout_csrf(&self, headers: &HeaderMap, token: &str) -> Result<()> {
        let ticket = self.checked_ticket(headers, "sign-out", "", token)?;
        let credential = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Csrf)?;
        if !session_token(&credential)
            || ticket.binding != hex(&Sha256::digest(credential.as_bytes()))
            || ticket.viewer.is_some()
        {
            return Err(SessionError::Csrf);
        }
        Ok(())
    }

    fn checked_ticket(
        &self,
        headers: &HeaderMap,
        scope: &str,
        target: &str,
        token: &str,
    ) -> Result<Ticket> {
        self.request_origin(headers)?;
        action(scope, target)?;
        if token.len() > 2048 {
            return Err(SessionError::Csrf);
        }
        let (payload, signature) = token.split_once('.').ok_or(SessionError::Csrf)?;
        let payload = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| SessionError::Csrf)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| SessionError::Csrf)?;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.csrf_key).expect("HMAC accepts this key");
        mac.update(b"openagents.cloud.csrf.v1\0");
        mac.update(&payload);
        mac.verify_slice(&signature)
            .map_err(|_| SessionError::Csrf)?;
        let ticket: Ticket = serde_json::from_slice(&payload).map_err(|_| SessionError::Csrf)?;
        let observed = now();
        if ticket.origin != self.origin
            || ticket.scope != scope
            || ticket.target != target
            || ticket.issued_at > observed
            || ticket.expires_at <= observed
            || ticket.expires_at <= ticket.issued_at
            || ticket.expires_at.saturating_sub(ticket.issued_at) > TICKET_SECONDS
        {
            return Err(SessionError::Csrf);
        }
        Ok(ticket)
    }

    fn ticket(
        &self,
        binding: &str,
        viewer: Option<&Viewer>,
        scope: &str,
        target: &str,
    ) -> Result<String> {
        self.seal_ticket(&self.new_ticket(binding, viewer, scope, target)?)
    }

    fn new_ticket(
        &self,
        binding: &str,
        viewer: Option<&Viewer>,
        scope: &str,
        target: &str,
    ) -> Result<Ticket> {
        action(scope, target)?;
        let issued_at = now();
        let expires_at = viewer.map_or(issued_at + TICKET_SECONDS, |viewer| {
            (issued_at + TICKET_SECONDS).min(viewer.expires_at)
        });
        if expires_at <= issued_at {
            return Err(SessionError::Unauthenticated);
        }
        Ok(Ticket {
            origin: self.origin.clone(),
            binding: binding.into(),
            scope: scope.into(),
            target: target.into(),
            viewer: identity(viewer),
            issued_at,
            expires_at,
        })
    }

    fn seal_ticket(&self, ticket: &Ticket) -> Result<String> {
        let payload = serde_json::to_vec(ticket).map_err(|_| SessionError::Unavailable)?;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.csrf_key).expect("HMAC accepts this key");
        mac.update(b"openagents.cloud.csrf.v1\0");
        mac.update(&payload);
        Ok(format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        ))
    }

    fn viewer_cookie(&self, headers: &HeaderMap, viewer: &Viewer) -> Result<()> {
        let token = value(headers, SESSION_COOKIE)?.ok_or(SessionError::Unauthenticated)?;
        if viewer.credential_digest != hex(&Sha256::digest(token.as_bytes()))
            || viewer.expires_at <= now()
            || value(headers, WORKSPACE_COOKIE)?.as_deref()
                != viewer
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.id.as_str())
        {
            return Err(SessionError::Csrf);
        }
        Ok(())
    }

    fn request_host(&self, headers: &HeaderMap) -> Result<()> {
        if headers.get_all(header::HOST).iter().count() != 1
            || headers
                .get(header::HOST)
                .and_then(|value| value.to_str().ok())
                != Some(&self.host)
        {
            return Err(SessionError::Forbidden);
        }
        Ok(())
    }

    fn request_origin(&self, headers: &HeaderMap) -> Result<()> {
        self.request_host(headers)?;
        if headers.get_all(header::ORIGIN).iter().count() != 1
            || headers
                .get(header::ORIGIN)
                .and_then(|value| value.to_str().ok())
                != Some(&self.origin)
            || headers
                .get("sec-fetch-site")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|site| !matches!(site, "same-origin" | "none"))
        {
            return Err(SessionError::Csrf);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {
    origin: String,
    binding: String,
    scope: String,
    target: String,
    viewer: Option<(String, String, Option<(String, u64)>)>,
    issued_at: u64,
    expires_at: u64,
}

fn identity(viewer: Option<&Viewer>) -> Option<(String, String, Option<(String, u64)>)> {
    viewer.map(|viewer| {
        (
            viewer.account_id.clone(),
            viewer.session_id.clone(),
            viewer
                .workspace
                .as_ref()
                .map(|workspace| (workspace.id.clone(), workspace.members_epoch)),
        )
    })
}

fn endpoint(value: &str, origin: bool) -> std::result::Result<url::Url, String> {
    let parsed =
        url::Url::parse(value).map_err(|_| "Cloud endpoints must be explicit HTTP URLs.")?;
    let loopback = match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str().is_none()
        || parsed.path() != "/"
    {
        return Err("Cloud endpoints require HTTPS or a literal loopback HTTP address without credentials, a path, a query, or a fragment.".into());
    }
    if origin && parsed.cannot_be_a_base() {
        return Err("Cloud origin is invalid.".into());
    }
    Ok(parsed)
}

fn cookie(name: &str, value: &str, seconds: u64, secure: bool) -> Result<HeaderValue> {
    cookie_at(name, value, seconds, secure, "/")
}

fn clear_legacy(name: &str, secure: bool) -> HeaderValue {
    cookie_at(name, "", 0, secure, "/cloud").expect("static cookie is valid")
}

fn legacy_clears(secure: bool) -> Vec<HeaderValue> {
    [SESSION_COOKIE, WORKSPACE_COOKIE, LOGIN_COOKIE]
        .map(|name| clear_legacy(name, secure))
        .into()
}

fn cookie_at(
    name: &str,
    value: &str,
    seconds: u64,
    secure: bool,
    path: &str,
) -> Result<HeaderValue> {
    let mut header = HeaderValue::from_str(&format!(
        "{name}={value}; Path={path}; HttpOnly; SameSite=Strict; Max-Age={seconds}{}",
        if secure { "; Secure" } else { "" },
    ))
    .map_err(|_| SessionError::InvalidRequest)?;
    header.set_sensitive(true);
    Ok(header)
}

fn value(headers: &HeaderMap, name: &str) -> Result<Option<String>> {
    if headers.get_all(header::COOKIE).iter().count() > 1 {
        return Err(SessionError::InvalidRequest);
    }
    let Some(cookie) = headers.get(header::COOKIE) else {
        return Ok(None);
    };
    let cookie = cookie.to_str().map_err(|_| SessionError::InvalidRequest)?;
    if cookie.len() > 12 * 1024 {
        return Err(SessionError::InvalidRequest);
    }
    let mut result = None;
    for part in cookie.split(';') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        if key == name {
            if result.is_some()
                || value.is_empty()
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_- .".contains(&byte))
                || value.contains(' ')
            {
                return Err(SessionError::InvalidRequest);
            }
            result = Some(value.into());
        }
    }
    Ok(result)
}

pub(crate) fn native_error(error: jev::Error) -> SessionError {
    match error {
        jev::Error::Api(error) => match error.status {
            401 => SessionError::Unauthenticated,
            403 => SessionError::Forbidden,
            400 | 404 | 409 => SessionError::Conflict,
            _ => SessionError::Unavailable,
        },
        _ => SessionError::Unavailable,
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_- .".contains(&byte))
        && !value.contains(' ')
}
fn session_token(value: &str) -> bool {
    value
        .strip_prefix("sess_")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}
fn supported_role(value: &str) -> bool {
    matches!(value, "owner" | "admin" | "member")
}
fn label(value: &str) -> Result<String> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(SessionError::Conflict);
    }
    Ok(value.chars().take(256).collect())
}
fn action(scope: &str, target: &str) -> Result<()> {
    if !identifier(scope) || target.len() > 512 || target.chars().any(char::is_control) {
        return Err(SessionError::InvalidRequest);
    }
    Ok(())
}
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The account standing a page or host operation pins: session, account,
/// selected workspace and its member list version, and a digest of the
/// account's workspace list.
pub(crate) fn standing_value(viewer: &Viewer) -> serde_json::Value {
    let projection = serde_json::json!({"account":viewer.account_id,"label":viewer.account_label,"workspaces":viewer.workspaces,"selected":viewer.workspace});
    let digest = hex(&Sha256::digest(projection.to_string().as_bytes()));
    serde_json::json!({"active":true,"session_id":viewer.session_id,"account":viewer.account_id,"workspace":viewer.workspace.as_ref().map(|v|&v.id),"members_epoch":viewer.workspace.as_ref().map(|v|v.members_epoch),"projection_digest":format!("sha256:{digest}"),"expires_at":viewer.expires_at})
}

#[cfg(test)]
mod tests;
