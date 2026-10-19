//! Authenticated payee statements over the receiver's existing ledger.
//!
//! An operator grants one account access to a payee under a workspace. Every
//! request resolves a current credential and checks current membership. The
//! dashboard uses the same authorization; its mutation form also carries a
//! session-bound forgery token. This module never starts or retries a payment.
//!
//! A party named by its own Nostr key (64 lowercase hex, as a pylon's
//! provider share is: the pylon's NIP-OA owner, else the pylon key) may
//! also act for itself with no account: a NIP-98 `Authorization: Nostr …`
//! event signed by that key, for the exact URL and method, within 60
//! seconds, bound to the body's SHA-256 on a `PUT`, reads its statement,
//! destination, and payouts and sets its own destination
//! (`openagents pylon earnings` and `payout set`). The signed body carries
//! the expected destination version, so a replayed change conflicts.

use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Form, OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{MethodRouter, get, post};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{self, Principal};
use crate::dashboard::{self, esc};
use crate::serve::ServeState;

const SCHEMA: &str = "openagents.earnings.v1";

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/earnings", get(index)),
        ("/v1/earnings/{party}", get(statement)),
        ("/v1/earnings/{party}/export", get(export)),
        (
            "/v1/earnings/{party}/destination",
            get(destination).put(change_destination),
        ),
        ("/v1/earnings/{party}/payouts/{payout}", get(payout)),
        ("/dashboard/earnings", get(index_page)),
        ("/dashboard/earnings/{party}", get(statement_page)),
        ("/dashboard/earnings/{party}/export", get(export_page)),
        ("/dashboard/earnings/{party}/destination", post(change_page)),
        (
            "/dashboard/earnings/{party}/payouts/{payout}",
            get(payout_page),
        ),
    ]
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default)]
    pub after_earning: i64,
    #[serde(default)]
    pub after_payout: i64,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub expected_version: u64,
    pub value: String,
}

fn failure(error: pay_ledger::Error) -> Response {
    let (status, code) = match error {
        pay_ledger::Error::Conflict(_) => (StatusCode::CONFLICT, "destination_conflict"),
        pay_ledger::Error::Invalid(_) => (StatusCode::BAD_REQUEST, "earnings_invalid"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "earnings_unavailable"),
    };
    // Database paths and rail errors never cross the private export boundary.
    accounts::refused(
        status,
        code,
        match status {
            StatusCode::CONFLICT => {
                "The destination changed. Refresh its version before trying again."
            }
            StatusCode::BAD_REQUEST => {
                "Use supported mainnet destination and bounded statement parameters."
            }
            _ => "The earnings ledger is unavailable. Try again later.",
        },
    )
}

fn private(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}

fn answer(body: Value) -> Response {
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    if bytes.len() > 1_048_576 {
        return private(accounts::refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "earnings_export_too_large",
            "This statement exceeds 1 MiB. Request a smaller page limit.",
        ));
    }
    private(([("content-type", "application/json")], bytes).into_response())
}

struct Credential<'a> {
    headers: &'a HeaderMap,
    browser: bool,
    /// The party key a NIP-98 header proved ([`party_key`]), if any.
    party_key: Option<String>,
}

impl Credential<'_> {
    fn principal(&self, state: &ServeState) -> Result<Principal, Response> {
        if self.browser {
            dashboard::principal_of(state, self.headers)
        } else {
            accounts::principal(state, self.headers)
        }
    }

    /// Who may act for `party`: the party's own key (`None`), or an
    /// account granted access to it.
    fn check(&self, state: &ServeState, party: &str) -> Result<Option<Principal>, Response> {
        if let Some(key) = &self.party_key {
            return if key == party {
                Ok(None)
            } else {
                Err(accounts::refused(
                    StatusCode::FORBIDDEN,
                    "payee_forbidden",
                    "A signed request reaches only the payee named by its own key.",
                ))
            };
        }
        let principal = self.principal(state)?;
        authorize(state, &principal, party)?;
        Ok(Some(principal))
    }
}

/// The longest `Authorization: Nostr …` value read.
const NOSTR_HEADER_MAX: usize = 8 * 1024;

