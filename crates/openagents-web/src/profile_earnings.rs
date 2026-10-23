//! Earnings API bridge and the public profile's selected receipt list.
use crate::App;
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use maud::{Markup, html};
use nostr::{
    domain::Event,
    earnings::{self, Category, State as EarningState},
};
use openagents_ui::content::MarkdownRoot;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/device/earnings/{party}", any(private))
        .route("/v1/device/earnings/{party}/receipts", any(private))
        .route(
            "/v1/device/earnings/{party}/receipts/{source}",
            any(private),
        )
        .route("/v1/earnings/public/{party}", get(public))
        .route("/v1/earnings/public/{party}/sources/{source}", get(public))
        .route("/v1/earnings/public/receipts/{id}", get(public))
        .layer(axum::extract::DefaultBodyLimit::max(4096))
}
pub(crate) fn owns(path: &str) -> bool {
    path.starts_with("/v1/device/earnings/") || path.starts_with("/v1/earnings/public/")
}
fn reply(status: StatusCode, body: Value) -> Response {
    let mut response = (status, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store, private".parse().unwrap());
    response
}
fn unavailable() -> Response {
    reply(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error":{"code":"earnings_unavailable","message":"Your earnings couldn't be reached. Try again later."}}),
    )
}
async fn private(
    State(app): State<App>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !matches!(method, Method::GET | Method::POST | Method::DELETE) {
        return reply(
            StatusCode::METHOD_NOT_ALLOWED,
            json!({"error":{"code":"method_not_allowed","message":"Choose a receipt from your earnings."}}),
        );
    }
    if !valid_path(uri.path()) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Some(authorization) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("Bearer ") && value.len() < 8192)
    else {
        return reply(
            StatusCode::UNAUTHORIZED,
            json!({"error":{"code":"unauthenticated","message":"Log in again to continue."}}),
        );
    };
    forward(
        &app,
        method,
        &uri.path().replacen("/v1/device/", "/v1/account/", 1),
        Some(authorization),
        &body,
    )
    .await
}
async fn public(State(app): State<App>, uri: Uri) -> Response {
    if !valid_path(uri.path()) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    forward(&app, Method::GET, uri.path(), None, &[]).await
}
async fn forward(
    app: &App,
    method: Method,
    path: &str,
    authorization: Option<&str>,
    body: &[u8],
) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    match service
        .earnings_call(method, path, authorization, body)
        .await
    {
        Ok(answer) => reply(
            StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY),
            answer.body,
        ),
        Err(_) => unavailable(),
    }
}
#[derive(Default, Deserialize, Serialize)]
pub(crate) struct Published {
    pub events: Vec<Event>,
    pub issuers: BTreeSet<String>,
}
pub(crate) async fn load(app: &App, party: &str) -> Published {
    let read = async {
        let service = app.config.cloud.as_deref()?;
        let reply = service
            .earnings_call(
                Method::GET,
                &format!("/v1/earnings/public/{party}"),
                None,
                &[],
            )
            .await
            .ok()?;
        if reply.status != 200 {
            return None;
        }
        serde_json::from_value(reply.body).ok()
    };
    tokio::time::timeout(Duration::from_secs(2), read)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}
pub(crate) async fn event(app: &App, id: &str) -> Option<Event> {
    let read = async {
        let service = app.config.cloud.as_deref()?;
        let reply = service
            .earnings_call(
                Method::GET,
                &format!("/v1/earnings/public/receipts/{id}"),
                None,
                &[],
            )
            .await
            .ok()?;
        if reply.status != 200 {
            return None;
        }
        let event: Event = serde_json::from_value(reply.body["event"].clone()).ok()?;
        (event.id == id && earnings::read(&event).is_ok()).then_some(event)
    };
    tokio::time::timeout(Duration::from_secs(2), read)
        .await
        .ok()
        .flatten()
}
pub(crate) fn section(published: &Published, party: &str, now: u64) -> Markup {
    let rows = earnings::selected(&published.events, party, &published.issuers, now);
    if rows.is_empty() {
        return html! {};
    }
    html! {
        section.oa-card aria-labelledby="profile-earnings" {
            (MarkdownRoot::new(html! {
                h2 id="profile-earnings" {"Published earnings"}
                ul {
                    @for (event,receipt) in rows.iter().take(20) {
                        li {
                            (match receipt.category {Category::Pylon=>"Pylon",Category::Plugin=>"Plugin sales",Category::Tip=>"Tips"})
                            " · " (bitcoin_amount::Format::Bip177.show_msat(receipt.amount_msat)) " · "
                            (match receipt.state {EarningState::Pending=>"Pending",EarningState::Earned=>"Earned",EarningState::Paid=>"Paid",EarningState::Reversed=>"Reversed"})
                            " · " a href=(format!("/event/{}",event.id)) {"Receipt"}
                        }
                    }
                }
            }))
        }
    }
}

fn valid_path(path: &str) -> bool {
    let key = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    let parts: Vec<_> = path.split('/').collect();
    match parts.as_slice() {
        ["", "v1", "device", "earnings", party]
        | ["", "v1", "device", "earnings", party, "receipts"] => key(party),
        ["", "v1", "device", "earnings", party, "receipts", source] => key(party) && key(source),
        ["", "v1", "earnings", "public", party] => key(party),
        ["", "v1", "earnings", "public", party, "sources", source] => key(party) && key(source),
        ["", "v1", "earnings", "public", "receipts", id] => key(id),
        _ => false,
    }
}

