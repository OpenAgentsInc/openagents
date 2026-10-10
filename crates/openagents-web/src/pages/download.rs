//! `/download`: every way to get OpenAgents, from one table
//! ([`DOWNLOADS`]): Coder for macOS, Linux, and Windows; the desktop app
//! for the same three; the Android and iPhone apps; and the web app.
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

/// The Coder release the page names and links. It stays at the newest
/// published release: set it to a new version only after
/// `scripts/release/coder.sh` has published that version
/// (`docs/release/terminal.md`), or the page links files that don't exist.
pub(crate) const CODER_VERSION: &str = "1.0.0-rc.6";
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

/// Which part of the page a download belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Desktop,
    Phone,
    Web,
}

/// One download: the platform it's for, what the link says, and where it
/// goes. A row with no `url` isn't shown; set it when the file is public.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Download {
    pub(crate) part: Part,
    pub(crate) platform: &'static str,
    pub(crate) label: &'static str,
    pub(crate) url: Option<&'static str>,
}

/// The desktop app's version: the phone app's, in lockstep
/// (`docs/desktop/release.md`, Version).
pub(crate) const DESKTOP_VERSION: &str = "1.0.0";

/// Whether the desktop release is out. The files under `desktop/<os>/1.0.0/`
/// before the release run are an earlier, never-released build that
/// `scripts/desktop/retire-prerelease.sh` moves aside; turn this on once the
/// release run has published the real ones (`docs/desktop/release.md`,
/// Release day). Until then the page shows no desktop downloads.
pub(crate) const DESKTOP_RELEASED: bool = false;

/// Every download the page offers outside Coder. Coder's own files come
/// from [`CODER_PLATFORMS`] and its installers.
pub(crate) const DOWNLOADS: [Download; 8] = [
    Download {
        part: Part::Desktop,
        platform: "macOS · Apple silicon and Intel",
        label: "Download .dmg",
        url: Some(
            "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg",
        ),
    },
    Download {
        part: Part::Desktop,
        platform: "Linux · x86_64",
        label: "Download AppImage",
        url: Some(
            "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/OpenAgents-1.0.0-x86_64.AppImage",
        ),
    },
    Download {
        part: Part::Desktop,
        platform: "Linux · x86_64",
        label: "Download .deb",
        url: Some(
            "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/openagents_1.0.0_amd64.deb",
        ),
    },
    Download {
        part: Part::Desktop,
        platform: "Windows · x64",
        label: "Download .msi",
        url: Some(
            "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/windows/1.0.0/OpenAgents-1.0.0-x64.msi",
        ),
    },
    Download {
        part: Part::Desktop,
        platform: "Windows · x64",
        label: "Download .zip",
        url: Some(
            "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/windows/1.0.0/OpenAgents-1.0.0-windows-x64.zip",
        ),
    },
    // No public Android build yet: set its address here to show it.
    Download {
        part: Part::Phone,
        platform: "Android",
        label: "download the APK",
        url: None,
    },
    Download {
        part: Part::Phone,
        platform: "iPhone",
        label: "join the beta",
        // Hidden until Apple's beta review approves a build for the external
        // group; until then the link says it isn't accepting testers. Set it
        // back to `Some(super::connect::TESTFLIGHT)` then.
        url: None,
    },
    Download {
        part: Part::Web,
        platform: "Web",
        label: "Open openagents.com",
        url: Some("https://openagents.com/"),
    },
];

/// TestFlight in the App Store, which the iPhone beta needs first.
pub(crate) const TESTFLIGHT_APP: &str = "https://apps.apple.com/app/testflight/id899247664";

/// The downloads of `part` the page shows: those with an address, and
/// the desktop ones only once `desktop` (the release) is out.
pub(crate) fn shown(part: Part, desktop: bool) -> impl Iterator<Item = (Download, &'static str)> {
    DOWNLOADS.into_iter().filter_map(move |download| {
        let live = download.part != Part::Desktop || desktop;
        (download.part == part && live)
            .then_some(download.url)
            .flatten()
            .map(|url| (download, url))
    })
}

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

