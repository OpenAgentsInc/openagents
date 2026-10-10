//! OAuth 2.1 for apps that act for a person on openagents.com (#11084):
//! MCP clients such as Claude, ChatGPT, Cursor, and VS Code sign in to
//! `https://openagents.com/mcp` on their own.
//!
//! The authorization server is this site's own sign-in (GitHub, the
//! invite list applies, docs/auth/github.md); no outside server is
//! involved.
//!
//! - `GET /.well-known/oauth-protected-resource` and
//!   `GET /.well-known/oauth-protected-resource/mcp`: RFC 9728 metadata
//!   for the origin and for `/mcp` (path form, RFC 9728 section 3.1),
//!   naming this site as the authorization server.
//! - `GET /.well-known/oauth-authorization-server`: RFC 8414 metadata,
//!   PKCE `S256` only, the registration endpoint, and the `agent_auth`
//!   block `/auth.md` describes.
//! - `POST /oauth/register`: RFC 7591 dynamic client registration for
//!   public clients (`token_endpoint_auth_method: none`). Nothing is
//!   stored: the `client_id` carries the client's name and redirect
//!   addresses under the server's MAC, so any instance can read it.
//! - `GET /oauth/authorize`: the authorization code grant. A visitor who
//!   isn't signed in goes through sign-in and comes back; a signed-in one
//!   sees "Let <app> use your OpenAgents account?" with Approve and Deny.
//! - `POST /oauth/authorize`: the decision. Approve starts a device
//!   sign-in for the app and approves it at once under the browser's
//!   session (`cloud::session::oauth`); the code handed back to the app
//!   carries that sign-in's device code sealed (AES-256-GCM) under a key
//!   derived from the server's secret, with the PKCE challenge, the
//!   client, the redirect address, and a five-minute expiry.
//! - `POST /oauth/token`: the app redeems the code with its PKCE
//!   verifier; the answer is the app session the device sign-in issues
//!   (a `sess_` bearer, 30 days). The account service issues it once, so
//!   a code works once. The app shows in Settings' Computers section as
//!   "<app> on <redirect host>" with Remove.
//!
//! `/mcp` itself is answered by the private service behind this site; a
//! `401` from it gets a `WWW-Authenticate` naming the path-form metadata
//! ([`challenge`], applied in `lib.rs`'s guard).

use std::collections::BTreeMap;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::account::Account;
use crate::cloud::session::{CloudSession, SessionError, Viewer};
use crate::cloud::{failure, protect, refused};
use crate::ui_page::UiPage;

/// RFC 9728 metadata for the origin.
pub(crate) const RESOURCE_METADATA: &str = "/.well-known/oauth-protected-resource";
/// RFC 9728 metadata for `/mcp`, in the path form.
pub(crate) const MCP_RESOURCE_METADATA: &str = "/.well-known/oauth-protected-resource/mcp";
/// RFC 8414 metadata.
pub(crate) const SERVER_METADATA: &str = "/.well-known/oauth-authorization-server";
pub(crate) const AUTHORIZE: &str = "/oauth/authorize";
pub(crate) const TOKEN: &str = "/oauth/token";
pub(crate) const REGISTER: &str = "/oauth/register";

/// The one scope: the app acts as the person, as a signed-in app does.
pub(crate) const SCOPE: &str = "account";

/// How long an authorization code stands.
const CODE_SECONDS: u64 = 300;
/// The cookie that keeps an authorization request across sign-in.
const REQUEST_COOKIE: &str = "oa_oauth_request";
/// How long it stands: the length of a sign-in.
const REQUEST_SECONDS: u64 = 600;
/// The longest authorization request kept across sign-in.
const REQUEST_MAX: usize = 3000;

const CLIENT_PREFIX: &str = "oac_";
const CODE_PREFIX: &str = "oaz_";
const CLIENT_ID_MAX: usize = 2048;
const REDIRECTS_MAX: usize = 5;
const REDIRECT_MAX: usize = 300;
const NAME_MAX: usize = 48;
const STATE_MAX: usize = 1024;

/// The CSRF scope of the consent form.
const CONSENT: &str = "oauth-authorize";

/// Whether `path` is one of this module's addresses (the upstream guard
/// keeps them here).
pub(crate) fn owns(path: &str) -> bool {
    matches!(
        path,
        RESOURCE_METADATA | MCP_RESOURCE_METADATA | SERVER_METADATA
    ) || path.starts_with("/oauth/")
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(RESOURCE_METADATA, get(resource_metadata).options(preflight))
        .route(MCP_RESOURCE_METADATA, get(mcp_metadata).options(preflight))
        .route(SERVER_METADATA, get(server_metadata).options(preflight))
        .route(REGISTER, post(register).options(preflight))
        .route(AUTHORIZE, get(authorize).post(decide))
        .route(TOKEN, post(token).options(preflight))
        .layer(DefaultBodyLimit::max(16 * 1024))
}

// ---------------------------------------------------------------------
// Metadata

/// The origin the documents name: the account service's public origin,
/// else the site's.
fn origin(app: &App) -> String {
    match app.config.cloud.as_deref() {
        Some(service) => service.origin().to_owned(),
        None => crate::wellknown::origin(app),
    }
}

/// RFC 9728 metadata for the resource at `origin` + `path` (`""` for the
/// origin, `"/mcp"` for the MCP server).
pub(crate) fn protected_resource(origin: &str, path: &str) -> Value {
    let name = if path.is_empty() {
        "OpenAgents"
    } else {
        "OpenAgents MCP server"
    };
    json!({
        "resource": format!("{origin}{path}"),
        "authorization_servers": [origin],
        "scopes_supported": [SCOPE],
        "bearer_methods_supported": ["header"],
        "resource_name": name,
        "resource_documentation": format!("{origin}/auth.md"),
    })
}

