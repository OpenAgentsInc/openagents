//! The customer dashboard — a member's usage, members, keys, and
//! billing as server-rendered pages.
//!
//! A browser signs in by pasting its `sess_` session token; the
//! dashboard stores it in an `HttpOnly` cookie and resolves it through
//! the same [`accounts::principal`] path the JSON API uses, so a page
//! never checks a weaker rule than the API it renders. Every
//! workspace page re-runs the membership check — a revoked member's
//! next page load refuses, and nothing here caches an authorization.
//!
//! The pages render only references: key ids, request ids, digests,
//! and amounts. Raw request state, answer payloads, and key secrets
//! never appear — the receipt's digests stand in for content.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Form, Path, Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{MethodRouter, get, post};
use serde::Deserialize;
use tenancy::accounts::Role;
use tenancy::keys;
use tenancy::money::Phase;

use crate::accounts::{self, unix_now};
use crate::serve::{self, ServeState};
use crate::usage::{self, Filter};

/// The cookie the session token rides in — `HttpOnly`, `SameSite=Lax`,
/// no `Secure` flag so a localhost deployment still works; a TLS
/// deployment puts the flag on at its terminator or serves the token
/// through the API instead.
const COOKIE: &str = "oa_session";

/// The dashboard's pages — mounted beside the usage routes only when
/// `accounts` is configured.
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/dashboard", get(home)),
        ("/dashboard/session", post(session)),
        ("/dashboard/sign-out", post(sign_out)),
        ("/dashboard/w/{workspace}", get(overview)),
        ("/dashboard/w/{workspace}/usage", get(usage_page)),
        ("/dashboard/w/{workspace}/activity", get(activity_page)),
        (
            "/dashboard/w/{workspace}/receipts/{digest}",
            get(receipt_page),
        ),
        ("/dashboard/w/{workspace}/members", get(members_page)),
        ("/dashboard/w/{workspace}/keys", get(keys_page)),
        ("/dashboard/w/{workspace}/billing", get(billing_page)),
    ]
}

/// HTML-escape every value a page interpolates — receipts carry
/// caller-chosen strings (request ids, model names) that must never
/// render as markup.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A fixed-point millionths amount as a decimal string.
fn amount(value: u64) -> String {
    let whole = value / 1_000_000;
    let fraction = format!("{:06}", value % 1_000_000);
    let trimmed = fraction.trim_end_matches('0');
    if trimmed.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{trimmed}")
    }
}

