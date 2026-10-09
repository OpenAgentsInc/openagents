//! What a chat suggests the person ask, one source for every surface: the
//! phone and desktop (`openagents-chat-app`) and the website
//! (`openagents-web`).
//!
//! - [`SUGGESTIONS`] and [`suggestions`]: a new chat's starter chips, the
//!   first [`SUGGESTIONS_SHOWN`] not used yet, then used ones, so a new chat
//!   always shows four.
//! - [`followups`]: the worker's suggested next questions under a prepared
//!   answer, less the ones already used.
//! - [`screen_offers`] and [`screen_label`]: the worker's typed
//!   `open_screen` offers that become routed chips, chosen from the reply's
//!   [`Offer`]s, never from its words.
//! - [`markers`]: the used marks a surface that keeps whole conversations
//!   (the website) derives from what it already stores.
//!
//! A suggestion's `id` is the prepared answer's bank id where it leads to
//! one. Tapping a chip sends its `message` like typed words; the worker's
//! router picks the prepared answer from the words and names it in the
//! reply's [`Meta::answer`], which marks that suggestion used.

use crate::basic_chats::{id_mark, suggestion_used, words_mark};
use crate::router::{Followup, Meta, Offer, Screen};

/// A suggested question above a new chat's field: `id` is stable (a
/// prepared answer's bank id where it leads to one, so the follow-up chip
/// for that answer counts as the same suggestion), `label` is what the chip
/// reads, and `message` what a tap sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub id: &'static str,
    pub label: &'static str,
    pub message: &'static str,
}

/// How many suggestions a new chat shows at once.
pub const SUGGESTIONS_SHOWN: usize = 4;

/// Every new chat's suggestions, in order (`SCR-15.E06`): exactly these
/// four, on the website, the phone, and the desktop app (owner,
/// 2026-10-09, #11095). Used ones move after the ones not used yet.
pub const SUGGESTIONS: &[Suggestion] = &[
    Suggestion {
        id: "meta.who",
        label: "What is OpenAgents?",
        message: "What is OpenAgents?",
    },
    Suggestion {
        id: "meta.model",
        label: "What models do you use?",
        message: "What models do you use?",
    },
    Suggestion {
        id: "meta.codebase",
        label: "How do I connect my codebase?",
        message: "How do I connect my codebase?",
    },
    Suggestion {
        id: "meta.plugins",
        label: "What are plugins?",
        message: "What are plugins?",
    },
];

/// The suggestion with this `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static Suggestion> {
    SUGGESTIONS.iter().find(|suggestion| suggestion.id == id)
}

/// Whether `suggestion` has been used: tapped (its ID), its prepared
/// answer shown, or its label's or message's words sent.
#[must_use]
pub fn used(suggestion: &Suggestion, used: &[String]) -> bool {
    suggestion_used(
        used,
        Some(suggestion.id),
        &[suggestion.label, suggestion.message],
    )
}

/// A new chat's suggestions: the ones not used yet come first; used ones
/// fill the rest, so a new chat always shows [`SUGGESTIONS_SHOWN`] (owner,
/// 2026-10-01).
pub fn suggestions(markers: &[String]) -> impl Iterator<Item = &'static Suggestion> + '_ {
    let fresh = |suggestion: &&Suggestion| !used(suggestion, markers);
    let all = SUGGESTIONS.iter();
    all.clone()
        .filter(fresh)
        .chain(all.filter(move |suggestion| !fresh(suggestion)))
        .take(SUGGESTIONS_SHOWN)
}

/// The reply's follow-ups not used yet, each with its index in the signed
/// reply's metadata. Tapping one sends its `label` as the person's words.
pub fn followups<'a>(
    meta: &'a Meta,
    used: &'a [String],
) -> impl Iterator<Item = (usize, &'a Followup)> + 'a {
    meta.followups.iter().enumerate().filter(|(_, followup)| {
        !suggestion_used(used, followup.answer.as_deref(), &[&followup.label])
    })
}

/// The reply's `open_screen` offers a surface shows as routed chips, each
/// with its index among the reply's offers and whether it is the
/// "connect a computer" form of [`Screen::Computers`]:
///
/// - never [`Screen::GymResult`] (the result card offers it), and
///   [`Screen::GymPublish`] only with a result to add (`has_result`);
/// - [`Screen::Computers`] reads as connecting when the person has no
///   computer (`no_computer`), and is left out then when the reply also
///   offers Coder (`run_offered`), whose own chip already connects one.
pub fn screen_offers(
    meta: &Meta,
    has_result: bool,
    no_computer: bool,
    run_offered: bool,
) -> impl Iterator<Item = (usize, Screen, bool)> + '_ {
    meta.offers
        .iter()
        .enumerate()
        .filter_map(move |(index, offer)| {
            let Offer::OpenScreen { screen } = offer else {
                return None;
            };
            let screen = *screen;
            if screen == Screen::GymResult || (screen == Screen::GymPublish && !has_result) {
                return None;
            }
            let connecting = screen == Screen::Computers && no_computer;
            if connecting && run_offered {
                return None;
            }
            Some((index, screen, connecting))
        })
}

