//! A small facade over a saved chat's Lab cards (#11884): the files a card
//! row shows, its page, its notes, and following its document while the row
//! is visible. Everything goes through the signed-in account's saved-history
//! reader, which reads only a shown card's own files.
//!
//! Live sessions' Lab cards (#11948) go through the same calls under
//! `lab-live:` keys: each is a [`LabRow`] that reads through the same
//! account, until it signs out. Either kind lists its document's versions.
use super::{CoreError, CoreResult, CoreSavedHistory};
use crate::layout::lab::LabMediaView;
use openagents_account::lab_cards::{LiveCard, Media, VersionItem, pictures};
use openagents_account::lab_row::LabRow;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// What a Lab card's media cell draws, as artifact ids the cell passes to
/// `lab_media`.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LabCardFiles {
    pub title: String,
    pub view: Option<LabMediaView>,
    /// The large picture: an image, or a video's, model's, or game's poster.
    pub main: Option<String>,
    /// A compare's two sides, and their version labels.
    pub side_a: Option<String>,
    pub side_b: Option<String>,
    pub side_labels: Vec<String>,
    /// Frames shown in a row under the picture.
    pub strip: Vec<String>,
    /// The video that plays in place of the picture.
    pub plays: Option<String>,
    /// A Model3d card's model, which turns in place of its poster.
    pub model: Option<String>,
    /// The card has a page to open (models, games, reports).
    pub has_page: bool,
}

/// One version in a card's version list, newest first.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LabVersion {
    pub number: u64,
    /// What the list shows: `v3 · Thicker base`.
    pub label: String,
    /// The version the row shows now.
    pub shown: bool,
}

/// One of a card's files.
#[derive(Clone, Debug, PartialEq, uniffi::Enum)]
pub enum LabMediaFile {
    Image {
        bytes: Vec<u8>,
    },
    /// A signed link that carries no account credential.
    Video {
        url: String,
    },
    /// A glTF binary model, for `TranscriptView::lab_model`.
    Model {
        bytes: Vec<u8>,
    },
    /// A file the phone doesn't draw (a build).
    Other,
    /// Not available: the chat closed, the account changed, or it failed.
    Missing,
}

impl From<Option<Media>> for LabMediaFile {
    fn from(media: Option<Media>) -> Self {
        match media {
            Some(Media::Image(bytes)) => Self::Image { bytes },
            Some(Media::Video(url)) => Self::Video { url },
            Some(Media::Model(bytes)) => Self::Model { bytes },
            Some(Media::Other) => Self::Other,
            None => Self::Missing,
        }
    }
}

pub(crate) fn files(card: &lab_kit::Card, has_page: bool) -> LabCardFiles {
    let p = pictures(card);
    let (side_a, side_b) = p.pair.clone().unwrap_or_default();
    let side_labels = match (&p.pair, &card.compare) {
        (Some(_), Some(c)) => vec![format!("v{}", c.a.version), format!("v{}", c.b.version)],
        _ => Vec::new(),
    };
    LabCardFiles {
        title: card.title.clone(),
        view: match card.view.drawn() {
            lab_kit::View::Video => Some(LabMediaView::Video),
            lab_kit::View::Image => Some(LabMediaView::Image),
            lab_kit::View::Compare => Some(LabMediaView::Compare),
            lab_kit::View::Model3d => Some(LabMediaView::Model3d),
            lab_kit::View::Playable => Some(LabMediaView::Playable),
            _ => None,
        },
        main: p.main,
        side_a,
        side_b,
        side_labels,
        strip: p.strip,
        plays: p.plays,
        model: p.model,
        has_page,
    }
}

fn versions(items: Vec<VersionItem>, shown: u64) -> Vec<LabVersion> {
    items
        .into_iter()
        .map(|v| LabVersion {
            number: v.number,
            label: v.label(),
            shown: v.number == shown,
        })
        .collect()
}

fn is_live(card: &str) -> bool {
    card.starts_with(crate::layout::lab::LIVE_PREFIX)
}