/// The page shell — one stylesheet, the amber-on-dark terminal theme
/// the rest of the product uses, semantic markup a reader navigates.
fn page(title: &str, workspace: Option<&str>, body: &str) -> Html<String> {
    let nav = match workspace {
        Some(ws) => {
            let ws = esc(ws);
            format!(
                r#"<nav aria-label="Workspace"><a href="/dashboard">workspaces</a> / <strong>{ws}</strong>
                <span class="tabs"><a href="/dashboard/w/{ws}">overview</a><a href="/dashboard/w/{ws}/usage">usage</a><a href="/dashboard/w/{ws}/activity">activity</a><a href="/dashboard/w/{ws}/members">members</a><a href="/dashboard/w/{ws}/keys">keys</a><a href="/dashboard/w/{ws}/billing">billing</a></span></nav>
                <form method="post" action="/dashboard/sign-out" class="out"><button type="submit">sign out</button></form>"#
            )
        }
        None => String::new(),
    };
    Html(format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · OpenAgents</title>
<style>
:root {{ color-scheme: dark; }}
body {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: #0d0a08; color: #e8dcc8; margin: 0; padding: 1rem; max-width: 72rem; }}
a {{ color: #ffb454; }}
h1 {{ font-size: 1.2rem; color: #ffb454; }} h2 {{ font-size: 1rem; color: #d9973b; }}
nav {{ margin-bottom: .5rem; }}
nav .tabs a {{ margin-right: .8rem; }}
table {{ border-collapse: collapse; width: 100%; font-size: .85rem; margin: .5rem 0 1.5rem; }}
th, td {{ text-align: left; padding: .3rem .6rem; border-bottom: 1px solid #3a2f22; }}
th {{ color: #d9973b; font-weight: normal; }}
code {{ color: #ffd9a0; word-break: break-all; }}
.cards {{ display: flex; flex-wrap: wrap; gap: .8rem; margin: .8rem 0 1.5rem; }}
.card {{ border: 1px solid #3a2f22; padding: .6rem .9rem; min-width: 9rem; }}
.card .n {{ font-size: 1.4rem; color: #ffb454; }} .card .l {{ font-size: .75rem; color: #a08a68; }}
.bar {{ display: flex; align-items: flex-end; height: 6rem; gap: 2px; margin: .5rem 0 1rem; }}
.bar div {{ background: #d9973b; min-width: 10px; flex: 1; }}
.bar div:hover {{ background: #ffb454; }}
.bars {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(10px, 1fr)); }}
label {{ display: inline-block; font-size: .8rem; color: #a08a68; margin-right: .8rem; }}
input, select {{ background: #1a1410; color: #e8dcc8; border: 1px solid #3a2f22; padding: .3rem .5rem; font: inherit; }}
button {{ background: #d9973b; color: #0d0a08; border: 0; padding: .35rem .9rem; font: inherit; cursor: pointer; }}
.out {{ display: inline; }} .out button {{ background: none; color: #a08a68; text-decoration: underline; padding: 0; }}
.err {{ color: #ff7a5c; }} .dim {{ color: #a08a68; font-size: .8rem; }}
form.filters {{ margin: .5rem 0 1rem; }}
@media (max-width: 40rem) {{ th, td {{ padding: .2rem .3rem; font-size: .78rem; }} }}
</style></head><body>{nav}<main>{body}</main></body></html>"#,
        title = esc(title),
    ))
}

/// An HTML error page — the dashboard answers a failure in the medium
/// the browser asked for, with the same status the API would return.
fn page_error(status: StatusCode, title: &str, detail: &str) -> Response {
    (
        status,
        page(
            title,
            None,
            &format!(
                r#"<h1>{title}</h1><p class="err">{}</p><p><a href="/dashboard">back to sign-in</a></p>"#,
                esc(detail)
            ),
        ),
    )
        .into_response()
}

/// The session cookie's token, when one rides the request.
fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for pair in header.split(';') {
        let pair = pair.trim();
        if let Some(token) = pair.strip_prefix(&format!("{COOKIE}="))
            && !token.is_empty()
        {
            return Some(token.to_string());
        }
    }
    None
}

/// Resolve the cookie to a principal — by handing the token to the
/// same `principal()` path an `Authorization` header takes, so a
/// closed or anonymous session refuses exactly as it would over JSON.
fn principal_of(state: &ServeState, headers: &HeaderMap) -> Result<accounts::Principal, Response> {
    let token = cookie_token(headers).ok_or_else(|| {
        page_error(
            StatusCode::UNAUTHORIZED,
            "sign in required",
            "no session cookie — sign in with a session token",
        )
    })?;
    let mut forwarded = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
        forwarded.insert(axum::http::header::AUTHORIZATION, value);
    }
    accounts::principal(state, &forwarded).map_err(|response| {
        page_error(
            response.status(),
            "sign in refused",
            "the session token did not resolve to an active account session — sign in again",
        )
    })
}

/// Authorize a workspace page — cookie principal, then membership.
/// The account id comes back so the page can mark the caller's own row.
fn member_of(state: &ServeState, headers: &HeaderMap, workspace: &str) -> Result<String, Response> {
    let principal = principal_of(state, headers)?;
    let account = accounts::member_account(&principal)
        .map_err(|response| {
            page_error(
                response.status(),
                "no account",
                "an anonymous session has no workspace view — sign in with a user session",
            )
        })?
        .to_string();
    accounts::member(state, &account, workspace).map_err(|response| {
        page_error(
            response.status(),
            "not a member",
            "this account is not an active member of the workspace",
        )
    })?;
    Ok(account)
}

/// `GET /dashboard` — the workspace picker under a session, or the
/// sign-in form without one.
async fn home(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let Ok(principal) = principal_of(&state, &headers) else {
        return page(
            "sign in",
            None,
            r#"<h1>OpenAgents dashboard</h1>
            <p>Paste a session token (<code>sess_…</code>) to view your workspaces.</p>
            <form method="post" action="/dashboard/session">
            <label>session token <input name="token" type="password" size="48" required autocomplete="off"></label>
            <button type="submit">sign in</button></form>
            <p class="dim">Mint a session with <code>POST /v1/sessions</code> — the token is stored
            in an HttpOnly cookie and can be revoked by signing out or closing the session.</p>"#,
        )
        .into_response();
    };
    let Some(account_id) = principal.account().map(|account| account.to_string()) else {
        return page_error(
            StatusCode::FORBIDDEN,
            "no account",
            "an anonymous session has no workspace view — sign in with a user session",
        );
    };
    let accounts = match accounts::accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => {
            return page_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts unavailable",
                &trouble.to_string(),
            );
        }
    };
    let mut rows = String::new();
    for (id, ws) in &store.workspaces {
        let Some(membership) = ws.members.get(&account_id) else {
            continue;
        };
        if membership.status != tenancy::accounts::MemberStatus::Active {
            continue;
        }
        let kind = match ws.kind {
            tenancy::accounts::WorkspaceKind::Personal => "personal",
            tenancy::accounts::WorkspaceKind::Organization => "organization",
        };
        rows.push_str(&format!(
            r#"<tr><td><a href="/dashboard/w/{id}"><code>{id}</code></a></td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
            esc(&ws.name),
            kind,
            membership.role,
            id = esc(id),
        ));
    }
    let body = if rows.is_empty() {
        format!(
            r#"<h1>workspaces</h1><p class="dim">{} holds no active workspace membership.</p>"#,
            esc(&account_id)
        )
    } else {
        format!(
            r#"<h1>workspaces</h1><table><tr><th>id</th><th>name</th><th>kind</th><th>role</th></tr>{rows}</table>"#
        )
    };
    page("workspaces", None, &body).into_response()
}

#[derive(Deserialize)]
struct SessionForm {
    token: String,
}

/// `POST /dashboard/session` — validate the pasted token through the
/// real principal path, then set the cookie only on success.
async fn session(State(state): State<Arc<ServeState>>, Form(form): Form<SessionForm>) -> Response {
    let mut forwarded = HeaderMap::new();
    let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", form.token.trim())) else {
        return page_error(
            StatusCode::BAD_REQUEST,
            "bad token",
            "the token is not a valid header value",
        );
    };
    forwarded.insert(axum::http::header::AUTHORIZATION, value);
    if let Err(response) = accounts::principal(&state, &forwarded) {
        return page_error(
            response.status(),
            "sign in refused",
            "the token did not resolve — check it is a live session token",
        );
    }
    let cookie = format!(
        "{COOKIE}={}; Path=/; HttpOnly; SameSite=Lax",
        form.token.trim()
    );
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header("location", "/dashboard")
        .header(
            SET_COOKIE,
            HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
        )
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// `POST /dashboard/sign-out` — expire the cookie. The session itself
/// stays live; `POST /v1/sessions/{id}/close` ends it.
async fn sign_out() -> Response {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header("location", "/dashboard")
        .header(
            SET_COOKIE,
            HeaderValue::from_static("oa_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
        )
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// `GET /dashboard/w/{ws}` — the workspace at a glance: standing,
/// balance, today's calls, outstanding reservations, and the
/// subscription's state.
async fn overview(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let account = match member_of(&state, &headers, &workspace) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let accounts = match accounts::accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => {
            return page_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts unavailable",
                &trouble.to_string(),
            );
        }
    };
    let Some(ws) = store.workspaces.get(&workspace) else {
        return page_error(
            StatusCode::NOT_FOUND,
            "unknown workspace",
            "the workspace is gone",
        );
    };
    let role = ws
        .members
        .get(&account)
        .map(|membership| membership.role.to_string())
        .unwrap_or_else(|| "member".to_string());

    // Exact ledger reads — the same numbers the JSON summary reports.
    let balance_html = match state.money_lock().await {
        Some(ledger) => {
            let balance = match ledger.balance(&workspace) {
                Ok(balance) => balance,
                Err(cause) => {
                    return page_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "balance unavailable",
                        &cause,
                    );
                }
            };
            let outstanding = ledger
                .holds(&workspace)
                .into_iter()
                .filter(|(_, hold)| matches!(hold.phase, Phase::Held | Phase::Unknown))
                .map(|(attempt, hold)| {
                    format!(
                        r#"<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>"#,
                        esc(attempt),
                        amount(hold.reserved),
                        match hold.phase {
                            Phase::Held => "held",
                            Phase::Unknown => "unknown",
                            Phase::Settled => "settled",
                            Phase::Released => "released",
                        },
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<div class="cards">
                <div class="card"><div class="n">{}</div><div class="l">credited {}</div></div>
                <div class="card"><div class="n">{}</div><div class="l">reserved</div></div>
                <div class="card"><div class="n">{}</div><div class="l">settled spend</div></div>
                <div class="card"><div class="n">{}</div><div class="l">available</div></div></div>
                <h2>outstanding reservations</h2>{}"#,
                amount(balance.credited),
                esc(&balance.currency),
                amount(balance.reserved),
                amount(balance.settled),
                amount(balance.available),
                if outstanding.is_empty() {
                    r#"<p class="dim">no outstanding reservations</p>"#.to_string()
                } else {
                    format!(
                        r#"<table><tr><th>hold</th><th>reserved</th><th>phase</th></tr>{outstanding}</table>"#
                    )
                },
            )
        }
        None => {
            r#"<p class="dim">this deployment runs no monetary admission — usage is unpriced</p>"#
                .to_string()
        }
    };

    let today = serve::now_utc()[..10].to_string();
    let filter = Filter {
        from: Some(today.clone()),
        ..Filter::default()
    };
    let today_holds = usage_holds(&state, &workspace).await;
    let today_calls = usage::scan(&state, &workspace, &filter, &today_holds)
        .map(|scan| scan.receipts.len())
        .unwrap_or(0);

    page(
        &format!("{} · overview", ws.name),
        Some(&workspace),
        &format!(
            r#"<h1>{} <span class="dim">{}</span></h1>
            <p class="dim">signed in as <code>{}</code> · role {} · today <code>{}</code>: {} calls</p>
            {balance_html}"#,
            esc(&ws.name),
            esc(&workspace),
            esc(&account),
            esc(&role),
            esc(&today),
            today_calls,
        ),
    )
    .into_response()
}

/// `GET /dashboard/w/{ws}/usage` — the summary's numbers plus a
/// per-day call chart, both folded from the same receipts the JSON
/// reads report.
async fn usage_page(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    if let Err(response) = member_of(&state, &headers, &workspace) {
        return response;
    }
    let holds = usage_holds(&state, &workspace).await;
    let scan = match usage::scan(&state, &workspace, &filter, &holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    let quota = state.quota_lock().await;

    let mut retail = 0u64;
    let mut questions = 0u64;
    let mut input_bytes = 0u64;
    let mut days: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for receipt in &scan.receipts {
        if let Some(hold) = holds.get(&format!("{}#{}", receipt.request, receipt.attempt))
            && let Some(charge) = hold.retail_charge
        {
            retail += charge;
        }
        if let Some(reservation) = quota.reservation(&receipt.request, receipt.attempt) {
            questions += reservation.units.questions;
            input_bytes += reservation.units.input_bytes;
        }
        if let Some(at) = &receipt.timing.resolved_at {
            let day = at[..10.min(at.len())].to_string();
            let entry = days.entry(day).or_default();
            entry.0 += 1;
        }
    }
    let answered = scan
        .receipts
        .iter()
        .filter(|receipt| receipt.outcome == receipts::execution::Outcome::Answered)
        .count();

    let max = days
        .values()
        .map(|(calls, _)| *calls)
        .max()
        .unwrap_or(1)
        .max(1);
    let bars = days
        .iter()
        .map(|(day, (calls, _))| {
            let height = (calls * 100 / max).max(4);
            format!(
                r#"<div style="height:{height}%" title="{day}: {calls} calls"></div>"#,
                day = esc(day),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let day_labels = days
        .keys()
        .map(|day| {
            format!(
                r#"<span class="dim" style="font-size:.6rem">{}</span> "#,
                &day[5..]
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let window = format!(
        r#"<form method="get" class="filters"><label>from <input name="from" value="{}" placeholder="2026-01-01"></label>
        <label>to <input name="to" value="{}" placeholder="2026-02-01"></label>
        <label>model <input name="model" value="{}" size="12"></label>
        <label>outcome <select name="outcome"><option value="">any</option>{}</select></label>
        <button type="submit">filter</button> <a href="/dashboard/w/{}/usage">reset</a></form>"#,
        esc(filter.from.as_deref().unwrap_or("")),
        esc(filter.to.as_deref().unwrap_or("")),
        esc(filter.model.as_deref().unwrap_or("")),
        [
            "answered",
            "refused",
            "unavailable",
            "unattempted",
            "unknown"
        ]
        .iter()
        .map(|name| {
            let selected = if filter.outcome.as_deref() == Some(*name) {
                " selected"
            } else {
                ""
            };
            format!(r#"<option value="{name}"{selected}>{name}</option>"#)
        })
        .collect::<Vec<_>>()
        .join(""),
        esc(&workspace),
    );

    page(
        "usage",
        Some(&workspace),
        &format!(
            r#"<h1>usage</h1>{window}
            <div class="cards">
            <div class="card"><div class="n">{}</div><div class="l">calls</div></div>
            <div class="card"><div class="n">{}</div><div class="l">answered</div></div>
            <div class="card"><div class="n">{}</div><div class="l">questions</div></div>
            <div class="card"><div class="n">{}</div><div class="l">input bytes</div></div>
            <div class="card"><div class="n">{}</div><div class="l">retail spend</div></div></div>
            <h2>calls per day <span class="dim">UTC</span></h2>
            <div class="bar">{bars}</div><div>{day_labels}</div>
            <p class="dim">exact sums over {} receipts — unattributed {}, other-workspace {}, unverifiable {}</p>"#,
            scan.receipts.len(),
            answered,
            questions,
            input_bytes,
            amount(retail),
            scan.receipts.len(),
            scan.unattributed,
            scan.other_workspace,
            scan.unverifiable,
        ),
    )
    .into_response()
}

/// The workspace's holds as an owned map — the dashboard joins the
/// same `{request}#{attempt}` key the JSON summary uses.
async fn usage_holds(
    state: &ServeState,
    workspace: &str,
) -> BTreeMap<String, tenancy::money::Hold> {
    match state.money_lock().await {
        Some(ledger) => ledger
            .holds(workspace)
            .into_iter()
            .map(|(attempt, hold)| (attempt.to_string(), hold.clone()))
            .collect(),
        None => BTreeMap::new(),
    }
}

/// `GET /dashboard/w/{ws}/activity` — the receipt table, newest first,
/// with the filters the JSON activity read takes.
async fn activity_page(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    if let Err(response) = member_of(&state, &headers, &workspace) {
        return response;
    }
    let activity_holds = usage_holds(&state, &workspace).await;
    let mut scan = match usage::scan(&state, &workspace, &filter, &activity_holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    scan.receipts.sort_by(|a, b| {
        b.timing
            .resolved_at
            .cmp(&a.timing.resolved_at)
            .then_with(|| b.digest.cmp(&a.digest))
    });
    let limit = filter.limit.unwrap_or(50).clamp(1, 200);
    let page_rows: Vec<_> = scan.receipts.iter().take(limit + 1).collect();
    let has_more = page_rows.len() > limit;
    let rows = page_rows
        .iter()
        .take(limit)
        .map(|receipt| {
            let outcome = match receipt.outcome {
                receipts::execution::Outcome::Answered => "answered",
                receipts::execution::Outcome::Refused => "refused",
                receipts::execution::Outcome::Unavailable => "unavailable",
                receipts::execution::Outcome::Unattempted => "unattempted",
                receipts::execution::Outcome::Unknown => "unknown",
            };
            format!(
                r#"<tr><td><a href="/dashboard/w/{ws}/receipts/{digest}"><code>{short}</code></a></td>
                <td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                esc(receipt.timing.resolved_at.as_deref().unwrap_or("—")),
                esc(&receipt.served.model),
                outcome,
                esc(receipt.tenant.as_deref().unwrap_or("—")),
                receipt
                    .timing
                    .latency_ms
                    .map(|ms| ms.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                ws = esc(&workspace),
                digest = esc(&receipt.digest),
                short = esc(&receipt.digest[..16.min(receipt.digest.len())]),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let more = if has_more {
        format!(
            r#"<p><a href="/dashboard/w/{}/activity?limit={}">show {} more</a></p>"#,
            esc(&workspace),
            limit + 50,
            50,
        )
    } else {
        String::new()
    };
    page(
        "activity",
        Some(&workspace),
        &format!(
            r#"<h1>activity</h1>
            <p class="dim">newest first · {} receipts in window{}</p>
            <table><tr><th>receipt</th><th>resolved (UTC)</th><th>model</th><th>outcome</th><th>key</th><th>ms</th></tr>{}</table>{}
            <p><a href="/v1/workspaces/{}/usage/export">export NDJSON</a> <span class="dim">— the sealed receipts, verbatim</span></p>"#,
            scan.receipts.len(),
            if scan.truncated { " (scan truncated)" } else { "" },
            rows,
            more,
            esc(&workspace),
        ),
    )
    .into_response()
}

/// `GET /dashboard/w/{ws}/receipts/{digest}` — one receipt's fields,
/// joined to its hold and quota units. Digests and references render;
/// request and result bodies never do — the receipt never held them.
async fn receipt_page(
    State(state): State<Arc<ServeState>>,
    Path((workspace, digest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = member_of(&state, &headers, &workspace) {
        return response;
    }
    let receipt_holds = usage_holds(&state, &workspace).await;
    let scan = match usage::scan(&state, &workspace, &Filter::default(), &receipt_holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    let Some(receipt) = scan.receipts.iter().find(|r| r.digest == digest) else {
        return page_error(
            StatusCode::NOT_FOUND,
            "unknown receipt",
            "no receipt of this workspace carries that digest",
        );
    };
    let holds = usage_holds(&state, &workspace).await;
    let hold = holds.get(&format!("{}#{}", receipt.request, receipt.attempt));
    let quota = state.quota_lock().await;
    let units = quota
        .reservation(&receipt.request, receipt.attempt)
        .map(|reservation| reservation.units.clone());
    let cost_html = match hold {
        Some(hold) => format!(
            r#"<tr><th>cost</th><td>retail {} · provider {} · hosting {} · refunded {} · {}</td></tr>"#,
            hold.retail_charge.map(amount).unwrap_or_else(|| "—".into()),
            hold.provider_cost.map(amount).unwrap_or_else(|| "—".into()),
            hold.hosting_cost.map(amount).unwrap_or_else(|| "—".into()),
            amount(hold.refunded),
            match hold.phase {
                Phase::Held => "held",
                Phase::Unknown => "unknown",
                Phase::Settled => "settled",
                Phase::Released => "released",
            },
        ),
        None => r#"<tr><th>cost</th><td class="dim">unpriced — no monetary admission</td></tr>"#
            .to_string(),
    };
    let units_html = match units {
        Some(units) => format!(
            r#"<tr><th>quota units</th><td>{} questions · {} input bytes · {} options</td></tr>"#,
            units.questions, units.input_bytes, units.options,
        ),
        None => r#"<tr><th>quota units</th><td class="dim">no reservation recorded</td></tr>"#
            .to_string(),
    };
    let outcome = match receipt.outcome {
        receipts::execution::Outcome::Answered => "answered",
        receipts::execution::Outcome::Refused => "refused",
        receipts::execution::Outcome::Unavailable => "unavailable",
        receipts::execution::Outcome::Unattempted => "unattempted",
        receipts::execution::Outcome::Unknown => "unknown",
    };
    page(
        "receipt",
        Some(&workspace),
        &format!(
            r#"<h1>receipt <code>{}</code></h1>
            <table>
            <tr><th>request</th><td><code>{}</code> attempt {}</td></tr>
            <tr><th>attempt id</th><td><code>{}</code></td></tr>
            <tr><th>transport</th><td>{}</td></tr>
            <tr><th>key</th><td><code>{}</code></td></tr>
            <tr><th>requested model</th><td><code>{}</code></td></tr>
            <tr><th>served model</th><td><code>{}</code></td></tr>
            <tr><th>served artifact</th><td><code>{}</code></td></tr>
            <tr><th>outcome</th><td>{}{}</td></tr>
            <tr><th>timing</th><td>queued {} ms · served {} ms · resolved {}</td></tr>
            <tr><th>request digest</th><td><code>{}</code></td></tr>
            <tr><th>result digest</th><td><code>{}</code></td></tr>
            {units_html}{cost_html}
            <tr><th>receipt digest</th><td><code>{}</code></td></tr>
            </table>
            <p class="dim">raw request state and answers are not held by the receipt — the digests attest them.</p>"#,
            esc(&receipt.digest),
            esc(&receipt.request),
            receipt.attempt,
            esc(&receipt.attempt_id),
            esc(&receipt.transport),
            esc(receipt.tenant.as_deref().unwrap_or("—")),
            esc(&receipt.requested.model),
            esc(&receipt.served.model),
            esc(&receipt.served.artifact_signature),
            outcome,
            receipt
                .cause
                .as_ref()
                .map(|cause| format!(" · {}", esc(cause)))
                .unwrap_or_default(),
            receipt
                .timing
                .queued_ms
                .map(|ms| ms.to_string())
                .unwrap_or_else(|| "—".to_string()),
            receipt
                .timing
                .latency_ms
                .map(|ms| ms.to_string())
                .unwrap_or_else(|| "—".to_string()),
            esc(receipt.timing.resolved_at.as_deref().unwrap_or("—")),
            esc(&receipt.request_digest),
            esc(receipt.result_digest.as_deref().unwrap_or("—")),
            esc(&receipt.digest),
        ),
    )
    .into_response()
}

/// `GET /dashboard/w/{ws}/members` — the roster and the live
/// invitations. Reads only; membership changes go through the JSON API.
async fn members_page(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let account = match member_of(&state, &headers, &workspace) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let accounts = match accounts::accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => {
            return page_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts unavailable",
                &trouble.to_string(),
            );
        }
    };
    let Some(ws) = store.workspaces.get(&workspace) else {
        return page_error(
            StatusCode::NOT_FOUND,
            "unknown workspace",
            "the workspace is gone",
        );
    };
    let caller_admin = ws
        .members
        .get(&account)
        .is_some_and(|membership| membership.role >= Role::Admin);
    let rows = ws
        .members
        .iter()
        .map(|(id, membership)| {
            let status = if membership.status == tenancy::accounts::MemberStatus::Active {
                "active"
            } else {
                "revoked"
            };
            let you = if id == &account { " · you" } else { "" };
            format!(
                r#"<tr><td><code>{}</code>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                esc(id),
                you,
                membership.role,
                status,
                esc(&membership.granted),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let now = unix_now();
    let invites = store
        .invitations
        .values()
        .filter(|invite| {
            invite.workspace == workspace
                && invite.status == tenancy::accounts::InviteStatus::Pending
                && invite.expires_unix > now
        })
        .map(|invite| {
            format!(
                r#"<tr><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>"#,
                esc(&invite.id),
                invite.role,
                esc(&invite.invited_by),
                invite.expires_unix,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let seats = ws
        .seats
        .map(|seats| format!(" · seats {seats}"))
        .unwrap_or_default();
    page(
        "members",
        Some(&workspace),
        &format!(
            r#"<h1>members</h1><p class="dim">epoch {}{}</p>
            <table><tr><th>account</th><th>role</th><th>status</th><th>granted</th></tr>{rows}</table>
            <h2>pending invitations</h2>{}
            <p class="dim">member and invitation changes go through the JSON API{}.</p>"#,
            ws.members_epoch,
            seats,
            if invites.is_empty() {
                r#"<p class="dim">none</p>"#.to_string()
            } else {
                format!(r#"<table><tr><th>id</th><th>role</th><th>principal</th><th>expires</th></tr>{invites}</table>"#)
            },
            if caller_admin {
                " — your role permits them"
            } else {
                " — an admin or owner runs them"
            },
        ),
    )
    .into_response()
}

/// `GET /dashboard/w/{ws}/keys` — the workspace's credentials: admins
/// see every key bound to the workspace tenant, members see their own.
/// Id, name, status, scopes — never a secret; none is stored to show.
async fn keys_page(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let account = match member_of(&state, &headers, &workspace) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let accounts = match accounts::accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => {
            return page_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts unavailable",
                &trouble.to_string(),
            );
        }
    };
    let Some(ws) = store.workspaces.get(&workspace) else {
        return page_error(
            StatusCode::NOT_FOUND,
            "unknown workspace",
            "the workspace is gone",
        );
    };
    let admin = ws
        .members
        .get(&account)
        .is_some_and(|membership| membership.role >= Role::Admin);
    let key_store = match keys::load(&state.dir) {
        Ok(store) => store,
        Err(trouble) => {
            return page_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "keys unavailable",
                &trouble.to_string(),
            );
        }
    };
    let mut rows = String::new();
    for key in key_store.keys.values() {
        if key.tenant != ws.tenant {
            continue;
        }
        let owner = store
            .accounts
            .values()
            .find(|candidate| {
                candidate
                    .principals
                    .iter()
                    .any(|principal| principal == &format!("key:{}", key.id))
            })
            .map(|candidate| candidate.id.clone());
        if !admin && owner.as_deref() != Some(account.as_str()) {
            continue;
        }
        let status = match key.status {
            keys::Status::Active => "active",
            keys::Status::Paused => "paused",
            keys::Status::Revoked => "revoked",
        };
        let scopes = key
            .scopes
            .as_ref()
            .map(|scopes| {
                let models = scopes
                    .models
                    .as_ref()
                    .map(|set| set.len().to_string())
                    .unwrap_or_else(|| "*".to_string());
                let actions = scopes
                    .actions
                    .as_ref()
                    .map(|set| set.iter().cloned().collect::<Vec<_>>().join(","))
                    .unwrap_or_else(|| "*".to_string());
                format!("models:{models} actions:{actions}")
            })
            .unwrap_or_else(|| "unscoped".to_string());
        rows.push_str(&format!(
            r#"<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>"#,
            esc(&key.id),
            esc(key.name.as_deref().unwrap_or("—")),
            status,
            esc(&scopes),
            esc(owner.as_deref().unwrap_or("unbound")),
        ));
    }
    page(
        "keys",
        Some(&workspace),
        &format!(
            r#"<h1>keys</h1>
            <p class="dim">{} · secrets are never shown — a key's secret is returned once, at issue</p>
            <table><tr><th>id</th><th>name</th><th>status</th><th>scopes</th><th>account</th></tr>{}</table>"#,
            if admin { "all workspace keys" } else { "your keys" },
            rows,
        ),
    )
    .into_response()
}

/// `GET /dashboard/w/{ws}/billing` — the subscription, invoices, and
/// checkouts the billing book holds for the workspace. Reads only —
/// management operations stay on the owner-only JSON API.
async fn billing_page(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = member_of(&state, &headers, &workspace) {
        return response;
    }
    let Some(_billing_config) = &state.config.billing else {
        return page(
            "billing",
            Some(&workspace),
            r#"<h1>billing</h1><p class="dim">this deployment runs no billing — there is no plan, no checkout, and no invoice</p>"#,
        )
        .into_response();
    };
    let book = match tenancy::billing::Billing::open(&state.dir).and_then(|billing| billing.store())
    {
        Ok(store) => store.book,
        Err(trouble) => {
            return page_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing unavailable",
                &trouble.to_string(),
            );
        }
    };
    let subscription_html = match book.subscription_for(&workspace) {
        Some(subscription) => format!(
            r#"<table><tr><th>subscription</th><td><code>{}</code></td></tr>
            <tr><th>plan</th><td><code>{}</code> v{}</td></tr>
            <tr><th>state</th><td>{}</td></tr>
            <tr><th>period</th><td>{} · ends {}</td></tr>{}</table>"#,
            esc(&subscription.id),
            esc(&subscription.plan),
            esc(&subscription.plan_version),
            subscription.state,
            subscription.period,
            subscription.period_ends,
            subscription
                .cancel_at
                .map(|at| format!(r#"<tr><th>cancels</th><td>{at}</td></tr>"#))
                .unwrap_or_default(),
        ),
        None => r#"<p class="dim">no subscription — the workspace has no billing entitlement</p>"#
            .to_string(),
    };
    let invoices = book
        .invoices
        .values()
        .filter(|invoice| invoice.workspace == workspace)
        .map(|invoice| {
            format!(
                r#"<tr><td><code>{}</code></td><td>{}</td><td>{} {}</td><td>{}</td></tr>"#,
                esc(&invoice.id),
                invoice.period,
                amount(invoice.amount),
                esc(&invoice.currency),
                invoice.state,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    page(
        "billing",
        Some(&workspace),
        &format!(
            r#"<h1>billing</h1><h2>subscription</h2>{subscription_html}
            <h2>invoices</h2>{}
            <p class="dim">plan changes, checkouts, and provider details run through the owner-only JSON API — read
            <code>docs/decision-models/service/billing.md</code>.</p>"#,
            if invoices.is_empty() {
                r#"<p class="dim">none</p>"#.to_string()
            } else {
                format!(r#"<table><tr><th>id</th><th>period</th><th>amount</th><th>state</th></tr>{invoices}</table>"#)
            },
        ),
    )
    .into_response()
}
