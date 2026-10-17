//! The desktop film editor: shared video intents, version saves, and account-bound reads.

use crate::{Lease, lab_cards::Media};
use lab_kit::{
    Card, Change, JobStatus, Version, Want,
    api::{CreateJob, EditDocument},
};
use openagents::{
    CancellationToken,
    labs::{self, DocumentRef, EditResult},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// Shot and scene renders have no cut timeline to edit.
pub fn has_timeline(card: &Card) -> bool {
    card.kind == "video"
        && card.detail["shots"]
            .as_array()
            .is_some_and(|shots| !shots.is_empty())
}

#[derive(Clone, Debug)]
pub enum Gesture {
    Move { id: String, at: f64 },
    Link { id: String, shot: Option<String> },
    Speed { id: String, value: f64 },
    Text { id: String, value: String },
    Duration { id: String, value: f64 },
    Dissolve { id: String, enabled: bool },
}

impl Gesture {
    pub fn command(&self) -> Value {
        match self {
            Self::Move { id, at } => json!({"op":"move","id":id,"at":at}),
            Self::Link { id, shot } => {
                json!({"op":"link","id":id,"shot":shot.as_deref().unwrap_or("")})
            }
            Self::Speed { id, value } => json!({"op":"speed","id":id,"value":value}),
            Self::Text { id, value } => json!({"op":"text","id":id,"value":value}),
            Self::Duration { id, value } => json!({"op":"duration","id":id,"value":value}),
            Self::Dissolve { id, enabled } => json!({"op":"transition","id":id,"value":enabled}),
        }
    }

    fn summary(&self) -> &'static str {
        match self {
            Self::Move { .. } => "Move a timeline clip",
            Self::Link { shot: Some(_), .. } => "Link a timeline clip",
            Self::Link { shot: None, .. } => "Unlink a timeline clip",
            Self::Speed { .. } => "Change clip speed",
            Self::Text { .. } => "Edit caption text",
            Self::Duration { .. } => "Change caption length",
            Self::Dissolve { enabled: true, .. } => "Add a dissolve",
            Self::Dissolve { enabled: false, .. } => "Use a cut",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub id: String,
    pub track: String,
    pub label: String,
    pub at: f64,
    pub duration: f64,
    pub link: Option<String>,
    pub embedded: bool,
    pub speed: f64,
    pub dissolve: bool,
}

pub struct Preview {
    pub clips: Vec<Clip>,
    pub duration: f64,
}

/// A local preview never advances the saved version. A lost save response
/// requires a reload, so another gesture cannot overwrite an unknown outcome.
pub struct Timeline {
    version: Version,
    document: Value,
    view: Value,
    media: Value,
    pending: bool,
    pub requires_reload: bool,
    pub notice: Option<String>,
    pub playhead: f64,
    pub selected: Option<String>,
}

impl Timeline {
    pub fn new(version: Version, media: Value) -> Result<Self, String> {
        if version.schema != "cut.v1"
            || version.number == 0
            || lab_kit::digest(&version.content) != version.content_sha256
        {
            return Err("This video doesn't have an editable timeline.".into());
        }
        let view = video_lab::timeline::view(&version.content, &media)?;
        if !view["fps"]
            .as_f64()
            .is_some_and(|n| n.is_finite() && n > 0.)
            || !view["duration"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= 0.)
        {
            return Err("This film has invalid timing. Check it before editing.".into());
        }
        Ok(Self {
            document: version.content.clone(),
            version,
            view,
            media,
            pending: false,
            requires_reload: false,
            notice: None,
            playhead: 0.,
            selected: None,
        })
    }

    pub fn version(&self) -> &Version {
        &self.version
    }
    pub fn document(&self) -> &Value {
        &self.document
    }
    pub fn fps(&self) -> f64 {
        self.view["fps"].as_f64().unwrap_or(30.)
    }
    pub fn duration(&self) -> f64 {
        self.view["duration"].as_f64().unwrap_or(0.)
    }
    pub fn saving(&self) -> bool {
        self.pending
    }
    pub fn editable(&self) -> bool {
        !self.pending && !self.requires_reload
    }

    pub fn clips(&self) -> Vec<Clip> {
        Self::clips_in(&self.view)
    }

    fn clips_in(view: &Value) -> Vec<Clip> {
        view["clips"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| Clip {
                id: c["id"].as_str().unwrap_or_default().into(),
                track: c["track"].as_str().unwrap_or_default().into(),
                label: c["label"].as_str().unwrap_or_default().into(),
                at: c["at"].as_f64().unwrap_or(0.),
                duration: c["dur"].as_f64().unwrap_or(0.),
                link: c["link"].as_str().map(str::to_owned),
                embedded: c["embedded"] == true,
                speed: c["speed"].as_f64().unwrap_or(1.),
                dissolve: c["transition"].as_f64().unwrap_or(0.) > 0.,
            })
            .collect()
    }

    pub fn select(&mut self, id: &str) {
        if self.clips().iter().any(|c| c.id == id) {
            self.selected = Some(id.into());
        }
    }

    pub fn seek(&mut self, seconds: f64) {
        if seconds.is_finite() {
            self.playhead =
                ((seconds * self.fps()).round() / self.fps()).clamp(0., self.duration());
        }
    }

    pub fn step(&mut self, direction: f64, second: bool) {
        self.seek(self.playhead + direction * if second { 1. } else { 1. / self.fps() });
    }

    pub fn nudge(&self, direction: f64, second: bool) -> Option<Gesture> {
        let id = self.selected.as_ref()?;
        let clip = self.clips().into_iter().find(|clip| &clip.id == id)?;
        Some(Gesture::Move {
            id: id.clone(),
            at: (clip.at + direction * if second { 1. } else { 1. / self.fps() }).max(0.),
        })
    }

    pub fn begin(&mut self, space: &str, gesture: Gesture) -> Result<EditDocument, String> {
        let edited = self.project(&gesture)?;
        let command = gesture.command();
        self.document = edited["document"].clone();
        self.view = edited["timeline"].clone();
        self.pending = true;
        self.notice = None;
        Ok(EditDocument {
            space: space.into(),
            base: self.version.number,
            change: Change::Intent {
                intent: json!({"intent":"timeline","command":command,"media":self.media}),
            },
            summary: gesture.summary().into(),
        })
    }

    fn project(&self, gesture: &Gesture) -> Result<Value, String> {
        if !self.editable() {
            return Err("Wait for the save, or reload the film before editing.".into());
        }
        video_lab::timeline::request(&json!({"action":"edit",
            "document":self.version.content,"command":gesture.command(),"media":self.media}))
    }

    pub fn preview(&self, gesture: &Gesture) -> Result<Preview, String> {
        let edited = self.project(gesture)?;
        Ok(Preview {
            clips: Self::clips_in(&edited["timeline"]),
            duration: edited["timeline"]["duration"]
                .as_f64()
                .unwrap_or(self.duration()),
        })
    }

    pub fn finish(&mut self, result: EditResult) -> Result<(), String> {
        if !self.pending {
            return Err("Reload this film before editing it again.".into());
        }
        match result {
            EditResult::Created(version) => {
                if version.document != self.version.document
                    || version.parent != Some(self.version.number)
                    || self.version.number.checked_add(1) != Some(version.number)
                {
                    self.unknown();
                    return Err(
                        "We couldn't confirm the save. Reload the film before editing.".into(),
                    );
                }
                let mut next = match Self::new(version, self.media.clone()) {
                    Ok(next) => next,
                    Err(error) => {
                        self.unknown();
                        return Err(error);
                    }
                };
                next.selected = self.selected.clone();
                next.seek(self.playhead);
                *self = next;
            }
            EditResult::Stale => {
                self.restore();
                self.requires_reload = true;
                self.notice = Some("This film changed elsewhere. Reload it before editing.".into());
            }
            EditResult::Refused(refusal) => {
                self.restore();
                self.notice = Some(refusal.message);
            }
        }
        Ok(())
    }

    fn restore(&mut self) {
        self.document = self.version.content.clone();
        self.view = video_lab::timeline::view(&self.document, &self.media).unwrap_or_default();
        self.pending = false;
        self.seek(self.playhead);
    }

    pub fn unknown(&mut self) {
        self.restore();
        self.requires_reload = true;
        self.notice = Some("We couldn't confirm the save. Reload the film before editing.".into());
    }
}

#[derive(Clone)]
pub struct Source {
    lease: Lease,
    client: labs::Client,
    document: DocumentRef,
    cancel: CancellationToken,
    preview_request: Arc<Mutex<Option<(u64, Arc<labs::JobRequest>)>>>,
}

fn request_error(error: labs::Error) -> String {
    error
        .refusal()
        .map(|r| r.message.clone())
        .unwrap_or_else(|| "We couldn't reach this film. Try again.".into())
}

impl Source {
    pub(crate) fn new(lease: Lease, document: DocumentRef) -> Option<Self> {
        if !lease.is_current() {
            return None;
        }
        let client = lease.client().labs(lease.session()).ok()?;
        let cancel = lease.cancel.child_token();
        Some(Self {
            lease,
            client,
            document,
            cancel,
            preview_request: Arc::new(Mutex::new(None)),
        })
    }
    pub fn current(&self) -> bool {
        self.lease.is_current() && !self.cancel.is_cancelled()
    }
    pub fn close(&self) {
        self.cancel.cancel();
    }
    pub async fn closed(&self) {
        while self.current() {
            tokio::select! {
                _ = self.cancel.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        }
        self.close();
    }
    fn failed(&self, error: labs::Error) -> String {
        if error
            .status()
            .is_some_and(|status| matches!(status, 401 | 403))
        {
            self.close();
        }
        request_error(error)
    }
    fn document_failed(&self, error: labs::Error) -> String {
        if error.status() == Some(404) {
            self.close();
        }
        self.failed(error)
    }
    /// Each editor window cancels only its own pending reads and writes.
    pub fn fork(&self) -> Self {
        Self {
            cancel: self.lease.cancel.child_token(),
            ..self.clone()
        }
    }
    pub fn space(&self) -> &str {
        self.document.space()
    }
    pub fn page(&self, card: &Card) -> Option<String> {
        let website = self.lease.client().deployment().website();
        let url = website.join(card.page.as_deref()?).ok()?;
        (self.current() && url.origin() == website.origin()).then(|| url.to_string())
    }
    fn check(&self) -> Result<(), String> {
        self.current()
            .then_some(())
            .ok_or_else(|| "Sign in again to open this film.".into())
    }

    pub async fn load(&self) -> Result<(Timeline, Card), String> {
        self.check()?;
        let meta = self
            .client
            .document(&self.document, &self.cancel)
            .await
            .map_err(|error| self.document_failed(error))?;
        if meta.kind != "video" || meta.schema != "cut.v1" {
            return Err("This video doesn't have an editable timeline.".into());
        }
        let version = self
            .client
            .version(&self.document, meta.latest, &self.cancel)
            .await
            .map_err(|error| self.document_failed(error))?;
        let card = self
            .client
            .card(&self.document, version.number, &self.cancel)
            .await
            .map_err(|error| self.document_failed(error))?;
        self.check()?;
        Ok((Timeline::new(version, json!({}))?, card))
    }

    pub async fn save(&self, edit: EditDocument) -> Result<EditResult, String> {
        self.check()?;
        let request = self
            .client
            .bind(edit)
            .map_err(|error| self.document_failed(error))?;
        let result = self
            .client
            .edit(&self.document, &request, &self.cancel)
            .await
            .map_err(|error| self.document_failed(error))?;
        self.check()?;
        Ok(result)
    }

    pub async fn card(&self, version: u64) -> Result<Card, String> {
        self.check()?;
        let card = self
            .client
            .card(&self.document, version, &self.cancel)
            .await
            .map_err(|error| self.document_failed(error))?;
        self.check()?;
        Ok(card)
    }

    pub async fn media(&self, card: &Card, id: &str) -> Option<Media> {
        self.check().ok()?;
        if card.document != self.document.id()
            || !crate::lab_cards::artifacts(card).iter().any(|v| v == id)
        {
            return None;
        }
        let artifact = self
            .client
            .artifact(self.space(), id, &self.cancel)
            .await
            .map_err(|error| self.failed(error))
            .ok()?;
        let meta = artifact.metadata();
        if meta.media_type.starts_with("video/") {
            let video = artifact.signed_url().map(|url| Media::Video(url.into()));
            return self.current().then_some(video).flatten();
        }
        if !meta.media_type.starts_with("image/") || meta.size > crate::lab_cards::MAX_IMAGE_BYTES {
            return None;
        }
        let mut download = self
            .client
            .download(&artifact, None, &self.cancel)
            .await
            .map_err(|error| self.failed(error))
            .ok()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = download
            .next_chunk(&self.cancel)
            .await
            .map_err(|error| self.failed(error))
            .ok()?
        {
            bytes.extend(chunk);
        }
        self.current().then_some(Media::Image(bytes))
    }

    pub async fn render(
        &self,
        version: &Version,
        cards: tokio::sync::mpsc::Sender<Card>,
    ) -> Result<(), String> {
        self.check()?;
        if version.document != self.document.id() {
            return Err("Open this film again.".into());
        }
        let request = {
            let mut cached = self.preview_request.lock().unwrap();
            match cached
                .as_ref()
                .filter(|(number, _)| *number == version.number)
            {
                Some((_, request)) => request.clone(),
                None => {
                    let request = Arc::new(
                        self.client
                            .job_request(CreateJob {
                                space: self.space().into(),
                                version: version.reference(),
                                want: Want::Card,
                            })
                            .map_err(|error| self.failed(error))?,
                    );
                    *cached = Some((version.number, request.clone()));
                    request
                }
            }
        };
        let mut job = self
            .client
            .create_job(&request, &self.cancel)
            .await
            .map_err(|error| self.failed(error))?;
        let reference = self
            .client
            .job_ref(self.space(), &job.id)
            .map_err(|error| self.failed(error))?;
        let mut events = self
            .client
            .events(&self.document, 0)
            .map_err(|error| self.failed(error))?;
        let follow = async {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            tick.tick().await;
            loop {
                let card = self.card(version.number).await?;
                cards
                    .send(card)
                    .await
                    .map_err(|_| "The film editor was closed.".to_string())?;
                if job.status.finished() {
                    if job.status != JobStatus::Done {
                        let mut cached = self.preview_request.lock().unwrap();
                        if cached
                            .as_ref()
                            .is_some_and(|(_, previous)| Arc::ptr_eq(previous, &request))
                        {
                            *cached = None;
                        }
                    }
                    return if job.status == JobStatus::Done {
                        Ok(())
                    } else {
                        Err(job.message.unwrap_or_else(|| {
                            "The preview couldn't be rendered. Try again.".into()
                        }))
                    };
                }
                loop {
                    tokio::select! {
                        _ = tick.tick() => break,
                        event = events.next(&self.cancel) => { event.map_err(|error| self.failed(error))?; }
                    }
                }
                job = self
                    .client
                    .job(&reference, &self.cancel)
                    .await
                    .map_err(|error| self.failed(error))?;
                self.check()?;
            }
        };
        tokio::time::timeout(Duration::from_secs(3600), follow)
            .await
            .map_err(|_| {
                "The preview is still running. Reload this film to check it.".to_string()
            })?
    }
}

#[cfg(test)]
mod tests;
