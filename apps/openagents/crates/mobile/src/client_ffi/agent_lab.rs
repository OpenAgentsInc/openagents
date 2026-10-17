//! Native callbacks over the shared account-bound Agent Lab controller.
use super::{CoreError, CoreResult};
use std::{path::PathBuf, sync::Arc};
#[uniffi::export(with_foreign)]
pub trait AgentLabListener: Send + Sync {
    fn on_agent_lab_changed(&self);
}
#[derive(uniffi::Object)]
pub struct CoreAgentLab {
    lab: Arc<coder_account::agent_lab::Lab>,
    cancel: tokio_util::sync::CancellationToken,
    visible: Arc<std::sync::atomic::AtomicBool>,
}
fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
impl CoreAgentLab {
    pub(super) fn new(
        controller: Arc<coder_account::Controller>,
        root: PathBuf,
        listener: Arc<dyn AgentLabListener>,
    ) -> Arc<Self> {
        let lab = coder_account::agent_lab::Lab::new(controller, root);
        let cancel = tokio_util::sync::CancellationToken::new();
        let stop = cancel.clone();
        let visible = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shown = visible.clone();
        let owned = lab.clone();
        let mut updates = lab.watch();
        zeron_client::runtime::shared().spawn(async move{let mut interval=tokio::time::interval(std::time::Duration::from_secs(5));loop{tokio::select!{_=stop.cancelled()=>break,_=interval.tick()=>{if shown.load(std::sync::atomic::Ordering::Relaxed){let _=owned.refresh().await;}},changed=updates.changed()=>{if changed.is_err(){break}listener.on_agent_lab_changed();}}}});
        Arc::new(Self {
            lab,
            cancel,
            visible,
        })
    }
}
impl Drop for CoreAgentLab {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
#[uniffi::export]
impl CoreAgentLab {
    pub fn set_visible(&self, visible: bool) {
        self.visible
            .store(visible, std::sync::atomic::Ordering::Relaxed);
        if visible {
            let lab = self.lab.clone();
            zeron_client::runtime::shared().spawn(async move {
                let _ = lab.refresh().await;
            });
        }
    }
    pub fn snapshot(&self) -> CoreResult<String> {
        let v = self
            .lab
            .view()
            .validate()
            .map_err(|_| failed("Couldn't open Agent Lab."))?;
        serde_json::to_string(v.view()).map_err(|_| failed("Couldn't open Agent Lab."))
    }
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        self.lab
            .change_field(rust_native::FieldChange {
                activation: rust_native::Activation {
                    instance,
                    revision,
                    node: key,
                },
                value,
            })
            .map_err(failed)
    }
    pub async fn activate(
        &self,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<String> {
        let lab = self.lab.clone();
        let effect = zeron_client::runtime::shared()
            .spawn(async move {
                lab.activate(rust_native::Activation {
                    instance,
                    revision,
                    node: key,
                })
                .await
            })
            .await
            .map_err(|_| failed("Couldn't update Agent Lab."))?
            .map_err(failed)?;
        serde_json::to_string(&effect).map_err(|_| failed("Couldn't open that result."))
    }
}
