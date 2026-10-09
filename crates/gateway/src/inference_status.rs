//! The inference meter's admin status (`docs/inference/gateway.md`,
//! sections 5 and 6): per-account credit burn-downs, the alerts that hold
//! now, and live rates per upstream and model.
//!
//! `GET /v1/admin/inference/status` answers only a bearer equal to the
//! environment variable `inference.admin_token_env` names; with that
//! variable unset it refuses everyone. Mounted only when `inference` is
//! configured. The meter itself raises alerts as log lines; this route
//! is where an operator reads the same state.
//!
//! `GET /admin/inference` is the same state as a page: each credit
//! account's burn-down (balance, spend today, seven-day average, days
//! left, days to expiry), the alerts that hold, and the last hour's live
//! rates per upstream and model. A browser pastes the admin token once
//! (`POST /admin/inference/session` keeps it in an `HttpOnly` cookie).

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use serde_json::json;

use crate::serve::ServeState;

/// The status route's path.
pub const PATH: &str = "/v1/admin/inference/status";
pub const OUTCOMES: &str = "/v1/admin/inference/outcomes";

/// How often the meter's daily hook runs (it reconciles once a day and
/// checks time-based alerts such as credit nearing expiry).
const TICK: Duration = Duration::from_secs(3_600);

/// The operator page's path.
pub const PAGE: &str = "/admin/inference";
/// Where the page's token form posts.
pub const SESSION: &str = "/admin/inference/session";
/// The cookie the pasted admin token rides in.
const COOKIE: &str = "oa_inference_admin";

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        (PATH, get(status)),
        (OUTCOMES, axum::routing::post(outcome)),
        (PAGE, get(page)),
        (SESSION, axum::routing::post(session)),
    ]
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_millis() as u64)
        .unwrap_or_default()
}

/// Start the hourly tick inside the runtime. No provider billing source
/// is wired yet; the adapters (#11061) bring them.
pub(crate) fn resume(state: &Arc<ServeState>) {
    let Some(meter) = state.meter.clone() else {
        return;
    };
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        let mut every = tokio::time::interval(TICK);
        loop {
            every.tick().await;
            meter.tick(&[], now_ms());
        }
    });
}

async fn outcome(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !authorized(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":{"message":"An admin token is required."}})),
        )
            .into_response();
    }
    let Some(book) = state
        .inference
        .as_ref()
        .and_then(|gateway| gateway.outcomes())
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"message":"Paid outcomes are not set up here."}})),
        )
            .into_response();
    };
    if body.len() > 8192 {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":{"message":"This outcome is too large."}})),
        )
            .into_response();
    }
    let receipt = match serde_json::from_slice::<inference::outcomes::Receipt>(&body) {
        Ok(receipt) => receipt,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":{"message":"Send a signed paid-work outcome."}})),
            )
                .into_response();
        }
    };
    let mode = receipt.outcome.mode;
    match book.record(receipt) {
        Ok(recorded) => Json(json!({"recorded":recorded, "mode":mode, "counts_toward_quality":mode == inference::outcomes::Mode::Real})).into_response(),
        Err(message) => {
            let status = if book.summary().is_err() { StatusCode::SERVICE_UNAVAILABLE } else { StatusCode::BAD_REQUEST };
            (status, Json(json!({"error":{"message":message}}))).into_response()
        },
    }
}

fn expected(state: &ServeState) -> Option<String> {
    let config = state.config.inference.as_ref()?;
    std::env::var(&config.admin_token_env)
        .ok()
        .filter(|token| !token.is_empty())
}

fn same(given: &str, expected: &str) -> bool {
    let (a, b) = (given.as_bytes(), expected.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn authorized(state: &ServeState, headers: &HeaderMap) -> bool {
    let Some(expected) = expected(state) else {
        return false;
    };
    let Some(given) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return false;
    };
    same(given, &expected)
}

/// The admin token from the page's cookie, checked.
fn cookie_authorized(state: &ServeState, headers: &HeaderMap) -> bool {
    let Some(expected) = expected(state) else {
        return false;
    };
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .any(|(name, value)| name == COOKIE && same(value, &expected))
}

#[derive(serde::Deserialize)]
struct SessionForm {
    token: String,
}

async fn session(
    State(state): State<Arc<ServeState>>,
    axum::extract::Form(form): axum::extract::Form<SessionForm>,
) -> Response {
    let token = form.token.trim();
    let ok = expected(&state).is_some_and(|expected| same(token, &expected));
    if !ok
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b';')
    {
        return crate::dashboard::page_error(
            StatusCode::UNAUTHORIZED,
            "Inference",
            "That token isn't the admin token.",
        );
    }
    let mut response = axum::response::Redirect::to(PAGE).into_response();
    if let Ok(value) = axum::http::HeaderValue::from_str(&format!(
        "{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/admin/inference"
    )) {
        response.headers_mut().insert("set-cookie", value);
    }
    response
}

