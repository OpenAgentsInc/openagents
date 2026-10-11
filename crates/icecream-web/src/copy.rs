//! Every word the game shows.

use icecream_rules::{Pickup, RunCard, Tool};

pub const TITLE: &str = "The Ice Cream Man";
pub const TAGLINE: &str =
    "You're stuck in an Airbnb that keeps going. Find three keys and get out before he scoops you.";
pub const NOTE: &str = "Check-out is at 11. Don't let him in.";
pub const PLAY: &str = "Play";
pub const HOW_TO_PLAY: &str = "How to play";
pub const MENU: &str = "Menu";
pub const PAUSED: &str = "Paused";
pub const RESUME: &str = "Resume";
pub const NEW_HOUSE: &str = "New house";
pub const TRY_AGAIN: &str = "Try again";
pub const PLAY_AGAIN: &str = "Play again";
pub const STUDIO: &str = "Back to Blue Rush";
pub const NO_WEBGL: &str =
    "This browser can't show the game. Try a recent Chrome, Safari, or Firefox.";
pub const HEALTH: &str = "Health";
pub const KEYS: &str = "Keys";
pub const ESCAPED: &str = "You escaped!";
pub const ESCAPED_LINE: &str = "You burst out into the sunshine. He waves sadly from the window.";
pub const SCOOPED: &str = "Scooped!";
pub const SCOOPED_LINE: &str = "He plopped you into a cone. Yuck.";
pub const SHIFT_LOCK_ON: &str = "Shift Lock on";
pub const SHIFT_LOCK_OFF: &str = "Shift Lock off";

/// The touch screen's buttons.
pub const JUMP: &str = "Jump";
pub const FIRE: &str = "Fire";
pub const SPRINT: &str = "Sprint";
pub const USE: &str = "Use";

/// Short lines that pop up during a run.
pub const FOUND_ALL: &str = "All three keys! Get to the front door.";
pub const FROZEN: &str = "He's frozen solid!";
pub const FART_HIT: &str = "Pfffft! He forgot where you are.";
pub const SPOTTED: &str = "He sees you! Run!";
pub const LOST: &str = "You lost him.";
pub const HID: &str = "Shh. You're hiding.";
pub const PULLED_OUT: &str = "He saw you hide in there!";
pub const LOCKED: &str = "Locked. Find all three keys first.";
pub const EMPTY_FUDGE: &str = "Out of fudge. Check the fridges.";
pub const EMPTY_FART: &str = "No fart clouds left.";
pub const JINGLE: &str = "Jingle jingle. He heard that.";
pub const UNLOCKING: &str = "Unlocking\u{2026}";

/// The tutorial: its own buttons and the link to the written guide.
pub const TOUR_NEXT: &str = "Next";
pub const TOUR_BACK: &str = "Back";
pub const TOUR_SKIP: &str = "Skip";
pub const TOUR_DONE: &str = "Start playing";
pub const GUIDE: &str = "Read the full guide";

#[must_use]
pub fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Fudge => "Hot Fudge",
        Tool::Fart => "Fart Launcher",
        Tool::Keys => "Key Ring",
    }
}

#[must_use]
pub fn key_found(count: u32) -> String {
    format!("You found a key! {count} of 3")
}

#[must_use]
pub fn pickup(pickup: Pickup) -> &'static str {
    match pickup {
        Pickup::Fudge => "More hot fudge!",
        Pickup::Fart => "One more fart cloud!",
    }
}

/// What the interact key does here, for the prompt over the hot bar.
#[must_use]
pub fn prompt(prompt: icecream_rules::Prompt) -> &'static str {
    use icecream_rules::Prompt;
    match prompt {
        Prompt::Hide => "Hide in the closet",
        Prompt::Leave => "Come out",
        Prompt::Fridge => "Open the fridge",
        Prompt::Unlock => "Hold to unlock the front door",
        Prompt::Locked => "The front door. Find all three keys",
    }
}

#[must_use]
pub fn time_line(seconds: u32) -> String {
    format!("Time: {}:{:02}", seconds / 60, seconds % 60)
}

/// The run card's lines.
#[must_use]
pub fn card_lines(card: &RunCard) -> Vec<String> {
    vec![
        time_line(card.seconds),
        format!("Keys: {} of 3", card.keys),
        format!("Hits taken: {}", card.hits_taken),
        match card.accuracy {
            Some(percent) => format!("Fudge accuracy: {percent}%"),
            None => "Fudge accuracy: no squirts fired".to_owned(),
        },
    ]
}

