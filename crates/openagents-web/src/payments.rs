//! The ways to pay per request that the API this site points at takes
//! right now (#11137), read from the gateway's own OpenAPI document
//! (`x-openagents-payment-methods`, the payment router's live list), so
//! `llms.txt`, `auth.md`, the catalogs, the agent card, the docs MCP
//! server, and the For agents page name a method only while the API
//! takes it. With no gateway, or one that doesn't answer in time, the list
//! is empty and every surface says to use a key.

use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Method as HttpMethod, Request, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::App;

/// Where the For agents page draws the table of ways to pay.
pub(crate) const TABLE: &str = "{{payment methods}}";
/// Where a guide draws the one-line summary of them.
pub(crate) const SENTENCE: &str = "{{ways to pay}}";

const WAIT: Duration = Duration::from_secs(2);
const LIMIT: usize = 4 * 1024 * 1024;

/// One way to pay per request, as the gateway describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Method {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub rail: String,
    pub challenge: String,
    pub credential: String,
    pub receipt: String,
    pub spec: String,
}

/// The ways to pay the gateway takes now; empty without one.
pub(crate) async fn live(app: &App) -> Vec<Method> {
    read(app).await.unwrap_or_default()
}

async fn read(app: &App) -> Option<Vec<Method>> {
    let gateway = app.config.inference.as_ref()?;
    let request = Request::builder()
        .method(HttpMethod::GET)
        .uri("/v1/openapi.json")
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .ok()?;
    let response = tokio::time::timeout(WAIT, gateway.forward(request))
        .await
        .ok()?;
    if response.status() != StatusCode::OK {
        return None;
    }
    let body = tokio::time::timeout(WAIT, to_bytes(response.into_body(), LIMIT))
        .await
        .ok()?
        .ok()?;
    let document: Value = serde_json::from_slice(&body).ok()?;
    serde_json::from_value(document.get("x-openagents-payment-methods")?.clone()).ok()
}

/// One line: what an agent with no key can pay with here.
pub(crate) fn sentence(methods: &[Method]) -> String {
    if methods.is_empty() {
        return "Paying per request is off on this API right now; use an API key.".to_owned();
    }
    let ways: Vec<String> = methods
        .iter()
        .map(|method| format!("{} (`{}`)", method.name, method.credential))
        .collect();
    format!(
        "With no key, pay for each request over Lightning: {}. One invoice per request, \
paid once.",
        ways.join(", or ")
    )
}

/// The For agents page's table: each way to pay this API takes now, then
/// the ones still to come.
pub(crate) fn table(methods: &[Method]) -> String {
    let mut out = String::from(
        "| Method | Challenge | You send | You get back | Status |\n| --- | --- | --- | --- | --- |\n\
| A key with credit, bought by card | `401` without one | `Authorization: Bearer oak_...` | the answer | Works now |\n",
    );
    for method in methods {
        out.push_str(&format!(
            "| [{}]({}) | `{}` | `{}` | `{}` | Works now |\n",
            method.name, method.spec, method.challenge, method.credential, method.receipt
        ));
    }
    let coming = [
        (
            "x402",
            "[x402](https://www.x402.org/) on Lightning (`PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`)",
        ),
        (
            "mpp",
            "The `Payment` scheme on Lightning, as used by [MPP](https://mpp.dev/) (`WWW-Authenticate: Payment`)",
        ),
        (
            "l402",
            "[L402](https://docs.lightning.engineering/the-lightning-network/l402) (`WWW-Authenticate: L402`)",
        ),
        (
            "taproot-assets",
            "Dollar stablecoins on Bitcoin ([Taproot Assets](https://github.com/OpenAgentsInc/tap-ldk)) over Lightning",
        ),
        (
            "checkout",
            "Agent checkout for credits and Pro: [ACP](https://www.agenticcommerce.dev/), [UCP](https://ucp.dev/), [AP2](https://ap2-protocol.org/)",
        ),
    ];
    for (id, name) in coming {
        if methods.iter().any(|method| method.id == id) {
            continue;
        }
        out.push_str(&format!("| {name} | | | | Coming |\n"));
    }
    out
}

/// The ids, for the machine-read catalogs.
pub(crate) fn ids(methods: &[Method]) -> Vec<String> {
    methods.iter().map(|method| method.id.clone()).collect()
}
