//! One Rust-owned settings surface; Android only mounts and reports native events.
mod model;
mod preferences;
use super::{CoreAccount, CoreClient, CoreError, CoreResult};
pub use model::SettingsCapabilities;
use model::{Action, Device, Input, Model, Operation};
use openagents_account::Phase;
pub use preferences::{
    SettingsAppearance, SettingsPreferences, SettingsWallpaper, SettingsWallpaperEffect,
};
use rust_native::{Activation, FieldChange};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SettingsEffect {
    None,
    OpenPairing,
    OpenArchive,
    OpenProjectSharing,
    OpenNotificationSettings,
    ChooseWallpaper { ticket: String },
    PreferencesChanged { preferences: SettingsPreferences },
    ShareText { title: String, text: String },
}
#[uniffi::export(with_foreign)]
pub trait SettingsListener: Send + Sync {
    fn on_settings_changed(&self);
}
#[derive(uniffi::Object)]
pub struct CoreSettings {
    account: Arc<CoreAccount>,
    client: Option<Arc<CoreClient>>,
    generation: u64,
    caps: SettingsCapabilities,
    version: String,
    store: preferences::Store,
    model: Mutex<Model>,
    cloud_running: Mutex<Option<bool>>,
    io: tokio::sync::Mutex<()>,
    cancel: CancellationToken,
    listener: Arc<dyn SettingsListener>,
}
fn failed(message: impl Into<String>) -> CoreError {
    CoreError::Storage {
        message: message.into(),
    }
}
fn closed() -> CoreError {
    CoreError::Closed
}
fn input(
    account: &CoreAccount,
    client: Option<&CoreClient>,
    caps: &SettingsCapabilities,
    version: &str,
    cloud_running: Option<bool>,
) -> Input {
    let snapshot = account.controller().snapshot();
    let (signed_in, name) = match snapshot.phase {
        Phase::SignedIn { account } => (true, account.label),
        _ => (false, "Your account".into()),
    };
    Input {
        generation: snapshot.generation,
        signed_in,
        name,
        devices: if signed_in {
            client
                .map(|client| {
                    client
                        .devices()
                        .into_iter()
                        .map(|device| Device {
                            id: device.id,
                            name: device.name,
                            online: device.online,
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![]
        },
        cloud_running,
        caps: caps.clone(),
        version: version.into(),
    }
}
#[uniffi::export]
pub fn open_settings(
    account: Arc<CoreAccount>,
    client: Option<Arc<CoreClient>>,
    capabilities: SettingsCapabilities,
    version: String,
    listener: Arc<dyn SettingsListener>,
) -> CoreResult<Arc<CoreSettings>> {
    if version.len() > 80 || version.chars().any(char::is_control) {
        return Err(CoreError::InvalidArgument {
            message: "The app version could not be read.".into(),
        });
    }
    let store = preferences::Store::new(account.data_dir().join("phone-settings.json"));
    let (prefs, error) = match store.read() {
        Ok(prefs) => (prefs, None),
        Err(message) => (Default::default(), Some(message)),
    };
    let state = input(&account, client.as_deref(), &capabilities, &version, None);
    let this = Arc::new(CoreSettings {
        generation: state.generation,
        account,
        client,
        caps: capabilities,
        version,
        store,
        model: Mutex::new(Model::new(state, prefs, error)),
        cloud_running: Mutex::new(None),
        io: Default::default(),
        cancel: CancellationToken::new(),
        listener,
    });
    let mut changes = this.account.controller().watch();
    let weak = Arc::downgrade(&this);
    let cancel = this.cancel.clone();
    openagents_client::runtime::shared().spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                changed = changes.changed() => {
                    let Some(this) = weak.upgrade() else { break; };
                    if changed.is_err() || this.current().is_err() {
                        this.model.lock().unwrap().close();
                        this.listener.on_settings_changed();
                        break;
                    }
                    this.listener.on_settings_changed();
                }
            }
        }
    });
    Ok(this)
}
impl CoreSettings {
    fn current(&self) -> CoreResult<()> {
        if self.cancel.is_cancelled()
            || self.account.controller().snapshot().generation != self.generation
        {
            Err(closed())
        } else {
            Ok(())
        }
    }
    fn update(&self) -> CoreResult<()> {
        self.current()?;
        self.model
            .lock()
            .unwrap()
            .update(input(
                &self.account,
                self.client.as_deref(),
                &self.caps,
                &self.version,
                *self.cloud_running.lock().unwrap(),
            ))
            .map_err(failed)
    }
    async fn perform(
        self: Arc<Self>,
        operation: Operation,
        action: Action,
    ) -> CoreResult<SettingsEffect> {
        let _io = self.io.lock().await;
        self.current()?;
        if !self.model.lock().unwrap().accepts(&operation) {
            return Err(closed());
        }
        let mut effect = SettingsEffect::None;
        let result: Result<Option<SettingsPreferences>, String> = match action {
            Action::Delete(forfeit) => {
                let result = self
                    .account
                    .controller()
                    .delete_account_for_generation(operation.generation, forfeit)
                    .await;
                // Deletion's account transition is observed by the shared account
                // owner. An old surface never navigates or clears a newer account.
                if self.current().is_ok() {
                    self.model.lock().unwrap().deletion(&operation, result);
                }
                self.listener.on_settings_changed();
                return Ok(SettingsEffect::None);
            }
            Action::SignOut => {
                self.account
                    .controller()
                    .sign_out_for_generation(operation.generation)
                    .await;
                self.listener.on_settings_changed();
                return Ok(SettingsEffect::None);
            }
            Action::Save(prefs) => {
                let store = self.store.clone();
                let saved = prefs.clone();
                match tokio::task::spawn_blocking(move || store.write(&saved)).await {
                    Ok(Ok(())) => {
                        effect = SettingsEffect::PreferencesChanged {
                            preferences: prefs.clone(),
                        };
                        Ok(Some(prefs))
                    }
                    Ok(Err(message)) => Err(message),
                    Err(_) => Err("Couldn't save your settings. Try again.".into()),
                }
            }
            Action::Rename { id, name } => self
                .client
                .as_ref()
                .ok_or_else(|| "Connect your computer again to rename it.".into())
                .and_then(|client| {
                    client
                        .rename_device(id, name)
                        .map_err(|_| "Couldn't rename this computer. Try again.".into())
                })
                .map(|_| None),
            Action::Cloud(start) => {
                let Some(lease) = self
                    .account
                    .controller()
                    .lease()
                    .filter(|lease| lease.generation == operation.generation)
                else {
                    return Err(closed());
                };
                let result = if start {
                    lease
                        .client()
                        .start_cloud_host(lease.session(), &lease.cancel)
                        .await
                } else {
                    lease
                        .client()
                        .stop_cloud_host(lease.session(), &lease.cancel)
                        .await
                };
                if !lease.is_current() {
                    return Err(closed());
                }
                match result {
                    Ok(host) => {
                        *self.cloud_running.lock().unwrap() = Some(host.running());
                        Ok(None)
                    }
                    Err(error) => Err(match super::cloud_host::message(&error) {
                        CoreError::Auth { message }
                        | CoreError::Network { message }
                        | CoreError::HostError { message } => message,
                        _ => "Couldn't update OpenAgents Cloud. Try again.".into(),
                    }),
                }
            }
            Action::Pair => {
                effect = SettingsEffect::OpenPairing;
                Ok(None)
            }
            Action::Archive => {
                effect = SettingsEffect::OpenArchive;
                Ok(None)
            }
            Action::Sharing => {
                effect = SettingsEffect::OpenProjectSharing;
                Ok(None)
            }
            Action::Notifications => {
                effect = SettingsEffect::OpenNotificationSettings;
                Ok(None)
            }
            Action::ChooseWallpaper { ticket } => {
                effect = SettingsEffect::ChooseWallpaper { ticket };
                Ok(None)
            }
            Action::ShareApp => {
                effect = share_app();
                Ok(None)
            }
        };
        self.current()?;
        self.model.lock().unwrap().complete(&operation, result);
        self.listener.on_settings_changed();
        Ok(effect)
    }
}
#[uniffi::export]
impl CoreSettings {
    /// Only bytes prepared by the landed Rust AndroidView bridge may be mounted.
    pub fn snapshot(&self) -> CoreResult<Vec<u8>> {
        self.update()?;
        self.model.lock().unwrap().prepare().map_err(failed)
    }
    pub fn preferences(&self) -> CoreResult<SettingsPreferences> {
        self.current()?;
        Ok(self.model.lock().unwrap().prefs.clone())
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> CoreResult<()> {
        self.current()?;
        self.model
            .lock()
            .unwrap()
            .acknowledge(&instance, revision)
            .map_err(failed)
    }
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        self.current()?;
        self.model
            .lock()
            .unwrap()
            .change_field(FieldChange {
                activation: Activation {
                    instance,
                    revision,
                    node: key,
                },
                value,
            })
            .map_err(failed)?;
        self.listener.on_settings_changed();
        Ok(())
    }
    pub async fn activate(
        self: &Arc<Self>,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<SettingsEffect> {
        self.current()?;
        let action = self
            .model
            .lock()
            .unwrap()
            .activate(Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(failed)?;
        self.listener.on_settings_changed();
        let Some((operation, action)) = action else {
            return Ok(SettingsEffect::None);
        };
        let this = self.clone();
        openagents_client::runtime::shared()
            .spawn(this.perform(operation, action))
            .await
            .map_err(|_| closed())?
    }
    /// The platform reports success only after retaining its document permission.
    pub async fn wallpaper_selected(
        self: &Arc<Self>,
        ticket: String,
        uri: String,
        name: String,
        permission_saved: bool,
    ) -> CoreResult<SettingsEffect> {
        self.current()?;
        let (operation, action) = self
            .model
            .lock()
            .unwrap()
            .wallpaper(&ticket, uri, name, permission_saved)
            .map_err(failed)?;
        let this = self.clone();
        openagents_client::runtime::shared()
            .spawn(this.perform(operation, action))
            .await
            .map_err(|_| closed())?
    }
    /// Cloud controls appear only after an authorized status read succeeds.
    pub async fn refresh_cloud(self: &Arc<Self>) -> CoreResult<()> {
        self.current()?;
        if !self.caps.cloud {
            return Ok(());
        }
        let Some(lease) = self
            .account
            .controller()
            .lease()
            .filter(|lease| lease.generation == self.generation)
        else {
            return Ok(());
        };
        let this = self.clone();
        openagents_client::runtime::shared()
            .spawn(async move {
                let result = lease
                    .client()
                    .cloud_host(lease.session(), &lease.cancel)
                    .await;
                this.current()?;
                if !lease.is_current() {
                    return Err(closed());
                }
                *this.cloud_running.lock().unwrap() = result.ok().map(|host| host.running());
                this.listener.on_settings_changed();
                Ok(())
            })
            .await
            .map_err(|_| closed())?
    }
    pub fn dispose(&self) {
        self.cancel.cancel();
        self.model.lock().unwrap().close();
    }
}
impl Drop for CoreSettings {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn share_app() -> SettingsEffect {
    SettingsEffect::ShareText {
        title: openagents_brand::NAME.into(),
        text: openagents_brand::DOWNLOAD_URL.into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_share_uses_only_rust_brand_values() {
        let SettingsEffect::ShareText { title, text } = share_app() else {
            panic!("expected share effect");
        };
        assert_eq!(title, openagents_brand::NAME);
        assert_eq!(text, openagents_brand::DOWNLOAD_URL);
        assert!(text.starts_with("https://"));
    }
}