#[must_use]
pub fn tour_step_line(step: usize, steps: usize) -> String {
    format!("{step} of {steps}")
}

/// One step of the tutorial: a title, then its lines for a keyboard and
/// for a touch screen.
pub struct TourStep {
    pub title: &'static str,
    pub keys: &'static [&'static str],
    pub touch: &'static [&'static str],
}

/// The tutorial's four cards: move, look, shoot, find the keys.
pub const TOUR: [TourStep; 4] = [
    TourStep {
        title: "Move",
        keys: &[
            "Walk with W A S D or the arrow keys. Hold Shift to sprint.",
            "Space jumps. Jump over his sprinkles!",
        ],
        touch: &[
            "Drag the stick on the left to walk. Tap Sprint to run.",
            "Tap Jump to jump. Jump over his sprinkles!",
        ],
    },
    TourStep {
        title: "Look",
        keys: &[
            "Hold the right mouse button and drag to look around. Scroll to zoom in and out.",
            "Press Ctrl for Shift Lock: the camera sits over your shoulder with a crosshair for aiming.",
        ],
        touch: &[
            "Drag on the right side of the screen to look around.",
            "Your tools aim at him when he's in front of you.",
        ],
    },
    TourStep {
        title: "Shoot",
        keys: &[
            "Click to use your tool. Press 1 for Hot Fudge, 2 for the Fart Launcher, 3 for the Key Ring.",
            "Fudge slows him down, and enough of it freezes him. A fart cloud knocks him back and he forgets where you are.",
        ],
        touch: &[
            "Tap Fire to use your tool. Tap the bar at the bottom to switch tools.",
            "Fudge slows him down, and enough of it freezes him. A fart cloud knocks him back and he forgets where you are.",
        ],
    },
    TourStep {
        title: "Find the keys",
        keys: &[
            "Three check-out keys are hidden in the house. Walk over one to pick it up.",
            "Then hold E at the front door to escape. Press E by a closet to hide, or by a fridge for more ammo. Esc opens the menu.",
        ],
        touch: &[
            "Three check-out keys are hidden in the house. Walk over one to pick it up.",
            "Then hold Use at the front door to escape. Tap Use by a closet to hide, or by a fridge for more ammo.",
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use icecream_rules::Prompt;

    #[test]
    fn the_words_are_plain() {
        let mut all: Vec<String> = [
            TITLE,
            TAGLINE,
            NOTE,
            PLAY,
            HOW_TO_PLAY,
            MENU,
            PAUSED,
            RESUME,
            NEW_HOUSE,
            TRY_AGAIN,
            PLAY_AGAIN,
            STUDIO,
            NO_WEBGL,
            HEALTH,
            KEYS,
            ESCAPED,
            ESCAPED_LINE,
            SCOOPED,
            SCOOPED_LINE,
            SHIFT_LOCK_ON,
            SHIFT_LOCK_OFF,
            JUMP,
            FIRE,
            SPRINT,
            USE,
            FOUND_ALL,
            FROZEN,
            FART_HIT,
            SPOTTED,
            LOST,
            HID,
            PULLED_OUT,
            LOCKED,
            EMPTY_FUDGE,
            EMPTY_FART,
            JINGLE,
            UNLOCKING,
            TOUR_NEXT,
            TOUR_BACK,
            TOUR_SKIP,
            TOUR_DONE,
            GUIDE,
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        for tool in Tool::ALL {
            all.push(tool_name(tool).to_owned());
        }
        for p in [
            Prompt::Hide,
            Prompt::Leave,
            Prompt::Fridge,
            Prompt::Unlock,
            Prompt::Locked,
        ] {
            all.push(prompt(p).to_owned());
        }
        all.push(key_found(2));
        all.push(pickup(Pickup::Fudge).to_owned());
        all.push(pickup(Pickup::Fart).to_owned());
        all.extend(card_lines(&RunCard {
            escaped: true,
            seconds: 83,
            keys: 3,
            hits_taken: 4,
            accuracy: Some(62),
        }));
        for step in &TOUR {
            all.push(step.title.to_owned());
            all.extend(step.keys.iter().chain(step.touch).map(|l| (*l).to_owned()));
        }
        for text in &all {
            assert_eq!(oa_copy::violations(text, &[]), vec![], "{text}");
            assert!(!text.contains("Roblox"), "{text}");
        }
        assert_eq!(time_line(83), "Time: 1:23");
        assert!(
            card_lines(&RunCard {
                escaped: false,
                seconds: 5,
                keys: 1,
                hits_taken: 0,
                accuracy: None,
            })[3]
                .contains("no squirts")
        );
    }
}
