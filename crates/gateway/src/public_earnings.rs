//! Account-owned publication of redacted Pylon, plugin, and tip receipts.
mod postgres;
use crate::serve::ServeState;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{MethodRouter, get, post},
};
use nostr::{domain::RelaySigner, earnings};
use pay_ledger::public_earnings::{Preview, Published, Source, Store};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeSet, path::Path as FilePath, sync::Arc, time::Duration};

pub(crate) fn open(dir: &FilePath) -> Result<(Store, RelaySigner), String> {
    let store = if let Some(database) = tenancy::db::bound(dir) {
        Store::with_backend(Box::new(postgres::Postgres::new(database)))
    } else {
        let private = dir.join("public-earnings");
        std::fs::create_dir_all(&private).map_err(|_| "could not create receipt directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "could not protect receipt directory")?;
        }
        Store::open(&private.join("receipts.sqlite")).map_err(|_| "could not open receipt store")?
    };
    let candidate = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let encoded = zeroize::Zeroizing::new(hex::encode(candidate.secret_bytes()));
    let signer = store
        .signer(&encoded)
        .map_err(|_| "could not open receipt signing key")?;
    Ok((store, signer))
}

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/account/earnings/{party}", get(own)),
        ("/v1/account/earnings/{party}/receipts", post(publish)),
        (
            "/v1/account/earnings/{party}/receipts/{source}",
            axum::routing::delete(hide),
        ),
        ("/v1/earnings/public/{party}", get(public)),
        ("/v1/earnings/public/receipts/{id}", get(event)),
    ]
    .into_iter()
    .map(|(path, route)| {
        (
            path,
            route.layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
    })
    .collect()
}
fn answer(status: StatusCode, value: serde_json::Value) -> Response {
    let mut response = (status, Json(value)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store, private".parse().unwrap());
    response
}
fn unavailable() -> Response {
    answer(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error":{"code":"earnings_unavailable","message":"Your earnings couldn't be reached. Try again later."}}),
    )
}
fn invalid() -> Response {
    answer(
        StatusCode::BAD_REQUEST,
        json!({"error":{"code":"earnings_invalid","message":"Choose a receipt from your earnings."}}),
    )
}
fn missing() -> Response {
    answer(
        StatusCode::NOT_FOUND,
        json!({"error":{"code":"receipt_not_found","message":"That receipt isn't available."}}),
    )
}
fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn authorized(state: &ServeState, headers: &HeaderMap, party: &str) -> Result<String, Response> {
    let account = crate::nostr_sign_in::caller(state, headers)?;
    let accounts = tenancy::Accounts::open(&state.dir)
        .and_then(|book| book.store())
        .map_err(|_| unavailable())?;
    if !hex64(party)
        || accounts.accounts.get(&account).is_none_or(|record| {
            !tenancy::accounts::nostr::keys_of(record)
                .iter()
                .any(|key| key == party)
        })
    {
        return Err(answer(
            StatusCode::FORBIDDEN,
            json!({"error":{"code":"earnings_forbidden","message":"Choose your own profile to view its earnings."}}),
        ));
    }
    Ok(account)
}
fn visible(accounts: &tenancy::accounts::Store, row: &Published) -> bool {
    let Ok(receipt) = earnings::read(&row.event) else {
        return false;
    };
    accounts
        .accounts
        .get(&row.account)
        .is_some_and(|account| tenancy::accounts::nostr::keys_of(account).contains(&receipt.party))
}
async fn refresh_tips(state: &ServeState, account: &str, party: &str) {
    let Some(book) = &state.public_receipts else {
        return;
    };
    let waiting = book
        .lock()
        .await
        .waiting_tips(account, party, 8)
        .unwrap_or_default();
    let work = async {
        use futures_util::{StreamExt, stream};
        let results = stream::iter(waiting)
            .map(|invoice| async move {
                let value = crate::lightning_address::verification(&invoice.payment_hash)
                    .await
                    .ok()?;
                match value["settled"].as_bool()? {
                    true => Some((
                        invoice.payment_hash,
                        Some(value["preimage"].as_str()?.to_owned()),
                    )),
                    false => Some((invoice.payment_hash, None)),
                }
            })
            .buffer_unordered(4)
            .filter_map(|value| async move { value })
            .collect::<Vec<_>>()
            .await;
        let book = book.lock().await;
        for (hash, preimage) in results {
            if let Some(preimage) = preimage {
                let _ = book.confirm(&hash, &preimage, crate::accounts::unix_now());
            } else {
                let _ = book.verified_unpaid(&hash, crate::accounts::unix_now());
            }
        }
        let _ = book.prune_unpaid(crate::accounts::unix_now());
    };
    let _ = tokio::time::timeout(Duration::from_secs(3), work).await;
}
async fn previews(
    state: &ServeState,
    account: &str,
    party: &str,
) -> Result<Vec<Preview>, Response> {
    refresh_tips(state, account, party).await;
    let Some(book) = &state.public_receipts else {
        return Err(unavailable());
    };
    let mut rows = Vec::new();
    if let Some(ledger) = &state.earnings {
        let mut ledger = ledger.lock().await;
        for source in ledger
            .public_earning_sources(party, 20)
            .map_err(|_| unavailable())?
        {
            if let Source::Ledger { sequence, role } = &source
                && let Some((source, receipt)) = ledger
                    .public_earning(party, *sequence, role)
                    .map_err(|_| unavailable())?
            {
                rows.push(Preview {
                    source,
                    receipt,
                    published: false,
                });
            }
        }
    }
    let book = book.lock().await;
    for row in &mut rows {
        row.published = book
            .is_published(&row.receipt.source)
            .map_err(|_| unavailable())?;
    }
    rows.extend(book.tips(account, party, 20).map_err(|_| unavailable())?);
    rows.sort_by(|a, b| {
        b.receipt
            .recorded_at
            .cmp(&a.receipt.recorded_at)
            .then(a.receipt.source.cmp(&b.receipt.source))
    });
    Ok(rows)
}
async fn own(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    headers: HeaderMap,
) -> Response {
    let account = match authorized(&state, &headers, &party) {
        Ok(a) => a,
        Err(e) => return e,
    };
    match previews(&state, &account, &party).await {
        Ok(rows) => match authorized(&state, &headers, &party) {
            Ok(current) if current == account => answer(StatusCode::OK, json!({"rows":rows})),
            Ok(_) => invalid(),
            Err(error) => error,
        },
        Err(e) => e,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publish {
    source: Source,
}
async fn publish(
    State(state): State<Arc<ServeState>>,
    Path(party): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let account = match authorized(&state, &headers, &party) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let Ok(request) = serde_json::from_slice::<Publish>(&body) else {
        return invalid();
    };
    let receipt = match &request.source {
        Source::Ledger { sequence, role } => {
            let Some(ledger) = &state.earnings else {
                return missing();
            };
            match ledger.lock().await.public_earning(&party, *sequence, role) {
                Ok(Some((_, receipt))) => receipt,
                Ok(None) => return missing(),
                Err(_) => return invalid(),
            }
        }
        Source::Tip { payment_hash } => {
            if !hex64(payment_hash) {
                return invalid();
            }
            let Some(book) = &state.public_receipts else {
                return unavailable();
            };
            let invoice = book.lock().await.invoice(payment_hash).ok().flatten();
            if invoice.is_none_or(|invoice| {
                invoice.account != account || invoice.party != party || !invoice.tip
            }) {
                return missing();
            }
            if let Ok(value) = crate::lightning_address::verification(payment_hash).await
                && value["settled"] == true
                && let Some(preimage) = value["preimage"].as_str()
            {
                if book
                    .lock()
                    .await
                    .confirm(payment_hash, preimage, crate::accounts::unix_now())
                    .is_err()
                {
                    return unavailable();
                }
            }
            match book.lock().await.tip(&account, &party, payment_hash) {
                Ok(Some(row)) => row.receipt,
                _ => return missing(),
            }
        }
    };
    if authorized(&state, &headers, &party).is_err() {
        return missing();
    }
    let (Some(book), Some(signer)) = (&state.public_receipts, &state.receipt_signer) else {
        return unavailable();
    };
    match book.lock().await.publish(
        &account,
        &request.source,
        &receipt,
        signer,
        crate::accounts::unix_now(),
    ) {
        Ok(event) => answer(StatusCode::OK, json!({"event":event})),
        Err(_) => unavailable(),
    }
}
async fn hide(
    State(state): State<Arc<ServeState>>,
    Path((party, source)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let account = match authorized(&state, &headers, &party) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if !hex64(&source) {
        return invalid();
    }
    let Some(book) = &state.public_receipts else {
        return unavailable();
    };
    match book.lock().await.hide(&account, &party, &source) {
        Ok(()) => answer(StatusCode::OK, json!({"hidden":true})),
        Err(_) => unavailable(),
    }
}
async fn public(State(state): State<Arc<ServeState>>, Path(party): Path<String>) -> Response {
    if !hex64(&party) {
        return missing();
    }
    let Some(book) = &state.public_receipts else {
        return unavailable();
    };
    let accounts = match tenancy::Accounts::open(&state.dir).and_then(|book| book.store()) {
        Ok(a) => a,
        Err(_) => return unavailable(),
    };
    let selected = book.lock().await.published(&party, 20).unwrap_or_default();
    if let (Some(ledger), Some(signer)) = (&state.earnings, &state.receipt_signer) {
        for row in selected.iter().filter(|row| visible(&accounts, row)) {
            if let Source::Ledger { sequence, role } = &row.source {
                let current = ledger
                    .lock()
                    .await
                    .public_earning(&party, *sequence, role)
                    .ok()
                    .flatten();
                if let Some((_, receipt)) = current {
                    let _ = book.lock().await.refresh(
                        row,
                        &receipt,
                        signer,
                        crate::accounts::unix_now(),
                    );
                }
            }
        }
    }
    match book.lock().await.published(&party, 20) {
        Ok(rows) => {
            let events: Vec<_> = rows
                .into_iter()
                .filter(|row| visible(&accounts, row))
                .map(|row| row.event)
                .collect();
            let issuers: BTreeSet<_> = events.iter().map(|event| event.pubkey.clone()).collect();
            answer(StatusCode::OK, json!({"events":events,"issuers":issuers}))
        }
        Err(_) => unavailable(),
    }
}
async fn event(State(state): State<Arc<ServeState>>, Path(id): Path<String>) -> Response {
    if !hex64(&id) {
        return missing();
    }
    let Some(book) = &state.public_receipts else {
        return unavailable();
    };
    let accounts = match tenancy::Accounts::open(&state.dir).and_then(|book| book.store()) {
        Ok(a) => a,
        Err(_) => return unavailable(),
    };
    match book.lock().await.event(&id) {
        Ok(Some(row)) if visible(&accounts, &row) => {
            answer(StatusCode::OK, json!({"event":row.event}))
        }
        Ok(_) => missing(),
        Err(_) => unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_receipts_disappear_when_the_account_no_longer_holds_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = tenancy::Accounts::install(dir.path()).unwrap();
        let party = "11".repeat(32);
        let account = accounts
            .sign_in_nostr(&party, "receipt-test")
            .unwrap()
            .account;
        let signer = RelaySigner::from_secret_hex(&"04".repeat(32)).unwrap();
        let receipt = earnings::Receipt {
            v: earnings::SCHEMA.into(),
            party: party.clone(),
            source: "22".repeat(32),
            category: earnings::Category::Plugin,
            state: earnings::State::Earned,
            amount_msat: 1000,
            network: "bitcoin".into(),
            recorded_at: 1,
            evidence: None,
            plugin: Some("plugin".into()),
            release: None,
        };
        let row = Published {
            account: account.id.clone(),
            source: Source::Ledger {
                sequence: 1,
                role: "author".into(),
            },
            event: receipt.event(&signer, 2).unwrap(),
        };
        assert!(visible(&accounts.store().unwrap(), &row));
        let mut detached = accounts.store().unwrap();
        detached
            .accounts
            .get_mut(&account.id)
            .unwrap()
            .principals
            .clear();
        assert!(!visible(&detached, &row));
        accounts.delete_account(&account.id).unwrap();
        assert!(!visible(&accounts.store().unwrap(), &row));
    }
}
