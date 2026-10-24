//! The launch playtest, the waitlist, and the short profile (launch,
//! 2026-10-24).
//!
//! The flow: the homepage's Play the beta goes to [`INTRO`], which says
//! what the visitor will do and what we record, and asks whether we may
//! record their steps. Starting sets the `oa_playtest` cookie (a random
//! session id `pt_…`, whether they agreed, and where they came from) and
//! opens the Haunted House tutorial (`/new-lander/house`), which then shows
//! a Finish playtest button ([`finish_controls`]). That button, and a
//! prompt after a few minutes or at the tutorial's end, lead to the
//! survey ([`SURVEY`]); its thank-you page has a link to share. The
//! profile ([`PROFILE`]) is offered after sign-up, and to guests before
//! and after the playtest. The waitlist posts to [`WAITLIST`].
//!
//! Each funnel step goes to [`events`] (a no-op until analytics installs
//! a sink), with the session id only when the visitor agreed and their
//! browser sends neither `DNT: 1` nor `Sec-GPC: 1`. Records live in the
//! site's store ([`store`]); staff download them at [`ADMIN`].

pub mod events;
pub(crate) mod forms;
pub(crate) mod store;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use sha2::{Digest, Sha256};

use crate::App;
use crate::pages::new_lander::{Surface, footer, launch_document};
use events::Step;
use forms::{ENGINES, EXPERIENCE, FINISHED, Fields, ROLES};
use store::{Kept, Profile, Survey};

/// The playtest's intro.
pub(crate) const INTRO: &str = "/playtest";
const START: &str = "/playtest/start";
pub(crate) const SURVEY: &str = "/playtest/survey";
pub(crate) const PROFILE: &str = "/playtest/profile";
/// Where the page's script reports [`Step::from_page`] steps.
pub(crate) const EVENTS: &str = "/playtest/events";
const SCRIPT: &str = "/playtest/playtest.js";
pub(crate) const WAITLIST: &str = "/waitlist";
/// Staff's page of sign-ups, and its downloads under it.
pub(crate) const ADMIN: &str = "/admin/launch";
/// Sign-up's `return_to`: the profile, percent-encoded.
pub(crate) const PROFILE_RETURN: &str = "%2Fplaytest%2Fprofile";
/// The Haunted House tutorial the playtest opens.
const HOUSE: &str = crate::pages::new_lander_house::HOUSE;
/// What a visitor shares with a friend.
const SHARE: &str = "https://openagents.com/playtest?utm_source=friend&utm_medium=share&utm_campaign=playtest-launch";

const COOKIE: &str = crate::research::SESSION_COOKIE;
const FROM_COOKIE: &str = "oa_playtest_from";
/// The research script's opt-out cookie (`/static/r.js`).
const OPT_OUT_COOKIE: &str = "oa_research_optout";
/// A playtest session lasts a week.
const COOKIE_AGE: u64 = 7 * 24 * 3600;
/// Seconds in the house before the survey is offered.
const PROMPT_AFTER: u32 = 240;
const SCRIPT_BODY: &str = include_str!("playtest.js");

/// Hourly limits per client address.
const WAITLIST_PER_HOUR: usize = 10;
const PROFILE_PER_HOUR: usize = 20;
/// Surveys share an address at events and schools, so more go through.
const SURVEY_PER_HOUR: usize = 30;
/// Page-reported steps per session.
const EVENTS_PER_HOUR: usize = 300;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(INTRO, get(intro))
        .route(START, post(start))
        .route(SURVEY, get(survey_page).post(survey_send))
        .route(PROFILE, get(profile_page).post(profile_send))
        .route(EVENTS, post(page_event))
        .route(SCRIPT, get(script))
        .route(WAITLIST, get(waitlist_page).post(waitlist_send))
        .route(ADMIN, get(admin_page))
        .route("/admin/launch/export.json", get(export_json))
        .route("/admin/launch/{file}", get(export_csv))
}

/// The paths this module answers.
pub(crate) fn owns(path: &str) -> bool {
    path == INTRO
        || path.starts_with("/playtest/")
        || path == WAITLIST
        || path == ADMIN
        || path.starts_with("/admin/launch/")
}

// --- Sessions ------------------------------------------------------------

/// A visitor's playtest: the id in `oa_playtest` (which the research script
/// reads too, so it isn't `HttpOnly`) and where they came from, in
/// `oa_playtest_from`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) source: String,
}

impl Session {
    fn new(source: &str) -> Self {
        let random = crate::cloud::byo::fresh_request();
        Session {
            id: format!("pt_{}", &random[..24]),
            source: forms::source(source),
        }
    }

    /// The `Set-Cookie` values that keep this session.
    fn cookies(&self, secure: bool) -> Vec<HeaderValue> {
        let secure = if secure { "; Secure" } else { "" };
        let mut values = vec![format!(
            "{COOKIE}={}; Max-Age={COOKIE_AGE}; Path=/; SameSite=Lax{secure}",
            self.id
        )];
        if !self.source.is_empty() {
            values.push(format!(
                "{FROM_COOKIE}={}; Max-Age={COOKIE_AGE}; Path=/; HttpOnly; SameSite=Lax{secure}",
                self.source
            ));
        }
        values
            .into_iter()
            .map(|value| HeaderValue::from_str(&value).expect("a cleaned cookie is valid"))
            .collect()
    }
}

