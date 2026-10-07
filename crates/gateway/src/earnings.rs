//! Authenticated payee statements over the receiver's existing ledger.
//!
//! An operator grants one account access to a payee under a workspace. Every
//! request resolves a current credential and checks current membership. The
//! dashboard uses the same authorization; its mutation form also carries a
//! session-bound forgery token. This module never starts or retries a payment.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
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
}

impl Credential<'_> {
    fn principal(&self, state: &ServeState) -> Result<Principal, Response> {
        if self.browser {
            dashboard::principal_of(state, self.headers)
        } else {
            accounts::principal(state, self.headers)
        }
    }
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
        .filter(|g| g.account == account && accounts::member(state, account, &g.workspace).is_ok())
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

async fn statement_body(
    state: &ServeState,
    credential: &Credential<'_>,
    party: &str,
    page: &Page,
) -> Result<Value, Response> {
    let mut ledger = state.earnings.as_ref().unwrap().lock().await;
    // Recheck after waiting for the ledger, before reading private state.
    let principal = credential.principal(state)?;
    authorize(state, &principal, party)?;
    let statement = ledger
        .earnings_statement(party, page.after_earning, page.after_payout, page.limit)
        .map_err(failure)?;
    let mut body = json!({"v":SCHEMA,"party":party,"unit":"msat","statement":statement,"destination":destination_body(state,&ledger,party)?,"capabilities":{"commissions":"unavailable","reversals":"unavailable"},"terms":"Author fees and resource shares come from the recorded release and split rule. Bonuses are separate obligations; no referral commission is inferred."});
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
    let principal = credential.principal(state)?;
    authorize(state, &principal, party)?;
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
    ledger
        .change_account_payout(
            party,
            change.expected_version,
            &change.value,
            accounts::unix_now() as i64,
        )
        .map_err(failure)?;
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
    headers: HeaderMap,
) -> Response {
    match statement_body(
        &state,
        &Credential {
            headers: &headers,
            browser: false,
        },
        &party,
        &page,
    )
    .await
    {
        Ok(body) => answer(body),
        Err(r) => private(r),
    }
}

async fn destination(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    headers: HeaderMap,
) -> Response {
    let ledger = state.earnings.as_ref().unwrap().lock().await;
    let p = match accounts::principal(&state, &headers) {
        Ok(p) => p,
        Err(r) => return private(r),
    };
    if let Err(r) = authorize(&state, &p, &party) {
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
    headers: HeaderMap,
    Json(change): Json<Change>,
) -> Response {
    match update(
        &state,
        &Credential {
            headers: &headers,
            browser: false,
        },
        &party,
        change,
    )
    .await
    {
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
    let principal = credential.principal(state)?;
    authorize(state, &principal, party)?;
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
    headers: HeaderMap,
) -> Response {
    match payout_body(
        &state,
        &Credential {
            headers: &headers,
            browser: false,
        },
        &party,
        &id,
    )
    .await
    {
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
    headers: HeaderMap,
) -> Response {
    match statement_body(
        &state,
        &Credential {
            headers: &headers,
            browser: false,
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
<p>Unknown payouts stay reserved until the payout worker resolves the same wallet reference. This page never sends or retries payments. Commissions and reversals are unavailable in this deployment.</p>
<h2>Eligible obligations</h2><table><tr><th>Sequence</th><th>Resource</th><th>Release</th><th>Role</th><th>msat</th><th>State</th></tr>{rows}</table>
<h2>Payouts</h2><table><tr><th>Attempt</th><th>State</th><th>Reserved msat</th><th>Rail msat</th><th>Wallet reference</th></tr>{payouts}</table>{more}
<h2>Destination</h2><pre>{destination}</pre><p>Source validation checks mainnet format. The qualification reference is the owner's rail evidence; a saved setting is not proof of payment. Signed releases and profiles retain priority. Changes affect future reservations.</p>
<form method="post" action="/dashboard/earnings/{party}/destination"><input type="hidden" name="expected_version" value="{version}"><input type="hidden" name="csrf" value="{csrf}"><label>Mainnet Spark or Lightning address <input name="value" required maxlength="256"></label><button type="submit">Save destination</button></form>"#,
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
    match payout_body(&state,&Credential { headers: &headers, browser: true },&party,&id).await {
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
        for action in ["revoke", "pause", "unlink"] {
            let dir = tempfile::tempdir().unwrap();
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
            let config:crate::config::Config=serde_json::from_value(json!({"v":crate::config::SCHEMA,"listen":"127.0.0.1:0","registry":dir.path(),"accounts":{},"earnings":{"ledger":dir.path().join("pay.sqlite"),"grants":[{"party":"author","account":account.id,"workspace":workspace.id}],"rails":{"lightning":"synthetic-qualification"}}})).unwrap();
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
            };
            let mut pending = Box::pin(update(
                &state,
                &credential,
                "author",
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
                _ => {
                    accounts.update_principals(&account.id, &[]).unwrap();
                }
            }
            drop(held);
            let refused = pending.await.unwrap_err();
            assert!(
                matches!(
                    refused.status(),
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
                ),
                "{action}"
            );
            assert!(
                state
                    .earnings
                    .as_ref()
                    .unwrap()
                    .lock()
                    .await
                    .account_payout("author")
                    .unwrap()
                    .is_none()
            );
            assert!(
                statement_body(
                    &state,
                    &credential,
                    "author",
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
}
