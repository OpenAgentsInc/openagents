//! `/desktop`: where to get OpenAgents for Mac.
//!
//! The `.dmg` is the one `scripts/desktop/package-macos.sh` builds (signed
//! with the OpenAgents Developer ID, notarized, stapled) and that the
//! release puts in the public bucket `openagentsgemini-oa-updates` under
//! `desktop/macos/VERSION/` (`docs/desktop/release.md`,
//! `scripts/desktop/sign-manifest.sh`). The installed app updates itself
//! from the signed manifest beside it.

use axum::Router;
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::{boxed, page};

/// The published desktop version this page links.
pub(crate) const MAC_VERSION: &str = "0.1.0";

/// The published `.dmg`, a universal build for Apple silicon and Intel.
pub(crate) const MAC_DMG: &str = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/0.1.0/OpenAgents-0.1.0.dmg";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/desktop", get(desktop))
}

async fn desktop() -> Response {
    let download = format!(
        "<p>OpenAgents for Mac runs on your Mac and connects your phone to it. \
It shows a QR code; scan it with the OpenAgents app on your iPhone, and your phone \
can chat with the agents on this computer and run Coder in your projects.</p>\
<p><a class=\"button\" href=\"{MAC_DMG}\">[ Download OpenAgents {MAC_VERSION} for Mac (.dmg) ]</a></p>\
<p class=\"hint\">macOS 13 or later. One universal build for Apple silicon and Intel, \
signed by OpenAgents, Inc. and notarized by Apple.</p>"
    );
    let install = "<ol><li>Open the downloaded <code>.dmg</code>.</li>\
<li>Drag <strong>OpenAgents</strong> onto <strong>Applications</strong>.</li>\
<li>Open OpenAgents from Applications. It shows a QR code.</li>\
<li>On your iPhone, open the OpenAgents app, tap <strong>Account</strong>, \
<strong>Computers</strong>, <strong>Connect a computer</strong>, and scan the code. \
The iPhone Camera can scan it too.</li></ol>\
<p class=\"hint\">To run Coder on the Mac, sign in to Codex or Claude Code there first.</p>";
    let phone = format!(
        "<p>The OpenAgents iPhone app is in testing: \
<a href=\"{}\">get OpenAgents on TestFlight</a>.</p>\
<p class=\"hint\">The Android app is in testing and not yet public. \
Builds of the desktop app for Linux and Windows are not published yet.</p>",
        super::TESTFLIGHT
    );
    let terminal = format!(
        "<p>Prefer the terminal? Coder Terminal installs with one command:</p>\
<code class=\"command\">{}</code><p><a href=\"/docs/install\">[ Install guide ]</a></p>",
        crate::layout::escape(super::UNIX_COMMAND)
    );
    let body = format!(
        "<h1>OpenAgents for Mac</h1>{}{}{}{}",
        boxed("download", &download),
        boxed("install", install),
        boxed("iphone", &phone),
        boxed("terminal", &terminal)
    );
    page("OpenAgents for Mac", Some("/desktop"), &body)
}
