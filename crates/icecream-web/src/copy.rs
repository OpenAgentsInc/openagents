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
/// The link home, to openagents.com.
pub const HOME: &str = "OpenAgents home";
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
pub const FIRST_PERSON_ON: &str = "First person. Press V for third person.";
pub const THIRD_PERSON_ON: &str = "Third person. Press V for first person.";
pub const MELTED: &str = "You melted him!";
pub const MELTED_LINE: &str = "He's a puddle of vanilla on the carpet. The house is yours.";
pub const KEEP_PLAYING: &str = "Keep playing with the Sprinkle Staff";
pub const FREE_ROAM: &str = "You got the Sprinkle Staff! Click to make your own minions.";
pub const MAN_HEALTH: &str = "Ice Cream Man";

/// The touch screen's buttons.
pub const JUMP: &str = "Jump";
pub const FIRE: &str = "Fire";
pub const SPRINT: &str = "Sprint";
pub const USE: &str = "Use";
/// Switches first and third person, like V.
pub const VIEW: &str = "View";
pub const FIRST_PERSON_TOUCH: &str = "First person. Tap View for third person.";
pub const THIRD_PERSON_TOUCH: &str = "Third person. Tap View for first person.";
/// The button at the end of the hot bar that picks the next tool.
pub const NEXT_ITEM: &str = "Next";
pub const NEXT_ITEM_LABEL: &str = "Next item";

/// The menu's settings: each button says what it is now.
pub const TIMER_ON: &str = "Timer: on";
pub const TIMER_OFF: &str = "Timer: off";
pub const JUMPSCARES_ON: &str = "Jumpscares: on";
pub const JUMPSCARES_OFF: &str = "Jumpscares: off";
pub const SPRINT_HOLD: &str = "Sprint button: hold";
pub const SPRINT_TAP: &str = "Sprint button: tap";
pub const NEW_BEST: &str = "New best!";

/// The escape card's time, to a tenth of a second.
#[must_use]
pub fn your_time(clock: &str) -> String {
    format!("Your time: {clock}")
}

/// The best escape time kept in this browser.
#[must_use]
pub fn best_time(clock: &str) -> String {
    format!("Best: {clock}")
}

/// The build's version, small at the foot of the menu.
#[must_use]
pub fn version_line(version: &str) -> String {
    format!("Version {version}")
}

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
pub const WEAPONS_HINT: &str = "Four weapons are hiding in here. Melt minions, freeze him, and press the sprinkle buttons in order.";
pub const MINION_CAME: &str = "A minion is coming for you!";
pub const MINION_MELTED: &str = "Minion melted!";
pub const MINION_BITE: &str = "Ow! A minion bit you.";
pub const CONE_DROPPED: &str = "It dropped an ice cream cone! Eat it.";
pub const CONE_EATEN: &str = "Yum! 50% more health for a while.";
pub const BOOST_OVER: &str = "The cone wore off.";
pub const PUZZLE_WRONG: &str = "Wrong order. The buttons went dark.";
pub const PUZZLE_SOLVED: &str = "Click! The cone cabinet opened.";
pub const CABINET_LOCKED: &str =
    "Locked. Press the three sprinkle buttons in order: pink, blue, yellow.";
pub const MAN_HIT: &str = "Got him!";
pub const FRIENDS: &str = "Your minions are here!";
pub const EMPTY_CONES: &str = "Out of cones. Check the fridges.";
pub const EMPTY_SUGAR: &str = "Out of sugar. Check the fridges.";

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
        Tool::Sword => "Candy Cane Sword",
        Tool::Spear => "Waffle Cone Spear",
        Tool::Pistol => "Cone Pistol",
        Tool::Sugar => "Sugar Launcher",
        Tool::Staff => "Sprinkle Staff",
    }
}

/// How each weapon was earned, when it is.
#[must_use]
pub fn earned(tool: Tool) -> &'static str {
    match tool {
        Tool::Sword => "The minion dropped a Candy Cane Sword!",
        Tool::Spear => "You got the Waffle Cone Spear! It reaches a long way.",
        Tool::Pistol => "You froze him! Here's a Cone Pistol.",
        Tool::Sugar => "You melted enough minions! Here's a Sugar Launcher.",
        Tool::Staff => FREE_ROAM,
        Tool::Fudge | Tool::Fart | Tool::Keys => "",
    }
}

/// The puzzle's buttons, in the order they're pressed.
pub const BUTTON_NAMES: [&str; 3] = ["pink", "blue", "yellow"];

