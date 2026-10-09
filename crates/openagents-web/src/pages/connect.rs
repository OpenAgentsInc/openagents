//! The phone app's links: `/connect`, and the two files that let the
//! OpenAgents app on a phone claim it.
//!
//! The OpenAgents desktop app's pairing QR code is the link
//! `https://openagents.com/connect#<code>` (NIP-HOST, "Connect codes";
//! `openagents_connect::code::LINK_PREFIX`). A phone's system camera opens
//! it: with the app installed, iOS (through
//! `/.well-known/apple-app-site-association`) and Android (through
//! `/.well-known/assetlinks.json`) hand the link to the app, which pairs
//! with the computer. Without the app, the browser loads `/connect`, which
//! says where to get it.
//!
//! The code is the URL's fragment, which a browser never sends, so no
//! request here ever carries it. The page keeps it that way: it runs no
//! script, its content security policy allows none, it sends no referrer,
//! and it links nowhere that could read the fragment. Nothing here logs.
//!
//! This is the private Coder service's `app_links.rs`, reimplemented here
//! with the same association files, policy, and copy.

use axum::Router;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::html;
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::ui_page::UiPage;

/// The OpenAgents app's Apple team and bundle, and its Android package.
pub(crate) const APPLE_APP_ID: &str = "HQWSG26L43.com.openagents.app";
pub(crate) const ANDROID_PACKAGE: &str = "com.openagents.app";
/// The SHA-256 of the certificate that signs the Android app's releases
/// (alias `openagents`; `bins/openagents-android/README.md`).
pub(crate) const ANDROID_CERT_SHA256: &str = "DB:D0:E9:65:5A:7A:0D:E2:E7:A4:F6:D4:59:F9:AF:52:A8:54:D8:CB:C1:FC:3B:B0:62:7E:2B:C2:AD:44:44:80";
/// The public TestFlight link for the OpenAgents iPhone app.
pub(crate) const TESTFLIGHT: &str = "https://testflight.apple.com/join/dvQdns5B";

/// No script, no frame, no form, no connection: styles and images from
/// this site only.
pub(crate) const PAGE_POLICY: &str = "default-src 'none'; style-src 'self'; img-src 'self'; font-src 'self'; \
     base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/connect", get(connect))
        .route(
            "/.well-known/apple-app-site-association",
            get(apple_app_site_association),
        )
        .route("/.well-known/assetlinks.json", get(assetlinks))
}

/// iOS universal links: the OpenAgents app opens `/connect` and nothing
/// else.
pub(crate) fn apple_association() -> serde_json::Value {
    serde_json::json!({
        "applinks": {
            "details": [{
                "appIDs": [APPLE_APP_ID],
                "components": [{
                    "/": "/connect",
                    "comment": "Pairing a computer; the code is the fragment."
                }]
            }]
        }
    })
}

/// Android App Links: the OpenAgents app, signed with its release key,
/// may open this site's links; its manifest claims only `/connect`.
pub(crate) fn android_association() -> serde_json::Value {
    serde_json::json!([{
        "relation": ["delegate_permission/common.handle_all_urls"],
        "target": {
            "namespace": "android_app",
            "package_name": ANDROID_PACKAGE,
            "sha256_cert_fingerprints": [ANDROID_CERT_SHA256]
        }
    }])
}

/// Served as JSON with no redirect, as iOS requires.
async fn apple_app_site_association() -> Response {
    json(&apple_association())
}

async fn assetlinks() -> Response {
    json(&android_association())
}

fn json(value: &serde_json::Value) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        value.to_string(),
    )
        .into_response()
}

/// `GET /connect`: the page a phone without the OpenAgents app lands on
/// when its camera opens the desktop app's QR code.
pub(crate) async fn connect(headers: HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        article.oa-card id="connect" {
            (MarkdownRoot::new(html! {
                h1 { "Get the OpenAgents app" }
                p {
                    "This code connects your phone to a computer running OpenAgents. \
    Install the OpenAgents app, then scan the code on your computer again."
                }
                p { "iPhone: " a href=(TESTFLIGHT) { "get OpenAgents on TestFlight" } "." }
                p { "Android: the OpenAgents app is in testing and not yet public." }
                p {
                    "Already have the app? Open it, choose Connect a computer, and point it \
    at the code."
                }
            }))
        }
        (MarkdownRoot::new(html! {
            p.oa-page-meta {
                "On a computer? " a href="/download" { "Get OpenAgents for Mac" }
                ", and it shows the code to scan."
            }
        }))
    });
    // No script and no form: the code in the fragment stays in the browser.
    let mut response = UiPage::new("Get the OpenAgents app")
        .path("/connect")
        .scriptless()
        .without_toggle()
        .content(content)
        .respond(&headers);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PAGE_POLICY),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{HeaderMap, StatusCode};

    async fn read(response: Response) -> (StatusCode, HeaderMap, String) {
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, headers, String::from_utf8(body.to_vec()).unwrap())
    }

    /// The page a phone without the app lands on runs no script and allows
    /// none, so nothing can read the code in its fragment or send it on.
    #[tokio::test]
    async fn the_connect_page_runs_no_script_and_points_to_the_app() {
        let (status, headers, body) = read(connect(HeaderMap::new()).await).await;
        assert_eq!(status, StatusCode::OK);
        let lower = body.to_ascii_lowercase();
        assert!(!lower.contains("<script"), "{body}");
        assert!(!lower.contains("javascript:"), "{body}");
        // No inline event handler such as `onload=`.
        for (at, _) in lower.match_indices(" on") {
            let name: String = lower[at + 3..]
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            assert!(
                !lower[at + 3 + name.len()..].starts_with('='),
                "an inline handler: {body}"
            );
        }
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(policy.starts_with("default-src 'none';"), "{policy}");
        assert!(!policy.contains("script-src"), "{policy}");
        assert!(policy.contains("form-action 'none'"), "{policy}");
        assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert!(body.contains("Get the OpenAgents app"));
        assert!(body.contains(TESTFLIGHT));
        assert!(!lower.contains("<form"), "{body}");
    }

    /// iOS reads the association as JSON at the exact path, for this app
    /// and only `/connect`.
    #[tokio::test]
    async fn the_apple_association_names_the_app_and_only_connect() {
        let (status, headers, body) = read(apple_app_site_association().await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        let details = &value["applinks"]["details"];
        assert_eq!(details.as_array().unwrap().len(), 1);
        assert_eq!(details[0]["appIDs"], serde_json::json!([APPLE_APP_ID]));
        let components = details[0]["components"].as_array().unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0]["/"], "/connect");
    }

    #[tokio::test]
    async fn the_android_association_names_the_release_key() {
        let (status, headers, body) = read(assetlinks().await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        let target = &value[0]["target"];
        assert_eq!(target["package_name"], ANDROID_PACKAGE);
        let fingerprint = target["sha256_cert_fingerprints"][0].as_str().unwrap();
        assert_eq!(fingerprint.split(':').count(), 32);
        assert!(fingerprint.split(':').all(|byte| {
            byte.len() == 2
                && byte
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
        }));
    }
}