/// `pt_` and 24 lowercase hex digits.
pub(crate) fn valid_session(id: &str) -> bool {
    id.strip_prefix("pt_").is_some_and(|hex| {
        hex.len() == 24
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.trim())
}

/// The request's playtest session, if it carries one.
pub(crate) fn session(headers: &HeaderMap) -> Option<Session> {
    let id = cookie(headers, COOKIE).filter(|id| valid_session(id))?;
    Some(Session {
        id: id.to_owned(),
        source: forms::source(cookie(headers, FROM_COOKIE).unwrap_or("")),
    })
}

/// Whether the visitor turned recording off (the intro's choice, kept by
/// the research script's `oa_research_optout` cookie) or their browser
/// asks not to be tracked.
pub(crate) fn opted_out(headers: &HeaderMap) -> bool {
    cookie(headers, OPT_OUT_COOKIE) == Some("1")
        || ["dnt", "sec-gpc"]
            .iter()
            .any(|name| headers.get(*name).is_some_and(|v| v.as_bytes() == b"1"))
}

/// The `Set-Cookie` for the intro's recording choice, matching what the
/// research script's `window.oaResearch.optOut` writes.
fn opt_out_cookie(off: bool) -> HeaderValue {
    let value = if off {
        format!("{OPT_OUT_COOKIE}=1; Max-Age=31536000; Path=/; SameSite=Lax")
    } else {
        format!("{OPT_OUT_COOKIE}=0; Max-Age=0; Path=/; SameSite=Lax")
    };
    HeaderValue::from_str(&value).expect("a fixed cookie is valid")
}

/// Send `step` for this request's playtest, with the source, the device
/// class, and `extra`. Nothing goes without a session; when the visitor
/// opted out, only that choice is sent.
pub(crate) fn step(
    headers: &HeaderMap,
    step: Step,
    session: Option<&Session>,
    extra: &[(&str, &str)],
) {
    let Some(session) = session else {
        return;
    };
    if opted_out(headers) && step != Step::PlaytestOptedOut {
        return;
    }
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let mut props = vec![("device", forms::device_and_browser(agent).0)];
    if !session.source.is_empty() {
        props.push(("source", session.source.as_str()));
    }
    props.extend_from_slice(extra);
    events::emit(&session.id, step, &props);
}

// --- Limits --------------------------------------------------------------

/// Requests per key within an hour.
struct Limiter {
    per_hour: usize,
    seen: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Limiter {
    fn new(per_hour: usize) -> Self {
        Limiter {
            per_hour,
            seen: Mutex::new(HashMap::new()),
        }
    }

    fn allow(&self, key: &str) -> bool {
        let Ok(mut seen) = self.seen.lock() else {
            return true;
        };
        let now = Instant::now();
        let hour = Duration::from_secs(3600);
        if seen.len() > 50_000 {
            seen.retain(|_, times| times.iter().any(|at| now.duration_since(*at) < hour));
        }
        let times = seen.entry(key.to_owned()).or_default();
        times.retain(|at| now.duration_since(*at) < hour);
        if times.len() >= self.per_hour {
            return false;
        }
        times.push(now);
        true
    }
}

fn limiter(kind: &'static str) -> &'static Limiter {
    static WAITLIST: OnceLock<Limiter> = OnceLock::new();
    static PROFILE: OnceLock<Limiter> = OnceLock::new();
    static SURVEY: OnceLock<Limiter> = OnceLock::new();
    static EVENTS: OnceLock<Limiter> = OnceLock::new();
    match kind {
        "waitlist" => WAITLIST.get_or_init(|| Limiter::new(WAITLIST_PER_HOUR)),
        "profile" => PROFILE.get_or_init(|| Limiter::new(PROFILE_PER_HOUR)),
        "survey" => SURVEY.get_or_init(|| Limiter::new(SURVEY_PER_HOUR)),
        _ => EVENTS.get_or_init(|| Limiter::new(EVENTS_PER_HOUR)),
    }
}

/// The client address for limits: the last `X-Forwarded-For` entry, the
/// one Cloud Run's front end appends, hashed and kept in memory only.
fn client(headers: &HeaderMap) -> String {
    let address = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(str::trim)
        .unwrap_or("local");
    let digest = Sha256::digest(address.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// A form posted from another site.
fn cross_site(headers: &HeaderMap) -> bool {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if let Some(site) = text("sec-fetch-site") {
        return !matches!(site, "same-origin" | "none");
    }
    match (text("origin"), text("host")) {
        (Some(origin), Some(host)) => {
            origin
                .strip_prefix("https://")
                .or_else(|| origin.strip_prefix("http://"))
                != Some(host)
        }
        _ => false,
    }
}

// --- Page frame ----------------------------------------------------------

fn account_links(app: &App) -> bool {
    app.config
        .features
        .on(crate::features::Feature::HomeAccount)
}

/// A launch page's response: the shared frame, the policy, never stored.
fn page(app: &App, status: StatusCode, title: &str, path: &str, content: Markup) -> Response {
    page_with(app, status, title, path, content, Vec::new())
}

fn page_with(
    app: &App,
    status: StatusCode,
    title: &str,
    path: &str,
    content: Markup,
    cookie: Vec<HeaderValue>,
) -> Response {
    let body = html! {
        (content)
        (footer())
        script type="module" src=(SCRIPT) {}
    };
    let mut response = crate::pages::new_lander::respond(launch_document(
        body,
        &format!("{title} · OpenAgents"),
        path,
        account_links(app),
        Surface::Preview,
    ));
    *response.status_mut() = status;
    for value in cookie {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

fn problem_box(problem: Option<&str>) -> Markup {
    html! {
        @if let Some(problem) = problem {
            p.hud-toast.hud-problem role="alert" { (problem) }
        }
    }
}

fn options(name: &str, choices: &[(&str, &str)], selected: &str, kind: &str) -> Markup {
    html! {
        @for (value, label) in choices {
            label.hud-choice {
                input type=(kind) name=(name) value=(value) checked[*value == selected];
                span { (label) }
            }
        }
    }
}

/// A row of numbered choices from `low` to `high`, with words at each end.
fn scale(name: &str, low: u8, high: u8, ends: (&str, &str), selected: &str) -> Markup {
    html! {
        div.hud-scale {
            div.hud-scale-row {
                @for n in low..=high {
                    @let value = n.to_string();
                    label.hud-choice.hud-scale-step {
                        input type="radio" name=(name) value=(value) checked[value == selected] required;
                        span { (value) }
                    }
                }
            }
            p.hud-scale-ends aria-hidden="true" { span { (ends.0) } span { (ends.1) } }
        }
    }
}

// --- Waitlist ------------------------------------------------------------

/// What the visitor typed, to show again with a problem.
#[derive(Default)]
pub(crate) struct WaitlistDraft {
    pub(crate) email: String,
    pub(crate) role: String,
}

/// The waitlist form, on the homepage and at [`WAITLIST`].
pub(crate) fn waitlist_section(
    source: &str,
    problem: Option<&str>,
    draft: &WaitlistDraft,
) -> Markup {
    html! {
        section.lp-waitlist #waitlist aria-labelledby="waitlist-title" {
            div.lp-section-heading {
                p.hud-label { "WAITLIST" }
                h2 #waitlist-title { "Not ready to play? " span { "Join the waitlist." } }
                p { "We'll email you when new worlds and tools open. No spam, and you can leave any time." }
            }
            form.lp-form.hud-panel method="post" action=(WAITLIST) {
                (problem_box(problem))
                div.hud-field {
                    label for="waitlist-email" { "Email" }
                    input #waitlist-email type="email" name="email" required maxlength="200" autocomplete="email" placeholder="you@example.com" value=(draft.email);
                }
                div.hud-field {
                    label for="waitlist-role" { "What describes you best? " span.hud-hint { "Optional" } }
                    select #waitlist-role name="role" {
                        option value="" selected[draft.role.is_empty()] { "Choose one" }
                        @for (value, label) in ROLES { option value=(value) selected[draft.role == value] { (label) } }
                    }
                }
                input type="hidden" name="source" value=(source);
                div.hud-trap aria-hidden="true" { label { "Leave this empty" input type="text" name="website" tabindex="-1" autocomplete="off"; } }
                button.hud-button.hud-primary type="submit" { "Join the waitlist" }
            }
        }
    }
}

async fn waitlist_page(State(app): State<App>, RawQuery(query): RawQuery) -> Response {
    let source = crate::pages::new_lander::source_of(query.as_deref());
    waitlist_form(
        &app,
        StatusCode::OK,
        &source,
        None,
        &WaitlistDraft::default(),
    )
}

fn waitlist_form(
    app: &App,
    status: StatusCode,
    source: &str,
    problem: Option<&str>,
    draft: &WaitlistDraft,
) -> Response {
    page(
        app,
        status,
        "Join the waitlist",
        WAITLIST,
        html! { main.lp-page #main { (waitlist_section(source, problem, draft)) } },
    )
}

async fn waitlist_send(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let fields = Fields::parse(&body);
    let draft = WaitlistDraft {
        email: fields.get("email").chars().take(200).collect(),
        role: fields.get("role").chars().take(20).collect(),
    };
    let source = forms::source(fields.get("source"));
    if !fields.get("website").is_empty() {
        return waitlist_done(&app);
    }
    if cross_site(&headers) {
        return waitlist_form(
            &app,
            StatusCode::FORBIDDEN,
            &source,
            Some("Send the form from this page."),
            &draft,
        );
    }
    let entry = match forms::waitlist(&fields, crate::chat_store::now_unix()) {
        Ok(entry) => entry,
        Err(problem) => {
            return waitlist_form(
                &app,
                StatusCode::BAD_REQUEST,
                &source,
                Some(problem),
                &draft,
            );
        }
    };
    if !limiter("waitlist").allow(&client(&headers)) {
        return waitlist_form(
            &app,
            StatusCode::TOO_MANY_REQUESTS,
            &source,
            Some("That's a lot of sign-ups from here. Try again in an hour."),
            &draft,
        );
    }
    match store::join_waitlist(&app.config.chat_store, &entry).await {
        Ok(Kept::New) => step(
            &headers,
            Step::WaitlistJoined,
            session(&headers).as_ref(),
            &[],
        ),
        Ok(Kept::AlreadyThere) => {}
        Err(error) => {
            eprintln!("openagents-web: waitlist entry not kept: {error}");
            return waitlist_form(
                &app,
                StatusCode::SERVICE_UNAVAILABLE,
                &source,
                Some("We couldn't add you just now. Try again in a minute."),
                &draft,
            );
        }
    }
    // The same answer whether the address was new, so the page never
    // says who else signed up.
    waitlist_done(&app)
}

fn waitlist_done(app: &App) -> Response {
    page(
        app,
        StatusCode::OK,
        "You're on the list",
        WAITLIST,
        html! {
            main.lp-page #main {
                section.hud-panel.lp-done aria-labelledby="done-title" {
                    p.hud-label { span.hud-signal {} "WAITLIST" }
                    h1 #done-title { "You're on the list." }
                    p { "We'll email you when there's something new to try. While you wait, the beta is open now." }
                    div.lp-actions {
                        a.hud-button.hud-primary href=(INTRO) { "Play the beta" }
                        a.hud-button.hud-ghost href="/" { "Back to OpenAgents" }
                    }
                }
            }
        },
    )
}

// --- Intro and start -----------------------------------------------------

/// The intro's address from `from` (a page name) with `source` kept.
pub(crate) fn intro_href(from: &str, source: &str) -> String {
    let mut href = format!("{INTRO}?from={from}");
    if !source.is_empty() {
        href.push_str("&utm_source=");
        href.push_str(source);
    }
    href
}

async fn intro(State(app): State<App>, headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    let source = crate::pages::new_lander::source_of(query.as_deref());
    let current = session(&headers);
    let source = if source.is_empty() {
        current
            .as_ref()
            .map(|s| s.source.clone())
            .unwrap_or_default()
    } else {
        source
    };
    let from_home = url::form_urlencoded::parse(query.as_deref().unwrap_or("").as_bytes())
        .any(|(k, v)| k == "from" && v == "home");
    if from_home {
        step(&headers, Step::PlayClicked, current.as_ref(), &[]);
    }
    let open = crate::pages::new_lander_house::available();
    page(
        &app,
        StatusCode::OK,
        "Play the beta",
        INTRO,
        html! {
            main.lp-page #main {
                section.lp-intro aria-labelledby="intro-title" {
                    p.hud-label { span.hud-signal {} "PLAYTEST · ABOUT 5 MINUTES" }
                    h1 #intro-title { "Play the beta." }
                    p.lp-lede { "Thanks for helping us test OpenAgents. Here's what you'll do:" }
                    ol.lp-steps {
                        li { strong { "Step into the Haunted House." } " A short tutorial shows you around." }
                        li { strong { "Make a change." } " Move something, or ask the agent to change it for you." }
                        li { strong { "Tell us how it went." } " Choose Finish playtest for a short survey." }
                    }
                    p.hud-hint { "Works best on a computer with a recent browser. Phones work too." }
                }
                section.hud-card.lp-consent aria-labelledby="consent-title" {
                    h2 #consent-title { "What we record" }
                    p { "While you play, we record the pages you open, the buttons you choose, and how far you get, under a random ID for this playtest. We use it only to make OpenAgents better." }
                    p { "We never record what you type. If you're signed in, we keep your account with it." }
                    p { "Rather we didn't? Choose Play without recording, and we'll only count visits. Your browser's Do Not Track or Global Privacy Control setting does the same. " a href="/privacy" { "Read our privacy policy" } "." }
                }
                @if open {
                    form method="post" action=(START) {
                        input type="hidden" name="source" value=(source);
                        div.lp-actions {
                            button.hud-button.hud-primary.hud-large type="submit" name="record" value="1" { span { "Start the playtest" } span aria-hidden="true" { "↗" } }
                            button.hud-button.hud-ghost type="submit" name="record" value="0" { "Play without recording" }
                        }
                    }
                } @else {
                    p.hud-toast role="status" { "The house is closed for a moment. Join the waitlist and we'll tell you when it's open." }
                    div.lp-actions { a.hud-button.hud-primary href=(format!("{WAITLIST}?utm_source={source}")) { "Join the waitlist" } }
                }
                p.hud-hint { "Want to tell us about yourself first? " a href=(PROFILE) { "Fill in a short profile" } " (optional)." }
            }
        },
    )
}

async fn start(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let fields = Fields::parse(&body);
    if cross_site(&headers) {
        return Redirect::to(INTRO).into_response();
    }
    let source = match fields.get("source") {
        "" => session(&headers).map(|s| s.source).unwrap_or_default(),
        source => source.to_owned(),
    };
    let session = Session::new(&source);
    let off = fields.get("record") == "0";
    let mut response = Redirect::to(HOUSE).into_response();
    let set = response.headers_mut();
    for cookie in session.cookies(app.config.secure_cookies) {
        set.append(header::SET_COOKIE, cookie);
    }
    set.append(header::SET_COOKIE, opt_out_cookie(off));
    // The choice made now counts, whatever an earlier cookie said.
    let mut asked = headers.clone();
    asked.remove(header::COOKIE);
    let started = if off {
        Step::PlaytestOptedOut
    } else {
        Step::PlaytestStarted
    };
    step(&asked, started, Some(&session), &[]);
    response
}

/// The Finish playtest button and prompt the house page shows during a
/// playtest, with the script that reports the steps only the page sees.
pub(crate) fn finish_controls() -> Markup {
    html! {
        a.hud-button.hud-primary.playtest-finish #playtest-finish href=(format!("{SURVEY}?from=finish")) data-prompt-after=(PROMPT_AFTER) { "Finish playtest" }
        div.hud-toast.playtest-prompt #playtest-prompt role="status" hidden {
            p { "Done looking around? Tell us how it went." }
            div.lp-actions {
                a.hud-button.hud-primary href=(format!("{SURVEY}?from=prompt")) { "Finish playtest" }
                button.hud-button.hud-ghost #playtest-later type="button" { "Not yet" }
            }
        }
        script type="module" src=(SCRIPT) {}
    }
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        SCRIPT_BODY,
    )
        .into_response()
}

/// A step only the page sees: `{"step": "..."}` from `navigator.sendBeacon`.
async fn page_event(headers: HeaderMap, body: Bytes) -> Response {
    let Some(current) = session(&headers) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    if cross_site(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if body.len() > 512 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let named = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("step").and_then(|s| s.as_str()).and_then(Step::parse));
    let Some(named) = named.filter(|s| s.from_page()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !limiter("events").allow(&current.id) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    step(&headers, named, Some(&current), &[]);
    StatusCode::NO_CONTENT.into_response()
}

// --- Survey --------------------------------------------------------------

/// The session to tie a survey to: the cookie's, or a new one for someone
/// who reached the survey another way.
fn survey_session(headers: &HeaderMap, source: &str) -> (Session, bool) {
    match session(headers) {
        Some(session) => (session, false),
        None => (Session::new(source), true),
    }
}

async fn survey_page(
    State(app): State<App>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let source = crate::pages::new_lander::source_of(query.as_deref());
    let (current, fresh) = survey_session(&headers, &source);
    let from = url::form_urlencoded::parse(query.as_deref().unwrap_or("").as_bytes())
        .find(|(k, _)| k == "from")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default();
    if matches!(from.as_str(), "finish" | "prompt") {
        step(
            &headers,
            Step::TutorialFinishClicked,
            Some(&current),
            &[("from", from.as_str())],
        );
    }
    let cookie = if fresh {
        current.cookies(app.config.secure_cookies)
    } else {
        Vec::new()
    };
    if store::survey_done(&app.config.chat_store, &current.id)
        .await
        .unwrap_or(false)
    {
        return thanks(&app, &current, cookie, true);
    }
    step(&headers, Step::SurveyStarted, Some(&current), &[]);
    survey_form(&app, StatusCode::OK, &Fields::default(), None, cookie)
}

fn survey_form(
    app: &App,
    status: StatusCode,
    draft: &Fields,
    problem: Option<&str>,
    cookie: Vec<HeaderValue>,
) -> Response {
    let content = html! {
        main.lp-page #main {
            p.hud-label { span.hud-signal {} "PLAYTEST · LAST STEP" }
            h1 { "How did it go?" }
            p.lp-lede { "Six quick questions. Only the ones with a star are required." }
            form.lp-form.hud-panel.lp-survey method="post" action=(SURVEY) {
                (problem_box(problem))
                fieldset.hud-field {
                    legend { "How easy was it to use? " span.hud-required aria-label="required" { "*" } }
                    (scale("ease", 1, 5, ("Very hard", "Very easy"), draft.get("ease")))
                }
                fieldset.hud-field {
                    legend { "Did you finish the tutorial? " span.hud-required aria-label="required" { "*" } }
                    div.hud-choices { (options("finished", &FINISHED, draft.get("finished"), "radio")) }
                }
                div.hud-field {
                    label for="survey-confused" { "What confused you, if anything?" }
                    textarea #survey-confused name="confused" rows="3" maxlength=(forms::TEXT_MAX) { (draft.get("confused")) }
                }
                div.hud-field {
                    label for="survey-make" { "What would you want to make?" }
                    textarea #survey-make name="make" rows="3" maxlength=(forms::TEXT_MAX) placeholder="A game, a place, a story…" { (draft.get("make")) }
                }
                fieldset.hud-field {
                    legend { "How likely are you to use it again? " span.hud-required aria-label="required" { "*" } }
                    (scale("again", 0, 10, ("Not likely", "Very likely"), draft.get("again")))
                }
                div.hud-field {
                    label for="survey-email" { "Your email " span.hud-hint { "Optional" } }
                    input #survey-email type="email" name="email" maxlength="200" autocomplete="email" placeholder="you@example.com" value=(draft.get("email"));
                    label.hud-check {
                        input type="checkbox" name="contact_ok" checked[draft.checked("contact_ok")];
                        span { "You can email me about my answers and the next playtest." }
                    }
                }
                p.hud-hint { "We also note your device type and browser, so we can fix problems on each." }
                div.hud-trap aria-hidden="true" { label { "Leave this empty" input type="text" name="website" tabindex="-1" autocomplete="off"; } }
                button.hud-button.hud-primary.hud-large type="submit" { span { "Send my answers" } span aria-hidden="true" { "↗" } }
            }
        }
    };
    page_with(app, status, "Playtest survey", SURVEY, content, cookie)
}

async fn survey_send(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let fields = Fields::parse(&body);
    let (current, fresh) = survey_session(&headers, "");
    let cookie = || {
        if fresh {
            current.cookies(app.config.secure_cookies)
        } else {
            Vec::new()
        }
    };
    if !fields.get("website").is_empty() {
        return thanks(&app, &current, cookie(), false);
    }
    if cross_site(&headers) {
        return survey_form(
            &app,
            StatusCode::FORBIDDEN,
            &fields,
            Some("Send the survey from this page."),
            cookie(),
        );
    }
    let mut survey = match forms::survey(&fields) {
        Ok(survey) => survey,
        Err(problem) => {
            return survey_form(
                &app,
                StatusCode::BAD_REQUEST,
                &fields,
                Some(problem),
                cookie(),
            );
        }
    };
    if !limiter("survey").allow(&client(&headers)) {
        return survey_form(
            &app,
            StatusCode::TOO_MANY_REQUESTS,
            &fields,
            Some("We've had a lot of answers from here. Try again in an hour."),
            cookie(),
        );
    }
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (device, browser) = forms::device_and_browser(agent);
    survey.session = current.id.clone();
    survey.device = device.to_owned();
    survey.browser = browser.to_owned();
    survey.source = current.source.clone();
    survey.recorded = !opted_out(&headers);
    survey.created_unix = crate::chat_store::now_unix();
    if let Some(service) = app.config.cloud.as_deref()
        && crate::chat_owner::session(&headers).is_some()
        && let Ok(viewer) = service.authenticate(&headers).await
    {
        survey.account_id = Some(viewer.account_id.clone());
        if survey.email.is_empty()
            && let Some(email) = viewer.email
        {
            survey.email = email.to_ascii_lowercase();
        }
    }
    match store::keep_survey(&app.config.chat_store, &survey).await {
        Ok(Kept::New) => step(&headers, Step::SurveyCompleted, Some(&current), &[]),
        Ok(Kept::AlreadyThere) => {}
        Err(error) => {
            eprintln!("openagents-web: playtest survey not kept: {error}");
            return survey_form(
                &app,
                StatusCode::SERVICE_UNAVAILABLE,
                &fields,
                Some("We couldn't save your answers just now. Try again in a minute."),
                cookie(),
            );
        }
    }
    thanks(&app, &current, cookie(), false)
}

/// The link a visitor shares, tagged with a short code for their playtest
/// (not the playtest's id).
fn share_link(session: &Session) -> String {
    let digest = Sha256::digest(session.id.as_bytes());
    let code: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
    format!("{SHARE}&utm_content={code}")
}

fn thanks(app: &App, session: &Session, cookie: Vec<HeaderValue>, again: bool) -> Response {
    let share = share_link(session);
    let content = html! {
        main.lp-page #main {
            section.hud-panel.lp-done aria-labelledby="thanks-title" {
                p.hud-label { span.hud-signal {} "PLAYTEST COMPLETE" }
                h1 #thanks-title { "Thank you!" }
                p { @if again { "We already have your answers for this playtest." } @else { "Your answers help us decide what to build next." } }
                div.lp-share {
                    label for="playtest-share" { "Know someone who'd like this? Send them the playtest." }
                    div.lp-share-row {
                        input #playtest-share type="url" readonly value=(share);
                        button.hud-button #playtest-copy type="button" { "Copy link" }
                    }
                }
                div.lp-actions {
                    a.hud-button.hud-primary href=(PROFILE) { "Tell us about you" }
                    @if account_links(app) { a.hud-button href=(format!("/signup?return_to={PROFILE_RETURN}")) { "Create an account" } }
                    a.hud-button.hud-ghost href="/" { "Back to OpenAgents" }
                }
            }
        }
    };
    page_with(app, StatusCode::OK, "Thank you", SURVEY, content, cookie)
}

// --- Profile -------------------------------------------------------------

/// Who is filling in the profile: the account's id and email when signed in.
async fn member(app: &App, headers: &HeaderMap) -> Option<(String, Option<String>)> {
    let service = app.config.cloud.as_deref()?;
    crate::chat_owner::session(headers)?;
    let viewer = service.authenticate(headers).await.ok()?;
    Some((viewer.account_id.clone(), viewer.email.clone()))
}

async fn profile_page(
    State(app): State<App>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let mut source = crate::pages::new_lander::source_of(query.as_deref());
    let current = session(&headers);
    if source.is_empty() {
        source = current
            .as_ref()
            .map(|s| s.source.clone())
            .unwrap_or_default();
    }
    step(&headers, Step::ProfileStarted, current.as_ref(), &[]);
    let who = member(&app, &headers).await;
    let mut draft = Profile {
        heard: source,
        ..Profile::default()
    };
    if let Some((account, _)) = &who
        && let Ok(Some(kept)) = store::account_profile(&app.config.chat_store, account).await
    {
        draft = kept;
    }
    profile_form(&app, StatusCode::OK, &draft, who.as_ref(), None)
}

fn profile_form(
    app: &App,
    status: StatusCode,
    draft: &Profile,
    who: Option<&(String, Option<String>)>,
    problem: Option<&str>,
) -> Response {
    let account_email = who.and_then(|(_, email)| email.clone());
    let content = html! {
        main.lp-page #main {
            p.hud-label { span.hud-signal {} "YOUR PROFILE" }
            h1 { "Tell us about you." }
            p.lp-lede { "A few questions so we build the right things. It takes a minute." }
            form.lp-form.hud-panel method="post" action=(PROFILE) {
                (problem_box(problem))
                fieldset.hud-field {
                    legend { "What describes you best? " span.hud-required aria-label="required" { "*" } }
                    div.hud-choices { (options("role", &ROLES, &draft.role, "radio")) }
                }
                fieldset.hud-field {
                    legend { "Which engines do you use?" }
                    div.hud-choices {
                        @for (value, label) in ENGINES {
                            label.hud-choice {
                                input type="checkbox" name="engines" value=(value) checked[draft.engines.iter().any(|e| e == value)];
                                span { (label) }
                            }
                        }
                    }
                }
                div.hud-field {
                    label for="profile-experience" { "How long have you been making games?" }
                    select #profile-experience name="experience" {
                        option value="" selected[draft.experience.is_empty()] { "Choose one" }
                        @for (value, label) in EXPERIENCE { option value=(value) selected[draft.experience == value] { (label) } }
                    }
                }
                div.hud-field {
                    label for="profile-make" { "What do you want to make?" }
                    textarea #profile-make name="make" rows="3" maxlength=(forms::TEXT_MAX) placeholder="A game, a place, a story…" { (draft.make) }
                }
                div.hud-field {
                    label for="profile-heard" { "How did you hear about us?" }
                    input #profile-heard type="text" name="heard" maxlength="40" placeholder="A friend, a newsletter, a video…" value=(draft.heard);
                }
                div.hud-field {
                    @if who.is_some() {
                        label for="profile-email" { "Email " span.hud-hint { "Optional, if different from your account's" } }
                    } @else {
                        label for="profile-email" { "Email " span.hud-required aria-label="required" { "*" } }
                    }
                    input #profile-email type="email" name="email" maxlength="200" autocomplete="email" placeholder=(account_email.as_deref().unwrap_or("you@example.com")) value=(if who.is_some() && account_email.as_deref() == Some(draft.email.as_str()) { "" } else { draft.email.as_str() }) required[who.is_none()];
                    label.hud-check {
                        input type="checkbox" name="contact_ok" checked[draft.contact_ok];
                        span { "You can email me about the beta and future playtests." }
                    }
                }
                button.hud-button.hud-primary.hud-large type="submit" { span { "Save my profile" } span aria-hidden="true" { "↗" } }
            }
        }
    };
    page(app, status, "Your profile", PROFILE, content)
}

