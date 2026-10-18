//! Android-facing ownership over the existing Rust project, session, cloud, and continuation APIs.
use super::chat_homes::{CoreChatHomes, CoreContinuationDraft};
use super::own_computer::CoreOwnComputerOffer;
use super::*;
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::Serialize;
use std::sync::Mutex;
mod draft;
use draft::{Draft, Menu};
fn failed(message: &str) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
#[derive(Clone, Serialize)]
enum Intent {
    Cloud,
    NewCloud,
    Computers,
    Projects,
    Harnesses,
    Models,
    Efforts,
    Branches,
    Browse,
    Sharing,
    Create,
    Back,
    Next,
    Previous,
    Computer(usize),
    Project(usize),
    Harness(usize),
    Model(usize),
    Effort(Option<usize>),
    Branch(usize),
    Folder(usize),
    UseFolder,
    CloudStatus,
    CloudStart,
    CloudStop,
    RetryCatalog,
    OwnCopy,
    OwnComputer,
    CloseOwnComputer,
}
#[derive(uniffi::Enum)]
pub enum AndroidSessionAction {
    Updated,
    Cloud { chat: Arc<CoreCloudChat> },
    Session { id: String, client: Arc<CoreClient> },
    Sharing,
    OpenUrl { url: String },
    Clipboard { text: String },
}
struct State {
    draft: Draft,
    client: Option<Arc<CoreClient>>,
    menu: Menu,
    page: usize,
    revision: u64,
    pending: Option<ValidatedView<Intent>>,
    applied: Option<ValidatedView<Intent>>,
    devices: Vec<DeviceView>,
    projects: Vec<ProjectView>,
    refs: Vec<RepoRef>,
    folders: Option<FolderListing>,
    folder_is_repo: Option<bool>,
    notice: Option<String>,
    closed: bool,
}
#[derive(uniffi::Object)]
pub struct CoreAndroidSessions {
    account: Arc<CoreAccount>,
    cloud: Arc<CoreCloudChat>,
    offer: Arc<CoreOwnComputerOffer>,
    state: Mutex<State>,
    instance: String,
    actions: tokio::sync::Mutex<()>,
}
fn node(key: impl Into<String>, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn button(
    key: impl Into<String>,
    label: impl Into<String>,
    enabled: bool,
    intent: Intent,
) -> Node<Intent> {
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
fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}
impl CoreAndroidSessions {
    fn current(&self, s: &mut State) -> Result<(), CoreError> {
        if s.closed {
            return Err(CoreError::Closed);
        }
        if self.account.snapshot().generation != s.draft.generation {
            s.draft = Draft::new(self.account.snapshot().generation);
            s.client = None;
            s.menu = Menu::Home;
            s.pending = None;
            s.applied = None;
            s.projects.clear();
            s.devices.clear();
            s.refs.clear();
            s.folders = None;
            return Err(CoreError::Closed);
        }
        if let Some(core) = &s.client {
            let lease = self.account.controller().lease().ok_or(CoreError::Closed)?;
            if !core.client().uses_account(
                &lease.session().account().id,
                lease.session().bearer_token(),
            ) {
                return Err(CoreError::Closed);
            }
        }
        Ok(())
    }
    fn core(&self) -> Result<(Arc<CoreClient>, u64), CoreError> {
        let mut s = self.state.lock().unwrap();
        self.current(&mut s)?;
        Ok((
            s.client
                .clone()
                .ok_or_else(|| failed("Your chats are still loading. Try again in a moment."))?,
            s.draft.generation,
        ))
    }
    fn menu(s: &mut State, menu: Menu) {
        s.menu = menu;
        s.page = 0;
        s.applied = None;
        s.pending = None;
    }
    async fn catalog(
        &self,
        device: String,
        harness: Option<String>,
        token: u64,
        generation: u64,
    ) -> Result<(), CoreError> {
        let (core, _) = self.core()?;
        let client = core.client().clone();
        let requested = device.clone();
        let (selected, harnesses, models) = super::on_runtime(async move {
            client
                .live_session_catalog(&requested, harness.as_deref())
                .await
        })
        .await?;
        let mut s = self.state.lock().unwrap();
        self.current(&mut s)?;
        s.draft.catalog(
            generation,
            token,
            &device,
            selected,
            harnesses.into_iter().map(Into::into).collect(),
            models.into_iter().map(Into::into).collect(),
        )?;
        Self::menu(&mut s, Menu::Models);
        Ok(())
    }
    async fn folders(&self, path: Option<String>, is_repo: Option<bool>) -> Result<(), CoreError> {
        let (core, generation) = self.core()?;
        let (device, token) = {
            let mut s = self.state.lock().unwrap();
            self.current(&mut s)?;
            s.draft.token += 1;
            s.applied = None;
            s.pending = None;
            (
                s.draft.device.clone().ok_or(CoreError::Closed)?,
                s.draft.token,
            )
        };
        let listing = core.list_folders(device.clone(), path).await?;
        let mut s = self.state.lock().unwrap();
        self.current(&mut s)?;
        if s.draft.generation != generation
            || s.draft.token != token
            || s.draft.device.as_deref() != Some(&device)
        {
            return Err(CoreError::Closed);
        }
        s.folders = Some(listing);
        s.folder_is_repo = is_repo;
        Self::menu(&mut s, Menu::Folders);
        Ok(())
    }
}
#[uniffi::export]
impl CoreAndroidSessions {
    /// Own this for the account's screen lifetime. Construction creates no chat or computer.
    #[uniffi::constructor]
    pub fn new(account: Arc<CoreAccount>, cloud_listener: Arc<dyn CloudChatListener>) -> Arc<Self> {
        let generation = account.snapshot().generation;
        let cloud = account.cloud_chat(cloud_listener);
        let offer = account.own_computer_offer();
        Arc::new(Self {
            account,
            cloud,
            offer,
            instance: format!("android-sessions-{}", uuid::Uuid::new_v4()),
            actions: tokio::sync::Mutex::new(()),
            state: Mutex::new(State {
                draft: Draft::new(generation),
                client: None,
                menu: Menu::Home,
                page: 0,
                revision: 0,
                pending: None,
                applied: None,
                devices: vec![],
                projects: vec![],
                refs: vec![],
                folders: None,
                folder_is_repo: None,
                notice: None,
                closed: false,
            }),
        })
    }
    /// Resume/onboarding completion reuses this exact object; only NewCloud explicitly resets it.
    pub fn default_chat(&self) -> Result<Arc<CoreCloudChat>, CoreError> {
        if self.state.lock().unwrap().closed {
            return Err(CoreError::Closed);
        }
        Ok(self.cloud.clone())
    }
    /// Attach only a client created by this account's existing connect method.
    pub fn attach_client(&self, core: Arc<CoreClient>) -> Result<(), CoreError> {
        let lease = self.account.controller().lease().ok_or(CoreError::Closed)?;
        if !core.client().uses_account(
            &lease.session().account().id,
            lease.session().bearer_token(),
        ) {
            return Err(CoreError::Closed);
        }
        let mut s = self.state.lock().unwrap();
        if s.closed {
            return Err(CoreError::Closed);
        }
        s.draft = Draft::new(lease.generation);
        s.client = Some(core);
        s.pending = None;
        s.applied = None;
        s.menu = Menu::Home;
        Ok(())
    }
    pub fn sharing(
        &self,
        listener: Arc<dyn SharingListener>,
    ) -> Result<Arc<CoreSharing>, CoreError> {
        {
            let mut s = self.state.lock().unwrap();
            self.current(&mut s)?;
        }
        self.account.project_sharing(listener)
    }
    /// The same account/room/grant continuation owner as the other shells.
    pub fn chat_homes(&self, chat: String) -> Result<Arc<CoreChatHomes>, CoreError> {
        let (core, _) = self.core()?;
        self.account.chat_homes(core, chat)
    }
    pub fn view(&self) -> Result<Vec<u8>, CoreError> {
        let mut s = self.state.lock().unwrap();
        // A first-launch account generation may advance before a client is attached.
        if s.client.is_none() && !s.closed {
            s.draft.generation = self.account.snapshot().generation;
        }
        self.current(&mut s)?;
        s.applied = None;
        if let Some(core) = s.client.clone() {
            let workspace = core.workspace();
            s.devices = workspace
                .devices
                .into_iter()
                .filter(|d| d.is_execution_host)
                .collect();
            s.projects = core.projects();
        }
        let mut body = vec![text("title", "New chat", TextRole::Heading)];
        let start = s.page * 16;
        let mut total = 0;
        match s.menu {
            Menu::Home => {
                body.push(button("cloud", "Chat here", true, Intent::Cloud));
                body.push(button(
                    "new-cloud",
                    "New cloud chat",
                    true,
                    Intent::NewCloud,
                ));
                body.push(button(
                    "computers",
                    s.draft
                        .device
                        .as_ref()
                        .and_then(|id| {
                            s.devices
                                .iter()
                                .find(|d| d.id == *id)
                                .map(|d| d.name.as_str())
                        })
                        .unwrap_or("Run on"),
                    s.client.is_some(),
                    Intent::Computers,
                ));
                if s.draft.device.is_some() {
                    if s.draft.models.is_empty() {
                        body.push(button(
                            "retry-models",
                            "Retry models",
                            true,
                            Intent::RetryCatalog,
                        ));
                    }

                    body.push(button(
                        "projects",
                        s.draft
                            .project
                            .as_ref()
                            .map(|p| p.name.as_str())
                            .unwrap_or("Choose a project"),
                        true,
                        Intent::Projects,
                    ));
                    body.push(button(
                        "providers",
                        s.draft
                            .harness
                            .as_ref()
                            .and_then(|id| s.draft.harnesses.iter().find(|h| &h.id == id))
                            .map(|h| h.label.as_str())
                            .unwrap_or("Choose a provider"),
                        !s.draft.harnesses.is_empty(),
                        Intent::Harnesses,
                    ));
                    body.push(button(
                        "models",
                        s.draft
                            .selected()
                            .map(|m| m.label.as_str())
                            .unwrap_or("Choose a model"),
                        !s.draft.models.is_empty(),
                        Intent::Models,
                    ));
                    body.push(button(
                        "efforts",
                        s.draft
                            .effort
                            .as_deref()
                            .map(openagents_client::catalog::reasoning_label)
                            .as_deref()
                            .unwrap_or("Default effort"),
                        s.draft.selected().is_some(),
                        Intent::Efforts,
                    ));
                    if s.draft.project.as_ref().is_some_and(|p| p.git_detected) {
                        body.push(button(
                            "branches",
                            s.draft
                                .branch
                                .as_ref()
                                .map(|b| b.name.as_str())
                                .unwrap_or("Default branch"),
                            true,
                            Intent::Branches,
                        ));
                    }
                    body.push(button(
                        "create",
                        "Create chat",
                        s.draft.session().is_ok(),
                        Intent::Create,
                    ));
                    body.push(button("browse", "Browse folders", true, Intent::Browse));
                }
                body.push(button("sharing", "Shared projects", true, Intent::Sharing));
                body.push(button(
                    "cloud-status",
                    "Our cloud computer",
                    true,
                    Intent::CloudStatus,
                ));
                body.push(button(
                    "cloud-start",
                    "Start cloud computer",
                    true,
                    Intent::CloudStart,
                ));
                body.push(button(
                    "cloud-stop",
                    "Stop cloud computer",
                    true,
                    Intent::CloudStop,
                ));
                // DeviceView does not identify cloud-managed computers. Do not mark the offer done by guessing from names.
                if s.devices.is_empty() && self.offer.shows(self.cloud.clone(), 0) {
                    let card = self.offer.card();
                    body.push(text("own-title", card.title, TextRole::Heading));
                    body.push(text("own-body", card.body, TextRole::Body));
                    body.push(button(
                        "own-copy",
                        "Copy install command",
                        true,
                        Intent::OwnCopy,
                    ));
                    body.push(button(
                        "own-download",
                        card.download_label,
                        true,
                        Intent::OwnComputer,
                    ));
                    body.push(button("own-close", "Close", true, Intent::CloseOwnComputer));
                }
            }
            Menu::Computers => {
                total = s.devices.len();
                for (i, d) in s.devices.iter().enumerate().skip(start).take(16) {
                    body.push(button(
                        format!("computer-{i}"),
                        &d.name,
                        d.online,
                        Intent::Computer(i),
                    ));
                }
            }
            Menu::Projects => {
                let device = s.draft.device.as_deref();
                let projects: Vec<_> = s
                    .projects
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| Some(p.device_id.as_str()) == device)
                    .collect();
                total = projects.len();
                for (i, p) in projects.into_iter().skip(start).take(16) {
                    body.push(button(
                        format!("project-{i}"),
                        &p.name,
                        true,
                        Intent::Project(i),
                    ));
                }
            }
            Menu::Harnesses => {
                total = s.draft.harnesses.len();
                for (i, h) in s.draft.harnesses.iter().enumerate().skip(start).take(16) {
                    body.push(button(
                        format!("harness-{i}"),
                        &h.label,
                        h.offered,
                        Intent::Harness(i),
                    ));
                }
            }
            Menu::Models => {
                total = s.draft.models.len();
                for (i, m) in s.draft.models.iter().enumerate().skip(start).take(16) {
                    body.push(button(
                        format!("model-{i}"),
                        &m.label,
                        true,
                        Intent::Model(i),
                    ));
                }
            }
            Menu::Efforts => {
                body.push(button(
                    "default-effort",
                    "Default effort",
                    true,
                    Intent::Effort(None),
                ));
                if let Some(m) = s.draft.selected() {
                    total = m.reasoning_levels.len();
                    for (i, e) in m.reasoning_levels.iter().enumerate().skip(start).take(16) {
                        body.push(button(
                            format!("effort-{i}"),
                            e,
                            true,
                            Intent::Effort(Some(i)),
                        ));
                    }
                }
            }
            Menu::Branches => {
                total = s.refs.len();
                for (i, r) in s.refs.iter().enumerate().skip(start).take(16) {
                    body.push(button(
                        format!("branch-{i}"),
                        &r.name,
                        true,
                        Intent::Branch(i),
                    ));
                }
            }
            Menu::Folders => {
                if let Some(listing) = &s.folders {
                    body.push(text("folder-path", &listing.path, TextRole::Body));
                    let entries: Vec<_> = listing
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.is_dir)
                        .collect();
                    total = entries.len();
                    for (i, e) in entries.into_iter().skip(start).take(16) {
                        body.push(button(
                            format!("folder-{i}"),
                            &e.name,
                            true,
                            Intent::Folder(i),
                        ));
                    }
                    if s.folder_is_repo.is_some() {
                        body.push(button(
                            "use-folder",
                            "Use this folder",
                            true,
                            Intent::UseFolder,
                        ));
                    }
                    if listing.truncated {
                        body.push(text(
                            "folder-truncated",
                            "This folder has more items. Choose a smaller folder.",
                            TextRole::Status,
                        ));
                    }
                }
            }
        }
        if s.menu != Menu::Home {
            body.push(button("back", "Back", true, Intent::Back));
            if s.page > 0 {
                body.push(button("previous", "Previous", true, Intent::Previous));
            }
            if start + 16 < total {
                body.push(button("next", "Next", true, Intent::Next));
            }
        }
        if let Some(notice) = &s.notice {
            body.push(text("notice", notice, TextRole::Status));
        }
        s.revision = s.revision.checked_add(1).ok_or(CoreError::Closed)?;
        let view = View::new_v3(
            &self.instance,
            s.revision,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children: body,
                },
            ),
        )
        .validate()
        .map_err(|_| failed("Couldn't open chat options."))?;
        let json = view
            .to_json()
            .map_err(|_| failed("Couldn't open chat options."))?;
        s.pending = Some(view);
        Ok(json)
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> Result<(), CoreError> {
        let mut s = self.state.lock().unwrap();
        self.current(&mut s)?;
        let p = s.pending.as_ref().ok_or(CoreError::Closed)?;
        if p.view().instance != instance || p.view().revision != revision {
            return Err(CoreError::Closed);
        }
        s.applied = s.pending.take();
        Ok(())
    }
    pub async fn activate(
        &self,
        instance: String,
        revision: u64,
        key: String,
    ) -> Result<AndroidSessionAction, CoreError> {
        let _action = self
            .actions
            .try_lock()
            .map_err(|_| failed("Wait for the current request to finish."))?;
        let intent = {
            let mut s = self.state.lock().unwrap();
            self.current(&mut s)?;
            s.applied
                .as_ref()
                .ok_or(CoreError::Closed)?
                .activate(&Activation {
                    instance,
                    revision,
                    node: key,
                })
                .map_err(|_| CoreError::Closed)?
                .clone()
        };
        match intent {
            Intent::Cloud => {
                return Ok(AndroidSessionAction::Cloud {
                    chat: self.default_chat()?,
                });
            }
            Intent::NewCloud => {
                self.cloud.new_chat();
                return Ok(AndroidSessionAction::Cloud {
                    chat: self.default_chat()?,
                });
            }
            Intent::Sharing => return Ok(AndroidSessionAction::Sharing),
            Intent::OwnCopy => {
                return Ok(AndroidSessionAction::Clipboard {
                    text: self.offer.card().command,
                });
            }
            Intent::RetryCatalog => {
                let (device, token, generation) = {
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    s.draft.token += 1;
                    s.applied = None;
                    (
                        s.draft.device.clone().ok_or(CoreError::Closed)?,
                        s.draft.token,
                        s.draft.generation,
                    )
                };
                self.catalog(device, None, token, generation).await?;
            }
            Intent::OwnComputer => {
                return Ok(AndroidSessionAction::OpenUrl {
                    url: self.offer.card().download_url,
                });
            }
            Intent::CloseOwnComputer => self.offer.close(),
            Intent::CloudStatus | Intent::CloudStart | Intent::CloudStop => {
                let result = match intent {
                    Intent::CloudStart => self.account.start_cloud_host().await,
                    Intent::CloudStop => self.account.stop_cloud_host().await,
                    _ => self.account.cloud_host().await,
                };
                let mut s = self.state.lock().unwrap();
                self.current(&mut s)?;
                s.notice = Some(match result {
                    Ok(host) => {
                        if host.running {
                            "Your cloud computer is running.".into()
                        } else {
                            "Your cloud computer is stopped.".into()
                        }
                    }
                    Err(error) => return Err(error),
                });
                Self::menu(&mut s, Menu::Home);
            }
            Intent::Computer(index) => {
                let (device, token, generation) = {
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    let d = s
                        .devices
                        .get(index)
                        .filter(|d| d.online && d.is_execution_host)
                        .ok_or(CoreError::Closed)?
                        .id
                        .clone();
                    s.draft.computer(d.clone());
                    s.applied = None;
                    (d, s.draft.token, s.draft.generation)
                };
                self.catalog(device, None, token, generation).await?;
            }
            Intent::Harness(index) => {
                let (device, harness, token, generation) = {
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    let harness = s
                        .draft
                        .harnesses
                        .get(index)
                        .filter(|h| h.offered)
                        .ok_or(CoreError::Closed)?
                        .id
                        .clone();
                    s.draft.token += 1;
                    s.draft.model = None;
                    s.draft.effort = None;
                    s.draft.models.clear();
                    s.applied = None;
                    (
                        s.draft.device.clone().ok_or(CoreError::Closed)?,
                        harness,
                        s.draft.token,
                        s.draft.generation,
                    )
                };
                self.catalog(device, Some(harness), token, generation)
                    .await?;
            }
            Intent::Project(index) => {
                let (core, generation) = self.core()?;
                let (project, token) = {
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    let project = s
                        .projects
                        .get(index)
                        .filter(|p| Some(&p.device_id) == s.draft.device.as_ref())
                        .ok_or(CoreError::Closed)?
                        .clone();
                    s.draft.project = Some(project.clone());
                    s.draft.branch = None;
                    s.refs.clear();
                    s.draft.token += 1;
                    Self::menu(&mut s, Menu::Home);
                    (project, s.draft.token)
                };
                if project.git_detected {
                    let refs = core
                        .list_refs(project.device_id.clone(), project.path.clone())
                        .await?;
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    if s.draft.token != token || s.draft.generation != generation {
                        return Err(CoreError::Closed);
                    }
                    s.refs = refs;
                }
            }
            Intent::Browse => self.folders(None, None).await?,
            Intent::Folder(index) => {
                let (path, is_repo) = {
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    let listing = s.folders.as_ref().ok_or(CoreError::Closed)?;
                    let entry = listing
                        .entries
                        .get(index)
                        .filter(|e| {
                            e.is_dir
                                && !e.name.is_empty()
                                && !e.name.contains(['/', '\\'])
                                && e.name != ".."
                                && e.name != "."
                        })
                        .ok_or(CoreError::Closed)?;
                    (
                        format!("{}/{}", listing.path.trim_end_matches('/'), entry.name),
                        entry.is_repo,
                    )
                };
                self.folders(Some(path), Some(is_repo)).await?;
            }
            Intent::UseFolder => {
                let (core, generation) = self.core()?;
                let (device, path, is_repo, token) = {
                    let s = self.state.lock().unwrap();
                    (
                        s.draft.device.clone().ok_or(CoreError::Closed)?,
                        s.folders.as_ref().ok_or(CoreError::Closed)?.path.clone(),
                        s.folder_is_repo.ok_or(CoreError::Closed)?,
                        s.draft.token,
                    )
                };
                let id = core.create_project(device, path, is_repo).await?;
                let mut s = self.state.lock().unwrap();
                self.current(&mut s)?;
                if s.draft.generation != generation || s.draft.token != token {
                    return Err(CoreError::Closed);
                }
                s.draft.project = core.project(id);
                s.draft.branch = None;
                s.refs.clear();
                Self::menu(&mut s, Menu::Home);
            }
            Intent::Create => {
                let (core, generation) = self.core()?;
                let request = {
                    let mut s = self.state.lock().unwrap();
                    self.current(&mut s)?;
                    let request = s.draft.session()?;
                    if let Some(project) = &s.draft.project {
                        let current = core.project(project.id.clone()).ok_or(CoreError::Closed)?;
                        if current.device_id != project.device_id || current.path != project.path {
                            return Err(CoreError::Closed);
                        }
                    }
                    s.applied = None;
                    request
                };
                let id = core.create_session(request)?;
                let mut s = self.state.lock().unwrap();
                self.current(&mut s)?;
                if s.draft.generation != generation {
                    return Err(CoreError::Closed);
                }
                return Ok(AndroidSessionAction::Session { id, client: core });
            }
            other => {
                let mut s = self.state.lock().unwrap();
                self.current(&mut s)?;
                match other {
                    Intent::Computers => Self::menu(&mut s, Menu::Computers),
                    Intent::Projects => Self::menu(&mut s, Menu::Projects),
                    Intent::Harnesses => Self::menu(&mut s, Menu::Harnesses),
                    Intent::Models => Self::menu(&mut s, Menu::Models),
                    Intent::Efforts => Self::menu(&mut s, Menu::Efforts),
                    Intent::Branches => Self::menu(&mut s, Menu::Branches),
                    Intent::Back => Self::menu(&mut s, Menu::Home),
                    Intent::Next => {
                        s.page += 1;
                        s.applied = None;
                    }
                    Intent::Previous => {
                        s.page = s.page.saturating_sub(1);
                        s.applied = None;
                    }
                    Intent::Model(index) => {
                        let m = s
                            .draft
                            .models
                            .get(index)
                            .ok_or(CoreError::Closed)?
                            .id
                            .clone();
                        s.draft.model = Some(m);
                        s.draft.effort = None;
                        Self::menu(&mut s, Menu::Home);
                    }
                    Intent::Effort(index) => {
                        let effort = index
                            .map(|i| {
                                s.draft
                                    .selected()
                                    .and_then(|m| m.reasoning_levels.get(i))
                                    .cloned()
                                    .ok_or(CoreError::Closed)
                            })
                            .transpose()?;
                        s.draft.effort = effort;
                        Self::menu(&mut s, Menu::Home);
                    }
                    Intent::Branch(index) => {
                        s.draft.branch = Some(s.refs.get(index).ok_or(CoreError::Closed)?.clone());
                        Self::menu(&mut s, Menu::Home);
                    }
                    _ => return Err(CoreError::Closed),
                }
            }
        }
        Ok(AndroidSessionAction::Updated)
    }
    pub fn close(&self) {
        let mut s = self.state.lock().unwrap();
        s.closed = true;
        s.pending = None;
        s.applied = None;
        s.client = None;
        s.draft = Draft::new(s.draft.generation);
        s.projects.clear();
        s.devices.clear();
        s.refs.clear();
        s.folders = None;
    }
}

