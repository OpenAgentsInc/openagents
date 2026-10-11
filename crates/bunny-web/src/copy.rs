//! Every word the game shows.

pub const TITLE: &str = "Grow Little Bunny";
pub const GOAL: &str = "Eat everything in the garden. Don't get caught.";
pub const KEYS: &str =
    "\u{2190} \u{2192} dodge and turn \u{b7} \u{2191} jump \u{b7} \u{2193} duck \u{b7} X turn back";
pub const SWIPES: &str = "Swipe left or right to dodge and turn \u{b7} up to jump \u{b7} down to duck \u{b7} \u{21b6} turns back";
pub const PLAY: &str = "Play";
pub const PLAY_AGAIN: &str = "Play again";
pub const RESTART: &str = "Restart";
pub const CARROTS: &str = "Food left";
pub const WON: &str = "Garden cleared!";
pub const CAUGHT: &str = "Caught!";
pub const CAUGHT_LINE: &str = "The farmer got you with his net.";
pub const NO_WEBGL: &str =
    "This browser can't show the game. Try a recent Chrome, Safari, or Firefox.";
pub const MAP: &str = "Map of the garden";
pub const PAUSE: &str = "Pause";
pub const PAUSED: &str = "Paused";
pub const RESUME: &str = "Resume";
pub const LEAVE: &str = "Leave the garden";
pub const NEXT: &str = "Next garden";
pub const GARDENS: &str = "All gardens";
pub const TURN_BACK: &str = "Turn back";
pub const MEADOW: &str = "Back to the meadow";
pub const MEADOW_KEYS: &str = "Hop around the meadow and walk into a rabbit hole to play.";
pub const MEADOW_DRAG: &str = "Drag to hop around the meadow, and walk into a rabbit hole to play.";
pub const BURROW: &str = "The Burrow";
pub const BOARD: &str = "Carrot Board";
pub const ARCH: &str = "Leave the meadow?";
pub const ARCH_PROMPT: &str = "The way out";
pub const HOME: &str = "Back to OpenAgents";
pub const CLOSE: &str = "Close";
pub const NEXT_SHADE: &str = "One more win turns your bunny a shade more orange.";
pub const LAST_SHADE: &str = "Your bunny is as orange as it gets.";

/// The tutorial: its button, its own buttons, and the link to the written
/// guide on the Blue Rush Studios site.
pub const HOW_TO_PLAY: &str = "How to play";
pub const TOUR_NEXT: &str = "Next";
pub const TOUR_BACK: &str = "Back";
pub const TOUR_SKIP: &str = "Skip";
pub const TOUR_DONE: &str = "Start playing";
pub const TOUR_CLOSE: &str = "Got it";
pub const GUIDE: &str = "Read the full guide";

/// One step of the tutorial: a title, then its lines for a keyboard and
/// for a touch screen.
pub struct TourStep {
    pub title: &'static str,
    pub keys: &'static [&'static str],
    pub touch: &'static [&'static str],
}

/// The tutorial's steps, in order.
pub const TOUR: [TourStep; 6] = [
    TourStep {
        title: "Welcome, little bunny",
        keys: &[
            "You are a tiny white bunny, and the farmer's gardens are full of food.",
            "Eat it all, grow big, and don't get caught.",
        ],
        touch: &[
            "You are a tiny white bunny, and the farmer's gardens are full of food.",
            "Eat it all, grow big, and don't get caught.",
        ],
    },
    TourStep {
        title: "Hop to a garden",
        keys: &[
            "Use the arrow keys or W A S D to hop around the meadow.",
            "Walk into a rabbit hole to go into its garden.",
        ],
        touch: &[
            "Drag anywhere on the screen to hop around the meadow.",
            "Walk into a rabbit hole to go into its garden.",
        ],
    },
    TourStep {
        title: "Run and dodge",
        keys: &[
            "In a garden you run by yourself. \u{2190} and \u{2192} dodge left and right and turn at corners.",
            "\u{2191} or Space jumps, \u{2193} ducks, X turns you around, and Esc pauses.",
        ],
        touch: &[
            "In a garden you run by yourself. Swipe left or right to dodge and to turn at corners.",
            "Swipe up to jump and down to duck. The \u{21b6} button turns you around.",
        ],
    },
    TourStep {
        title: "Eat and grow",
        keys: &[
            "Food is in colour. Eat it to grow from Kit to Giant.",
            "Big bunnies smash flower pots. Bumping into things makes you a little smaller.",
        ],
        touch: &[
            "Food is in colour. Eat it to grow from Kit to Giant.",
            "Big bunnies smash flower pots. Bumping into things makes you a little smaller.",
        ],
    },
    TourStep {
        title: "Watch for the farmer",
        keys: &[
            "The farmer chases you with his net. When he's out of sight, an arrow points to him.",
            "Eat a golden carrot to scare him, then bump into him to make him dizzy.",
        ],
        touch: &[
            "The farmer chases you with his net. When he's out of sight, an arrow points to him.",
            "Eat a golden carrot to scare him, then bump into him to make him dizzy.",
        ],
    },
    TourStep {
        title: "Clear the garden",
        keys: &[
            "Eat every last bite to win. The map shows the food that's left.",
            "Each win turns your bunny a shade more orange. Too fast? Turn on Gentle mode in the Burrow.",
        ],
        touch: &[
            "Eat every last bite to win. The map shows the food that's left.",
            "Each win turns your bunny a shade more orange. Too fast? Turn on Gentle mode in the Burrow.",
        ],
    },
];

