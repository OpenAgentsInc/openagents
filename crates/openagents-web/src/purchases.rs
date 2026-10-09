//! The local paid-plugin purchase browser at `/app/purchases` (REV-44).
//!
//! It projects the purchases the installed `openagents plugin purchase`
//! client quoted, approved, paid, or recovered, read from the same customer
//! store and shown with the same payer, quote digest, approval digest, and
//! receipt. The browser is read only: it never opens the store for writing,
//! takes its lock, or reaches a wallet, so a reload cannot approve, pay, or
//! dispatch. Each page says which operations stay on the installed client.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use coder::customer::plugins::{Phase, Summary, browse};

use maud::PreEscaped;

use crate::App;
use crate::layout::{escape, problem};
use crate::ui_page::{UiPage, prose};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/app/purchases", get(purchases))
        .route("/app/purchases/{id}", get(purchase))
}

const UNAVAILABLE: &str = "<h2>Manage purchases in the app</h2><p>To buy, approve, or cancel a \
purchase, use the OpenAgents app on this computer. This page only shows them.</p>";

fn load(app: &App) -> Result<Vec<Summary>, Response> {
    let Some(root) = app.config.customer.clone() else {
        return Err(problem(
            StatusCode::NOT_FOUND,
            "Purchases are unavailable",
            "This server has no purchases to show.",
            ("/app", "Back to tasks"),
        ));
    };
    match browse(&root) {
        Ok(list) => Ok(list),
        Err(error) => Err(problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Purchases are unavailable",
            &error.to_string(),
            ("/app/purchases", "Try again"),
        )),
    }
}

fn phase_text(phase: Phase) -> &'static str {
    match phase {
        Phase::Quoted => "waiting for your approval in the app",
        Phase::Approved => "approved, not started yet",
        Phase::Cancelled => "cancelled before payment",
        Phase::Paying => "paying",
        Phase::Paid => "paid, waiting for the result",
        Phase::Completed => "done",
        Phase::Failed => "failed",
        Phase::Unknown => "we couldn't confirm the payment or result; check it in the app",
    }
}

fn msat(value: u64) -> String {
    format!("{value} msat")
}

fn option(value: Option<&str>) -> String {
    value.map_or_else(|| "unknown".to_string(), escape)
}

pub(crate) fn render_list(list: &[Summary]) -> String {
    let mut body = String::from(
        "<p class=\"oa-page-eyebrow\">LOCAL / READ ONLY</p><h1>Plugin purchases</h1>\
<p class=\"oa-page-meta\">Plugins you bought from this computer. \
<a href=\"/app/purchases\">Refresh</a></p><ul class=\"oa-item-list\">",
    );
    if list.is_empty() {
        body.push_str("<li><p class=\"oa-item-title\">No purchases</p><p>Plugins you buy in the app show up here.</p></li>");
    }
    for item in list {
        body.push_str(&format!(
            "<li><p><a class=\"oa-item-title\" href=\"/app/purchases/{id}\">{plugin}</a></p><p class=\"oa-page-meta\">{id} \u{b7} {phase:?} / {price}</p></li>",
            id = escape(&item.id),
            plugin = option(item.plugin.as_deref()),
            phase = item.phase,
            price = msat(item.price_msat),
        ));
    }
    body.push_str("</ul>");
    body.push_str(UNAVAILABLE);
    body
}

