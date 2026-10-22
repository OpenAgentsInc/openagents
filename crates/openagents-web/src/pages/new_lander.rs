//! An isolated marketing preview and on-entry universe selector. `/` is unchanged.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use maud::{Markup, PreEscaped, html};

use crate::App;

const PATH: &str = "/new-lander";
const VERSE: &str = "/new-lander/verse";
const SCRIPT_PATH: &str = "/new-lander/assets/verse.js";
const STYLE: &str = include_str!("new_lander.css");
const SCRIPT: &str = include_str!("new_lander.js");

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(marketing))
        .route(VERSE, get(universe))
        .route(SCRIPT_PATH, get(script))
}

fn mark() -> Markup {
    html! { svg viewBox="0 0 40 40" fill="none" aria-hidden="true" { path d="M12 7a16 16 0 1 0 16 0M20 3v17" stroke="currentColor" stroke-width="3" stroke-linecap="round" {} } }
}

fn header(verse: bool, account: bool) -> Markup {
    let back = if verse {
        "%2Fnew-lander%2Fverse"
    } else {
        "%2Fnew-lander"
    };
    html! {
        header.site-header {
            a.brand href=(PATH) aria-label="OpenAgents home" { (mark()) span { "OpenAgents" } }
            @if verse { a.back-link href=(PATH) { "← Back to OpenAgents" } }
            @else { nav.site-nav aria-label="Main navigation" { a href="#products" { "Products" } a href="/docs" { "Docs" } a href="https://github.com/OpenAgentsInc/openagents" { "GitHub ↗" } } }
            nav.account-nav aria-label="Account" {
                @if account {
                    a href=(format!("/login?return_to={back}")) { "Log in" }
                    a.nav-button href=(format!("/signup?return_to={back}")) { "Sign up ↗" }
                } @else { a.nav-button href="/download" { "Get OpenAgents ↗" } }
            }
        }
    }
}

/// A still, original illustration; marketing never initializes the renderer.
fn poster() -> Markup {
    html! {
        svg.system-art viewBox="0 0 900 700" role="img" aria-label="A luminous planet surrounded by orbits and distant worlds" {
            defs {
                radialGradient id="planet-light" cx="28%" cy="22%" r="80%" { stop offset="0" stop-color="#f7f8fa" {} stop offset=".35" stop-color="#b8bdc5" {} stop offset=".68" stop-color="#3d424b" {} stop offset="1" stop-color="#08090c" {} }
                radialGradient id="halo" { stop stop-color="#c9d5ed" stop-opacity=".15" {} stop offset="1" stop-color="#c9d5ed" stop-opacity="0" {} }
                pattern id="star-pattern" x="0" y="0" width="109" height="91" patternUnits="userSpaceOnUse" { circle cx="18" cy="15" r=".7" fill="#b8c2d0" {} circle cx="76" cy="67" r=".4" fill="#b8c2d0" {} }
            }
            rect width="900" height="700" fill="url(#star-pattern)" opacity=".5" {}
            circle cx="490" cy="341" r="310" fill="url(#halo)" {}
            g fill="none" stroke="#c0c9da" stroke-opacity=".17" {
                ellipse cx="490" cy="341" rx="339" ry="132" transform="rotate(-28 490 341)" {}
                ellipse cx="490" cy="341" rx="278" ry="108" transform="rotate(-28 490 341)" {}
                ellipse cx="490" cy="341" rx="217" ry="86" transform="rotate(-28 490 341)" {}
                circle cx="490" cy="341" r="174" stroke-dasharray="1 10" {}
            }
            circle cx="490" cy="341" r="111" fill="url(#planet-light)" stroke="#e4e8f3" stroke-opacity=".23" {}
            path d="M419 264c38-20 50 24 69 27s17-27 37-7-28 42-12 66 43 12 51 34" fill="none" stroke="#171c27" stroke-width="18" opacity=".2" {}
            circle cx="211" cy="390" r="12" fill="#8f959e" {} circle cx="704" cy="290" r="17" fill="#565b65" {} circle cx="685" cy="155" r="5" fill="#d4d7df" {}
            g fill="#c6cbd7" { circle cx="297" cy="300" r="3" {} circle cx="737" cy="243" r="2" {} circle cx="362" cy="495" r="3" {} circle cx="596" cy="187" r="2" {} }
        }
    }
}

