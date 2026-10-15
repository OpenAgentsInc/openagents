//! `/landing`: the homepage we're building to replace `/` when it's ready.
//! A statement on the left with the download button, and our own product
//! screenshots on the right, turning on their own (CSS only; no script).
//! Not linked from anywhere yet.

use axum::Router;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};

use crate::App;

const CSS: &str = include_str!("../../static/landing/landing.css");
const CSS_PATH: &str = "/landing/landing.css";
const RESEARCH: &str =
    "https://github.com/OpenAgentsInc/openagents/blob/main/docs/inference/briefed-agent-ab.md";
const GITHUB: &str = "https://github.com/OpenAgentsInc/openagents";
const X: &str = "https://x.com/OpenAgentsInc";

/// The screenshots, in order: path, picture, title and line.
const SLIDES: [(&str, &[u8], &str, &str); 3] = [
    (
        "/landing/web-cad.jpg",
        include_bytes!("../../static/landing/web-cad.jpg"),
        "OpenAgents on the web",
        "Design a part by talking, then pick a face and change it.",
    ),
    (
        "/landing/web-measure.jpg",
        include_bytes!("../../static/landing/web-measure.jpg"),
        "Every edit checked",
        "Measure it, cut a section, compare versions.",
    ),
    (
        "/landing/phone-coder.jpg",
        include_bytes!("../../static/landing/phone-coder.jpg"),
        "Coder on your phone",
        "Follow every step it takes, wherever you are.",
    ),
];

pub(crate) fn routes() -> Router<App> {
    let mut router = Router::new()
        .route("/landing", get(|| async { Html(page().into_string()) }))
        .route(
            CSS_PATH,
            get(|| async { ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], CSS) }),
        );
    for (path, bytes, _, _) in SLIDES {
        router =
            router.route(
                path,
                get(move || async move {
                    ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response()
                }),
            );
    }
    router
}

pub(crate) fn page() -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="dark";
                title { "OpenAgents" }
                meta name="description" content="Coder sits in front of every coding agent you use, and sends each step to the model it needs.";
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                link rel="stylesheet" href=(CSS_PATH);
            }
            body.landing {
                main.landing-main {
                    section.landing-copy {
                        p.landing-lead {
                            img.landing-mark src="/favicon.svg" alt="" width="28" height="28";
                            "Coder sits in front of every coding agent you use."
                        }
                        p { "It works out what each step needs." }
                        p {
                            "Small steps go to small, fast models."
                            br;
                            "Only the hard parts reach the big one."
                        }
                        p.landing-research {
                            "Research: "
                            a href=(RESEARCH) {
                                "same model, 21 real issues: half the cost per fix, twice as fast"
                            }
                        }
                        a.landing-download href="/download" { "Download Coder" }
                    }
                    section.landing-shots aria-label="OpenAgents screenshots" {
                        div.landing-stage {
                            @for (index, (path, _, title, line)) in SLIDES.iter().enumerate() {
                                figure class=(format!("landing-slide landing-slide-{index}")) {
                                    div.landing-frame {
                                        img src=(path) alt=(title) loading=(if index == 0 { "eager" } else { "lazy" });
                                    }
                                    figcaption {
                                        span.landing-caption-title { (title) }
                                        span.landing-caption-line { (line) }
                                    }
                                }
                            }
                        }
                        div.landing-dots aria-hidden="true" {
                            @for index in 0..SLIDES.len() {
                                span class=(format!("landing-dot landing-dot-{index}")) {}
                            }
                        }
                    }
                }
                footer.landing-footer {
                    div.landing-badges {
                        a href=(GITHUB) { "Open source" }
                        span { "Plugin authors paid in Bitcoin" }
                    }
                    nav.landing-links aria-label="Links" {
                        a href=(X) { "X" }
                        a href=(GITHUB) { "GitHub" }
                        a href="/privacy" { "Privacy" }
                        a href="/terms" { "Terms" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_landing_page_has_its_statement_download_and_shots() {
        let page = page().into_string();
        assert!(page.contains("Coder sits in front of every coding agent you use."));
        assert!(page.contains("href=\"/download\""));
        assert!(page.contains(RESEARCH));
        for (path, bytes, title, _) in SLIDES {
            assert!(page.contains(path), "{path}");
            assert!(page.contains(title), "{title}");
            assert!(bytes.starts_with(&[0xFF, 0xD8]), "{path} is a JPEG");
        }
        // No inline styles or scripts: the site's policy allows neither.
        assert!(!page.contains("<script") && !page.contains("style="));
        crate::copy_guard::assert_plain("/landing", &page);
    }
}
