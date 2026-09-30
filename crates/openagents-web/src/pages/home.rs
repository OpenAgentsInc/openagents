//! The homepage and its ask box.
//!
//! `/` is a terminal on a black page: the welcome card, one link to
//! `/install`, and a composer. The composer is a plain form that sends the
//! line to `/ask`, so it works with no script: `/ask` draws the line you
//! typed and the answer under it, the way a terminal prints a command's
//! output, and puts the composer back under both.
//!
//! A command is answered here: `download`, `install`, `desktop`, `mac`, and
//! `iphone` print a short install summary that links `/install`, `help`
//! lists the commands, and `clear` returns to `/`. Any other line is a
//! question for OpenAgents, answered by the backend. A development server
//! has no chat backend, and says so.

use axum::Router;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use serde::Deserialize;

use crate::App;
use crate::layout::{escape, page};
use crate::markdown;

/// The longest line the ask box takes, in characters.
const MAX_LINE: usize = 2000;

/// What the composer's input tells a reader that does not see the terminal.
const LABEL: &str = "Type a command such as install or help, or ask a question";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home)).route("/ask", get(ask))
}

#[derive(Deserialize)]
struct Line {
    q: Option<String>,
}

/// `2500` as `$25`, `2550` as `$25.50`, nothing for zero or none.
fn credit(cents: Option<u64>) -> Option<String> {
    match cents? {
        0 => None,
        cents if cents % 100 == 0 => Some(format!("${}", cents / 100)),
        cents => Some(format!("${}.{:02}", cents / 100, cents % 100)),
    }
}

/// The welcome card: what OpenAgents and Coder are, the credit a new
/// account starts with when the service gives one, and what to type.
fn welcome(credit: Option<&str>) -> String {
    let mut out = String::from(
        "<section class=\"box\"><h2 class=\"box-title\">OpenAgents</h2>\
<p class=\"loud\">Welcome to OpenAgents.</p>\
<p>Chat with OpenAgents on your phone and your computer. Its agents work on your own \
machines, and Coder is the one that writes code.</p>",
    );
    if let Some(credit) = credit {
        out.push_str(&format!(
            "<p>Every new account starts with {} of credit.</p>",
            escape(credit)
        ));
    }
    out.push_str(
        "<p class=\"dim\">Type install to get OpenAgents on your Mac and iPhone, \
or ask a question.</p></section>",
    );
    out
}

/// The single way in to installing: a link to `/install`.
const INSTALL_LINK: &str = "<div class=\"line\"><p><a class=\"button\" href=\"/install\">[ Install OpenAgents ]</a></p></div>";

/// The install summary the ask box prints.
const INSTALL_SUMMARY: &str = "<p>Get OpenAgents for Mac and the OpenAgents iPhone app on TestFlight, then scan the \
Mac's QR code with your phone to connect them.</p>\
<p><a href=\"/install\">[ Install OpenAgents ]</a></p>";

fn composer(value: &str) -> String {
    format!(
        "<form class=\"composer\" method=\"get\" action=\"/ask\" role=\"search\">\
<span class=\"prompt\" aria-hidden=\"true\">&gt;</span>\
<input type=\"text\" name=\"q\" value=\"{}\" maxlength=\"{MAX_LINE}\" autocomplete=\"off\" \
autocapitalize=\"off\" spellcheck=\"false\" enterkeyhint=\"send\" autofocus \
placeholder=\"install, help, or a question\" aria-label=\"{LABEL}\">\
<button type=\"submit\">[ Send ]</button></form>",
        escape(value)
    )
}

fn terminal(inner: &str) -> String {
    format!(
        "<h1 class=\"unseen\">OpenAgents: chat with agents that work on your computers</h1>\
<div class=\"terminal\" id=\"terminal\">{inner}</div>"
    )
}

async fn home(State(app): State<App>) -> Response {
    let credit = credit(app.config.backend.new_account_credit_cents());
    let inner = format!(
        "{}{INSTALL_LINK}{}",
        welcome(credit.as_deref()),
        composer("")
    );
    page("OpenAgents", None, &terminal(&inner))
}

/// The help line.
const HELP: &str = "**install** shows how to get OpenAgents on your Mac and iPhone. **clear** \
clears this screen. Type a command, or ask a question.";

async fn ask(State(app): State<App>, Query(line): Query<Line>) -> Response {
    let text: String = line
        .q
        .unwrap_or_default()
        .trim()
        .chars()
        .take(MAX_LINE)
        .collect();
    if text.is_empty() {
        return Redirect::to("/").into_response();
    }
    let command = text.trim_start_matches('/').to_ascii_lowercase();
    let answer = match command.as_str() {
        "clear" => return Redirect::to("/").into_response(),
        "download" | "install" | "desktop" | "mac" | "iphone" => INSTALL_SUMMARY.to_owned(),
        "help" => markdown::render(HELP),
        _ => match app.config.backend.answer(&text).await {
            Some(answer) => markdown::render(&answer),
            None => markdown::render(
                "Chat with OpenAgents isn't connected on this server, so it can't answer \
                 questions here. Type **install** to get OpenAgents on your Mac and iPhone.",
            ),
        },
    };
    let inner = format!(
        "<p class=\"typed\">{}</p><div class=\"answer\">{answer}</div>{}\
<p class=\"hint\"><a href=\"/\">[ Start over ]</a></p>",
        escape(&text),
        composer("")
    );
    page("OpenAgents", None, &terminal(&inner))
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
        assert!(welcome(Some("$25")).contains("Every new account starts with $25 of credit."));
        assert!(!welcome(None).contains("credit"));
    }
}