/// The key a NIP-98 `Authorization: Nostr …` header proves for this
/// request: kind 27235, signed within 60 seconds, its `u` tag this exact
/// URL on an accepted origin (`oa_auth::nostr::DEFAULT_ORIGINS`, the
/// configured `public_origin`, or the request's own `Host`), its `method`
/// tag `method`, and its `payload` tag the SHA-256 of `body` when there is
/// one. `None` when the request carries no Nostr authorization.
fn party_key(
    state: &ServeState,
    headers: &HeaderMap,
    method: &str,
    uri: &Uri,
    body: Option<&[u8]>,
) -> Result<Option<String>, Response> {
    let Some(header) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .filter(|h| h.starts_with("Nostr "))
    else {
        return Ok(None);
    };
    let denied = || {
        accounts::refused(
            StatusCode::UNAUTHORIZED,
            "nostr_auth_denied",
            "That signed request didn't check out. Sign it with the payee's key for this exact URL and method, and send it within 60 seconds.",
        )
    };
    if header.len() > NOSTR_HEADER_MAX {
        return Err(denied());
    }
    let path = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .filter(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_graphic() && b != b'/'));
    let origins = oa_auth::nostr::DEFAULT_ORIGINS
        .iter()
        .map(|o| (*o).to_string())
        .chain(state.config.public_origin.clone())
        .chain(
            host.into_iter()
                .flat_map(|h| [format!("https://{h}"), format!("http://{h}")]),
        );
    let payload = body
        .filter(|b| !b.is_empty())
        .map(|b| hex::encode(<sha2::Sha256 as sha2::Digest>::digest(b)));
    let now = accounts::unix_now();
    for origin in origins {
        let url = format!("{}{path}", origin.trim_end_matches('/'));
        if let Ok(auth) = nostr::domain::parse_http_authorization_hash(
            header,
            method,
            &url,
            payload.as_deref(),
            now,
        ) {
            return Ok(Some(auth.pubkey));
        }
    }
    Err(denied())
}

/// The credential for a JSON API request: a NIP-98 party key, else the
/// account behind the headers.
fn api_credential<'a>(
    state: &ServeState,
    headers: &'a HeaderMap,
    method: &str,
    uri: &Uri,
    body: Option<&[u8]>,
) -> Result<Credential<'a>, Response> {
    Ok(Credential {
        headers,
        browser: false,
        party_key: party_key(state, headers, method, uri, body)?,
    })
}

fn authorize(state: &ServeState, principal: &Principal, party: &str) -> Result<(), Response> {
    let account = accounts::member_account(principal)?;
    if let Principal::Account {
        session: Some(id), ..
    } = principal
    {
        let sessions = tenancy::Sessions::open(&state.dir)
            .and_then(|s| s.store())
            .map_err(|_| {
                accounts::refused(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "sessions_unavailable",
                    "The session store is unavailable.",
                )
            })?;
        if sessions
            .book
            .session(&tenancy::SessionId::from(id.as_str()))
            .is_none_or(|s| {
                s.user.as_str() != account
                    || s.standing(accounts::unix_now()) != tenancy::SessionState::Active
            })
        {
            return Err(accounts::refused(
                StatusCode::UNAUTHORIZED,
                "session_closed",
                "Your session has ended. Sign in again.",
            ));
        }
    }
    let grant = state
        .config
        .earnings
        .as_ref()
        .and_then(|e| {
            e.grants
                .iter()
                .find(|g| g.party == party && g.account == account)
        })
        .ok_or_else(|| {
            accounts::refused(
                StatusCode::FORBIDDEN,
                "payee_forbidden",
                "This account has no access to that payee.",
            )
        })?;
    accounts::member(state, account, &grant.workspace)?;
    if let Some(referrer) = party.strip_prefix("referrer:") {
        if !state
            .config
            .earnings
            .as_ref()
            .is_some_and(|c| c.commissions)
            || tenancy::Accounts::open(&state.dir)
                .ok()
                .and_then(|a| a.referrer(account, referrer).ok())
                .is_none()
        {
            return Err(accounts::refused(
                StatusCode::FORBIDDEN,
                "commission_payee_forbidden",
                "Current native referrer management and enabled commission statements are required.",
            ));
        }
    }
    Ok(())
}

fn parties(state: &ServeState, principal: &Principal) -> Result<Vec<String>, Response> {
    let account = accounts::member_account(principal)?;
    Ok(state
        .config
        .earnings
        .as_ref()
        .unwrap()
        .grants
        .iter()
        .filter(|g| g.account == account && authorize(state, principal, &g.party).is_ok())
        .map(|g| g.party.clone())
        .collect())
}

