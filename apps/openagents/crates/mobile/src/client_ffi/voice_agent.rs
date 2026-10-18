//! Phone callbacks over the shared voice-agent picker.
use super::{CoreError, CoreResult};
use std::sync::Arc;
#[uniffi::export(with_foreign)]
pub trait VoiceAgentListener: Send + Sync {
    fn on_voice_agent_changed(&self);
}
#[derive(uniffi::Object)]
pub struct CoreVoiceAgent {
    picker: Arc<openagents_account::voice_agent::Picker>,
    cancel: tokio_util::sync::CancellationToken,
    visible: Arc<std::sync::atomic::AtomicBool>,
}
fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
impl CoreVoiceAgent {
    pub(super) fn new(
        controller: Arc<openagents_account::Controller>,
        listener: Arc<dyn VoiceAgentListener>,
    ) -> Arc<Self> {
        let picker = openagents_account::voice_agent::Picker::new(controller);
        let cancel = tokio_util::sync::CancellationToken::new();
        let stop = cancel.clone();
        let visible = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shown = visible.clone();
        let owned = picker.clone();
        let mut updates = picker.watch();
        openagents_client::runtime::shared().spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop { tokio::select! {
                _ = stop.cancelled() => break,
                _ = interval.tick() => if shown.load(std::sync::atomic::Ordering::Relaxed) { let _ = owned.refresh().await; },
                changed = updates.changed() => { if changed.is_err() { break; } listener.on_voice_agent_changed(); }
            }}
        });
        Arc::new(Self {
            picker,
            cancel,
            visible,
        })
    }
}
impl Drop for CoreVoiceAgent {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
#[uniffi::export]
impl CoreVoiceAgent {
    pub fn set_visible(&self, visible: bool) {
        self.visible
            .store(visible, std::sync::atomic::Ordering::Relaxed);
        if visible {
            let picker = self.picker.clone();
            openagents_client::runtime::shared().spawn(async move {
                let _ = picker.refresh().await;
            });
        }
    }
    pub fn snapshot(&self) -> CoreResult<String> {
        let view = self
            .picker
            .view()
            .validate()
            .map_err(|_| failed("Couldn't open Voice."))?;
        serde_json::to_string(view.view()).map_err(|_| failed("Couldn't open Voice."))
    }
    pub async fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<()> {
        let picker = self.picker.clone();
        openagents_client::runtime::shared()
            .spawn(async move {
                picker
                    .activate(rust_native::Activation {
                        instance,
                        revision,
                        node: key,
                    })
                    .await
            })
            .await
            .map_err(|_| failed("Couldn't save your voice agent."))?
            .map_err(failed)
    }
}
