//! Native mounting and callbacks over the shared Rust Home controller.
use super::{CoreClient, CoreError, CoreResult};
use coder_account::{Lease, dashboard::Home};
use openagents_dashboard::{Computer, Work};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[uniffi::export(with_foreign)]
pub trait HomeListener: Send + Sync {
    fn on_home_changed(&self);
}
#[derive(uniffi::Object)]
pub struct CoreHome {
    home: Arc<Home>,
    cancel: CancellationToken,
}
fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
impl CoreHome {
    pub(super) fn new(
        lease: Option<Lease>,
        path: PathBuf,
        listener: Arc<dyn HomeListener>,
    ) -> Arc<Self> {
        let home = Home::new(lease, path);
        let cancel = CancellationToken::new();
        let mut updates = home.watch();
        let owned = home.clone();
        let stopped = cancel.clone();
        zeron_client::runtime::shared().spawn(async move{
            Home::start(&owned);
            loop{tokio::select!{_ = stopped.cancelled()=>break,changed=updates.changed()=>{if changed.is_err(){break}listener.on_home_changed();}}}
        });
        Arc::new(Self { home, cancel })
    }
}
#[uniffi::export]
impl CoreHome {
    #[uniffi::constructor]
    pub fn demo(data_dir: String, listener: Arc<dyn HomeListener>) -> Arc<Self> {
        Self::new(
            None,
            PathBuf::from(data_dir).join("demo-home.json"),
            listener,
        )
    }
    pub fn snapshot(&self) -> CoreResult<String> {
        let view = self
            .home
            .view()
            .validate()
            .map_err(|_| failed("Couldn't open Home."))?;
        serde_json::to_string(view.view()).map_err(|_| failed("Couldn't open Home."))
    }
    pub fn set_visible(&self, visible: bool, foreground: bool) {
        self.home.set_visible(visible, foreground);
    }
    pub fn update_client(&self, client: Arc<CoreClient>) {
        let snapshot = client.client().workspace();
        let reports = client.client().reported_sessions();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.home.set_local_projects(
            snapshot
                .projects
                .iter()
                .map(|p| openagents_dashboard::Project {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    repositories: vec![],
                })
                .collect(),
        );
        let work = snapshot
            .sessions
            .values()
            .filter(|row| !row.archived)
            .map(|row| {
                let report = reports.iter().find(|s| s.chat_id == row.id);
                let at = report
                    .map(|s| s.updated_at.timestamp().max(0) as u64)
                    .unwrap_or(row.last_activity_ms.max(0) as u64 / 1000);
                Work {
                    id: row.id.clone(),
                    session: Some(row.id.clone()),
                    destination: Some(openagents_dashboard::WorkTarget::Session {
                        id: row.id.clone(),
                    }),
                    parent: row.parent_chat_id.clone(),
                    agent: row.harness_label.clone().unwrap_or_else(|| "Agent".into()),
                    title: row.title.clone(),
                    project: row.project.as_ref().map(|p| p.id.clone()),
                    host: row.device_name.clone().unwrap_or_else(|| "Computer".into()),
                    state: match report.map(|s| s.status) {
                        Some(zeron_proto::SessionStatus::Working) => "working",
                        Some(zeron_proto::SessionStatus::AwaitingInput) => "awaiting",
                        Some(zeron_proto::SessionStatus::Errored) => "failed",
                        Some(zeron_proto::SessionStatus::Idle) => {
                            if report.is_some_and(|s| s.last_completed_turn.is_some()) {
                                "completed"
                            } else {
                                "idle"
                            }
                        }
                        None => "unknown",
                    }
                    .into(),
                    online: row.device_online && report.is_some() && now.saturating_sub(at) < 45,
                    observed_at: at,
                }
            })
            .collect();
        let computers = snapshot
            .devices
            .iter()
            .map(|d| Computer {
                name: d.name.clone(),
                online: d.online,
            })
            .collect();
        self.home.set_live(work, computers);
    }
    pub async fn refresh(&self) {
        let home = self.home.clone();
        let _ = zeron_client::runtime::shared()
            .spawn(async move {
                home.refresh(true).await;
            })
            .await;
    }
    pub async fn activate(
        &self,
        instance: String,
        revision: u64,
        node: String,
        value: Option<String>,
    ) -> CoreResult<String> {
        let home = self.home.clone();
        let effect = zeron_client::runtime::shared()
            .spawn(async move {
                home.activate(
                    rust_native::Activation {
                        instance,
                        revision,
                        node,
                    },
                    value,
                )
                .await
            })
            .await
            .map_err(|_| failed("Couldn't update Home."))?
            .map_err(failed)?;
        serde_json::to_string(&effect).map_err(|_| failed("Couldn't open that item."))
    }
    pub async fn import_layout(&self, bytes: Vec<u8>) -> CoreResult<()> {
        let home = self.home.clone();
        zeron_client::runtime::shared()
            .spawn(async move { home.import(&bytes).await })
            .await
            .map_err(|_| failed("Couldn't import this layout."))?
            .map_err(failed)
    }
    pub async fn open_layout_link(&self, link: String) -> CoreResult<()> {
        let home = self.home.clone();
        zeron_client::runtime::shared()
            .spawn(async move { home.open_link(&link).await })
            .await
            .map_err(|_| failed("Couldn't open this layout."))?
            .map_err(failed)
    }
    pub async fn activate_form(
        &self,
        instance: String,
        revision: u64,
        node: String,
        fields: String,
    ) -> CoreResult<String> {
        if fields.len() > 64 * 1024 {
            return Err(failed("The settings are too long."));
        }
        let fields = serde_json::from_str(&fields)
            .map_err(|_| failed("Check the settings and try again."))?;
        let home = self.home.clone();
        let effect = zeron_client::runtime::shared()
            .spawn(async move {
                home.activate_form(
                    rust_native::Activation {
                        instance,
                        revision,
                        node,
                    },
                    fields,
                )
                .await
            })
            .await
            .map_err(|_| failed("Couldn't update Home."))?
            .map_err(failed)?;
        serde_json::to_string(&effect).map_err(|_| failed("Couldn't open that item."))
    }
    pub fn reorder(
        &self,
        instance: String,
        revision: u64,
        source: String,
        target: String,
    ) -> CoreResult<()> {
        self.home
            .reorder(
                rust_native::Activation {
                    instance: instance.clone(),
                    revision,
                    node: source,
                },
                rust_native::Activation {
                    instance,
                    revision,
                    node: target,
                },
            )
            .map_err(failed)
    }
    pub fn shutdown(&self) {
        self.cancel.cancel();
        self.home.shutdown();
    }
}
impl Drop for CoreHome {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.home.shutdown();
    }
}