fn destination_body(
    state: &ServeState,
    ledger: &pay_ledger::Ledger,
    party: &str,
) -> Result<Value, Response> {
    let effective = ledger.payee(party).map_err(failure)?;
    let setting = ledger.account_payout(party).map_err(failure)?;
    let effective = match effective {
        Some(p) => {
            let rail = match p.destination_kind.as_str() {
                "spark" => Some("spark"),
                "lud16" => Some("lightning"),
                _ => None,
            };
            let evidence = rail.and_then(|r| state.config.earnings.as_ref()?.rails.get(r));
            json!({"kind":p.destination_kind,"value":p.destination_value,"source":p.source,"source_verified_at":p.verified_at,"rail":rail,"rail_status":if evidence.is_some() {"owner_qualified"} else {"unavailable"},"qualification_reference":evidence})
        }
        None => Value::Null,
    };
    Ok(
        json!({"setting":setting,"effective":effective,"verification":"mainnet_source_validation","reserved_payouts":"unchanged"}),
    )
}

fn native_custody(state: &ServeState, ledger: &pay_ledger::Ledger) -> Result<(), Response> {
    if state
        .config
        .earnings
        .as_ref()
        .is_some_and(|c| c.commissions)
    {
        #[cfg(unix)]
        ledger.require_native_custody().map_err(failure)?;
        #[cfg(not(unix))]
        return Err(failure(pay_ledger::Error::Denied(
            "native commission custody unsupported",
        )));
    }
    Ok(())
}

async fn statement_body(
    state: &ServeState,
    credential: &Credential<'_>,
    party: &str,
    page: &Page,
) -> Result<Value, Response> {
    let mut ledger = state.earnings.as_ref().unwrap().lock().await;
    native_custody(state, &ledger)?;
    // Recheck after waiting for the ledger, before reading private state.
    credential.check(state, party)?;
    let statement = ledger
        .earnings_statement(party, page.after_earning, page.after_payout, page.limit)
        .map_err(failure)?;
    let mut body = json!({"v":SCHEMA,"party":party,"unit":"msat","statement":statement,"destination":destination_body(state,&ledger,party)?,"capabilities":{"commissions":"unavailable","reversals":"unavailable"},"terms":"Author fees and resource shares come from the recorded release and split rule. Bonuses are separate obligations; no referral commission is inferred."});
    if party.starts_with("referrer:")
        && state
            .config
            .earnings
            .as_ref()
            .is_some_and(|c| c.commissions)
    {
        body["commission"] = json!(ledger.commission_payee_figures(party).map_err(failure)?);
        body["capabilities"] =
            json!({"commissions":"native_plugin_merchant","reversals":"verified_native_refund"});
        body["terms"] = json!(
            "Each obligation retains its original accepted attribution and bilateral terms. Unknown costs, refunds, and payouts stay held. Sub-satoshi remainders remain liabilities. Author fees and bonuses stay unchanged."
        );
    }
    body["payee_details_needed"] = body["statement"]["payee_details_needed"].clone();
    for payout in body["statement"]["payouts"].as_array_mut().unwrap() {
        let id = payout["id"].as_str().unwrap().to_owned();
        payout["reconciliation"] = json!(format!("/v1/earnings/{party}/payouts/{id}"));
    }
    Ok(body)
}

