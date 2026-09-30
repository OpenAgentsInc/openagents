//! The homepage: what OpenAgents is, one link to `/install`, and a
//! screenshot of the Verse.
//!
//! The image is the Grid, the OpenAgents Verse world, seen from above as
//! the desktop app's backdrop draws it, captured from the live relay with
//! `crates/verse/examples/overlook_capture.rs`
//! (`OVERLOOK_SIZE=1600x900 … /tmp/grid.png 45 wss://relay.openagents.com 8`).
//! It is served from `/static/verse-grid.jpg`.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::{escape, page};

/// What the screenshot shows, for a reader who does not see it.
const GRID_ALT: &str = "The Grid, the OpenAgents Verse world, seen from above: the Gym building, \
a ball, and scattered blocks on a floor of white lines.";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// `2500` as `$25`, `2550` as `$25.50`, nothing for zero or none.
fn credit(cents: Option<u64>) -> Option<String> {
    match cents? {
        0 => None,
        cents if cents % 100 == 0 => Some(format!("${}", cents / 100)),
        cents => Some(format!("${}.{:02}", cents / 100, cents % 100)),
    }
}

/// The intro: what OpenAgents is, the credit a new account starts with
/// when the service gives one, and the way in.
fn intro(credit: Option<&str>) -> String {
    let mut out = String::from(
        "<section class=\"intro\"><h1>OpenAgents</h1>\
<p class=\"lede\">Chat with OpenAgents on your phone and your computer. Its agents work on \
your own machines, and Coder is the one that writes code.</p>",
    );
    if let Some(credit) = credit {
        out.push_str(&format!(
            "<p>Every new account starts with {} of credit.</p>",
            escape(credit)
        ));
    }
    out.push_str(
        "<p><a class=\"button\" href=\"/install\">[ Install OpenAgents ]</a></p></section>",
    );
    out
}

/// The Verse screenshot.
fn grid() -> String {
    format!(
        "<figure class=\"shot\"><img src=\"/static/verse-grid.jpg\" width=\"1600\" \
height=\"900\" alt=\"{GRID_ALT}\"><figcaption>The Grid, the OpenAgents Verse world, seen \
from above.</figcaption></figure>"
    )
}

async fn home(State(app): State<App>) -> Response {
    let credit = credit(app.config.backend.new_account_credit_cents());
    page(
        "OpenAgents",
        None,
        &format!("{}{}", intro(credit.as_deref()), grid()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_credit_line_reads_whole_dollars_when_it_can() {
        assert_eq!(credit(Some(2500)).as_deref(), Some("$25"));
        assert_eq!(credit(Some(2550)).as_deref(), Some("$25.50"));
        assert_eq!(credit(Some(5)).as_deref(), Some("$0.05"));
        assert_eq!(credit(Some(0)), None);
        assert_eq!(credit(None), None);
        assert!(intro(Some("$25")).contains("Every new account starts with $25 of credit."));
        assert!(!intro(None).contains("credit"));
    }
}
