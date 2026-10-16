//! `/studios/blue-rush`: the Blue Rush Studios site, OpenAgents' game
//! studio (Grow Little Bunny, `docs/verse/games/grow-little-bunny.md`,
//! Snap Station, `docs/verse/games/snap-station.md`, and the games after
//! them), and its pages: the Grow Little Bunny guide at [`GUIDE`] and Snap
//! Station's at [`SNAP_GUIDE`]. The Ice Cream Man is not a Blue Rush game; its old guide
//! address here, [`ICECREAM_GUIDE`], redirects to its guide beside the game.
//!
//! It is its own brand, so it does not use the OpenAgents shell
//! (`UiPage`): the pages link their own stylesheet (and the homepage its
//! script) from `static/bluerush/` and draw their own nav and footer.
//!
//! **Its own domain.** The studio's hosts ([`HOSTS`] by default, more with
//! `--studio-host`) serve the same pages at the root: `/` is the homepage,
//! `/grow-little-bunny/how-to-play` the guide, `/static/...` the studio's
//! files, and `/games/grow-little-bunny` the game ([`studio_path`]
//! maps each request before routing; anything else is `404` there). The
//! pages drawn for those hosts link the root paths and send the legal links
//! to openagents.com ([`Site`]).
//!
//! The homepage's policy is the site's plus `script-src 'self'` for the one
//! script, which draws the water and crabs behind the game cards and runs
//! the sand garden at the bottom. Without the script the page still reads
//! in full; only the two canvases stay still. The guide has no script.

use std::sync::LazyLock;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};
use sha2::{Digest, Sha256};

use crate::App;

/// The homepage.
pub(crate) const PATH: &str = "/studios/blue-rush";

/// The Grow Little Bunny guide. `bunny-web` links it from the game's How
/// to play cards (`tutorial::GUIDE`), so the path stays put.
pub(crate) const GUIDE: &str = "/studios/blue-rush/grow-little-bunny/how-to-play";

/// Where The Ice Cream Man's guide was while it was listed here. The game
/// has no studio now; this address redirects to its guide beside the game
/// ([`super::icecream_guide::ICECREAM_GUIDE_PATH`]), on openagents.com.
pub(crate) const ICECREAM_GUIDE: &str = "/studios/blue-rush/the-ice-cream-man/how-to-play";

/// The Snap Station guide. `snap-web` links it from its tutorial and menu
/// (`tutorial::GUIDE`), so the path stays put.
pub(crate) const SNAP_GUIDE: &str = "/studios/blue-rush/snap-station/how-to-play";

/// Where the pages' stylesheet, script, and pictures are served.
pub(crate) const ASSETS: &str = "/studios/blue-rush/static/";

/// The studio's own domain, answered without any flag. `--studio-host`
/// adds more (`Config::studio_hosts`).
pub const HOSTS: [&str; 2] = ["bluerushstudios.com", "www.bluerushstudios.com"];

/// Set on a request that came on a studio host, so its pages link the root
/// paths; a copy the browser sends is removed first (`crate::router`).
pub(crate) const STUDIO_HEADER: HeaderName = HeaderName::from_static("x-openagents-studio");

/// The homepage's policy: the site's, plus scripts from this site.
pub(crate) const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The guide's policy: the site's, with no script at all.
pub(crate) const GUIDE_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Where the hero's button and the featured card lead.
pub(crate) const BUNNY: &str = "/games/grow-little-bunny";

/// Snap Station's game.
pub(crate) const SNAP: &str = "/games/snap-station";

const CSS: &str = include_str!("../../static/bluerush/bluerush.css");
const JS: &str = include_str!("../../static/bluerush/bluerush.js");

/// The coast stills, from `bench/verse/2026-10-08/coast-c1` and `coast-c2`,
/// downscaled to JPEG, and the guide's screenshots of the real game
/// (`bunny-*.jpg`, 960 by 600), and Snap Station's (`snap-*.jpg`, 960 by
/// 600).
const IMAGES: [(&str, &[u8]); 20] = [
    (
        "snap-tour.jpg",
        include_bytes!("../../static/bluerush/snap-tour.jpg"),
    ),
    (
        "snap-jobs.jpg",
        include_bytes!("../../static/bluerush/snap-jobs.jpg"),
    ),
    (
        "snap-ask.jpg",
        include_bytes!("../../static/bluerush/snap-ask.jpg"),
    ),
    (
        "snap-drag.jpg",
        include_bytes!("../../static/bluerush/snap-drag.jpg"),
    ),
    (
        "snap-done.jpg",
        include_bytes!("../../static/bluerush/snap-done.jpg"),
    ),
    (
        "snap-storm.jpg",
        include_bytes!("../../static/bluerush/snap-storm.jpg"),
    ),
    (
        "snap-spin.jpg",
        include_bytes!("../../static/bluerush/snap-spin.jpg"),
    ),
    (
        "snap-dock.jpg",
        include_bytes!("../../static/bluerush/snap-dock.jpg"),
    ),
    (
        "hero-bay.jpg",
        include_bytes!("../../static/bluerush/hero-bay.jpg"),
    ),
    (
        "lighthouse.jpg",
        include_bytes!("../../static/bluerush/lighthouse.jpg"),
    ),
    (
        "harbor.jpg",
        include_bytes!("../../static/bluerush/harbor.jpg"),
    ),
    (
        "sea-arch.jpg",
        include_bytes!("../../static/bluerush/sea-arch.jpg"),
    ),
    (
        "bunny-tour.jpg",
        include_bytes!("../../static/bluerush/bunny-tour.jpg"),
    ),
    (
        "bunny-hole.jpg",
        include_bytes!("../../static/bluerush/bunny-hole.jpg"),
    ),
    (
        "bunny-run.jpg",
        include_bytes!("../../static/bluerush/bunny-run.jpg"),
    ),
    (
        "bunny-giant.jpg",
        include_bytes!("../../static/bluerush/bunny-giant.jpg"),
    ),
    (
        "bunny-farmer.jpg",
        include_bytes!("../../static/bluerush/bunny-farmer.jpg"),
    ),
    (
        "bunny-cleared.jpg",
        include_bytes!("../../static/bluerush/bunny-cleared.jpg"),
    ),
    (
        "bunny-burrow.jpg",
        include_bytes!("../../static/bluerush/bunny-burrow.jpg"),
    ),
    (
        "bunny-hop.jpg",
        include_bytes!("../../static/bluerush/bunny-hop.jpg"),
    ),
];

