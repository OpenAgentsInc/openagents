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
const RELEASE_PATH: &str = "/att/release.json";
const STATE_PATH: &str = "/att/api/state";
const SEND_PATH: &str = "/att/api/send";
const ANSWERS_PATH: &str = "/att/api/answers/{id}";
const LANES_PATH: &str = "/att/api/lanes";
const WAKE_PATH: &str = "/att/api/wake";

/// The relay the endpoint listens on.
pub(crate) const RELAY: &str = "wss://relay.openagents.com";
/// The OpenAgents key that publishes the sealed Clef releases.
pub(crate) const PUBLISHER: &str =
    "77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e";

/// The Pylon behind the open lane: CoderOS-4080's demo decision pylon
/// (`~/work/pylon-att-open` there), Clef M2 on CUDA. Not a TEE.
pub(crate) const OPEN_PYLON: &str =
    "81bb2b3588e8741b976a410cf52fe5df58b540475fd40cde7b96f14f80b9eba4";
const OPEN_SLUG: &str = "coderos-4080-att-open";
/// The weights every lane serves: Clef-Flash Q4_K_M.
const CLEF_WEIGHTS: &str =
    "sha256:fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c";
/// The sealed GPU machine the wake action starts (a stopped spot VM).
const GPU_VM: &str = "oa-att-h100-1";
const GPU_ZONE: &str = "us-central1-a";
const PROJECT: &str = "openagentsgemini";
/// Wakes all visitors may ask for per hour.
const WAKES_PER_HOUR: usize = 6;

/// A way to get a decision answered, with its trust level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Lane {
    /// Clef on CUDA on an H100 in Intel TDX + NVIDIA confidential computing.
    Gpu,
    /// Clef on the CPU in Intel TDX.
    Cpu,
    /// Clef on CUDA on an ordinary Pylon; sealed in transit only.
    Open,
}

impl Lane {
    const ALL: [Self; 3] = [Self::Gpu, Self::Cpu, Self::Open];

    fn parse(word: Option<&str>) -> Self {
        match word {
            Some("gpu") => Self::Gpu,
            Some("open") => Self::Open,
            _ => Self::Cpu,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
            Self::Open => "open",
        }
    }

    /// The NIP-ATT workload of an attested lane.
    fn workload(self) -> Option<&'static str> {
        match self {
            Self::Gpu => Some("clef-decisions-gpu"),
            Self::Cpu => Some(oa_att::WORKLOAD),
            Self::Open => None,
        }
    }

    fn vm(self) -> Option<(String, String)> {
        let env = |name: &str, default: &str| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.to_string())
        };
        (self == Self::Gpu).then(|| (env("ATT_GPU_VM", GPU_VM), env("ATT_GPU_ZONE", GPU_ZONE)))
    }

    /// What the page shows for the lane: its name, level, measured time
    /// per answer, cost, and who can see what.
    fn info(self) -> Value {
        match self {
            Self::Gpu => json!({
                "id": "gpu",
                "name": "Sealed GPU",
                "level": "tee-cloud",
                "level_words": "Sealed hardware: Intel TDX with an NVIDIA H100 in confidential-computing mode, in Google Confidential Space",
                "engine": "Psionic Clef on CUDA, H100",
                "seconds": LANE_SECONDS[0],
                "cost_per_hour": LANE_COST[0],
                "on_demand": true,
                "sees": [
                    ["Your browser", "Your question and the answer"],
                    ["OpenAgents relay and gateway", "Size, timing, sealed bytes"],
                    ["Google, Intel, NVIDIA", "That a sealed machine and GPU ran, not your data"],
                    ["Sealed program", "Your question, inside the sealed machine and GPU only"],
                ],
            }),
            Self::Cpu => json!({
                "id": "cpu",
                "name": "Sealed CPU",
                "level": "tee-cloud",
                "level_words": "Sealed hardware: Intel TDX in Google Confidential Space",
                "engine": "Psionic Clef on the CPU",
                "seconds": LANE_SECONDS[1],
                "cost_per_hour": LANE_COST[1],
                "on_demand": false,
                "sees": [
                    ["Your browser", "Your question and the answer"],
                    ["OpenAgents relay and gateway", "Size, timing, sealed bytes"],
                    ["Google / Intel", "That a sealed machine ran, not your data"],
                    ["Sealed program", "Your question, inside the sealed machine only"],
                ],
            }),
            Self::Open => json!({
                "id": "open",
                "name": "Fast GPU, not sealed",
                "level": "open",
                "level_words": "Not sealed hardware: an OpenAgents Pylon (an RTX 4080 at our office). Its owner could read your question",
                "engine": "Psionic Clef on CUDA, RTX 4080",
                "seconds": LANE_SECONDS[2],
                "cost_per_hour": LANE_COST[2],
                "on_demand": false,
                "sees": [
                    ["Your browser", "Your question and the answer"],
                    ["OpenAgents relay and gateway", "Size, timing, sealed bytes"],
                    ["The Pylon's owner", "Your question and the answer: the machine is not sealed"],
                    ["Anyone else on the relay", "Size, timing, sealed bytes"],
                ],
            }),
        }
    }
}

