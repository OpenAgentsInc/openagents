//! `/studios/blue-rush`: the Blue Rush Studios site, OpenAgents' game
//! studio (Grow Little Bunny, `docs/verse/games/grow-little-bunny.md`, The
//! Ice Cream Man, `docs/verse/games/the-ice-cream-man.md`, and the games
//! after them), and its pages: the Grow Little Bunny guide at [`GUIDE`] and
//! The Ice Cream Man's at [`ICECREAM_GUIDE`].
//!
//! It is its own brand, so it does not use the OpenAgents shell
//! (`UiPage`): the pages link their own stylesheet (and the homepage its
//! script) from `static/bluerush/` and draw their own nav and footer.
//!
//! **Its own domain.** The studio's hosts ([`HOSTS`] by default, more with
//! `--studio-host`) serve the same pages at the root: `/` is the homepage,
//! `/grow-little-bunny/how-to-play` and `/the-ice-cream-man/how-to-play`
//! the guides, `/static/...` the studio's files, and
//! `/games/grow-little-bunny` and `/games/the-ice-cream-man` the games ([`studio_path`]
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
use axum::extract::Path;
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

/// The Ice Cream Man guide. `icecream-web` links it from its tutorial and
/// menu, so the path stays put.
pub(crate) const ICECREAM_GUIDE: &str = "/studios/blue-rush/the-ice-cream-man/how-to-play";

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
img-src 'self'; script-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The guide's policy: the site's, with no script at all.
pub(crate) const GUIDE_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Where the hero's button and the featured card lead.
pub(crate) const BUNNY: &str = "/games/grow-little-bunny";

/// The Ice Cream Man's game.
pub(crate) const ICECREAM: &str = "/games/the-ice-cream-man";

const CSS: &str = include_str!("../../static/bluerush/bluerush.css");
const JS: &str = include_str!("../../static/bluerush/bluerush.js");

