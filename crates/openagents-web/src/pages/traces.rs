//! The traces viewer: `/traces` lists the traces people uploaded, and
//! `/trace/{key}` shows one by its digest or id.
//!
//! A listing shows every stored trace's digest, receipt, domain, and
//! visibility. A trace's steps show only when its visibility is `glass`;
//! `ledger` and `pulse` traces read for their uploader and administrators,
//! which needs a signed-in session this site does not have yet, so the page
//! says the content isn't public.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::backend::{NOT_CONNECTED, TraceListing};
use crate::layout::{escape, page, problem, segment};

/// The most steps one trace page draws.
const MAX_STEPS: usize = 500;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/traces", get(traces))
        .route("/trace/{key}", get(trace))
}

const LEAD: &str = "<h1>Traces</h1><p>Agent sessions people chose to share, in the Agent \
Trajectory Interchange Format (ATIF). Each is stored once per digest, redacted and screened \
for secrets before it is kept.</p>";

fn meta(listing: &TraceListing) -> String {
    format!(
        "{} \u{b7} {} \u{b7} {} \u{b7} {} bytes{}",
        escape(&listing.visibility),
        escape(&listing.domain),
        escape(&listing.received_at),
        listing.size_bytes,
        if listing.truncated {
            " \u{b7} truncated"
        } else {
            ""
        }
    )
}

async fn traces(State(app): State<App>) -> Response {
    let backend = &app.config.backend;
    if !backend.connected() {
        return page(
            "Traces",
            Some("/traces"),
            &format!("{LEAD}<p class=\"notice\">{}</p>", escape(NOT_CONNECTED)),
        );
    }
    let listings = backend.traces().await;
    let mut body = String::from(LEAD);
    if listings.is_empty() {
        body.push_str("<p class=\"dim\">No traces have been shared yet.</p>");
    } else {
        body.push_str("<ul class=\"list\">");
        for listing in &listings {
            body.push_str(&format!(
                "<li><a class=\"title\" href=\"/trace/{}\"><code>{}</code></a><p class=\"meta\">{}</p><p class=\"hint\">Receipt <code>{}</code></p></li>",
                segment(&listing.digest),
                escape(&listing.digest),
                meta(listing),
                escape(&listing.receipt)
            ));
        }
        body.push_str("</ul>");
    }
    page("Traces", Some("/traces"), &body)
}

async fn trace(State(app): State<App>, Path(key): Path<String>) -> Response {
    let backend = &app.config.backend;
    if !backend.connected() {
        return page(
            "Trace",
            Some("/traces"),
            &format!(
                "<p class=\"crumbs\"><a href=\"/traces\">traces</a> / <code>{}</code></p><h1>Trace</h1><p class=\"notice\">{}</p>",
                escape(&key),
                escape(NOT_CONNECTED)
            ),
        );
    }
    let Some(record) = backend.trace(&key).await else {
        return problem(
            StatusCode::NOT_FOUND,
            "Trace not found",
            "No stored trace has that digest or id.",
            ("/traces", "All traces"),
        );
    };
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"/traces\">traces</a> / trace</p><h1><code>{}</code></h1><p class=\"meta\">{}</p><p class=\"hint\">Receipt <code>{}</code></p>",
        escape(&record.listing.digest),
        meta(&record.listing),
        escape(&record.listing.receipt)
    );
    match (&record.document, record.listing.visibility.as_str()) {
        (Some(document), "glass") => body.push_str(&steps(document)),
        _ => body.push_str(
            "<p class=\"dim\">This trace's content is not public. Its uploader chose a \
             visibility that lists it by digest and receipt only.</p>",
        ),
    }
    page("Trace", Some("/traces"), &body)
}

/// An ATIF document's agent and steps.
fn steps(document: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(agent) = document.get("agent") {
        out.push_str(&format!(
            "<p>Agent: <code>{}</code> {}</p>",
            escape(agent["name"].as_str().unwrap_or("unknown")),
            escape(agent["version"].as_str().unwrap_or(""))
        ));
    }
    let empty = Vec::new();
    let steps = document["steps"].as_array().unwrap_or(&empty);
    out.push_str(&format!(
        "<h2>Steps ({})</h2><ol class=\"steps\">",
        steps.len()
    ));
    for step in steps.iter().take(MAX_STEPS) {
        let message = match &step["message"] {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Null => String::new(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };
        out.push_str(&format!(
            "<li><span class=\"at\">{} / {}</span><pre>{}</pre></li>",
            step["step_id"].as_u64().unwrap_or(0),
            escape(step["source"].as_str().unwrap_or("event")),
            escape(&message)
        ));
    }
    out.push_str("</ol>");
    if steps.len() > MAX_STEPS {
        out.push_str(&format!(
            "<p class=\"hint\">The first {MAX_STEPS} steps are shown.</p>"
        ));
    }
    out
}
