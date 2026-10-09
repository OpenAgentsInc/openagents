//! The Gym menu (`SCR-01`) and the Gym intro (`FLOW-01`: `SCR-02` Choose
//! your agent, the intro's end card, then the intro's chat).
//!
//! Chat first: a new install opens on a chat with OpenAgents, with the tab
//! bar, and nothing of the Gym is volunteered. The Gym is opt-in: **Train
//! Coder**, from the Verse's Gym board or from Account, opens the intro.
//! The intro is three taps to a test starting: **CHOOSE CODER**, **LET'S
//! GO**, and the intro chat's **START THE TEST**. The furthest step reached
//! is kept, so a relaunch reopens there, and the Gym menu is reachable from
//! the chat's header after the first result.
//!
//! Chat with OpenAgents is the menu's one primary action; the starter
//! chips each open a new chat with a question sent. The next-step line
//! comes from the phone's own state: a run in progress, a result not yet
//! added, a check that confirmed your result, or the first step to take.

use serde::Serialize;

use crate::eval_cards::{Action, Button};
use crate::gym::{FirstRun, Gym, level_line, next_step};

/// The intro chat's opening message: sent for the player, as a starter
/// chip would, so the capability card and its test come from the Gym's
/// records.
pub const FIRST_MESSAGE: &str = "Which plugins can I test?";

/// The Gym's starter chips on the Gym menu: `(id, label, message)`. A new
/// chat's suggestions are [`SUGGESTIONS`].
pub const STARTERS: &[(&str, &str, &str)] = &[
    ("news", "What's new", "What's new in the Gym?"),
    ("check", "Check a result", "Find me a result to check"),
];

/// A new chat's suggestions, their order, and what a tap sends: one list
/// for the phone, the desktop and the website
/// ([`openagents_chat::suggestions`]).
pub use openagents_chat::suggestions::{SUGGESTIONS, SUGGESTIONS_SHOWN, Suggestion};

/// The player card on the menu (`SCR-01.E04`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Player {
    pub name: String,
    /// `None` while the ledger is read: the host draws a gray bar, never 0.
    pub level: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xp_label: Option<String>,
    pub bar_value: u64,
    pub bar_max: u64,
}

/// A big row on the menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    pub button: Button,
    pub subtitle: String,
    /// `person`, `globe`.
    pub glyph: &'static str,
}

/// `SCR-01` The Gym menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MenuView {
    pub player: Player,
    /// The hero's status pill.
    pub status: String,
    /// `SCR-01.E11`.
    pub next: String,
    /// `SCR-01.E12`, and its subtitle.
    pub primary: Button,
    pub primary_subtitle: String,
    /// `SCR-01.E13`.
    pub chips: Vec<Button>,
    pub rows: Vec<Row>,
    /// `SCR-01.E10`.
    pub footer: String,
}

/// A step of the Gym intro before its chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FirstRunView {
    /// `choose` or `end_card`.
    pub step: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indicator: Option<String>,
    /// Which of three dots is lit.
    pub dot: u8,
    pub title: String,
    pub lines: Vec<String>,
    /// The agent card on `SCR-02`: its name and one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<(String, String)>,
    pub next: String,
    pub primary: Button,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<Button>,
}

/// Which screen the Chat tab shows now: the chat until the person opts
/// into the Gym; then the intro's steps, and the menu when the person
/// opened it.
pub fn screen(gym: &Gym) -> &'static str {
    if !gym.opted_in() {
        return "chat";
    }
    match gym.first_run() {
        FirstRun::Choose | FirstRun::EndCard => "first_run",
        FirstRun::Chat => "chat",
        FirstRun::Done if gym.on_menu => "menu",
        FirstRun::Done => "chat",
    }
}

/// The intro's screen, when it shows one.
pub fn first_run(gym: &mut Gym) -> Option<FirstRunView> {
    if !gym.opted_in() {
        return None;
    }
    match gym.first_run() {
        FirstRun::Choose => Some(FirstRunView {
            step: "choose",
            indicator: Some("STEP 1 OF 3".into()),
            dot: 1,
            title: "Choose your agent".into(),
            lines: vec![
                "Your agent is an AI that writes code.".into(),
                "You'll train it to get better.".into(),
            ],
            agent: Some(("CODER".into(), "Writes and fixes code.".into())),
            next: "Next: choose Coder to begin.".into(),
            primary: gym
                .actions
                .button("first.choose", "CHOOSE CODER", None, Action::ChooseCoder),
            secondary: Some(
                gym.actions
                    .button("first.later", "Not now", None, Action::NotNow),
            ),
        }),
        FirstRun::EndCard => Some(FirstRunView {
            step: "end_card",
            indicator: None,
            dot: 1,
            title: "Coder is ready.".into(),
            lines: vec![
                "Plugins can make Coder better. Tests show whether they do.".into(),
                "Let's see if a plugin makes Coder better.".into(),
            ],
            agent: None,
            next: "Next: we'll test a plugin together in chat.".into(),
            primary: gym
                .actions
                .button("first.go", "LET'S GO", None, Action::LetsGo),
            secondary: None,
        }),
        _ => None,
    }
}

