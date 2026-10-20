//! The browser's wallet, phone-approved and spend-limited (#12258).
//!
//! The browser holds no wallet and no key. It asks; the person's phone,
//! which holds the Spark wallet from their 12 words, decides and pays. This
//! is the agent spend protocol (`docs/breez/spend-protocol.md`) with the
//! website as the asking side: the same `openagents.spend-grant.v1`,
//! `openagents.spend-request.v1`, and `openagents.spend-receipt.v1`
//! documents (`coder_access::spend`), over HTTPS instead of NIP-HOST.
//!
//! - `GET /v1/wallet/browser`: the browser's grantee key, the phone's
//!   grant if any, and the requests with their receipts.
//! - `PUT /v1/wallet/browser/grant` `{grant}`, from the phone: the bounds
//!   it allows (one payment's cap, a day's, the grant's life). The grantee
//!   must be this account's browser key.
//! - `POST /v1/wallet/browser/requests` `{payment, purpose, resource?,
//!   note?}`, from the browser: asks to pay a BOLT11 invoice. Refused at
//!   once without a grant or over its caps; the phone checks again.
//! - `GET /v1/wallet/browser/requests/{id}`: one request and its receipt.
//! - `POST /v1/wallet/browser/requests/{id}/receipt` `{receipt}`, from the
//!   phone: what it did. A `paid` receipt must prove the payment (its
//!   preimage hashes to the invoice's payment hash), so the browser can
//!   hand the preimage to x402.
//!
//! Every route acts as the signed-in account. The grantee key is derived
//! from the site's agent key seed and the account, so nothing secret is
//! stored and no request carries a word or a private key.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post, put};
use coder_access::spend::{Context, Grant, Purpose, REQUEST, Receipt, SpendRequest};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;

use crate::App;
use crate::chat_store::{Error, Store};
use crate::coder_sync::{answer, refused, stored};

/// The longest a browser's request stands, as the host's default.
const REQUEST_TTL: u64 = 10 * 60;
/// The most requests kept per account.
const KEPT: usize = 64;

pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/wallet/browser" || path.starts_with("/v1/wallet/browser/")
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/wallet/browser", get(show))
        .route("/v1/wallet/browser/grant", put(set_grant))
        .route("/v1/wallet/browser/requests", post(ask))
        .route("/v1/wallet/browser/requests/{id}", get(one))
        .route("/v1/wallet/browser/requests/{id}/receipt", post(settle))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    #[serde(default)]
    grant: Option<Grant>,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    request: SpendRequest,
    #[serde(default)]
    receipt: Option<Receipt>,
}

fn now() -> u64 {
    crate::cloud::session::now()
}

/// The browser's grantee key for `account`: an x-only public key, hex.
pub(crate) fn grantee(app: &App, account: &str) -> Option<String> {
    let seed = app.config.agent_key_seed.as_ref()?;
    for round in 0u8..8 {
        let mut mac = Hmac::<Sha256>::new_from_slice(seed).ok()?;
        mac.update(b"openagents.browser-wallet.v1\0");
        mac.update(account.as_bytes());
        mac.update(&[round]);
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if let Ok(signer) = nostr::domain::RelaySigner::from_secret_hex(&hex) {
            return Some(signer.pubkey().to_string());
        }
    }
    None
}

fn key(owner: &str) -> Result<String, Error> {
    Store::owner_key(owner, "browser-spend.json")
}

async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    match store.read_key(&key(owner)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The wallet requests could not be opened."))?,
            Some(generation),
        )),
        None => Ok((Book::default(), None)),
    }
}