/// Measured seconds per answer (gpu, cpu, open), browser round included.
const LANE_SECONDS: [f64; 3] = [0.0, 27.0, 1.5];
/// Dollars per hour while the lane's machine runs (gpu, cpu, open).
const LANE_COST: [f64; 3] = [0.0, 0.40, 0.0];

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
        .route(RELEASE_PATH, get(release))
        .route(ATT_FILE, get(build_file))
        .route(STATE_PATH, get(state))
        .route(SEND_PATH, post(send))
        .route(ANSWERS_PATH, get(answers))
        .route(LANES_PATH, get(lanes))
        .route(WAKE_PATH, post(wake))
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
        .replace(
            "<link rel=\"stylesheet\" href=\"/static/ui.css\">",
            &crate::theme::style_tag(),
        )
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

/// `GET /att/release.json`: the page's code by digest (SHA-256, and the
/// SHA-384 subresource-integrity form), so anyone can compare what this
/// page runs with a rebuild of `crates/att-web`
/// (`scripts/build-att-web.sh`) at the named commit.
async fn release(State(app): State<App>) -> Response {
    use base64::Engine as _;
    use sha2::{Digest, Sha256, Sha384};
    let Some(directory) = build(&app) else {
        return crate::not_found().await;
    };
    let mut files = Vec::new();
    for name in [START, GLUE, WASM, STYLE] {
        let Ok(bytes) = tokio::fs::read(directory.join(name)).await else {
            continue;
        };
        files.push(json!({
            "path": format!("{ATT_PATH}/{name}"),
            "bytes": bytes.len(),
            "sha256": format!("{:x}", Sha256::digest(&bytes)),
            "integrity": format!(
                "sha384-{}",
                base64::engine::general_purpose::STANDARD.encode(Sha384::digest(&bytes))
            ),
        }));
    }
    let page = markup();
    json_response(
        StatusCode::OK,
        &json!({
            "v": "openagents.att-page-release.v1",
            "commit": option_env!("OPENAGENTS_COMMIT").unwrap_or("unknown"),
            "source": "crates/att-web (built by scripts/build-att-web.sh)",
            "page": {"path": ATT_PATH, "sha256": format!("{:x}", Sha256::digest(page.as_bytes()))},
            "files": files,
            "verify_without_this_page": "cargo run -p oa-att --features net -- round --publisher PUBLISHER --state TEXT --question TEXT",
            "publisher": PUBLISHER,
        }),
    )
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
    cache: Mutex<HashMap<Lane, (Instant, Records, Value)>>,
    /// The open lane's beacon and the gateway's check of it.
    open: Mutex<Option<(Instant, Event, Value)>>,
    wakes: Mutex<VecDeque<Instant>>,
    /// The GPU machine's state as Compute Engine last said.
    vm: Mutex<Option<(Instant, String)>>,
    visitors: Mutex<HashMap<String, VecDeque<Instant>>>,
    everyone: Mutex<VecDeque<Instant>>,
    rounds: Mutex<HashMap<String, (Instant, Round)>>,
    changed: tokio::sync::Notify,
}

