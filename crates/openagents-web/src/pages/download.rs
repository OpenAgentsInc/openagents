//! `/download`: Coder's published release candidate and its installers.
//! Coder is one download per platform: an archive holding `coder`, the
//! `openagents` command, and the engine Coder runs its turns with, which
//! the page never offers as a separate product (`scripts/release/coder.sh`).
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

/// The Coder release the page and the installers' default channel name.
pub(crate) const CODER_VERSION: &str = "1.0.0-rc.5";
pub(crate) const CODER_BASE: &str =
    "https://storage.googleapis.com/openagentsgemini-cli-releases/coder";
pub(crate) const CODER_SH: &str = "curl -fsSL https://openagents.com/cli/install.sh | bash";
pub(crate) const CODER_PS1: &str = "irm https://openagents.com/cli/install.ps1 | iex";

/// Each platform's name on the page and in its archive's file name.
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
    let manual = coder_downloads(CODER_VERSION);
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Download Coder" }
            section aria-labelledby="coder-title" {
                h2 #coder-title { "Coder" }
                p.oa-page-meta { "Release candidate " (CODER_VERSION) "." }
                p { "macOS and Linux:" }
                pre { code { (CODER_SH) } }
                p { "Windows, in PowerShell:" }
                pre { code { (CODER_PS1) } }
                p {
                    "Run " code { "coder" } " to open the new terminal. Coder also adds the "
                    code { "openagents" } " command; run " code { "openagents --help" }
                    " to see it. Run the install command again to update."
                }
                p.oa-page-meta {
                    "Windows RC: local task services and background automation require macOS or Linux."
                }
            }
        }))
        @if let Some(manual) = manual {
            details.oa-disclosure {
                summary { "Download Coder manually" }
                (manual)
            }
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

/// Whether `version` was published as one archive per platform. Releases
/// up to `1.0.0-rc.5` were published as separate executables, which the
/// installers still read but the page does not list.
pub(crate) fn published_as_archives(version: &str) -> bool {
    match version.strip_prefix("1.0.0-rc.") {
        Some(candidate) => candidate.parse::<u32>().is_ok_and(|n| n > 5),
        None => true,
    }
}

/// The one file a platform's Coder download is: a `.tar.gz`, or a `.zip`
/// on Windows.
pub(crate) fn coder_archive(version: &str, platform: &str) -> String {
    let extension = if platform.starts_with("windows-") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("coder-{version}-{platform}.{extension}")
}

/// One "Coder for <platform>" download per platform, or nothing for a
/// release published before the archives.
pub(crate) fn coder_downloads(version: &str) -> Option<Markup> {
    if !published_as_archives(version) {
        return None;
    }
    let mut table = Table::new()
        .label("Coder downloads")
        .header(["Platform", "Download"]);
    for (label, platform) in CODER_PLATFORMS {
        table = table.row([
            html! { strong { (label) } },
            html! { (file(&format!("Coder for {label}"), &coder_archive(version, platform))) },
        ]);
    }
    Some(html! {
        (table)
        (MarkdownRoot::new(html! {
            p.oa-page-meta {
                "The installers check the "
                (file("SHA-256 checksums", &format!("SHA256SUMS-coder-{version}")))
                " for you. To install by hand on macOS or Linux, extract your platform's \
    download into " code { "~/.openagents/bin" } ": "
                code { "mkdir -p ~/.openagents/bin && tar -xzf coder-" (version) "-PLATFORM.tar.gz -C ~/.openagents/bin" }
                ". On Windows, extract the .zip into "
                code { "%USERPROFILE%\\.openagents\\bin" }
                ". Add that folder to PATH, or run Coder by its full path."
            }
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earlier_candidates_were_separate_files_and_later_releases_are_archives() {
        for version in ["1.0.0-rc.1", "1.0.0-rc.5"] {
            assert!(!published_as_archives(version), "{version}");
            assert!(coder_downloads(version).is_none(), "{version}");
        }
        for version in ["1.0.0-rc.6", "1.0.0-rc.12", "1.0.0", "1.1.0-rc.1"] {
            assert!(published_as_archives(version), "{version}");
        }
    }

    #[test]
    fn each_platform_is_one_coder_download() {
        let page = coder_downloads("1.0.0").unwrap().into_string();
        let links: Vec<&str> = page
            .split("href=\"")
            .skip(1)
            .map(|rest| &rest[..rest.find('"').unwrap()])
            .collect();
        let mut expected: Vec<String> = CODER_PLATFORMS
            .iter()
            .map(|(_, platform)| format!("{CODER_BASE}/{}", coder_archive("1.0.0", platform)))
            .collect();
        expected.push(format!("{CODER_BASE}/SHA256SUMS-coder-1.0.0"));
        assert_eq!(links, expected);
        assert!(expected[0].ends_with("coder-1.0.0-macos-aarch64.tar.gz"));
        assert!(expected[6].ends_with("coder-1.0.0-windows-x86_64.zip"));
        for (label, _) in CODER_PLATFORMS {
            assert!(page.contains(&format!(">Coder for {label}<")), "{label}");
        }
        assert!(!page.to_lowercase().contains("microcoder"), "{page}");
    }
}