async fn change<T>(
    store: &Store,
    owner: &str,
    f: impl Fn(&mut Book) -> Result<T, Response>,
) -> Result<Result<T, Response>, Error> {
    for _ in 0..16 {
        let (mut book, generation) = load(store, owner).await?;
        let out = match f(&mut book) {
            Ok(out) => out,
            Err(response) => return Ok(Err(response)),
        };
        let bytes = serde_json::to_vec(&book)
            .map_err(|_| Error::Invalid("The wallet requests could not be saved."))?;
        match store
            .write_key(&key(owner)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(Ok(out)),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

/// The caller's account and its chat-store owner.
async fn who(app: &App, headers: &HeaderMap) -> Result<(String, String), Response> {
    let account = crate::coder_sync::account(app, headers).await?;
    let owner = crate::chat_store::account_owner(&account);
    Ok((account, owner))
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Paying from the browser isn't set up here.",
    )
}

fn bad(message: &str) -> Response {
    refused(StatusCode::BAD_REQUEST, "invalid_request", message)
}

async fn show(State(app): State<App>, headers: HeaderMap) -> Response {
    let (account, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    let Some(grantee) = grantee(&app, &account) else {
        return unavailable();
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((book, _)) => answer(
            StatusCode::OK,
            json!({"grantee": grantee, "grant": book.grant, "requests": book.entries}),
        ),
        Err(e) => stored(&e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetGrant {
    grant: Grant,
}

async fn set_grant(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<SetGrant>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (account, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    let Some(grantee) = grantee(&app, &account) else {
        return unavailable();
    };
    let Ok(Json(body)) = body else {
        return bad("Send the grant from your phone.");
    };
    let grant = body.grant;
    if grant.validate().is_err() || grant.grantee != grantee || grant.expires_at <= now() {
        return bad("That grant isn't for this browser.");
    }
    match change(&app.config.chat_store, &owner, |book| {
        book.grant = Some(grant.clone());
        Ok(())
    })
    .await
    {
        Ok(Ok(())) => answer(StatusCode::OK, json!({"grant": grant})),
        Ok(Err(r)) => r,
        Err(e) => stored(&e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ask {
    payment: String,
    purpose: Purpose,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

/// The request the browser's ask makes under `grant`, or why not.
fn request_for(grant: &Grant, ask: &Ask, now: u64) -> Result<SpendRequest, &'static str> {
    if grant.expires_at <= now {
        return Err("Your phone's permission for this browser ran out. Open the app to renew it.");
    }
    if !grant.purposes.contains(&ask.purpose) {
        return Err("Your phone doesn't allow this kind of payment from the browser.");
    }
    let mut request = SpendRequest {
        v: REQUEST.into(),
        requires: vec![],
        request: SpendRequest::id_for(&ask.payment),
        grant: grant.grant.clone(),
        epoch: grant.epoch,
        grantee: grant.grantee.clone(),
        payment: ask.payment.trim().to_string(),
        amount_msat: 0,
        fee_max_msat: 0,
        purpose: ask.purpose,
        context: Context {
            task: None,
            title: Some("openagents.com".into()),
            resource: ask.resource.clone(),
            note: ask.note.clone(),
        },
        issued_at: now,
        expires_at: now + REQUEST_TTL,
    };
    let invoice = request
        .invoice()
        .map_err(|_| "That isn't a Lightning invoice we can pay.")?;
    request.amount_msat = invoice.amount_msat;
    request.expires_at = request.expires_at.min(invoice.expires_at());
    request.fee_max_msat = grant.fee_max.ceiling(invoice.amount_msat);
    if request.amount_msat.saturating_add(request.fee_max_msat) > grant.per_payment_max {
        return Err("That's more than your phone allows for one payment from the browser.");
    }
    request
        .validate()
        .map_err(|_| "That invoice can't be paid from the browser.")?;
    Ok(request)
}

async fn ask(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Ask>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (_, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    let Ok(Json(ask)) = body else {
        return bad("Send the invoice to pay and what it's for.");
    };
    let at = now();
    let made = change(&app.config.chat_store, &owner, |book| {
        let Some(grant) = book.grant.clone() else {
            return Err(refused(
                StatusCode::CONFLICT,
                "no_grant",
                "Open the OpenAgents app on your phone to let this browser ask for payments.",
            ));
        };
        let request = request_for(&grant, &ask, at)
            .map_err(|why| refused(StatusCode::FORBIDDEN, "not_allowed", why))?;
        if let Some(held) = book
            .entries
            .iter()
            .find(|e| e.request.request == request.request)
        {
            return Ok(held.request.clone());
        }
        book.entries.push(Entry {
            request: request.clone(),
            receipt: None,
        });
        if book.entries.len() > KEPT {
            book.entries.remove(0);
        }
        Ok(request)
    })
    .await;
    match made {
        Ok(Ok(request)) => answer(StatusCode::OK, json!({"request": request})),
        Ok(Err(r)) => r,
        Err(e) => stored(&e),
    }
}

async fn one(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (_, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((book, _)) => match book.entries.iter().find(|e| e.request.request == id) {
            Some(entry) => answer(StatusCode::OK, json!(entry)),
            None => refused(
                StatusCode::NOT_FOUND,
                "not_found",
                "There's no such request.",
            ),
        },
        Err(e) => stored(&e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settle {
    receipt: Receipt,
}

async fn settle(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<Settle>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (_, owner) = match who(&app, &headers).await {
        Ok(found) => found,
        Err(r) => return r,
    };
    let Ok(Json(body)) = body else {
        return bad("Send the receipt from your phone.");
    };
    let receipt = body.receipt;
    let done = change(&app.config.chat_store, &owner, |book| {
        let entry = book
            .entries
            .iter_mut()
            .find(|e| e.request.request == id)
            .ok_or_else(|| {
                refused(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "There's no such request.",
                )
            })?;
        receipt
            .answers(&entry.request)
            .map_err(|_| bad("That receipt doesn't answer this request."))?;
        if entry.receipt.as_ref().is_some_and(Receipt::is_final) {
            return Ok(entry.receipt.clone());
        }
        entry.receipt = Some(receipt.clone());
        Ok(entry.receipt.clone())
    })
    .await;
    match done {
        Ok(Ok(receipt)) => answer(StatusCode::OK, json!({"receipt": receipt})),
        Ok(Err(r)) => r,
        Err(e) => stored(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::x402::test_invoice::{described, signed_at};

    fn grant(grantee: &str, now: u64) -> Grant {
        Grant::request_mode(
            "11".repeat(32),
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
            grantee,
            1,
            now,
        )
    }

    #[test]
    fn asks_are_bounded_by_the_phones_grant() {
        let now = 1_800_000_000;
        let grantee = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let g = grant(grantee, now);
        let small = signed_at(
            "lnbc10u",
            described([5; 32], "Search API call", 600),
            false,
            false,
            now,
        );
        let ask = |payment: &str| Ask {
            payment: payment.into(),
            purpose: Purpose::X402Purchase,
            resource: Some("https://example.com/search".into()),
            note: None,
        };
        let request = request_for(&g, &ask(&small), now).unwrap();
        assert_eq!(request.amount_msat, 1_000_000);
        assert_eq!(request.grantee, grantee);
        assert!(request.expires_at <= now + 600);
        // Over one payment's cap (10,000 sats by default).
        let big = signed_at(
            "lnbc200u",
            described([6; 32], "Big", 600),
            false,
            false,
            now,
        );
        assert!(request_for(&g, &ask(&big), now).is_err());
        // An expired grant, and something that isn't an invoice.
        assert!(request_for(&g, &ask(&small), g.expires_at).is_err());
        assert!(request_for(&g, &ask("not an invoice"), now).is_err());
    }
}