async fn update(
    state: &ServeState,
    credential: &Credential<'_>,
    party: &str,
    change: Change,
) -> Result<Value, Response> {
    let mut ledger = state.earnings.as_ref().unwrap().lock().await;
    native_custody(state, &ledger)?;
    let principal = credential.check(state, party)?;
    let (kind, _) = pay_ledger::payee::classify(&change.value)
        .ok_or_else(|| failure(pay_ledger::Error::Invalid("destination")))?;
    let rail = match kind {
        pay_ledger::payee::Kind::Spark => "spark",
        pay_ledger::payee::Kind::LightningAddress => "lightning",
        pay_ledger::payee::Kind::NodeKey => {
            return Err(failure(pay_ledger::Error::Invalid("unsupported rail")));
        }
    };
    if !state
        .config
        .earnings
        .as_ref()
        .unwrap()
        .rails
        .contains_key(rail)
    {
        return Err(accounts::refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "payout_rail_unavailable",
            "The owner has not qualified this payout rail. Your existing destination and reservations stay unchanged.",
        ));
    }
    if party.starts_with("referrer:")
        && !ledger
            .commission_destination_qualified(party, kind.as_str())
            .map_err(failure)?
    {
        return Err(failure(pay_ledger::Error::Invalid(
            "accepted commission destination",
        )));
    }
    let Some(principal) = principal else {
        // The party's own key: no account membership to hold.
        ledger
            .change_account_payout(
                party,
                change.expected_version,
                &change.value,
                accounts::unix_now() as i64,
            )
            .map_err(failure)?;
        return Ok(
            json!({"v":SCHEMA,"party":party,"destination":destination_body(state,&ledger,party)?}),
        );
    };
    // Keep current native membership and referrer management locked through
    // the short destination append. An ownership transfer cannot win between
    // authorization and this mutation.
    let accounts = tenancy::Accounts::open(&state.dir)
        .map_err(|_| failure(pay_ledger::Error::Denied("native accounts unavailable")))?;
    let mut changed = None;
    accounts
        .read_locked::<_, tenancy::accounts::Trouble>(|_| {
            changed = Some((|| {
                let current = credential.principal(state)?;
                authorize(state, &current, party)?;
                native_custody(state, &ledger)?;
                ledger
                    .change_account_payout(
                        party,
                        change.expected_version,
                        &change.value,
                        accounts::unix_now() as i64,
                    )
                    .map_err(failure)?;
                Ok(())
            })());
            Ok(())
        })
        .map_err(|_| failure(pay_ledger::Error::Denied("native accounts unavailable")))?;
    changed.ok_or_else(|| {
        failure(pay_ledger::Error::Denied(
            "native destination admission absent",
        ))
    })??;
    accounts::record(
        state,
        &principal,
        "payout-destination-change",
        None,
        Some(format!(
            "payee:{party};version:{}",
            change.expected_version + 1
        )),
    );
    Ok(json!({"v":SCHEMA,"party":party,"destination":destination_body(state,&ledger,party)?}))
}

async fn index(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let result = accounts::principal(&state, &headers).and_then(|p| parties(&state, &p));
    match result {
        Ok(parties) => answer(json!({"v":SCHEMA,"payees":parties})),
        Err(r) => private(r),
    }
}

async fn statement(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    Query(page): Query<Page>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let credential = match api_credential(&state, &headers, "GET", &uri, None) {
        Ok(credential) => credential,
        Err(r) => return private(r),
    };
    match statement_body(&state, &credential, &party, &page).await {
        Ok(body) => answer(body),
        Err(r) => private(r),
    }
}

async fn destination(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let credential = match api_credential(&state, &headers, "GET", &uri, None) {
        Ok(credential) => credential,
        Err(r) => return private(r),
    };
    let ledger = state.earnings.as_ref().unwrap().lock().await;
    if let Err(r) = native_custody(&state, &ledger) {
        return private(r);
    }
    if let Err(r) = credential.check(&state, &party) {
        return private(r);
    }
    match destination_body(&state, &ledger, &party) {
        Ok(body) => answer(json!({"v":SCHEMA,"party":party,"destination":body})),
        Err(r) => private(r),
    }
}

async fn change_destination(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let credential = match api_credential(&state, &headers, "PUT", &uri, Some(&body)) {
        Ok(credential) => credential,
        Err(r) => return private(r),
    };
    let Ok(Json(change)) = Json::<Change>::from_bytes(&body) else {
        return private(failure(pay_ledger::Error::Invalid("destination")));
    };
    match update(&state, &credential, &party, change).await {
        Ok(body) => answer(body),
        Err(r) => private(r),
    }
}

async fn payout_body(
    state: &ServeState,
    credential: &Credential<'_>,
    party: &str,
    id: &str,
) -> Result<Value, Response> {
    let ledger = state.earnings.as_ref().unwrap().lock().await;
    native_custody(state, &ledger)?;
    credential.check(state, party)?;
    let p = ledger
        .earnings_payout(party, id)
        .map_err(failure)?
        .ok_or_else(|| {
            accounts::refused(
                StatusCode::NOT_FOUND,
                "payout_not_found",
                "That payee has no such payout.",
            )
        })?;
    Ok(
        json!({"v":SCHEMA,"party":party,"payout":p,"reconciliation":{"source":"payout_worker_journal","lookup":"The payout worker looks up this same wallet reference. Unknown outcomes stay reserved; this view does not send or retry a payment.","proof":"The statement reports journaled rail outcomes, not remote attestation."}}),
    )
}