#[must_use]
pub fn button_lit(count: u32) -> String {
    format!("Button {count} of 3 is lit.")
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
        Prompt::Button => "Press the sprinkle button",
        Prompt::Cabinet => "The cone cabinet",
    }
}

#[must_use]
pub fn time_line(seconds: u32) -> String {
    format!("Time: {}:{:02}", seconds / 60, seconds % 60)
}

/// The run card's lines.
#[must_use]
pub fn card_lines(card: &RunCard) -> Vec<String> {
    let mut lines = card_lines_base(card);
    if card.minions_melted > 0 {
        lines.push(format!("Minions melted: {}", card.minions_melted));
    }
    lines
}

fn card_lines_base(card: &RunCard) -> Vec<String> {
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
            "Drag the stick on the left to walk. Hold Sprint to run. You can make it a tap in the menu.",
            "Tap Jump to jump. Jump over his sprinkles!",
        ],
    },
    TourStep {
        title: "Look",
        keys: &[
            "Hold the right mouse button and drag to look around. Scroll to zoom in and out.",
            "Press Ctrl for Shift Lock: the camera sits over your shoulder with a crosshair for aiming. Press V to switch between first person and third person.",
        ],
        touch: &[
            "Drag on the right side of the screen to look around.",
            "Your tools aim at him when he's in front of you. Tap View to switch between first person and third person.",
        ],
    },
    TourStep {
        title: "Shoot",
        keys: &[
            "Click to use your tool. Press 1 for Hot Fudge, 2 for the Fart Launcher, 3 for the Key Ring, and 4 and up for weapons you earn. Q picks the next one.",
            "Fudge slows him down, and enough of it freezes him. A fart cloud knocks him back and he forgets where you are.",
        ],
        touch: &[
            "Tap Fire to use your tool. To switch tools, tap one in the bar at the bottom, swipe the bar, or tap Next.",
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
            HOME,
            NEXT_ITEM,
            NEXT_ITEM_LABEL,
            TIMER_ON,
            TIMER_OFF,
            JUMPSCARES_ON,
            JUMPSCARES_OFF,
            SPRINT_HOLD,
            SPRINT_TAP,
            NEW_BEST,
            NO_WEBGL,
            HEALTH,
            KEYS,
            ESCAPED,
            ESCAPED_LINE,
            SCOOPED,
            SCOOPED_LINE,
            SHIFT_LOCK_ON,
            SHIFT_LOCK_OFF,
            FIRST_PERSON_ON,
            THIRD_PERSON_ON,
            MELTED,
            MELTED_LINE,
            KEEP_PLAYING,
            FREE_ROAM,
            MAN_HEALTH,
            WEAPONS_HINT,
            MINION_CAME,
            MINION_MELTED,
            MINION_BITE,
            CONE_DROPPED,
            CONE_EATEN,
            BOOST_OVER,
            PUZZLE_WRONG,
            PUZZLE_SOLVED,
            CABINET_LOCKED,
            MAN_HIT,
            FRIENDS,
            EMPTY_CONES,
            EMPTY_SUGAR,
            JUMP,
            FIRE,
            SPRINT,
            USE,
            VIEW,
            FIRST_PERSON_TOUCH,
            THIRD_PERSON_TOUCH,
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
            all.push(earned(tool).to_owned());
        }
        all.push(button_lit(2));
        all.push(your_time("0:26.4"));
        all.push(best_time("0:24.1"));
        all.push(version_line("3f9a2c1b"));
        for text in &all {
            assert!(!text.contains("Blue Rush"), "no studio name: {text}");
        }
        all.retain(|t| !t.is_empty());
        for p in [
            Prompt::Hide,
            Prompt::Leave,
            Prompt::Fridge,
            Prompt::Unlock,
            Prompt::Locked,
            Prompt::Button,
            Prompt::Cabinet,
        ] {
            all.push(prompt(p).to_owned());
        }
        all.push(key_found(2));
        all.push(pickup(Pickup::Fudge).to_owned());
        all.push(pickup(Pickup::Fart).to_owned());
        all.extend(card_lines(&RunCard {
            escaped: true,
            melted: false,
            minions_melted: 3,
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
                melted: false,
                minions_melted: 0,
                seconds: 5,
                keys: 1,
                hits_taken: 0,
                accuracy: None,
            })[3]
                .contains("no squirts")
        );
    }
}
