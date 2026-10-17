//! Signing a browser in with a Nostr key (#12001): by scanning a code with
//! the OpenAgents phone app, or with a browser extension (NIP-07). Both
//! end in the same browser session cookie GitHub sign-in sets, so
//! `/device` (Coder 1.0's approval page), `/oauth/authorize`, Settings and
//! billing work for an account that has only a key.
//!
//! The account service checks every signature and issues every session
//! (`oa_auth::nostr`, #11993). This server only remembers which browser
//! asked to be signed in, for a few minutes, in memory.
//!
//! **Scan with your phone.**
//!
//! 1. `/login` runs `sign-in.js`, which sends `POST /login/key` with
//!    `{"return_to"}`. The answer is `{request, link, qr, expires_at,
//!    interval}`, and an HttpOnly cookie ties the request to this browser.
//!    The request holds a fresh challenge from the account service and
//!    stands as long as the challenge does (five minutes).
//! 2. The page shows `qr` (`GET /login/key/{request}/qr.svg`), the QR code
//!    of `link`: `{origin}/login/scan#request={request}&challenge={challenge}`.
//! 3. The phone may read `GET /v1/web-sign-in/{request}` (`{request,
//!    browser, origin, expires_at}`) to say which browser it is signing in,
//!    then sends `POST /v1/web-sign-in/approve` with the body `{"request"}`
//!    and `Authorization: Nostr <base64 event>`: NIP-98, kind 27235, `u`
//!    the exact URL it calls (on the API host or this site), `method`
//!    `POST`, a `challenge` tag holding the scanned challenge, and a
//!    `payload` tag holding the SHA-256 of the exact body. The answer is
//!    `{"approved": true, "browser", "created"}`, or the account service's
//!    refusal. These two are API routes, so they answer on
//!    `api.openagents.com/v1/` too.
//! 4. The page polls `GET /login/key/{request}/status` every `interval`
//!    seconds. Once approved, that answer sets the session cookie and says
//!    `{"status": "signed_in", "return_to"}`, once.
//!
//! **Browser extension.** `sign-in.js` gets a challenge from
//! `GET /v1/device/nostr/challenge`, asks `window.nostr.signEvent` for the
//! same NIP-98 event over `POST {origin}/login/key/extension` with the body
//! `{"return_to"}`, and sends it there. The answer sets the session cookie
//! and says `{"status": "signed_in", "return_to"}`.
//!
//! The browser routes stay off `/v1/`: the API host drops cookies.
//!
//! Refusals are `{"error": {"code", "message"}}`.

use std::collections::HashMap;
use std::sync::Mutex;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, SessionError, now};

/// Where the browser starts a scan sign-in.
pub(crate) const START: &str = "/login/key";
/// Where the phone approves one.
pub(crate) const APPROVE: &str = "/v1/web-sign-in/approve";
/// Where a browser extension signs its own browser in.
pub(crate) const EXTENSION: &str = "/login/key/extension";
/// The page script.
pub(crate) const SCRIPT: &str = "/login/sign-in.js";
/// The page a scanned link opens when the phone has no app to open it.
pub(crate) const SCAN: &str = "/login/scan";
/// The cookie that ties a waiting sign-in to its browser.
const COOKIE: &str = "oa_web_sign_in";
/// How often the page asks whether the phone approved, in seconds.
const INTERVAL: u64 = 2;
/// The most sign-ins waiting at once; the oldest give way.
const WAITING_MAX: usize = 10_000;
/// The longest body or `Authorization` header accepted.
const BODY_MAX: usize = 4 * 1024;
const HEADER_MAX: usize = 8 * 1024;

/// Whether `path` is one of these routes, which this site answers itself
/// and never forwards upstream.
pub(crate) fn owns(path: &str) -> bool {
    path.starts_with("/login/") || path.starts_with("/v1/web-sign-in/")
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(START, post(start))
        .route(APPROVE, post(approve))
        .route(EXTENSION, post(extension))
        .route("/v1/web-sign-in/{request}", get(details))
        .route("/login/key/{request}/status", get(status))
        .route("/login/key/{request}/qr.svg", get(qr))
        .route(SCRIPT, get(script))
        .route(SCAN, get(scan_page))
}