/// RFC 8414 metadata, with the `agent_auth` block (`/auth.md`).
pub(crate) fn authorization_server(origin: &str) -> Value {
    let authorize = format!("{origin}{AUTHORIZE}");
    let token = format!("{origin}{TOKEN}");
    let register = format!("{origin}{REGISTER}");
    json!({
        "issuer": origin,
        "authorization_endpoint": authorize,
        "token_endpoint": token,
        "registration_endpoint": register,
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": [SCOPE],
        "authorization_response_iss_parameter_supported": true,
        "client_id_metadata_document_supported": false,
        "service_documentation": format!("{origin}/auth.md"),
        "agent_auth": {
            "skill": format!("{origin}/auth.md"),
            "register_uri": register,
            "registration_methods": [
                {
                    "type": "oauth2_dynamic_client_registration",
                    "registration_endpoint": register,
                    "authorization_endpoint": authorize,
                    "token_endpoint": token,
                    "grant_types": ["authorization_code"],
                    "response_types": ["code"],
                    "code_challenge_methods": ["S256"],
                    "token_endpoint_auth_method": "none",
                    "scopes": [SCOPE],
                    "approval": "A signed-in person approves the app in the browser.",
                },
                {
                    "type": "api_key",
                    "issue_uri": format!("{origin}/settings/api-keys"),
                    "header": "Authorization: Bearer <key>",
                    "documentation": format!("{origin}/auth.md"),
                },
            ],
        },
    })
}

/// The `WWW-Authenticate` a `401` from `/mcp` carries: where its
/// metadata is and the scope to ask for (RFC 9728 section 5.1).
pub(crate) fn challenge(origin: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "Bearer resource_metadata=\"{origin}{MCP_RESOURCE_METADATA}\", scope=\"{SCOPE}\""
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("Bearer"))
}

/// Whether `path` is the keyed MCP server answered behind this site
/// (not the public docs server at `/mcp/docs`).
pub(crate) fn keyed_mcp(path: &str) -> bool {
    (path == "/mcp" || path.starts_with("/mcp/"))
        && path != crate::agent_ready::MCP_PATH
        && !path.starts_with("/mcp/docs/")
}

/// Every answer an app reads from another origin carries CORS (a browser
/// MCP client calls these): no cookies are read on them.
fn cors(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Authorization, Content-Type, MCP-Protocol-Version"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    response
}

async fn preflight() -> Response {
    cors(StatusCode::NO_CONTENT.into_response())
}

fn metadata(document: Value) -> Response {
    let mut response = axum::Json(document).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    cors(response)
}

async fn resource_metadata(State(app): State<App>) -> Response {
    metadata(protected_resource(&origin(&app), ""))
}

async fn mcp_metadata(State(app): State<App>) -> Response {
    metadata(protected_resource(&origin(&app), "/mcp"))
}

async fn server_metadata(State(app): State<App>) -> Response {
    metadata(authorization_server(&origin(&app)))
}

// ---------------------------------------------------------------------
// Keys, clients, and codes

fn mac(key: &[u8; 32], label: &[u8], data: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts this key");
    mac.update(label);
    mac.update(b"\0");
    mac.update(data);
    mac
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A registered client: what the `client_id` carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Client {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "r")]
    pub redirect_uris: Vec<String>,
    #[serde(rename = "t")]
    pub issued_at: u64,
}

/// The `client_id` for `client`: its fields and a MAC.
pub(crate) fn client_id(key: &[u8; 32], client: &Client) -> String {
    let payload = serde_json::to_vec(client).expect("a client serializes");
    let body = URL_SAFE_NO_PAD.encode(payload);
    let tag = mac(key, b"openagents.oauth.client.v1", body.as_bytes())
        .finalize()
        .into_bytes();
    format!(
        "{CLIENT_PREFIX}{body}.{}",
        URL_SAFE_NO_PAD.encode(&tag[..16])
    )
}

/// The client a `client_id` names, when this server made it.
pub(crate) fn read_client(key: &[u8; 32], id: &str) -> Option<Client> {
    if id.len() > CLIENT_ID_MAX {
        return None;
    }
    let (body, tag) = id.strip_prefix(CLIENT_PREFIX)?.split_once('.')?;
    let tag = URL_SAFE_NO_PAD.decode(tag).ok()?;
    if tag.len() != 16 {
        return None;
    }
    mac(key, b"openagents.oauth.client.v1", body.as_bytes())
        .verify_truncated_left(&tag)
        .ok()?;
    let client: Client = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(body).ok()?).ok()?;
    (!client.redirect_uris.is_empty()).then_some(client)
}

/// What an authorization code carries, sealed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Code {
    /// The approved device sign-in's code, redeemed at the token endpoint.
    #[serde(rename = "d")]
    pub device: String,
    /// SHA-256 of the `client_id`.
    #[serde(rename = "c")]
    pub client: String,
    /// The redirect address the code was sent to.
    #[serde(rename = "r")]
    pub redirect_uri: String,
    /// Whether the request named it (then the token request must too).
    #[serde(rename = "g")]
    pub redirect_given: bool,
    /// The PKCE `S256` challenge.
    #[serde(rename = "p")]
    pub challenge: String,
    /// When it stops working (Unix seconds).
    #[serde(rename = "e")]
    pub expires_at: u64,
}

const CODE_BINDING: &[u8] = b"openagents.oauth.code.v1";

fn keyring(key: &[u8; 32]) -> Option<oa_seal::Keyring> {
    let document = json!({
        "schema": oa_seal::KEYRING_SCHEMA,
        "current": "oauth",
        "keys": {"oauth": STANDARD.encode(key)},
    });
    oa_seal::Keyring::parse(document.to_string().as_bytes()).ok()
}