/// The cards a transcript shows, for the layout.
pub(crate) fn layout_cards(
    reader: &openagents_account::reader::Reader,
) -> crate::layout::lab::LabCards {
    std::sync::Arc::new(
        reader
            .lab_cards()
            .into_iter()
            .map(|(path, saved)| {
                let space = saved.document_ref().space().to_owned();
                (
                    path,
                    crate::layout::lab::LabCardInput {
                        space,
                        card: saved.card,
                    },
                )
            })
            .collect(),
    )
}

/// Which cards are on screen, and the followers running for them.
#[derive(Default)]
pub(crate) struct LabFollow {
    visible: Mutex<HashSet<String>>,
    running: Mutex<HashMap<String, CancellationToken>>,
    /// Live sessions' cards, by `lab-live:` key.
    live: Mutex<HashMap<String, Arc<LabRow>>>,
}

impl LabFollow {
    /// Starts a follower for each visible card when `active`, and stops
    /// every follower otherwise.
    pub(crate) fn sync(
        &self,
        reader: &std::sync::Arc<openagents_account::reader::Reader>,
        active: bool,
    ) {
        let visible = self.visible.lock().unwrap().clone();
        let mut running = self.running.lock().unwrap();
        running.retain(|card, stop| {
            let keep = active && visible.contains(card) && !stop.is_cancelled();
            if !keep {
                stop.cancel();
            }
            keep
        });
        if !active {
            return;
        }
        for card in visible {
            if running.contains_key(&card) {
                continue;
            }
            let stop = CancellationToken::new();
            if is_live(&card) {
                let Some(row) = self.live(&card) else {
                    continue;
                };
                running.insert(card, stop.clone());
                openagents_client::runtime::shared().spawn(async move {
                    row.follow(&stop).await;
                    stop.cancel();
                });
                continue;
            }
            running.insert(card.clone(), stop.clone());
            let reader = reader.clone();
            openagents_client::runtime::shared().spawn(async move {
                reader.follow_lab(&card, &stop).await;
                stop.cancel();
            });
        }
    }

    pub(crate) fn set_visible(&self, card: String, visible: bool) {
        let mut set = self.visible.lock().unwrap();
        if visible {
            set.insert(card);
        } else {
            set.remove(&card);
        }
    }

    /// A live card's row, while its account can still read it.
    pub(crate) fn live(&self, key: &str) -> Option<Arc<LabRow>> {
        let mut live = self.live.lock().unwrap();
        let row = live.get(key)?.clone();
        if row.current() {
            Some(row)
        } else {
            live.remove(key);
            None
        }
    }

    pub(crate) fn stop(&self) {
        self.live.lock().unwrap().clear();
        self.visible.lock().unwrap().clear();
        for (_, stop) in self.running.lock().unwrap().drain() {
            stop.cancel();
        }
    }

    #[cfg(test)]
    pub(crate) fn running(&self) -> usize {
        self.running.lock().unwrap().len()
    }
}

impl CoreSavedHistory {
    /// A row for a live session's Lab card under `key`, read through this
    /// history's account. `None` when it is signed out or the card names no
    /// readable document.
    pub(crate) fn open_live(&self, key: &str, live: LiveCard) -> Option<Arc<LabRow>> {
        if !self.current_view() || !is_live(key) {
            return None;
        }
        if let Some(row) = self.labs().live(key) {
            return Some(row);
        }
        let row = LabRow::open(self.lease.clone(), live)?;
        self.labs()
            .live
            .lock()
            .unwrap()
            .insert(key.to_owned(), row.clone());
        Some(row)
    }

    fn live_row(&self, card: &str) -> Option<Arc<LabRow>> {
        if !self.current_view() {
            return None;
        }
        self.labs().live(card)
    }
}

#[uniffi::export]
impl CoreSavedHistory {
    /// The files a shown Lab card's media cell draws.
    pub fn lab_card(&self, card: String) -> Option<LabCardFiles> {
        if !self.current_view() {
            return None;
        }
        if is_live(&card) {
            let row = self.live_row(&card)?;
            return Some(files(&row.card()?, row.page().is_some()));
        }
        let saved = self.reader().lab_card(&card)?;
        Some(files(&saved.card, self.reader().lab_page(&card).is_some()))
    }

    /// The web page a shown Lab card opens, on the chat's own website.
    pub fn lab_page(&self, card: String) -> Option<String> {
        if is_live(&card) {
            return self.live_row(&card)?.page();
        }
        self.current_view()
            .then(|| self.reader().lab_page(&card))
            .flatten()
    }

