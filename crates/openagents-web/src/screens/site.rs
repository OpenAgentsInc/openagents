//! Site pages, docs, and problem pages, from the live page functions over
//! sample data. Nothing here reads the pay host, a store, or the network.

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::{Group, Screen, state};
use crate::pages::live::Moment;
use crate::pages::stats::{Snapshot, Stats};

/// A same-site address standing in for Google's return.
const GOOGLE_NEXT: &str = "/auth/google/finish?state=sample-state&code=sample-code";

pub(crate) fn screens() -> Vec<Screen> {
    vec![
        page(
            "download",
            "Download",
            &["/download"],
            vec![
                state("default", "As served", download),
                state("desktop", "With desktop downloads", download_desktop),
                state("get-app", "Get the OpenAgents app", get_app),
            ],
        ),
        page(
            "docs",
            "Docs",
            &["/docs", "/docs/{slug}", "/docs/api", "/docs/api/{slug}"],
            vec![
                state("index", "Docs index", docs_index),
                state("article", "One guide", docs_article),
                state("missing", "No guide by that name", docs_missing),
                state("api-index", "API docs index", api_index),
                state("api-guide", "One API guide", api_guide),
            ],
        ),
        page(
            "pricing",
            "Pricing",
            &["/pricing", "/docs/pricing"],
            vec![state("default", "As served", pricing)],
        ),
        page(
            "privacy",
            "Privacy",
            &["/privacy"],
            vec![state("default", "As served", privacy)],
        ),
        page(
            "terms",
            "Terms",
            &["/terms"],
            vec![state("default", "As served", terms)],
        ),
        page(
            "roadmap",
            "Roadmap",
            &["/roadmap"],
            vec![state("default", "As served", roadmap)],
        ),
        page(
            "promises",
            "What works today",
            &["/promises"],
            vec![state("default", "As served", promises)],
        ),
        page(
            "pilot",
            "Pilot request",
            &["/pilot"],
            vec![
                state("retired", "Form retired (as served)", pilot_retired),
                state("invalid", "Something in the form is wrong", pilot_invalid),
                state("received", "Pilot request received", pilot_received),
            ],
        ),
        page(
            "stats",
            "Stats",
            &["/stats"],
            vec![
                state("populated", "Populated", stats_populated),
                state("empty", "Nothing paid yet", stats_empty),
                state("no-data", "No data", stats_no_data),
            ],
        ),
        page(
            "live",
            "Live",
            &["/live"],
            vec![
                state("waiting", "Nothing live", live_waiting),
                state("one", "One live session", live_one),
            ],
        ),
        page(
            "efficiency",
            "Efficiency",
            &["/efficiency"],
            vec![state("default", "As served", efficiency)],
        ),
        page(
            "connect-google",
            "Connecting Google",
            &["/auth/google/callback"],
            vec![state("connecting", "Connecting Google", connecting_google)],
        ),
        page(
            "problem",
            "Problem pages",
            &["any address"],
            vec![
                state("not-found", "Not found", not_found),
                state("refused", "Refused", refused),
                state("gone", "Moved away", gone),
                state("broken", "Something went wrong", broken),
                state("unavailable", "Unavailable", unavailable),
                state("sign-in-first", "Sign in first", sign_in_first),
                state("rate-limited", "Too many tries", rate_limited),
            ],
        ),
    ]
}

fn page(
    id: &'static str,
    title: &'static str,
    routes: &'static [&'static str],
    states: Vec<super::State>,
) -> Screen {
    Screen {
        id,
        title,
        group: Group::Page,
        routes,
        states,
    }
}

fn download(headers: &HeaderMap) -> Response {
    crate::pages::download::download_page(headers, crate::pages::download::DESKTOP_RELEASED)
}

fn download_desktop(headers: &HeaderMap) -> Response {
    crate::pages::download::download_page(headers, true)
}

fn get_app(headers: &HeaderMap) -> Response {
    crate::pages::connect::connect_page(headers)
}

fn docs_index(headers: &HeaderMap) -> Response {
    crate::pages::content::docs_index_page(headers)
}

fn docs_article(headers: &HeaderMap) -> Response {
    crate::pages::content::doc_page(headers, "what-is-openagents")
}

fn docs_missing(headers: &HeaderMap) -> Response {
    crate::pages::content::doc_page(headers, "no-such-guide")
}

fn api_index(headers: &HeaderMap) -> Response {
    crate::pages::api_docs::index_page(headers)
}

fn api_guide(headers: &HeaderMap) -> Response {
    let (index, (_, source)) = crate::pages::api_docs::API_DOCS
        .iter()
        .enumerate()
        .find(|(_, (name, _))| *name == "quickstart")
        .expect("the quickstart guide");
    crate::pages::api_docs::guide_page(headers, index, source)
}

fn pricing(headers: &HeaderMap) -> Response {
    crate::pages::content::doc_page(headers, "pricing")
}

fn privacy(headers: &HeaderMap) -> Response {
    crate::pages::content::privacy_page(headers)
}

fn terms(headers: &HeaderMap) -> Response {
    crate::pages::content::terms_page(headers)
}