/// A versioned file: its link carries `?v=` and the first bytes of its
/// SHA-256, so it can be cached for a year.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// The pictures change rarely and carry no version.
const PICTURE_CACHE: &str = "public, max-age=86400";

fn version(body: &str) -> String {
    Sha256::digest(body.as_bytes())[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The guide stylesheet, which The Ice Cream Man's guide shares.
pub(crate) const GUIDE_CSS: &str = CSS;

static CSS_VERSION: LazyLock<String> = LazyLock::new(|| version(CSS));

/// The stylesheet's version, for a link that can be cached for a year.
pub(crate) fn css_version() -> &'static str {
    &CSS_VERSION
}
static JS_VERSION: LazyLock<String> = LazyLock::new(|| version(JS));

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(page))
        .route(GUIDE, get(guide))
        .route(ICECREAM_GUIDE, get(icecream_moved))
        .route(SNAP_GUIDE, get(snap_guide))
        .route("/studios/blue-rush/static/{file}", get(asset))
}

/// The path a request on a studio host is answered at. The studio's own
/// paths, the game and its files, and the files the game's page loads
/// stay as they are; `/` is the homepage; `/static/FILE` is the studio's
/// file; anything else is looked for under [`PATH`], so
/// `/grow-little-bunny/how-to-play` is the guide and an unknown path is a
/// `404` rather than another part of openagents.com.
pub(crate) fn studio_path(path: &str) -> String {
    let kept = path == PATH
        || path.starts_with("/studios/blue-rush/")
        || path == BUNNY
        || path.starts_with("/games/grow-little-bunny/")
        || path == SNAP
        || path.starts_with("/games/snap-station/")
        || matches!(
            path,
            "/favicon.svg" | "/favicon.ico" | "/static/legacy-demo.css" | "/health"
        );
    if kept {
        path.to_owned()
    } else if path == "/" || path.is_empty() {
        PATH.to_owned()
    } else if let Some(file) = path.strip_prefix("/static/") {
        format!("{ASSETS}{file}")
    } else {
        format!("{PATH}{path}")
    }
}

/// Where a page's links lead: openagents.com's paths, or the studio
/// domain's root paths.
struct Site {
    home: &'static str,
    guide: &'static str,
    snap_guide: &'static str,
    assets: &'static str,
    /// Prefixed to the links to the rest of openagents.com.
    openagents: &'static str,
}

const ON_OPENAGENTS: Site = Site {
    home: PATH,
    guide: GUIDE,
    snap_guide: SNAP_GUIDE,
    assets: ASSETS,
    openagents: "",
};

const ON_STUDIO: Site = Site {
    home: "/",
    guide: "/grow-little-bunny/how-to-play",
    snap_guide: "/snap-station/how-to-play",
    assets: "/static/",
    openagents: "https://openagents.com",
};

fn site(headers: &HeaderMap) -> &'static Site {
    if headers.contains_key(STUDIO_HEADER) {
        &ON_STUDIO
    } else {
        &ON_OPENAGENTS
    }
}

async fn asset(State(app): State<App>, Path(file): Path<String>, request: HeaderMap) -> Response {
    if matches!(file.as_str(), "sand_garden.js" | "sand_garden_bg.wasm") {
        let kind = if file.ends_with(".wasm") {
            "application/wasm"
        } else {
            "text/javascript; charset=utf-8"
        };
        let mut response =
            super::everglade::serve_build(&app.config.sand_build, &file, kind, &request).await;
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        return response;
    }
    let (content_type, cache, body): (&str, &str, &'static [u8]) = match file.as_str() {
        "bluerush.css" => ("text/css; charset=utf-8", IMMUTABLE, CSS.as_bytes()),
        "bluerush.js" => ("text/javascript; charset=utf-8", IMMUTABLE, JS.as_bytes()),
        name => match IMAGES.iter().find(|(image, _)| *image == name) {
            Some((_, bytes)) => ("image/jpeg", PICTURE_CACHE, bytes),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache),
        ],
        body,
    )
        .into_response()
}

fn with_policy(markup: Markup, policy: &'static str) -> Response {
    let mut response = markup.into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(policy),
    );
    response
}

async fn page(headers: HeaderMap) -> Response {
    with_policy(render(site(&headers)), POLICY)
}

async fn guide(headers: HeaderMap) -> Response {
    with_policy(render_guide(site(&headers)), GUIDE_POLICY)
}

/// The Ice Cream Man's guide moved beside the game: a permanent redirect,
/// to openagents.com from the studio's own domain.
async fn snap_guide(headers: HeaderMap) -> Response {
    with_policy(render_snap_guide(site(&headers)), GUIDE_POLICY)
}

async fn icecream_moved(headers: HeaderMap) -> Response {
    let to = if headers.contains_key(STUDIO_HEADER) {
        format!(
            "https://openagents.com{}",
            super::icecream_guide::ICECREAM_GUIDE_PATH
        )
    } else {
        super::icecream_guide::ICECREAM_GUIDE_PATH.to_owned()
    };
    axum::response::Redirect::permanent(&to).into_response()
}

