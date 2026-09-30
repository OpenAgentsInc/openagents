//! The homepage and its ask box.
//!
//! `/` is a terminal on a black page: the welcome card, the install step,
//! the desktop app, and a composer. The composer is a plain form that
//! sends the line to `/ask`, so it works with no script: `/ask` draws the
//! line you typed and the answer under it, the way a terminal prints a
//! command's output, and puts the composer back under both.
//!
//! A command is answered here: `download` (or `install`) prints the
//! install step, `desktop` the desktop app, `blog` and `docs` link their
//! sections, `help` lists the commands, and `clear` returns to `/`. Any
//! other line is a question for OpenAgents, answered by the backend. A
//! development server has no chat backend, and says so.

use axum::Router;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use serde::Deserialize;

use crate::App;
use crate::layout::{escape, page};
use crate::markdown;

/// The install command on macOS and Linux.
pub(crate) const UNIX_COMMAND: &str =
    "curl -fsSL https://openagents.com/releases/install-terminal.sh | sh";

/// The install command on Windows, in PowerShell.
pub(crate) const WINDOWS_COMMAND: &str =
    "irm https://openagents.com/releases/install-terminal.ps1 | iex";

/// The longest line the ask box takes, in characters.
const MAX_LINE: usize = 2000;

/// What the composer's input tells a reader that does not see the terminal.
const LABEL: &str = "Type a command such as download, blog, or help, or ask a question";

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
machines, and Coder is the one that writes code.</p>\
<p>Coder Terminal is a coding agent that runs in your terminal. It works in your repository \
on your own computer: it reads the code, runs commands, and edits files, and it shows you \
each step.</p>\
<p>In Coder Terminal you can resume a session, add plugins, and hand work to other agents.</p>",
    );
    if let Some(credit) = credit {
        out.push_str(&format!(
            "<p>Every new account starts with {} of credit.</p>",
            escape(credit)
        ));
    }
    out.push_str(
        "<p class=\"dim\">Type download to install Coder Terminal, type blog to read the blog, \
or ask a question.</p></section>",
    );
    out
}

/// The install step, for both platforms.
fn install() -> String {
    format!(
        "<div class=\"line\"><p class=\"label\">Install Coder Terminal on macOS and Linux</p>\
<code class=\"command\">{}</code>\
<p class=\"label\">On Windows, in PowerShell</p><code class=\"command\">{}</code>\
<p class=\"hint\"><a href=\"/docs/install\">[ Install guide ]</a></p></div>",
        escape(UNIX_COMMAND),
        escape(WINDOWS_COMMAND)
    )
}

/// The desktop step.
fn desktop() -> String {
    format!(
        "<div class=\"line\"><p class=\"label\">OpenAgents for Mac</p>\
<p>Pairs your phone with your computer by QR code. \
<a href=\"/desktop\">[ Get OpenAgents for Mac ]</a> \
<a href=\"{}\">[ Download the .dmg ]</a></p></div>",
        super::MAC_DMG
    )
}

fn composer(value: &str) -> String {
    format!(
        "<form class=\"composer\" method=\"get\" action=\"/ask\" role=\"search\">\
<span class=\"prompt\" aria-hidden=\"true\">&gt;</span>\
<input type=\"text\" name=\"q\" value=\"{}\" maxlength=\"{MAX_LINE}\" autocomplete=\"off\" \
autocapitalize=\"off\" spellcheck=\"false\" enterkeyhint=\"send\" autofocus \
placeholder=\"download, blog, help, or a question\" aria-label=\"{LABEL}\">\
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
        "{}{}{}{}",
        welcome(credit.as_deref()),
        install(),
        desktop(),
        composer("")
    );
    page("OpenAgents", None, &terminal(&inner))
}

/// The help line.
const HELP: &str = "**download** installs Coder Terminal. **desktop** gets OpenAgents for Mac. \
**blog** opens the blog, **docs** the documentation. **clear** clears this screen. Type a \
command, or ask a question.";

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
        "download" | "install" => install(),
        "desktop" | "mac" => desktop(),
        "blog" => "<p><a href=\"/blog\">[ Open the blog ]</a></p>".to_owned(),
        "docs" => "<p><a href=\"/docs\">[ Open the docs ]</a></p>".to_owned(),
        "help" => markdown::render(HELP, "/docs"),
        _ => match app.config.backend.answer(&text).await {
            Some(answer) => markdown::render(&answer, "/docs"),
            None => markdown::render(
                "Chat with OpenAgents isn't connected on this server, so it can't answer \
                 questions here. Type **download** to run Coder on your own computer, or \
                 **desktop** to get OpenAgents for Mac.",
                "/docs",
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
