//! The feature cards the website's home shows (#11123).
//!
//! Each card is one headline and one line, and links its page,
//! [`HomeCard::href`]. The phone's new chat no longer shows them: it is a
//! clean composer (#11126, owner review of the launch film, 2026-10-13).

/// One feature card: `id` is stable, `title` and `line` are what the card
/// reads, and `href` is the website's page for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeCard {
    pub id: &'static str,
    pub title: &'static str,
    pub line: &'static str,
    pub href: &'static str,
}

/// The website home's feature cards, in order.
pub const HOME_CARDS: &[HomeCard] = &[
    HomeCard {
        id: "verse",
        title: "Explore the Verse",
        line: "A shared world where people and their agents meet.",
        href: "/docs/verse",
    },
    HomeCard {
        id: "coder",
        title: "Meet Coder",
        line: "An agent that writes code on your own computer.",
        href: "/docs/coder",
    },
    HomeCard {
        id: "codebase",
        title: "Tour the codebase",
        line: "This site and our apps are open source. Take a look around.",
        href: "https://github.com/OpenAgentsInc/openagents",
    },
    HomeCard {
        id: "roadmap",
        title: "See the roadmap",
        line: "What we're building next, and when it ships.",
        href: "/roadmap",
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
            assert!(card.href.starts_with('/') || card.href.starts_with("https://"));
        }
        let mut ids: Vec<_> = HOME_CARDS.iter().map(|card| card.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), HOME_CARDS.len());
    }
}