/// `SCR-01` The Gym menu, from the phone's state.
pub fn menu(gym: &mut Gym, app_build: Option<&str>) -> MenuView {
    let standing = gym.standing.clone();
    let player = Player {
        name: if standing.name.is_empty() {
            "Trainer".into()
        } else {
            standing.name.clone()
        },
        level: standing.read.then_some(standing.level),
        xp_label: standing
            .read
            .then(|| format!("{} / {} XP", standing.xp, standing.next_at)),
        bar_value: standing.xp.saturating_sub(standing.level_at),
        bar_max: standing.next_at.saturating_sub(standing.level_at).max(1),
    };
    let next = match gym.new_credit() {
        // New XP can come from a check of your result, your own check
        // confirmed, or an adoption: say it earned, not how.
        Some(xp) => format!("Your work earned XP. +{xp} XP."),
        None => next_step(gym).to_owned(),
    };
    let primary = gym.actions.button(
        "menu.chat",
        "CHAT WITH OPENAGENTS",
        Some("ask"),
        Action::Chat,
    );
    let chips = STARTERS
        .iter()
        .map(|(id, label, message)| {
            gym.actions.button(
                format!("menu.{id}"),
                label,
                Some(match *id {
                    "news" => "news",
                    _ => "check",
                }),
                Action::Say {
                    text: (*message).to_owned(),
                    fresh: true,
                },
            )
        })
        .collect();
    let profile_line = if standing.read {
        level_line(&standing)
    } else {
        "Level, XP, what you made".into()
    };
    let rows = vec![
        Row {
            button: gym
                .actions
                .button("menu.profile", "PROFILE", None, Action::Profile),
            subtitle: profile_line,
            glyph: "person",
        },
        Row {
            button: gym.actions.button(
                "menu.verse",
                "THE GYM IN THE VERSE",
                None,
                Action::VerseGym,
            ),
            subtitle: "See every result on the boards".into(),
            glyph: "globe",
        },
    ];
    MenuView {
        player,
        status: "GYM OPEN".into(),
        next,
        primary,
        primary_subtitle: "See what's new, check results, earn XP".into(),
        chips,
        rows,
        footer: match app_build {
            Some(build) => format!("Gym open · v{build} · Playtest"),
            None => "Gym open · Playtest".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_install_opens_on_the_chat_and_train_coder_opens_the_intro() {
        let mut gym = Gym::empty();
        assert_eq!(screen(&gym), "chat");
        assert!(first_run(&mut gym).is_none());
        // Train Coder: the intro, at step 1.
        gym.opt_in();
        assert_eq!(screen(&gym), "first_run");
        let view = first_run(&mut gym).expect("step 1");
        assert_eq!(view.step, "choose");
        assert_eq!(view.primary.label, "CHOOSE CODER");
        assert_eq!(
            view.secondary.as_ref().map(|b| b.label.as_str()),
            Some("Not now")
        );
        assert_eq!(view.indicator.as_deref(), Some("STEP 1 OF 3"));
        // Not now: the chat again, and no intro until the next Train Coder.
        gym.opt_out();
        assert_eq!(screen(&gym), "chat");
        assert!(first_run(&mut gym).is_none());
        // The intro keeps its furthest step across an opt-out.
        gym.opt_in();
        gym.set_first_run(FirstRun::EndCard);
        gym.opt_out();
        gym.opt_in();
        assert_eq!(first_run(&mut gym).expect("end card").step, "end_card");
    }

    #[test]
    fn the_menu_shows_no_number_it_hasnt_read() {
        let mut gym = Gym::empty();
        gym.set_start("done");
        gym.on_menu = true;
        assert_eq!(screen(&gym), "menu");
        let menu = menu(&mut gym, Some("1.0.0 (21)"));
        assert_eq!(menu.player.level, None);
        assert_eq!(menu.player.xp_label, None);
        assert_eq!(menu.primary.label, "CHAT WITH OPENAGENTS");
        assert_eq!(menu.next, "Next: ask what's new in the Gym.");
        let labels: Vec<&str> = menu.chips.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["What's new", "Check a result"]);
        assert_eq!(menu.footer, "Gym open · v1.0.0 (21) · Playtest");
    }
}