fn dollars(micros: u64) -> String {
    format!("${}", inference::router::micros_usd(micros))
}

fn days(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |days| format!("{days:.1}"))
}

async fn page(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    use crate::dashboard::esc;
    if !(authorized(&state, &headers) || cookie_authorized(&state, &headers)) {
        let body = format!(
            r#"<h1>Inference</h1><p>Paste the admin token to see credit burn-downs and live rates.</p>
<form method="post" action="{SESSION}"><label>Admin token <input type="password" name="token" autocomplete="off"></label> <button type="submit">Open</button></form>"#
        );
        return (
            StatusCode::UNAUTHORIZED,
            crate::dashboard::page("Inference", None, &body),
        )
            .into_response();
    }
    let Some(status) = state
        .meter
        .as_ref()
        .and_then(|meter| meter.status(now_ms()))
    else {
        return crate::dashboard::page_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Inference",
            "The meter isn't available.",
        );
    };
    let mut body = String::from("<h1>Inference</h1>");
    body.push_str("<h2>Credit burn-down</h2><table><tr><th>Account</th><th>Upstream</th><th>Kind</th><th>Balance</th><th>Left</th><th>Spent today</th><th>7-day average</th><th>Days left</th><th>Days to expiry</th></tr>");
    for account in &status.accounts {
        let kind = match account.basis {
            inference::meter::Basis::Prepaid => "prepaid credit",
            inference::meter::Basis::FreeCapacity => "free capacity",
            inference::meter::Basis::PayAsYouGo => "pay as you go",
        };
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{kind}</td><td>{}</td><td>{:.1}%</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&account.account),
            esc(&account.upstream),
            dollars(account.balance),
            account.remaining_pct,
            dollars(account.spent_today),
            dollars(account.average_daily),
            days(account.days_left),
            days(account.days_to_expiry),
        ));
    }
    if status.accounts.is_empty() {
        body.push_str(
            "<tr><td colspan=\"9\" class=\"dim\">No credit accounts are configured.</td></tr>",
        );
    }
    body.push_str("</table>");
    body.push_str("<h2>Alerts</h2>");
    if status.alerts.is_empty() {
        body.push_str("<p class=\"dim\">None.</p>");
    } else {
        body.push_str("<ul>");
        for alert in &status.alerts {
            let line = serde_json::to_string(alert).unwrap_or_default();
            body.push_str(&format!("<li><code>{}</code></li>", esc(&line)));
        }
        body.push_str("</ul>");
    }
    for window in ["1h", "24h"] {
        body.push_str(&format!(
            "<h2>Last {window}</h2><table><tr><th>Upstream</th><th>Model</th><th>Attempts</th><th>Errors</th><th>First token p50</th><th>p90</th><th>Tokens</th><th>Cost</th></tr>"
        ));
        let rates = status.rates.get(window).cloned().unwrap_or_default();
        for rate in &rates {
            let ms =
                |value: Option<u64>| value.map_or_else(|| "—".to_owned(), |ms| format!("{ms} ms"));
            body.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&rate.upstream),
                esc(&rate.model),
                rate.attempts,
                rate.errors,
                ms(rate.ttft_p50_ms),
                ms(rate.ttft_p90_ms),
                rate.tokens,
                dollars(rate.cost),
            ));
        }
        if rates.is_empty() {
            body.push_str("<tr><td colspan=\"8\" class=\"dim\">No requests yet.</td></tr>");
        }
        body.push_str("</table>");
    }
    body.push_str(&format!(
        "<p class=\"dim\">{} attempts kept. <a href=\"{PATH}\">JSON</a> needs the token as a bearer.</p>",
        status.records.kept
    ));
    crate::dashboard::page("Inference", None, &body).into_response()
}

async fn status(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }
    match state
        .meter
        .as_ref()
        .and_then(|meter| meter.status(now_ms()))
    {
        Some(status) => Json(status).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "meter unavailable"})),
        )
            .into_response(),
    }
}