fn marketing_content() -> Markup {
    html! {
        main {
            section.hero aria-labelledby="hero-title" {
                div.hero-copy {
                    p.eyebrow { span.signal {} "OPEN TO EVERY IDEA" }
                    h1 #hero-title { "Your ideas." br; span { "A whole universe." } }
                    p.hero-description { "Tools to create. Agents to help. A place to bring it all together." }
                    a.enter-button href=(VERSE) { span { "Enter the Verse" } span aria-hidden="true" { "↗" } }
                    p.hero-caption { "Explore a first look at our connected worlds." }
                }
                div.hero-art { (poster()) span.orbit-caption { "A WORLD OF POSSIBILITIES" } }
                div.scroll-note { span { "BUILT FOR WHAT COMES NEXT" } a href="#products" aria-label="Explore products" { "↓" } }
            }
            section.products #products aria-labelledby="products-title" {
                div.section-heading { p.eyebrow { "THE OPENAGENTS TOOLKIT" } h2 #products-title { "One platform." br; span { "More ways to create." } } p { "Work with an agent, build with Coder, or bring OpenAgents into your own tools." } }
                div.product-grid {
                    a.product-card href="/chat" { span.product-number { "01 / ASSISTANT" } div.product-icon aria-hidden="true" { "✳" } h3 { "OpenAgents" } p { "An agent to think, build, and work with you." } span.product-action { "Open chat" span { "↗" } } }
                    a.product-card href="/download" { span.product-number { "02 / CODE" } div.product-icon.code-icon aria-hidden="true" { ">_" } h3 { "Coder" } p { "Bring an agent into your development workflow." } span.product-action { "Get Coder" span { "↗" } } }
                    a.product-card href="/download" { span.product-number { "03 / EVERYWHERE" } div.product-icon.app-icon aria-hidden="true" { (mark()) } h3 { "The OpenAgents app" } p { "Find the app and tools for your devices." } span.product-action { "View downloads" span { "↗" } } }
                    a.product-card href="/docs/api" { span.product-number { "04 / BUILD" } div.product-icon.api-icon aria-hidden="true" { "{ }" } h3 { "For developers" } p { "Connect your ideas to the OpenAgents API." } span.product-action { "Explore the API" span { "↗" } } }
                }
            }
            section.invitation { p.eyebrow { "MAKE ROOM FOR YOUR IMAGINATION" } h2 { "Something starts" br; "when you enter." } a.enter-button href=(VERSE) { span { "Enter the Verse" } span aria-hidden="true" { "↗" } } }
        }
        footer.site-footer { a.brand href=(PATH) { (mark()) "OpenAgents" } span { "Create anything." } nav aria-label="Footer" { a href="/docs" { "Docs" } a href="/privacy" { "Privacy" } a href="https://github.com/OpenAgentsInc/openagents" { "Open source ↗" } } }
    }
}

const WORLDS: [(&str, &str, &str); 6] = [
    ("openagents", "OpenAgents", "Company world · Locked"),
    ("blue-rush", "Blue Rush", "Creator studio"),
    ("haunted-house", "Haunted House", "By Blue Rush"),
    ("creator-worlds", "Creator worlds", "Future destination"),
    ("new-horizons", "New horizons", "Future destination"),
    ("far-field", "The far field", "Future destination"),
];

