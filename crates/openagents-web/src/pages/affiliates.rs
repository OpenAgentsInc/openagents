//! `/affiliates` (#11698): the affiliate program's terms in plain words.
//!
//! The terms are the referral system's published text
//! ([`tenancy::accounts::referrals::affiliate::TERMS_POINTS`]), the same
//! words affiliates accept before they earn, so this page and what they
//! accept can't drift apart. The numbers come from
//! [`tenancy::accounts::referrals::affiliate::PROGRAM`].

use axum::Router;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, html};
use tenancy::accounts::referrals::affiliate::{PROGRAM, TERMS_POINTS, TERMS_VERSION};

use crate::App;
use crate::ui_page::{UiPage, prose};

/// The page's path.
pub(crate) const AFFILIATES: &str = "/affiliates";

pub(crate) fn routes() -> Router<App> {
    Router::new().route(AFFILIATES, get(affiliates_page))
}

async fn affiliates_page(headers: HeaderMap) -> Response {
    page(&headers)
}

/// The terms' date, from their version (`affiliate-YYYY-MM-DD`).
fn effective() -> &'static str {
    TERMS_VERSION
        .strip_prefix("affiliate-")
        .unwrap_or(TERMS_VERSION)
}

/// The page body.
pub(crate) fn body() -> Markup {
    html! {
        h1 { "Affiliates" }
        p {
            "Share OpenAgents and earn " (PROGRAM.rate_percent()) "% of what the people you send pay for Pro, for their first "
            (PROGRAM.paid_months) " paid months, paid in bitcoin over Lightning."
        }
        p {
            "To get your link, "
            a href=(crate::pages::contact::CONTACT) { "write to us" }
            " and we'll set it up. Your link starts with openagents.com/r/."
        }
        h2 { "The terms" }
        @for point in TERMS_POINTS {
            p { (point) }
        }
        p.oa-page-meta { "These terms take effect on " (effective()) "." }
    }
}

/// The page for `headers`.
pub(crate) fn page(headers: &HeaderMap) -> Response {
    UiPage::new("Affiliates")
        .path(AFFILIATES)
        .scriptless()
        .content(prose(body()))
        .respond(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_affiliates_page_is_plain_and_states_the_terms() {
        let html = body().into_string();
        crate::copy_guard::assert_plain(AFFILIATES, &html);
        assert_eq!(PROGRAM.rate_percent(), 20);
        for text in [
            "20%",
            "first 12 paid months",
            "30 days",
            "10,000 sats",
            "Lightning",
            "#ad",
            "/go/",
            "/r/",
            "real person",
            "30 days&#39; notice",
        ] {
            assert!(
                html.contains(text) || html.contains(&text.replace("&#39;", "'")),
                "{text}"
            );
        }
        assert!(html.contains("href=\"/contact\""));
    }
}
