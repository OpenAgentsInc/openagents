//! `/install`: the one download page. It links only the release
//! candidates published on 2026-10-01 (#10126): OpenAgents for Mac
//! 1.0.0-rc.2, the notarized `.dmg` under `desktop/macos/rc/` in the public
//! bucket `openagentsgemini-oa-updates` (`docs/desktop/release.md`), and
//! OpenAgents Terminal 1.0.0-rc.2, installed by the scripts in
//! `openagentsgemini-cli-releases/openagents/` (`docs/release/terminal.md`).
//! Every other app and platform is built from source, and the page says so
//! with one link to the repository. `/desktop` redirects here.

use axum::Router;
use axum::response::{Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::layout::page;

/// The published desktop version this page links.
pub(crate) const MAC_VERSION: &str = "1.0.0-rc.2";

/// The published `.dmg`, a universal build for Apple silicon and Intel.
pub(crate) const MAC_DMG: &str = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/rc/1.0.0-rc.2/OpenAgents-1.0.0-rc.2.dmg";

/// The published OpenAgents Terminal version the install commands fetch.
pub(crate) const TERMINAL_VERSION: &str = "1.0.0-rc.2";

/// OpenAgents Terminal's install command on macOS and Linux.
pub(crate) const TERMINAL_SH: &str = "curl -fsSL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh | sh";

/// OpenAgents Terminal's install command on Windows, in PowerShell.
pub(crate) const TERMINAL_PS1: &str =
    "irm https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.ps1 | iex";

/// Where everything else is built from.
pub(crate) const SOURCE: &str = "https://github.com/OpenAgentsInc/openagents";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/install", get(install)).route(
        "/desktop",
        get(|| async { Redirect::permanent("/install") }),
    )
}

/// A numbered section: `[1] Title`, a line under it, and its body.
fn section(number: u8, title: &str, lead: &str, body: &str) -> String {
    format!(
        "<section class=\"dl-section\"><h2><span class=\"dim\">[{number}]</span> {title}</h2>\
<p class=\"label\">{lead}</p>{body}</section>"
    )
}

async fn install() -> Response {
    let mac = format!(
        "<ul class=\"dl-list\"><li class=\"dl-row\"><span class=\"dl-name\"><strong>macOS</strong> \
<span class=\"dim\">(Apple silicon and Intel)</span></span>\
<a class=\"button\" href=\"{MAC_DMG}\">[ Download .dmg ]</a></li></ul>\
<p class=\"hint\">macOS 13 or later. Open the <code>.dmg</code> and drag \
<strong>OpenAgents</strong> onto <strong>Applications</strong>.</p>"
    );
    let terminal = format!(
        "<p class=\"hint\">macOS and Linux:</p><pre><code>{TERMINAL_SH}</code></pre>\
<p class=\"hint\">Windows, in PowerShell:</p><pre><code>{TERMINAL_PS1}</code></pre>"
    );
    let other = format!(
        "<p>iPhone, Android, and OpenAgents for Linux and Windows: \
<a href=\"{SOURCE}\">build from source</a>.</p>"
    );
    let body = format!(
        "<h1>Download OpenAgents</h1>{}{}{}",
        section(
            1,
            "OpenAgents for Mac",
            &format!("Version {MAC_VERSION}."),
            &mac
        ),
        section(
            2,
            "OpenAgents Terminal",
            &format!("Version {TERMINAL_VERSION}."),
            &terminal
        ),
        section(3, "Everything else", "", &other),
    );
    page("Download OpenAgents", Some("/install"), &body)
}
