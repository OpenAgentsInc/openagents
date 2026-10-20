//! Settings > Your ID > Public profile on the phone (#12298): the shared
//! Rust Native form (`openagents_account::public::ProfileForm`), the same as
//! the desktop's. The phone's key signs the profile; the native shells only
//! draw the form and pass its events here.
use super::{CoreError, CoreResult};
use openagents_account::public::{KeySource, ProfileForm};
use rust_native::{Activation, FieldChange};
use std::sync::Arc;

/// Told whenever the form changes, so the shell draws it again.
#[uniffi::export(with_foreign)]
pub trait PublicProfileListener: Send + Sync {
    fn on_public_profile_changed(&self);
}

/// The profile form for this phone's key.
#[derive(uniffi::Object)]
pub struct CorePublicProfile {
    form: Arc<ProfileForm>,
    cancel: tokio_util::sync::CancellationToken,
}

fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}

impl CorePublicProfile {
    pub(super) fn new(
        controller: Arc<openagents_account::Controller>,
        key: KeySource,
        listener: Arc<dyn PublicProfileListener>,
    ) -> Arc<Self> {
        let form = ProfileForm::new(controller, key);
        let cancel = tokio_util::sync::CancellationToken::new();
        let stop = cancel.clone();
        let mut updates = form.watch();
        openagents_client::runtime::shared().spawn(async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    changed = updates.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        listener.on_public_profile_changed();
                    }
                }
            }
        });
        let loading = form.clone();
        openagents_client::runtime::shared().spawn(async move {
            // A failed read shows in the form.
            let _ = loading.load().await;
        });
        Arc::new(Self { form, cancel })
    }
}

impl Drop for CorePublicProfile {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[uniffi::export]
impl CorePublicProfile {
    /// The form as Rust Native view JSON.
    pub fn snapshot(&self) -> CoreResult<String> {
        let view = self
            .form
            .view()
            .validate()
            .map_err(|_| failed("Couldn't open your profile."))?;
        serde_json::to_string(view.view()).map_err(|_| failed("Couldn't open your profile."))
    }

    /// Keep a typed change to one field.
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        self.form
            .change_field(&FieldChange {
                activation: Activation {
                    instance,
                    revision,
                    node: key,
                },
                value,
            })
            .map_err(failed)
    }

    /// Read the profile again.
    pub async fn reload(&self) -> CoreResult<()> {
        let form = self.form.clone();
        openagents_client::runtime::shared()
            .spawn(async move { form.load().await })
            .await
            .map_err(|_| failed("Couldn't open your profile."))?
            .map_err(failed)
    }

    /// Press a button: Save claims the name and publishes the profile.
    pub async fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<()> {
        let form = self.form.clone();
        openagents_client::runtime::shared()
            .spawn(async move {
                form.activate(&Activation {
                    instance,
                    revision,
                    node: key,
                })
                .await
            })
            .await
            .map_err(|_| failed("Couldn't save your profile."))?
            .map_err(failed)
    }
}
