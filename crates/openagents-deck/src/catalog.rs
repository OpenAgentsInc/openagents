//! The decks the repository ships, by id and title: what a host lists or
//! a chat router chooses from. It needs none of the viewer's pieces, so a
//! crate built without the `viewer` feature (the chat worker) reads the
//! same list the desktop app opens from.

use crate::slide::{Deck, SCRIPTS};
use std::fmt;

/// A deck the repository ships: the id the viewer opens it by, and the
/// title a host shows for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckEntry {
    pub id: &'static str,
    pub title: String,
}

/// The decks the repository ships, in the order they are filed: the
/// default deck, `three-devdays-later`, first.
pub fn decks() -> Vec<DeckEntry> {
    SCRIPTS
        .iter()
        .map(|(id, _)| DeckEntry {
            id,
            title: Deck::named(id).map(|deck| deck.title()).unwrap_or_default(),
        })
        .collect()
}

/// A deck id the repository ships no script for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownDeck(pub String);

impl fmt::Display for UnknownDeck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let known: Vec<&str> = SCRIPTS.iter().map(|(id, _)| *id).collect();
        write!(
            f,
            "no deck named {}; the decks are: {}",
            self.0,
            known.join(", ")
        )
    }
}

impl std::error::Error for UnknownDeck {}