async fn profile_send(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let fields = Fields::parse(&body);
    let who = member(&app, &headers).await;
    let now = crate::chat_store::now_unix();
    let draft = Profile {
        role: fields.get("role").chars().take(20).collect(),
        engines: fields
            .all("engines")
            .iter()
            .map(|e| e.chars().take(20).collect())
            .collect(),
        experience: fields.get("experience").chars().take(20).collect(),
        make: fields.get("make").chars().take(forms::TEXT_MAX).collect(),
        heard: forms::source(fields.get("heard")),
        email: fields.get("email").chars().take(200).collect(),
        contact_ok: fields.checked("contact_ok"),
        ..Profile::default()
    };
    if cross_site(&headers) {
        return profile_form(
            &app,
            StatusCode::FORBIDDEN,
            &draft,
            who.as_ref(),
            Some("Send the form from this page."),
        );
    }
    let mut profile = match forms::profile(
        &fields,
        who.as_ref().map(|(id, _)| id.as_str()),
        who.as_ref().and_then(|(_, email)| email.as_deref()),
        now,
    ) {
        Ok(profile) => profile,
        Err(problem) => {
            return profile_form(
                &app,
                StatusCode::BAD_REQUEST,
                &draft,
                who.as_ref(),
                Some(problem),
            );
        }
    };
    if !limiter("profile").allow(&client(&headers)) {
        return profile_form(
            &app,
            StatusCode::TOO_MANY_REQUESTS,
            &draft,
            who.as_ref(),
            Some("That's a lot of profiles from here. Try again in an hour."),
        );
    }
    let current = session(&headers);
    profile.session = current.as_ref().map(|s| s.id.clone());
    let first_for_account = match &who {
        Some((account, _)) => matches!(
            store::account_profile(&app.config.chat_store, account).await,
            Ok(None)
        ),
        None => false,
    };
    if let Err(error) = store::keep_profile(&app.config.chat_store, &profile).await {
        eprintln!("openagents-web: launch profile not kept: {error}");
        return profile_form(
            &app,
            StatusCode::SERVICE_UNAVAILABLE,
            &draft,
            who.as_ref(),
            Some("We couldn't save your profile just now. Try again in a minute."),
        );
    }
    step(&headers, Step::ProfileCompleted, current.as_ref(), &[]);
    if first_for_account {
        step(&headers, Step::SignupCompleted, current.as_ref(), &[]);
    }
    let played = current.is_some();
    page(
        &app,
        StatusCode::OK,
        "Profile saved",
        PROFILE,
        html! {
            main.lp-page #main {
                section.hud-panel.lp-done aria-labelledby="saved-title" {
                    p.hud-label { span.hud-signal {} "YOUR PROFILE" }
                    h1 #saved-title { "Thanks, we saved your profile." }
                    p { "You can change your answers here any time." }
                    div.lp-actions {
                        @if played {
                            a.hud-button.hud-primary href=(HOUSE) { "Back to the house" }
                        } @else {
                            a.hud-button.hud-primary href=(INTRO) { "Play the beta" }
                        }
                        a.hud-button.hud-ghost href="/" { "Back to OpenAgents" }
                    }
                }
            }
        },
    )
}