async fn payout(
    State(state): State<Arc<ServeState>>,
    Path((party, id)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let credential = match api_credential(&state, &headers, "GET", &uri, None) {
        Ok(credential) => credential,
        Err(r) => return private(r),
    };
    match payout_body(&state, &credential, &party, &id).await {
        Ok(body) => answer(body),
        Err(r) => private(r),
    }
}

fn download(body: Value) -> Response {
    let mut response = answer(body);
    response.headers_mut().insert(
        "content-disposition",
        "attachment; filename=earnings.json".parse().unwrap(),
    );
    response
}

async fn export(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    Query(page): Query<Page>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let credential = match api_credential(&state, &headers, "GET", &uri, None) {
        Ok(credential) => credential,
        Err(r) => return private(r),
    };
    match statement_body(&state, &credential, &party, &page).await {
        Ok(body) => download(body),
        Err(r) => private(r),
    }
}

async fn index_page(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let p = match dashboard::principal_of(&state, &headers) {
        Ok(p) => p,
        Err(r) => return private(r),
    };
    let parties = match parties(&state, &p) {
        Ok(parties) => parties,
        Err(r) => return private(r),
    };
    let rows: String = parties
        .into_iter()
        .map(|party| {
            format!(
                "<li><a href=\"/dashboard/earnings/{}\">{}</a></li>",
                esc(&party),
                esc(&party)
            )
        })
        .collect();
    private(dashboard::page("Earnings",None,&format!("<h1>Earnings</h1><p><a href=\"/dashboard\">Workspaces</a></p><ul>{rows}</ul><p>Only payees granted to your account with current workspace access appear here.</p>")).into_response())
}

fn csrf(headers: &HeaderMap, party: &str, version: u64) -> String {
    pay_ledger::digest(&format!(
        "earnings:{}:{party}:{version}",
        dashboard::cookie_token(headers).unwrap_or_default()
    ))
}

async fn statement_page(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    Query(page): Query<Page>,
    headers: HeaderMap,
) -> Response {
    let body = match statement_body(
        &state,
        &Credential {
            headers: &headers,
            browser: true,
            party_key: None,
        },
        &party,
        &page,
    )
    .await
    {
        Ok(body) => body,
        Err(r) => return private(r),
    };
    let figures = &body["statement"]["figures"];
    let mut rows = String::new();
    for earning in body["statement"]["earnings"].as_array().unwrap() {
        for obligation in earning["obligations"].as_array().unwrap() {
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                earning["sequence"],
                esc(earning["resource"].as_str().unwrap_or("")),
                esc(earning["release_id"].as_str().unwrap_or("—")),
                esc(obligation["role"].as_str().unwrap_or("")),
                obligation["amount_msat"],
                esc(obligation["state"].as_str().unwrap_or(""))
            ));
        }
    }
    let mut payouts = String::new();
    for payout in body["statement"]["payouts"].as_array().unwrap() {
        payouts.push_str(&format!("<tr><td><a href=\"/dashboard/earnings/{}/payouts/{}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",esc(&party),esc(payout["id"].as_str().unwrap()),esc(payout["id"].as_str().unwrap()),esc(payout["state"].as_str().unwrap()),payout["amount_msat"],payout["sent_msat"],esc(payout["wallet_reference"].as_str().unwrap_or("—"))));
    }
    let version = body["destination"]["setting"]["version"]
        .as_u64()
        .unwrap_or(0);
    let destination = esc(&serde_json::to_string_pretty(&body["destination"]).unwrap());
    let next_earning = body["statement"]["next_earning"].as_i64();
    let next_payout = body["statement"]["next_payout"].as_i64();
    let more = if next_earning.is_some() || next_payout.is_some() {
        format!(
            "<a href=\"?after_earning={}&amp;after_payout={}&amp;limit={}\">Next page</a>",
            next_earning.unwrap_or(i64::MAX),
            next_payout.unwrap_or(i64::MAX),
            page.limit
        )
    } else {
        String::new()
    };
    let html = format!(
        r#"<h1>Earnings for {party}</h1><p><a href="/dashboard/earnings">Payees</a> · <a href="/dashboard/earnings/{party}/export?after_earning={after_earning}&amp;after_payout={after_payout}&amp;limit={limit}">Export this page</a></p>
<p>All amounts are exact msat. Earned: {earned}; available: {accrued}; reserved: {reserved}; claims consumed: {consumed}; sent on the rail: {sent}; rounding: {rounding}; sent claims without an exact rail amount: {unverified}.</p>
<p>Unknown payouts stay reserved until the payout worker resolves the same wallet reference. This page never sends or retries payments. {commission_notice}</p>
<h2>Eligible obligations</h2><table><tr><th>Sequence</th><th>Resource</th><th>Release</th><th>Role</th><th>msat</th><th>State</th></tr>{rows}</table>
<h2>Payouts</h2><table><tr><th>Attempt</th><th>State</th><th>Reserved msat</th><th>Rail msat</th><th>Wallet reference</th></tr>{payouts}</table>{more}
<h2>Destination</h2><pre>{destination}</pre><p>Source validation checks mainnet format. The qualification reference is the owner's rail evidence; a saved setting is not proof of payment. Signed releases and profiles retain priority. Changes affect future reservations.</p>
<form method="post" action="/dashboard/earnings/{party}/destination"><input type="hidden" name="expected_version" value="{version}"><input type="hidden" name="csrf" value="{csrf}"><label>Mainnet Spark or Lightning address <input name="value" required maxlength="256"></label><button type="submit">Save destination</button></form>"#,
        commission_notice = if body.get("commission").is_some() {
            format!(
                "Native commission and reversal figures: {}",
                esc(&body["commission"].to_string())
            )
        } else {
            "Commissions and reversals are unavailable in this deployment.".into()
        },
        party = esc(&party),
        after_earning = page.after_earning,
        after_payout = page.after_payout,
        limit = page.limit,
        earned = figures["earned_msat"],
        accrued = figures["accrued_msat"],
        reserved = figures["reserved_msat"],
        consumed = figures["consumed_msat"],
        sent = figures["sent_msat"],
        rounding = figures["rounding_msat"],
        unverified = figures["unverified_sent_msat"],
        csrf = csrf(&headers, &party, version)
    );
    private(dashboard::page("Earnings", None, &html).into_response())
}

