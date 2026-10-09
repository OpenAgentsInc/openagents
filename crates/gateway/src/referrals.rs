//! Authenticated introduction management over canonical account custody.
use crate::{accounts, serve::ServeState};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{MethodRouter, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tenancy::accounts::referrals::{self, Capture, Error, Kind};
use tenancy::accounts::referrals::{attribution, commission};

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/join", get(join)),
        ("/v1/account/acquisition", get(source).post(capture)),
        ("/v1/account/referrers", post(create)),
        ("/v1/account/attribution/policy", get(policy)),
        ("/v1/account/attribution", get(attributed).post(propose)),
        ("/v1/account/attribution/confirm", post(confirm)),
        ("/v1/account/referral-terms", get(commission_terms)),
        (
            "/v1/account/referral-agreement",
            get(commission_agreement).post(commission_accept),
        ),
        (
            "/v1/workspaces/{workspace}/attribution",
            get(workspace_attribution).post(adopt),
        ),
        ("/v1/account/referrers/{referrer}/lineage", get(lineage)),
        ("/v1/account/referrers/{referrer}", get(show)),
        (
            "/v1/account/referrers/{referrer}/link",
            post(link).delete(disable),
        ),
        ("/v1/account/referrers/{referrer}/migration", post(migrate)),
        (
            "/v1/account/referrers/{referrer}/migration/accept",
            post(accept),
        ),
    ]
}
pub(crate) fn refused(error: Error) -> Response {
    let status = match error {
        Error::Unauthorized => StatusCode::FORBIDDEN,
        Error::Invalid => StatusCode::BAD_REQUEST,
        Error::Conflict => StatusCode::CONFLICT,
        Error::Bound => StatusCode::TOO_MANY_REQUESTS,
        Error::Unavailable => StatusCode::NOT_FOUND,
        Error::Store(_) => StatusCode::SERVICE_UNAVAILABLE,
    };
    accounts::refused(status, error.code(), "The referral operation was refused.")
}
fn answer(value: impl serde::Serialize) -> Response {
    (
        StatusCode::OK,
        Json(json!({"v":"openagents.accounts.v1", "referral": value})),
    )
        .into_response()
}
use axum::response::IntoResponse;
/// The published agreement shape. `current_referrer_owner` is in-process
/// custody input for native settlement review; it names another party's
/// account and is not part of the authenticated wire contract.
#[derive(serde::Serialize)]
struct AgreementWire<'a> {
    agreement: &'a commission::Agreement,
    terms: &'a commission::Publication,
    state: &'a str,
    terms_qualified: bool,
    active_for_new_transactions: bool,
    accrual_enabled: bool,
    payout_qualified: bool,
    payout_enabled: bool,
}
fn agreement_wire(v: &commission::View) -> AgreementWire<'_> {
    AgreementWire {
        agreement: &v.agreement,
        terms: &v.terms,
        state: &v.state,
        terms_qualified: v.terms_qualified,
        active_for_new_transactions: v.active_for_new_transactions,
        accrual_enabled: v.accrual_enabled,
        payout_qualified: v.payout_qualified,
        payout_enabled: v.payout_enabled,
    }
}
async fn actor(
    state: &ServeState,
    headers: &HeaderMap,
) -> Result<(tenancy::Accounts, String), Response> {
    let principal = accounts::principal(state, headers)?;
    let account = accounts::member_account(&principal)?.to_owned();
    if let Some(expected) = headers.get("x-openagents-referral-account") {
        if headers
            .get_all("x-openagents-referral-account")
            .iter()
            .count()
            != 1
            || expected.to_str().ok() != Some(account.as_str())
        {
            return Err(refused(Error::Conflict));
        }
    }
    Ok((accounts::accounts_store(state)?, account))
}

fn current(state: &ServeState, headers: &HeaderMap, account: &str) -> bool {
    accounts::principal(state, headers)
        .ok()
        .as_ref()
        .and_then(|p| accounts::member_account(p).ok())
        == Some(account)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyQuery {
    digest: Option<String>,
}
async fn commission_terms(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Query(query): Query<PolicyQuery>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.commission_publication_guarded(&account, query.digest.as_deref(), || {
        current(&state, &headers, &account)
    }) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgreementQuery {
    customer: String,
    agreement: Option<String>,
}
async fn commission_agreement(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Query(query): Query<AgreementQuery>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.commission_agreement_guarded(
        &account,
        &query.customer,
        query.agreement.as_deref(),
        || current(&state, &headers, &account),
    ) {
        Ok(v) => answer(v.as_ref().map(agreement_wire)),
        Err(e) => refused(e),
    }
}
async fn commission_accept(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: commission::Input = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store
        .accept_commission_terms_guarded(&account, &input, || current(&state, &headers, &account))
    {
        Ok(v) => answer(agreement_wire(&v)),
        Err(e) => refused(e),
    }
}
async fn policy(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Query(query): Query<PolicyQuery>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.attribution_policy_version(&account, query.digest.as_deref()) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn attributed(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.attribution(&account) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn propose(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: attribution::Proposal = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store
        .propose_attribution_guarded(&account, &input, || current(&state, &headers, &account))
    {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Confirmation {
    customer: String,
    decision: String,
}
async fn confirm(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Confirmation = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.confirm_attribution_guarded(&account, &input.customer, &input.decision, || {
        current(&state, &headers, &account)
    }) {
        Ok(v) => answer(
            json!({"customer":v.customer,"decision":v.digest,"policy_digest":v.policy_digest,"status":v.status,"referrer":v.referrer,"commission_eligibility":false}),
        ),
        Err(e) => refused(e),
    }
}
async fn workspace_attribution(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.workspace_attribution(&account, &workspace) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Adoption {
    decision: String,
}
async fn adopt(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Adoption = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.adopt_workspace_attribution_guarded(&account, &workspace, &input.decision, || {
        current(&state, &headers, &account)
    }) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn lineage(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.referrer_successors(&account, &id) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Join {
    #[serde(rename = "ref")]
    token: String,
}
/// Public source lookup material grants no account access or payout right.
async fn join(Query(input): Query<Join>) -> Response {
    if !input.token.strip_prefix("rfr_").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return refused(Error::Invalid);
    }
    answer(
        json!({"token":input.token,"consent_version":referrals::CONSENT,"consent_text":"Allow this introduction source to be recorded privately with your account. This records an introduction and grants no commission or payment right.","signup":"/v1/accounts","capture":"/v1/account/acquisition"}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    kind: Kind,
    label: String,
}
async fn create(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Create = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.create_referrer(&account, input.kind, &input.label) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn show(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.referrer(&account, &id) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn link(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.issue_referral_link(&account, &id) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn disable(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.disable_referral_links(&account, &id) {
        Ok(()) => answer(json!({"disabled":true})),
        Err(e) => refused(e),
    }
}
async fn source(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.acquisition(&account) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn capture(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Capture = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.capture_acquisition(&account, &input) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    account: String,
}
async fn migrate(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Migration = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.offer_referrer_migration_guarded(&account, &id, &input.account, || {
        current(&state, &headers, &account)
    }) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn accept(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store
        .accept_referrer_migration_guarded(&account, &id, || current(&state, &headers, &account))
    {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