/// `code`, sealed under `key`: `oaz_` and the 96-bit IV with the
/// ciphertext and its tag, URL-safe.
pub(crate) fn seal_code(key: &[u8; 32], code: &Code) -> Option<String> {
    let plain = serde_json::to_vec(code).ok()?;
    let sealed = keyring(key)?.seal(CODE_BINDING, &plain).ok()?;
    let mut bytes = STANDARD.decode(&sealed.nonce).ok()?;
    bytes.extend(STANDARD.decode(&sealed.ciphertext).ok()?);
    Some(format!("{CODE_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

/// The code `text` carries, when this server sealed it and nobody
/// changed it.
pub(crate) fn open_code(key: &[u8; 32], text: &str) -> Option<Code> {
    if text.len() > 2048 {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(text.strip_prefix(CODE_PREFIX)?)
        .ok()?;
    if bytes.len() <= 12 + 16 {
        return None;
    }
    let sealed = oa_seal::Sealed {
        key_id: "oauth".into(),
        nonce: STANDARD.encode(&bytes[..12]),
        ciphertext: STANDARD.encode(&bytes[12..]),
    };
    let plain = keyring(key)?.open(CODE_BINDING, &sealed).ok()?;
    serde_json::from_slice(&plain).ok()
}

/// PKCE `S256`: the challenge a verifier answers (RFC 7636 4.6).
pub(crate) fn s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// A verifier's shape: 43 to 128 unreserved characters (RFC 7636 4.1).
fn verifier_shape(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

/// A `S256` challenge's shape: 43 URL-safe base64 characters.
fn challenge_shape(challenge: &str) -> bool {
    challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

// ---------------------------------------------------------------------
// Redirect addresses

fn loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Whether an app may register `uri` (RFC 8252, OAuth 2.1 section 2.3):
/// `https`, `http` on this computer's loopback, or a native app's own
/// scheme; never a fragment, credentials, or a scheme a browser runs.
pub(crate) fn redirect_allowed(uri: &str) -> bool {
    if uri.len() > REDIRECT_MAX {
        return false;
    }
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    match url.scheme() {
        "https" => url.host_str().is_some_and(|host| !host.is_empty()),
        "http" => loopback(&url),
        "javascript" | "data" | "file" | "blob" | "vbscript" | "about" | "ftp" | "ws" | "wss"
        | "mailto" | "tel" | "sms" | "filesystem" | "view-source" | "chrome" | "intent" => false,
        _ => true,
    }
}

/// Whether `given` is the registered `registered`: the same string, or,
/// for a loopback address, the same but for the port (RFC 8252 7.3).
pub(crate) fn same_redirect(registered: &str, given: &str) -> bool {
    if registered == given {
        return true;
    }
    match (url::Url::parse(registered), url::Url::parse(given)) {
        (Ok(a), Ok(b)) => {
            a.scheme() == "http"
                && b.scheme() == "http"
                && loopback(&a)
                && loopback(&b)
                && a.host() == b.host()
                && a.path() == b.path()
                && a.query() == b.query()
        }
        _ => false,
    }
}

/// What Settings calls where the app runs: the redirect's host, "this
/// computer" for a loopback address, or a native app's scheme.
fn computer_label(uri: &str) -> String {
    let label = match url::Url::parse(uri) {
        Ok(url) if loopback(&url) => "this computer".to_owned(),
        Ok(url) => match url.host_str() {
            Some(host) if !host.is_empty() && url.scheme() == "https" => host.to_owned(),
            _ => format!("{} app", url.scheme()),
        },
        Err(_) => "this computer".to_owned(),
    };
    label.chars().take(64).collect()
}

/// `base` with the query pairs added.
fn with_query(base: &str, pairs: &[(&str, &str)]) -> Option<String> {
    let mut url = url::Url::parse(base).ok()?;
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in pairs {
            query.append_pair(name, value);
        }
    }
    Some(url.to_string())
}

/// The form target the consent page's policy allows besides this site:
/// the redirect's origin, or a native app's scheme.
fn form_target(uri: &str) -> Option<String> {
    let url = url::Url::parse(uri).ok()?;
    let target = match url.scheme() {
        "http" | "https" => url.origin().ascii_serialization(),
        scheme => format!("{scheme}:"),
    };
    target
        .bytes()
        .all(|b| b.is_ascii_graphic() && !matches!(b, b';' | b',' | b'\''))
        .then_some(target)
}

// ---------------------------------------------------------------------
// Registration (RFC 7591)

/// Why a registration is refused: the RFC 7591 code and plain words.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Refused(pub &'static str, pub &'static str);

/// The client a registration request describes.
pub(crate) fn registration(body: &Value, now: u64) -> Result<Client, Refused> {
    let uris = body["redirect_uris"]
        .as_array()
        .filter(|uris| !uris.is_empty() && uris.len() <= REDIRECTS_MAX)
        .ok_or(Refused(
            "invalid_redirect_uri",
            "Send one to five redirect_uris.",
        ))?;
    let mut redirect_uris = Vec::new();
    for uri in uris {
        let uri = uri
            .as_str()
            .filter(|uri| redirect_allowed(uri))
            .ok_or(Refused(
                "invalid_redirect_uri",
                "Each redirect address must be https, http on 127.0.0.1 or localhost, or the app's own scheme, with no fragment.",
            ))?;
        if !redirect_uris.iter().any(|known| known == uri) {
            redirect_uris.push(uri.to_owned());
        }
    }
    if let Some(types) = body.get("grant_types") {
        let ok = types
            .as_array()
            .is_some_and(|types| types.iter().any(|t| t == "authorization_code"));
        if !ok {
            return Err(Refused(
                "invalid_client_metadata",
                "Only the authorization_code grant is offered.",
            ));
        }
    }
    if let Some(types) = body.get("response_types") {
        let ok = types
            .as_array()
            .is_some_and(|types| types.iter().any(|t| t == "code"));
        if !ok {
            return Err(Refused(
                "invalid_client_metadata",
                "Only the code response type is offered.",
            ));
        }
    }
    let named: String = body["client_name"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(NAME_MAX)
        .collect();
    let name = if named.is_empty() {
        url::Url::parse(&redirect_uris[0])
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .filter(|host| !host.is_empty())
            .unwrap_or_else(|| "An app".to_owned())
    } else {
        named
    };
    Ok(Client {
        name,
        redirect_uris,
        issued_at: now,
    })
}

fn oauth_error(status: StatusCode, code: &str, description: &str) -> Response {
    cors(protect(
        (
            status,
            axum::Json(json!({"error": code, "error_description": description})),
        )
            .into_response(),
    ))
}

fn unavailable() -> Response {
    oauth_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "temporarily_unavailable",
        "Sign-in isn't available right now. Try again in a minute.",
    )
}

/// `POST /oauth/register` — a JSON client registration.
async fn register(State(app): State<App>, body: Bytes) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            "Send the registration as a JSON object.",
        );
    };
    let now = crate::cloud::session::now();
    let client = match registration(&request, now) {
        Ok(client) => client,
        Err(Refused(code, words)) => return oauth_error(StatusCode::BAD_REQUEST, code, words),
    };
    let id = client_id(&service.oauth_key("client"), &client);
    if id.len() > CLIENT_ID_MAX {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            "Register fewer or shorter redirect addresses.",
        );
    }
    cors(protect(
        (
            StatusCode::CREATED,
            axum::Json(json!({
                "client_id": id,
                "client_id_issued_at": client.issued_at,
                "client_name": client.name,
                "redirect_uris": client.redirect_uris,
                "grant_types": ["authorization_code"],
                "response_types": ["code"],
                "token_endpoint_auth_method": "none",
                "scope": SCOPE,
            })),
        )
            .into_response(),
    ))
}

// ---------------------------------------------------------------------
// Authorization

/// A checked authorization request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Authorization {
    pub client_id: String,
    pub client: Client,
    pub redirect_uri: String,
    pub redirect_given: bool,
    pub challenge: String,
    pub state: Option<String>,
}