/// The site's policy plus this site's one counting script and its beacon
/// (`/static/a.js` to `POST /a`, #11153); the page runs no other script.
pub(crate) const DOWNLOAD_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; \
frame-ancestors 'none'";

async fn download(headers: HeaderMap) -> Response {
    let mut response = UiPage::new("Download OpenAgents")
        .section("/download")
        .path("/download")
        .scriptless()
        .head(html! { script src=(crate::analytics::SCRIPT) defer {} })
        .content(page(DESKTOP_RELEASED))
        .respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(DOWNLOAD_POLICY),
    );
    response
}

/// The page's content; `desktop` shows the desktop downloads.
pub(crate) fn page(desktop: bool) -> PageColumn {
    let manual = coder_downloads(CODER_VERSION);
    PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Download OpenAgents" }
            section aria-labelledby="coder-title" {
                h2 #coder-title { "Coder" }
                p.oa-page-meta {
                    @if CODER_VERSION.contains("-rc.") { "Release candidate " } @else { "Version " }
                    (CODER_VERSION) "."
                }
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
        @if desktop {
            (MarkdownRoot::new(html! {
                h2 #desktop-title { "Desktop" }
                p.oa-page-meta { "Version " (DESKTOP_VERSION) "." }
            }))
            (downloads_table("Desktop downloads", Part::Desktop, desktop))
            (MarkdownRoot::new(html! {
                p {
                    "Mac: open the .dmg and drag OpenAgents onto Applications. \
    Needs macOS 13 or later."
                }
                p {
                    "Linux: run " code { "chmod +x OpenAgents-" (DESKTOP_VERSION) "-x86_64.AppImage" }
                    " and open it, or install the .deb with "
                    code { "sudo apt install ./openagents_" (DESKTOP_VERSION) "_amd64.deb" } "."
                }
                p { "Windows may ask you to confirm: More info \u{2192} Run anyway." }
            }))
        }
        (MarkdownRoot::new(html! {
            @if shown(Part::Phone, desktop).next().is_some() {
            section aria-labelledby="phone-title" {
                h2 #phone-title { "Phone" }
                @for (download, url) in shown(Part::Phone, desktop) {
                    @if download.platform == "iPhone" {
                        p {
                            "iPhone: install "
                            (TextLink::new("TestFlight", TESTFLIGHT_APP))
                            " from the App Store, then "
                            (TextLink::new(download.label, url))
                            "."
                        }
                    } @else {
                        p {
                            (download.platform) ": "
                            (TextLink::new(download.label, url).force_external(false))
                            ", open it on your phone, and allow your browser to install apps \
    when asked."
                        }
                    }
                }
            }
            }
            section aria-labelledby="web-title" {
                h2 #web-title { "Web" }
                @for (download, url) in shown(Part::Web, desktop) {
                    p {
                        "Nothing to install: "
                        (TextLink::new(download.label, url).force_external(false))
                        " in any browser."
                    }
                }
            }
        }))
    })
}