/// The sign-ins browsers are waiting on, in memory.
#[derive(Default)]
pub(crate) struct Waiting(Mutex<HashMap<String, Request>>);

struct Request {
    /// SHA-256 of the browser's secret.
    secret: [u8; 32],
    challenge: String,
    link: String,
    expires_at: u64,
    /// The browser, in a few words, for the phone to show.
    browser: String,
    return_to: String,
    approved: Option<Approved>,
}

struct Approved {
    token: String,
    created: bool,
}

impl Waiting {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Request>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn insert(&self, id: String, request: Request) {
        let mut map = self.lock();
        let at = now();
        map.retain(|_, r| r.expires_at > at);
        while map.len() >= WAITING_MAX {
            let Some(oldest) = map
                .iter()
                .min_by_key(|(_, r)| r.expires_at)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            map.remove(&oldest);
        }
        map.insert(id, request);
    }
}

fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    protect(
        (
            status,
            axum::Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response(),
    )
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "sign_in_unavailable",
        "Login isn't available right now. Try again in a minute.",
    )
}

fn expired() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "expired",
        "This code expired. Show a new one and scan again.",
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn service(app: &App) -> Result<&CloudSession, Response> {
    app.config.cloud.as_deref().ok_or_else(unavailable)
}

/// A request from this site's own page: its `Origin` is ours, or the
/// browser says it is same-origin. A sign-in that sets a cookie must not
/// start from another site.
fn same_origin(service: &CloudSession, headers: &HeaderMap) -> bool {
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(origin) => origin == service.origin(),
        None => headers
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|site| site == "same-origin"),
    }
}

fn cross_site() -> Response {
    refused(
        StatusCode::FORBIDDEN,
        "cross_site",
        "Start the login on this site.",
    )
}

fn json_body(body: &[u8]) -> Value {
    serde_json::from_slice::<Value>(body)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn return_to(body: &Value) -> String {
    oa_auth::return_to(body.get("return_to").and_then(Value::as_str))
}

/// The browser and system a `User-Agent` names, in a few words.
fn browser_name(headers: &HeaderMap) -> String {
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let browser = if agent.contains("Firefox/") {
        "Firefox"
    } else if agent.contains("Edg/") {
        "Edge"
    } else if agent.contains("Chrome/") {
        "Chrome"
    } else if agent.contains("Safari/") {
        "Safari"
    } else {
        return "A web browser".into();
    };
    let system = if agent.contains("iPhone") || agent.contains("iPad") {
        "iOS"
    } else if agent.contains("Android") {
        "Android"
    } else if agent.contains("Mac OS X") || agent.contains("Macintosh") {
        "macOS"
    } else if agent.contains("Windows") {
        "Windows"
    } else if agent.contains("Linux") {
        "Linux"
    } else {
        return browser.into();
    };
    format!("{browser} on {system}")
}

/// The `challenge` tag of the event in a `Nostr <base64 event>` value.
fn event_challenge(authorization: &str) -> Option<String> {
    let encoded = authorization.strip_prefix("Nostr ")?.trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(encoded))
        .ok()?;
    let event: Value = serde_json::from_slice(&bytes).ok()?;
    event["tags"]
        .as_array()?
        .iter()
        .filter_map(Value::as_array)
        .find(|tag| tag.first().and_then(Value::as_str) == Some("challenge"))
        .and_then(|tag| tag.get(1))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn signed(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("Nostr ") && v.len() <= HEADER_MAX)
}

fn unsigned() -> Response {
    refused(
        StatusCode::UNAUTHORIZED,
        "unauthenticated",
        "Sign the request with your key.",
    )
}