/// Why an authorization request can't go on: a page for the person when
/// the app or its redirect address can't be trusted, else an error sent
/// back to the app (RFC 6749 4.1.2.1).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    Page(&'static str),
    Back {
        redirect_uri: String,
        state: Option<String>,
        error: &'static str,
        description: &'static str,
    },
}

const BAD_LINK: &str = "This sign-in link isn't valid. Start again in the app.";

/// Whether `resource` (RFC 8707) is one this server protects.
fn resource_allowed(origin: &str, resource: &str) -> bool {
    let trimmed = resource.trim_end_matches('/');
    trimmed == origin || trimmed == format!("{origin}/mcp")
}

/// Check an authorization request's query.
pub(crate) fn authorization(
    client_key: &[u8; 32],
    origin: &str,
    query: &str,
) -> Result<Authorization, Stop> {
    let mut params = BTreeMap::new();
    for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if params
            .insert(name.into_owned(), value.into_owned())
            .is_some()
        {
            return Err(Stop::Page(BAD_LINK));
        }
    }
    let client_id = params.get("client_id").ok_or(Stop::Page(BAD_LINK))?;
    let client = read_client(client_key, client_id).ok_or(Stop::Page(BAD_LINK))?;
    let (redirect_uri, redirect_given) = match params.get("redirect_uri") {
        Some(given) => client
            .redirect_uris
            .iter()
            .any(|registered| same_redirect(registered, given))
            .then(|| (given.clone(), true))
            .ok_or(Stop::Page(BAD_LINK))?,
        None if client.redirect_uris.len() == 1 => (client.redirect_uris[0].clone(), false),
        None => return Err(Stop::Page(BAD_LINK)),
    };
    let state = params.get("state").cloned();
    if state.as_ref().is_some_and(|state| state.len() > STATE_MAX) {
        return Err(Stop::Page(BAD_LINK));
    }
    let back = |error, description| Stop::Back {
        redirect_uri: redirect_uri.clone(),
        state: state.clone(),
        error,
        description,
    };
    if params.get("response_type").map(String::as_str) != Some("code") {
        return Err(back(
            "unsupported_response_type",
            "Only response_type=code is offered.",
        ));
    }
    let challenge = params
        .get("code_challenge")
        .filter(|challenge| challenge_shape(challenge));
    let Some(challenge) = challenge else {
        return Err(back(
            "invalid_request",
            "Send a PKCE code_challenge with code_challenge_method=S256.",
        ));
    };
    if params.get("code_challenge_method").map(String::as_str) != Some("S256") {
        return Err(back(
            "invalid_request",
            "Only code_challenge_method=S256 is offered.",
        ));
    }
    if params
        .get("resource")
        .is_some_and(|resource| !resource_allowed(origin, resource))
    {
        return Err(back(
            "invalid_target",
            "The resource must be this site or its /mcp address.",
        ));
    }
    Ok(Authorization {
        client_id: client_id.clone(),
        client,
        redirect_uri,
        redirect_given,
        challenge: challenge.clone(),
        state,
    })
}

/// Send the app back to its redirect address with `pairs` and `iss`.
fn back_to_app(
    origin: &str,
    redirect_uri: &str,
    state: Option<&str>,
    pairs: &[(&str, &str)],
) -> Response {
    let mut all: Vec<(&str, &str)> = pairs.to_vec();
    if let Some(state) = state {
        all.push(("state", state));
    }
    all.push(("iss", origin));
    match with_query(redirect_uri, &all) {
        Some(to) => protect(Redirect::to(&to).into_response()),
        None => failure(StatusCode::BAD_REQUEST, "Sign-in link not valid", BAD_LINK),
    }
}

