//! `/live`: the Episode 289 route-map picture driven by real payments
//! (#10197).
//!
//! The page is a canvas (`static/flow.js`, the site's second script) that
//! draws the route map from the pay host's `/flow/snapshot` topology and
//! animates each event from `/flow/stream` as the desktop deck's
//! `routes-live` scene does: a white request out to its node, a gold
//! payment back to the router, a gold share on to the author, a gold
//! payout to the author's wallet, and a ring for a bonus
//! (`docs/payments/2026-10-02-central-receive-and-splits.md`, section 7).
//! Both are same-origin under `/api/flow/`, which this server proxies to
//! the pay host (#10195). A totals ticker and the time of the last event
//! sit under the map. Nothing is synthetic: with no stream the page says
//! so and draws no dots.

use axum::Router;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::page;

/// Where the page reads the snapshot and the stream.
pub(crate) const SNAPSHOT: &str = "/api/flow/snapshot";
pub(crate) const STREAM: &str = "/api/flow/stream";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/live", get(live))
}

/// The page's policy: the site's, plus its one script and its reads of
/// the flow endpoints on this origin.
const LIVE_POLICY: &str = "default-src 'none'; style-src 'self'; img-src 'self'; \
script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; \
frame-ancestors 'none'";

fn body() -> String {
    format!(
        "<section class=\"live\" aria-labelledby=\"live-title\">\
<h1 id=\"live-title\">Live</h1>\
<p class=\"lede\">Payments, plugin calls, payouts, and Coder runs on OpenAgents as they \
happen, on the route map. Nothing here is simulated.</p>\
<div class=\"flow\" id=\"flow\" data-snapshot=\"{SNAPSHOT}\" data-stream=\"{STREAM}\">\
<canvas id=\"flow-map\" role=\"img\" aria-label=\"The route map with live traffic\"></canvas>\
</div>\
<p class=\"flow-status\" id=\"flow-status\" aria-live=\"polite\">Connecting to the flow \
stream.</p>\
<dl class=\"flow-totals\" id=\"flow-totals\">\
<div><dt>Received</dt><dd id=\"flow-received\">\u{2014}</dd></div>\
<div><dt>Paid out</dt><dd id=\"flow-paid\">\u{2014}</dd></div>\
<div><dt>Calls</dt><dd id=\"flow-calls\">\u{2014}</dd></div></dl>\
<p class=\"dim\">White dots are calls and runs going out from the router. Gold dots are \
payments coming back, shares going on to a plugin's author, and payouts to the author's \
wallet; a gold dot with a ring is a bonus. Payers show only as a daily alias.</p>\
<h2>Recent events</h2>\
<ol class=\"flow-recent\" id=\"flow-recent\"><li class=\"dim\">None yet.</li></ol>\
<p><a href=\"/stats\">[ Stats ]</a> <span class=\"dim\">The totals, plugins, authors, and payouts \
as tables.</span></p>\
<noscript><p class=\"dim\">Turn on JavaScript to see the live map, or read the numbers on <a href=\"/stats\">Stats</a>.</p></noscript>\
</section>\
<script src=\"/static/flow.js\" defer></script>"
    )
}

async fn live() -> Response {
    let mut response = page("Live", None, &body());
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(LIVE_POLICY),
    );
    response
}
