//! Shared Rust composer projection for the Android host's cloud and live routes.
use super::*;
use crate::client_ffi::{CoreAccount, CoreCloudChat};
use rust_native::{Element, Node, View};
use std::sync::atomic::{AtomicBool, Ordering};
const MAX_BYTES: usize = 16_000;
#[derive(Clone, uniffi::Record)]
pub struct AndroidComposerFrame {
    pub instance: String,
    pub revision: u64,
    pub token: String,
    pub editing_lifetime: u64,
    pub editor_revision: u64,
    pub draft: String,
    pub placeholder: String,
    pub max_bytes: u32,
    pub enabled: bool,
    pub busy: bool,
    pub primary_label: String,
    pub stop_key: Option<String>,
    pub attachment_key: Option<String>,
    pub error: Option<String>,
}
enum Target {
    Live(Arc<SessionHandle>),
    Cloud {
        chat: Arc<CoreCloudChat>,
        account: Option<Arc<CoreAccount>>,
        generation: u64,
    },
}
struct Model {
    revision: u64,
    registry: NativeViewRegistry,
    mirror: Mirror,
    pending_send: bool,
    drafts: Option<Arc<ComposerDraftStore>>,
    listener: Option<Arc<dyn crate::client_ffi::AndroidHostListener>>,
}
#[derive(uniffi::Object)]
pub struct CoreAndroidComposer {
    target: Target,
    instance: String,
    token: String,
    model: Arc<Mutex<Model>>,
    closed: AtomicBool,
}
impl CoreAndroidComposer {
    fn make(target: Target) -> Arc<Self> {
        Arc::new(Self {
            target,
            instance: format!("android-editor-{}", uuid::Uuid::new_v4()),
            token: "message".into(),
            model: Arc::new(Mutex::new(Model {
                revision: 0,
                registry: Default::default(),
                mirror: Default::default(),
                pending_send: false,
                drafts: None,
                listener: None,
            })),
            closed: AtomicBool::new(false),
        })
    }
    pub(crate) fn bind_drafts(&self, drafts: Arc<ComposerDraftStore>) -> CoreResult<()> {
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        if model
            .mirror
            .state()
            .is_none_or(|state| state.text.is_empty())
            && !model.pending_send
        {
            if let Some(draft) = drafts.restore("android:cloud-default")? {
                model.mirror.dispose();
                model
                    .mirror
                    .mount("message", MAX_BYTES, Some(&draft))
                    .map_err(|_| CoreError::Closed)?;
            }
        }
        model.drafts = Some(drafts);
        Ok(())
    }
    pub(crate) fn bind_listener(&self, listener: Arc<dyn crate::client_ffi::AndroidHostListener>) {
        if let Ok(mut model) = self.model.lock() {
            model.listener = Some(listener);
        }
    }
    fn current(&self) -> CoreResult<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CoreError::Closed);
        }
        if let Target::Cloud {
            account: Some(account),
            generation,
            ..
        } = &self.target
        {
            if account.snapshot().generation != *generation {
                return Err(CoreError::Closed);
            }
        }
        Ok(())
    }
}
#[uniffi::export]
impl CoreAndroidComposer {
    #[uniffi::constructor]
    pub fn live(session: Arc<SessionHandle>) -> Arc<Self> {
        Self::make(Target::Live(session))
    }
    #[uniffi::constructor]
    pub fn cloud(chat: Arc<CoreCloudChat>, account: Option<Arc<CoreAccount>>) -> Arc<Self> {
        let generation = account
            .as_ref()
            .map(|a| a.snapshot().generation)
            .unwrap_or(0);
        Self::make(Target::Cloud {
            chat,
            account,
            generation,
        })
    }
    pub fn frame(&self) -> CoreResult<AndroidComposerFrame> {
        self.current()?;
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        let (enabled, busy, attachment, error, draft, stamp) = match &self.target {
            Target::Live(session) => {
                let c = session.composer();
                let editor = session
                    .composer_editor
                    .lock()
                    .map_err(|_| CoreError::Closed)?;
                (
                    c.open_input.is_none(),
                    c.live.turn_running,
                    c.host.capabilities.queue_attachments,
                    None,
                    editor
                        .state()
                        .map(|s| s.text)
                        .or(session.restore_composer_draft()?)
                        .unwrap_or_default(),
                    editor.stamp(),
                )
            }
            Target::Cloud { chat, account, .. } => {
                let snapshot = chat.snapshot();
                let enabled = account.as_ref().is_none_or(|a| {
                    matches!(
                        a.snapshot().state,
                        crate::client_ffi::AccountState::SignedIn { .. }
                    )
                });
                (
                    enabled,
                    snapshot.answering || model.pending_send,
                    false,
                    snapshot.error,
                    model
                        .mirror
                        .state()
                        .map(|s| s.text)
                        .or(snapshot.unsent)
                        .or(model
                            .drafts
                            .as_ref()
                            .map(|d| d.restore("android:cloud-default"))
                            .transpose()?
                            .flatten())
                        .unwrap_or_default(),
                    model.mirror.stamp(),
                )
            }
        };
        model.revision = model.revision.checked_add(1).ok_or(CoreError::Closed)?;
        let revision = model.revision;
        let view = View::new_v3(
            &self.instance,
            revision,
            Node {
                key: "composer".into(),
                style: Default::default(),
                element: Element::Composer {
                    token: self.token.clone(),
                    placeholder: "Message".into(),
                    max_bytes: MAX_BYTES,
                    enabled,
                    busy,
                    stop: busy.then_some(SessionViewIntent::Interrupt),
                    choices: vec![],
                    draft: Some(draft.clone()),
                    focus: false,
                    attach: attachment.then_some(SessionViewIntent::RequestAttachment),
                },
            },
        )
        .validate()
        .map_err(|_| CoreError::Closed)?
        .to_json()
        .map_err(|_| CoreError::Closed)?;
        match &self.target {
            Target::Live(s) => {
                s.prepare_native_view(view)?;
            }
            Target::Cloud { .. } => {
                model.registry.prepare(&view)?;
            }
        }
        Ok(AndroidComposerFrame {
            instance: self.instance.clone(),
            revision,
            token: self.token.clone(),
            editing_lifetime: stamp.as_ref().map(|s| s.lifetime).unwrap_or(0),
            editor_revision: stamp.as_ref().map(|s| s.revision).unwrap_or(0),
            draft,
            placeholder: "Message".into(),
            max_bytes: MAX_BYTES as u32,
            enabled,
            busy,
            primary_label: "Send".into(),
            stop_key: busy.then(|| "composer".into()),
            attachment_key: attachment.then(|| "composer".into()),
            error,
        })
    }
    pub fn editor(&self, instance: String, revision: u64, request: Vec<u8>) -> CoreResult<Vec<u8>> {
        self.current()?;
        if let Target::Live(s) = &self.target {
            return s.composer_editor(instance, revision, request);
        }
        if request.len() > rust_native::edit::mirror::MAX_REQUEST_BYTES {
            return Err(CoreError::Closed);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&request).map_err(|_| CoreError::Closed)?;
        let token = value
            .get("token")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("stamp")?.get("token")?.as_str())
            .ok_or(CoreError::Closed)?;
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        if value["op"] == "mount" {
            let limit = model
                .registry
                .composer_mount_max_bytes(&instance, revision, token)?;
            if value["max_bytes"].as_u64() != Some(limit as u64) {
                return Err(CoreError::Closed);
            }
        } else if value["op"] == "apply" {
            model
                .registry
                .composer_max_bytes(&instance, revision, token)?;
        } else {
            return Err(CoreError::Closed);
        }
        let response = model.mirror.call_json(&request);
        if let Some(drafts) = &model.drafts {
            if let Some(state) = model.mirror.state() {
                drafts.save("android:cloud-default", &state.text)?;
            }
        }
        Ok(response)
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> CoreResult<()> {
        self.current()?;
        match &self.target {
            Target::Live(s) => s.acknowledge_native_view(instance, revision),
            Target::Cloud { .. } => self
                .model
                .lock()
                .map_err(|_| CoreError::Closed)?
                .registry
                .acknowledge(&instance, revision),
        }
    }
    pub fn submit(
        &self,
        instance: String,
        revision: u64,
        token: String,
        lifetime: u64,
        editor_revision: u64,
        choice: Option<String>,
        text: String,
        attachments: Vec<OutgoingAttachment>,
    ) -> CoreResult<bool> {
        self.current()?;
        if let Target::Live(s) = &self.target {
            return s
                .submit_native_composer(
                    instance,
                    revision,
                    token,
                    lifetime,
                    editor_revision,
                    choice,
                    text,
                    attachments,
                )
                .map(|r| {
                    matches!(
                        r,
                        NativeActionResult::SendStarted { .. }
                            | NativeActionResult::SendSteered { .. }
                            | NativeActionResult::SendQueued { .. }
                    )
                });
        }
        if !attachments.is_empty() {
            return Err(CoreError::Unsupported {
                message: "Attachments aren't available in this chat.".into(),
            });
        }
        if text.trim().is_empty() || text.trim().chars().count() > 4000 {
            return Ok(false);
        }
        let Target::Cloud {
            chat,
            account,
            generation,
        } = &self.target
        else {
            unreachable!()
        };
        let mut model = self.model.lock().map_err(|_| CoreError::Closed)?;
        let stamp = EditorStamp {
            token: token.clone(),
            lifetime,
            revision: editor_revision,
        };
        if model.mirror.stamp().as_ref() != Some(&stamp)
            || model.mirror.state().is_none_or(|s| s.text != text)
        {
            return Err(CoreError::Closed);
        }
        model.registry.composer_busy_policy(
            &instance,
            revision,
            choice.as_deref().unwrap_or(&token),
            &text,
        )?;
        if model.pending_send || chat.snapshot().answering {
            return Ok(false);
        }
        let lease = account
            .as_ref()
            .map(|a| a.controller().lease().ok_or(CoreError::Closed))
            .transpose()?;
        if lease
            .as_ref()
            .is_some_and(|lease| lease.generation != *generation)
        {
            return Err(CoreError::Closed);
        }
        let completion = chat.send_completion(text.clone(), lease)?;
        if let Some(drafts) = &model.drafts {
            drafts.save("android:cloud-default", "")?;
        }
        model.pending_send = true;
        model
            .mirror
            .submitted(&stamp, &text)
            .map_err(|_| CoreError::Closed)?;
        let cleared = model.mirror.stamp();
        let owned = self.model.clone();
        let chat = chat.clone();
        openagents_client::runtime::shared().spawn(async move {
            completion.await;
            if let Ok(mut model) = owned.lock() {
                model.pending_send = false;
                let snapshot = chat.snapshot();
                if snapshot.unsent.as_deref() == Some(text.trim()) {
                    restore_unsent(&mut model.mirror, cleared.as_ref(), &text);
                }
                if let (Some(drafts), Some(state)) = (&model.drafts, model.mirror.state()) {
                    let _ = drafts.save("android:cloud-default", &state.text);
                }
                let listener = model.listener.clone();
                drop(model);
                if let Some(listener) = listener {
                    listener.on_android_host_changed();
                }
            }
        });
        let listener = model.listener.clone();
        drop(model);
        if let Some(listener) = listener {
            listener.on_android_host_changed();
        }
        Ok(true)
    }
    pub fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<bool> {
        self.current()?;
        if let Target::Live(s) = &self.target {
            return s
                .activate_native(instance, revision, key, None, vec![])
                .map(|r| r == NativeActionResult::AttachmentPickerRequested);
        }
        let intent = self
            .model
            .lock()
            .map_err(|_| CoreError::Closed)?
            .registry
            .activate(&rust_native::Activation {
                instance,
                revision,
                node: key,
            })?;
        if let (SessionViewIntent::Interrupt, Target::Cloud { chat, .. }) = (intent, &self.target) {
            chat.stop();
            return Ok(false);
        }
        Err(CoreError::Closed)
    }
    pub fn dispose(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

// A failed send restores its text only if no later edit changed the cleared editor.
fn restore_unsent(mirror: &mut Mirror, cleared: Option<&EditorStamp>, text: &str) {
    if mirror.stamp().as_ref() == cleared && mirror.state().is_some_and(|s| s.text.is_empty()) {
        mirror.dispose();
        let _ = mirror.mount("message", MAX_BYTES, Some(text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_send_restores_only_the_unchanged_editor() {
        let mut mirror = Mirror::default();
        mirror.mount("message", MAX_BYTES, Some("first")).unwrap();
        let stamp = mirror.stamp().unwrap();
        mirror.submitted(&stamp, "first").unwrap();
        let cleared = mirror.stamp();
        restore_unsent(&mut mirror, cleared.as_ref(), "first");
        assert_eq!(mirror.state().unwrap().text, "first");
        assert_ne!(mirror.stamp(), cleared);
        restore_unsent(&mut mirror, cleared.as_ref(), "old");
        assert_eq!(mirror.state().unwrap().text, "first");
    }
    #[test]
    fn failed_send_keeps_text_typed_after_submit() {
        let mut mirror = Mirror::default();
        mirror.mount("message", MAX_BYTES, Some("first")).unwrap();
        let stamp = mirror.stamp().unwrap();
        mirror.submitted(&stamp, "first").unwrap();
        let cleared = mirror.stamp().unwrap();
        mirror
            .apply(
                &cleared,
                rust_native::edit::mirror::Change::Sync {
                    text: "next".into(),
                    selection: [4, 4],
                    marked: None,
                    at_ms: 0,
                },
            )
            .unwrap();
        restore_unsent(&mut mirror, Some(&cleared), "first");
        assert_eq!(mirror.state().unwrap().text, "next");
    }
}