/// What a routed chip for `screen` reads.
#[must_use]
pub fn screen_label(screen: Screen, connecting: bool) -> &'static str {
    match screen {
        Screen::Wallet => "Open Wallet",
        Screen::Computers if connecting => "Connect a computer",
        Screen::Computers => "Your computers",
        Screen::Keys => "Identity keys",
        Screen::Playtest => "Playtest",
        Screen::Report => "Report a problem",
        Screen::VerseGym => "See the board",
        Screen::GymResult => "See your result",
        Screen::GymPublish => "Add to the Gym",
        Screen::GymTestSet => "See the tests",
        Screen::RoutesMap => "Open the map",
    }
}

/// What the chip that connects a computer under a reply offering Coder
/// reads, when the person has none.
pub const CONNECT_LABEL: &str = "Connect a computer";

/// The part of a reply's [`Meta`] the chips and cards under it read: its
/// prepared answer, follow-ups, the offers to run Coder or open a screen,
/// and the plugins it shows as cards. A surface that stores replies keeps
/// this, not the router's whole record.
#[must_use]
pub fn chip_meta(meta: &Meta) -> Meta {
    Meta {
        answer: meta.answer.clone(),
        followups: meta.followups.clone(),
        plugins: meta.plugins.clone(),
        offers: meta
            .offers
            .iter()
            .filter(|offer| matches!(offer, Offer::RunCoder | Offer::OpenScreen { .. }))
            .cloned()
            .collect(),
        ..Meta::default()
    }
}

/// The used marks a surface derives from the conversations it keeps: the
/// words of every message the person `sent`, and every prepared `answer`
/// shown (`id@version`). The same marks the phone keeps as it goes
/// ([`crate::basic_chats::BasicChats::used_markers`]).
pub fn markers<'a>(
    sent: impl IntoIterator<Item = &'a str>,
    answers: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut marks: Vec<String> = vec![];
    for mark in sent
        .into_iter()
        .filter_map(words_mark)
        .chain(answers.into_iter().map(id_mark))
    {
        if !marks.contains(&mark) {
            marks.push(mark);
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_suggestions_come_first_and_used_ones_fill_to_four() {
        let shown: Vec<&str> = suggestions(&[]).map(|s| s.label).collect();
        assert_eq!(
            shown,
            [
                "What is OpenAgents?",
                "What models do you use?",
                "How do I connect my codebase?",
                "What are plugins?"
            ]
        );
        // Typed words, a tapped id, and a shown answer each count as used.
        let used = markers(["what is openagents"], ["meta.plugins@1"]);
        let shown: Vec<&str> = suggestions(&used).map(|s| s.id).collect();
        assert_eq!(
            shown,
            ["meta.model", "meta.codebase", "meta.who", "meta.plugins"]
        );
        // Everything used: still four, in order.
        let all: Vec<&str> = SUGGESTIONS.iter().map(|s| s.message).collect();
        let used = markers(all, []);
        assert_eq!(suggestions(&used).count(), SUGGESTIONS_SHOWN);
        assert_eq!(suggestions(&used).next().map(|s| s.id), Some("meta.who"));
        assert_eq!(
            find("meta.codebase").map(|s| s.label),
            Some("How do I connect my codebase?")
        );
    }

    #[test]
    fn followups_drop_used_answers_and_words() {
        let meta = Meta {
            followups: vec![
                Followup {
                    answer: Some("meta.model@v1".into()),
                    label: "What model is this?".into(),
                },
                Followup {
                    answer: None,
                    label: "Are you open source?".into(),
                },
                Followup {
                    answer: None,
                    label: "What does it cost?".into(),
                },
            ],
            ..Meta::default()
        };
        let used = markers(["are you open-source"], ["meta.model@v2"]);
        let left: Vec<usize> = followups(&meta, &used).map(|(i, _)| i).collect();
        assert_eq!(left, [2]);
    }

    #[test]
    fn routed_chips_come_from_typed_offers() {
        let meta = Meta {
            offers: vec![
                Offer::RunCoder,
                Offer::OpenScreen {
                    screen: Screen::Computers,
                },
                Offer::OpenScreen {
                    screen: Screen::GymResult,
                },
                Offer::OpenScreen {
                    screen: Screen::GymPublish,
                },
                Offer::OpenScreen {
                    screen: Screen::Wallet,
                },
            ],
            ..Meta::default()
        };
        let chosen: Vec<_> = screen_offers(&meta, false, true, false).collect();
        assert_eq!(
            chosen,
            [(1, Screen::Computers, true), (4, Screen::Wallet, false)]
        );
        // Coder's own chip connects a computer, so the screen chip goes.
        let chosen: Vec<_> = screen_offers(&meta, true, true, true).collect();
        assert_eq!(
            chosen,
            [(3, Screen::GymPublish, false), (4, Screen::Wallet, false)]
        );
        assert_eq!(screen_label(Screen::Computers, true), CONNECT_LABEL);
        assert_eq!(screen_label(Screen::Computers, false), "Your computers");
        // What a surface keeps of the reply.
        let mut full = meta.clone();
        full.answer = Some("account.computers@v1".into());
        full.route = Some("account".into());
        full.offers
            .push(Offer::OpenPresentation { deck: "x".into() });
        let kept = chip_meta(&full);
        assert_eq!(kept.offers, meta.offers);
        assert_eq!(kept.answer.as_deref(), Some("account.computers@v1"));
        assert_eq!(kept.route, None);
    }
}