pub(crate) fn extend_contract(document: &mut Value) {
    for (method, path, id, private) in [
        (
            "get",
            "/v1/device/earnings/{party}",
            "profileEarnings",
            true,
        ),
        (
            "post",
            "/v1/device/earnings/{party}/receipts",
            "publishEarning",
            true,
        ),
        (
            "delete",
            "/v1/device/earnings/{party}/receipts/{source}",
            "hideEarning",
            true,
        ),
        (
            "get",
            "/v1/earnings/public/{party}",
            "publishedEarnings",
            false,
        ),
        (
            "get",
            "/v1/earnings/public/receipts/{id}",
            "publishedEarningReceipt",
            false,
        ),
        (
            "get",
            "/v1/earnings/public/{party}/sources/{source}",
            "currentPublishedEarning",
            false,
        ),
    ] {
        let mut op = json!({"operationId":id,"tags":["Earnings"],"summary":"Read or choose profile earnings receipts","description":"Private previews require the account session and an attached Nostr key. Publication is explicit. Public receipts omit account IDs, payment hashes, preimages, and payout destinations.","responses":{"200":{"description":"Earnings or the selected receipt","content":{"application/json":{"schema":{"type":"object"}}}},"401":{"description":"Log in again"},"403":{"description":"Choose your own profile"},"503":{"description":"Earnings are unavailable"}}});
        let parameters:Vec<_>= ["party","source","id"].iter().filter(|p|path.contains(&format!("{{{p}}}"))).map(|p|json!({"name":p,"in":"path","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}})).collect();
        op["parameters"] = json!(parameters);
        op["security"] = if private {
            json!([{"appBearer":[]}])
        } else {
            json!([])
        };
        if method == "post" {
            op["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"type":"object","required":["source"],"additionalProperties":false,"properties":{"source":{"oneOf":[{"type":"object","required":["kind","sequence","role"],"properties":{"kind":{"const":"ledger"},"sequence":{"type":"integer","minimum":1},"role":{"type":"string","maxLength":64}}},{"type":"object","required":["kind","payment_hash"],"properties":{"kind":{"const":"tip"},"payment_hash":{"type":"string","pattern":"^[0-9a-f]{64}$"}}}]}}}}}});
        }
        document["paths"][path][method] = op;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_lists_verified_selected_receipts_without_wallet_totals() {
        let signer = nostr::domain::RelaySigner::from_secret_hex(&"04".repeat(32)).unwrap();
        let party = "11".repeat(32);
        let receipt = earnings::Receipt {
            v: earnings::SCHEMA.into(),
            party: party.clone(),
            source: "22".repeat(32),
            category: Category::Tip,
            state: EarningState::Paid,
            amount_msat: 12001,
            network: "bitcoin".into(),
            recorded_at: 1,
            evidence: None,
            plugin: None,
            release: None,
        };
        let event = receipt.event(&signer, 2).unwrap();
        let published = Published {
            events: vec![event.clone(), event.clone()],
            issuers: BTreeSet::from([signer.pubkey().into()]),
        };
        let shown = section(&published, &party, 3).into_string();
        assert_eq!(shown.matches(&format!("/event/{}", event.id)).count(), 1);
        assert!(shown.contains("Tips") && shown.contains("12,001 msat"));
        let card = receipt_card(&event).unwrap().into_string();
        assert!(
            card.contains("Earnings receipt")
                && card.contains("12,001 msat")
                && card.contains("Signed receipt")
        );
        assert!(!shown.contains("balance") && !shown.contains("Total"));
        assert!(
            section(&published, &"33".repeat(32), 3)
                .into_string()
                .is_empty()
        );
        let untrusted = Published {
            issuers: BTreeSet::new(),
            ..published
        };
        assert!(section(&untrusted, &party, 3).into_string().is_empty());
    }
    #[test]
    fn bridge_paths_cannot_escape_the_earnings_surface() {
        let party = "11".repeat(32);
        assert!(valid_path(&format!("/v1/device/earnings/{party}/receipts")));
        assert!(valid_path(&format!("/v1/earnings/public/receipts/{party}")));
        assert!(valid_path(&format!(
            "/v1/earnings/public/{party}/sources/{party}"
        )));
        for path in [
            "/v1/device/earnings/..%2faccount",
            "/v1/device/earnings/../account",
            "/v1/earnings/public/receipts/%2e%2e",
        ] {
            assert!(!valid_path(path));
        }
    }
}

/// A selected service receipt's human-readable amount, state, and work evidence.
pub(crate) fn receipt_card(event: &Event) -> Option<Markup> {
    let receipt = earnings::read(event).ok()?;
    let pretty = serde_json::to_string_pretty(event).ok()?;
    Some(html! {section.oa-card {(MarkdownRoot::new(html! {
        h1 {"Earnings receipt"}
        p {(match receipt.category {Category::Pylon=>"Pylon",Category::Plugin=>"Plugin sales",Category::Tip=>"Tips"}) " · " (bitcoin_amount::Format::Bip177.show_msat(receipt.amount_msat)) " · " (match receipt.state {EarningState::Pending=>"Pending",EarningState::Earned=>"Earned",EarningState::Paid=>"Paid",EarningState::Reversed=>"Reversed"})}
        p {a href=(crate::profiles::href(&receipt.party,None)) {"Profile"}}
        @if let Some(plugin)=receipt.plugin {p {"Plugin: " (plugin)}}
        @if let Some(release)=receipt.release {p {"Release: " (release)}}
        @if let Some(evidence)=receipt.evidence {p {a href=(format!("/event/{evidence}")) {"Work receipt"}}}
        details {summary {"Signed receipt"} pre {code {(pretty)}}}
    }))}})
}
