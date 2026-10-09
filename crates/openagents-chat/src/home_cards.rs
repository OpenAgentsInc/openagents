//! The feature cards a new chat shows, one list for every surface: the
//! phone's new chat (#11126) and the website's home (#11123).
//!
//! Each card is one headline and one line. A surface that links pages
//! opens [`HomeCard::href`]; the phone's **Try it** starts a chat with
//! [`HomeCard::message`], as a typed message would. Swap a card here and
//! every surface follows.

/// One feature card: `id` is stable, `title` and `line` are what the card
/// reads, `href` is the website's page for it, and `message` is what a
/// phone's **Try it** sends, unless `opens_verse`: then **Try it** takes
/// the phone straight into the Verse's Grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeCard {
    pub id: &'static str,
    pub title: &'static str,
    pub line: &'static str,
    pub href: &'static str,
    pub message: &'static str,
    pub opens_verse: bool,
}

/// Every new chat's feature cards, in order.
pub const HOME_CARDS: &[HomeCard] = &[
    HomeCard {
        id: "verse",
        title: "Explore the Verse",
        line: "A shared world where people and their agents meet.",
        href: "/docs/verse",
        message: "What is the Verse?",
        opens_verse: true,
    },
    HomeCard {
        id: "coder",
        title: "Meet Coder",
        line: "An agent that writes code on your own computer.",
        href: "/docs/coder",
        message: "What is Coder and how do I start?",
        opens_verse: false,
    },
    HomeCard {
        id: "codebase",
        title: "Tour the codebase",
        line: "Everything we build is open source. Take a look around.",
        href: "https://github.com/OpenAgentsInc/openagents",
        message: "Give me a tour of the OpenAgents codebase.",
        opens_verse: false,
    },
    HomeCard {
        id: "roadmap",
        title: "See the roadmap",
        line: "What we're building next, and when it ships.",
        href: "/roadmap",
        message: "What's on the OpenAgents roadmap?",
        opens_verse: false,
    },
];

/// The card with this `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static HomeCard> {
    HOME_CARDS.iter().find(|card| card.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_cards_with_unique_ids_and_short_words() {
        assert_eq!(HOME_CARDS.len(), 4);
        for card in HOME_CARDS {
            assert_eq!(find(card.id), Some(card));
            assert!(card.title.len() <= 24, "{}", card.title);
            assert!(card.line.len() <= 64, "{}", card.line);
            assert!(!card.message.is_empty());
            assert_eq!(card.opens_verse, card.id == "verse");
            assert!(card.href.starts_with('/') || card.href.starts_with("https://"));
        }
        let mut ids: Vec<_> = HOME_CARDS.iter().map(|card| card.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), HOME_CARDS.len());
    }
}