fn universe_content(haunt: Option<&str>, account: bool, signed_in: bool) -> Markup {
    html! {
        main.atlas aria-labelledby="atlas-title" {
            div.universe-stage {
                div.still-universe aria-hidden="true" { (poster()) }
                canvas #universe tabindex="0" aria-label="Interactive universe. Drag to orbit, scroll to zoom, or use the arrow keys and plus and minus. Escape resets the view." {}
                div.world-markers aria-label="Worlds on the map" {
                    @for (id, name, _) in WORLDS {
                        button.world-marker id=(format!("marker-{id}")) type="button" data-world=(id) aria-pressed=(if id=="openagents" {"true"} else {"false"}) { span.marker-dot {} span { (name) } }
                    }
                }
            }
            div.atlas-heading { p.eyebrow { span.signal {} "THE VERSE / A FIRST LOOK" } h1 #atlas-title { "A universe" br; span { "worth exploring." } } p { "Find a world. Follow your curiosity." } }
            aside.destination-panel aria-labelledby="world-title" {
                div.panel-label { span { "DESTINATION" } span { "◎" } }
                p #world-category { "Company world · Locked" }
                h2 #world-title { "OpenAgents" }
                p #world-description { "Our home in the Verse. The Gym, Pylons, and more will open here for their communities." }
                div.destination-actions {
                    div data-destination="openagents" { span.locked-label { "◇ Locked for now" } a.text-link href="/docs" { "About OpenAgents ↗" } }
                    div data-destination="blue-rush" hidden { a.small-button href=(super::blue_rush::PATH) { "Meet Blue Rush ↗" } }
                    div data-destination="haunted-house" hidden {
                        @if let Some(path) = haunt { a.small-button href=(path) { "Enter Haunted House ↗" } }
                        @else { p.destination-note { "The house is not open in this preview. You can explore its studio." } a.text-link href=(super::blue_rush::PATH) { "Meet Blue Rush ↗" } }
                    }
                    @for id in ["creator-worlds", "new-horizons", "far-field"] { div data-destination=(id) hidden { span.locked-label { "◇ Still taking shape" } } }
                }
                div.world-list aria-label="Choose a destination" {
                    @for (id,name,category) in WORLDS {
                        button.world-option type="button" data-world=(id) aria-pressed=(if id=="openagents" {"true"} else {"false"}) { span.world-option-dot {} span { strong { (name) } small { (category) } } span.option-arrow aria-hidden="true" { "↗" } }
                    }
                }
            }
            div.atlas-footer { a.map-home href=(PATH) aria-label="Back to the marketing homepage" { "←" } p #universe-status role="status" { "Preparing the universe…" } div.camera-controls aria-label="Camera controls" { button #zoom-in type="button" aria-label="Zoom in" { "+" } button #zoom-out type="button" aria-label="Zoom out" { "−" } button #reset-view type="button" { "Reset view" } } }
            @if account && !signed_in {
                aside.signin-card aria-label="Sign in to the Verse" { p.eyebrow { "YOUR PLACE IN THE VERSE" } h2 { "Your next world is waiting." } p { "Log in to continue your journey." } a.small-button href="/login?return_to=%2Fnew-lander%2Fverse" { "Log in ↗" } }
            }
            noscript { div.no-script { "This is a still preview. Enable JavaScript for destination selection and the interactive view." a href=(PATH) { "Back to OpenAgents" } } }
        }
    }
}

fn document(content: Markup, verse: bool, account: bool) -> String {
    let path = if verse { VERSE } else { PATH };
    let title = if verse {
        "The Verse · OpenAgents"
    } else {
        "OpenAgents · Create anything"
    };
    html! {
        (maud::DOCTYPE)
        html lang="en" {
            head { meta charset="utf-8"; meta name="viewport" content="width=device-width, initial-scale=1"; meta name="robots" content="noindex, nofollow"; meta name="description" content="Tools to create. Agents to help. Explore the OpenAgents toolkit and the Verse."; meta name="color-scheme" content="dark"; title { (title) } link rel="canonical" href=(format!("https://openagents.com{path}")); style { (PreEscaped(STYLE)) } }
            body class=(if verse {"verse-page"} else {"marketing-page"}) { a.skip-link href=(if verse {"#world-title"} else {"#products"}) { "Skip to content" } (header(verse,account)) (content) @if verse { script type="module" src=(SCRIPT_PATH) {} } }
        }
    }.into_string()
}

fn respond(content: String) -> Response {
    let mut response = Html(content).into_response();
    let headers = response.headers_mut();
    let policy = format!(
        "default-src 'none'; style-src {}; script-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
        super::lander5::source_hash(STYLE)
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&policy).expect("fixed preview policy"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    response
}

async fn marketing(State(app): State<App>) -> Response {
    respond(document(
        marketing_content(),
        false,
        app.config
            .features
            .on(crate::features::Feature::HomeAccount),
    ))
}

async fn universe(State(app): State<App>, headers: HeaderMap) -> Response {
    let haunt = if crate::live_games::tenants().iter().any(|t| t == "haunt") {
        Some("/t/haunt/")
    } else if app.config.haunt.is_some() {
        Some(super::haunt::HAUNT_PATH)
    } else {
        None
    };
    let account = app.config.cloud.is_some();
    let signed_in = if let Some(service) = app.config.cloud.as_deref() {
        service.authenticate(&headers).await.is_ok()
    } else {
        false
    };
    respond(document(
        universe_content(haunt, account, signed_in),
        true,
        account,
    ))
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        SCRIPT,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[test]
    fn new_lander_marketing_never_starts_a_game() {
        let page = document(marketing_content(), false, true);
        assert!(!page.contains("<script") && !page.contains("<canvas") && !page.contains(".wasm"));
        assert!(page.contains("Enter the Verse") && page.contains("noindex, nofollow"));
        crate::copy_guard::assert_plain(PATH, &page);
        crate::copy_guard::assert_plain(
            VERSE,
            &document(universe_content(None, false, false), true, false),
        );
    }

    #[tokio::test]
    async fn new_lander_routes_are_owned_and_the_root_stays_the_grid() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        for path in [PATH, VERSE, SCRIPT_PATH, "/"] {
            let request = Request::builder()
                .uri(path)
                .header(header::HOST, "127.0.0.1:4300")
                .body(Body::empty())
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            if path != "/" {
                assert!(crate::upstream::owned(path));
            }
            let body = String::from_utf8(
                to_bytes(response.into_body(), 2 * 1024 * 1024)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();
            if path == "/" {
                assert!(body.contains("everglade-canvas"));
                assert!(!body.contains(SCRIPT_PATH));
            }
            if path == VERSE {
                assert!(body.contains(SCRIPT_PATH) && body.contains("Locked for now"));
                assert!(!body.contains("Enter Haunted House"));
            }
        }
    }
}