/// Forward a signed sign-in to the account service, byte for byte.
/// `Ok((token, created))` once it issued a session; `Err` the response to
/// answer.
async fn forward(
    service: &CloudSession,
    authorization: &str,
    body: &[u8],
) -> Result<(String, bool), Response> {
    let answer = service
        .nostr_call(
            reqwest::Method::POST,
            "/v1/sessions/nostr",
            Some(authorization),
            Some(body.to_vec()),
        )
        .await
        .map_err(|_| unavailable())?;
    if answer.status != 200 {
        let status = StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY);
        return Err(protect((status, axum::Json(answer.body)).into_response()));
    }
    let token = answer.body["token"]
        .as_str()
        .filter(|t| crate::cloud::session::session_token(t))
        .ok_or_else(unavailable)?;
    Ok((token.to_string(), answer.body["created"] == true))
}

/// The cookies that sign this browser in with `token`, as GitHub sign-in's
/// end does: the session, the account's own workspace, and the chats this
/// browser made signed out.
async fn session_cookies(
    app: &App,
    service: &CloudSession,
    headers: &HeaderMap,
    token: &str,
    created: bool,
) -> Result<Vec<HeaderValue>, SessionError> {
    let grant = service.web_grant(token, created).await?;
    let mut cookies = grant.cookies()?;
    cookies.extend(crate::cloud::default_workspace_cookies(
        service,
        &grant.viewer,
    )?);
    crate::chat_owner::claim(app, headers, &grant.viewer.account_id).await;
    Ok(cookies)
}

fn signed_in(cookies: Vec<HeaderValue>, return_to: &str) -> Response {
    let mut response =
        protect(axum::Json(json!({"status": "signed_in", "return_to": return_to})).into_response());
    for cookie in cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// `POST /v1/web-sign-in` `{"return_to"}`: a new sign-in for this browser.
async fn start(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let service = match service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    if !same_origin(service, &headers) {
        return cross_site();
    }
    if body.len() > BODY_MAX {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid_request",
            "That request is too large.",
        );
    }
    let back = return_to(&json_body(&body));
    let Ok(answer) = service
        .nostr_call(
            reqwest::Method::GET,
            "/v1/sessions/nostr/challenge",
            None,
            None,
        )
        .await
    else {
        return unavailable();
    };
    let challenge = answer.body["challenge"].as_str().unwrap_or_default();
    let at = now();
    let expires_at = answer.body["expires_at"].as_u64().unwrap_or(0);
    if answer.status != 200
        || challenge.is_empty()
        || challenge.len() > 256
        || !challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.')
        || expires_at <= at
    {
        return unavailable();
    }
    let id = hex(&secp256k1::rand::random::<[u8; 16]>());
    let secret = hex(&secp256k1::rand::random::<[u8; 32]>());
    let link = format!(
        "{}{SCAN}#request={id}&challenge={challenge}",
        service.origin()
    );
    let lifetime = expires_at - at;
    let Ok(cookie) =
        service.web_sign_in_cookie(COOKIE, &format!("{id}.{secret}"), lifetime, "/login/key")
    else {
        return unavailable();
    };
    app.config.web_sign_in.insert(
        id.clone(),
        Request {
            secret: Sha256::digest(secret.as_bytes()).into(),
            challenge: challenge.to_string(),
            link: link.clone(),
            expires_at,
            browser: browser_name(&headers),
            return_to: back,
            approved: None,
        },
    );
    let mut response = protect(
        axum::Json(json!({
            "request": id,
            "link": link,
            "qr": format!("{START}/{id}/qr.svg"),
            "expires_at": expires_at,
            "interval": INTERVAL,
        }))
        .into_response(),
    );
    response.headers_mut().append(header::SET_COOKIE, cookie);
    response
}

/// `GET /v1/web-sign-in/{request}`: what the phone shows before it signs.
async fn details(State(app): State<App>, Path(id): Path<String>) -> Response {
    let Ok(service) = service(&app) else {
        return unavailable();
    };
    let map = app.config.web_sign_in.lock();
    match map
        .get(&id)
        .filter(|r| valid_id(&id) && r.expires_at > now() && r.approved.is_none())
    {
        Some(request) => protect(
            axum::Json(json!({
                "request": id,
                "browser": request.browser,
                "origin": service.origin(),
                "expires_at": request.expires_at,
            }))
            .into_response(),
        ),
        None => expired(),
    }
}