fn stopped(origin: &str, stop: Stop) -> Response {
    match stop {
        Stop::Page(words) => failure(StatusCode::BAD_REQUEST, "Sign-in link not valid", words),
        Stop::Back {
            redirect_uri,
            state,
            error,
            description,
        } => back_to_app(
            origin,
            &redirect_uri,
            state.as_deref(),
            &[("error", error), ("error_description", description)],
        ),
    }
}

/// The request kept across sign-in, when the cookie holds one.
fn kept_request(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .find(|(name, _)| *name == REQUEST_COOKIE)
        .and_then(|(_, value)| URL_SAFE_NO_PAD.decode(value).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|query| query.len() <= REQUEST_MAX)
}

fn request_cookie(value: &str, max_age: u64, secure: bool) -> Option<HeaderValue> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{REQUEST_COOKIE}={value}; Path=/oauth/; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    ))
    .ok()
}

/// `GET /oauth/authorize?...`, or `?resume=1` back from sign-in.
async fn authorize(State(app): State<App>, headers: HeaderMap, uri: Uri) -> Response {
    let service = match crate::cloud::service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let origin = service.origin().to_owned();
    let query = uri.query().unwrap_or_default();
    let resumed = query == "resume=1";
    let query = if resumed {
        match kept_request(&headers) {
            Some(kept) => kept,
            None => {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "Sign-in took too long",
                    "Start again in the app.",
                );
            }
        }
    } else {
        query.to_owned()
    };
    let request = match authorization(&service.oauth_key("client"), &origin, &query) {
        Ok(request) => request,
        Err(stop) => return stopped(&origin, stop),
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(viewer) => viewer,
        Err(SessionError::Unauthenticated) => {
            if query.len() > REQUEST_MAX {
                return failure(StatusCode::BAD_REQUEST, "Sign-in link not valid", BAD_LINK);
            }
            let back = format!("{AUTHORIZE}?resume=1");
            let mut response =
                protect(Redirect::to(&crate::auth::login_href(&back, false)).into_response());
            if let Some(cookie) = request_cookie(
                &URL_SAFE_NO_PAD.encode(query.as_bytes()),
                REQUEST_SECONDS,
                service.secure(),
            ) {
                response.headers_mut().append(header::SET_COOKIE, cookie);
            }
            return response;
        }
        Err(error) => return refused(error),
    };
    let csrf = match service.csrf(&headers, &viewer, CONSENT, &sha256_hex(&query)) {
        Ok(csrf) => csrf,
        Err(error) => return refused(error),
    };
    let mut response = consent_page(
        &headers,
        service,
        &viewer,
        &request,
        &consent_form(&request, &query, &csrf),
    );
    if resumed && let Some(cookie) = request_cookie("", 0, service.secure()) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// "Let <app> use your OpenAgents account?" with Approve and Deny.
pub(crate) fn consent_form(request: &Authorization, query: &str, csrf: &str) -> Markup {
    let name = &request.client.name;
    let place = computer_label(&request.redirect_uri);
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Let " (name) " use your OpenAgents account?" }
            p {
                (name) " will be able to use OpenAgents as you: your chats, your projects, and anything else your account can do. Approve only if you just asked "
                (name) " to sign in."
            }
            p { "After you choose, you go back to " strong { (place) } ". You can remove " (name) " any time in Settings." }
        }))
        form method="post" action=(AUTHORIZE) {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="request" value=(query);
            div.oa-page-actions {
                (Button::new("Approve").kind(ButtonType::Submit).name("decision").value("approve"))
                (Button::new("Deny")
                    .kind(ButtonType::Submit)
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary)
                    .name("decision")
                    .value("deny"))
            }
        }
    }
}

/// The consent page's policy: the account pages' own, with the form also
/// allowed to land on the app's redirect address.
fn consent_policy(redirect_uri: &str) -> String {
    let extra = form_target(redirect_uri)
        .map(|target| format!(" {target}"))
        .unwrap_or_default();
    format!(
        "default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self'; \
         script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'{extra}; \
         frame-ancestors 'none'"
    )
}

fn consent_page(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    request: &Authorization,
    body: &Markup,
) -> Response {
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    let title = format!("Let {} use your account?", request.client.name);
    let mut response = protect(
        UiPage::new(title)
            .path(AUTHORIZE)
            .status(StatusCode::OK)
            .account(account)
            .content(PageColumn::new(body.clone()))
            .respond(headers),
    );
    if let Ok(policy) = HeaderValue::from_str(&consent_policy(&request.redirect_uri)) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, policy);
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideForm {
    csrf: String,
    request: String,
    decision: String,
}

