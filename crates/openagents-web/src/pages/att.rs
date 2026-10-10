//! `/att`: sealed inference, live (NIP-ATT, `docs/security/private-inference.md`).
//!
//! The page is `crates/att-web`'s markup and build (`scripts/build-att-web.sh`),
//! read from `--att DIR` (default `/srv/att` when it exists). The browser
//! does every check itself in WebAssembly; this server is the gateway in
//! the picture, and it only carries:
//!
//! - `GET /att/api/state`: the publisher's newest release head, the newest
//!   attested endpoint under it, the release it runs and its beacon, as the
//!   signed events the relay holds. The gateway also runs the same checks
//!   (`oa-att`) and says what it found, so a round to an endpoint that does
//!   not verify is refused here too.
//! - `POST /att/api/send`: one sealed decision request (kind `25910`,
//!   NIP-44 to the endpoint key), published to the relay unchanged. It
//!   answers with what the gateway could see: the kind, the ids and the
//!   ciphertext's size and digest.
//! - `GET /att/api/answers/{id}`: the endpoint's sealed answers to that
//!   request, as they arrive (long poll).
//!
//! Rounds are rate limited per visitor address and overall.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use maud::html;
use nostr::domain::Event;
use oa_att::net::{self, Exchanged};
use oa_att::nostr;
use oa_att::{Policy, Records, Tamper};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use secp256k1::SecretKey;
use serde_json::{Value, json};

use crate::App;
use crate::ui_page::{UiPage, action_link};

/// The page.
pub(crate) const ATT_PATH: &str = "/att";
const ATT_FILE: &str = "/att/{file}";
const STATE_PATH: &str = "/att/api/state";
const SEND_PATH: &str = "/att/api/send";
const ANSWERS_PATH: &str = "/att/api/answers/{id}";

/// The relay the endpoint listens on.
pub(crate) const RELAY: &str = "wss://relay.openagents.com";
/// The OpenAgents key that publishes the sealed Clef releases.
pub(crate) const PUBLISHER: &str =
    "77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e";

/// The build's files (`scripts/build-att-web.sh`).
const GLUE: &str = "att_web.js";
const WASM: &str = "att_web_bg.wasm";
const START: &str = "start.js";
const STYLE: &str = "att.css";

/// The page's policy: scripts, styles and the module from this site only,
/// compiling WebAssembly, and reads of this site's API only.
pub(crate) const ATT_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

const TITLE: &str = "Sealed inference, live";

/// Rounds one visitor address may start per minute and per hour, and all
/// visitors together per hour.
const PER_MINUTE: usize = 3;
const PER_HOUR: usize = 20;
const ALL_PER_HOUR: usize = 240;
/// How long fetched records are reused.
const STATE_TTL: Duration = Duration::from_secs(15);
/// How long a round's answers are kept.
const ROUND_TTL: Duration = Duration::from_secs(600);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(ATT_PATH, get(page))
        .route(ATT_FILE, get(build_file))
        .route(STATE_PATH, get(state))
        .route(SEND_PATH, post(send))
        .route(ANSWERS_PATH, get(answers))
}

