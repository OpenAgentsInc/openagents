//! `/install`: the one download page, laid out like opencode.ai/download:
//! numbered sections for the desktop apps (macOS, Windows, Linux) and the
//! mobile apps (iPhone, Android), each a row with its button, then how to
//! connect them and a short FAQ. A platform with nothing published says
//! "Coming soon" and links nowhere. `/desktop` redirects here.
//!
//! The `.dmg` is the one `scripts/desktop/package-macos.sh` builds (signed
//! with the OpenAgents Developer ID, notarized, stapled) and that the
//! release puts in the public bucket `openagentsgemini-oa-updates` under
//! `desktop/macos/VERSION/` (`docs/desktop/release.md`,
//! `scripts/desktop/sign-manifest.sh`). The installed app updates itself
//! from the signed manifest beside it, and it bundles this repository's
//! `coder` and `microcoder`. The Linux AppImage and `.deb` are the ones
//! `scripts/desktop/build-linux-release.sh` builds and
//! `scripts/desktop/sign-manifest-linux.sh` signs and publishes under
//! `desktop/linux/VERSION/`, beside `SHA256SUMS` and its signature.

use axum::Router;
use axum::response::{Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::layout::page;

/// The published desktop version this page links.
pub(crate) const MAC_VERSION: &str = "1.0.0";

/// The published `.dmg`, a universal build for Apple silicon and Intel.
pub(crate) const MAC_DMG: &str = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg";

/// The published Linux AppImage, x86_64, at the same version.
pub(crate) const LINUX_APPIMAGE: &str = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/OpenAgents-1.0.0-x86_64.AppImage";

/// The published Linux `.deb`, amd64, at the same version.
pub(crate) const LINUX_DEB: &str = "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/openagents_1.0.0_amd64.deb";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/install", get(install)).route(
        "/desktop",
        get(|| async { Redirect::permanent("/install") }),
    )
}

/// One download row: the platform, what it is, and its button, or
/// "Coming soon" when nothing is published for it yet.
fn row(platform: &str, detail: &str, link: Option<(&str, &str)>) -> String {
    let action = match link {
        Some((href, label)) => format!("<a class=\"button\" href=\"{href}\">[ {label} ]</a>"),
        None => "<span class=\"soon\">Coming soon</span>".to_owned(),
    };
    format!(
        "<li class=\"dl-row\"><span class=\"dl-name\"><strong>{platform}</strong> \
<span class=\"dim\">({detail})</span></span>{action}</li>"
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
    let desktop = format!(
        "<ul class=\"dl-list\">{}{}{}{}</ul>\
<p class=\"hint\">OpenAgents {MAC_VERSION} for Mac is one universal build, macOS 13 or later, \
signed by OpenAgents, Inc. and notarized by Apple. Open the <code>.dmg</code> and drag \
<strong>OpenAgents</strong> onto <strong>Applications</strong>. It updates itself.</p>\
<p class=\"hint\">On Linux, x86_64 with glibc 2.31 or later (Debian 11, Ubuntu 20.04, and newer): \
make the AppImage executable and open it, and it updates itself; or install the <code>.deb</code> \
with your package manager.</p>",
        row(
            "macOS",
            "Apple silicon and Intel",
            Some((MAC_DMG, "Download .dmg"))
        ),
        row("Windows", "x64, .msi", None),
        row("Linux", "x86_64, .deb", Some((LINUX_DEB, "Download .deb"))),
        row(
            "Linux",
            "x86_64, AppImage",
            Some((LINUX_APPIMAGE, "Download AppImage"))
        ),
    );
    let mobile = format!(
        "<ul class=\"dl-list\">{}{}</ul>",
        row(
            "iPhone",
            "TestFlight beta",
            Some((super::TESTFLIGHT, "Join on TestFlight"))
        ),
        row("Android", "in testing", None),
    );
    let connect = "<ol><li>Open OpenAgents on your computer. It shows a QR code.</li>\
<li>Scan it with the iPhone Camera, or in the app tap <strong>Account</strong>, \
<strong>Computers</strong>, <strong>Connect a computer</strong>.</li>\
<li>To let your phone run Coder there, sign in to Codex or Claude Code on the computer.</li></ol>\
<p class=\"hint\">More in <a href=\"/docs/connect-a-computer\">Connect a computer</a> and \
<a href=\"/docs/coder\">Coder</a>.</p>";
    let faq = "<dl class=\"faq\">\
<dt>Do I need Tailscale?</dt><dd>No. Your phone connects to your computer by scanning its QR \
code, directly when it can and through our relay when it can't.</dd>\
<dt>Do I need an account or a model key?</dt><dd>No. The app makes its own key the first time \
it opens. Coder uses the Codex or Claude Code sign-in already on your computer.</dd>\
<dt>Does my code leave my computer?</dt><dd>Coder works in your projects on your computer. \
Like Codex and Claude Code on their own, it sends what it reads to the model provider you \
signed in to. See <a href=\"/docs/privacy-and-security\">Privacy and security</a>.</dd>\
</dl>";
    let body = format!(
        "<h1>Download OpenAgents</h1>\
<p class=\"lede\">Chat with OpenAgents on your phone and your computer, and let Coder work \
in your projects.</p>{}{}{}{}",
        section(
            1,
            "OpenAgents Desktop",
            "Runs on your computer and connects your phone to it.",
            &desktop
        ),
        section(
            2,
            "OpenAgents Mobile",
            "Chat with OpenAgents and send work to your computers.",
            &mobile
        ),
        section(3, "Connect them", "Once, with a QR code.", connect),
        section(4, "FAQ", "", faq),
    );
    page("Download OpenAgents", Some("/install"), &body)
}
