//! `/download`: Coder's published release candidate and its installers.
//! Coder's terminal ships with the OpenAgents CLI and Microcoder.
//! `/install` and `/desktop` redirect here permanently (`308`).

use axum::Router;
use axum::http::header;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::layout::page;

/// The Coder terminal and companion OpenAgents CLI test release.
pub(crate) const CODER_VERSION: &str = "1.0.0-rc.5";
pub(crate) const CODER_BASE: &str =
    "https://storage.googleapis.com/openagentsgemini-cli-releases/coder";
pub(crate) const CODER_SH: &str = "curl -fsSL https://openagents.com/cli/install.sh | bash";
pub(crate) const CODER_PS1: &str = "irm https://openagents.com/cli/install.ps1 | iex";

/// Artifact suffixes shared by the commands in each platform build.
pub(crate) const CODER_PLATFORMS: [(&str, &str); 7] = [
    ("macOS · Apple silicon", "macos-aarch64"),
    ("macOS · Intel", "macos-x86_64"),
    ("Linux · x86_64", "linux-x86_64"),
    ("Linux · x86_64 · musl", "linux-x86_64-musl"),
    ("Linux · ARM64", "linux-aarch64"),
    ("Linux · ARM64 · musl", "linux-aarch64-musl"),
    ("Windows · x86_64", "windows-x86_64"),
];

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/download", get(download))
        .route("/cli/install.sh", get(shell_installer))
        .route("/cli/install.ps1", get(powershell_installer))
        .route(
            "/install",
            get(|| async { Redirect::permanent("/download") }),
        )
        .route(
            "/desktop",
            get(|| async { Redirect::permanent("/download") }),
        )
}

/// The installers are published with the site, separately from the binaries.
async fn shell_installer() -> Response {
    installer(include_str!("../../../../scripts/install/coder.sh"))
}

async fn powershell_installer() -> Response {
    installer(include_str!("../../../../scripts/install/coder.ps1"))
}

fn installer(script: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        script,
    )
        .into_response()
}

/// A numbered section: `[1] Title`, a line under it, and its body.
fn section(number: u8, title: &str, lead: &str, body: &str) -> String {
    format!(
        "<section class=\"dl-section\"><h2><span class=\"dim\">[{number}]</span> {title}</h2>\
<p class=\"label\">{lead}</p>{body}</section>"
    )
}

async fn download() -> Response {
    let coder = format!(
        "<p class=\"hint\">macOS and Linux:</p><pre><code>{CODER_SH}</code></pre>\
<p class=\"hint\">Windows, in PowerShell:</p><pre><code>{CODER_PS1}</code></pre>\
<p>Installs <code>coder</code>, <code>openagents</code>, and <code>microcoder</code> together. \
Run <code>coder</code> to open the new terminal, or <code>openagents --help</code> for the CLI. \
Run the install command again to update.</p>\
<p class=\"hint\">Windows RC: local task services and background automation require macOS or Linux.</p>\
<details><summary>Download binaries manually</summary>{}</details>",
        coder_binaries(),
    );
    let body = format!(
        "<h1>Download Coder</h1>{}",
        section(
            1,
            "Coder + OpenAgents CLI",
            &format!("Release candidate {CODER_VERSION}."),
            &coder
        ),
    );
    page("Download Coder", Some("/download"), &body)
}

fn coder_binaries() -> String {
    let mut body = String::from("<ul class=\"dl-list\">");
    for (label, platform) in CODER_PLATFORMS {
        let extension = if platform.starts_with("windows-") {
            ".exe"
        } else {
            ""
        };
        body.push_str(&format!(
            "<li class=\"dl-row\"><span class=\"dl-name\"><strong>{label}</strong></span>\
<span><a href=\"{CODER_BASE}/coder-{CODER_VERSION}-{platform}{extension}\">[ Coder ]</a> \
<a href=\"{CODER_BASE}/openagents-{CODER_VERSION}-{platform}{extension}\">[ CLI ]</a> \
<a href=\"{CODER_BASE}/microcoder-{CODER_VERSION}-{platform}{extension}\">[ Microcoder ]</a>"
        ));
        if platform.starts_with("windows-") {
            body.push_str(&format!(
                " <a href=\"{CODER_BASE}/coder-boundary-{CODER_VERSION}-{platform}.exe\">[ Launcher ]</a>"
            ));
        }
        body.push_str("</span></li>");
    }
    body.push_str(&format!(
        "</ul><p class=\"hint\">The installers verify the \
<a href=\"{CODER_BASE}/SHA256SUMS-coder-{CODER_VERSION}\">SHA-256 checksums</a> \
and install the companion commands. For a manual install, download every file in your \
platform's row into <code>~/.openagents/bin</code>. On macOS and Linux, rename the \
files to <code>coder</code>, <code>openagents</code>, and <code>microcoder</code>, then run \
<code>chmod +x ~/.openagents/bin/coder ~/.openagents/bin/openagents \
~/.openagents/bin/microcoder</code>. On Windows, use \
<code>coder.exe</code>, <code>openagents.exe</code>, <code>microcoder.exe</code>, and \
<code>coder-boundary.exe</code>. Add that directory to PATH or run Coder by its full path.</p>"
    ));
    body
}