/// `POST /oauth/authorize` — Approve or Deny.
async fn decide(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<DecideForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let service = match crate::cloud::service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let origin = service.origin().to_owned();
    let viewer = match service.authenticate(&headers).await {
        Ok(viewer) => viewer,
        Err(error) => return refused(error),
    };
    if form.request.len() > REQUEST_MAX {
        return refused(SessionError::InvalidRequest);
    }
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        CONSENT,
        &sha256_hex(&form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    let request = match authorization(&service.oauth_key("client"), &origin, &form.request) {
        Ok(request) => request,
        Err(stop) => return stopped(&origin, stop),
    };
    let state = request.state.as_deref();
    match form.decision.as_str() {
        "deny" => {
            return back_to_app(
                &origin,
                &request.redirect_uri,
                state,
                &[
                    ("error", "access_denied"),
                    ("error_description", "The person denied the sign-in."),
                ],
            );
        }
        "approve" => {}
        _ => return refused(SessionError::InvalidRequest),
    }
    let place = computer_label(&request.redirect_uri);
    let device = match service
        .oauth_approve(&headers, &request.client.name, &place)
        .await
    {
        Ok(Ok(device)) => device,
        Ok(Err(_)) => {
            return back_to_app(
                &origin,
                &request.redirect_uri,
                state,
                &[
                    ("error", "server_error"),
                    (
                        "error_description",
                        "The sign-in couldn't be made. Try again.",
                    ),
                ],
            );
        }
        Err(error) => return refused(error),
    };
    let code = Code {
        device,
        client: sha256_hex(&request.client_id),
        redirect_uri: request.redirect_uri.clone(),
        redirect_given: request.redirect_given,
        challenge: request.challenge.clone(),
        expires_at: crate::cloud::session::now() + CODE_SECONDS,
    };
    let Some(sealed) = seal_code(&service.oauth_key("code"), &code) else {
        return refused(SessionError::Unavailable);
    };
    back_to_app(
        &origin,
        &request.redirect_uri,
        state,
        &[("code", sealed.as_str())],
    )
}

// ---------------------------------------------------------------------
// Token

/// A form body (or, leniently, a JSON object) as a flat map of strings.
fn token_fields(headers: &HeaderMap, body: &Bytes) -> Option<BTreeMap<String, String>> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if json {
        let value: Value = serde_json::from_slice(body).ok()?;
        return Some(
            value
                .as_object()?
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                .collect(),
        );
    }
    let mut fields = BTreeMap::new();
    for (name, value) in url::form_urlencoded::parse(body) {
        if fields
            .insert(name.into_owned(), value.into_owned())
            .is_some()
        {
            return None;
        }
    }
    Some(fields)
}

/// The client id from HTTP Basic, when a client sends it that way.
fn basic_client(headers: &HeaderMap) -> Option<String> {
    let encoded = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Basic ")?;
    let decoded = String::from_utf8(STANDARD.decode(encoded.trim()).ok()?).ok()?;
    let user = decoded.split_once(':').map_or(decoded.as_str(), |(u, _)| u);
    Some(
        url::form_urlencoded::parse(format!("u={user}").as_bytes())
            .next()?
            .1
            .into_owned(),
    )
}

/// Why a token request is refused before the account service is asked.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TokenRefused(pub &'static str, pub &'static str);

/// Check a token request against the code it redeems: answers the
/// device code to redeem.
pub(crate) fn redeemable(
    code_key: &[u8; 32],
    origin: &str,
    fields: &BTreeMap<String, String>,
    basic: Option<&str>,
    now: u64,
) -> Result<String, TokenRefused> {
    let field = |name: &str| fields.get(name).map(String::as_str);
    if field("grant_type") != Some("authorization_code") {
        return Err(TokenRefused(
            "unsupported_grant_type",
            "Use grant_type=authorization_code.",
        ));
    }
    let invalid = |words| TokenRefused("invalid_grant", words);
    let code = field("code")
        .and_then(|code| open_code(code_key, code))
        .ok_or(invalid("The code isn't valid. Sign in again."))?;
    if code.expires_at <= now {
        return Err(invalid("The code expired. Sign in again."));
    }
    let client = field("client_id")
        .or(basic)
        .ok_or(TokenRefused("invalid_request", "Send client_id."))?;
    if sha256_hex(client) != code.client {
        return Err(invalid("The code was made for another app."));
    }
    match field("redirect_uri") {
        Some(given) if given != code.redirect_uri => {
            return Err(invalid("redirect_uri doesn't match the sign-in request."));
        }
        None if code.redirect_given => {
            return Err(invalid("Send the redirect_uri the sign-in request named."));
        }
        _ => {}
    }
    let verifier = field("code_verifier")
        .filter(|verifier| verifier_shape(verifier))
        .ok_or(TokenRefused(
            "invalid_request",
            "Send the PKCE code_verifier.",
        ))?;
    if s256(verifier) != code.challenge {
        return Err(invalid(
            "The code_verifier doesn't match the code_challenge.",
        ));
    }
    if field("resource").is_some_and(|resource| !resource_allowed(origin, resource)) {
        return Err(TokenRefused(
            "invalid_target",
            "The resource must be this site or its /mcp address.",
        ));
    }
    Ok(code.device)
}

