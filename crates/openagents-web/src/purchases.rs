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

/// Millisatoshis as sats: `21 sats`, `0.012 sats`, `1 sat`.
fn msat(value: u64) -> String {
    let whole = value / 1000;
    let part = value % 1000;
    if part == 0 {
        if whole == 1 {
            "1 sat".to_owned()
        } else {
            format!("{whole} sats")
        }
    } else {
        let fraction = format!("{part:03}");
        format!("{whole}.{} sats", fraction.trim_end_matches('0'))
    }
}

/// `phase_text` with a capital first letter, for the start of a line.
fn phase_title(phase: Phase) -> String {
    let text = phase_text(phase);
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn option(value: Option<&str>) -> String {
    value.map_or_else(|| "unknown".to_string(), escape)
}

pub(crate) fn render_list(list: &[Summary]) -> String {
    let mut body = String::from(
        "<h1>Plugin purchases</h1>\
<p class=\"oa-page-meta\">Plugins you bought from this computer. \
<a href=\"/app/purchases\">Refresh</a></p><ul class=\"oa-item-list\">",
    );
    if list.is_empty() {
        body.push_str("<li><p class=\"oa-item-title\">No purchases</p><p>Plugins you buy in the app show up here.</p></li>");
    }
    for item in list {
        body.push_str(&format!(
            "<li><p><a class=\"oa-item-title\" href=\"/app/purchases/{id}\">{plugin}</a></p><p class=\"oa-page-meta\">{phase} \u{b7} {price}</p></li>",
            id = escape(&item.id),
            plugin = option(item.plugin.as_deref()),
            phase = phase_title(item.phase),
            price = msat(item.price_msat),
        ));
    }
    body.push_str("</ul>");
    body.push_str(UNAVAILABLE);
    body
}

pub(crate) fn render_one(item: &Summary) -> String {
    let id = escape(&item.id);
    let expires = i64::try_from(item.expires_at_ms).ok().map_or_else(
        || "Not known".to_owned(),
        |ms| format!("{} UTC", crate::pages::stats::utc(ms)),
    );
    let mut body = format!(
        "<p class=\"oa-page-meta\"><a href=\"/app/purchases\">All purchases</a> / {id}</p><h1>{plugin}</h1>\
<p><a href=\"/app/purchases/{id}\">Refresh</a></p>\
<dl class=\"oa-facts\">\
<div><dt>Status</dt><dd>{phase}</dd></div>\
<div><dt>Version</dt><dd>{release}</dd></div>\
<div><dt>Price</dt><dd>{price}, fee at most {fee}</dd></div>\
<div><dt>Price good until</dt><dd>{expires}</dd></div>",
        plugin = option(item.plugin.as_deref()),
        phase = phase_title(item.phase),
        release = item
            .release
            .as_deref()
            .map_or_else(|| "Not known".to_owned(), escape),
        price = msat(item.price_msat),
        fee = msat(item.max_fee_msat),
    );
    match &item.charge {
        Some(charge) => body.push_str(&format!(
            "<div><dt>Paid</dt><dd>{}, plus {} fee</dd></div>",
            msat(charge.amount_msat),
            msat(charge.fee_msat),
        )),
        None => body.push_str("<div><dt>Paid</dt><dd>Nothing yet</dd></div>"),
    }
    body.push_str(&format!(
        "<div><dt>Settled</dt><dd>{}</dd></div>",
        match item.settled {
            Some(true) => "Yes",
            Some(false) => "No, it failed",
            None => "Not yet",
        }
    ));
    body.push_str(&format!(
        "<div><dt>Delivered</dt><dd>{}</dd></div><div><dt>Result</dt><dd>{}</dd></div>",
        match item.delivery_status {
            Some(status) if (200..300).contains(&status) => "Yes".to_owned(),
            Some(status) => format!("No, the plugin answered with an error ({status})"),
            None => "Not yet".to_owned(),
        },
        if item.result_present {
            "Saved; see it with <code>openagents plugin purchase show</code>"
        } else {
            "None yet"
        }
    ));
    if let Some(maximum) = item.unresolved_maximum_msat {
        body.push_str(&format!(
            "<div><dt>May still be charged</dt><dd>Up to {}</dd></div>",
            msat(maximum)
        ));
    }
    if item.recovery_present {
        body.push_str("<div><dt>Checked again</dt><dd>Yes</dd></div>");
    }
    body.push_str("</dl>");
    let mut details = format!(
        "Plugin address: {}\nAccount: {}\nWorkspace: {}\nPaid from: {} on {}\nQuote: {}\nApproval: {}",
        escape(&item.url),
        escape(&item.account),
        escape(&item.workspace),
        escape(&item.payer_node),
        escape(&item.payer_network),
        escape(&item.quote_digest),
        escape(&item.approval_digest),
    );
    if let Some(charge) = &item.charge {
        details.push_str(&format!("\nPayment: {}", escape(&charge.payment_hash)));
    }
    if let Some(transaction) = item.transaction.as_deref() {
        details.push_str(&format!("\nTransaction: {}", escape(transaction)));
    }
    body.push_str(&format!(
        "<details><summary>Details for support</summary><pre>{details}</pre></details>"
    ));
    body.push_str("<h2>Next step</h2><p>Run this on your computer:</p>");
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

pub(crate) fn list_page(headers: &HeaderMap, list: &[Summary]) -> Response {
    UiPage::new("Plugin purchases")
        .path("/app/purchases")
        .scriptless()
        .content(prose(PreEscaped(render_list(list))))
        .respond(headers)
}

pub(crate) fn one_page(headers: &HeaderMap, item: &Summary) -> Response {
    UiPage::new("Plugin purchase")
        .path(format!(
            "/app/purchases/{}",
            crate::layout::segment(&item.id)
        ))
        .scriptless()
        .content(prose(PreEscaped(render_one(item))))
        .respond(headers)
}

async fn purchases(State(app): State<App>, headers: HeaderMap) -> Response {
    match load(&app) {
        Ok(list) => list_page(&headers, &list),
        Err(response) => response,
    }
}

async fn purchase(Path(id): Path<String>, State(app): State<App>, headers: HeaderMap) -> Response {
    match load(&app) {
        Ok(list) => match list.iter().find(|item| item.id == id) {
            Some(item) => one_page(&headers, item),
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
