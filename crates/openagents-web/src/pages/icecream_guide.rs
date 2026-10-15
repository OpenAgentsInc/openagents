//! `/games/the-ice-cream-man/how-to-play`: The Ice Cream Man's written
//! guide, beside the game. The game has no studio: the page is plain
//! OpenAgents, with the guide stylesheet the games share and screenshots
//! from the real game served beside it ([`super::icecream`]). The old
//! address under the studio pages redirects here
//! ([`super::blue_rush::ICECREAM_GUIDE`]).
//!
//! It runs no script ([`super::blue_rush::GUIDE_POLICY`]).

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use maud::{DOCTYPE, Markup, html};

use super::blue_rush::{GUIDE_POLICY, key, step_in};

/// The guide.
pub(crate) const ICECREAM_GUIDE_PATH: &str = "/games/the-ice-cream-man/how-to-play";

/// The game.
const GAME: &str = super::icecream::ICECREAM_PATH;

/// Where the guide's stylesheet and pictures are: beside the game.
const ASSETS: &str = "/games/the-ice-cream-man/";

/// The guide's sections: their anchors and titles.
const SECTIONS: [(&str, &str); 13] = [
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
    ("speedrun", "Speedrun"),
    ("staff", "The Sprinkle Staff"),
    ("tips", "Tips"),
];

pub(crate) async fn page() -> Response {
    let mut response = render().into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(GUIDE_POLICY),
    );
    response
}

/// A screenshot from the game, with its caption.
fn shot(file: &str, alt: &str, caption: &str) -> Markup {
    html! {
        figure class="br-shot" {
            img src={(ASSETS) (file)} alt=(alt) width="960" height="600" loading="lazy";
            figcaption { (caption) }
        }
    }
}