/// Where the tutorial is.
#[must_use]
pub fn tour_step_line(step: usize, steps: usize) -> String {
    format!("Step {step} of {steps}")
}

/// The player's win count.
#[must_use]
pub fn wins_line(wins: u32) -> String {
    format!("Wins: {wins}")
}

/// The bunny's shade.
#[must_use]
pub fn shade_line(name: &str, shade: u32) -> String {
    format!("Colour: {name} ({shade} of 20)")
}

/// The Gentle mode switch.
#[must_use]
pub fn gentle(on: bool) -> String {
    format!("Gentle mode: {}", if on { "on" } else { "off" })
}

/// The high contrast switch.
#[must_use]
pub fn contrast(on: bool) -> String {
    format!("High contrast: {}", if on { "on" } else { "off" })
}

/// A garden's best run on the Carrot Board.
#[must_use]
pub fn best_line(n: usize, name: &str, seconds: u32, score: u32) -> String {
    format!(
        "{n}. {name}: {}:{:02}, {score} points",
        seconds / 60,
        seconds % 60
    )
}

/// A garden not cleared yet.
#[must_use]
pub fn not_cleared(n: usize, name: &str) -> String {
    format!("{n}. {name}: not cleared yet")
}

/// A button that opens garden `n`.
#[must_use]
pub fn garden_button(n: usize, name: &str) -> String {
    format!("{n}. {name}")
}

/// How long a cleared garden took.
#[must_use]
pub fn time_line(seconds: u32) -> String {
    format!("Time: {}:{:02}", seconds / 60, seconds % 60)
}

/// The run's score.
#[must_use]
pub fn score_line(score: u32) -> String {
    format!("Score: {score}")
}

/// The line under a win.
#[must_use]
pub fn won_line(wins: u32) -> String {
    let shade = if wins >= bunny_rules::shade::MAX_SHADE {
        "Your bunny is as orange as it gets."
    } else {
        "Your bunny is one shade more orange."
    };
    format!("{shade} Wins: {wins}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_are_plain() {
        let mut all = vec![
            TITLE.to_owned(),
            GOAL.to_owned(),
            KEYS.to_owned(),
            SWIPES.to_owned(),
            PLAY.to_owned(),
            PLAY_AGAIN.to_owned(),
            RESTART.to_owned(),
            CARROTS.to_owned(),
            WON.to_owned(),
            CAUGHT.to_owned(),
            CAUGHT_LINE.to_owned(),
            NO_WEBGL.to_owned(),
            MAP.to_owned(),
            won_line(1),
            won_line(40),
            PAUSE.to_owned(),
            PAUSED.to_owned(),
            RESUME.to_owned(),
            LEAVE.to_owned(),
            NEXT.to_owned(),
            GARDENS.to_owned(),
            TURN_BACK.to_owned(),
            time_line(83),
            MEADOW.to_owned(),
            MEADOW_KEYS.to_owned(),
            MEADOW_DRAG.to_owned(),
            BURROW.to_owned(),
            BOARD.to_owned(),
            ARCH.to_owned(),
            ARCH_PROMPT.to_owned(),
            HOME.to_owned(),
            CLOSE.to_owned(),
            NEXT_SHADE.to_owned(),
            LAST_SHADE.to_owned(),
            wins_line(3),
            shade_line("Cream", 3),
            gentle(true),
            contrast(false),
            best_line(1, "Kitchen Bed", 83, 16_365),
            not_cleared(2, "Herb Corner"),
            score_line(1200),
            HOW_TO_PLAY.to_owned(),
            TOUR_NEXT.to_owned(),
            TOUR_BACK.to_owned(),
            TOUR_SKIP.to_owned(),
            TOUR_DONE.to_owned(),
            TOUR_CLOSE.to_owned(),
            GUIDE.to_owned(),
            tour_step_line(1, TOUR.len()),
        ];
        for step in &TOUR {
            all.push(step.title.to_owned());
            all.extend(step.keys.iter().chain(step.touch).map(|l| (*l).to_owned()));
        }
        all.extend(bunny_rules::TIER_NAMES.iter().map(|n| (*n).to_owned()));
        for n in 1..=bunny_rules::level::COUNT {
            all.push(garden_button(n, &bunny_rules::level::garden(n).name));
        }
        for text in all {
            assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
        }
        assert!(won_line(25).contains("as orange as it gets"));
        assert_eq!(time_line(83), "Time: 1:23");
    }
}
