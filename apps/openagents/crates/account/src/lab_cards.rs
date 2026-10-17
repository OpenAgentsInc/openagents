//! Lab cards in saved chats (#11883): the transcript row that shows one, and
//! which of its files each standard view draws.
//!
//! A saved message's typed cards (`/v1/history`, `lab.card.v1`) become
//! visual parts whose path is an opaque `lab-card:` reference, so the
//! transcript inserts one row per card through its visual row. The row draws
//! `lab-ui`'s Rust Native card; this module holds the parts that need no UI.

use lab_kit::{Card, View};

/// The path prefix of a Lab card row.
pub const SCHEME: &str = "lab-card:";

/// The opaque transcript reference for a message's `index`th card. It holds
/// no URL, space or document name.
pub fn card_path(generation: u64, message: &str, index: usize) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(format!("{generation}:{message}:{index}").as_bytes());
    let hex: String = hash.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("{SCHEME}{hex}")
}

pub fn is_card_path(path: &str) -> bool {
    path.starts_with(SCHEME)
}

/// The pictures a card's view draws, as artifact ids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pictures {
    /// The large picture: an image, or the poster of a video, model or game.
    pub main: Option<String>,
    /// A compare's two sides, in order.
    pub pair: Option<(Option<String>, Option<String>)>,
    /// Frames shown in a row under the main picture.
    pub strip: Vec<String>,
    /// What plays in place of the main picture (a video, or one side of a
    /// compare); the desktop plays only videos.
    pub plays: Option<String>,
}

/// What the desktop draws for each standard view. A report draws no picture.
pub fn pictures(card: &Card) -> Pictures {
    let strip = || card.strip.iter().take(24).cloned().collect();
    match card.view.drawn() {
        View::Image => Pictures {
            main: card.primary.clone().or_else(|| card.poster.clone()),
            strip: strip(),
            ..Pictures::default()
        },
        View::Video => Pictures {
            main: card.poster.clone(),
            strip: strip(),
            plays: card.primary.clone(),
            ..Pictures::default()
        },
        View::Compare => match &card.compare {
            Some(c) if c.a_primary.is_some() || c.b_primary.is_some() => Pictures {
                pair: Some((c.a_primary.clone(), c.b_primary.clone())),
                plays: card.primary.clone(),
                ..Pictures::default()
            },
            _ => Pictures {
                main: card.poster.clone(),
                plays: card.primary.clone(),
                ..Pictures::default()
            },
        },
        View::Model3d | View::Playable => Pictures {
            main: card.poster.clone(),
            strip: strip(),
            ..Pictures::default()
        },
        View::Report | View::None | View::Unknown => Pictures::default(),
    }
}

/// Every file a card names. A row may fetch only these.
pub fn artifacts(card: &Card) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    let compare = card
        .compare
        .iter()
        .flat_map(|c| [c.a_primary.clone(), c.b_primary.clone()]);
    for id in [card.primary.clone(), card.poster.clone()]
        .into_iter()
        .chain(compare)
        .flatten()
        .chain(card.strip.iter().cloned())
    {
        if !all.contains(&id) {
            all.push(id);
        }
    }
    all
}

/// A file a card row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Media {
    /// Image bytes, at most [`MAX_IMAGE_BYTES`].
    Image(Vec<u8>),
    /// A video, by a signed link that carries no account credential.
    Video(String),
    /// A file this row doesn't draw (a model, a build).
    Other,
}

/// The largest picture a row downloads.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> Vec<(String, Card)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../crates/lab-kit/fixtures/cards");
        let mut all: Vec<(String, Card)> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| {
                let card: Card = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                (p.file_stem().unwrap().to_string_lossy().into_owned(), card)
            })
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }

    #[test]
    fn every_fixture_card_draws_its_view() {
        let cards = fixtures();
        assert!(cards.len() >= 8);
        for (name, card) in &cards {
            let drawn = lab_ui::card(card, "me", "test", 1).unwrap();
            let json = String::from_utf8(drawn.view.to_json().unwrap()).unwrap();
            assert!(!json.contains("/v1/"), "{name}");
            let state = card.state();
            let words: Vec<&str> = [card.title.as_str(), card.summary.as_str()]
                .into_iter()
                .chain(state.status.as_deref())
                .chain(state.refusal.as_deref())
                .collect();
            for text in words {
                assert!(oa_copy::violations(text, &[]).is_empty(), "{name}: {text}");
            }
            let pictures = pictures(card);
            let ids = artifacts(card);
            for id in pictures
                .main
                .iter()
                .chain(pictures.strip.iter())
                .chain(pictures.plays.iter())
                .chain(
                    pictures
                        .pair
                        .iter()
                        .flat_map(|(a, b)| a.iter().chain(b.iter())),
                )
            {
                assert!(ids.contains(id), "{name}: {id}");
            }
            match name.as_str() {
                "image" => assert_eq!(pictures.main.as_deref(), Some("art_photo_v2")),
                "compare" => {
                    assert_eq!(
                        pictures.pair,
                        Some((Some("art_render_v2".into()), Some("art_render_v3".into())))
                    );
                    assert_eq!(pictures.plays.as_deref(), Some("art_side_by_side"));
                }
                "video" => assert!(pictures.plays.is_some()),
                "report-refused" | "unknown-view" => {
                    assert_eq!(pictures, Pictures::default());
                    assert!(drawn.media.is_empty(), "{name}");
                }
                "model3d" => assert!(pictures.plays.is_none()),
                _ => {}
            }
        }
    }

    #[test]
    fn card_paths_are_opaque_and_stable() {
        let a = card_path(3, "msg_1", 0);
        assert!(is_card_path(&a));
        assert_eq!(a, card_path(3, "msg_1", 0));
        assert_ne!(a, card_path(3, "msg_1", 1));
        assert_ne!(a, card_path(4, "msg_1", 0));
        assert!(!a.contains("msg_1"));
        assert!(!is_card_path("/tmp/x-visual.luau"));
    }
}
