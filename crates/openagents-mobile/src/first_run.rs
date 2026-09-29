//! The main menu (`SCR-01`) and the guided first run (`FLOW-01`: `SCR-02`
//! Choose your agent, the intro's end card, then the first-run chat).
//!
//! Chat with OpenAgents is the menu's one primary action; the starter chips
//! each open a new chat with a question sent. The next-step line comes from
//! the phone's own state: a run in progress, a result not yet added, a
//! check that confirmed your result, or the first step to take.
//!
//! The first run is three taps from a new install to a test starting:
//! **CHOOSE CODER**, **LET'S GO**, and the first-run chat's **START THE
//! TEST**. The furthest step reached is kept, so a relaunch reopens there,
//! and the menu shows only after the first result.

use serde::Serialize;

use crate::eval_cards::{Action, Button};
use crate::gym::{FirstRun, Gym, level_line, next_step};

/// The first-run chat's opening message: sent for the player, as a starter
/// chip would, so the tool card and its test come from the Gym's records.
pub(crate) const FIRST_MESSAGE: &str = "Test Project map on Coder";

/// The starter chips on the menu and on a new chat: `(id, label, message)`.
pub(crate) const STARTERS: &[(&str, &str, &str)] = &[
    ("test", "Test a tool", "Which tool should I try?"),
    ("news", "What's new", "What's new in the Gym?"),
    ("check", "Check a result", "Find me a result to check"),
];

/// First-time questions on a new chat, before any chat exists
/// (`SCR-15.E06`).
pub(crate) const FIRST_QUESTIONS: &[&str] = &[
    "Who are you?",
    "What can you do?",
    "What does it cost?",
    "What's the Gym?",
];

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

/// `SCR-01` Main menu.
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

/// A step of the first run before its chat.
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

/// Which screen the Chat tab shows now.
pub(crate) fn screen(gym: &Gym) -> &'static str {
    match gym.first_run() {
        FirstRun::Choose if !gym.asked_first() => "first_run",
        FirstRun::EndCard => "first_run",
        FirstRun::Choose | FirstRun::Chat => "chat",
        FirstRun::Done if gym.on_menu => "menu",
        FirstRun::Done => "chat",
    }
}

/// The first run's screen, when it shows one.
pub(crate) fn first_run(gym: &mut Gym) -> Option<FirstRunView> {
    match gym.first_run() {
        FirstRun::Choose if !gym.asked_first() => Some(FirstRunView {
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
            secondary: Some(gym.actions.button(
                "first.ask",
                "Ask OpenAgents a question first",
                None,
                Action::AskFirst,
            )),
        }),
        FirstRun::EndCard => Some(FirstRunView {
            step: "end_card",
            indicator: None,
            dot: 1,
            title: "Coder is ready.".into(),
            lines: vec![
                "Tools can make Coder better. Tests show whether they do.".into(),
                "Let's see if a tool makes Coder better.".into(),
            ],
            agent: None,
            next: "Next: we'll test a tool together in chat.".into(),
            primary: gym
                .actions
                .button("first.go", "LET'S GO", None, Action::LetsGo),
            secondary: None,
        }),
        _ => None,
    }
}

/// `SCR-01` Main menu, from the phone's state.
pub(crate) fn menu(gym: &mut Gym, app_build: Option<&str>) -> MenuView {
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
        Some(xp) => format!("Your work was checked. +{xp} XP."),
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
                    "test" => "test",
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
        primary_subtitle: "Test a tool, see what's new, earn XP".into(),
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
    fn a_new_install_opens_on_step_one() {
        let mut gym = Gym::empty();
        assert_eq!(screen(&gym), "first_run");
        let view = first_run(&mut gym).expect("step 1");
        assert_eq!(view.step, "choose");
        assert_eq!(view.primary.label, "CHOOSE CODER");
        assert_eq!(view.indicator.as_deref(), Some("STEP 1 OF 3"));
        // Asking first shows the chat; step 1 is still the step.
        gym.set_asked_first(true);
        assert_eq!(screen(&gym), "chat");
        gym.set_asked_first(false);
        assert_eq!(screen(&gym), "first_run");
    }

    #[test]
    fn the_menu_shows_no_number_it_hasnt_read() {
        let mut gym = Gym::empty();
        gym.set_first_run(FirstRun::Done);
        gym.on_menu = true;
        let menu = menu(&mut gym, Some("1.0.0 (21)"));
        assert_eq!(menu.player.level, None);
        assert_eq!(menu.player.xp_label, None);
        assert_eq!(menu.primary.label, "CHAT WITH OPENAGENTS");
        assert_eq!(
            menu.next,
            "Next: test a tool to see if it makes Coder better."
        );
        let labels: Vec<&str> = menu.chips.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Test a tool", "What's new", "Check a result"]);
        assert_eq!(menu.footer, "Gym open · v1.0.0 (21) · Playtest");
    }
}