    /// One of a shown Lab card's files.
    pub async fn lab_media(&self, card: String, artifact: String) -> LabMediaFile {
        if !self.current_view() {
            return LabMediaFile::Missing;
        }
        if is_live(&card) {
            let Some(row) = self.live_row(&card) else {
                return LabMediaFile::Missing;
            };
            return openagents_client::runtime::shared()
                .spawn(async move { row.media(&artifact).await })
                .await
                .ok()
                .flatten()
                .into();
        }
        let reader = self.reader().clone();
        openagents_client::runtime::shared()
            .spawn(async move { reader.lab_media(&card, &artifact).await })
            .await
            .ok()
            .flatten()
            .into()
    }

    /// Adds a note to a shown Lab card, at `seconds` into its video when it
    /// plays.
    pub async fn lab_add_note(
        &self,
        card: String,
        seconds: Option<f64>,
        text: String,
    ) -> CoreResult<()> {
        if !self.current_view() {
            return Err(CoreError::Auth {
                message: "Sign in again to add a note.".into(),
            });
        }
        let reader = self.reader().clone();
        let live = is_live(&card).then(|| self.live_row(&card));
        openagents_client::runtime::shared()
            .spawn(async move {
                match live {
                    Some(Some(row)) => row.add_note(seconds, &text).await,
                    Some(None) => Err(openagents_account::lab_cards::NoteError::Unavailable),
                    None => reader.lab_add_note(&card, seconds, &text).await,
                }
            })
            .await
            .map_err(|_| CoreError::Network {
                message: "Couldn't add the note. Try again.".into(),
            })?
            .map_err(|e| CoreError::InvalidArgument {
                message: e.message(),
            })
    }

    /// Whether a Lab card row is on screen. A visible card follows its
    /// document while the app is in the foreground, so its state and notes
    /// stay current.
    pub fn lab_visible(&self, card: String, visible: bool) {
        self.labs().set_visible(card, visible);
        self.sync_labs();
    }

    /// A shown Lab card's document versions, newest first.
    pub async fn lab_versions(&self, card: String) -> CoreResult<Vec<LabVersion>> {
        let unavailable = || CoreError::Network {
            message: openagents_account::lab_cards::VERSIONS_UNAVAILABLE.into(),
        };
        if !self.current_view() {
            return Err(unavailable());
        }
        let reader = self.reader().clone();
        let live = is_live(&card).then(|| self.live_row(&card));
        openagents_client::runtime::shared()
            .spawn(async move {
                match live {
                    Some(Some(row)) => {
                        let shown = row.card()?.version;
                        Some(versions(row.versions().await?, shown))
                    }
                    Some(None) => None,
                    None => {
                        let shown = reader.lab_card(&card)?.card.version;
                        Some(versions(reader.lab_versions(&card).await?, shown))
                    }
                }
            })
            .await
            .ok()
            .flatten()
            .ok_or_else(unavailable)
    }