/// `POST /oauth/token`.
async fn token(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let Some(fields) = token_fields(&headers, &body) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send the token request as a form.",
        );
    };
    let basic = basic_client(&headers);
    let now = crate::cloud::session::now();
    let device = match redeemable(
        &service.oauth_key("code"),
        service.origin(),
        &fields,
        basic.as_deref(),
        now,
    ) {
        Ok(device) => device,
        Err(TokenRefused(code, words)) => {
            return oauth_error(StatusCode::BAD_REQUEST, code, words);
        }
    };
    match service.device_poll(&device).await {
        Ok(answer) if answer.status == 200 => {
            let Some(access) = answer.body["token"].as_str() else {
                return unavailable();
            };
            let expires = answer.body["session"]["expires_at"]
                .as_u64()
                .unwrap_or(now)
                .saturating_sub(now);
            let mut response = cors(protect(
                axum::Json(json!({
                    "access_token": access,
                    "token_type": "Bearer",
                    "expires_in": expires,
                    "scope": SCOPE,
                }))
                .into_response(),
            ));
            response
                .headers_mut()
                .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
            response
        }
        Ok(_) | Err(SessionError::InvalidRequest) => oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "This code was already used or has expired. Sign in again.",
        ),
        Err(_) => unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];
    const ORIGIN: &str = "https://openagents.com";

    fn claude() -> Client {
        registration(
            &json!({
                "client_name": "Claude",
                "redirect_uris": ["https://claude.ai/api/mcp/auth_callback"],
                "grant_types": ["authorization_code", "refresh_token"],
                "response_types": ["code"],
                "token_endpoint_auth_method": "none",
            }),
            1_791_504_000,
        )
        .unwrap()
    }

    fn verifier() -> &'static str {
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
    }

    fn query(client_id: &str, extra: &str) -> String {
        format!(
            "response_type=code&client_id={client_id}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state=xyz&resource={}{extra}",
            urlencode("https://claude.ai/api/mcp/auth_callback"),
            s256(verifier()),
            urlencode("https://openagents.com/mcp"),
        )
    }

    fn urlencode(value: &str) -> String {
        url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
    }

    #[test]
    fn the_metadata_names_this_site_pkce_and_registration() {
        let mcp = protected_resource(ORIGIN, "/mcp");
        assert_eq!(mcp["resource"], "https://openagents.com/mcp");
        assert_eq!(mcp["authorization_servers"], json!([ORIGIN]));
        assert_eq!(mcp["scopes_supported"], json!([SCOPE]));
        assert_eq!(mcp["bearer_methods_supported"], json!(["header"]));
        let root = protected_resource(ORIGIN, "");
        assert_eq!(root["resource"], ORIGIN);
        let server = authorization_server(ORIGIN);
        assert_eq!(server["issuer"], ORIGIN);
        assert_eq!(server["code_challenge_methods_supported"], json!(["S256"]));
        assert_eq!(
            server["registration_endpoint"],
            "https://openagents.com/oauth/register"
        );
        assert_eq!(
            server["agent_auth"]["skill"],
            "https://openagents.com/auth.md"
        );
        assert_eq!(
            server["agent_auth"]["register_uri"],
            server["registration_endpoint"]
        );
        assert!(
            !server["agent_auth"]["registration_methods"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            challenge(ORIGIN).to_str().unwrap(),
            "Bearer resource_metadata=\"https://openagents.com/.well-known/oauth-protected-resource/mcp\", scope=\"account\""
        );
    }

    #[test]
    fn the_keyed_mcp_server_is_told_apart_from_the_docs_server() {
        assert!(keyed_mcp("/mcp"));
        assert!(keyed_mcp("/mcp/session"));
        assert!(!keyed_mcp("/mcp/docs"));
        assert!(!keyed_mcp("/mcpx"));
        assert!(owns(SERVER_METADATA) && owns(MCP_RESOURCE_METADATA) && owns(TOKEN));
        assert!(!owns("/mcp"));
    }

    #[test]
    fn redirect_addresses_are_https_loopback_or_an_apps_own_scheme() {
        for good in [
            "https://claude.ai/api/mcp/auth_callback",
            "https://chatgpt.com/connector_platform_oauth_redirect",
            "http://127.0.0.1:33418/callback",
            "http://localhost:6274/oauth/callback",
            "http://[::1]:8080/cb",
            "cursor://anysphere.cursor-mcp/oauth/callback",
        ] {
            assert!(redirect_allowed(good), "{good}");
        }
        for bad in [
            "http://evil.example/cb",
            "https://claude.ai/cb#frag",
            "https://user:pw@claude.ai/cb",
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "not a url",
        ] {
            assert!(!redirect_allowed(bad), "{bad}");
        }
        assert!(same_redirect(
            "http://127.0.0.1:1000/cb",
            "http://127.0.0.1:5000/cb"
        ));
        assert!(!same_redirect(
            "http://127.0.0.1:1000/cb",
            "http://127.0.0.1:5000/other"
        ));
        assert!(!same_redirect(
            "https://claude.ai/cb",
            "https://claude.ai/cb2"
        ));
    }

    #[test]
    fn registration_checks_the_request_and_the_client_id_round_trips() {
        let client = claude();
        assert_eq!(client.name, "Claude");
        let id = client_id(&KEY, &client);
        assert!(id.starts_with("oac_"));
        assert_eq!(read_client(&KEY, &id), Some(client.clone()));
        assert_eq!(read_client(&[8; 32], &id), None);
        let mut changed = id.clone();
        changed.insert(6, 'A');
        assert_eq!(read_client(&KEY, &changed), None);

        assert_eq!(
            registration(&json!({"redirect_uris": []}), 0),
            Err(Refused(
                "invalid_redirect_uri",
                "Send one to five redirect_uris."
            ))
        );
        assert!(registration(&json!({"redirect_uris": ["http://evil.example/cb"]}), 0).is_err());
        assert!(
            registration(
                &json!({"redirect_uris": ["https://a.example/cb"], "grant_types": ["client_credentials"]}),
                0
            )
            .is_err()
        );
        let unnamed = registration(
            &json!({"redirect_uris": ["https://vscode.dev/redirect"]}),
            0,
        )
        .unwrap();
        assert_eq!(unnamed.name, "vscode.dev");
    }

    #[test]
    fn codes_are_sealed_and_open_only_unchanged_under_the_same_key() {
        let code = Code {
            device: "dvc_abc".into(),
            client: sha256_hex("oac_x"),
            redirect_uri: "https://claude.ai/cb".into(),
            redirect_given: true,
            challenge: s256(verifier()),
            expires_at: 10,
        };
        let sealed = seal_code(&KEY, &code).unwrap();
        assert!(sealed.starts_with("oaz_"));
        assert!(!sealed.contains("dvc_"));
        assert_eq!(open_code(&KEY, &sealed), Some(code.clone()));
        assert_eq!(open_code(&[9; 32], &sealed), None);
        let mut bytes = sealed.into_bytes();
        let last = bytes.len() - 2;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        assert_eq!(open_code(&KEY, &String::from_utf8(bytes).unwrap()), None);
    }

    #[test]
    fn pkce_s256_matches_rfc_7636_appendix_b() {
        assert_eq!(
            s256(verifier()),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert!(verifier_shape(verifier()));
        assert!(!verifier_shape("short"));
    }

    #[test]
    fn an_authorization_request_is_checked_before_anything_is_shown() {
        let id = client_id(&KEY, &claude());
        let request = authorization(&KEY, ORIGIN, &query(&id, "")).unwrap();
        assert_eq!(request.client.name, "Claude");
        assert_eq!(request.state.as_deref(), Some("xyz"));
        assert!(request.redirect_given);

        // An unknown client or redirect address never redirects.
        assert_eq!(
            authorization(&[1; 32], ORIGIN, &query(&id, "")),
            Err(Stop::Page(BAD_LINK))
        );
        let elsewhere = query(&id, "").replace(
            &urlencode("https://claude.ai/api/mcp/auth_callback"),
            &urlencode("https://evil.example/cb"),
        );
        assert_eq!(
            authorization(&KEY, ORIGIN, &elsewhere),
            Err(Stop::Page(BAD_LINK))
        );
        assert_eq!(
            authorization(&KEY, ORIGIN, &query(&id, "&state=again")),
            Err(Stop::Page(BAD_LINK))
        );

        // Without PKCE S256, the app hears why.
        let plain =
            query(&id, "").replace("code_challenge_method=S256", "code_challenge_method=plain");
        assert!(matches!(
            authorization(&KEY, ORIGIN, &plain),
            Err(Stop::Back {
                error: "invalid_request",
                ..
            })
        ));
        let other = query(&id, "").replace(
            &urlencode("https://openagents.com/mcp"),
            &urlencode("https://example.com/mcp"),
        );
        assert!(matches!(
            authorization(&KEY, ORIGIN, &other),
            Err(Stop::Back {
                error: "invalid_target",
                ..
            })
        ));
    }

    #[test]
    fn a_token_request_must_match_the_code() {
        let id = client_id(&KEY, &claude());
        let code = Code {
            device: "dvc_abc".into(),
            client: sha256_hex(&id),
            redirect_uri: "https://claude.ai/api/mcp/auth_callback".into(),
            redirect_given: true,
            challenge: s256(verifier()),
            expires_at: 1_000,
        };
        let sealed = seal_code(&KEY, &code).unwrap();
        let fields = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        };
        let good = fields(&[
            ("grant_type", "authorization_code"),
            ("code", sealed.as_str()),
            ("client_id", id.as_str()),
            ("redirect_uri", "https://claude.ai/api/mcp/auth_callback"),
            ("code_verifier", verifier()),
            ("resource", "https://openagents.com/mcp"),
        ]);
        assert_eq!(
            redeemable(&KEY, ORIGIN, &good, None, 500),
            Ok("dvc_abc".to_owned())
        );
        assert_eq!(
            redeemable(&KEY, ORIGIN, &good, None, 1_000).unwrap_err().0,
            "invalid_grant"
        );
        let mut wrong = good.clone();
        wrong.insert(
            "code_verifier".into(),
            "a-different-verifier-that-is-long-enough-to-pass".into(),
        );
        assert_eq!(
            redeemable(&KEY, ORIGIN, &wrong, None, 500).unwrap_err().0,
            "invalid_grant"
        );
        let mut other_app = good.clone();
        other_app.insert("client_id".into(), "oac_other".into());
        assert_eq!(
            redeemable(&KEY, ORIGIN, &other_app, None, 500)
                .unwrap_err()
                .0,
            "invalid_grant"
        );
        let mut basic = good.clone();
        basic.remove("client_id");
        assert_eq!(
            redeemable(&KEY, ORIGIN, &basic, Some(id.as_str()), 500),
            Ok("dvc_abc".to_owned())
        );
        let mut no_redirect = good.clone();
        no_redirect.remove("redirect_uri");
        assert!(redeemable(&KEY, ORIGIN, &no_redirect, None, 500).is_err());
        let mut grant = good;
        grant.insert("grant_type".into(), "refresh_token".into());
        assert_eq!(
            redeemable(&KEY, ORIGIN, &grant, None, 500).unwrap_err().0,
            "unsupported_grant_type"
        );
    }

    #[test]
    fn the_consent_page_names_the_app_and_where_it_goes_back_to() {
        let id = client_id(&KEY, &claude());
        let raw = query(&id, "");
        let request = authorization(&KEY, ORIGIN, &raw).unwrap();
        let html = consent_form(&request, &raw, "ticket").into_string();
        assert!(html.contains("Let Claude use your OpenAgents account?"));
        assert!(html.contains("claude.ai"));
        assert!(html.contains("name=\"decision\" value=\"approve\""));
        assert!(html.contains("name=\"decision\" value=\"deny\""));
        assert!(html.contains("name=\"csrf\" value=\"ticket\""));
        crate::copy_guard::assert_plain(AUTHORIZE, &html);
        let policy = consent_policy(&request.redirect_uri);
        assert!(policy.contains("form-action 'self' https://claude.ai;"));
        assert!(
            consent_policy("cursor://anysphere.cursor-mcp/cb")
                .contains("form-action 'self' cursor:;")
        );
    }

    #[test]
    fn the_app_hears_back_with_state_and_issuer() {
        let to = with_query(
            "https://claude.ai/cb?x=1",
            &[("code", "oaz_1"), ("state", "s t"), ("iss", ORIGIN)],
        )
        .unwrap();
        assert_eq!(
            to,
            "https://claude.ai/cb?x=1&code=oaz_1&state=s+t&iss=https%3A%2F%2Fopenagents.com"
        );
        assert_eq!(computer_label("http://127.0.0.1:9/cb"), "this computer");
        assert_eq!(computer_label("https://claude.ai/cb"), "claude.ai");
    }
}
