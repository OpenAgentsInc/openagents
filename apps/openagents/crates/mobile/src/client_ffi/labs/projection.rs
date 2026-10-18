//! Account-fenced Lab cards and their registered media for native adapters.
use super::{CoreError, CoreResult, CoreSavedHistory, LabCardFiles, LabMediaFile, files, is_live};
use lab_ui::{Intent, Projection};
use rust_native::Activation;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

const MAX_VIEW: usize = 512 * 1024;
const MAX_IMAGE: usize = 16 * 1024 * 1024;
const MAX_MODEL: usize = openagents_account::lab_cards::MAX_MODEL_BYTES as usize;

#[derive(Clone, uniffi::Record)]
pub struct LabSurfaceRegistration {
    pub resource: String,
    pub files: LabCardFiles,
}
#[derive(Clone, uniffi::Record)]
pub struct LabCardProjection {
    pub json: Vec<u8>,
    pub surfaces: Vec<LabSurfaceRegistration>,
}
#[derive(Clone, uniffi::Enum)]
pub enum LabCardEffect {
    OpenPage { url: String },
}
struct Document {
    projection: Projection,
    fingerprint: String,
    space: String,
    shown: bool,
    closed: bool,
}
impl Document {
    fn new(card: &lab_kit::Card, space: &str, instance: &str) -> CoreResult<Self> {
        let projection = lab_ui::card(card, space, instance, 1).map_err(|_| unavailable())?;
        let this = Self {
            projection,
            fingerprint: fingerprint(card)?,
            space: space.into(),
            shown: false,
            closed: false,
        };
        this.snapshot()?;
        Ok(this)
    }
    fn snapshot(&self) -> CoreResult<LabCardProjection> {
        if self.closed {
            return Err(unavailable());
        }
        let json = self.projection.view.to_json().map_err(|_| unavailable())?;
        if json.len() > MAX_VIEW || self.projection.media.len() > 8 {
            return Err(unavailable());
        }
        let mut surfaces = Vec::new();
        for (resource, media) in &self.projection.media {
            let files = files(&media.card, media.card.page.is_some());
            let ids = artifacts(&files);
            if ids.len() > 64
                || files.strip.len() > 32
                || files.title.len() > 8192
                || files.side_labels.iter().any(|label| label.len() > 256)
                || ids
                    .iter()
                    .any(|id| id.is_empty() || id.len() > 512 || id.chars().any(char::is_control))
            {
                return Err(unavailable());
            }
            surfaces.push(LabSurfaceRegistration {
                resource: resource.clone(),
                files,
            });
        }
        Ok(LabCardProjection { json, surfaces })
    }
    fn matches(&self, card: &lab_kit::Card) -> bool {
        !self.closed && fingerprint(card).is_ok_and(|value| value == self.fingerprint)
    }
    fn identity(&self, instance: &str, revision: u64) -> bool {
        !self.closed && self.projection.view.view().instance == instance && revision == 1
    }
    fn acknowledge(&mut self, instance: &str, revision: u64) -> CoreResult<()> {
        if !self.identity(instance, revision) {
            return Err(unavailable());
        }
        self.shown = true;
        Ok(())
    }
    fn permits(&self, instance: &str, revision: u64, resource: &str, artifact: &str) -> bool {
        self.identity(instance, revision)
            && self.projection.media.get(resource).is_some_and(|media| {
                artifacts(&files(&media.card, media.card.page.is_some())).contains(artifact)
            })
    }
    fn returned_media(
        &self,
        current: bool,
        instance: &str,
        revision: u64,
        resource: &str,
        artifact: &str,
        value: LabMediaFile,
    ) -> LabMediaFile {
        if !current || !self.permits(instance, revision, resource, artifact) {
            LabMediaFile::Missing
        } else {
            bounded(value)
        }
    }
    fn activate(&self, event: &Activation) -> CoreResult<Intent> {
        if !self.shown || self.closed {
            return Err(unavailable());
        }
        self.projection
            .view
            .activate(event)
            .cloned()
            .map_err(|_| unavailable())
    }
    fn close(&mut self) {
        self.closed = true;
        self.shown = false;
        self.projection.media.clear();
    }
}
fn artifacts(files: &LabCardFiles) -> BTreeSet<String> {
    files
        .main
        .iter()
        .chain(files.side_a.iter())
        .chain(files.side_b.iter())
        .chain(files.strip.iter())
        .chain(files.plays.iter())
        .chain(files.model.iter())
        .cloned()
        .collect()
}
fn fingerprint(card: &lab_kit::Card) -> CoreResult<String> {
    let value = serde_json::to_value(card).map_err(|_| unavailable())?;
    Ok(lab_kit::digest(&value))
}
fn unavailable() -> CoreError {
    CoreError::NotFound {
        message: "This card changed. Open it again to continue.".into(),
    }
}
fn bounded(media: LabMediaFile) -> LabMediaFile {
    match media {
        LabMediaFile::Image { ref bytes } if bytes.len() > MAX_IMAGE => LabMediaFile::Missing,
        LabMediaFile::Model { ref bytes } if bytes.len() > MAX_MODEL => LabMediaFile::Missing,
        LabMediaFile::Video { ref url } if url.len() > 4096 => LabMediaFile::Missing,
        value => value,
    }
}
impl CoreSavedHistory {
    fn projection_card(&self, key: &str) -> Option<(String, lab_kit::Card)> {
        if !self.current_view() {
            return None;
        }
        if is_live(key) {
            let row = self.live_row(key)?;
            Some((row.space().into(), row.card()?))
        } else {
            let saved = self.reader().lab_card(key)?;
            Some((saved.document_ref().space().into(), saved.card))
        }
    }
}
#[derive(uniffi::Object)]
pub struct CoreLabCard {
    history: Arc<CoreSavedHistory>,
    key: String,
    document: Mutex<Document>,
}
#[uniffi::export]
impl CoreSavedHistory {
    /// Read only an existing account-scoped saved or live Lab card.
    pub fn lab_projection(self: &Arc<Self>, card: String) -> CoreResult<Arc<CoreLabCard>> {
        let (space, value) = self.projection_card(&card).ok_or_else(unavailable)?;
        let document = Document::new(&value, &space, &format!("lab-{}", uuid::Uuid::new_v4()))?;
        Ok(Arc::new(CoreLabCard {
            history: self.clone(),
            key: card,
            document: Mutex::new(document),
        }))
    }
}
impl CoreLabCard {
    fn current(&self) -> CoreResult<()> {
        let (space, card) = self
            .history
            .projection_card(&self.key)
            .ok_or_else(unavailable)?;
        let document = self.document.lock().unwrap();
        if document.space == space && document.matches(&card) {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
}
#[uniffi::export]
impl CoreLabCard {
    /// Check before publishing work decoded asynchronously by the native shell.
    pub fn is_current(&self) -> bool {
        self.current().is_ok()
    }
    pub fn snapshot(&self) -> CoreResult<LabCardProjection> {
        self.current()?;
        self.document.lock().unwrap().snapshot()
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> CoreResult<()> {
        self.current()?;
        self.document
            .lock()
            .unwrap()
            .acknowledge(&instance, revision)
    }
    /// Resource ids resolve only inside this frozen card, never as URLs or paths.
    pub async fn media(
        &self,
        instance: String,
        revision: u64,
        resource: String,
        artifact: String,
    ) -> LabMediaFile {
        if self.current().is_err()
            || !self
                .document
                .lock()
                .unwrap()
                .permits(&instance, revision, &resource, &artifact)
        {
            return LabMediaFile::Missing;
        }
        let value = self
            .history
            .lab_media(self.key.clone(), artifact.clone())
            .await;
        let current = self.current().is_ok();
        self.document
            .lock()
            .unwrap()
            .returned_media(current, &instance, revision, &resource, &artifact, value)
    }
    /// Resolve an identity-only activation through Rust's validated view.
    pub fn activate(
        &self,
        instance: String,
        revision: u64,
        node: String,
    ) -> CoreResult<LabCardEffect> {
        self.current()?;
        let intent = self.document.lock().unwrap().activate(&Activation {
            instance,
            revision,
            node,
        })?;
        match intent {
            Intent::Open { .. } => self
                .history
                .lab_page(self.key.clone())
                .filter(|url| url.len() <= 4096)
                .map(|url| LabCardEffect::OpenPage { url })
                .ok_or_else(unavailable),
        }
    }
    pub fn dispose(&self) {
        self.document.lock().unwrap().close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card() -> lab_kit::Card {
        serde_json::from_value(serde_json::json!({"kind":"video","document":"synthetic","version":1,
            "title":"Test video","view":"video","primary":"test-video","poster":"test-poster","page":"/test"})).unwrap()
    }
    #[test]
    fn registered_media_rejects_missing_foreign_and_stale_identities() {
        let card = card();
        let mut doc = Document::new(&card, "test", "test-card").unwrap();
        let snapshot = doc.snapshot().unwrap();
        assert_eq!(snapshot.surfaces.len(), 1);
        let resource = &snapshot.surfaces[0].resource;
        assert!(doc.permits("test-card", 1, resource, "test-video"));
        assert!(!doc.permits("test-card", 1, "unregistered", "test-video"));
        assert!(!doc.permits("test-card", 1, resource, "foreign"));
        assert!(!doc.permits("older-card", 1, resource, "test-video"));
        assert!(!doc.permits("test-card", 2, resource, "test-video"));
        let mut newer = card.clone();
        newer.version += 1;
        assert!(!doc.matches(&newer));
        doc.close();
        assert!(!doc.permits("test-card", 1, resource, "test-video"));
        assert!(doc.snapshot().is_err());
    }
    #[test]
    fn navigation_requires_mounted_rust_identity_and_disposal_is_terminal() {
        let mut doc = Document::new(&card(), "test", "test-card").unwrap();
        let view = doc.projection.view.view();
        let rust_native::Element::Stack { children, .. } = &view.root.element else {
            panic!()
        };
        let event = Activation {
            instance: "test-card".into(),
            revision: 1,
            node: children.last().unwrap().key.clone(),
        };
        assert!(doc.activate(&event).is_err());
        doc.acknowledge("test-card", 1).unwrap();
        assert!(matches!(doc.activate(&event), Ok(Intent::Open { .. })));
        doc.close();
        assert!(doc.activate(&event).is_err());
    }
    #[test]
    fn android_authored_lab_fixtures_validate_with_shared_rust_intents() {
        for name in ["video", "compare", "model3d", "report"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../apps/android/app/src/test/resources/fixtures/lab-{name}-view.json"
            ));
            let bytes = std::fs::read(path).unwrap();
            let view = rust_native::View::<Intent>::from_json(&bytes).unwrap();
            assert!(view.to_json().unwrap().len() <= MAX_VIEW);
        }
    }
    #[test]
    fn media_completion_rechecks_current_card_and_disposal_before_returning_bytes() {
        let mut doc = Document::new(&card(), "test", "test-card").unwrap();
        let resource = doc.snapshot().unwrap().surfaces[0].resource.clone();
        let image = || LabMediaFile::Image {
            bytes: vec![1, 2, 3],
        };
        assert!(matches!(
            doc.returned_media(true, "test-card", 1, &resource, "test-poster", image()),
            LabMediaFile::Image { .. }
        ));
        assert!(matches!(
            doc.returned_media(false, "test-card", 1, &resource, "test-poster", image()),
            LabMediaFile::Missing
        ));
        doc.close();
        assert!(matches!(
            doc.returned_media(true, "test-card", 1, &resource, "test-poster", image()),
            LabMediaFile::Missing
        ));
    }
}