// --- Staff export --------------------------------------------------------

/// Staff only: signed out goes to sign-in, signed in without admin gets
/// the staff page, and a dashboard key works for scripts.
async fn staff(app: &App, headers: &HeaderMap, path: &str) -> Result<(), Response> {
    if crate::analytics::dashboard::allowed(app, headers).await {
        return Ok(());
    }
    use crate::pages::admin::{Visitor, guarded, sign_in, staff_only, visitor};
    match visitor(app, headers).await {
        Visitor::Admin => Ok(()),
        Visitor::Member => Err(guarded(staff_only(headers, path))),
        Visitor::SignedOut => Err(sign_in(path)),
    }
}

async fn admin_page(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(refused) = staff(&app, &headers, ADMIN).await {
        return refused;
    }
    let store = &app.config.chat_store;
    let count = |folder: &'static str| async move {
        store
            .folder_keys_limit(folder, 100_000)
            .await
            .map(|keys| keys.len().to_string())
            .unwrap_or_else(|_| "—".into())
    };
    let (waitlist, profiles, surveys) = tokio::join!(
        count(store::WAITLIST),
        count(store::PROFILES),
        count(store::SURVEYS)
    );
    let response = page(
        &app,
        StatusCode::OK,
        "Launch sign-ups",
        ADMIN,
        html! {
            main.lp-page #main {
                p.hud-label { "STAFF" }
                h1 { "Launch sign-ups" }
                div.lp-stats {
                    @for (label, count, file) in [("Waitlist", &waitlist, "waitlist"), ("Profiles", &profiles, "profiles"), ("Surveys", &surveys, "surveys")] {
                        div.hud-card.lp-stat {
                            span.hud-index { (label) }
                            strong { (count) }
                            a.hud-link href=(format!("{ADMIN}/{file}.csv")) { "Download CSV" }
                        }
                    }
                }
                p { a.hud-link href=(format!("{ADMIN}/export.json")) { "Download everything as JSON" } }
            }
        },
    );
    crate::pages::admin::guarded(response)
}

