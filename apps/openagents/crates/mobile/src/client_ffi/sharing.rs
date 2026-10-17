//! Account-bound callbacks for the shared Rust Native project-sharing form.
use super::{CoreError, CoreResult};
use coder_sharing::{
    Client,
    model::{Effect, Model},
};
use computer_driver::{CommandCursor, CommandProgress};
use openagents_account::Lease;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio_util::sync::CancellationToken;

#[uniffi::export(with_foreign)]
pub trait SharingListener: Send + Sync {
    fn on_sharing_changed(&self);
}
struct State {
    model: Model,
    cursor: CommandCursor,
}
struct Inner {
    lease: Lease,
    client: Client,
    state: Mutex<State>,
    io: tokio::sync::Mutex<()>,
    visible: AtomicBool,
    listener: Arc<dyn SharingListener>,
    cancel: CancellationToken,
}
#[derive(uniffi::Object)]
pub struct CoreSharing {
    inner: Arc<Inner>,
}
fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
fn auth() -> CoreError {
    CoreError::Auth {
        message: "Log in again to continue.".into(),
    }
}
fn now() -> u64 {
    coder_access::unix_time().unwrap_or(u64::MAX)
}
impl Inner {
    fn current(&self) -> CoreResult<()> {
        if self.cancel.is_cancelled() || !self.lease.is_current() {
            Err(auth())
        } else {
            Ok(())
        }
    }
    fn refresh(&self) -> coder_sharing::Result<()> {
        let computers = self.client.computers()?;
        let jobs = self.client.jobs()?;
        let contacts = self.client.contacts()?;
        self.state
            .lock()
            .unwrap()
            .model
            .refresh(computers, jobs, contacts);
        Ok(())
    }
    async fn poll(&self) -> coder_sharing::Result<()> {
        let active = {
            let state = self.state.lock().unwrap();
            state.model.active_job.clone().map(|id| (id, state.cursor))
        };
        let Some((id, cursor)) = active else {
            return Ok(());
        };
        let read = self.client.read_job(&id, cursor).await?;
        if self.current().is_err() {
            return Ok(());
        }
        let finished = matches!(
            read.progress,
            CommandProgress::Exited { .. } | CommandProgress::Lost
        );
        let detail = match read.progress {
            CommandProgress::Exited { code: 0 } => Some("Command finished.".into()),
            CommandProgress::Exited { code } => Some(format!("Command exited with code {code}.")),
            CommandProgress::Lost => {
                Some("The computer could not confirm how the command ended.".into())
            }
            CommandProgress::Absent => {
                Some("The computer has not started this command. You can try again.".into())
            }
            CommandProgress::Running => None,
        };
        {
            let mut state = self.state.lock().unwrap();
            if state.model.active_job.as_deref() != Some(&id) {
                return Ok(());
            }
            state.cursor.stdout = cursor.stdout.saturating_add(read.stdout.len() as u64);
            state.cursor.stderr = cursor.stderr.saturating_add(read.stderr.len() as u64);
            state.model.idle();
            if read.progress == CommandProgress::Absent {
                state.model.running = false;
            }
            state
                .model
                .append_output(&read.stdout, &read.stderr, finished, detail.as_deref());
        }
        self.refresh()?;
        Ok(())
    }
    async fn effect(&self, effect: Effect) -> coder_sharing::Result<()> {
        match effect {
            Effect::Refresh => {
                self.refresh()?;
                self.state.lock().unwrap().model.idle();
            }
            Effect::Connect {
                code,
                label,
                shared,
            } => {
                self.client.connect(&code, shared, &label).await?;
                self.state.lock().unwrap().model.connected();
                self.refresh()?;
            }
            Effect::Manage(id) => {
                let projects = self.client.projects(&id).await?;
                let shares = self.client.shares(&id).await?;
                self.state
                    .lock()
                    .unwrap()
                    .model
                    .managed(&id, projects, shares);
            }
            Effect::Invite {
                id,
                operation,
                recipient,
                projects,
                can_run,
                expires_at,
            } => {
                let (code, expiry) = self
                    .client
                    .invite(&id, &operation, &recipient, projects, can_run, expires_at)
                    .await?;
                self.state.lock().unwrap().model.invited(code, expiry);
                self.refresh()?;
            }
            Effect::Revoke {
                id,
                operation,
                grant,
            } => {
                self.client.revoke(&id, &operation, &grant).await?;
                let projects = self.client.projects(&id).await?;
                let shares = self.client.shares(&id).await?;
                self.state
                    .lock()
                    .unwrap()
                    .model
                    .managed(&id, projects, shares);
            }
            Effect::ReadFile { id, project, path } => {
                let bytes = self.client.read_file(&id, &project, &path).await?;
                self.state.lock().unwrap().model.file(&bytes);
            }
            Effect::WriteFile {
                id,
                project,
                path,
                bytes,
            } => {
                self.client.write_file(&id, &project, &path, &bytes).await?;
                let mut state = self.state.lock().unwrap();
                state.model.idle();
                state.model.notice = Some("File saved.".into());
            }
            Effect::Run {
                id,
                project,
                job,
                script,
            } => {
                // The client saves this identity before sending. An uncertain
                // start remains inspectable; polling never starts a command.
                let result = self.client.start(&id, &project, &job, &script, 300).await;
                self.refresh()?;
                if self.client.jobs()?.iter().any(|record| record.id == job) {
                    let mut state = self.state.lock().unwrap();
                    state.cursor = CommandCursor::default();
                    state.model.command(job);
                }
                result?;
            }
            Effect::Stop(job) => {
                self.client.stop_job(&job).await?;
                self.poll().await?;
            }
            Effect::Job(_) => {
                self.state.lock().unwrap().cursor = CommandCursor::default();
                self.poll().await?;
            }
            Effect::Forget(id) => {
                self.client.forget(&id)?;
                self.refresh()?;
                self.state.lock().unwrap().model.idle();
            }
            Effect::Copy(_) => {}
        }
        Ok(())
    }
}
impl CoreSharing {
    pub(super) fn new(
        lease: Lease,
        directory: &Path,
        listener: Arc<dyn SharingListener>,
    ) -> CoreResult<Arc<Self>> {
        if !lease.is_current() {
            return Err(auth());
        }
        let account = lease.session().account();
        let client = Client::open(
            directory,
            &account.id,
            &account.label,
            coder_access::RelayPolicy::Production,
        )
        .map_err(|e| failed(e.message))?;
        let model = Model::new(client.recipient());
        let inner = Arc::new(Inner {
            lease,
            client,
            state: Mutex::new(State {
                model,
                cursor: CommandCursor::default(),
            }),
            io: Default::default(),
            visible: AtomicBool::new(false),
            listener,
            cancel: CancellationToken::new(),
        });
        inner.refresh().map_err(|e| failed(e.message))?;
        let weak = Arc::downgrade(&inner);
        let stop = inner.cancel.clone();
        let logout = inner.lease.cancel.clone();
        openagents_client::runtime::shared().spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                tokio::select! {
                    biased;
                    _ = stop.cancelled() => break,
                    _ = logout.cancelled() => {
                        if let Some(inner) = weak.upgrade() {
                            inner.listener.on_sharing_changed();
                        }
                        break;
                    }
                    _ = interval.tick() => {
                        let Some(inner) = weak.upgrade() else { break };
                        if inner.current().is_err() { inner.listener.on_sharing_changed(); break; }
                        if !inner.visible.load(Ordering::Relaxed) { continue; }
                        let Ok(_guard) = inner.io.try_lock() else { continue };
                        let running = inner.state.lock().unwrap().model.running;
                        if running {
                            tokio::select! {
                                biased;
                                _ = stop.cancelled() => break,
                                _ = logout.cancelled() => break,
                                result = inner.poll() => {
                                    if let Err(error) = result {
                                        inner.state.lock().unwrap().model.failed(&error);
                                    }
                                }
                            }
                        }
                        inner.listener.on_sharing_changed();
                    }
                }
            }
        });
        Ok(Arc::new(Self { inner }))
    }
}
impl Drop for CoreSharing {
    fn drop(&mut self) {
        self.inner.cancel.cancel();
    }
}
#[uniffi::export]
impl CoreSharing {
    pub fn set_visible(&self, visible: bool) {
        self.inner.visible.store(visible, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> CoreResult<String> {
        self.inner.current()?;
        self.inner
            .state
            .lock()
            .unwrap()
            .model
            .json(now())
            .map_err(|e| failed(e.message))
    }
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        self.inner.current()?;
        self.inner
            .state
            .lock()
            .unwrap()
            .model
            .change(
                rust_native::FieldChange {
                    activation: rust_native::Activation {
                        instance,
                        revision,
                        node: key,
                    },
                    value,
                },
                now(),
            )
            .map_err(|e| failed(e.message))?;
        self.inner.listener.on_sharing_changed();
        Ok(())
    }
    /// Return clipboard text only for an explicit copy action.
    pub async fn activate(
        &self,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<Option<String>> {
        self.inner.current()?;
        let effect = self
            .inner
            .state
            .lock()
            .unwrap()
            .model
            .activate(
                rust_native::Activation {
                    instance,
                    revision,
                    node: key,
                },
                now(),
            )
            .map_err(|e| failed(e.message))?;
        self.inner.listener.on_sharing_changed();
        let Some(effect) = effect else {
            return Ok(None);
        };
        if let Effect::Copy(text) = effect {
            self.inner.current()?;
            return Ok(Some(text));
        }
        let inner = self.inner.clone();
        openagents_client::runtime::shared()
            .spawn(async move {
                let _guard = inner.io.lock().await;
                inner.current()?;
                let result = tokio::select! {
                    biased;
                    _ = inner.cancel.cancelled() => return Err(auth()),
                    _ = inner.lease.cancel.cancelled() => return Err(auth()),
                    result = inner.effect(effect) => result,
                };
                inner.current()?;
                if let Err(error) = result {
                    inner.state.lock().unwrap().model.failed(&error);
                }
                inner.listener.on_sharing_changed();
                Ok(None)
            })
            .await
            .map_err(|_| failed("Could not update project sharing."))?
    }
}
