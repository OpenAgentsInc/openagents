//! Every hotbar card in every zone fits its card and the screen: each line,
//! keys and details included, lies within the card's inner width, and the
//! card stays inside the window at the smallest sizes Verse draws at.

use super::everglade::{demolition, hotbar as everglade};
use super::grove::{hotbar as grove, kit::Spell, slots};
use super::water::hotbar as water;
use crate::tooltip::{self, Card, MARGIN, PAD_X};
use crate::ui::Atlas;

/// A small desktop window, a phone held upright, and a common laptop.
const SCREENS: [[f32; 2]; 3] = [[640.0, 480.0], [390.0, 844.0], [1280.0, 800.0]];

fn every_card() -> Vec<(String, Card)> {
    let mut cards = Vec::new();
    for index in 0..everglade::FULL_COUNT {
        if let Some(card) = everglade::card(index) {
            cards.push((format!("everglade {index}"), card));
        }
    }
    let mut index = 0;
    while let Some(card) = demolition::hotbar::card(index) {
        cards.push((format!("demolition {index}"), card));
        index += 1;
    }
    for index in 0..water::COUNT {
        cards.push((
            format!("water {index}"),
            water::card(index).expect("a card"),
        ));
    }
    // The Grove's 48 slots, each with every spell it may hold.
    for index in 0..slots::ROWS * slots::COLUMNS {
        for spell in Spell::ALL {
            cards.push((
                format!("grove {index} {spell:?}"),
                grove::card_of(spell, index),
            ));
        }
    }
    cards
}

#[test]
fn every_card_fits_its_lines_and_the_screen() {
    let atlas = Atlas::new(14.0);
    let cards = every_card();
    assert!(cards.len() > 3000);
    for screen in SCREENS {
        // Over a slot at the bottom middle and at either corner.
        for anchor in [
            [screen[0] * 0.5 - 18.0, screen[1] - 60.0, 36.0, 36.0],
            [4.0, screen[1] - 60.0, 36.0, 36.0],
            [screen[0] - 40.0, screen[1] - 60.0, 36.0, 36.0],
        ] {
            for (name, card) in &cards {
                let laid = tooltip::layout(&atlas, card, anchor, screen);
                let [x, y, w, h] = laid.rect;
                let inner = w - 2.0 * PAD_X;
                let widths = laid
                    .title
                    .iter()
                    .chain(&laid.body)
                    .map(|line| atlas.measure(line))
                    .chain(
                        laid.details
                            .iter()
                            .map(|line| line.iter().map(|(t, _)| atlas.measure(t)).sum()),
                    );
                for used in widths {
                    assert!(
                        used <= inner + 0.5,
                        "{name} on {screen:?}: {used} > {inner}"
                    );
                }
                assert!(
                    x >= MARGIN - 0.01 && x + w <= screen[0] - MARGIN + 0.01,
                    "{name} on {screen:?} runs off the side: {:?}",
                    laid.rect
                );
                assert!(
                    y >= 0.0 && y + h <= screen[1],
                    "{name} on {screen:?} runs off the top or bottom: {:?}",
                    laid.rect
                );
                // Above its slot, it never covers the bar.
                assert!(y + h <= anchor[1], "{name} on {screen:?} covers its slot");
            }
        }
    }
}