fn download(body: String, kind: &'static str, name: &str) -> Response {
    let mut response = (
        [
            (header::CONTENT_TYPE, kind),
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        body,
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    crate::pages::admin::guarded(response)
}

fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "The records couldn't be read. Try again.",
    )
        .into_response()
}

async fn export_json(State(app): State<App>, headers: HeaderMap) -> Response {
    let path = format!("{ADMIN}/export.json");
    if let Err(refused) = staff(&app, &headers, &path).await {
        return refused;
    }
    let store = &app.config.chat_store;
    let (waitlist, profiles, surveys) = tokio::join!(
        store::all::<store::WaitlistEntry>(store, store::WAITLIST),
        store::all::<Profile>(store, store::PROFILES),
        store::all::<Survey>(store, store::SURVEYS),
    );
    let (Ok(waitlist), Ok(profiles), Ok(surveys)) = (waitlist, profiles, surveys) else {
        return unavailable();
    };
    let body = serde_json::json!({
        "exported_unix": crate::chat_store::now_unix(),
        "waitlist": waitlist,
        "profiles": profiles,
        "surveys": surveys,
    });
    download(body.to_string(), "application/json", "launch-export.json")
}

/// One CSV cell: quoted when needed, and a leading `=`, `+`, `-`, or `@`
/// escaped so a spreadsheet never runs it as a formula.
fn cell(value: &str) -> String {
    let value = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

fn csv(header_row: &[&str], rows: impl Iterator<Item = Vec<String>>) -> String {
    let mut out = header_row.join(",");
    out.push('\n');
    for row in rows {
        out.push_str(&row.iter().map(|v| cell(v)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

async fn export_csv(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Path(file): axum::extract::Path<String>,
) -> Response {
    let path = format!("{ADMIN}/{file}");
    if !matches!(
        file.as_str(),
        "waitlist.csv" | "profiles.csv" | "surveys.csv"
    ) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Err(refused) = staff(&app, &headers, &path).await {
        return refused;
    }
    let store = &app.config.chat_store;
    let body = match file.as_str() {
        "waitlist.csv" => match store::all::<store::WaitlistEntry>(store, store::WAITLIST).await {
            Ok(rows) => csv(
                &["created_unix", "email", "role", "source"],
                rows.into_iter()
                    .map(|r| vec![r.created_unix.to_string(), r.email, r.role, r.source]),
            ),
            Err(_) => return unavailable(),
        },
        "profiles.csv" => match store::all::<Profile>(store, store::PROFILES).await {
            Ok(rows) => csv(
                &[
                    "created_unix",
                    "updated_unix",
                    "account_id",
                    "email",
                    "role",
                    "engines",
                    "experience",
                    "make",
                    "heard",
                    "contact_ok",
                    "session",
                ],
                rows.into_iter().map(|r| {
                    vec![
                        r.created_unix.to_string(),
                        r.updated_unix.to_string(),
                        r.account_id.unwrap_or_default(),
                        r.email,
                        r.role,
                        r.engines.join(" "),
                        r.experience,
                        r.make,
                        r.heard,
                        r.contact_ok.to_string(),
                        r.session.unwrap_or_default(),
                    ]
                }),
            ),
            Err(_) => return unavailable(),
        },
        _ => match store::all::<Survey>(store, store::SURVEYS).await {
            Ok(rows) => csv(
                &[
                    "created_unix",
                    "session",
                    "ease",
                    "finished",
                    "confused",
                    "make",
                    "again",
                    "device",
                    "browser",
                    "email",
                    "contact_ok",
                    "account_id",
                    "source",
                    "recorded",
                ],
                rows.into_iter().map(|r| {
                    vec![
                        r.created_unix.to_string(),
                        r.session,
                        r.ease.to_string(),
                        r.finished,
                        r.confused,
                        r.make,
                        r.again.to_string(),
                        r.device,
                        r.browser,
                        r.email,
                        r.contact_ok.to_string(),
                        r.account_id.unwrap_or_default(),
                        r.source,
                        r.recorded.to_string(),
                    ]
                }),
            ),
            Err(_) => return unavailable(),
        },
    };
    download(body, "text/csv; charset=utf-8", &file)
}