/// A 5x7 pixel font for the wordmark and the headline: each glyph is seven
/// rows of five columns, `#` for a block.
#[rustfmt::skip]
fn glyph(letter: char) -> [&'static str; 7] {
    match letter {
        'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'B' => ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
        'C' => [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."],
        'D' => ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
        'E' => ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
        'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
        'G' => [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"],
        'H' => ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'I' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"],
        'J' => ["..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."],
        'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
        'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
        'M' => ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"],
        'N' => ["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"],
        'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
        'Q' => [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
        'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
        'S' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
        'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
        'U' => ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'V' => ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
        'W' => ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."],
        'X' => ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
        'Y' => ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
        'Z' => ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
        _ => [".....", ".....", ".....", ".....", ".....", ".....", "....."],
    }
}

/// One word in the pixel font, as an SVG of blocks with a darker copy
/// behind for depth. Screen readers read the text next to it instead.
fn pixel_word(word: &str) -> Markup {
    let mut path = String::new();
    let mut columns = 0;
    for (index, letter) in word.chars().enumerate() {
        let left = index * 6;
        for (y, row) in glyph(letter.to_ascii_uppercase()).iter().enumerate() {
            for (x, cell) in row.chars().enumerate() {
                if cell == '#' {
                    path.push_str(&format!("M{} {}h1v1h-1z", left + x, y));
                }
            }
        }
        columns = left + 5;
    }
    let view_box = format!("0 0 {}.6 7.6", columns);
    html! {
        svg class="br-pixel" viewBox=(view_box) aria-hidden="true" focusable="false" {
            path class="br-pixel-depth" transform="translate(0.6 0.6)" d=(path) {}
            path class="br-pixel-face" d=(path) {}
        }
    }
}

/// A line of words in the pixel font, each word its own SVG so the line
/// wraps between words.
fn pixel_text(text: &str) -> Markup {
    html! {
        span class="br-sr" { (text) }
        @for word in text.split_whitespace() {
            (pixel_word(word))
        }
    }
}

struct Game {
    title: &'static str,
    image: &'static str,
    alt: &'static str,
    blurb: &'static str,
}

const LAB: [Game; 2] = [
    Game {
        title: "Harbor Builders",
        image: "harbor.jpg",
        alt: "A sailboat beside a wooden pier on blue water",
        blurb: "Design a floating town. Test it against wind and waves, then keep it afloat.",
    },
    Game {
        title: "Reef Keepers",
        image: "sea-arch.jpg",
        alt: "Dark rocks forming a sea arch in deep blue water",
        blurb: "Dive under the sea arch and bring a reef back to life, one species at a time.",
    },
];

/// A page's `<head>`: its title and description, the stylesheet, and the
/// script when the page has one.
fn head(site: &Site, title: &str, description: &str, script: bool) -> Markup {
    let css = format!("{}bluerush.css?v={}", site.assets, *CSS_VERSION);
    let js = format!("{}bluerush.js?v={}", site.assets, *JS_VERSION);
    html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { (title) }
            meta name="description" content=(description);
            meta name="theme-color" content="#000930";
            link rel="stylesheet" href=(css);
            @if script {
                script src=(js) defer {}
            }
        }
    }
}

/// The nav: the wordmark home, the page's own links, and Play now
/// (`play`, the page's game).
fn nav(site: &Site, links: &[(&str, &str)], play: &str) -> Markup {
    html! {
        a class="br-skip" href="#main" { "Skip to content" }
        header class="br-nav" {
            a class="br-wordmark" href=(site.home) aria-label="Blue Rush Studios home" {
                (pixel_word("BLUE"))
                (pixel_word("RUSH"))
            }
            nav class="br-links" aria-label="Main" {
                @for (label, href) in links {
                    a href=(href) { (label) }
                }
            }
            a class="br-outline" href=(play) { "Play now" }
        }
    }
}

fn footer(site: &Site) -> Markup {
    html! {
        footer class="br-foot" {
            div class="br-wrap br-foot-row" {
                div {
                    p class="br-foot-name" { "Blue Rush Studios" }
                    p { a href={(site.openagents) "/"} { "An OpenAgents studio" } }
                }
                nav class="br-foot-links" aria-label="Legal" {
                    a href={(site.openagents) "/terms"} { "Terms" }
                    a href={(site.openagents) "/privacy"} { "Privacy" }
                }
            }
        }
    }
}

fn render(site: &Site) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            (head(site, "Blue Rush Studios", "Learning games about living things, the ocean, and building new homes on the sea.", true))
            body class="br" {
                (nav(site, &[("Games", "#games"), ("How to play", site.guide), ("Learn", "#learn"), ("Sand garden", "#garden")], BUNNY))
                main id="main" {
                    section class="br-hero" {
                        img class="br-hero-img" src={(site.assets) "hero-bay.jpg"} alt="" width="960" height="540";
                        div class="br-hero-text" {
                            h1 class="br-headline" { (pixel_text("Play the future")) }
                            p class="br-pitch" {
                                "Learning games about living things, the ocean, and building new homes on the sea."
                            }
                            div class="br-hero-actions" {
                                a class="br-cta" href=(BUNNY) { "Play Grow Little Bunny" }
                                a class="br-hero-link" href=(site.guide) { "How to play" }
                            }
                        }
                    }
                    section class="br-games" id="games" aria-labelledby="games-title" {
                        canvas class="br-water" id="br-water" aria-hidden="true" {}
                        div class="br-wrap" {
                            h2 class="br-h2" id="games-title" { "Our games" }
                            div class="br-cards" {
                                article class="br-card br-card-featured" {
                                    a class="br-card-media" href=(BUNNY) tabindex="-1" aria-hidden="true" {
                                        img src={(site.assets) "lighthouse.jpg"} alt="" width="960" height="540" loading="lazy";
                                    }
                                    div class="br-card-body" {
                                        span class="br-tag br-tag-hot" { "Featured" }
                                        h3 { "Grow Little Bunny" }
                                        p { "Hop through a farmer's garden, eat everything you can, and grow from a tiny white bunny into a big orange one. Watch out for the net." }
                                        div class="br-card-actions" {
                                            a class="br-card-go" href=(BUNNY) { "Play" }
                                            a class="br-card-link" href=(site.guide) { "How to play" }
                                        }
                                    }
                                }
                                article class="br-card br-card-snap" {
                                    a class="br-card-media" href=(SNAP) tabindex="-1" aria-hidden="true" {
                                        img src={(site.assets) "snap-spin.jpg"} alt="" width="960" height="600" loading="lazy";
                                    }
                                    div class="br-card-body" {
                                        span class="br-tag br-tag-new" { "New" }
                                        h3 { "Snap Station" }
                                        p { "Snap magnetic pieces into stations, ships and docks in space, then press Go and see if they hold. Jobs come from a friendly robot crew. For builders about 5 to 12." }
                                        div class="br-card-actions" {
                                            a class="br-card-go" href=(SNAP) { "Play" }
                                            a class="br-card-link" href=(site.snap_guide) { "How to play" }
                                        }
                                    }
                                }
                                @for game in &LAB {
                                    article class="br-card" {
                                        img src={(site.assets) (game.image)} alt=(game.alt) width="960" height="540" loading="lazy";
                                        div class="br-card-body" {
                                            span class="br-tag" { "In the lab" }
                                            h3 { (game.title) }
                                            p { (game.blurb) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    section class="br-learn" id="learn" aria-labelledby="learn-title" {
                        div class="br-wrap" {
                            h2 class="br-h2" id="learn-title" { "For teachers and homeschoolers" }
                            ul class="br-points" {
                                li {
                                    strong { "Life science" }
                                    span { "How living things eat, grow, and change." }
                                }
                                li {
                                    strong { "The ocean" }
                                    span { "How tides, reefs, and sea life fit together." }
                                }
                                li {
                                    strong { "Engineering" }
                                    span { "How to build things that float, hold, and last." }
                                }
                            }
                        }
                    }
                    section class="br-garden" id="garden" aria-labelledby="garden-title" {
                        div class="br-wrap" {
                            h2 class="br-h2" id="garden-title" { "Sand garden" }
                            p class="br-lede" { "Shape the sand. Gather little treasures. Leave your own mark." }
                            div class="br-garden-frame" {
                                div class="br-garden-top" {
                                    span class="br-garden-caption" { "Your little stretch of beach" }
                                    span class="br-garden-caption" { "Blue Rush · Low tide" }
                                }
                                canvas class="br-sand" id="br-sand" tabindex="0" role="application" aria-roledescription="sand garden"
                                    aria-label="Sand garden. Choose a tool below. Drag to draw or tap to place a beach find. With a keyboard, Space starts or stops a mark, and arrow keys move it." {}
                                div class="br-garden-tools" id="br-garden-tools" hidden {
                                    div class="br-tool-tray" role="group" aria-label="Sand tools and beach finds" {
                                        @for (tool, label, icon) in [("rake", "Rake", "≋"), ("finger", "Finger", "〰"), ("smooth", "Smooth", "◯"), ("conch", "Conch", ""), ("stone", "Stone", ""), ("glass", "Sea glass", ""), ("tooth", "Shark tooth", "")] {
                                            button class="br-tool br-material" type="button" id=(format!("br-tool-{tool}")) aria-pressed=(if tool == "rake" { "true" } else { "false" }) {
                                                @if !icon.is_empty() { span class="br-tool-symbol" aria-hidden="true" { (icon) } }
                                                span { (label) }
                                            }
                                        }
                                    }
                                    div class="br-tool-options" {
                                        label for="br-size" { "Size" input id="br-size" type="range" min="0.6" max="1.8" step="0.1" value="1"; }
                                        button class="br-tool" type="button" id="br-rotate" { "Turn find ↻" }
                                        button class="br-tool" type="button" id="br-mandala" aria-pressed="false" { "Mandala" }
                                        button class="br-tool" type="button" id="br-undo" disabled { "Undo" }
                                        button class="br-tool" type="button" id="br-redo" disabled { "Redo" }
                                        button class="br-tool" type="button" id="br-clear" { "Clear" }
                                    }
                                    div class="br-garden-bottom" {
                                        p id="br-status" role="status" { "Loading your beach…" }
                                        div class="br-garden-actions" {
                                            button class="br-tool" type="button" id="br-download" { "Save image" }
                                            button class="br-tool" type="button" id="br-gallery-open" { "Gallery" }
                                            button class="br-tool br-share" type="button" id="br-share-open" { "Share creation ↑" }
                                        }
                                    }
                                }
                                p class="br-keys" id="br-load-status" role="status" { "Loading your beach…" }
                                noscript { p class="br-keys" { "Turn on JavaScript to shape the sand." } }
                                dialog class="br-garden-card br-gallery" id="br-gallery" aria-labelledby="br-gallery-title" {
                                    div class="br-card-heading" {
                                        div { span class="br-eyebrow" { "Made at low tide" } h3 id="br-gallery-title" { "Little stretches of beach" } }
                                        button class="br-tool" type="button" id="br-gallery-close" aria-label="Close gallery" { "Close ×" }
                                    }
                                    p id="br-gallery-status" role="status" {}
                                    div class="br-gallery-grid" id="br-gallery-grid" {}
                                    div class="br-gallery-pagination" {
                                        button class="br-tool" type="button" id="br-gallery-prev" { "Previous" }
                                        span id="br-gallery-page" {}
                                        button class="br-tool" type="button" id="br-gallery-next" { "Next" }
                                        button class="br-tool" type="button" id="br-gallery-retry" { "Refresh" }
                                    }
                                    a class="br-manage" href="https://openagents.com/login?return_to=%2Fstudios%2Fblue-rush%23garden" target="_blank" rel="noopener" { "Studio sign-in" }
                                }
                                dialog class="br-garden-card br-share-card" id="br-share-card" aria-labelledby="br-share-title" {
                                    div class="br-card-heading" { h3 id="br-share-title" { "Share your beach" } button class="br-tool" type="button" id="br-share-close" aria-label="Close sharing" { "Close ×" } }
                                    canvas id="br-share-preview" class="br-preview" role="img" aria-label="Your sand design" {}
                                    label for="br-title" { "Name your creation" }
                                    input id="br-title" type="text" maxlength="48" placeholder="A pocketful of sea glass" autocomplete="off";
                                    p { "Everyone can see and upvote it. You can delete it from this browser." }
                                    p id="br-share-status" role="status" {}
                                    button class="br-tool br-share" type="button" id="br-publish" { "Upload to gallery ↑" }
                                }
                                dialog class="br-garden-card br-clear-card" id="br-clear-card" aria-labelledby="br-clear-title" {
                                    h3 id="br-clear-title" { "Start with fresh sand?" }
                                    p { "This clears your beach here. Your shared creations stay in the gallery." }
                                    button class="br-tool" type="button" id="br-clear-cancel" { "Keep drawing" }
                                    button class="br-tool" type="button" id="br-clear-confirm" { "Clear the sand" }
                                }
                            }
                            p class="br-keys" { "Drag to shape · Tap to place · Space and arrows work, too" }
                        }
                    }
                }
                (footer(site))
            }
        }
    }
}

/// A screenshot from the game, with its caption.
fn shot(site: &Site, file: &str, alt: &str, caption: &str) -> Markup {
    html! {
        figure class="br-shot" {
            img src={(site.assets) (file)} alt=(alt) width="960" height="600" loading="lazy";
            figcaption { (caption) }
        }
    }
}

/// One key, drawn as a key cap.
pub(crate) fn key(name: &str) -> Markup {
    html! { kbd { (name) } }
}

/// The guide's sections: their anchors and titles, for the contents.
const SECTIONS: [(&str, &str); 10] = [
    ("goal", "The goal"),
    ("meadow", "Start in the meadow"),
    ("burrow", "Inside the Burrow"),
    ("controls", "Controls"),
    ("jump", "Jump high"),
    ("run", "Run through a garden"),
    ("grow", "Eat and grow"),
    ("farmer", "Watch for the farmer"),
    ("win", "Clear the garden"),
    ("tips", "Tips"),
];

fn section_title(sections: &[(&str, &'static str)], id: &str) -> &'static str {
    sections
        .iter()
        .find(|(anchor, _)| *anchor == id)
        .map_or("", |(_, title)| title)
}

fn step(id: &str, number: usize, body: Markup, picture: Option<Markup>) -> Markup {
    step_in(&SECTIONS, id, number, body, picture)
}

pub(crate) fn step_in(
    sections: &[(&str, &'static str)],
    id: &str,
    number: usize,
    body: Markup,
    picture: Option<Markup>,
) -> Markup {
    let heading = format!("{id}-title");
    html! {
        section class=(if picture.is_some() { "br-step" } else { "br-step br-step-solo" }) id=(id) aria-labelledby=(heading) {
            div class="br-step-text" {
                h2 id=(heading) {
                    span class="br-step-num" { (number) }
                    (section_title(sections, id))
                }
                (body)
            }
            @if let Some(picture) = picture {
                (picture)
            }
        }
    }
}

fn render_guide(site: &Site) -> Markup {
    let control_rows: [(&str, Markup, Markup); 9] = [
        (
            "Walk in the meadow",
            html! { (key("\u{2190}")) " " (key("\u{2191}")) " " (key("\u{2192}")) " " (key("\u{2193}")) " or " (key("W")) " " (key("A")) " " (key("S")) " " (key("D")) },
            html! { "Drag anywhere" },
        ),
        (
            "Hop in the meadow",
            html! { (key("Space")) ", held to hop higher" },
            html! { "Hold the " (key("\u{2191}")) " button" },
        ),
        (
            "Dodge, and turn at corners",
            html! { (key("\u{2190}")) " " (key("\u{2192}")) " or " (key("A")) " " (key("D")) },
            html! { "Swipe left or right" },
        ),
        (
            "Jump (hold to go higher)",
            html! { (key("\u{2191}")) ", " (key("W")) " or " (key("Space")) },
            html! { "Swipe up, or hold the " (key("\u{2191}")) " button" },
        ),
        (
            "Duck",
            html! { (key("\u{2193}")) " or " (key("S")) },
            html! { "Swipe down" },
        ),
        (
            "Turn around",
            html! { (key("X")) " or " (key("Backspace")) },
            html! { "The " (key("\u{21b6}")) " button" },
        ),
        (
            "Pause",
            html! { (key("Esc")) " or " (key("P")) },
            html! { "The " (key("II")) " button" },
        ),
        (
            "Start the garden again",
            html! { (key("R")) },
            html! { "Pause, then Restart" },
        ),
        (
            "Open the tutorial",
            html! { (key("?")) },
            html! { "The " (key("?")) " button, or How to play" },
        ),
    ];
    html! {
        (DOCTYPE)
        html lang="en" {
            (head(site, "How to play Grow Little Bunny \u{b7} Blue Rush Studios", "How to play Grow Little Bunny: the controls, how your bunny grows, how to dodge the farmer, and how to clear a garden.", false))
            body class="br" {
                (nav(site, &[("Games", &format!("{}#games", site.home)), ("How to play", site.guide)], BUNNY))
                main id="main" {
                    header class="br-guide-head" {
                        div class="br-wrap" {
                            p class="br-crumbs" {
                                a href=(site.home) { "Blue Rush Studios" }
                                " / Grow Little Bunny"
                            }
                            h1 class="br-guide-title" { "How to play Grow Little Bunny" }
                            p class="br-guide-lede" {
                                "You are a tiny white bunny in a farmer's garden. Eat everything, grow big, and don't get caught in his net. Here's everything you need to know, with pictures from the game."
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(BUNNY) { "Play Grow Little Bunny" }
                            }
                            nav class="br-toc" aria-label="On this page" {
                                @for (anchor, title) in &SECTIONS {
                                    a href={"#" (anchor)} { (title) }
                                }
                            }
                        }
                    }
                    div class="br-guide br-wrap" {
                        (step("goal", 1, html! {
                            p { "Every garden is a little maze of rows, full of food. You win a garden by eating " strong { "all" } " of it. There's no clock: take your time, but keep moving." }
                            p { "The only way to lose is to get caught by the farmer's net. Every garden you clear turns your bunny one shade more orange, from snow white all the way to bright orange." }
                        }, Some(shot(site, "bunny-cleared.jpg", "The Garden cleared card, with the time, the score, and buttons for the next garden", "Clear a garden to see your time and score."))))
                        (step("meadow", 2, html! {
                            p { "Press " strong { "Play" } " to go to Warren Meadow, your bunny's home. Walk around with the arrow keys or " (key("W")) " " (key("A")) " " (key("S")) " " (key("D")) ", and press " (key("Space")) " to hop. On a phone or tablet, drag anywhere on the screen, and use the " (key("\u{2191}")) " button to hop." }
                            p { "The rabbit holes are to the right of the big mound. Walk into a hole to go into its garden. Start with " strong { "1. Kitchen Bed" } ": it's the easiest." }
                            ul class="br-list" {
                                li { strong { "The Burrow" } " is inside the big mound. Walk into one of its dark doors to go in." }
                                li { strong { "The Carrot Board" } ": your best time and score in each garden." }
                                li { strong { "The arch" } ": the way out of the game." }
                            }
                        }, Some(shot(site, "bunny-hole.jpg", "The white bunny in the meadow next to a rabbit hole with a signpost", "Walk into a rabbit hole. The sign says which garden it is."))))
                        (step("burrow", 3, html! {
                            p { "The big mound in the middle of the meadow has four dark doors, one on each side. Walk straight into any of them and your bunny dives inside." }
                            p { "Inside the Burrow you can see your wins and your bunny's colour, and:" }
                            ul class="br-list" {
                                li { "Take a " strong { "tunnel" } " straight to any garden." }
                                li { "Turn on " strong { "Gentle mode" } " to make the farmer slower, or " strong { "High contrast" } " to make things easier to see." }
                                li { "Open " strong { "How to play" } " again." }
                            }
                            p { "Press " strong { "Go back outside" } " (or " (key("Esc")) ") to come back out." }
                        }, Some(shot(site, "bunny-burrow.jpg", "Inside the Burrow: the bunny's wins and colour, tunnels to the five gardens, and Go back outside", "Inside the Burrow."))))
                        (step("controls", 4, html! {
                            p { "In a garden your bunny runs by itself. You only steer." }
                            div class="br-table-wrap" {
                                table class="br-keys-table" {
                                    thead {
                                        tr {
                                            th scope="col" { "To" }
                                            th scope="col" { "Keyboard" }
                                            th scope="col" { "Phone or tablet" }
                                        }
                                    }
                                    tbody {
                                        @for (action, keys, touch) in &control_rows {
                                            tr {
                                                th scope="row" { (action) }
                                                td { (keys) }
                                                td { (touch) }
                                            }
                                        }
                                    }
                                }
                            }
                            p { "The first time you play, a short tutorial shows these controls. Open it again any time with the " (key("?")) " button in the meadow, or " strong { "How to play" } " on the pause card or inside the Burrow." }
                        }, Some(shot(site, "bunny-tour.jpg", "The game's tutorial card, Run and dodge, step 3 of 6", "The tutorial in the game."))))
                        (step("jump", 5, html! {
                            p { "The longer you hold the jump key, the higher your bunny goes. A quick tap is a small hop; hold it and your bunny keeps rising, up to a hop about four times as high. Let go and it hangs for a moment, then drops back down." }
                            p { "It works in the meadow with " (key("Space")) " and in a garden with " (key("\u{2191}")) ", " (key("W")) " or " (key("Space")) ". On a phone or tablet, hold the " (key("\u{2191}")) " button in the corner. A swipe up is a quick hop." }
                            p { "In a garden, a long jump stays in the air longer, so it can carry you over things a short one can't. It never takes you over the hedges." }
                        }, Some(shot(site, "bunny-hop.jpg", "The bunny high in the air over the meadow after holding Space", "Hold Space for a big hop."))))
                        (step("run", 6, html! {
                            p { "Each row is wide enough for three bunnies. Move left or right to eat the food on each side. When you reach a corner, press left or right to turn. If you don't, your bunny stops and waits, and the farmer gets closer." }
                            p { "Around the edges of the screen:" }
                            ul class="br-list" {
                                li { strong { "Top left" } ": how much food is left, with a ring that fills up as you eat, and your size." }
                                li { strong { "Top right" } ": your score. Eat three things quickly and a multiplier like " strong { "\u{d7}2" } " shows up." }
                                li { strong { "Bottom left" } ": the map. Orange dots are food you haven't eaten yet." }
                            }
                        }, Some(shot(site, "bunny-run.jpg", "A small white bunny running down a garden row toward green seedlings and a carrot, with the food count, score, and map on screen", "Kitchen Bed: 148 bites left, and the map shows where they are."))))
                        (step("grow", 7, html! {
                            p { "Everything you can eat is in colour: green seedlings, orange carrots, red radishes, strawberries, and lettuce. Each bite helps you grow, through five sizes:" }
                            ol class="br-sizes" {
                                li { "Kit" }
                                li { "Bunny" }
                                li { "Jack" }
                                li { "Big Bun" }
                                li { "Giant" }
                            }
                            p { "Size changes what you can do:" }
                            ul class="br-list" {
                                li { strong { "Flower pots" } " trip up a small bunny. From Jack up, you smash right through them." }
                                li { strong { "Garden hoses" } ": jump over them while you're small. Bigger bunnies just step over." }
                                li { strong { "Bird nets" } " hang across the row: duck under them." }
                                li { strong { "Fence gaps" } " only fit a small bunny, so eat what's behind them early." }
                                li { strong { "Lettuce" } " is too big for a small bunny. Come back for it once you're a Jack." }
                            }
                            p { "Bumping into something makes you tumble: you shrink a little and slow down for a moment." }
                        }, Some(shot(site, "bunny-giant.jpg", "A Giant bunny, five size dots full, running beside the farmer", "A Giant: all five size dots are full."))))
                        (step("farmer", 8, html! {
                            p { "The farmer walks his garden with a big net. When he spots you, a " strong { "!" } " appears over his head and he runs after you. A " strong { "?" } " means he lost you and is looking around." }
                            p { "When he's out of sight, an arrow at the edge of the screen points to him. If his net lands on you, you're caught and the garden is over." }
                            p { "Each garden has a few " strong { "golden carrots" } ". Eat one and the farmer gets scared for a few seconds. Bump into him then and he gets dizzy and goes back to his shed, and you score bonus points." }
                        }, Some(shot(site, "bunny-farmer.jpg", "The farmer with his net at the end of a garden row, with an exclamation mark over his head", "The farmer has seen you. Turn around, or find a golden carrot."))))
                        (step("win", 9, html! {
                            p { "Eat the very last bite and the garden is cleared. You'll see your time and score, then you can go on to the next garden, play again for a better time, or go back to the meadow." }
                            p { "There are five gardens so far, each a little harder: Kitchen Bed, Herb Corner, Potting Row, Pea Trellis, and Kitchen Garden. Each win makes your bunny one shade more orange, up to 20 shades." }
                            p { "Leaving a garden from the pause card doesn't count as a win or a loss." }
                        }, None))
                        (step("tips", 10, html! {
                            ul class="br-list" {
                                li { "Follow the rows of seedlings. They lead you through the whole garden." }
                                li { "Look at the map when only a few bites are left." }
                                li { "Farmer ahead? Turn around with " (key("X")) " or the " (key("\u{21b6}")) " button." }
                                li { "Save a golden carrot for when the farmer is close." }
                                li { "Playing with a young child? Turn on Gentle mode in the Burrow." }
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(BUNNY) { "Play now" }
                                a class="br-hero-link" href=(site.home) { "Back to Blue Rush Studios" }
                            }
                        }, None))
                    }
                }
                (footer(site))
            }
        }
    }
}

/// The Snap Station guide's sections: their anchors and titles.
const SNAP_SECTIONS: [(&str, &str); 10] = [
    ("goal", "The goal"),
    ("crew", "The robot crew"),
    ("controls", "Controls"),
    ("snap", "Snap pieces on"),
    ("pieces", "The pieces"),
    ("go", "Press Go"),
    ("strong", "Make it strong"),
    ("jobs", "The jobs"),
    ("stars", "Stars"),
    ("tips", "Tips"),
];

fn render_snap_guide(site: &Site) -> Markup {
    let step = |id, number, body, picture| step_in(&SNAP_SECTIONS, id, number, body, picture);
    let control_rows: [(&str, Markup, Markup); 8] = [
        (
            "Pick up a piece",
            html! { "Press it in the tray at the bottom" },
            html! { "Touch it in the tray" },
        ),
        (
            "Snap it on",
            html! { "Drag it next to the station and let go" },
            html! { "Drag it next to the station and lift your finger, or tap it in the tray and then tap where it goes" },
        ),
        (
            "Turn a piece",
            html! { (key("R")) " or the mouse wheel while you hold it" },
            html! { "Tap " strong { "Turn" } },
        ),
        (
            "Take a piece back",
            html! { "Drag it off the station, or press " strong { "Undo" } },
            html! { "Drag it off the station, or tap " strong { "Undo" } },
        ),
        (
            "Test your build",
            html! { strong { "Go" } ", or " (key("G")) },
            html! { "Tap " strong { "Go" } },
        ),
        (
            "Build with the keyboard",
            html! { (key("\u{2190}")) " " (key("\u{2192}")) " pick a piece, " (key("Enter")) " picks it up, the arrows move it, " (key("Enter")) " snaps it on, " (key("X")) " takes the last piece back" },
            html! { "Not needed" },
        ),
        (
            "Menu",
            html! { (key("Esc")) },
            html! { "The \u{2630} button" },
        ),
        (
            "Open the tutorial",
            html! { (key("?")) ", or the " strong { "?" } " button" },
            html! { "The " strong { "?" } " button" },
        ),
    ];
    html! {
        (DOCTYPE)
        html lang="en" {
            (head(site, "How to play Snap Station \u{b7} Blue Rush Studios", "How to play Snap Station: snap magnetic pieces into stations, ships and docks in space, press Go, and do the robot crew's jobs.", false))
            body class="br" {
                (nav(site, &[("Games", &format!("{}#games", site.home)), ("How to play", site.snap_guide)], SNAP))
                main id="main" {
                    header class="br-guide-head" {
                        div class="br-wrap" {
                            p class="br-crumbs" {
                                a href=(site.home) { "Blue Rush Studios" }
                                " / Snap Station"
                            }
                            h1 class="br-guide-title" { "How to play Snap Station" }
                            p class="br-guide-lede" {
                                "We made Snap Station for builders about 5 to 12. You snap colourful magnetic pieces together into stations, ships and docks in space, then press Go and watch what happens. Here's how, with pictures from the game."
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(SNAP) { "Play Snap Station" }
                            }
                            nav class="br-toc" aria-label="On this page" {
                                @for (anchor, title) in &SNAP_SECTIONS {
                                    a href={"#" (anchor)} { (title) }
                                }
                            }
                        }
                    }
                    div class="br-guide br-wrap" {
                        (step("goal", 1, html! {
                            p { "The robot crew of Snap Station needs a builder. Float around the Crew Deck and visit a board to choose a job: join two modules, push a crate, spin a garden, or give a cargo ship a place to dock." }
                            p { "Build with the pieces in your tray, then press " strong { "Go" } ". If your build does the job, you win stars. If it doesn't, fix it and try again. No reading needed: the pictures show the way." }
                        }, Some(shot(site, "snap-jobs.jpg", "The Crew Deck with all five robots and four job boards", "Float around the Crew Deck and choose a job."))))
                        (step("crew", 2, html! {
                            ul class="br-list" {
                                li { strong { "Captain Cog" } ", the captain: a round orange robot with gears all around." }
                                li { strong { "Pip" } ", the helper drone: small, blue, one big eye." }
                                li { strong { "Rivet" } ", the builder: tall and green, with arms that wave." }
                                li { strong { "Nova" } ", the scientist: a purple ring with an eye in the middle." }
                                li { strong { "Tock" } ", the cargo bot: a yellow box with a clock on its tummy." }
                            }
                            p { "Tap the job card at the top of the screen to hear the job again." }
                        }, Some(shot(site, "snap-ask.jpg", "Pip asking: join my two modules, with a Start building button", "Pip's job: join the two modules."))))
                        (step("controls", 3, html! {
                            div class="br-table-wrap" {
                                table class="br-keys-table" {
                                    thead {
                                        tr {
                                            th scope="col" { "To" }
                                            th scope="col" { "Mouse and keyboard" }
                                            th scope="col" { "Phone or tablet" }
                                        }
                                    }
                                    tbody {
                                        @for (action, keys, touch) in &control_rows {
                                            tr {
                                                th scope="row" { (action) }
                                                td { (keys) }
                                                td { (touch) }
                                            }
                                        }
                                    }
                                }
                            }
                            p { "The first time you play, five quick cards show you how. Skip them if you like, and open them again with the " strong { "?" } " button." }
                        }, Some(shot(site, "snap-tour.jpg", "The game's first tutorial card, Welcome to Snap Station, with Skip and Next", "The tutorial in the game."))))
                        (step("snap", 4, html! {
                            p { "Pieces are magnetic. Hold one close to a side of the station, lined up with it, and a glowing outline shows where it will go. Let go and it snaps on, exactly flush." }
                            p { "Too far away, or turned the wrong way, and it doesn't snap. Turn it and try again. A triangle's sides point three ways, so sometimes it needs a turn before a side lines up." }
                            p { "A piece dropped into a corner joins both of its neighbours, and a ring of pieces closes by itself." }
                        }, Some(shot(site, "snap-drag.jpg", "A square held next to the station, with a glowing outline where it will snap", "Close and lined up: it snaps there."))))
                        (step("pieces", 5, html! {
                            ul class="br-list" {
                                li { strong { "Square" } ": four rods with magnets in the corners. Push it sideways and it leans." }
                                li { strong { "Triangle" } " and " strong { "half square" } ": they can't lean. Two half squares make a square that can't lean." }
                                li { strong { "Panel" } ": a solid square. Stiff, but twice as heavy." }
                                li { strong { "Strut" } ": drop it on a square to brace it corner to corner, so it can't lean." }
                                li { strong { "Bolt" } ": drop it on a join so the magnets can't let go." }
                                li { strong { "Thruster" } ": it snaps on by its flat side and pushes away from its nozzle while it burns." }
                                li { strong { "Solar wing" } ": long and light. It catches the solar wind." }
                                li { strong { "Crate" } " and " strong { "plant pod" } ": heavy things the jobs ask you to move or spin." }
                            }
                        }, None))
                        (step("go", 6, html! {
                            p { "Press " strong { "Go" } " to test your build in space. The physics is simple, but honest:" }
                            ul class="br-list" {
                                li { "Thrusters push. Push through the middle and the build goes straight; push off to one side and it spins." }
                                li { "Spinning makes gravity: the faster it spins, and the farther out a plant pod sits, the heavier the pod feels." }
                                li { "Heavy things pull hardest on their joins, so heavy parts belong in the middle." }
                                li { "Wobbly builds shake apart; braced ones hold." }
                            }
                            p { "Press " strong { "Stop" } " to go back to building at any time." }
                        }, Some(shot(site, "snap-spin.jpg", "The garden spinning with thrusters on its pods, and the plant gravity meter in the green", "Spinning makes gravity for the plants."))))
                        (step("strong", 7, html! {
                            p { "Squares lean when they're pushed sideways, and a square that leans too far pops apart. Triangles don't lean at all. A strut across a square stops it leaning." }
                            p { "Magnets hold, but only so hard. Pull too hard and a join lets go. A bolt on the join locks it for good." }
                            p { "When something breaks, the robot tells you what happened and what to try. Press " strong { "Fix it" } " and change your build." }
                        }, Some(shot(site, "snap-storm.jpg", "The solar storm blowing: a bare square bridge has come apart and a square drifts away", "In the storm, bare squares pop."))))
                        (step("jobs", 8, html! {
                            p { "Twenty jobs come from four boards: Dock Yard, Garden Ring, Storm Watch, and Deep Space. These five teach the first steps:" }
                            ul class="br-list" {
                                li { strong { "1. Hello, Magnets" } " (Pip): join two modules with a row of pieces." }
                                li { strong { "2. Straight Push" } " (Tock): push a crate into the green ring without spinning it." }
                                li { strong { "3. Storm Bridge" } " (Nova): join two modules before the solar storm comes. A clock shows how long you have. Brace and bolt your bridge." }
                                li { strong { "4. Garden Spin" } " (Rivet): spin the garden until the plant gravity meter sits in the green, and keep the pods on." }
                                li { strong { "5. Cargo Dock" } " (Captain Cog): build arms that fit around the cargo ship's outline. It flies in, ties up to the ends of your arms, and tugs." }
                            }
                            p { "Later jobs add hinges that turn, springs that soften bumps, tethers that only pull, reaction wheels that take spin, and cargo arms for towing. Some test the same build several ways: fix anything you want between tests, then press Go again." }
                            p { "Finishing a job opens the next one." }
                        }, Some(shot(site, "snap-dock.jpg", "Two braced, bolted arms around the cargo ship's outline in its berth", "A berth that fits the ship."))))
                        (step("stars", 9, html! {
                            p { "Every job done earns stars. Use as few pieces as the robot crew would, struts and bolts included, for three stars. The star board on the Crew Deck shows your best." }
                            p { "Your stars stay in this browser. Choose Save and visit Crew Deck to keep your build too. Your builds reopens it, ready to change or run again." }
                        }, Some(shot(site, "snap-done.jpg", "The Job done card with three stars and a button to save the build on the Crew Deck", "Job done, three stars!"))))
                        (step("tips", 10, html! {
                            ul class="br-list" {
                                li { "Watch what breaks: a flash of sparks shows where." }
                                li { "Triangles and struts make a build stiff; bolts make it strong." }
                                li { "Put thrusters in pairs, opposite each other, to spin without drifting." }
                                li { "Keep heavy parts close to the middle." }
                                li { "Leave room for the cargo ship: its outline shows where it goes." }
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(SNAP) { "Play now" }
                                a class="br-hero-link" href=(site.home) { "Back to Blue Rush Studios" }
                            }
                        }, None))
                    }
                }
                (footer(site))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_studio_host_maps_its_paths_to_the_studio() {
        assert_eq!(studio_path("/"), PATH);
        assert_eq!(studio_path("/grow-little-bunny/how-to-play"), GUIDE);
        assert_eq!(
            studio_path("/the-ice-cream-man/how-to-play"),
            ICECREAM_GUIDE
        );
        assert_eq!(
            studio_path("/static/bluerush.css"),
            format!("{ASSETS}bluerush.css")
        );
        for kept in [
            PATH,
            GUIDE,
            "/studios/blue-rush/static/bunny-run.jpg",
            BUNNY,
            "/games/grow-little-bunny/start.js",
            ICECREAM_GUIDE,
            "/static/legacy-demo.css",
            "/favicon.svg",
            "/health",
        ] {
            assert_eq!(studio_path(kept), kept);
        }
        assert_eq!(studio_path("/login"), "/studios/blue-rush/login");
        assert_eq!(studio_path("/app/tasks"), "/studios/blue-rush/app/tasks");
    }

    #[test]
    fn the_studio_pages_link_the_root_paths() {
        let home = render(&ON_STUDIO).into_string();
        let guide = render_guide(&ON_STUDIO).into_string();
        let snap = render_snap_guide(&ON_STUDIO).into_string();
        assert!(snap.contains("href=\"/\""), "home");
        assert!(home.contains("href=\"/snap-station/how-to-play\""));
        assert_eq!(studio_path("/snap-station/how-to-play"), SNAP_GUIDE);
        for kept in [SNAP, "/games/snap-station/start.js", SNAP_GUIDE] {
            assert_eq!(studio_path(kept), kept);
        }
        for html in [&home, &guide, &snap] {
            assert!(!html.contains("/studios/blue-rush"), "root paths only");
            assert!(html.contains("href=\"https://openagents.com/privacy\""));
        }
        assert!(guide.contains("href=\"/\""), "home");
        // The Ice Cream Man isn't a Blue Rush game.
        assert!(!home.contains("Ice Cream"));
        assert!(!home.contains("the-ice-cream-man"));
        assert_eq!(
            studio_path("/games/the-ice-cream-man"),
            "/studios/blue-rush/games/the-ice-cream-man"
        );
    }
}
