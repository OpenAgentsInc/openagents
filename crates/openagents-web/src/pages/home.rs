//! The homepage: what OpenAgents is, one link to `/download`, and a terminal
//! to ask OpenAgents about itself. The Grid's screenshot is on
//! `/docs/the-grid`.
//!
//! The terminal (#10106) is the one script on the site, `static/ask.js`.
//! `help`, `download` (or `install`), `docs`, and `clear` are its commands,
//! matched whole;
//! any other line is a question for [`crate::ask`], which answers it from
//! the same OpenAgents chat the apps use, as the website: about OpenAgents
//! only, never Coder or a computer. Without the script the box says to
//! turn scripts on and the download link still works.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::{escape, page};

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
        "<p><a class=\"button\" href=\"/download\">[ Download OpenAgents ]</a></p></section>",
    );
    out
}

/// The homepage's policy: the site's, plus its one script and its
/// questions to `/ask`.
const HOME_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; \
frame-ancestors 'none'";

/// The terminal: a welcome, the screen the answers land on, and the line
/// a visitor types into.
fn terminal() -> &'static str {
    "<section class=\"box term\" aria-labelledby=\"term-title\">\
<h2 class=\"box-title\" id=\"term-title\">Ask OpenAgents</h2>\
<div class=\"term-screen\" id=\"term-screen\" role=\"log\" aria-live=\"polite\">\
<p class=\"term-welcome\">Ask us anything about OpenAgents: the apps, Coder, plugins, \
pricing, or privacy. Coder works on your own computer through the OpenAgents app for Mac. \
Type <code>help</code> for commands.</p>\
<noscript><p class=\"dim\">Turn on JavaScript to ask a question here, or \
<a href=\"/download\">download OpenAgents</a>.</p></noscript></div>\
<form class=\"term-line\" id=\"term-form\" action=\"/download\" method=\"get\">\
<label class=\"term-prompt\" for=\"term-input\">&gt;</label>\
<input id=\"term-input\" name=\"q\" type=\"text\" autocomplete=\"off\" \
spellcheck=\"false\" maxlength=\"4000\" placeholder=\"Ask about OpenAgents\" \
aria-label=\"Ask OpenAgents\"></form></section>\
<script src=\"/static/ask.js\" defer></script>"
}

async fn home(State(app): State<App>) -> Response {
    let credit = credit(app.config.backend.new_account_credit_cents());
    let mut response = page(
        "OpenAgents",
        None,
        &format!("{}{}", intro(credit.as_deref()), terminal()),
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(HOME_POLICY),
    );
    response
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