fn build(app: &App) -> Option<&Path> {
    let directory = app.config.att.as_deref()?;
    [GLUE, WASM, START, STYLE]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// The page markup, from `crates/att-web/index.html`, with its files
/// under `/att/`.
fn markup() -> String {
    include_str!("../../../att-web/index.html")
        .replace("href=\"att.css\"", "href=\"/att/att.css\"")
        .replace("src=\"start.js\"", "src=\"/att/start.js\"")
}

fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="att-title" {
            (MarkdownRoot::new(html! {
                h1 id="att-title" { (TITLE) }
                p.oa-page-lead { "This demo can't run here right now." }
            }))
            div.oa-page-actions {
                (action_link("Read how sealed inference works", "/docs/privacy-and-security"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(ATT_PATH)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    if build(&app).is_none() {
        return unavailable(&headers);
    }
    let mut response = (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        markup(),
    )
        .into_response();
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(ATT_POLICY),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

async fn build_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let Some(directory) = build(&app) else {
        return crate::not_found().await;
    };
    let content_type = if file == STYLE {
        Some("text/css; charset=utf-8")
    } else {
        super::everglade::build_type(&file)
    };
    let Some(content_type) = content_type else {
        return crate::not_found().await;
    };
    super::everglade::serve_build(directory, &file, content_type, &request).await
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// One round in flight or recently done.
#[derive(Default)]
struct Round {
    accepted_ms: Option<u64>,
    events: Vec<Event>,
    done: bool,
    error: Option<String>,
}

struct Shared {
    /// The gateway's own relay key, made at start.
    secret: SecretKey,
    cache: Mutex<Option<(Instant, Records, Value)>>,
    visitors: Mutex<HashMap<String, VecDeque<Instant>>>,
    everyone: Mutex<VecDeque<Instant>>,
    rounds: Mutex<HashMap<String, (Instant, Round)>>,
    changed: tokio::sync::Notify,
}

static SHARED: LazyLock<Arc<Shared>> = LazyLock::new(|| {
    Arc::new(Shared {
        secret: SecretKey::new(&mut secp256k1::rand::rng()),
        cache: Mutex::new(None),
        visitors: Mutex::new(HashMap::new()),
        everyone: Mutex::new(VecDeque::new()),
        rounds: Mutex::new(HashMap::new()),
        changed: tokio::sync::Notify::new(),
    })
});

fn json_response(status: StatusCode, body: &Value) -> Response {
    let mut response = (status, axum::Json(body.clone())).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn refuse(status: StatusCode, message: &str) -> Response {
    json_response(status, &json!({"error": message}))
}

fn policy() -> Policy {
    Policy {
        publisher: PUBLISHER.into(),
        workload: oa_att::WORKLOAD.into(),
        required: nostr::att::Level::TeeCloud,
        seen_generation: None,
    }
}

/// The gateway's own check of the records, with every step's verdict.
fn gateway_check(records: &Records, at: u64) -> Value {
    let policy = policy();
    let parsed = match oa_att::parse(records, &policy, at) {
        Ok(parsed) => parsed,
        Err(why) => return json!({"ok": false, "step": "fetch", "reason": why.0}),
    };
    let claims = match oa_att::chain(&parsed, at) {
        Ok(claims) => claims,
        Err(why) => return json!({"ok": false, "step": "chain", "reason": why.0}),
    };
    let measured = match oa_att::measure(&parsed, &claims, at, Tamper::None) {
        Ok(measured) => measured,
        Err(why) => return json!({"ok": false, "step": "measure", "reason": why.0}),
    };
    match oa_att::bind(&parsed, &claims, &policy, None) {
        Ok(bound) => json!({
            "ok": true,
            "level": bound.level.as_str(),
            "measurement": measured.reported,
            "endpoint": parsed.endpoint.address(),
        }),
        Err(why) => json!({"ok": false, "step": "bind", "reason": why.0}),
    }
}

/// The records, fetched or reused, and the gateway's check of them.
async fn records(shared: &Shared) -> Result<(Records, Value, u64, bool), String> {
    if let Ok(held) = shared.cache.lock()
        && let Some((at, records, check)) = held.as_ref()
        && at.elapsed() < STATE_TTL
    {
        return Ok((records.clone(), check.clone(), 0, true));
    }
    let fetched = net::fetch(RELAY, &shared.secret, PUBLISHER, oa_att::WORKLOAD, now()).await?;
    let check = gateway_check(&fetched.records, now());
    if let Ok(mut held) = shared.cache.lock() {
        *held = Some((Instant::now(), fetched.records.clone(), check.clone()));
    }
    Ok((fetched.records, check, fetched.ms, false))
}

async fn state() -> Response {
    let shared = Arc::clone(&SHARED);
    match records(&shared).await {
        Ok((records, check, fetched_ms, cached)) => json_response(
            StatusCode::OK,
            &json!({
                "relay": RELAY,
                "publisher": PUBLISHER,
                "workload": oa_att::WORKLOAD,
                "fetched_ms": fetched_ms,
                "cached": cached,
                "events": {
                    "release": records.release,
                    "head": records.head,
                    "endpoint": records.endpoint,
                    "beacon": records.beacon,
                },
                "gateway": check,
                "now": now(),
            }),
        ),
        Err(why) => json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &json!({"error": format!("The sealed machine is not reachable right now: {why}")}),
        ),
    }
}

fn visitor(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map_or_else(|| "local".to_string(), |v| v.trim().to_string())
}

/// Take one round from the visitor's and everyone's allowance.
fn admit(shared: &Shared, who: &str) -> Result<(), String> {
    let now = Instant::now();
    let mut everyone = shared.everyone.lock().map_err(|_| "busy")?;
    while everyone
        .front()
        .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(3_600))
    {
        everyone.pop_front();
    }
    if everyone.len() >= ALL_PER_HOUR {
        return Err("The demo has run many rounds this hour. Try again later.".into());
    }
    let mut visitors = shared.visitors.lock().map_err(|_| "busy")?;
    visitors.retain(|_, times| {
        times
            .back()
            .is_some_and(|t| now.duration_since(*t) < Duration::from_secs(3_600))
    });
    let times = visitors.entry(who.to_string()).or_default();
    while times
        .front()
        .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(3_600))
    {
        times.pop_front();
    }
    let last_minute = times
        .iter()
        .filter(|t| now.duration_since(**t) < Duration::from_secs(60))
        .count();
    if last_minute >= PER_MINUTE || times.len() >= PER_HOUR {
        return Err("You've run several rounds just now. Wait a minute and try again.".into());
    }
    times.push_back(now);
    everyone.push_back(now);
    Ok(())
}

async fn send(headers: HeaderMap, body: Bytes) -> Response {
    let shared = Arc::clone(&SHARED);
    if body.len() > 32 * 1024 {
        return refuse(StatusCode::PAYLOAD_TOO_LARGE, "The request is too large.");
    }
    let Ok(event) = serde_json::from_slice::<Event>(&body) else {
        return refuse(StatusCode::BAD_REQUEST, "That is not a signed event.");
    };
    if event.kind != nostr::decision::REQUEST_KIND
        || event.validate_structure().is_err()
        || event.validate_crypto().is_err()
    {
        return refuse(
            StatusCode::BAD_REQUEST,
            "That is not a signed decision request.",
        );
    }
    if event.content.len() > 16 * 1024 || event.created_at.abs_diff(now()) > 120 {
        return refuse(
            StatusCode::BAD_REQUEST,
            "The request is too large or too old.",
        );
    }
    let (records, check, _, _) = match records(&shared).await {
        Ok(found) => found,
        Err(why) => {
            return refuse(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!("The sealed machine is not reachable right now: {why}"),
            );
        }
    };
    if check["ok"] != true {
        return refuse(
            StatusCode::CONFLICT,
            "The gateway could not verify the sealed machine, so it sends nothing to it.",
        );
    }
    let endpoint_key = records.endpoint.pubkey.clone();
    let tagged: Vec<&str> = event.tag_values("p").collect();
    if tagged != [endpoint_key.as_str()] {
        return refuse(
            StatusCode::BAD_REQUEST,
            "The request is not addressed to the verified sealed machine.",
        );
    }
    if let Err(why) = admit(&shared, &visitor(&headers)) {
        return refuse(StatusCode::TOO_MANY_REQUESTS, &why);
    }
    let id = event.id.clone();
    {
        let Ok(mut rounds) = shared.rounds.lock() else {
            return refuse(StatusCode::SERVICE_UNAVAILABLE, "busy");
        };
        rounds.retain(|_, (at, _)| at.elapsed() < ROUND_TTL);
        rounds.insert(id.clone(), (Instant::now(), Round::default()));
    }
    let worker = Arc::clone(&shared);
    let request = event.clone();
    tokio::spawn(async move {
        let key = request.id.clone();
        let sink = Arc::clone(&worker);
        let result = net::exchange(
            RELAY,
            &worker.secret,
            &request,
            Duration::from_secs(105),
            move |what| {
                if let Ok(mut rounds) = sink.rounds.lock()
                    && let Some((_, round)) = rounds.get_mut(&key)
                {
                    match what {
                        Exchanged::Accepted(ms) => round.accepted_ms = Some(ms),
                        Exchanged::Answer(event) => round.events.push(event),
                    }
                }
                sink.changed.notify_waiters();
            },
        )
        .await;
        if let Ok(mut rounds) = worker.rounds.lock()
            && let Some((_, round)) = rounds.get_mut(&request.id)
        {
            round.done = true;
            if let Err(why) = result {
                round.error = Some(why);
            }
        }
        worker.changed.notify_waiters();
    });
    // Wait for the relay's acknowledgement, briefly.
    let started = Instant::now();
    let accepted = loop {
        let (accepted, error) = shared
            .rounds
            .lock()
            .ok()
            .and_then(|rounds| {
                rounds
                    .get(&id)
                    .map(|(_, r)| (r.accepted_ms, r.error.clone()))
            })
            .unwrap_or((None, None));
        if accepted.is_some() || error.is_some() || started.elapsed() > Duration::from_secs(15) {
            break (accepted, error);
        }
        let _ = tokio::time::timeout(Duration::from_millis(250), shared.changed.notified()).await;
    };
    if let (None, Some(error)) = &accepted {
        return refuse(
            StatusCode::BAD_GATEWAY,
            &format!("The relay refused the request: {error}"),
        );
    }
    json_response(
        StatusCode::OK,
        &json!({
            "request": id,
            "relay": RELAY,
            "accepted_ms": accepted.0,
            "gateway_ms": ms(started),
            "saw": {
                "kind": event.kind,
                "id": event.id,
                "from": event.pubkey,
                "to": endpoint_key,
                "ciphertext_bytes": event.content.len(),
                "ciphertext_sha256": nostr::att::sha256_hex(event.content.as_bytes()),
                "ciphertext_head": event.content.chars().take(48).collect::<String>(),
            },
        }),
    )
}

#[derive(serde::Deserialize)]
struct Have {
    #[serde(default)]
    have: usize,
}

async fn answers(UrlPath(id): UrlPath<String>, Query(have): Query<Have>) -> Response {
    let shared = Arc::clone(&SHARED);
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        let snapshot = shared.rounds.lock().ok().and_then(|rounds| {
            rounds
                .get(&id)
                .map(|(_, r)| (r.events.clone(), r.done, r.accepted_ms, r.error.clone()))
        });
        let Some((events, done, accepted_ms, error)) = snapshot else {
            return refuse(StatusCode::NOT_FOUND, "No such round.");
        };
        if events.len() > have.have || done || Instant::now() >= deadline {
            return json_response(
                StatusCode::OK,
                &json!({
                    "events": events,
                    "done": done,
                    "accepted_ms": accepted_ms,
                    "error": error,
                }),
            );
        }
        let _ = tokio::time::timeout(Duration::from_millis(500), shared.changed.notified()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_is_strict() {
        assert!(ATT_POLICY.starts_with("default-src 'none'"));
        assert!(ATT_POLICY.contains("connect-src 'self';"));
        assert!(!ATT_POLICY.contains("unsafe-inline"));
        assert!(!ATT_POLICY.contains("'unsafe-eval'"));
    }

    #[test]
    fn the_markup_runs_no_inline_script_or_style() {
        let page = markup();
        assert!(page.contains("<script type=\"module\" src=\"/att/start.js\"></script>"));
        assert!(page.contains("href=\"/att/att.css\""));
        assert_eq!(page.matches("<script").count(), 1);
        assert!(!page.contains("style=\""));
        assert!(!page.contains(" onclick="));
        for id in [
            "att-canvas",
            "att-run",
            "att-steps",
            "att-panels",
            "att-verdict",
        ] {
            assert!(page.contains(&format!("id=\"{id}\"")), "{id}");
        }
    }

    #[test]
    fn a_visitor_is_rate_limited() {
        let shared = Shared {
            secret: SecretKey::new(&mut secp256k1::rand::rng()),
            cache: Mutex::new(None),
            visitors: Mutex::new(HashMap::new()),
            everyone: Mutex::new(VecDeque::new()),
            rounds: Mutex::new(HashMap::new()),
            changed: tokio::sync::Notify::new(),
        };
        for _ in 0..PER_MINUTE {
            admit(&shared, "203.0.113.7").unwrap();
        }
        assert!(admit(&shared, "203.0.113.7").is_err());
        assert!(admit(&shared, "203.0.113.8").is_ok());
    }
}
