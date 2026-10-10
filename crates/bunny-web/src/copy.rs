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
            score_line(1200),
        ];
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