#[derive(Deserialize)]
struct ChangeForm {
    expected_version: u64,
    value: String,
    csrf: String,
}

async fn change_page(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    headers: HeaderMap,
    Form(form): Form<ChangeForm>,
) -> Response {
    if form.csrf != csrf(&headers, &party, form.expected_version) {
        return private(accounts::refused(
            StatusCode::FORBIDDEN,
            "invalid_form",
            "Refresh the earnings page before changing a destination.",
        ));
    }
    match update(
        &state,
        &Credential {
            headers: &headers,
            browser: true,
            party_key: None,
        },
        &party,
        Change {
            expected_version: form.expected_version,
            value: form.value,
        },
    )
    .await
    {
        Ok(_) => private(Redirect::to(&format!("/dashboard/earnings/{party}")).into_response()),
        Err(r) => private(r),
    }
}

async fn export_page(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    Query(page): Query<Page>,
    headers: HeaderMap,
) -> Response {
    match statement_body(
        &state,
        &Credential {
            headers: &headers,
            browser: true,
            party_key: None,
        },
        &party,
        &page,
    )
    .await
    {
        Ok(body) => download(body),
        Err(r) => private(r),
    }
}

async fn payout_page(
    State(state): State<Arc<ServeState>>,
    Path((party, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    match payout_body(&state,&Credential { headers: &headers, browser: true, party_key: None },&party,&id).await {
        Ok(body) => private(dashboard::page("Payout reconciliation",None,&format!("<h1>Payout reconciliation</h1><p><a href=\"/dashboard/earnings/{}\">Back to earnings</a></p><pre>{}</pre>",esc(&party),esc(&serde_json::to_string_pretty(&body).unwrap()))).into_response()),
        Err(r) => private(r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Waker};
    use tenancy::{Accounts, Manifest, Registry, Tenant, WorkspaceKind, keys};

    #[tokio::test]
    async fn a_key_revoked_paused_or_unlinked_while_waiting_cannot_change_a_destination() {
        for action in [
            "revoke",
            "pause",
            "unlink",
            "migrate-referrer",
            "replace-ledger",
            "disclose-ledger",
        ] {
            let dir = tempfile::tempdir().unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
            let registry = Registry::install(
                dir.path(),
                Manifest {
                    v: tenancy::SCHEMA.into(),
                    sequence: 0,
                    supersedes: None,
                    shared: Default::default(),
                    tenants: [(
                        "acme".into(),
                        Tenant {
                            credential: "key-ref:acme".into(),
                            principals: vec![],
                            doors: Default::default(),
                            quota: None,
                        },
                    )]
                    .into(),
                    digest: String::new(),
                },
            )
            .unwrap();
            let key = keys::issue(dir.path(), registry.manifest(), "acme").unwrap();
            let accounts = Accounts::install(dir.path()).unwrap();
            let account = accounts
                .create_account("Author", &[format!("key:{}", key.key.id)])
                .unwrap();
            let workspace = accounts
                .create_workspace(&account.id, "Author", WorkspaceKind::Personal, "acme", None)
                .unwrap();
            let migration = if action == "migrate-referrer" {
                let referrer = accounts
                    .create_referrer(
                        &account.id,
                        tenancy::accounts::referrals::Kind::Person,
                        "Synthetic source",
                    )
                    .unwrap();
                let next = accounts.create_account("Synthetic successor", &[]).unwrap();
                Some((referrer.id, next.id))
            } else {
                None
            };
            let party = migration
                .as_ref()
                .map_or_else(|| "author".to_owned(), |(id, _)| format!("referrer:{id}"));
            let config:crate::config::Config=serde_json::from_value(json!({"v":crate::config::SCHEMA,"listen":"127.0.0.1:0","registry":dir.path(),"accounts":{},"earnings":{"commissions":true,"ledger":dir.path().join("pay.sqlite"),"grants":[{"party":party,"account":account.id,"workspace":workspace.id}],"rails":{"lightning":"synthetic-qualification"}}})).unwrap();
            let path = dir.path().join("pay.sqlite");
            drop(pay_ledger::Ledger::open(&path).unwrap());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(ServeState::open(config.clone()).is_err());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let state = ServeState::open(config).unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(
                "authorization",
                format!("Bearer {}", key.token).parse().unwrap(),
            );
            assert!(accounts::principal(&state, &headers).is_ok());
            let held = state.earnings.as_ref().unwrap().lock().await;
            let credential = Credential {
                headers: &headers,
                browser: false,
                party_key: None,
            };
            let mut pending = Box::pin(update(
                &state,
                &credential,
                &party,
                Change {
                    expected_version: 0,
                    value: "author@example.com".into(),
                },
            ));
            assert!(
                pending
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            match action {
                "revoke" => keys::revoke(dir.path(), &key.key.id).unwrap(),
                "pause" => keys::pause(dir.path(), &key.key.id).unwrap(),
                "replace-ledger" => {
                    std::fs::rename(&path, dir.path().join("retired.sqlite")).unwrap();
                    std::fs::copy(dir.path().join("retired.sqlite"), &path).unwrap();
                }
                "disclose-ledger" => {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
                        .unwrap();
                }
                "migrate-referrer" => {
                    let (id, next) = migration.as_ref().unwrap();
                    accounts
                        .offer_referrer_migration(&account.id, id, next)
                        .unwrap();
                    accounts.accept_referrer_migration(next, id).unwrap();
                }
                _ => {
                    accounts.update_principals(&account.id, &[]).unwrap();
                }
            }
            drop(held);
            let refused = pending.await.unwrap_err();
            if matches!(action, "replace-ledger" | "disclose-ledger") {
                assert_eq!(
                    refused.status(),
                    StatusCode::SERVICE_UNAVAILABLE,
                    "{action}"
                );
            } else {
                assert!(
                    matches!(
                        refused.status(),
                        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
                    ),
                    "{action}"
                );
            }
            assert!(
                state
                    .earnings
                    .as_ref()
                    .unwrap()
                    .lock()
                    .await
                    .account_payout(&party)
                    .unwrap()
                    .is_none()
            );
            assert!(
                statement_body(
                    &state,
                    &credential,
                    &party,
                    &Page {
                        limit: 1,
                        ..Default::default()
                    }
                )
                .await
                .is_err()
            );
        }
    }

    fn nostr_header(
        signer: &nostr::domain::RelaySigner,
        method: &str,
        url: &str,
        body: Option<&[u8]>,
    ) -> String {
        use base64::Engine as _;
        let mut tags = vec![
            nostr::domain::Tag::new(vec!["u".into(), url.into()]),
            nostr::domain::Tag::new(vec!["method".into(), method.into()]),
        ];
        if let Some(body) = body {
            tags.push(nostr::domain::Tag::new(vec![
                "payload".into(),
                hex::encode(<sha2::Sha256 as sha2::Digest>::digest(body)),
            ]));
        }
        let event = signer.sign(accounts::unix_now(), 27_235, tags, String::new());
        format!(
            "Nostr {}",
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&event).unwrap())
        )
    }

    #[tokio::test]
    async fn a_party_key_reads_and_sets_only_its_own_destination() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        Registry::install(
            dir.path(),
            Manifest {
                v: tenancy::SCHEMA.into(),
                sequence: 0,
                supersedes: None,
                shared: Default::default(),
                tenants: [(
                    "acme".into(),
                    Tenant {
                        credential: "key-ref:acme".into(),
                        principals: vec![],
                        doors: Default::default(),
                        quota: None,
                    },
                )]
                .into(),
                digest: String::new(),
            },
        )
        .unwrap();
        Accounts::install(dir.path()).unwrap();
        let path = dir.path().join("pay.sqlite");
        drop(pay_ledger::Ledger::open(&path).unwrap());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config: crate::config::Config = serde_json::from_value(json!({"v":crate::config::SCHEMA,"listen":"127.0.0.1:0","registry":dir.path(),"accounts":{},"earnings":{"ledger":path,"grants":[],"rails":{"lightning":"synthetic-qualification","spark":"synthetic-qualification"}}})).unwrap();
        let state = ServeState::open(config).unwrap();

        let pylon = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let party = pylon.pubkey().to_string();
        let route = format!("/v1/earnings/{party}/destination");
        let uri: Uri = route.parse().unwrap();
        let url = format!("https://api.openagents.com{route}");
        let body =
            serde_json::to_vec(&json!({"expected_version":0,"value":"pylon@example.com"})).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            nostr_header(&pylon, "PUT", &url, Some(&body))
                .parse()
                .unwrap(),
        );

        // The signature covers the method, the URL, and the body.
        assert_eq!(
            party_key(&state, &headers, "PUT", &uri, Some(&body)).unwrap(),
            Some(party.clone())
        );
        assert!(party_key(&state, &headers, "GET", &uri, None).is_err());
        assert!(party_key(&state, &headers, "PUT", &uri, Some(b"{}")).is_err());
        let elsewhere: Uri = "/v1/earnings/someone/destination".parse().unwrap();
        assert!(party_key(&state, &headers, "PUT", &elsewhere, Some(&body)).is_err());
        assert_eq!(
            party_key(&state, &HeaderMap::new(), "PUT", &uri, Some(&body)).unwrap(),
            None
        );

        let credential = api_credential(&state, &headers, "PUT", &uri, Some(&body)).unwrap();
        let refused = update(
            &state,
            &credential,
            &"22".repeat(32),
            Change {
                expected_version: 0,
                value: "pylon@example.com".into(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
        update(
            &state,
            &credential,
            &party,
            Change {
                expected_version: 0,
                value: "pylon@example.com".into(),
            },
        )
        .await
        .unwrap();
        let setting = state
            .earnings
            .as_ref()
            .unwrap()
            .lock()
            .await
            .account_payout(&party)
            .unwrap()
            .unwrap();
        assert_eq!(setting.value, "pylon@example.com");
        assert_eq!(setting.version, 1);
        // A replay of the same signed change conflicts on the version.
        assert_eq!(
            update(
                &state,
                &credential,
                &party,
                Change {
                    expected_version: 0,
                    value: "pylon@example.com".into(),
                },
            )
            .await
            .unwrap_err()
            .status(),
            StatusCode::CONFLICT
        );

        // The same key reads its own statement, and only its own.
        let page = Page {
            limit: 1,
            ..Default::default()
        };
        let body = statement_body(&state, &credential, &party, &page)
            .await
            .unwrap();
        assert_eq!(body["party"], party);
        assert!(
            statement_body(&state, &credential, &"22".repeat(32), &page)
                .await
                .is_err()
        );
        // Without a Nostr header or an account, nothing is read.
        let anonymous = HeaderMap::new();
        let nobody = api_credential(&state, &anonymous, "GET", &uri, None).unwrap();
        assert!(
            statement_body(&state, &nobody, &party, &page)
                .await
                .is_err()
        );
    }
}