static SHARED: LazyLock<Arc<Shared>> = LazyLock::new(|| {
    Arc::new(Shared {
        secret: SecretKey::new(&mut secp256k1::rand::rng()),
        cache: Mutex::new(HashMap::new()),
        open: Mutex::new(None),
        wakes: Mutex::new(VecDeque::new()),
        vm: Mutex::new(None),
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

fn policy(workload: &str) -> Policy {
    Policy {
        publisher: PUBLISHER.into(),
        workload: workload.into(),
        required: nostr::att::Level::TeeCloud,
        seen_generation: None,
    }
}

/// The gateway's own check of the records, with every step's verdict.
fn gateway_check(records: &Records, workload: &str, at: u64) -> Value {
    let policy = policy(workload);
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
async fn records(shared: &Shared, lane: Lane) -> Result<(Records, Value, u64, bool), String> {
    let workload = lane.workload().ok_or("this lane has no attested records")?;
    if let Ok(held) = shared.cache.lock()
        && let Some((at, records, check)) = held.get(&lane)
        && at.elapsed() < STATE_TTL
    {
        return Ok((records.clone(), check.clone(), 0, true));
    }
    let fetched = net::fetch(RELAY, &shared.secret, PUBLISHER, workload, now()).await?;
    let check = gateway_check(&fetched.records, workload, now());
    if let Ok(mut held) = shared.cache.lock() {
        held.insert(
            lane,
            (Instant::now(), fetched.records.clone(), check.clone()),
        );
    }
    Ok((fetched.records, check, fetched.ms, false))
}

/// The open lane's beacon, fetched or reused, and the gateway's check.
async fn open_beacon(shared: &Shared) -> Result<(Event, Value, u64, bool), String> {
    if let Ok(held) = shared.open.lock()
        && let Some((at, beacon, check)) = held.as_ref()
        && at.elapsed() < STATE_TTL
    {
        return Ok((beacon.clone(), check.clone(), 0, true));
    }
    let (beacon, ms) = net::fetch_beacon(RELAY, &shared.secret, OPEN_PYLON, OPEN_SLUG).await?;
    let check =
        match oa_att::open::parse_beacon(&beacon, OPEN_PYLON, CLEF_WEIGHTS, now(), Tamper::None) {
            Ok(found) => json!({"ok": true, "level": found.level.as_str(), "served": found.served}),
            Err(why) => json!({"ok": false, "step": "beacon", "reason": why.0}),
        };
    if let Ok(mut held) = shared.open.lock() {
        *held = Some((Instant::now(), beacon.clone(), check.clone()));
    }
    Ok((beacon, check, ms, false))
}

#[derive(serde::Deserialize, Default)]
struct LaneQuery {
    lane: Option<String>,
}

async fn state(Query(query): Query<LaneQuery>) -> Response {
    let shared = Arc::clone(&SHARED);
    let lane = Lane::parse(query.lane.as_deref());
    if lane == Lane::Open {
        return match open_beacon(&shared).await {
            Ok((beacon, check, fetched_ms, cached)) => json_response(
                StatusCode::OK,
                &json!({
                    "lane": lane.id(),
                    "relay": RELAY,
                    "pylon": OPEN_PYLON,
                    "slug": OPEN_SLUG,
                    "artifact": CLEF_WEIGHTS,
                    "fetched_ms": fetched_ms,
                    "cached": cached,
                    "events": {"beacon": beacon},
                    "gateway": check,
                    "now": now(),
                }),
            ),
            Err(why) => json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &json!({"error": format!("The Pylon is not reachable right now: {why}")}),
            ),
        };
    }
    match records(&shared, lane).await {
        Ok((records, check, fetched_ms, cached)) => json_response(
            StatusCode::OK,
            &json!({
                "lane": lane.id(),
                "relay": RELAY,
                "publisher": PUBLISHER,
                "workload": lane.workload(),
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
        Err(why) => {
            let machine = machine_state(&shared, lane).await;
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &json!({
                    "error": format!("The sealed machine is not reachable right now: {why}"),
                    "machine": machine,
                    "can_wake": lane.vm().is_some(),
                }),
            )
        }
    }
}

/// The metadata server's access token for this service's own account.
async fn cloud_token() -> Result<String, String> {
    let response = reqwest::Client::new()
        .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
        .header("Metadata-Flavor", "Google")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| "no cloud identity here".to_string())?;
    let body: Value = response
        .json()
        .await
        .map_err(|_| "the cloud identity answered oddly".to_string())?;
    body["access_token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "no cloud identity here".into())
}

/// Compute Engine's word for the lane's machine (`RUNNING`, `TERMINATED`,
/// `STAGING` …), cached briefly; `None` for a lane without one or when it
/// can't be asked.
async fn machine_state(shared: &Shared, lane: Lane) -> Option<String> {
    let (vm, zone) = lane.vm()?;
    if let Ok(held) = shared.vm.lock()
        && let Some((at, status)) = held.as_ref()
        && at.elapsed() < Duration::from_secs(10)
    {
        return Some(status.clone());
    }
    let token = cloud_token().await.ok()?;
    let url = format!(
        "https://compute.googleapis.com/compute/v1/projects/{PROJECT}/zones/{zone}/instances/{vm}?fields=status"
    );
    let body: Value = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let status = body["status"].as_str()?.to_string();
    if let Ok(mut held) = shared.vm.lock() {
        *held = Some((Instant::now(), status.clone()));
    }
    Some(status)
}

/// `GET /att/api/lanes`: every lane, its level, measured time and cost,
/// who can see what, and whether it can answer now.
async fn lanes() -> Response {
    let shared = Arc::clone(&SHARED);
    let (gpu, cpu, open) = tokio::join!(
        records(&shared, Lane::Gpu),
        records(&shared, Lane::Cpu),
        open_beacon(&shared)
    );
    let machine = if gpu.is_err() {
        machine_state(&shared, Lane::Gpu).await
    } else {
        None
    };
    let mut out = Vec::new();
    for lane in Lane::ALL {
        let mut info = lane.info();
        let ready = match lane {
            Lane::Gpu => gpu.as_ref().is_ok_and(|r| r.1["ok"] == true),
            Lane::Cpu => cpu.as_ref().is_ok_and(|r| r.1["ok"] == true),
            Lane::Open => open.as_ref().is_ok_and(|r| r.1["ok"] == true),
        };
        let status = if ready {
            "ready"
        } else if lane == Lane::Gpu {
            match machine.as_deref() {
                Some("TERMINATED" | "STOPPED" | "SUSPENDED") => "asleep",
                Some("PROVISIONING" | "STAGING" | "RUNNING" | "REPAIRING") => "waking",
                Some("STOPPING" | "SUSPENDING") => "stopping",
                _ => "unavailable",
            }
        } else {
            "unavailable"
        };
        info["status"] = json!(status);
        if lane == Lane::Gpu {
            info["machine"] = json!(machine);
        }
        out.push(info);
    }
    json_response(StatusCode::OK, &json!({"lanes": out, "now": now()}))
}

/// `POST /att/api/wake`: start the sealed GPU machine (a stopped spot VM
/// that stops itself when idle and after its maximum run time). A few
/// wakes an hour for everyone together.
async fn wake() -> Response {
    let shared = Arc::clone(&SHARED);
    let Some((vm, zone)) = Lane::Gpu.vm() else {
        return refuse(StatusCode::NOT_FOUND, "No machine to wake.");
    };
    match machine_state(&shared, Lane::Gpu).await.as_deref() {
        Some("RUNNING" | "PROVISIONING" | "STAGING") => {
            return json_response(
                StatusCode::OK,
                &json!({"machine": "RUNNING", "message": "The sealed GPU is already awake or starting."}),
            );
        }
        Some(_) => {}
        None => {
            return refuse(
                StatusCode::SERVICE_UNAVAILABLE,
                "The sealed GPU can't be reached from here right now.",
            );
        }
    }
    {
        let Ok(mut wakes) = shared.wakes.lock() else {
            return refuse(StatusCode::SERVICE_UNAVAILABLE, "busy");
        };
        while wakes
            .front()
            .is_some_and(|t| t.elapsed() > Duration::from_secs(3_600))
        {
            wakes.pop_front();
        }
        if wakes.len() >= WAKES_PER_HOUR {
            return refuse(
                StatusCode::TOO_MANY_REQUESTS,
                "The sealed GPU has been woken several times this hour. Try the other lanes, or again later.",
            );
        }
        wakes.push_back(Instant::now());
    }
    let Ok(token) = cloud_token().await else {
        return refuse(StatusCode::SERVICE_UNAVAILABLE, "No cloud identity here.");
    };
    let url = format!(
        "https://compute.googleapis.com/compute/v1/projects/{PROJECT}/zones/{zone}/instances/{vm}/start"
    );
    let started = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .header(header::CONTENT_LENGTH, "0")
        .timeout(Duration::from_secs(15))
        .send()
        .await;
    if let Ok(mut held) = shared.vm.lock() {
        *held = None;
    }
    match started {
        Ok(response) if response.status().is_success() => json_response(
            StatusCode::OK,
            &json!({"machine": "STAGING", "message": "Waking the sealed GPU. It takes a few minutes to boot, check its GPU, load the model and publish its evidence."}),
        ),
        Ok(response) => {
            let status = response.status();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            let why = body["error"]["message"].as_str().unwrap_or("refused");
            refuse(
                StatusCode::BAD_GATEWAY,
                &format!("Google Cloud did not start the sealed GPU ({status}): {why}"),
            )
        }
        Err(_) => refuse(
            StatusCode::BAD_GATEWAY,
            "Google Cloud did not answer the wake request.",
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

async fn send(Query(query): Query<LaneQuery>, headers: HeaderMap, body: Bytes) -> Response {
    let lane = Lane::parse(query.lane.as_deref());
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
    let target = if lane == Lane::Open {
        match open_beacon(&shared).await {
            Ok((_, check, _, _)) if check["ok"] == true => OPEN_PYLON.to_string(),
            Ok(_) => {
                return refuse(
                    StatusCode::CONFLICT,
                    "The Pylon's beacon did not check out, so the gateway sends nothing to it.",
                );
            }
            Err(why) => {
                return refuse(
                    StatusCode::SERVICE_UNAVAILABLE,
                    &format!("The Pylon is not reachable right now: {why}"),
                );
            }
        }
    } else {
        let (records, check, _, _) = match records(&shared, lane).await {
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
        records.endpoint.pubkey.clone()
    };
    let endpoint_key = target;
    let tagged: Vec<&str> = event.tag_values("p").collect();
    if tagged != [endpoint_key.as_str()] {
        return refuse(
            StatusCode::BAD_REQUEST,
            "The request is not addressed to the lane's machine.",
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
        assert!(page.contains(&crate::theme::style_tag()));
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
    fn lanes_parse_and_say_their_level() {
        assert_eq!(Lane::parse(None), Lane::Cpu);
        assert_eq!(Lane::parse(Some("gpu")), Lane::Gpu);
        assert_eq!(Lane::parse(Some("open")), Lane::Open);
        assert_eq!(Lane::Open.info()["level"], "open");
        assert!(Lane::Open.workload().is_none());
        for lane in Lane::ALL {
            assert_eq!(lane.info()["id"], lane.id());
            assert_eq!(lane.info()["sees"].as_array().map(Vec::len), Some(4));
        }
    }

    #[test]
    fn a_visitor_is_rate_limited() {
        let shared = Shared {
            secret: SecretKey::new(&mut secp256k1::rand::rng()),
            cache: Mutex::new(HashMap::new()),
            open: Mutex::new(None),
            wakes: Mutex::new(VecDeque::new()),
            vm: Mutex::new(None),
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
