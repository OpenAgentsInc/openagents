//! `/lander2` and `/beta`: a second, minimal landing page and the beta
//! page it points to.
//!
//! `/lander2` is the landing page's header and footer
//! (`super::landing`), the promo video in the middle, and one Join the
//! Beta button. `/beta` says the beta is about to start and sends people
//! to the contact form with the beta topic, which keeps the request and
//! mails it to us. Neither page is linked from the rest of the site yet.
//!
//! TODO(owner): the promo is the 8-second Blender studio render of the
//! Episode 290 four-app intro (2026-10-19), in VP9 WebM and H.264 MP4,
//! standing in for the final promo cut. Replace `promo.webm`,
//! `static/landing/promo.mp4` and `promo-poster.jpg` when that cut exists.

use axum::Router;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};

use super::landing::{CSS_PATH, button, footer, navbar};
use crate::App;

pub(crate) const LANDER: &str = "/lander2";
pub(crate) const BETA: &str = "/beta";
/// Where `/beta`'s button goes: the contact form, beta topic selected.
const JOIN: &str = "/contact?topic=beta";

const CSS: &str = include_str!("../../static/landing/lander2.css");
const LANDER_CSS: &str = "/landing/lander2.css";
const VIDEO: (&str, &[u8]) = (
    "/landing/promo.mp4",
    include_bytes!("../../static/landing/promo.mp4"),
);
const WEBM: (&str, &[u8]) = (
    "/landing/promo.webm",
    include_bytes!("../../static/landing/promo.webm"),
);
const POSTER: (&str, &[u8]) = (
    "/landing/promo-poster.jpg",
    include_bytes!("../../static/landing/promo-poster.jpg"),
);

/// The site policy plus video from this site, which the promo needs.
const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
     media-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(
            LANDER,
            get(|headers: HeaderMap| async move { lander(&headers) }),
        )
        .route(
            BETA,
            get(|headers: HeaderMap| async move { beta(&headers) }),
        )
        .route(
            LANDER_CSS,
            get(|| async { ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], CSS) }),
        )
        .route(
            VIDEO.0,
            get(|| async { ([(header::CONTENT_TYPE, "video/mp4")], VIDEO.1) }),
        )
        .route(
            WEBM.0,
            get(|| async { ([(header::CONTENT_TYPE, "video/webm")], WEBM.1) }),
        )
        .route(
            POSTER.0,
            get(|| async { ([(header::CONTENT_TYPE, "image/jpeg")], POSTER.1) }),
        )
}

/// `/lander2` for `headers`, in the visitor's theme.
pub(crate) fn lander(headers: &HeaderMap) -> Response {
    let theme = crate::theme::from_headers(headers);
    respond(shell(
        theme,
        "OpenAgents",
        LANDER,
        html! {
            section.lander2 data-slot="promo-section" aria-label="OpenAgents" {
                div.lander2-video {
                    video poster=(POSTER.0) autoplay muted loop playsinline preload="auto"
                        aria-label="One chat on the terminal, the desktop app, the web, and the phone" {
                        source src=(WEBM.0) type="video/webm";
                        source src=(VIDEO.0) type="video/mp4";
                    }
                }
                div.lander2-actions { (button("Join the Beta", BETA, "3d")) }
            }
        },
    ))
}

/// `/beta` for `headers`, in the visitor's theme.
pub(crate) fn beta(headers: &HeaderMap) -> Response {
    let theme = crate::theme::from_headers(headers);
    respond(shell(
        theme,
        "Join the beta",
        BETA,
        html! {
            section.beta data-slot="beta-section" aria-labelledby="beta-title" {
                h1 #beta-title { "The beta is about to start" }
                p { "Hey, we're about to open the OpenAgents beta, and we'd love you to join us." }
                p { "Send us a note and we'll save you a spot. We'll let you know when it's ready." }
                div.lander2-actions { (button("Join the Beta", JOIN, "3d")) }
            }
        },
    ))
}

fn respond(page: Markup) -> Response {
    let mut response = Html(page.into_string()).into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    response
}

/// A page with the landing page's head, header, and footer around `body`.
fn shell(
    theme: Option<openagents_ui::shell::Theme>,
    title: &str,
    path: &str,
    body: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" data-theme=[theme.map(openagents_ui::shell::Theme::as_str)] {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { (title) }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                meta name="robots" content="noindex";
                (maud::PreEscaped(crate::agent_ready::head(title, None, Some(path))))
                (maud::PreEscaped(crate::theme::style_tag()))
                link rel="stylesheet" href=(CSS_PATH);
                link rel="stylesheet" href=(LANDER_CSS);
            }
            body.landing.lander2-page {
                (navbar("/"))
                main { (body) }
                (footer())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn body(response: Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn the_lander_is_the_header_the_video_one_button_and_the_footer() {
        let response = lander(&HeaderMap::new());
        let policy = response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(policy.contains("media-src 'self'"), "{policy}");
        let page = body(response).await;
        for slot in ["navbar", "promo-section", "footer"] {
            assert!(page.contains(&format!("data-slot=\"{slot}\"")), "{slot}");
        }
        assert!(page.contains(&format!("src=\"{}\"", VIDEO.0)));
        assert!(page.contains("muted") && page.contains("playsinline"));
        assert!(page.contains(&format!("href=\"{BETA}\"")));
        assert_eq!(page.matches("Join the Beta").count(), 1);
        // The header's section links go to the landing page's sections.
        assert!(page.contains("href=\"/#how\"") && !page.contains("href=\"#"));
        assert!(!page.contains("style=") && !page.contains("<script>"));
        assert!(page.contains(&format!("src=\"{}\"", WEBM.0)) && !page.contains("controls"));
        assert!(VIDEO.1.len() > 1000 && WEBM.1.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]));
        assert!(POSTER.1.starts_with(&[0xFF, 0xD8]));
        crate::copy_guard::assert_plain(LANDER, &page);
    }

    #[tokio::test]
    async fn the_beta_page_sends_people_to_the_beta_contact_form() {
        let page = body(beta(&HeaderMap::new())).await;
        assert!(page.contains("The beta is about to start"));
        assert!(page.contains("href=\"/contact?topic=beta\""));
        assert!(
            crate::pages::contact::TOPICS
                .iter()
                .any(|(v, _)| *v == "beta")
        );
        crate::copy_guard::assert_plain(BETA, &page);
        let dark = body(beta(&{
            let mut h = HeaderMap::new();
            h.insert(header::COOKIE, HeaderValue::from_static("oa_theme=dark"));
            h
        }))
        .await;
        assert!(dark.contains("data-theme=\"dark\""));
    }
}
