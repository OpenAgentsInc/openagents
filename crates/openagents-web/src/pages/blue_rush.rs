//! `/studios/blue-rush`: the Blue Rush Studios site, OpenAgents' game
//! studio (Grow Little Bunny, `docs/verse/games/grow-little-bunny.md`, and
//! the games after it).
//!
//! It is its own brand, so it does not use the OpenAgents shell
//! (`UiPage`): the page links its own stylesheet and script from
//! `static/bluerush/` and draws its own nav and footer. The page and its
//! files all live under [`PATH`], so the studio can later get its own
//! domain by mapping that host's `/` to this page and its `/static/...` to
//! [`ASSETS`] in the host guard, without moving anything.
//!
//! The policy is the site's plus `script-src 'self'` for the one script,
//! which draws the water and crabs behind the game cards and runs the sand
//! garden at the bottom. Without the script the page still reads in full;
//! only the two canvases stay still.

use std::sync::LazyLock;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};
use sha2::{Digest, Sha256};

use crate::App;

/// The page.
pub(crate) const PATH: &str = "/studios/blue-rush";

/// Where the page's stylesheet, script, and pictures are served.
pub(crate) const ASSETS: &str = "/studios/blue-rush/static/";

/// The page's policy: the site's, plus scripts from this site.
pub(crate) const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Where the hero's button and the featured card lead.
pub(crate) const BUNNY: &str = "/games/grow-little-bunny";

const CSS: &str = include_str!("../../static/bluerush/bluerush.css");
const JS: &str = include_str!("../../static/bluerush/bluerush.js");

/// The coast stills, from `bench/verse/2026-10-08/coast-c1` and `coast-c2`,
/// downscaled to JPEG.
const IMAGES: [(&str, &[u8]); 4] = [
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

static CSS_VERSION: LazyLock<String> = LazyLock::new(|| version(CSS));
static JS_VERSION: LazyLock<String> = LazyLock::new(|| version(JS));

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(page))
        .route("/studios/blue-rush/static/{file}", get(asset))
}

async fn asset(Path(file): Path<String>) -> Response {
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

async fn page() -> Response {
    let mut response = render().into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    response
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

fn render() -> Markup {
    let css = format!("{ASSETS}bluerush.css?v={}", *CSS_VERSION);
    let js = format!("{ASSETS}bluerush.js?v={}", *JS_VERSION);
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Blue Rush Studios" }
                meta name="description" content="Learning games about living things, the ocean, and building new homes on the sea.";
                meta name="theme-color" content="#000930";
                link rel="stylesheet" href=(css);
                script src=(js) defer {}
            }
            body class="br" {
                a class="br-skip" href="#main" { "Skip to content" }
                header class="br-nav" {
                    a class="br-wordmark" href=(PATH) aria-label="Blue Rush Studios home" {
                        (pixel_word("BLUE"))
                        (pixel_word("RUSH"))
                    }
                    nav class="br-links" aria-label="Main" {
                        a href="#games" { "Games" }
                        a href="#learn" { "Learn" }
                        a href="#garden" { "Sand garden" }
                    }
                    a class="br-outline" href=(BUNNY) { "Play now" }
                }
                main id="main" {
                    section class="br-hero" {
                        img class="br-hero-img" src={(ASSETS) "hero-bay.jpg"} alt="" width="960" height="540";
                        div class="br-hero-text" {
                            h1 class="br-headline" { (pixel_text("Play the future")) }
                            p class="br-pitch" {
                                "Learning games about living things, the ocean, and building new homes on the sea."
                            }
                            a class="br-cta" href=(BUNNY) { "Play Grow Little Bunny" }
                        }
                    }
                    section class="br-games" id="games" aria-labelledby="games-title" {
                        canvas class="br-water" id="br-water" aria-hidden="true" {}
                        div class="br-wrap" {
                            h2 class="br-h2" id="games-title" { "Our games" }
                            div class="br-cards" {
                                a class="br-card br-card-featured" href=(BUNNY) {
                                    img src={(ASSETS) "lighthouse.jpg"} alt="A lighthouse and a small cottage on a green hill" width="960" height="540" loading="lazy";
                                    div class="br-card-body" {
                                        span class="br-tag br-tag-hot" { "Featured" }
                                        h3 { "Grow Little Bunny" }
                                        p { "Hop through a farmer's garden, eat everything you can, and grow from a tiny white bunny into a big orange one. Watch out for the net." }
                                        span class="br-card-go" { "Play" }
                                    }
                                }
                                @for game in &LAB {
                                    article class="br-card" {
                                        img src={(ASSETS) (game.image)} alt=(game.alt) width="960" height="540" loading="lazy";
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
                            p class="br-lede" { "Drag to rake the sand. Tap to set a stone. Turn on Mandala to make patterns." }
                            div class="br-garden-frame" {
                                canvas class="br-sand" id="br-sand" tabindex="0" role="application" aria-roledescription="sand garden"
                                    aria-label="Sand garden. Use the arrow keys to move the rake, hold Space to rake, and press Enter to set a stone." {}
                            }
                            div class="br-garden-tools" {
                                button class="br-tool" type="button" id="br-mandala" aria-pressed="false" { "Mandala" }
                                button class="br-tool" type="button" id="br-smooth" { "Smooth the sand" }
                            }
                            p class="br-keys" { "On a keyboard: arrows move the rake, hold Space to rake, Enter sets a stone." }
                        }
                    }
                }
                footer class="br-foot" {
                    div class="br-wrap br-foot-row" {
                        div {
                            p class="br-foot-name" { "Blue Rush Studios" }
                            p { a href="/" { "An OpenAgents studio" } }
                        }
                        nav class="br-foot-links" aria-label="Legal" {
                            a href="/terms" { "Terms" }
                            a href="/privacy" { "Privacy" }
                        }
                    }
                }
            }
        }
    }
}