/// The coast stills, from `bench/verse/2026-10-08/coast-c1` and `coast-c2`,
/// downscaled to JPEG, and the guide's screenshots of the real game
/// (`bunny-*.jpg`, 960 by 600), and The Ice Cream Man's (`icecream-*.jpg`,
/// 960 by 600).
const IMAGES: [(&str, &[u8]); 18] = [
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
    (
        "icecream-hall.jpg",
        include_bytes!("../../static/bluerush/icecream-hall.jpg"),
    ),
    (
        "icecream-man.jpg",
        include_bytes!("../../static/bluerush/icecream-man.jpg"),
    ),
    (
        "icecream-fudge.jpg",
        include_bytes!("../../static/bluerush/icecream-fudge.jpg"),
    ),
    (
        "icecream-hide.jpg",
        include_bytes!("../../static/bluerush/icecream-hide.jpg"),
    ),
    (
        "icecream-door.jpg",
        include_bytes!("../../static/bluerush/icecream-door.jpg"),
    ),
    (
        "icecream-tour.jpg",
        include_bytes!("../../static/bluerush/icecream-tour.jpg"),
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
        .route(GUIDE, get(guide))
        .route(ICECREAM_GUIDE, get(icecream_guide))
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
        || path == ICECREAM
        || path.starts_with("/games/the-ice-cream-man/")
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
    icecream_guide: &'static str,
    assets: &'static str,
    /// Prefixed to the links to the rest of openagents.com.
    openagents: &'static str,
}

const ON_OPENAGENTS: Site = Site {
    home: PATH,
    guide: GUIDE,
    icecream_guide: ICECREAM_GUIDE,
    assets: ASSETS,
    openagents: "",
};

const ON_STUDIO: Site = Site {
    home: "/",
    guide: "/grow-little-bunny/how-to-play",
    icecream_guide: "/the-ice-cream-man/how-to-play",
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

async fn icecream_guide(headers: HeaderMap) -> Response {
    with_policy(render_icecream_guide(site(&headers)), GUIDE_POLICY)
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
                                article class="br-card br-card-new" {
                                    a class="br-card-media" href=(ICECREAM) tabindex="-1" aria-hidden="true" {
                                        img src={(site.assets) "icecream-hall.jpg"} alt="" width="960" height="600" loading="lazy";
                                    }
                                    div class="br-card-body" {
                                        span class="br-tag br-tag-new" { "New" }
                                        h3 { "The Ice Cream Man" }
                                        p { "You're stuck in an Airbnb that keeps going, and the Ice Cream Man wants you in a cone. Find three keys, blast him with hot fudge, and get out." }
                                        div class="br-card-actions" {
                                            a class="br-card-go" href=(ICECREAM) { "Play" }
                                            a class="br-card-link" href=(site.icecream_guide) { "How to play" }
                                        }
                                    }
                                }
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
fn key(name: &str) -> Markup {
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

fn step_in(
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

/// The Ice Cream Man guide's sections: their anchors and titles.
const ICECREAM_SECTIONS: [(&str, &str); 12] = [
    ("goal", "The goal"),
    ("controls", "Controls"),
    ("house", "The house"),
    ("keys", "Find the keys"),
    ("man", "The Ice Cream Man"),
    ("tools", "Hot fudge and farts"),
    ("minions", "Minions and cones"),
    ("weapons", "Earn weapons"),
    ("hide", "Hide"),
    ("escape", "Escape, melt him, or get scooped"),
    ("staff", "The Sprinkle Staff"),
    ("tips", "Tips"),
];

fn render_icecream_guide(site: &Site) -> Markup {
    let step = |id, number, body, picture| step_in(&ICECREAM_SECTIONS, id, number, body, picture);
    let control_rows: [(&str, Markup, Markup); 11] = [
        (
            "Move",
            html! { (key("W")) " " (key("A")) " " (key("S")) " " (key("D")) " or the arrow keys" },
            html! { "The stick on the left" },
        ),
        (
            "Look around",
            html! { "Hold the right mouse button and drag. The mouse wheel zooms; all the way in is first person." },
            html! { "Drag on the right side of the screen" },
        ),
        (
            "Shift Lock (aim)",
            html! { (key("Ctrl")) " locks the camera over your shoulder, with a crosshair" },
            html! { "Not needed: Fire aims at him when he's in front of you" },
        ),
        (
            "First or third person",
            html! { (key("V")) },
            html! { "The View button" },
        ),
        (
            "Jump",
            html! { (key("Space")) },
            html! { "The Jump button" },
        ),
        (
            "Sprint",
            html! { "Hold " (key("Shift")) },
            html! { "The Sprint button turns it on and off" },
        ),
        (
            "Use your tool",
            html! { "Left click" },
            html! { "The Fire button" },
        ),
        (
            "Pick a tool",
            html! { (key("1")) " to " (key("7")) ", or click the hot bar" },
            html! { "Tap the hot bar" },
        ),
        (
            "Hide, open a fridge, unlock the door",
            html! { (key("E")) },
            html! { "The Use button" },
        ),
        ("Menu", html! { (key("Esc")) }, html! { "The menu button" }),
        (
            "Open the tutorial",
            html! { (key("?")) ", or How to play in the menu" },
            html! { "How to play in the menu" },
        ),
    ];
    html! {
        (DOCTYPE)
        html lang="en" {
            (head(site, "How to play The Ice Cream Man \u{b7} Blue Rush Studios", "How to play The Ice Cream Man: the controls, the keys, hot fudge and fart clouds, minions, the weapons you earn, hiding, and how to get out of the Airbnb.", false))
            body class="br" {
                (nav(site, &[("Games", &format!("{}#games", site.home)), ("How to play", site.icecream_guide)], ICECREAM))
                main id="main" {
                    header class="br-guide-head" {
                        div class="br-wrap" {
                            p class="br-crumbs" {
                                a href=(site.home) { "Blue Rush Studios" }
                                " / The Ice Cream Man"
                            }
                            h1 class="br-guide-title" { "How to play The Ice Cream Man" }
                            p class="br-guide-lede" {
                                "You wake up in an Airbnb that keeps going. Somewhere inside is the Ice Cream Man, and he wants you in a cone. Find three keys and get out the front door, or earn weapons and melt him. Here's how, with pictures from the game."
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(ICECREAM) { "Play The Ice Cream Man" }
                            }
                            nav class="br-toc" aria-label="On this page" {
                                @for (anchor, title) in &ICECREAM_SECTIONS {
                                    a href={"#" (anchor)} { (title) }
                                }
                            }
                        }
                    }
                    div class="br-guide br-wrap" {
                        (step("goal", 1, html! {
                            p { "Your check-in note says: " strong { "\u{201c}Check-out is at 11. Don't let him in.\u{201d}" } " Too late. He's in." }
                            p { "Find the " strong { "three check-out keys" } ", then go to the front door and hold " (key("E")) " for 2 seconds to unlock it. Get out and you win." }
                        }, Some(shot(site, "icecream-hall.jpg", "The bedroom where you wake up: your blocky avatar with the hot fudge blaster next to a closet, and the hot bar at the bottom", "The bedroom where you wake up."))))
                        (step("controls", 2, html! {
                            div class="br-table-wrap" {
                                table class="br-keys-table" {
                                    thead {
                                        tr {
                                            th scope="col" { "To" }
                                            th scope="col" { "Keyboard and mouse" }
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
                            p { "The first time you play, four quick cards show these controls. Skip them if you like, and open them again with " (key("?")) " or " strong { "How to play" } " in the menu." }
                        }, Some(shot(site, "icecream-tour.jpg", "The game's tutorial card about looking around, with Back, Skip and Next buttons", "The tutorial in the game."))))
                        (step("house", 3, html! {
                            p { "You wake up in the bedroom. The house has a kitchen, a living room, a bathroom, a laundry, hallways, a stairwell, and the freezer room. Some rooms show up again, and some hallways are longer than they should be." }
                            p { "Past them are the Backrooms: wide, empty rooms of yellow wallpaper and damp carpet under buzzing lights that flicker. Extra doorways loop back on themselves, and the fog hides what's ahead." }
                            p { "Every run shuffles the rooms, so it's a new house every time." }
                        }, None))
                        (step("keys", 4, html! {
                            p { "Keys spin and sparkle just above the floor. Walk over one to pick it up. The count at the top of the screen shows how many you have." }
                            p { "A fridge with a pink sparkle on top hasn't been opened yet." }
                        }, None))
                        (step("man", 5, html! {
                            p { "He's a walking ice cream cone with a huge shiny forehead. He wanders the house and comes to check on noises: running, firing, jingling keys, and rattling the front door. When he sees you, he chases you. When he loses you, he searches where he last saw you." }
                            ul class="br-list" {
                                li { strong { "Sprinkles" } ": from far away he fires sprinkles. Each one costs 5 health. Jump to dodge them." }
                                li { strong { "Grab" } ": up close he grabs you. That costs 30 health and shoves you back." }
                                li { strong { "Speed" } ": he's about as fast as you walking, so sprinting gets you away." }
                            }
                            p { "Listen for the ice cream truck jingle. The louder it gets, the closer he is." }
                        }, Some(shot(site, "icecream-man.jpg", "The Ice Cream Man: a waffle-cone body with a vanilla scoop head, a huge bald forehead and a big grin", "He's coming. Hear the jingle?"))))
                        (step("tools", 6, html! {
                            p { "Your tools sit in the hot bar at the bottom of the screen." }
                            ul class="br-list" {
                                li { strong { "1. Hot Fudge Blaster" } " (30 squirts): each hit slows him down. Six quick hits and he freezes solid for 3 seconds." }
                                li { strong { "2. Fart Launcher" } " (3 clouds): knocks him back, and he forgets where you are for 4 seconds." }
                                li { strong { "3. Key Ring" } ": shows your keys. Click to jingle them and lure him somewhere." }
                            }
                            p { "Running low? Press " (key("E")) " at a fridge for more fudge or a fart cloud, and ammo for any weapons you've earned. Each fridge opens once." }
                        }, Some(shot(site, "icecream-fudge.jpg", "The Ice Cream Man frozen solid in hardened hot fudge, next to your avatar", "Six quick hits and he's frozen solid."))))
                        (step("minions", 7, html! {
                            p { "Soon after you start, his " strong { "minions" } " come for you: mini Ice Cream Men that hunt you down and bite. Up to three roam the house at once." }
                            p { "Hot fudge, fart clouds, and your weapons melt them. A melted minion sometimes drops an " strong { "ice cream cone" } ". Walk over it to eat it: 50% more health for 10 seconds." }
                        }, None))
                        (step("weapons", 8, html! {
                            p { "Four weapons are hidden in the house. Each one is earned, and each one hurts him. They go in the hot bar after your tools, on " (key("4")) " and up." }
                            ul class="br-list" {
                                li { strong { "Candy Cane Sword" } ": the first minion you melt drops it. Swing it up close." }
                                li { strong { "Waffle Cone Spear" } ": press the three sprinkle buttons in order, pink, blue, then yellow, and the cone cabinet opens. It reaches a long way." }
                                li { strong { "Cone Pistol" } ": freeze him with hot fudge once and it's yours. It fires cones from far away." }
                                li { strong { "Sugar Launcher" } ": melt four minions to earn it. Its sugar bursts hurt everything nearby." }
                            }
                            p { "A bar at the top shows his health. Wear it down and he melts." }
                        }, None))
                        (step("hide", 9, html! {
                            p { "Press " (key("E")) " next to a closet to hide inside, and " (key("E")) " again to come out. He can't open your closet, unless he saw you get in." }
                        }, Some(shot(site, "icecream-hide.jpg", "The view from inside a closet, peeking out at the room", "Hiding in a closet."))))
                        (step("escape", 10, html! {
                            p { "With all three keys, go to the front door and hold " (key("E")) " for 2 seconds. Rattling the door makes noise, so he may come running." }
                            p { "Or melt him with your weapons. That's a win too." }
                            p { "You have 100 health. It slowly comes back once you haven't been hit for 10 seconds. If it runs out, you're " strong { "scooped" } ": he plops you in a cone and the run is over. Press " strong { "Try again" } "." }
                            p { "After every run you get a card with your time, your keys, the hits you took, and how well you aimed your fudge." }
                        }, Some(shot(site, "icecream-door.jpg", "The run card after escaping, with the time, keys, hits taken and fudge accuracy", "Out! Your run card."))))
                        (step("staff", 11, html! {
                            p { "After you win, press " strong { "Keep playing with the Sprinkle Staff" } " to stay in the house as long as you like. The staff shoots one super long sprinkle that hurts whatever it touches, and makes minions of your own that fight for you." }
                        }, None))
                        (step("tips", 12, html! {
                            ul class="br-list" {
                                li { "Sprint to get out of his sight, then hide." }
                                li { "Save a fart cloud for when he's close." }
                                li { "Fudge him from far away, before he can grab you." }
                                li { "Freeze him, then hit him with your weapons while he can't move." }
                                li { "Hurt? Melt a minion and hope for a cone." }
                                li { "Jingle your keys in one room, then sneak off to another." }
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(ICECREAM) { "Play now" }
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
            ICECREAM,
            "/games/the-ice-cream-man/start.js",
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
        let icecream = render_icecream_guide(&ON_STUDIO).into_string();
        for html in [&home, &guide, &icecream] {
            assert!(!html.contains("/studios/blue-rush"), "root paths only");
            assert!(html.contains("href=\"https://openagents.com/privacy\""));
        }
        assert!(guide.contains("href=\"/\""), "home");
        assert!(icecream.contains("href=\"/\""), "home");
        assert!(home.contains("href=\"/the-ice-cream-man/how-to-play\""));
    }
}