    /// Shows version `number` in a shown Lab card's row; following then
    /// keeps that version current. False when it couldn't be read.
    pub async fn lab_show_version(&self, card: String, number: u64) -> bool {
        if !self.current_view() {
            return false;
        }
        let reader = self.reader().clone();
        let live = is_live(&card).then(|| self.live_row(&card));
        openagents_client::runtime::shared()
            .spawn(async move {
                match live {
                    Some(Some(row)) => row.show_version(number).await,
                    Some(None) => false,
                    None => reader.lab_show_version(&card, number).await,
                }
            })
            .await
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> Vec<(String, lab_kit::Card)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../crates/lab-kit/fixtures/cards");
        let mut all: Vec<(String, lab_kit::Card)> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| {
                let card = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                (p.file_stem().unwrap().to_string_lossy().into_owned(), card)
            })
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }

    #[test]
    fn every_fixture_card_names_only_its_own_files() {
        for (name, card) in fixtures() {
            let f = files(&card, card.page.is_some());
            let own = openagents_account::lab_cards::artifacts(&card);
            for id in f
                .main
                .iter()
                .chain(f.side_a.iter())
                .chain(f.side_b.iter())
                .chain(f.strip.iter())
                .chain(f.plays.iter())
            {
                assert!(own.contains(id), "{name}: {id}");
            }
            match name.as_str() {
                "video" => {
                    assert_eq!(f.view, Some(LabMediaView::Video));
                    assert_eq!(f.plays.as_deref(), Some("art_render_v3"));
                    assert_eq!(f.main.as_deref(), Some("art_poster_v3"));
                    assert_eq!(f.strip.len(), 2);
                }
                "compare" => {
                    assert_eq!(f.view, Some(LabMediaView::Compare));
                    assert_eq!(f.side_a.as_deref(), Some("art_render_v2"));
                    assert_eq!(f.side_b.as_deref(), Some("art_render_v3"));
                    assert_eq!(f.side_labels.len(), 2);
                }
                "image" => assert_eq!(f.view, Some(LabMediaView::Image)),
                "model3d" => {
                    assert_eq!(f.view, Some(LabMediaView::Model3d));
                    assert!(f.plays.is_none());
                }
                "playable" => assert_eq!(f.view, Some(LabMediaView::Playable)),
                "report-refused" | "unknown-view" => {
                    assert_eq!(f.view, None);
                    assert!(f.main.is_none() && f.plays.is_none());
                }
                _ => {}
            }
        }
    }

    #[test]
    fn followers_run_only_while_active_and_stop_on_sign_out() {
        let reader = openagents_account::reader::Reader::new();
        let follow = LabFollow::default();
        follow.set_visible("lab-card:a".into(), true);
        follow.sync(&reader, false);
        assert_eq!(follow.running(), 0);
        follow.sync(&reader, true);
        assert_eq!(follow.running(), 1);
        // Backgrounded: every follower stops.
        follow.sync(&reader, false);
        assert_eq!(follow.running(), 0);
        follow.sync(&reader, true);
        follow.set_visible("lab-card:a".into(), false);
        follow.sync(&reader, true);
        assert_eq!(follow.running(), 0);
        follow.set_visible("lab-card:b".into(), true);
        follow.sync(&reader, true);
        follow.stop();
        assert_eq!(follow.running(), 0);
        follow.sync(&reader, true);
        assert_eq!(follow.running(), 0, "sign-out forgets the visible cards");
    }

    #[test]
    fn media_files_map_to_cells() {
        assert_eq!(LabMediaFile::from(None), LabMediaFile::Missing);
        assert_eq!(
            LabMediaFile::from(Some(Media::Video("https://x.invalid/v".into()))),
            LabMediaFile::Video {
                url: "https://x.invalid/v".into()
            }
        );
        assert_eq!(LabMediaFile::from(Some(Media::Other)), LabMediaFile::Other);
        assert_eq!(
            LabMediaFile::from(Some(Media::Model(vec![1, 2]))),
            LabMediaFile::Model { bytes: vec![1, 2] }
        );
    }

    #[test]
    fn a_model3d_card_names_its_model_and_lists_versions_newest_first() {
        let (_, card) = fixtures()
            .into_iter()
            .find(|(n, _)| n == "model3d")
            .unwrap();
        let f = files(&card, false);
        assert!(f.model.is_some());
        assert!(
            openagents_account::lab_cards::artifacts(&card).contains(f.model.as_ref().unwrap())
        );
        let list = versions(
            vec![
                VersionItem {
                    number: 3,
                    summary: "Thicker base".into(),
                    created_unix_ms: 0,
                },
                VersionItem {
                    number: 2,
                    summary: " ".into(),
                    created_unix_ms: 0,
                },
            ],
            2,
        );
        assert_eq!(list[0].label, "v3 · Thicker base");
        assert_eq!(list[1].label, "v2");
        assert!(!list[0].shown && list[1].shown);
    }

    #[test]
    fn live_keys_need_a_live_row() {
        assert!(is_live(&crate::layout::lab::live_key(
            "chat/v1-visual.luau"
        )));
        assert!(!is_live("lab-card:1:m1:0"));
        let follow = LabFollow::default();
        assert!(follow.live("lab-live:x").is_none());
        // A visible live card without a row starts no follower.
        let reader = openagents_account::reader::Reader::new();
        follow.set_visible("lab-live:x".into(), true);
        follow.sync(&reader, true);
        assert_eq!(follow.running(), 0);
    }
}