fn roadmap(headers: &HeaderMap) -> Response {
    crate::promises::roadmap_view(headers)
}

fn promises(headers: &HeaderMap) -> Response {
    crate::promises::promises_view(headers)
}

fn pilot_retired(headers: &HeaderMap) -> Response {
    crate::not_found_for(headers)
}

fn pilot_invalid(headers: &HeaderMap) -> Response {
    crate::pilot::failure_page(
        headers,
        StatusCode::BAD_REQUEST,
        "Add a valid email, fill in each field, and tick the box that says we may write back. \
Nothing was sent.",
    )
}

fn pilot_received(headers: &HeaderMap) -> Response {
    crate::pilot::received_page(headers, "SAMPLE-0001")
}

/// Two sample plugins by two sample authors, with one payout.
fn sample_stats() -> (Stats, Snapshot) {
    let stats = serde_json::json!({
        "totals": {
            "received_sats": 12_500, "paid_out_sats": 8_000,
            "pending_accruals_sats": 1_250, "calls": 340, "earnings_sats": 9_250
        },
        "per_plugin": {
            "sample-weather": {
                "received_sats": 10_000, "paid_out_sats": 6_500, "calls": 300,
                "earnings_sats": 7_500
            },
            "sample-translate": {
                "received_sats": 2_500, "paid_out_sats": 1_500, "calls": 40,
                "earnings_sats": 1_750
            }
        },
        "per_author": {
            "Sample Author A": { "earnings_sats": 7_500, "paid_out_sats": 6_500,
                "pending_accruals_sats": 1_000 },
            "Sample Author B": { "earnings_sats": 1_750, "paid_out_sats": 1_500,
                "pending_accruals_sats": 250 }
        },
        "series_24h": [
            { "at": 1_791_600_000, "totals": { "received_sats": 400, "calls": 12 } },
            { "at": 1_791_603_600, "totals": { "received_sats": 900, "calls": 25 } }
        ],
        "series_30d": [
            { "at": 1_789_000_000, "totals": { "received_sats": 5_000, "calls": 140 } },
            { "at": 1_789_086_400, "totals": { "received_sats": 7_500, "calls": 200 } }
        ],
        "reconciliation": "ok"
    });
    let snapshot = serde_json::json!({
        "events": [{
            "at": 1_791_603_000, "type": "payout", "plugin": "sample-weather",
            "author": "Sample Author A", "amount_sats": 6_500
        }]
    });
    (
        serde_json::from_value(stats).expect("sample stats"),
        serde_json::from_value(snapshot).expect("sample events"),
    )
}

fn stats_populated(headers: &HeaderMap) -> Response {
    let (stats, snapshot) = sample_stats();
    crate::pages::stats::stats_page(headers, Some(&stats), Some(&snapshot))
}

fn stats_empty(headers: &HeaderMap) -> Response {
    let stats: Stats = serde_json::from_value(serde_json::json!({})).expect("empty stats");
    crate::pages::stats::stats_page(headers, Some(&stats), Some(&Snapshot::default()))
}

fn stats_no_data(headers: &HeaderMap) -> Response {
    crate::pages::stats::stats_page(headers, None, None)
}

fn live_waiting(headers: &HeaderMap) -> Response {
    crate::pages::live::live_page(headers, &crate::pages::live::WAITING)
}

fn live_one(headers: &HeaderMap) -> Response {
    crate::pages::live::live_page(
        headers,
        &Moment {
            status: "Live.",
            received: "1,250 sats",
            paid: "900 sats",
            calls: "14",
            recent: &["A Coder run started on Christopher's MacBook Pro."],
        },
    )
}

fn efficiency(headers: &HeaderMap) -> Response {
    crate::pages::efficiency::page(headers)
}

fn connecting_google(headers: &HeaderMap) -> Response {
    crate::connections::connecting_page(headers, GOOGLE_NEXT)
}

fn not_found(headers: &HeaderMap) -> Response {
    crate::not_found_for(headers)
}

fn refused(headers: &HeaderMap) -> Response {
    crate::work_runs::problem_page(headers, StatusCode::FORBIDDEN, "Pick a workspace first.")
}

fn gone(headers: &HeaderMap) -> Response {
    crate::ask::retired_page(headers)
}

fn broken(headers: &HeaderMap) -> Response {
    crate::ui_page::problem(
        headers,
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong",
        "Try again in a moment.",
        ("/", "Home"),
    )
}

fn unavailable(headers: &HeaderMap) -> Response {
    crate::pilot::failure_page(
        headers,
        StatusCode::SERVICE_UNAVAILABLE,
        "We aren't taking requests right now. Nothing was sent.",
    )
}

fn sign_in_first(headers: &HeaderMap) -> Response {
    // `environments::sign_in_first` sends a visitor here.
    crate::auth::sign_in_page(headers, false, false, "/environments")
}

fn rate_limited(headers: &HeaderMap) -> Response {
    crate::work_runs::problem_page(
        headers,
        StatusCode::TOO_MANY_REQUESTS,
        "Too many runs are waiting. Try again when some finish.",
    )
}
