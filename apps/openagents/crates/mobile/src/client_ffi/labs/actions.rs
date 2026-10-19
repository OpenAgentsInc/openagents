//! Retained Rust Native note and version forms over the existing Lab APIs.
use super::*;
use crate::client_ffi::AndroidHostListener;
use rust_native::{Activation, Axis, Element, FieldChange, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Intent {
    Write,
    Save,
    Choose { version: u64 },
    Reload,
    Dismiss,
}
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Note,
    Versions,
}
#[derive(Clone, uniffi::Record)]
pub struct LabActionsFrame {
    pub instance: String,
    pub card: String,
    pub needs_playback_position: bool,
    pub json: Vec<u8>,
    pub closed: bool,
}
struct Model {
    revision: u64,
    text: String,
    seconds: Option<f64>,
    playback_set: bool,
    busy: bool,
    closed: bool,
    versions: Vec<LabVersion>,
    error: Option<String>,
    pending: Option<ValidatedView<Intent>>,
    applied: Option<ValidatedView<Intent>>,
}
impl Model {
    fn admit(&mut self, event: Activation, kind: Kind) -> CoreResult<Intent> {
        let intent = self
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .activate(&event)
            .map_err(|_| CoreError::Closed)?
            .clone();
        match &intent {
            Intent::Dismiss => {
                self.closed = true;
                self.applied = None;
            }
            Intent::Save if valid_note(&self.text) && !self.busy && matches!(kind, Kind::Note) => {
                self.busy = true;
                self.applied = None;
            }
            Intent::Choose { version }
                if !self.busy
                    && matches!(kind, Kind::Versions)
                    && self
                        .versions
                        .iter()
                        .any(|v| v.number == *version && !v.shown) =>
            {
                self.busy = true;
                self.applied = None;
            }
            Intent::Reload if matches!(kind, Kind::Versions) && !self.busy => {}
            _ => return Err(CoreError::Closed),
        }
        Ok(intent)
    }
    fn complete(&mut self, success: bool, current: bool) {
        if !self.closed && current {
            self.busy = false;
            if success {
                self.closed = true;
                self.text.clear();
            } else {
                self.error = Some("Couldn't save the change. Try again.".into());
            }
        }
    }
}
#[derive(uniffi::Object)]
pub struct CoreLabActions {
    history: Arc<CoreSavedHistory>,
    document: Arc<CoreLabCard>,
    card: String,
    kind: Kind,
    instance: String,
    model: Mutex<Model>,
    listener: Arc<dyn AndroidHostListener>,
}
impl CoreLabActions {
    pub(crate) fn new(
        history: Arc<CoreSavedHistory>,
        card: String,
        kind: Kind,
        listener: Arc<dyn AndroidHostListener>,
    ) -> CoreResult<Arc<Self>> {
        let document = history.lab_projection(card.clone())?;
        let owner = Arc::new(Self {
            history,
            document,
            card,
            kind,
            instance: format!("lab-actions-{}", uuid::Uuid::new_v4()),
            listener,
            model: Mutex::new(Model {
                revision: 0,
                text: String::new(),
                seconds: None,
                playback_set: false,
                busy: false,
                closed: false,
                versions: vec![],
                error: None,
                pending: None,
                applied: None,
            }),
        });
        if matches!(kind, Kind::Versions) {
            let task = owner.clone();
            openagents_client::runtime::shared().spawn(async move {
                let _ = task.reload().await;
            });
        }
        Ok(owner)
    }
    fn current(&self) -> CoreResult<()> {
        if !self.history.current_view()
            || !self.document.is_current()
            || self.model.lock().map_err(|_| CoreError::Closed)?.closed
        {
            return Err(CoreError::Closed);
        }
        Ok(())
    }
    pub(crate) fn retired(&self) -> bool {
        self.current().is_err()
    }
    async fn reload(&self) -> CoreResult<()> {
        self.current()?;
        {
            let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
            if model.busy || !matches!(self.kind, Kind::Versions) {
                return Err(CoreError::Closed);
            }
            model.busy = true;
            model.applied = None;
        }
        self.listener.on_android_host_changed();
        let versions = self.history.lab_versions(self.card.clone()).await;
        self.current()?;
        {
            let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
            model.busy = false;
            match versions {
                Ok(versions) => {
                    model.versions = versions.into_iter().take(64).collect();
                    model.error = None;
                }
                Err(_) => model.error = Some("Couldn't load versions. Try again.".into()),
            }
        }
        self.listener.on_android_host_changed();
        Ok(())
    }
}
#[uniffi::export]
impl CoreLabActions {
    pub fn frame(&self) -> CoreResult<LabActionsFrame> {
        let current = self.current().is_ok();
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        if !current {
            model.closed = true;
        }
        model.revision = model.revision.checked_add(1).ok_or(CoreError::Closed)?;
        let view = form(&self.instance, model.revision, self.kind, &model)?;
        let json = view.to_json().map_err(|_| CoreError::Closed)?;
        model.pending = Some(view);
        Ok(LabActionsFrame {
            instance: self.instance.clone(),
            card: self.card.clone(),
            needs_playback_position: matches!(self.kind, Kind::Note) && !model.playback_set,
            json,
            closed: model.closed,
        })
    }
    /// A platform playback fact, sampled once before the person edits this note.
    pub fn playback_position(&self, seconds: Option<f64>) -> CoreResult<()> {
        self.current()?;
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        if !matches!(self.kind, Kind::Note) || model.playback_set || !model.text.is_empty() {
            return Err(CoreError::Closed);
        }
        if seconds.is_some_and(|s| !s.is_finite() || s < 0.0) {
            return Err(CoreError::Closed);
        }
        model.seconds = seconds;
        model.playback_set = true;
        Ok(())
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> CoreResult<()> {
        self.current()?;
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        let pending = model.pending.as_ref().ok_or(CoreError::Closed)?;
        if pending.view().instance != instance || pending.view().revision != revision {
            return Err(CoreError::Closed);
        }
        model.applied = model.pending.take();
        Ok(())
    }
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        node: String,
        value: String,
    ) -> CoreResult<()> {
        self.current()?;
        {
            let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
            let intent = model
                .applied
                .as_ref()
                .ok_or(CoreError::Closed)?
                .change_field(&FieldChange {
                    activation: Activation {
                        instance,
                        revision,
                        node,
                    },
                    value: value.clone(),
                })
                .map_err(|_| CoreError::Closed)?;
            if !matches!(intent, Intent::Write) || model.busy {
                return Err(CoreError::Closed);
            }
            model.text = value;
        }
        self.listener.on_android_host_changed();
        Ok(())
    }
    pub async fn activate(&self, instance: String, revision: u64, node: String) -> CoreResult<()> {
        self.current()?;
        let (intent, text, seconds) = {
            let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
            let intent = model.admit(
                Activation {
                    instance,
                    revision,
                    node,
                },
                self.kind,
            )?;
            (intent, model.text.trim().to_owned(), model.seconds)
        };
        if matches!(intent, Intent::Reload) {
            return self.reload().await;
        }
        self.listener.on_android_host_changed();
        let result = match intent {
            Intent::Save => {
                self.history
                    .lab_add_note(self.card.clone(), seconds, text)
                    .await
            }
            Intent::Choose { version } => {
                if self
                    .history
                    .lab_show_version(self.card.clone(), version)
                    .await
                {
                    Ok(())
                } else {
                    Err(CoreError::Closed)
                }
            }
            Intent::Dismiss => return Ok(()),
            _ => return Err(CoreError::Closed),
        };
        {
            let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
            model.complete(result.is_ok(), self.history.current_view());
        }
        self.listener.on_android_host_changed();
        Ok(())
    }
    pub fn dismiss(&self) {
        if let Ok(mut model) = self.model.lock() {
            model.closed = true;
            model.text.clear();
            model.applied = None;
            model.pending = None;
        }
    }
}
fn valid_note(text: &str) -> bool {
    !text.trim().is_empty()
        && text.trim().chars().count() <= openagents_account::lab_cards::MAX_NOTE_CHARS
}
fn form(
    instance: &str,
    revision: u64,
    kind: Kind,
    model: &Model,
) -> CoreResult<ValidatedView<Intent>> {
    let mut children = vec![];
    if let Some(error) = &model.error {
        children.push(node(
            "error",
            Element::Text {
                value: error.clone(),
                role: TextRole::Status,
            },
        ));
    }
    match kind {
        Kind::Note => {
            children.push(node(
                "note",
                Element::Field {
                    label: "Note".into(),
                    value: model.text.clone(),
                    placeholder: "Add a note".into(),
                    secret: false,
                    multiline: true,
                    enabled: !model.busy,
                    max_bytes: openagents_account::lab_cards::MAX_NOTE_CHARS * 4,
                    on_change: Intent::Write,
                },
            ));
            children.push(button(
                "save",
                "Save note",
                !model.busy && valid_note(&model.text),
                Intent::Save,
            ));
        }
        Kind::Versions => {
            if model.busy {
                children.push(node(
                    "loading",
                    Element::Text {
                        value: "Loading versions…".into(),
                        role: TextRole::Status,
                    },
                ));
            }
            for version in &model.versions {
                children.push(node(
                    format!("version-{}", version.number),
                    Element::Choice {
                        label: version.label.chars().take(512).collect(),
                        selected: version.shown,
                        enabled: !model.busy && !version.shown,
                        intent: Intent::Choose {
                            version: version.number,
                        },
                        children: vec![],
                    },
                ));
            }
            children.push(button("reload", "Refresh", !model.busy, Intent::Reload));
        }
    }
    children.push(button("close", "Close", true, Intent::Dismiss));
    View::new_v3(
        instance,
        revision,
        node(
            "dialog",
            Element::Dialog {
                label: match kind {
                    Kind::Note => "Add a note",
                    Kind::Versions => "Versions",
                }
                .into(),
                open: !model.closed,
                on_close: Intent::Dismiss,
                children: vec![node(
                    "body",
                    Element::Stack {
                        axis: Axis::Vertical,
                        children,
                    },
                )],
            },
        ),
    )
    .validate()
    .map_err(|_| CoreError::Closed)
}
fn node(key: impl Into<String>, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn button(key: &str, label: &str, enabled: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(text: &str) -> Model {
        Model {
            revision: 0,
            text: text.into(),
            seconds: None,
            playback_set: false,
            busy: false,
            closed: false,
            versions: vec![],
            error: None,
            pending: None,
            applied: None,
        }
    }
    fn event(node: &str, revision: u64) -> Activation {
        Activation {
            instance: "fixture".into(),
            revision,
            node: node.into(),
        }
    }
    fn mount(model: &mut Model, kind: Kind, revision: u64) {
        model.applied = Some(form("fixture", revision, kind, model).unwrap());
    }
    #[test]
    fn note_limits_use_the_existing_rust_policy_and_current_view() {
        let mut model = model("");
        mount(&mut model, Kind::Note, 1);
        assert!(model.admit(event("save", 1), Kind::Note).is_err());
        assert!(!valid_note(
            &"x".repeat(openagents_account::lab_cards::MAX_NOTE_CHARS + 1)
        ));
        assert!(valid_note(
            &"🙂".repeat(openagents_account::lab_cards::MAX_NOTE_CHARS)
        ));
        model.text = "A synthetic note".into();
        mount(&mut model, Kind::Note, 2);
        assert!(model.admit(event("save", 1), Kind::Note).is_err());
        assert!(matches!(
            model.admit(event("save", 2), Kind::Note),
            Ok(Intent::Save)
        ));
        assert!(model.busy);
        assert!(model.admit(event("save", 2), Kind::Note).is_err());
    }
    #[test]
    fn version_actions_only_select_the_current_list() {
        let mut model = model("");
        model.versions = vec![
            LabVersion {
                number: 2,
                label: "v2".into(),
                shown: true,
            },
            LabVersion {
                number: 1,
                label: "v1".into(),
                shown: false,
            },
        ];
        mount(&mut model, Kind::Versions, 4);
        assert!(model.admit(event("version-2", 4), Kind::Versions).is_err());
        assert!(model.admit(event("version-99", 4), Kind::Versions).is_err());
        assert!(matches!(
            model.admit(event("version-1", 4), Kind::Versions),
            Ok(Intent::Choose { version: 1 })
        ));
        assert!(model.admit(event("version-1", 4), Kind::Versions).is_err());
    }
    #[test]
    fn retired_or_changed_account_completion_cannot_publish_success() {
        let mut old = model("old account note");
        old.busy = true;
        old.complete(true, false);
        assert!(!old.closed);
        assert_eq!(old.text, "old account note");
        old.closed = true;
        old.text.clear();
        old.complete(false, true);
        assert!(old.error.is_none());
        assert!(old.closed && old.text.is_empty());
    }
    #[test]
    fn re_render_preserves_the_rust_note_draft_and_failed_save_can_retry() {
        let mut model = model("Retained synthetic draft");
        mount(&mut model, Kind::Note, 1);
        model.admit(event("save", 1), Kind::Note).unwrap();
        model.complete(false, true);
        assert_eq!(model.text, "Retained synthetic draft");
        assert!(!model.closed && !model.busy);
        mount(&mut model, Kind::Note, 2);
        model.admit(event("save", 2), Kind::Note).unwrap();
        model.complete(true, true);
        assert!(model.closed && model.text.is_empty());
    }
}