/// A table of `part`'s downloads, one row per platform.
fn downloads_table(label: &str, part: Part, desktop: bool) -> Markup {
    let mut rows: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
    for (download, url) in shown(part, desktop) {
        match rows
            .iter_mut()
            .find(|(platform, _)| *platform == download.platform)
        {
            Some((_, links)) => links.push((download.label, url)),
            None => rows.push((download.platform, vec![(download.label, url)])),
        }
    }
    let mut table = Table::new().label(label).header(["Platform", "Download"]);
    for (platform, links) in rows {
        table = table.row([
            html! { strong { (platform) } },
            html! {
                @for (i, (label, url)) in links.iter().enumerate() {
                    @if i > 0 { " \u{b7} " }
                    (TextLink::new(*label, *url).force_external(false))
                }
            },
        ]);
    }
    html! { (table) }
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
    use maud::Render;

    /// Every address on the page: the configured downloads, the
    /// installers, and Coder's files.
    fn every_url() -> Vec<String> {
        let mut urls: Vec<String> = DOWNLOADS
            .iter()
            .filter_map(|download| download.url.map(str::to_owned))
            .collect();
        urls.push(TESTFLIGHT_APP.to_owned());
        urls.push("https://openagents.com/cli/install.sh".to_owned());
        urls.push("https://openagents.com/cli/install.ps1".to_owned());
        for (_, platform) in CODER_PLATFORMS {
            urls.push(format!(
                "{CODER_BASE}/{}",
                coder_archive(CODER_VERSION, platform)
            ));
        }
        urls.push(format!("{CODER_BASE}/SHA256SUMS-coder-{CODER_VERSION}"));
        urls
    }

    #[test]
    fn every_configured_address_is_a_well_formed_https_link() {
        for url in every_url() {
            let parsed = url::Url::parse(&url).unwrap_or_else(|e| panic!("{url}: {e}"));
            assert_eq!(parsed.scheme(), "https", "{url}");
            assert!(parsed.host_str().is_some_and(|h| h.contains('.')), "{url}");
            assert!(!url.contains(char::is_whitespace), "{url}");
            assert_eq!(parsed.as_str(), url, "{url} is not in its plain form");
        }
        assert!(CODER_SH.contains("https://openagents.com/cli/install.sh"));
        assert!(CODER_PS1.contains("https://openagents.com/cli/install.ps1"));
        // Each desktop file is this version's, in its own platform's folder,
        // and is the kind its link says.
        for download in DOWNLOADS.iter().filter(|d| d.part == Part::Desktop) {
            let url = download.url.expect("every desktop file has an address");
            let os = if download.platform.starts_with("macOS") {
                "macos"
            } else if download.platform.starts_with("Linux") {
                "linux"
            } else {
                "windows"
            };
            assert!(
                url.contains(&format!("/desktop/{os}/{DESKTOP_VERSION}/")),
                "{url}"
            );
            let file = &url[url.rfind('/').unwrap() + 1..];
            assert!(file.contains(DESKTOP_VERSION), "{url}");
            let kind = download.label.rsplit(' ').next().unwrap();
            let kind = kind.trim_start_matches('.').to_lowercase();
            assert!(file.to_lowercase().ends_with(&kind), "{url}");
        }
    }

    #[test]
    fn the_page_offers_every_platform_that_has_a_download() {
        let page = page(true).render().into_string();
        for download in DOWNLOADS {
            match download.url {
                Some(url) => {
                    assert!(page.contains(&format!("href=\"{url}\"")), "{url}");
                    assert!(page.contains(download.label), "{}", download.label);
                    assert!(page.contains(download.platform), "{}", download.platform);
                }
                // A download with no address is not on the page at all.
                None => assert!(!page.contains(download.label), "{}", download.label),
            }
        }
        for heading in ["Coder", "Desktop", "Web"] {
            assert!(
                page.contains(&format!("-title\">{heading}</h2>")),
                "{heading}"
            );
        }
        // The iPhone beta is hidden until Apple's review approves a build.
        assert!(!page.contains("phone-title") && !page.contains(TESTFLIGHT_APP));
        assert!(page.contains("Windows may ask you to confirm: More info \u{2192} Run anyway."));
        assert!(page.contains(CODER_SH) && page.contains(CODER_PS1));
        crate::copy_guard::assert_plain("/download", &page);
    }

    #[test]
    fn desktop_downloads_wait_for_the_release() {
        let page = page(false).render().into_string();
        for download in DOWNLOADS.iter().filter(|d| d.part == Part::Desktop) {
            assert!(!page.contains(download.url.unwrap()), "{download:?}");
        }
        assert!(!page.contains("desktop-title") && !page.contains("Run anyway"));
        assert!(!page.contains(super::super::connect::TESTFLIGHT));
        assert!(page.contains("href=\"https://openagents.com/\""));
    }

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
