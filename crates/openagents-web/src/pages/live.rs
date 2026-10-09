//! `/live`: the Episode 289 route-map picture driven by real payments
//! (#10197).
//!
//! The page is a canvas (`static/flow.js`, the site's second script) that
//! draws the route map from the pay host's `/flow/snapshot` topology and
//! animates each event from `/flow/stream` as the desktop deck's
//! `routes-live` scene does: a neutral request out to its node, a warning-colored
//! payment back to the router, a warning-colored share on to the author, a warning-colored
//! payout to the author's wallet, and a ring for a bonus
//! (`docs/payments/2026-10-02-central-receive-and-splits.md`, section 7).
//! Both are same-origin under `/api/flow/`, which this server proxies to
//! the pay host (#10195). A totals ticker and the time of the last event
//! sit under the map. Nothing is synthetic: with no stream the page says
//! so and draws no dots.

use axum::Router;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, Render, html};
use openagents_ui::content::{Facts, MarkdownRoot, PageColumn};

use crate::App;
use crate::ui_page::{UiPage, action_link};

/// Where the page reads the snapshot and the stream.
pub(crate) const SNAPSHOT: &str = "/api/flow/snapshot";
pub(crate) const STREAM: &str = "/api/flow/stream";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/live", get(live))
}

/// The page's policy: the site's, plus its one script and its reads of
/// the flow endpoints on this origin.
const LIVE_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; \
frame-ancestors 'none'";

fn body() -> Markup {
    let totals = Facts::new()
        .id("flow-totals")
        .fact_with_id("Received", "\u{2014}", "flow-received")
        .fact_with_id("Paid out", "\u{2014}", "flow-paid")
        .fact_with_id("Calls", "\u{2014}", "flow-calls");
    PageColumn::new(html! {
        section aria-labelledby="live-title" {
            (MarkdownRoot::new(html! {
                h1 id="live-title" { "Live" }
                p.oa-page-lead {
                    "Payments, plugin calls, payouts, and Coder runs on OpenAgents as they \
happen, on the route map. Nothing here is simulated."
                }
            }))
            div.oa-canvas-panel id="flow" data-snapshot=(SNAPSHOT) data-stream=(STREAM) {
                canvas id="flow-map" role="img" aria-label="The route map with live traffic" {}
            }
            p.oa-page-meta id="flow-status" aria-live="polite" {
                "Connecting to the flow stream."
            }
            (totals)
            (MarkdownRoot::new(html! {
                p.oa-page-meta {
                    "Neutral dots are calls and runs going out from the router. Warning-colored \
dots are payments coming back, shares going on to a plugin's author, and payouts to the \
author's wallet; a warning-colored dot with a ring is a bonus. Payers show only as a daily alias."
                }
                h2 { "Recent events" }
                ol.oa-event-list id="flow-recent" { li { "None yet." } }
                noscript {
                    p.oa-page-meta {
                        "Turn on JavaScript to see the live map, or read the numbers on "
                        a href="/stats" { "Stats" } "."
                    }
                }
            }))
            div.oa-page-actions {
                (action_link("Stats", "/stats"))
                span.oa-page-meta { "The totals, plugins, authors, and payouts as tables." }
            }
        }
        script src="/static/flow.js" defer {}
    })
    .render()
}

async fn live(headers: HeaderMap) -> Response {
    let mut response = UiPage::new("Live")
        .path("/live")
        .scriptless()
        .content(body())
        .respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(LIVE_POLICY),
    );
    response
}