#[uniffi::export]
impl CoreAndroidSessions {
    /// Saved cloud/web chats use the same continuation/grant owner as live computer chats.
    pub fn saved_chat_homes(
        &self,
        history: Arc<CoreSavedHistory>,
    ) -> Result<Arc<CoreChatHomes>, CoreError> {
        let (core, _) = self.core()?;
        history.chat_homes(core)
    }
    /// Continue through the existing room/grant transfer path, never a second copied session path.
    pub async fn continue_on_selected(
        &self,
        draft: Arc<CoreContinuationDraft>,
    ) -> Result<AndroidSessionAction, CoreError> {
        let _action = self
            .actions
            .try_lock()
            .map_err(|_| failed("Wait for the current request to finish."))?;
        let (core, generation) = self.core()?;
        let new_session = {
            let mut s = self.state.lock().unwrap();
            self.current(&mut s)?;
            if s.draft.device.as_deref() != Some(draft.device_id().as_str()) {
                return Err(CoreError::Closed);
            }
            s.draft.session()?
        };
        let id = draft.create(core.clone(), new_session).await?;
        let mut s = self.state.lock().unwrap();
        self.current(&mut s)?;
        if s.draft.generation != generation {
            return Err(CoreError::Closed);
        }
        Ok(AndroidSessionAction::Session { id, client: core })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use openagents_account::openagents::{
        Client, CredentialRecord, CredentialStore, Deployment, StorageError,
    };
    #[derive(Default)]
    struct Store;
    impl CredentialStore for Store {
        fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
            Ok(None)
        }
        fn save(&self, _: &CredentialRecord) -> Result<(), StorageError> {
            Ok(())
        }
        fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
            Ok(())
        }
    }
    struct AccountEvents;
    impl AccountListener for AccountEvents {
        fn on_account_changed(&self, _: AccountSnapshot) {}
    }
    struct CloudEvents;
    impl CloudChatListener for CloudEvents {
        fn on_cloud_chat_changed(&self, _: CloudChatSnapshot) {}
    }
    fn owner(dir: &std::path::Path) -> Arc<CoreAndroidSessions> {
        let origin = "http://127.0.0.1:9";
        let account = CoreAccount::from_store(
            Client::new(Deployment::new(&format!("{origin}/v1"), origin).unwrap()).unwrap(),
            Arc::new(Store),
            dir.into(),
            Arc::new(AccountEvents),
        );
        CoreAndroidSessions::new(account, Arc::new(CloudEvents))
    }
    #[tokio::test]
    async fn onboarding_resume_and_picker_open_return_the_same_cloud_owner_without_a_computer() {
        let dir = tempfile::tempdir().unwrap();
        let owner = owner(dir.path());
        let a = owner.default_chat().unwrap();
        let b = owner.default_chat().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        let json: serde_json::Value = serde_json::from_slice(&owner.view().unwrap()).unwrap();
        owner
            .acknowledge(owner.instance.clone(), json["revision"].as_u64().unwrap())
            .unwrap();
        let action = owner
            .activate(
                owner.instance.clone(),
                json["revision"].as_u64().unwrap(),
                "cloud".into(),
            )
            .await
            .unwrap();
        let AndroidSessionAction::Cloud { chat } = action else {
            panic!("Expected the existing cloud chat")
        };
        assert!(Arc::ptr_eq(&chat, &a));
        assert!(a.snapshot().chat.is_none());

        let action = owner
            .activate(
                owner.instance.clone(),
                json["revision"].as_u64().unwrap(),
                "new-cloud".into(),
            )
            .await
            .unwrap();
        let AndroidSessionAction::Cloud { chat } = action else {
            panic!("Expected the explicitly reset cloud chat")
        };
        assert!(Arc::ptr_eq(&chat, &a));
        assert!(a.snapshot().chat.is_none());

        for key in ["cloud-status", "cloud-start", "cloud-stop"] {
            let result = owner
                .activate(
                    owner.instance.clone(),
                    json["revision"].as_u64().unwrap(),
                    key.into(),
                )
                .await;
            assert!(matches!(result, Err(CoreError::Auth { .. })), "{key}");
        }
        assert!(owner.state.lock().unwrap().client.is_none());
        owner.close();
        assert!(owner.default_chat().is_err());
    }

    #[tokio::test]
    async fn stale_and_disabled_native_menu_actions_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let owner = owner(dir.path());
        let first: serde_json::Value = serde_json::from_slice(&owner.view().unwrap()).unwrap();
        owner
            .acknowledge(owner.instance.clone(), first["revision"].as_u64().unwrap())
            .unwrap();
        let second: serde_json::Value = serde_json::from_slice(&owner.view().unwrap()).unwrap();
        assert!(matches!(
            owner
                .activate(
                    owner.instance.clone(),
                    first["revision"].as_u64().unwrap(),
                    "cloud".into(),
                )
                .await,
            Err(CoreError::Closed)
        ));
        owner
            .acknowledge(owner.instance.clone(), second["revision"].as_u64().unwrap())
            .unwrap();
        assert!(matches!(
            owner
                .activate(
                    owner.instance.clone(),
                    second["revision"].as_u64().unwrap(),
                    "computers".into(),
                )
                .await,
            Err(CoreError::Closed)
        ));
        owner.close();
    }
    #[test]
    fn previous_or_unmounted_menu_cannot_create_or_select_a_computer() {
        let dir = tempfile::tempdir().unwrap();
        let owner = owner(dir.path());
        let first: serde_json::Value = serde_json::from_slice(&owner.view().unwrap()).unwrap();
        owner
            .acknowledge(owner.instance.clone(), first["revision"].as_u64().unwrap())
            .unwrap();
        let second: serde_json::Value = serde_json::from_slice(&owner.view().unwrap()).unwrap();
        assert!(
            owner
                .acknowledge(owner.instance.clone(), first["revision"].as_u64().unwrap())
                .is_err()
        );
        assert!(owner.state.lock().unwrap().applied.is_none());
        owner
            .acknowledge(owner.instance.clone(), second["revision"].as_u64().unwrap())
            .unwrap();
        owner.close();
        assert!(owner.view().is_err());
    }
}