/// `GET /v1/web-sign-in/{request}/qr.svg`: the code the phone scans.
async fn qr(State(app): State<App>, Path(id): Path<String>) -> Response {
    let link = {
        let map = app.config.web_sign_in.lock();
        match map.get(&id).filter(|r| r.expires_at > now()) {
            Some(request) => request.link.clone(),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    };
    let Ok(code) = qrcodegen::QrCode::encode_text(&link, qrcodegen::QrCodeEcc::Medium) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut path = String::new();
    for y in 0..code.size() {
        for x in 0..code.size() {
            if code.get_module(x, y) {
                use std::fmt::Write as _;
                let _ = write!(path, "M{},{}h1v1h-1z", x + 4, y + 4);
            }
        }
    }
    let size = code.size() + 8;
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {size} {size}\" shape-rendering=\"crispEdges\" role=\"img\" aria-label=\"Login code\"><path fill=\"#fff\" d=\"M0,0h{size}v{size}H0z\"/><path fill=\"#000\" d=\"{path}\"/></svg>"
    );
    protect(([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response())
}

/// This browser's waiting sign-in, from its cookie: the request id when
/// the cookie's secret matches `id`'s.
fn started_here(app: &App, headers: &HeaderMap, id: &str) -> bool {
    let Some((cookie_id, secret)) = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(name, _)| *name == COOKIE)
        .find_map(|(_, value)| value.split_once('.'))
    else {
        return false;
    };
    if cookie_id != id {
        return false;
    }
    let digest: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
    app.config
        .web_sign_in
        .lock()
        .get(id)
        .is_some_and(|r| r.secret == digest)
}

/// `GET /v1/web-sign-in/{request}/status`: whether the phone approved, for
/// the browser that started it. Approved, this answer signs it in.
async fn status(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let service = match service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    if !valid_id(&id) || !started_here(&app, &headers, &id) {
        return expired();
    }
    let (approved, back) = {
        let mut map = app.config.web_sign_in.lock();
        let Some(request) = map.get(&id) else {
            return expired();
        };
        if request.approved.is_none() {
            if request.expires_at <= now() {
                map.remove(&id);
                return expired();
            }
            return protect(axum::Json(json!({"status": "waiting"})).into_response());
        }
        // Handed over once.
        let request = map.remove(&id).expect("present");
        (request.approved.expect("approved"), request.return_to)
    };
    match session_cookies(&app, service, &headers, &approved.token, approved.created).await {
        Ok(mut cookies) => {
            if let Ok(clear) = service.web_sign_in_cookie(COOKIE, "", 0, "/login/key") {
                cookies.push(clear);
            }
            signed_in(cookies, &back)
        }
        Err(_) => unavailable(),
    }
}

/// `POST /v1/web-sign-in/approve` `{"request"}`, signed by the phone.
async fn approve(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let service = match service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let Some(authorization) = signed(&headers) else {
        return unsigned();
    };
    if body.len() > BODY_MAX {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid_request",
            "That request is too large.",
        );
    }
    let fields = json_body(&body);
    let Some(id) = fields
        .get("request")
        .and_then(Value::as_str)
        .filter(|id| valid_id(id))
        .map(str::to_string)
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send the `request` from the code you scanned.",
        );
    };
    let browser = {
        let map = app.config.web_sign_in.lock();
        let Some(request) = map
            .get(&id)
            .filter(|r| r.expires_at > now() && r.approved.is_none())
        else {
            return expired();
        };
        // The phone signed this browser's own challenge, not another's.
        if event_challenge(authorization).as_deref() != Some(request.challenge.as_str()) {
            return refused(
                StatusCode::UNAUTHORIZED,
                "denied",
                "That code is for a different login. Scan the code again.",
            );
        }
        request.browser.clone()
    };
    let (token, created) = match forward(service, authorization, &body).await {
        Ok(issued) => issued,
        Err(response) => return response,
    };
    let unused = {
        let mut map = app.config.web_sign_in.lock();
        match map.get_mut(&id).filter(|r| r.approved.is_none()) {
            Some(request) => {
                request.approved = Some(Approved {
                    token: token.clone(),
                    created,
                });
                false
            }
            None => true,
        }
    };
    if unused {
        // The browser stopped waiting: the session nobody will use ends.
        service.end_web_session(&token).await;
        return expired();
    }
    protect(
        axum::Json(json!({"approved": true, "browser": browser, "created": created}))
            .into_response(),
    )
}

