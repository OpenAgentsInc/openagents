//! `/install`: the one page that installs everything OpenAgents is
//! launching, in order: OpenAgents for Mac, the iPhone app, pairing the
//! two, and, to let the phone run Coder, signing in to Codex or Claude Code
//! on the Mac. `/desktop` redirects here.
//!
//! The `.dmg` is the one `scripts/desktop/package-macos.sh` builds (signed
//! with the OpenAgents Developer ID, notarized, stapled) and that the
//! release puts in the public bucket `openagentsgemini-oa-updates` under
//! `desktop/macos/VERSION/` (`docs/desktop/release.md`,
//! `scripts/desktop/sign-manifest.sh`). The installed app updates itself
//! from the signed manifest beside it, and it bundles this repository's
//! `coder` and `microcoder`.

use axum::Router;
use axum::response::{Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::layout::{boxed, page};

/// The published desktop version this page links.
pub(crate) const MAC_VERSION: &str = "1.0.0";

/// The published `.dmg`, a universal build for Apple silicon and Intel.
pub(crate) const MAC_DMG: &str = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/install", get(install)).route(
        "/desktop",
        get(|| async { Redirect::permanent("/install") }),
    )
}

async fn install() -> Response {
    let mac = format!(
        "<p>OpenAgents for Mac runs on your Mac and connects your phone to it.</p>\
<p><a class=\"button\" href=\"{MAC_DMG}\">[ Download OpenAgents {MAC_VERSION} for Mac (.dmg) ]</a></p>\
<p class=\"hint\">macOS 13 or later. One universal build for Apple silicon and Intel, \
signed by OpenAgents, Inc. and notarized by Apple.</p>\
<ol><li>Open the downloaded <code>.dmg</code>.</li>\
<li>Drag <strong>OpenAgents</strong> onto <strong>Applications</strong>.</li></ol>"
    );
    let iphone = format!(
        "<p>The OpenAgents iPhone app is in testing on TestFlight.</p>\
<p><a class=\"button\" href=\"{}\">[ Get OpenAgents on TestFlight ]</a></p>",
        super::TESTFLIGHT
    );
    let connect = "<ol><li>Open OpenAgents from Applications on your Mac. It shows a QR code.</li>\
<li>Scan the code with the iPhone Camera, or in the OpenAgents app: tap \
<strong>Account</strong>, <strong>Computers</strong>, <strong>Connect a computer</strong>, \
and point it at the code.</li></ol>\
<p>Your phone can now chat with the agents on your Mac.</p>";
    let coder = "<p>To let your phone run Coder in your projects on the Mac, sign in \
to Codex or Claude Code at the terminal on the Mac first.</p>";
    let later = "<p class=\"hint\">Not yet: the Android app is in testing and not public, and \
desktop builds for Linux and Windows aren't published.</p>";
    let body = format!(
        "<h1>Install OpenAgents</h1>{}{}{}{}{later}",
        boxed("1. Get OpenAgents for Mac", &mac),
        boxed("2. Get the iPhone app", &iphone),
        boxed("3. Connect them", connect),
        boxed("4. Run Coder from your phone (optional)", coder),
    );
    page("Install OpenAgents", Some("/install"), &body)
}
