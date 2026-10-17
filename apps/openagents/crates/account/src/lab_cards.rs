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
    /// A Model3d card's model (glTF binary), which the desktop turns in
    /// place of its poster.
    pub model: Option<String>,
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
        View::Model3d => Pictures {
            main: card.poster.clone(),
            strip: strip(),
            model: card.primary.clone(),
            ..Pictures::default()
        },
        View::Playable => Pictures {
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
    /// A glTF binary model, at most [`MAX_MODEL_BYTES`].
    Model(Vec<u8>),
    /// A file this row doesn't draw (a model, a build).
    Other,
}

/// The longest note a card row sends.
pub const MAX_NOTE_CHARS: usize = 4000;

/// Why a note wasn't added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteError {
    Empty,
    TooLong,
    /// The chat closed, the account changed, or the network failed.
    Unavailable,
    /// The Lab refused the note, in its own words.
    Refused(String),
}

impl NoteError {
    /// What the person sees.
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "Write a note first.".into(),
            Self::TooLong => "That note is too long.".into(),
            Self::Unavailable => "Couldn't add the note. Try again.".into(),
            Self::Refused(message) => message.clone(),
        }
    }
}

pub use lab_ui::{clock, note_at};

/// The largest picture a row downloads.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

/// The largest model a row downloads: the most a visual's asset may be.
pub const MAX_MODEL_BYTES: u64 = 8 * 1024 * 1024;

/// A Model3d card's model, by its name in a visual scene's assets.
pub const MODEL_ASSET: &str = "model.glb";

/// The visual scene a Model3d card's model turns in (#11948), `height`
/// points tall: the model is parsed, fitted, and lit as an agent's visual
/// would be, and dragging orbits it on the desktop and the phone alike.
pub fn model_script(height: u32) -> String {
    format!(
        "return ui.scene {{ height = {height}, orbit = true, draw = function(s)\n  \
         s:model {{ asset = \"{MODEL_ASSET}\", fit = 2 }}\nend }}\n"
    )
}

/// The marker line that starts a live session's Lab visual (#11948). The
/// rest of the line is a [`LiveCard`] as JSON; the script under it draws the
/// card's title for clients that don't know the marker.
pub const LIVE_MARKER: &str = "-- coder:lab_card ";

/// A typed card a Lab tool returned in a live session, and the space its
/// document lives in.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LiveCard {
    pub space: String,
    pub card: Card,
}

/// The card a visual script carries, if it is a live Lab card.
pub fn live_card_in_script(script: &str) -> Option<LiveCard> {
    let line = script.lines().next()?.strip_prefix(LIVE_MARKER)?;
    let live: LiveCard = serde_json::from_str(line).ok()?;
    (live.card.schema == lab_kit::card::SCHEMA
        && !live.space.is_empty()
        && live.card.version > 0
        && !live.card.document.is_empty())
    .then_some(live)
}

/// One version in a card's version list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionItem {
    pub number: u64,
    pub summary: String,
    pub created_unix_ms: u64,
}

impl VersionItem {
    /// What the list shows: `v3 · Thicker base`.
    pub fn label(&self) -> String {
        let summary = self.summary.trim();
        if summary.is_empty() {
            format!("v{}", self.number)
        } else {
            format!("v{} · {summary}", self.number)
        }
    }
}

/// The most versions a list shows, newest first.
pub const MAX_VERSIONS: usize = 50;
/// The most versions a list reads to find the newest ones.
const MAX_VERSIONS_READ: usize = 1000;

/// Why the version list didn't load.
pub const VERSIONS_UNAVAILABLE: &str = "Couldn't load the versions. Try again.";

/// A document's versions, newest first, at most [`MAX_VERSIONS`].
pub(crate) async fn read_versions(
    labs: &openagents::labs::Client,
    document: &openagents::labs::DocumentRef,
    cancel: &openagents::CancellationToken,
) -> Option<Vec<VersionItem>> {
    let mut all = Vec::new();
    let mut page = openagents::labs::PageOptions::default();
    loop {
        let list = labs.versions(document, &page, cancel).await.ok()?;
        all.extend(list.data.into_iter().map(|v| VersionItem {
            number: v.number,
            summary: v.summary,
            created_unix_ms: v.created_unix_ms,
        }));
        match list.next {
            Some(next) if next > page.after && all.len() < MAX_VERSIONS_READ => page.after = next,
            _ => break,
        }
    }
    all.sort_by(|a, b| b.number.cmp(&a.number));
    all.dedup_by_key(|v| v.number);
    all.truncate(MAX_VERSIONS);
    Some(all)
}

/// One of a card's files, read through `labs` in `space`. Only files the
/// card names are read.
pub(crate) async fn read_media(
    labs: &openagents::labs::Client,
    space: &str,
    card: &Card,
    artifact: &str,
    cancel: &openagents::CancellationToken,
) -> Option<Media> {
    if !artifacts(card).iter().any(|id| id == artifact) {
        return None;
    }
    let handle = labs.artifact(space, artifact, cancel).await.ok()?;
    let meta = handle.metadata();
    if meta.media_type.starts_with("video/") {
        return Some(
            handle
                .signed_url()
                .map_or(Media::Other, |u| Media::Video(u.into())),
        );
    }
    let model = meta.media_type == "model/gltf-binary";
    let limit = if model {
        MAX_MODEL_BYTES
    } else {
        MAX_IMAGE_BYTES
    };
    if !(model || meta.media_type.starts_with("image/")) || meta.size > limit {
        return Some(Media::Other);
    }
    let mut download = labs.download(&handle, None, cancel).await.ok()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = download.next_chunk(cancel).await.ok()? {
        bytes.extend(chunk);
        if bytes.len() as u64 > limit {
            return Some(Media::Other);
        }
    }
    Some(if model {
        Media::Model(bytes)
    } else {
        Media::Image(bytes)
    })
}

/// Adds a note to `version` of `document`, at `seconds` into its video
/// when given.
pub(crate) async fn post_note(
    labs: &openagents::labs::Client,
    document: &openagents::labs::DocumentRef,
    version: u64,
    seconds: Option<f64>,
    text: &str,
    cancel: &openagents::CancellationToken,
) -> Result<(), NoteError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NoteError::Empty);
    }
    if text.chars().count() > MAX_NOTE_CHARS {
        return Err(NoteError::TooLong);
    }
    let locator = match seconds {
        Some(seconds) if seconds.is_finite() && seconds >= 0.0 => {
            lab_kit::Locator::Time { seconds }
        }
        _ => lab_kit::Locator::None,
    };
    let request = labs
        .bind(lab_kit::api::AddNote {
            space: document.space().to_owned(),
            version,
            locator,
            text: text.to_owned(),
        })
        .map_err(|_| NoteError::Unavailable)?;
    labs.add_note(document, &request, cancel)
        .await
        .map(|_| ())
        .map_err(|e| match e.refusal() {
            Some(refusal) => NoteError::Refused(refusal.message.clone()),
            None => NoteError::Unavailable,
        })
}

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
    fn note_times_read_as_a_clock() {
        assert_eq!(clock(86.4), "1:26");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(
            note_at(&lab_kit::Locator::Time { seconds: 5.0 }).as_deref(),
            Some("0:05")
        );
        assert_eq!(note_at(&lab_kit::Locator::None), None);
        for e in [NoteError::Empty, NoteError::TooLong, NoteError::Unavailable] {
            assert!(oa_copy::violations(&e.message(), &[]).is_empty());
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