pub(crate) fn render() -> Markup {
    let step = |id, number, body, picture| step_in(&SECTIONS, id, number, body, picture);
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
            html! { "Hold the Sprint button. In the menu you can make it a tap that turns sprint on and off." },
        ),
        (
            "Use your tool",
            html! { "Left click" },
            html! { "The Fire button" },
        ),
        (
            "Pick a tool",
            html! { (key("1")) " to " (key("8")) ", " (key("Q")) " for the next one, or click the hot bar" },
            html! { "Tap a tool in the hot bar, swipe the bar, or tap Next" },
        ),
        (
            "Hide, open a fridge, unlock the door",
            html! { (key("E")) },
            html! { "The Use button" },
        ),
        (
            "Menu and settings",
            html! { (key("Esc")) },
            html! { "The Menu button" },
        ),
        (
            "Open the tutorial",
            html! { (key("?")) ", or How to play in the menu" },
            html! { "How to play in the menu" },
        ),
    ];
    let css = format!(
        "{ASSETS}guide.css?v={}",
        super::icecream::guide_css_version()
    );
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "How to play The Ice Cream Man \u{b7} OpenAgents" }
                meta name="description" content="How to play The Ice Cream Man: the controls, the keys, hot fudge and fart clouds, minions, the weapons you earn, hiding, speedrunning, and how to get out of the Airbnb.";
                meta name="theme-color" content="#2b1a12";
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                link rel="stylesheet" href=(css);
            }
            body class="br" {
                a class="br-skip" href="#main" { "Skip to content" }
                header class="br-nav" {
                    a class="br-home" href="/" { "OpenAgents" }
                    nav class="br-links" aria-label="Main" {
                        a href=(ICECREAM_GUIDE_PATH) { "How to play" }
                    }
                    a class="br-outline" href=(GAME) { "Play now" }
                }
                main id="main" {
                    header class="br-guide-head" {
                        div class="br-wrap" {
                            p class="br-crumbs" {
                                a href="/" { "OpenAgents" }
                                " / The Ice Cream Man"
                            }
                            h1 class="br-guide-title" { "How to play The Ice Cream Man" }
                            p class="br-guide-lede" {
                                "You wake up in an Airbnb that keeps going. Somewhere inside is the Ice Cream Man, and he wants you in a cone. Find three keys and get out the front door, or earn weapons and melt him. Here's how, with pictures from the game."
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(GAME) { "Play The Ice Cream Man" }
                            }
                            p class="br-guide-lede" {
                                strong { "Play it like an app: " }
                                "on an iPhone or iPad, open the game in Safari, tap Share, then Add to Home Screen. On Android, open it in Chrome, tap the menu, then Add to Home screen or Install app. It opens full screen, with no browser bars and no zooming, and plays offline."
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
                            p { "Your check-in note says: " strong { "\u{201c}Check-out is at 11. Don't let him in.\u{201d}" } " Too late. He's in." }
                            p { "Find the " strong { "three check-out keys" } ", then go to the front door and hold " (key("E")) " for 2 seconds to unlock it. Get out and you win." }
                        }, Some(shot("icecream-hall.jpg", "The bedroom where you wake up: your blocky avatar with the hot fudge blaster next to a closet, and the hot bar at the bottom", "The bedroom where you wake up."))))
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
                            p { "On a phone you can hold Sprint with one thumb while the other works the stick. Tapping a button twice fast never zooms the page. If a page ever does end up zoomed in, tap " strong { "Zoom back out" } " at the top." }
                            p { "The first time you play, four quick cards show these controls. Skip them if you like, and open them again with " (key("?")) " or " strong { "How to play" } " in the menu." }
                        }, Some(shot("icecream-tour.jpg", "The game's tutorial card about looking around, with Back, Skip and Next buttons", "The tutorial in the game."))))
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
                        }, Some(shot("icecream-man.jpg", "The Ice Cream Man: a waffle-cone body with a vanilla scoop head, a huge bald forehead and a big grin", "He's coming. Hear the jingle?"))))
                        (step("tools", 6, html! {
                            p { "Your tools sit in the hot bar at the bottom of the screen." }
                            ul class="br-list" {
                                li { strong { "1. Hot Fudge Blaster" } " (30 squirts): each hit slows him down. Six quick hits and he freezes solid for 3 seconds." }
                                li { strong { "2. Fart Launcher" } " (3 clouds): knocks him back, and he forgets where you are for 4 seconds." }
                                li { strong { "3. Key Ring" } ": shows your keys. Click to jingle them and lure him somewhere." }
                            }
                            p { "Running low? Press " (key("E")) " at a fridge for more fudge or a fart cloud, and ammo for any weapons you've earned. Each fridge opens once." }
                        }, Some(shot("icecream-fudge.jpg", "The Ice Cream Man frozen solid in hardened hot fudge, next to your avatar", "Six quick hits and he's frozen solid."))))
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
                        }, Some(shot("icecream-hide.jpg", "The view from inside a closet, peeking out at the room", "Hiding in a closet."))))
                        (step("escape", 10, html! {
                            p { "With all three keys, go to the front door and hold " (key("E")) " for 2 seconds. Rattling the door makes noise, so he may come running." }
                            p { "Or melt him with your weapons. That's a win too." }
                            p { "You have 100 health. It slowly comes back once you haven't been hit for 10 seconds. If it runs out, you're " strong { "scooped" } ": he lunges right at you with a scream and splats the screen with strawberry syrup. It's a jumpscare! Turn " strong { "Jumpscares" } " off in the menu for a gentle ending instead. Then press " strong { "Try again" } "." }
                            p { "After every run you get a card with your time, your keys, the hits you took, and how well you aimed your fudge." }
                        }, Some(shot("icecream-door.jpg", "The run card after escaping, with the time, keys, hits taken and fudge accuracy", "Out! Your run card."))))
                        (step("speedrun", 11, html! {
                            p { "A timer at the top right counts every run to a tenth of a second. When you escape, the card shows your time and your best. Beat it and it says " strong { "New best!" } " Your best time stays on this device." }
                            p { "Turn the timer off or on with " strong { "Timer" } " in the menu. The menu also shows the game's version at the bottom." }
                        }, None))
                        (step("staff", 12, html! {
                            p { "After you win, press " strong { "Keep playing with the Sprinkle Staff" } " to stay in the house as long as you like. The staff shoots one super long sprinkle that hurts whatever it touches, and makes minions of your own that fight for you." }
                        }, None))
                        (step("tips", 13, html! {
                            ul class="br-list" {
                                li { "Sprint to get out of his sight, then hide." }
                                li { "Save a fart cloud for when he's close." }
                                li { "Fudge him from far away, before he can grab you." }
                                li { "Freeze him, then hit him with your weapons while he can't move." }
                                li { "Hurt? Melt a minion and hope for a cone." }
                                li { "Jingle your keys in one room, then sneak off to another." }
                            }
                            div class="br-guide-actions" {
                                a class="br-cta" href=(GAME) { "Play now" }
                            }
                        }, None))
                    }
                }
                footer class="br-foot" {
                    div class="br-wrap br-foot-row" {
                        div {
                            p class="br-foot-name" { "The Ice Cream Man" }
                            p { a href="/" { "By OpenAgents" } }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_has_no_studio_and_links_beside_the_game() {
        let html = render().into_string();
        assert!(!html.contains("Blue Rush"));
        assert!(!html.contains("/studios/"));
        assert!(html.contains("src=\"/games/the-ice-cream-man/icecream-hall.jpg\""));
        assert!(html.contains("href=\"/games/the-ice-cream-man/guide.css?v="));
        for words in [
            "Add to Home Screen",
            "Jumpscares",
            "New best!",
            "Hold the Sprint button",
            "swipe the bar",
        ] {
            assert!(html.contains(words), "{words}");
        }
    }
}
