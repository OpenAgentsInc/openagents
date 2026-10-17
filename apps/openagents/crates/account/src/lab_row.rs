//! A Lab card row that isn't tied to a saved chat (#11948): a card a Lab
//! tool returned in a live session, read through the signed-in account.
//!
//! The row reads only the card's own files, follows its document's
//! `/v1/labs` events while it is visible, adds notes at a playing time, and
//! lists the document's versions; picking one shows that version's card.
//! When the account changes or signs out, the row shows nothing and reads
//! nothing.

use std::sync::{Arc, Mutex};

use lab_kit::Card;
use openagents::CancellationToken;
use openagents::labs::{Client, DocumentRef};
use tokio::sync::watch;

use crate::Lease;
use crate::lab_cards::{LiveCard, Media, NoteError, VersionItem};

pub struct LabRow {
    lease: Lease,
    labs: Client,
    document: DocumentRef,
    card: Mutex<Card>,
    updates: watch::Sender<u64>,
}

impl LabRow {
    /// A row for `live`, read through `lease`. `None` when the lease is no
    /// longer the signed-in account's or the card names no valid document.
    pub fn open(lease: Lease, live: LiveCard) -> Option<Arc<Self>> {
        if !lease.is_current() {
            return None;
        }
        let labs = lease.client().labs(lease.session()).ok()?;
        let document = labs.document_ref(&live.space, &live.card.document).ok()?;
        let (updates, _) = watch::channel(0);
        Some(Arc::new(Self {
            lease,
            labs,
            document,
            card: Mutex::new(live.card),
            updates,
        }))
    }

    /// Whether the account that opened the row is still the signed-in one.
    pub fn current(&self) -> bool {
        self.lease.is_current() && !self.lease.cancel.is_cancelled()
    }

    /// The card the row shows; `None` once the account changed.
    pub fn card(&self) -> Option<Card> {
        self.current().then(|| self.card.lock().unwrap().clone())
    }

    /// Resolves once the account that opened the row signs out or changes.
    pub async fn closed(&self) {
        self.lease.cancel.cancelled().await;
    }

    /// The space the card's document lives in.
    pub fn space(&self) -> &str {
        self.document.space()
    }

    /// Bumped whenever the card changes.
    pub fn watch(&self) -> watch::Receiver<u64> {
        self.updates.subscribe()
    }

    /// The card's web page, on the account's own website.
    pub fn page(&self) -> Option<String> {
        let page = self.card()?.page?;
        let website = self.lease.client().deployment().website().clone();
        let url = website.join(&page).ok()?;
        (url.origin() == website.origin()).then(|| url.to_string())
    }

    /// One of the card's files: image or model bytes, or a signed link for
    /// a video.
    pub async fn media(&self, artifact: &str) -> Option<Media> {
        let card = self.card()?;
        let cancel = self.cancel();
        let media =
            crate::lab_cards::read_media(&self.labs, self.space(), &card, artifact, &cancel).await;
        self.current().then_some(media).flatten()
    }

    /// The document's versions, newest first.
    pub async fn versions(&self) -> Option<Vec<VersionItem>> {
        if !self.current() {
            return None;
        }
        let cancel = self.cancel();
        let versions = crate::lab_cards::read_versions(&self.labs, &self.document, &cancel).await;
        self.current().then_some(versions).flatten()
    }

    /// Shows version `number`; following then keeps that version current.
    /// False when it couldn't be read.
    pub async fn show_version(&self, number: u64) -> bool {
        if number == 0 || !self.current() {
            return false;
        }
        self.refresh(number, &self.cancel()).await
    }

    /// Adds a note to the shown version, at `seconds` into its video when
    /// given, then shows the card with it.
    pub async fn add_note(&self, seconds: Option<f64>, text: &str) -> Result<(), NoteError> {
        let version = self.card().ok_or(NoteError::Unavailable)?.version;
        let cancel = self.cancel();
        crate::lab_cards::post_note(&self.labs, &self.document, version, seconds, text, &cancel)
            .await?;
        self.refresh(version, &cancel).await;
        Ok(())
    }

    /// Follows the document until `stop` is cancelled or the account
    /// changes, keeping the shown version's state and notes current. Call
    /// it only while the row is visible.
    pub async fn follow(&self, stop: &CancellationToken) {
        let cancel = self.cancel();
        let work = async {
            let mut cursor = 0;
            loop {
                let page = self
                    .labs
                    .events_since(&self.document, cursor, 100, &cancel)
                    .await
                    .ok()?;
                if let Some(last) = page.data.last() {
                    cursor = cursor.max(last.cursor);
                }
                if page.next.is_none() || page.data.is_empty() {
                    break;
                }
            }
            self.refresh(self.shown()?, &cancel).await;
            let mut events = self.labs.events(&self.document, cursor).ok()?;
            while events.next(&cancel).await.is_ok() {
                self.refresh(self.shown()?, &cancel).await;
            }
            Some(())
        };
        tokio::select! {
            _ = stop.cancelled() => {}
            _ = cancel.cancelled() => {}
            _ = work => {}
        }
    }

    fn shown(&self) -> Option<u64> {
        self.card().map(|c| c.version)
    }

    fn cancel(&self) -> CancellationToken {
        self.lease.cancel.child_token()
    }

    async fn refresh(&self, version: u64, cancel: &CancellationToken) -> bool {
        let Ok(card) = self.labs.card(&self.document, version, cancel).await else {
            return false;
        };
        if !self.current() || card.document != self.document.id() || card.version != version {
            return false;
        }
        let mut shown = self.card.lock().unwrap();
        if *shown != card {
            *shown = card;
            self.updates.send_modify(|n| *n += 1);
        }
        true
    }
}