/// `POST /v1/web-sign-in/extension` `{"return_to"}`, signed by the
/// browser's extension: signs this browser in.
async fn extension(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let service = match service(&app) {
        Ok(service) => service,
        Err(response) => return response,
    };
    if !same_origin(service, &headers) {
        return cross_site();
    }
    let Some(authorization) = signed(&headers) else {
        return unsigned();
    };
    if body.len() > BODY_MAX {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid_request",
            "That request is too large.",
        );
    }
    let back = return_to(&json_body(&body));
    let (token, created) = match forward(service, authorization, &body).await {
        Ok(issued) => issued,
        Err(response) => return response,
    };
    match session_cookies(&app, service, &headers, &token, created).await {
        Ok(cookies) => signed_in(cookies, &back),
        Err(_) => unavailable(),
    }
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../static/sign-in.js"),
    )
        .into_response()
}

/// `/login/scan`: where a scanned code lands when the phone's camera opens
/// it without the app.
async fn scan_page(headers: HeaderMap) -> Response {
    use crate::ui_page::{UiPage, action_link, auth_card};
    use maud::html;
    let card = auth_card(
        "Open the OpenAgents app",
        Some(html! { "Scan this code from the OpenAgents app to log in on your computer." }),
        html! {
            div.oa-auth-login-form data-slot="login-form" {
                (action_link("Home", "/"))
            }
        },
    );
    protect(
        UiPage::new("Log in with your phone")
            .path(SCAN)
            .account(crate::account::Account::Unknown)
            .scriptless()
            .bare()
            .content(card)
            .respond(&headers),
    )
}

/// The `/login` part for signing in with a key: the code to scan, and the
/// extension button, both filled in by [`SCRIPT`]. `return_to` is where a
/// finished sign-in goes.
pub(crate) fn sign_in_block(return_to: &str) -> maud::Markup {
    use openagents_ui::actions::{Button, ButtonVariant, ControlSize};
    maud::html! {
        div.oa-web-sign-in data-oa-web-sign-in="" data-return-to=(return_to) {
            p.oa-auth-switch { "Scan with the OpenAgents app on your phone." }
            div.oa-web-sign-in-code data-oa-web-sign-in-code="" {
                img data-oa-web-sign-in-qr="" alt="Login code" width="200" height="200" hidden;
            }
            p.oa-auth-switch data-oa-web-sign-in-note="" aria-live="polite" { "Loading your code." }
            div data-oa-web-sign-in-extension="" hidden {
                (Button::new("Log in with your browser extension")
                    .variant(ButtonVariant::Outline)
                    .size(ControlSize::Lg)
                    .block(true)
                    .attr("data-oa-web-sign-in-nip07", ""))
            }
        }
        script src=(SCRIPT) defer {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_browser_is_named_in_a_few_words() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Safari/537.36",
            ),
        );
        assert_eq!(browser_name(&headers), "Chrome on macOS");
        assert_eq!(browser_name(&HeaderMap::new()), "A web browser");
    }

    #[test]
    fn the_challenge_is_read_from_the_signed_event() {
        let event = json!({"kind": 27235, "tags": [["u", "x"], ["challenge", "v1.1.ab.cd"]]});
        let value = format!(
            "Nostr {}",
            base64::engine::general_purpose::STANDARD.encode(event.to_string())
        );
        assert_eq!(event_challenge(&value).as_deref(), Some("v1.1.ab.cd"));
        assert_eq!(event_challenge("Bearer x"), None);
    }
}
