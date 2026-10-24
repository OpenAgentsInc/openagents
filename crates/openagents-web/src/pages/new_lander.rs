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
pub(crate) fn universe_release() -> Option<super::medieval_town::Release> {
    let text = std::env::var("OPENAGENTS_UNIVERSE_RELEASE")
        .unwrap_or_else(|_| include_str!("../../content/demos/universe.json").into());
    let release = super::medieval_town::Release::parse(&text)?;
    (release.project.id == "universe" && release.project.branch == "main").then_some(release)
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(super::verse_ui::routes())
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
        header.lp-header {
            a.lp-brand href=(PATH) aria-label="OpenAgents home" { (mark()) span { "OpenAgents" } }
            @if verse { a.lp-back href=(PATH) { "← OpenAgents" } }
            @else { nav.lp-nav aria-label="Main navigation" { a href="#products" { "Products" } a href="/docs" { "Docs" } a href="https://github.com/OpenAgentsInc/openagents" { "GitHub ↗" } } }
            nav.lp-account aria-label="Account" {
                @if account {
                    a.hud-button.hud-ghost href=(format!("/login?return_to={back}")) { "Log in" }
                    a.hud-button href=(format!("/signup?return_to={back}")) { "Sign up" }
                } @else if !verse {
                    a.hud-button href=(VERSE) { "Enter the Verse" }
                }
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
        main.lp {
            section.lp-hero aria-labelledby="hero-title" {
                div.lp-hero-copy {
                    p.hud-label { span.hud-signal {} "OPEN TO EVERY IDEA" }
                    h1 #hero-title { "Your ideas." br; span { "A whole universe." } }
                    p.lp-lede { "Tools to create. Agents to help. A place to bring it all together." }
                    a.hud-button.hud-primary.hud-large href=(VERSE) { span { "Enter the Verse" } span aria-hidden="true" { "↗" } }
                    p.lp-caption { "Explore a first look at our connected worlds." }
                }
                div.lp-hero-art aria-hidden="true" { (poster()) }
            }
            section.lp-products #products aria-labelledby="products-title" {
                div.lp-section-heading { p.hud-label { "THE OPENAGENTS TOOLKIT" } h2 #products-title { "One platform. " span { "More ways to create." } } p { "Explore the Verse, make a world, or build with our API." } }
                div.lp-product-grid {
                    a.hud-card.lp-product href=(VERSE) { span.hud-index { "01 / EXPLORE" } h3 { "The Verse" } p { "Discover connected worlds from your browser." } span.lp-product-action { "Enter the Verse ↗" } }
                    a.hud-card.lp-product href="/new-lander/house" { span.hud-index { "02 / CREATE" } h3 { "Haunted House" } p { "Step inside, make a change, and share your world." } span.lp-product-action { "Explore the house ↗" } }
                    a.hud-card.lp-product href="/docs/api" { span.hud-index { "03 / BUILD" } h3 { "For developers" } p { "Connect your ideas to the OpenAgents API." } span.lp-product-action { "Explore the API ↗" } }
                }
            }
            section.lp-invitation { p.hud-label { "MAKE ROOM FOR YOUR IMAGINATION" } h2 { "Something starts when you enter." } a.hud-button.hud-primary.hud-large href=(VERSE) { span { "Enter the Verse" } span aria-hidden="true" { "↗" } } }
        }
        footer.lp-footer { a.lp-brand href=(PATH) { (mark()) span { "OpenAgents" } } span { "Create anything." } nav aria-label="Footer" { a href="/docs" { "Docs" } a href="/privacy" { "Privacy" } a href="https://github.com/OpenAgentsInc/openagents" { "Open source ↗" } } }
    }
}

/// What the Verse controller (`apps/verse-ui/src/aethyr/verse.ts`) needs
/// from the server. World names come from the engine host's one list.
fn verse_config(haunt: Option<&str>, account: bool, signed_in: bool) -> String {
    let worlds: Vec<_> = aethyrforge_host::WORLDS
        .iter()
        .map(|w| serde_json::json!({"id": w.id, "name": w.name, "category": w.category, "description": w.description}))
        .collect();
    serde_json::json!({
        "accounts": account,
        "signedIn": signed_in,
        "hauntPath": haunt,
        "blueRushPath": super::blue_rush::PATH,
        "mapPath": VERSE,
        "worlds": worlds,
    })
    .to_string()
    .replace('<', "\\u003c")
}

/// The Verse page is a shell (#12588). With JavaScript, the Aethyr
/// controller draws the interface into `#verse-hud`. The `.verse-bridge`
/// blocks are the server's own copy of the controls: they are the page
/// without JavaScript, and they carry the element IDs the pinned engine
/// release (`aethyrforge-host`'s `mount_universe`) reads and writes. The
/// web host hides them once it mounts and relays intents to them.
fn universe_content(haunt: Option<&str>, account: bool, signed_in: bool) -> Markup {
    let release = universe_release();
    html! {
        main.verse id="verse-atlas" data-entry=(if signed_in {"member"} else {"locked"}) aria-labelledby="atlas-title" {
            div.verse-world #universe-exploration inert {
                div.verse-stage {
                    div.verse-still aria-hidden="true" { (poster()) }
                    @if let Some(release) = &release {
                        iframe id="engine" title="The Verse planets" data-engine-url=(&release.engine_url) {}
                        @let config = serde_json::json!({"release":release.release,"engine_abi":release.engine_abi,"project_format":release.project_format,"project":release.project});
                        script id="engine-config" type="application/json" { (PreEscaped(config.to_string().replace('<', "\\u003c"))) }
                    }
                    p.hud-toast id="world-notice" role="status" { "Opening the planets… You can also choose a destination below." }
                }
                div.verse-title { p.hud-label { span.hud-signal {} "THE VERSE / EXPLORATION" } h1 #atlas-title { "Choose your world." } }
                div.verse-bridge {
                    aside.hud-panel.verse-panel aria-labelledby="world-title" {
                        div.verse-panel-head {
                            div {
                                p.hud-label #world-category { "Company world · Locked" }
                                h2 #world-title { "OpenAgents" }
                            }
                            span.verse-panel-icon { (super::verse_ui::icon("map")) }
                        }
                        p #world-description { "Our home in the Verse. The Gym and Pylons will open here for their communities." }
                        div.verse-actions {
                            div id="destination-openagents" { span.hud-label.verse-locked { "◇ Locked for now" } a.hud-link href="/docs" { "About OpenAgents ↗" } }
                            div id="destination-blue-rush" hidden { a.hud-button.hud-primary href=(super::blue_rush::PATH) { "Meet Blue Rush ↗" } }
                            div id="destination-haunted-house" hidden {
                                @if let Some(path) = haunt { a.hud-button.hud-primary href=(path) { "Enter Haunted House ↗" } }
                                @else { p { "The house is not open here yet." } }
                            }
                            div id="destination-creator-worlds" hidden {
                                p id="planet-status" role="status" { "Publish your own house, choose Play or Share, then return here." }
                                div.hud-card id="planet-ready" hidden {
                                    h3 { "A place of your own" }
                                    p { "Your house is published. Create your first personal planet." }
                                    p.verse-fine { "64 × 64 metres · Up to 128 objects · Private to you. Uses one account project; your account's project limit applies." }
                                    button.hud-button.hud-primary id="planet-create" type="button" { "Create my planet" }
                                }
                                div.hud-card id="planet-owned" hidden {
                                    h3 { "Your planet is ready" }
                                    a.hud-button.hud-primary id="planet-setup" { "Set up my planet ↗" }
                                }
                            }
                            @for id in ["new-horizons", "far-field"] { div id=(format!("destination-{id}")) hidden { span.hud-label.verse-locked { "◇ Still taking shape" } } }
                        }
                        div.verse-worlds role="group" aria-label="Choose a destination" {
                            @for world in aethyrforge_host::WORLDS {
                                button.verse-world-chip id=(format!("option-{}",world.id)) data-world=(world.id) type="button" aria-pressed=(if world.id=="openagents" {"true"} else {"false"}) { span.verse-dot aria-hidden="true" {} span { strong { (world.name) } small { (world.category) } } }
                            }
                        }
                    }
                    div.verse-controls {
                        p.hud-label id="universe-status" { "Drag to orbit · Scroll to zoom · Select a world" }
                        a.hud-button.hud-ghost id="world-retry" href=(VERSE) { "Reload map" }
                        button.hud-button.hud-ghost id="world-fullscreen" type="button" { "Full screen" }
                    }
                }
            }
            div.verse-hud #verse-hud {}
            script id="verse-config" type="application/json" { (PreEscaped(verse_config(haunt, account, signed_in))) }
            div.verse-bridge {
                dialog.hud-panel.verse-entry id="verse-entry" aria-labelledby="entry-title" aria-describedby="entry-description" {
                    div.verse-entry-head { span.hud-label { "OPENAGENTS / THE VERSE" } span.verse-entry-mark aria-hidden="true" { (mark()) } }
                    section id="verse-account" {
                        p.hud-label { "YOUR PLACE IN THE VERSE" }
                        h2 #entry-title { "Your next world is waiting." }
                        p #entry-description { @if account { "Log in, or look around as a guest." } @else { "Look around as a guest." } }
                        div.verse-entry-actions {
                            a.hud-button id="verse-signin" hidden[!account] href="/login?return_to=%2Fnew-lander%2Fverse" { "Log in" span aria-hidden="true" { "↗" } }
                            button.hud-button.hud-primary id="continue-guest" type="button" { "Continue as guest" span aria-hidden="true" { "↗" } }
                        }
                    }
                    section id="verse-choices" hidden {
                        p.hud-label { "CHOOSE YOUR NEXT STEP" }
                        h2 id="choice-title" { "Where shall we go?" }
                        p id="choice-description" { "Explore the planets, or step inside the Haunted House." }
                        div.verse-entry-actions {
                            button.hud-button id="verse-explore" type="button" { "Explore the planets" span aria-hidden="true" { "↗" } }
                            @if let Some(path) = haunt { a.hud-button.hud-primary href=(path) { "Haunted House tutorial" span aria-hidden="true" { "↗" } } }
                        }
                    }
                    a.hud-link.verse-entry-back href=(PATH) { "← Back to OpenAgents" }
                }
                nav.hud-panel.verse-fallback aria-label="Other ways to explore" {
                    a href=(VERSE) { "Try the 3D view again" }
                    a href=(PATH) { "Back to OpenAgents" }
                    @if let Some(path) = haunt { a href=(path) { "Enter Haunted House" } }
                    a href=(super::blue_rush::PATH) { "Meet Blue Rush" }
                }
            }
            noscript { p.hud-toast { "This is a still view. You can use the links to visit a destination." } }
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
    let body = html! {
        (maud::DOCTYPE)
        html lang="en" {
            head { meta charset="utf-8"; meta name="viewport" content="width=device-width, initial-scale=1"; meta name="robots" content="noindex, nofollow"; meta name="description" content="Tools to create. Agents to help. Explore the OpenAgents toolkit and the Verse."; meta name="color-scheme" content="dark"; title { (title) } link rel="canonical" href=(format!("https://openagents.com{path}")); }
            body class=(if verse {"verse-page"} else {"marketing-page"}) { a.hud-skip href=(if verse {"#world-title"} else {"#products"}) { "Skip to content" } (header(verse,account)) (content) @if verse { script type="module" src=(SCRIPT_PATH) {} } }
        }
    }.into_string();
    super::verse_ui::document(body, if verse { "universe" } else { "marketing" })
}

fn respond(content: String) -> Response {
    let mut response = Html(content).into_response();
    let headers = response.headers_mut();
    let engine = universe_release();
    let origin = engine
        .as_ref()
        .map(|r| r.origin())
        .unwrap_or_else(|| "'none'".into());
    let policy = format!(
        "default-src 'none'; style-src 'self'; script-src 'self' 'wasm-unsafe-eval' {origin}; connect-src 'self' {origin}; frame-src {origin}; img-src 'self' data:; font-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
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
    let haunt = if super::new_lander_house::available() {
        Some(super::new_lander_house::HOUSE)
    } else if crate::live_games::tenants().iter().any(|t| t == "haunt") {
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
    let body = if let Some(release) = universe_release() {
        let url = serde_json::to_string(&release.sdk_url).expect("SDK URL");
        format!(
            r#"try{{const sdk=await import({url});await sdk.default();window.universeDemo=sdk.mount_universe();}}catch{{const n=document.getElementById('world-notice');n.dataset.state='error';n.textContent='The 3D view could not open. Reload the map or choose a destination below.';}}"#
        )
    } else {
        "const n=document.getElementById('world-notice');n.dataset.state='error';n.textContent='The 3D view is unavailable here. You can use the destination links below.';".into()
    };
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
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
        for account in [false, true] {
            for page in [
                document(marketing_content(), false, account),
                document(universe_content(None, account, false), true, account),
            ] {
                for old in [
                    "/download",
                    "Get OpenAgents",
                    "Get Coder",
                    "View downloads",
                    "The OpenAgents app",
                ] {
                    assert!(!page.contains(old), "{old}");
                }
                crate::copy_guard::assert_plain(PATH, &page);
            }
        }
        let page = document(marketing_content(), false, true);
        assert!(!page.contains("<script") && !page.contains("<canvas") && !page.contains(".wasm"));
        assert!(page.contains("Enter the Verse") && page.contains("noindex, nofollow"));
        crate::copy_guard::assert_plain(PATH, &page);
        crate::copy_guard::assert_plain(
            VERSE,
            &document(universe_content(None, false, false), true, false),
        );
    }

    /// The Verse page is a shell for the Aethyr controller, and it keeps every
    /// element the pinned engine release needs plus links that work without
    /// JavaScript.
    #[test]
    fn new_lander_verse_is_a_controller_shell_with_the_engine_bridge() {
        let page = document(
            universe_content(Some("/new-lander/house"), true, false),
            true,
            true,
        );
        assert!(page.contains(r#"id="verse-hud""#));
        for id in [
            "engine",
            "world-notice",
            "universe-exploration",
            "verse-entry",
            "verse-account",
            "verse-choices",
            "continue-guest",
            "verse-explore",
            "verse-signin",
            "verse-atlas",
            "world-title",
            "world-category",
            "world-description",
            "planet-ready",
            "planet-owned",
            "planet-create",
            "planet-setup",
            "planet-status",
            "world-retry",
            "world-fullscreen",
        ] {
            if id == "engine" && universe_release().is_none() {
                continue;
            }
            assert!(page.contains(&format!(r#"id="{id}""#)), "{id}");
        }
        for world in aethyrforge_host::WORLDS {
            assert!(page.contains(&format!(r#"id="option-{}""#, world.id)));
            assert!(page.contains(&format!(r#"id="destination-{}""#, world.id)));
        }
        // Without JavaScript, the fallback links still reach each destination.
        assert!(page.contains(r#"<nav class="hud-panel verse-fallback""#));
        assert!(page.contains(r#"href="/new-lander/house">Enter Haunted House</a>"#));

        let start = page
            .find(r#"<script id="verse-config" type="application/json">"#)
            .unwrap();
        let body = &page[start..];
        let json = &body[body.find('>').unwrap() + 1..body.find("</script>").unwrap()];
        let config: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(config["accounts"], true);
        assert_eq!(config["signedIn"], false);
        assert_eq!(config["hauntPath"], "/new-lander/house");
        assert_eq!(config["mapPath"], VERSE);
        assert_eq!(config["blueRushPath"], super::super::blue_rush::PATH);
        let worlds = config["worlds"].as_array().unwrap();
        assert_eq!(worlds.len(), aethyrforge_host::WORLDS.len());
        for (json, world) in worlds.iter().zip(aethyrforge_host::WORLDS.iter()) {
            assert_eq!(json["id"], world.id);
            assert_eq!(json["name"], world.name);
            crate::copy_guard::assert_plain(VERSE, world.description);
        }
        let closed = document(universe_content(None, false, true), true, false);
        assert!(closed.contains(r#""hauntPath":null"#) && closed.contains(r#""signedIn":true"#));
        // Kiki has no chat entry points.
        assert!(!page.contains("/chat"));
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
                assert_eq!(
                    body.contains("Enter Haunted House"),
                    super::super::new_lander_house::available()
                );
            }
        }
        // The self-hosted sans face (so no computer falls back to monospace).
        let font = super::super::verse_ui::FONT_PATH;
        let response = app
            .oneshot(
                Request::builder()
                    .uri(font)
                    .header(header::HOST, "127.0.0.1:4300")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(crate::upstream::owned(font));
        assert_eq!(response.headers()[header::CONTENT_TYPE], "font/woff2");
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(&bytes[..4], b"wOF2");
    }
}
