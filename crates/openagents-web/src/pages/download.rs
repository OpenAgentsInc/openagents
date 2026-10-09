//! `/download`: Coder's published release candidate and its installers.
//! Coder's terminal ships with the OpenAgents CLI and Microcoder.
//! `/install` and `/desktop` redirect here permanently (`308`).

use axum::Router;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::actions::TextLink;
use openagents_ui::content::{MarkdownRoot, PageColumn, Table};

use crate::App;
use crate::ui_page::UiPage;

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

async fn download(headers: HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Download Coder" }
            section aria-labelledby="coder-title" {
                h2 #coder-title { "Coder + OpenAgents CLI" }
                p.oa-page-meta { "Release candidate " (CODER_VERSION) "." }
                p { "macOS and Linux:" }
                pre { code { (CODER_SH) } }
                p { "Windows, in PowerShell:" }
                pre { code { (CODER_PS1) } }
                p {
                    "Installs " code { "coder" } ", " code { "openagents" } ", and "
                    code { "microcoder" } " together. Run " code { "coder" }
                    " to open the new terminal, or " code { "openagents --help" }
                    " for the CLI. Run the install command again to update."
                }
                p.oa-page-meta {
                    "Windows RC: local task services and background automation require macOS or Linux."
                }
            }
        }))
        details.oa-disclosure {
            summary { "Download binaries manually" }
            (coder_binaries())
        }
    });
    UiPage::new("Download Coder")
        .section("/download")
        .path("/download")
        .scriptless()
        .content(content)
        .respond(&headers)
}

/// A link to one release file, opening in place (a download, not a tab).
fn file(label: &str, name: &str) -> TextLink {
    TextLink::new(label, format!("{CODER_BASE}/{name}")).force_external(false)
}

fn coder_binaries() -> Markup {
    let mut table = Table::new()
        .label("Coder release files")
        .header(["Platform", "Files"]);
    for (label, platform) in CODER_PLATFORMS {
        let windows = platform.starts_with("windows-");
        let extension = if windows { ".exe" } else { "" };
        table = table.row([
            html! { strong { (label) } },
            html! {
                (file("Coder", &format!("coder-{CODER_VERSION}-{platform}{extension}")))
                " \u{b7} "
                (file("CLI", &format!("openagents-{CODER_VERSION}-{platform}{extension}")))
                " \u{b7} "
                (file("Microcoder", &format!("microcoder-{CODER_VERSION}-{platform}{extension}")))
                @if windows {
                    " \u{b7} "
                    (file("Launcher", &format!("coder-boundary-{CODER_VERSION}-{platform}.exe")))
                }
            },
        ]);
    }
    html! {
        (table)
        (MarkdownRoot::new(html! {
            p.oa-page-meta {
                "The installers verify the "
                (file("SHA-256 checksums", &format!("SHA256SUMS-coder-{CODER_VERSION}")))
                " and install the companion commands. For a manual install, download every \
    file in your platform's row into "
                code { "~/.openagents/bin" }
                ". On macOS and Linux, rename the files to " code { "coder" } ", "
                code { "openagents" } ", and " code { "microcoder" } ", then run "
                code {
                    "chmod +x ~/.openagents/bin/coder ~/.openagents/bin/openagents \
    ~/.openagents/bin/microcoder"
                }
                ". On Windows, use " code { "coder.exe" } ", " code { "openagents.exe" } ", "
                code { "microcoder.exe" } ", and " code { "coder-boundary.exe" }
                ". Add that directory to PATH or run Coder by its full path."
            }
        }))
    }
}