pub(crate) fn render_one(item: &Summary) -> String {
    let id = escape(&item.id);
    let mut body = format!(
        "<p class=\"oa-page-meta\"><a href=\"/app/purchases\">All purchases</a> / {id}</p><h1>{plugin}</h1>\
<p><a href=\"/app/purchases/{id}\">Refresh</a></p>\
<p class=\"oa-page-eyebrow\">LOCAL / READ ONLY</p><dl class=\"oa-facts\">\
<div><dt>Status</dt><dd>{phase_text}</dd></div>\
<div><dt>Release</dt><dd>{release}</dd></div>\
<div><dt>Endpoint</dt><dd>{url}</dd></div>\
<div><dt>Account / workspace</dt><dd>{account} / {workspace}</dd></div>\
<div><dt>Payer</dt><dd>{node} on {network}</dd></div>\
<div><dt>Price</dt><dd>{price}, fee at most {fee}</dd></div>\
<div><dt>Quote ID</dt><dd>{quote}</dd></div>\
<div><dt>Approval ID</dt><dd>{approval}</dd></div>\
<div><dt>Quote expires</dt><dd>{expires}</dd></div>",
        plugin = option(item.plugin.as_deref()),
        phase_text = phase_text(item.phase),
        release = option(item.release.as_deref()),
        url = escape(&item.url),
        account = escape(&item.account),
        workspace = escape(&item.workspace),
        node = escape(&item.payer_node),
        network = escape(&item.payer_network),
        price = msat(item.price_msat),
        fee = msat(item.max_fee_msat),
        quote = escape(&item.quote_digest),
        approval = escape(&item.approval_digest),
        expires = item.expires_at_ms,
    );
    match &item.charge {
        Some(charge) => body.push_str(&format!(
            "<div><dt>Charge</dt><dd>{} paid, {} fee, payment hash {}</dd></div>",
            msat(charge.amount_msat),
            msat(charge.fee_msat),
            escape(&charge.payment_hash)
        )),
        None => body.push_str("<div><dt>Charge</dt><dd>none</dd></div>"),
    }
    body.push_str(&format!(
        "<div><dt>Settlement</dt><dd>{}</dd></div>",
        match (item.settled, item.transaction.as_deref()) {
            (Some(true), Some(t)) => format!("settled, transaction {}", escape(t)),
            (Some(false), _) => "settlement failed".to_string(),
            _ => "unknown".to_string(),
        }
    ));
    body.push_str(&format!(
        "<div><dt>Delivery</dt><dd>{}</dd></div><div><dt>Result</dt><dd>{}</dd></div>",
        item.delivery_status
            .map_or_else(|| "unknown".to_string(), |s| format!("HTTP {s}")),
        if item.result_present {
            "saved; see it with <code>openagents plugin purchase show</code>"
        } else {
            "none"
        }
    ));
    if let Some(maximum) = item.unresolved_maximum_msat {
        body.push_str(&format!(
            "<div><dt>May still be charged</dt><dd>up to {}</dd></div>",
            msat(maximum)
        ));
    }
    if item.recovery_present {
        body.push_str("<div><dt>Recovery</dt><dd>checked</dd></div>");
    }
    body.push_str("</dl><h2>Next step</h2><p>Run this on your computer:</p>");
    let next = match item.phase {
        Phase::Quoted => format!(
            "openagents plugin purchase approve --root ROOT --purchase {id} --digest {}",
            escape(&item.approval_digest)
        ),
        Phase::Approved => format!("openagents plugin purchase invoke --root ROOT --purchase {id}"),
        Phase::Unknown | Phase::Paying | Phase::Paid => {
            format!("openagents plugin purchase recover --root ROOT --purchase {id}")
        }
        Phase::Cancelled | Phase::Completed | Phase::Failed => {
            format!("openagents plugin purchase show --root ROOT --purchase {id}")
        }
    };
    body.push_str(&format!("<pre><code>{next}</code></pre>"));
    body.push_str(UNAVAILABLE);
    body
}

async fn purchases(State(app): State<App>, headers: HeaderMap) -> Response {
    match load(&app) {
        Ok(list) => UiPage::new("Plugin purchases")
            .path("/app/purchases")
            .scriptless()
            .content(prose(PreEscaped(render_list(&list))))
            .respond(&headers),
        Err(response) => response,
    }
}

async fn purchase(Path(id): Path<String>, State(app): State<App>, headers: HeaderMap) -> Response {
    match load(&app) {
        Ok(list) => match list.iter().find(|item| item.id == id) {
            Some(item) => UiPage::new("Plugin purchase")
                .path(format!(
                    "/app/purchases/{}",
                    crate::layout::segment(&item.id)
                ))
                .scriptless()
                .content(prose(PreEscaped(render_one(item))))
                .respond(&headers),
            None => problem(
                StatusCode::NOT_FOUND,
                "Purchase not found",
                "No purchase with this identifier belongs to the selected customer.",
                ("/app/purchases", "All purchases"),
            ),
        },
        Err(response) => response,
    }
}
