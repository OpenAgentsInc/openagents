//! Account-bound agent management and a shared Rust Native creation form.
use super::{CoreClient, CoreError, CoreResult};
use coder_account::{
    Controller, Lease,
    openagents::{
        self,
        agents::{
            Access, AgentDefinition, AgentRequest, AgentSpec, CreateAgent, DeleteAgent,
            UpdateAgent,
            assignments::{AssignmentSpec, CreateAssignment},
        },
    },
};
use rust_native::{Activation, Axis, Element, FieldChange, Node, TextRole, View};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, uniffi::Record)]
pub struct CustomAgentRow {
    pub id: String,
    pub revision: u64,
    pub name: String,
    pub runtime: String,
    pub model: Option<String>,
}
impl From<&AgentDefinition> for CustomAgentRow {
    fn from(a: &AgentDefinition) -> Self {
        Self {
            id: a.id.clone(),
            revision: a.revision,
            name: a.spec.name.clone(),
            runtime: a.spec.runtime.clone(),
            model: a.spec.model.clone(),
        }
    }
}
#[derive(Clone, uniffi::Record)]
pub struct CustomAgentsSnapshot {
    pub generation: u64,
    pub rows: Vec<CustomAgentRow>,
    pub selected: Option<CustomAgentRow>,
    pub form_json: Option<String>,
    pub notice: Option<String>,
    pub saving: bool,
    pub saved: Option<CustomAgentRow>,
    pub assigning: bool,
    pub assignment_chat: Option<String>,
}
#[uniffi::export(with_foreign)]
pub trait CustomAgentsListener: Send + Sync {
    fn on_custom_agents_changed(&self, snapshot: CustomAgentsSnapshot);
}
#[derive(Clone, Serialize, Deserialize)]
enum Intent {
    Name,
    Description,
    Instructions,
    Runner,
    Reload,
    Runtime(String),
    Model(Option<String>),
    Access(Access),
    Save,
    Delete,
}
#[derive(Clone, Serialize, Deserialize)]
enum Mutation {
    Create(AgentRequest<CreateAgent>),
    Update(String, AgentRequest<UpdateAgent>),
    Delete(String, AgentRequest<DeleteAgent>),
}
#[derive(Clone, Serialize, Deserialize)]
struct Form {
    instance: String,
    revision: u64,
    spec: AgentSpec,
    editing: Option<AgentDefinition>,
    pending: Option<Mutation>,
    #[serde(default)]
    conflict: bool,
}
type ModelCatalogs = BTreeMap<String, Vec<zeron_client::catalog::ModelInfo>>;

struct State {
    catalogs: ModelCatalogs,
    model_device: String,
    catalog_ticket: u64,
    generation: u64,
    account_generation: u64,
    rows: Vec<AgentDefinition>,
    selected: Option<String>,
    form: Option<Form>,
    notice: Option<String>,
    saving: bool,
    saved: Option<CustomAgentRow>,
    assigning: bool,
    assignment_chat: Option<String>,
    list_ticket: u64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            catalogs: Default::default(),
            model_device: String::new(),
            catalog_ticket: 0,
            generation: 1,
            account_generation: 0,
            rows: vec![],
            selected: None,
            form: None,
            notice: None,
            saving: false,
            saved: None,
            assigning: false,
            assignment_chat: None,
            list_ticket: 0,
        }
    }
}
impl State {
    fn apply_models(
        &mut self,
        account: u64,
        ticket: u64,
        device: &str,
        catalogs: ModelCatalogs,
    ) -> bool {
        if self.account_generation != account
            || self.catalog_ticket != ticket
            || self.model_device != device
            || self.catalogs == catalogs
        {
            return false;
        }
        self.catalogs = catalogs;
        self.bump();
        true
    }
    fn bump(&mut self) {
        self.generation += 1;
        if let Some(f) = &mut self.form {
            f.revision += 1;
        }
    }
}
#[derive(uniffi::Object)]
pub struct CoreAgents {
    io: Mutex<()>,
    controller: Arc<Controller>,
    directory: PathBuf,
    state: Mutex<State>,
    listener: Arc<dyn CustomAgentsListener>,
}
fn failed(message: &str) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
fn node(key: impl Into<String>, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn text(key: &str, value: &str) -> Node<Intent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role: TextRole::Body,
        },
    )
}
fn choice(
    key: String,
    label: String,
    selected: bool,
    enabled: bool,
    intent: Intent,
) -> Node<Intent> {
    node(
        key,
        Element::Choice {
            label,
            selected,
            enabled,
            intent,
            children: vec![],
        },
    )
}
fn form_view(
    form: &Form,
    saving: bool,
    catalogs: &ModelCatalogs,
) -> Result<rust_native::ValidatedView<Intent>, rust_native::ViewError> {
    let editable = !saving && form.pending.is_none();
    let field = |key: &str, label: &str, value: &str, max, multiline, intent| {
        node(
            key,
            Element::Field {
                label: label.into(),
                value: value.into(),
                placeholder: String::new(),
                secret: false,
                multiline,
                enabled: editable,
                max_bytes: max,
                on_change: intent,
            },
        )
    };
    let mut children = vec![
        field("name", "Name", &form.spec.name, 80, false, Intent::Name),
        field(
            "description",
            "Description",
            &form.spec.description,
            512,
            false,
            Intent::Description,
        ),
        field(
            "instructions",
            "Instructions",
            &form.spec.instructions,
            16384,
            true,
            Intent::Instructions,
        ),
        text("runtime-label", "Runs with"),
    ];
    for (id, label) in [
        ("claude-code", "Claude Code"),
        ("codex", "Codex"),
        ("sdk", "SDK runner"),
    ] {
        children.push(choice(
            format!("runtime-{id}"),
            label.into(),
            form.spec.runtime == id,
            editable,
            Intent::Runtime(id.into()),
        ));
    }
    if form.spec.runtime == "sdk" {
        children.push(field(
            "runner",
            "Installed runner name",
            form.spec.model.as_deref().unwrap_or_default(),
            128,
            false,
            Intent::Runner,
        ));
    }
    if form.spec.runtime != "sdk" {
        children.push(text("model-label", "Model"));
        children.push(choice(
            "model-default".into(),
            "Default model".into(),
            form.spec.model.is_none(),
            editable,
            Intent::Model(None),
        ));
        let mut models = catalogs
            .get(&form.spec.runtime)
            .cloned()
            .unwrap_or_else(|| zeron_client::catalog::fallback_models(&form.spec.runtime));
        models.retain(|m| {
            !m.id.eq_ignore_ascii_case("default")
                && !m.id.is_empty()
                && m.id.len() <= 256
                && !m.id.chars().any(char::is_control)
        });
        for model in &mut models {
            model.label = model
                .label
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect();
            if model.label.trim().is_empty() {
                model.label = model.id.clone();
            }
        }
        if let Some(selected) = &form.spec.model {
            if !models.iter().take(128).any(|m| &m.id == selected) {
                models.insert(
                    0,
                    zeron_client::catalog::ModelInfo {
                        id: selected.clone(),
                        label: zeron_client::catalog::fallback_models(&form.spec.runtime)
                            .into_iter()
                            .find(|model| &model.id == selected)
                            .map(|model| model.label)
                            .unwrap_or_else(|| selected.clone()),
                        description: None,
                        reasoning_levels: vec![],
                        options: vec![],
                    },
                );
            }
        }
        for (i, model) in models.into_iter().take(128).enumerate() {
            children.push(choice(
                format!("model-{i}"),
                model.label,
                form.spec.model.as_deref() == Some(&model.id),
                editable,
                Intent::Model(Some(model.id)),
            ));
        }
    }
    children.push(text("access-label", "Access"));
    children.push(choice(
        "access-default".into(),
        "This computer's settings".into(),
        form.spec.access == Access::HostDefault,
        editable,
        Intent::Access(Access::HostDefault),
    ));
    if form.spec.runtime == "codex" {
        for (key, label, access) in [
            ("access-read", "Read only", Access::ReadOnly),
            ("access-write", "Edit project files", Access::WorkspaceWrite),
        ] {
            children.push(choice(
                key.into(),
                label.into(),
                form.spec.access == access,
                editable,
                Intent::Access(access),
            ));
        }
    }
    let deleting = matches!(form.pending, Some(Mutation::Delete(..)));
    children.push(node(
        "save",
        Element::Button {
            label: if saving {
                "Saving…"
            } else if form.editing.is_some() {
                "Save agent"
            } else {
                "Create agent"
            }
            .into(),
            enabled: !saving && !deleting && form.spec.validate().is_ok(),
            icon: None,
            shortcut: None,
            intent: Intent::Save,
        },
    ));
    if form.editing.is_some() {
        children.push(node(
            "delete",
            Element::Button {
                label: "Delete agent".into(),
                enabled: !saving && (form.pending.is_none() || deleting),
                icon: None,
                shortcut: None,
                intent: Intent::Delete,
            },
        ));
    }
    if form.conflict {
        children.push(node(
            "reload",
            Element::Button {
                label: "Reload saved agent".into(),
                enabled: !saving,
                icon: None,
                shortcut: None,
                intent: Intent::Reload,
            },
        ));
    }
    View::new_v3(
        &form.instance,
        form.revision,
        node(
            "agent-form",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        ),
    )
    .validate()
}
impl CoreAgents {
    pub(super) fn new(
        controller: Arc<Controller>,
        directory: PathBuf,
        listener: Arc<dyn CustomAgentsListener>,
    ) -> Arc<Self> {
        let this = Arc::new(Self {
            io: Mutex::new(()),
            controller: controller.clone(),
            directory,
            state: Mutex::new(State::default()),
            listener,
        });
        let weak = Arc::downgrade(&this);
        let mut updates = controller.watch();
        zeron_client::runtime::shared().spawn(async move {
            while updates.changed().await.is_ok() {
                let Some(this) = weak.upgrade() else { break };
                let generation = updates.borrow().generation;
                let mut state = this.state.lock().unwrap();
                if state.account_generation != generation {
                    *state = State {
                        account_generation: generation,
                        ..Default::default()
                    };
                }
                drop(state);
                this.publish();
            }
        });
        this.refresh();
        this
    }
    fn lease(&self) -> CoreResult<Lease> {
        let Some(lease) = self.controller.lease() else {
            *self.state.lock().unwrap() = State::default();
            return Err(failed("Sign in to manage your agents."));
        };
        let mut state = self.state.lock().unwrap();
        if state.account_generation != lease.generation {
            *state = State {
                account_generation: lease.generation,
                ..Default::default()
            };
        }
        Ok(lease)
    }
    fn publish(&self) {
        self.listener.on_custom_agents_changed(self.snapshot());
    }
    fn folder(&self, lease: &Lease) -> PathBuf {
        self.directory.join(format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}:{}:{}",
                    lease.session().account().id,
                    lease.client().deployment().api_base(),
                    lease.client().deployment().website()
                )
                .as_bytes()
            )
        ))
    }
    fn checkpoint(&self, lease: &Lease, form: &Form) -> CoreResult<()> {
        let folder = self.folder(lease);
        std::fs::create_dir_all(&folder)
            .map_err(|_| failed("The request could not be saved on this device."))?;
        let bytes =
            serde_json::to_vec(form).map_err(|_| failed("The request could not be saved."))?;
        private_write(&folder.join("form.json"), &bytes)
    }
    fn begin(&self, editing: Option<AgentDefinition>) -> CoreResult<()> {
        let io = self.io.lock().unwrap();
        let lease = self.lease()?;
        let path = self.folder(&lease).join("form.json");
        let mut form = if path.exists() {
            let bytes =
                std::fs::read(path).map_err(|_| failed("The saved change could not be opened."))?;
            if bytes.len() > 65536 {
                return Err(failed("The saved change could not be opened."));
            }
            serde_json::from_slice::<Form>(&bytes)
                .map_err(|_| failed("The saved change could not be opened."))?
        } else {
            Form {
                instance: openagents::agents::request_id()
                    .map_err(|_| failed("The form could not be opened."))?,
                revision: 1,
                spec: editing
                    .as_ref()
                    .map(|d| d.spec.clone())
                    .unwrap_or(AgentSpec {
                        name: String::new(),
                        description: String::new(),
                        instructions: String::new(),
                        runtime: "claude-code".into(),
                        model: None,
                        access: Access::HostDefault,
                    }),
                editing,
                pending: None,
                conflict: false,
            }
        };
        form.instance = openagents::agents::request_id()
            .map_err(|_| failed("The form could not be opened."))?;
        form.revision = 1;
        let mut state = self.state.lock().unwrap();
        if state.saving {
            return Err(failed("Wait for the current change to finish."));
        }
        state.form = Some(form);
        state.saved = None;
        state.notice = None;
        state.bump();
        drop(state);
        drop(io);
        self.publish();
        Ok(())
    }
    fn save(self: &Arc<Self>, delete: bool) -> CoreResult<()> {
        let io = self.io.lock().unwrap();
        let lease = self.lease()?;
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| failed("Sign in again."))?;
        let mut state = self.state.lock().unwrap();
        if state.saving {
            return Ok(());
        }
        let form = state
            .form
            .as_mut()
            .ok_or_else(|| failed("Open the agent form again."))?;
        if form.pending.is_none() {
            form.pending = Some(if delete {
                let d = form
                    .editing
                    .as_ref()
                    .ok_or_else(|| failed("Choose an agent."))?;
                Mutation::Delete(
                    d.id.clone(),
                    agents
                        .delete_request(d.revision)
                        .map_err(|_| failed("The request could not be saved."))?,
                )
            } else if let Some(d) = &form.editing {
                Mutation::Update(
                    d.id.clone(),
                    agents
                        .update_request(d.revision, form.spec.clone())
                        .map_err(|_| failed("Check the agent fields."))?,
                )
            } else {
                Mutation::Create(
                    agents
                        .create_request(form.spec.clone())
                        .map_err(|_| failed("Check the agent fields."))?,
                )
            });
        }
        let pending = form.pending.clone().unwrap();
        let instance = form.instance.clone();
        self.checkpoint(&lease, form)?;
        state.saving = true;
        state.notice = None;
        state.bump();
        drop(state);
        drop(io);
        self.publish();
        let weak = Arc::downgrade(self);
        zeron_client::runtime::shared().spawn(async move {
            let result = async {
                let agents = lease.client().agents(lease.session())?;
                match &pending {
                    Mutation::Create(r) => agents.create(r, &lease.cancel).await,
                    Mutation::Update(id, r) => agents.update(id, r, &lease.cancel).await,
                    Mutation::Delete(id, r) => agents.delete(id, r, &lease.cancel).await,
                }
            }
            .await;
            let Some(this) = weak.upgrade() else { return };
            if !lease.is_current() {
                return;
            }
            let mut state = this.state.lock().unwrap();
            state.saving = false;
            match result {
                Ok(d) => {
                    let _ = std::fs::remove_file(this.folder(&lease).join("form.json"));
                    state.list_ticket += 1;
                    state.rows.retain(|a| a.id != d.id);
                    if !d.deleted {
                        state.rows.push(d.clone());
                    }
                    if state.form.as_ref().is_some_and(|f| f.instance == instance) {
                        state.form = None;
                        state.saved = (!d.deleted).then(|| CustomAgentRow::from(&d));
                        state.selected = (!d.deleted).then(|| d.id.clone());
                    }
                }
                Err(e) => {
                    if matches!(
                        e,
                        openagents::Error::Refused {
                            code: Some("version_conflict"),
                            ..
                        }
                    ) {
                        if let Some(f) = &mut state.form {
                            f.conflict = true;
                        }
                    }
                    state.notice = Some(e.to_string());
                }
            }
            state.bump();
            drop(state);
            this.publish();
        });
        Ok(())
    }
}
#[uniffi::export]
impl CoreAgents {
    pub fn snapshot(&self) -> CustomAgentsSnapshot {
        let lease = self.lease().ok();
        let state = self.state.lock().unwrap();
        let current = lease.as_ref().is_some_and(|l| {
            l.generation == state.account_generation || state.account_generation == 0
        });
        let rows = if current {
            state.rows.iter().map(CustomAgentRow::from).collect()
        } else {
            vec![]
        };
        let selected = if current {
            state
                .rows
                .iter()
                .find(|d| Some(&d.id) == state.selected.as_ref())
                .map(CustomAgentRow::from)
        } else {
            None
        };
        CustomAgentsSnapshot {
            generation: state.generation,
            rows,
            selected,
            form_json: if current {
                state
                    .form
                    .as_ref()
                    .and_then(|f| form_view(f, state.saving, &state.catalogs).ok())
                    .and_then(|v| v.to_json().ok())
                    .and_then(|b| String::from_utf8(b).ok())
            } else {
                None
            },
            notice: state.notice.clone(),
            saving: state.saving,
            saved: if current { state.saved.clone() } else { None },
            assigning: state.assigning,
            assignment_chat: if current {
                state.assignment_chat.clone()
            } else {
                None
            },
        }
    }
    pub fn refresh(self: &Arc<Self>) {
        let Ok(lease) = self.lease() else { return };
        let ticket = {
            let mut s = self.state.lock().unwrap();
            s.account_generation = lease.generation;
            s.list_ticket += 1;
            s.list_ticket
        };
        let weak = Arc::downgrade(self);
        zeron_client::runtime::shared().spawn(async move {
            let result = async {
                let agents = lease.client().agents(lease.session())?;
                let mut rows = vec![];
                let mut after = None;
                loop {
                    let page = agents.list(after.as_deref(), 100, &lease.cancel).await?;
                    rows.extend(page.data);
                    after = page.next;
                    if after.is_none() {
                        break;
                    }
                }
                Ok::<_, openagents::Error>(rows)
            }
            .await;
            let Some(this) = weak.upgrade() else { return };
            if !lease.is_current() {
                return;
            }
            let mut s = this.state.lock().unwrap();
            if s.list_ticket != ticket {
                return;
            }
            match result {
                Ok(rows) => {
                    s.rows = rows;
                    s.notice = None;
                }
                Err(e) => s.notice = Some(e.to_string()),
            }
            s.bump();
            drop(s);
            this.publish();
        });
    }
    /// Use this computer's cached choices immediately, then refresh from its runtimes.
    pub fn load_models(
        self: &Arc<Self>,
        core: Arc<CoreClient>,
        device_id: String,
    ) -> CoreResult<()> {
        let lease = self.lease()?;
        let client = core.client().clone();
        let mut state = self.state.lock().unwrap();
        state.catalog_ticket += 1;
        let ticket = state.catalog_ticket;
        state.model_device = device_id.clone();
        state.catalogs = ["claude-code", "codex"]
            .into_iter()
            .map(|h| (h.into(), client.cached_models(&device_id, h)))
            .collect();
        state.bump();
        drop(state);
        self.publish();
        if device_id.is_empty() {
            return Ok(());
        }
        for harness in ["claude-code", "codex"] {
            let weak = Arc::downgrade(self);
            let client = client.clone();
            let lease = lease.clone();
            let device = device_id.clone();
            zeron_client::runtime::shared().spawn(async move {
                let models = tokio::select! {
                    _ = lease.cancel.cancelled() => return,
                    models = client.list_models(&device, harness) => models,
                };
                let Some(this) = weak.upgrade() else { return };
                if !lease.is_current() {
                    return;
                }
                let mut state = this.state.lock().unwrap();
                let mut catalogs = state.catalogs.clone();
                catalogs.insert(harness.into(), models);
                let changed = state.apply_models(lease.generation, ticket, &device, catalogs);
                drop(state);
                if changed {
                    this.publish();
                }
            });
        }
        Ok(())
    }
    pub fn create_form(&self) -> CoreResult<()> {
        self.begin(None)
    }
    pub fn edit_form(&self, id: String) -> CoreResult<()> {
        self.lease()?;
        let d = self
            .state
            .lock()
            .unwrap()
            .rows
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or_else(|| failed("Reload your agents and try again."))?;
        self.begin(Some(d))
    }
    pub fn select(&self, id: String) -> CoreResult<()> {
        self.lease()?;
        let mut s = self.state.lock().unwrap();
        if !s.rows.iter().any(|d| d.id == id) {
            return Err(failed("Reload your agents and try again."));
        }
        s.selected = Some(id);
        s.assignment_chat = None;
        s.saved = None;
        s.bump();
        drop(s);
        self.publish();
        Ok(())
    }
    pub fn selection_active(&self) -> bool {
        let _ = self.lease();
        self.state.lock().unwrap().selected.is_some()
    }
    pub fn clear_selection(&self) {
        let mut s = self.state.lock().unwrap();
        s.selected = None;
        s.saved = None;
        s.assignment_chat = None;
        s.bump();
        drop(s);
        self.publish();
    }
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        self.lease()?;
        let mut s = self.state.lock().unwrap();
        let f = s
            .form
            .as_ref()
            .ok_or_else(|| failed("Open the form again."))?;
        let event = FieldChange {
            activation: Activation {
                instance,
                revision,
                node: key,
            },
            value: value.clone(),
        };
        let intent = form_view(f, s.saving, &s.catalogs)
            .map_err(|_| failed("Open the form again."))?
            .change_field(&event)
            .map_err(|_| failed("The field changed. Try again."))?
            .clone();
        let f = s.form.as_mut().unwrap();
        match intent {
            Intent::Name => f.spec.name = value,
            Intent::Description => f.spec.description = value,
            Intent::Instructions => f.spec.instructions = value,
            Intent::Runner => f.spec.model = Some(value),
            _ => return Err(failed("Choose an editable field.")),
        };
        s.bump();
        drop(s);
        self.publish();
        Ok(())
    }
    pub fn activate(
        self: &Arc<Self>,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<()> {
        self.lease()?;
        let mut s = self.state.lock().unwrap();
        let f = s
            .form
            .as_ref()
            .ok_or_else(|| failed("Open the form again."))?;
        let intent = form_view(f, s.saving, &s.catalogs)
            .map_err(|_| failed("Open the form again."))?
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(|_| failed("The form changed. Try again."))?
            .clone();
        match intent {
            Intent::Reload => {
                let id = f
                    .editing
                    .as_ref()
                    .ok_or_else(|| failed("Choose an agent."))?
                    .id
                    .clone();
                drop(s);
                let lease = self.lease()?;
                let this = Arc::downgrade(self);
                zeron_client::runtime::shared().spawn(async move {
                    let result = match lease.client().agents(lease.session()) {
                        Ok(agents) => agents.get(&id, None, &lease.cancel).await,
                        Err(e) => Err(e),
                    };
                    let Some(this) = this.upgrade() else { return };
                    if !lease.is_current() {
                        return;
                    }
                    match result {
                        Ok(d) if !d.deleted => {
                            let _ = std::fs::remove_file(this.folder(&lease).join("form.json"));
                            let mut s = this.state.lock().unwrap();
                            if let Some(f) = &mut s.form {
                                f.spec = d.spec.clone();
                                f.editing = Some(d);
                                f.pending = None;
                                f.conflict = false;
                            }
                            s.notice = None;
                            s.bump();
                            drop(s);
                            this.publish();
                        }
                        _ => {
                            let mut s = this.state.lock().unwrap();
                            s.notice = Some("The agent could not be reloaded. Try again.".into());
                            s.bump();
                            drop(s);
                            this.publish();
                        }
                    }
                });
                return Ok(());
            }
            Intent::Save => {
                drop(s);
                return self.save(false);
            }
            Intent::Delete => {
                drop(s);
                return self.save(true);
            }
            Intent::Runtime(runtime) => {
                let f = s.form.as_mut().unwrap();
                f.spec.runtime = runtime;
                f.spec.model = None;
                f.spec.access = Access::HostDefault;
            }
            Intent::Model(model) => s.form.as_mut().unwrap().spec.model = model,
            Intent::Access(access) => s.form.as_mut().unwrap().spec.access = access,
            _ => return Err(failed("Choose a control.")),
        };
        s.bump();
        drop(s);
        self.publish();
        Ok(())
    }
    pub fn assign(
        self: &Arc<Self>,
        core: Arc<CoreClient>,
        host: String,
        project: Option<String>,
        task: String,
    ) -> CoreResult<()> {
        let io = self.io.lock().unwrap();
        let lease = self.lease()?;
        if core.client.user_id() != lease.session().account().id {
            return Err(failed("Sign in again before assigning work."));
        }
        let definition = {
            let s = self.state.lock().unwrap();
            if s.assigning {
                return Ok(());
            }
            s.rows
                .iter()
                .find(|d| Some(&d.id) == s.selected.as_ref())
                .cloned()
                .ok_or_else(|| failed("Choose an agent."))?
        };
        let spec = AssignmentSpec {
            agent: definition.id,
            agent_revision: definition.revision,
            host,
            project,
            task,
        };
        spec.validate().map_err(failed)?;
        let folder = self.folder(&lease);
        std::fs::create_dir_all(&folder).map_err(|_| failed("The request could not be saved."))?;
        let path = folder.join("assignment.json");
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| failed("Sign in again."))?;
        let request: AgentRequest<CreateAssignment> = if path.exists() {
            let bytes = std::fs::read(&path)
                .map_err(|_| failed("The previous request could not be opened."))?;
            if bytes.len() > 96 * 1024 {
                return Err(failed("The previous request could not be opened."));
            }
            let previous: AgentRequest<CreateAssignment> = serde_json::from_slice(&bytes)
                .map_err(|_| failed("The previous request could not be opened."))?;
            if previous.body.spec != spec {
                return Err(failed(
                    "Finish the previous assignment before starting another.",
                ));
            }
            previous
        } else {
            agents
                .assignment_request(spec)
                .map_err(|_| failed("The assignment could not be saved."))?
        };
        private_write(
            &path,
            &serde_json::to_vec(&request).map_err(|_| failed("The request could not be saved."))?,
        )?;
        {
            let mut s = self.state.lock().unwrap();
            s.assigning = true;
            s.assignment_chat = None;
            s.notice = None;
            s.bump();
        }
        drop(io);
        self.publish();
        let weak = Arc::downgrade(self);
        zeron_client::runtime::shared().spawn(async move {
            let mut refusal = None;
            let result = async {
                let agents = lease.client().agents(lease.session())?;
                let a = agents.assign(&request, &lease.cancel).await?;
                for _ in 0..160 {
                    if !lease.is_current() {
                        return Err(openagents::Error::Cancelled);
                    }
                    let observed = agents.assignment(&a.id, &lease.cancel).await?;
                    if matches!(
                        observed.state,
                        openagents::agents::assignments::AssignmentState::Failed
                    ) {
                        let _ = std::fs::remove_file(&path);
                        refusal = observed.result;
                        return Err(openagents::Error::Refused {
                            status: 409,
                            code: None,
                        });
                    }
                    if core.client.session_config(&a.chat).is_some() {
                        return Ok(a.chat);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
                Err(openagents::Error::Unavailable { retry_after: None })
            }
            .await;
            let Some(this) = weak.upgrade() else { return };
            if !lease.is_current() {
                return;
            }
            let mut s = this.state.lock().unwrap();
            s.assigning = false;
            match result {
                Ok(chat) => {
                    s.assignment_chat = Some(chat);
                    let _ = std::fs::remove_file(&path);
                }
                Err(e) => s.notice = refusal.or_else(|| Some(e.to_string())),
            };
            s.bump();
            drop(s);
            this.publish();
        });
        Ok(())
    }
}
fn private_write(path: &std::path::Path, bytes: &[u8]) -> CoreResult<()> {
    use std::io::Write;
    let pending = path.with_extension("pending");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&pending)
        .map_err(|_| failed("The request could not be saved on this device."))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| failed("The request could not be saved."))?;
    std::fs::rename(pending, path).map_err(|_| failed("The request could not be saved."))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form() -> Form {
        Form {
            instance: "fixture".into(),
            revision: 1,
            spec: AgentSpec {
                name: "Reviewer".into(),
                description: String::new(),
                instructions: "Find defects".into(),
                runtime: "codex".into(),
                model: None,
                access: Access::ReadOnly,
            },
            editing: None,
            pending: None,
            conflict: false,
        }
    }
    fn model(id: &str) -> zeron_client::catalog::ModelInfo {
        zeron_client::catalog::ModelInfo {
            id: id.into(),
            label: id.into(),
            description: None,
            reasoning_levels: vec![],
            options: vec![],
        }
    }
    #[test]
    fn forms_use_the_computers_models_and_keep_a_saved_choice() {
        let mut f = form();
        f.spec.runtime = "claude-code".into();
        let fallback = String::from_utf8(
            form_view(&f, false, &ModelCatalogs::new())
                .unwrap()
                .to_json()
                .unwrap(),
        )
        .unwrap();
        assert!(fallback.contains("Opus 5.5"));
        f.spec.model = Some("saved-model".into());
        let catalogs = [("claude-code".into(), vec![model("new-live-model")])].into();
        let view = form_view(&f, false, &catalogs).unwrap();
        let json = String::from_utf8(view.to_json().unwrap()).unwrap();
        assert!(json.contains("new-live-model"));
        assert!(json.contains("saved-model"));
        assert!(
            !json.contains("Opus 5.5"),
            "The live list replaces the bundled choices"
        );
        assert!(
            matches!(view.activate(&Activation { instance: f.instance.clone(), revision: f.revision, node: "model-0".into() }).unwrap(), Intent::Model(Some(id)) if id=="saved-model")
        );
        assert!(
            matches!(view.activate(&Activation { instance: f.instance.clone(), revision: f.revision, node: "model-1".into() }).unwrap(), Intent::Model(Some(id)) if id=="new-live-model")
        );
    }
    #[test]
    fn late_model_replies_cannot_cross_computers_accounts_or_newer_requests() {
        let mut state = State {
            account_generation: 4,
            catalog_ticket: 2,
            model_device: "second-computer".into(),
            form: Some(form()),
            ..Default::default()
        };
        let catalogs = || [("codex".into(), vec![model("live-model")])].into();
        assert!(!state.apply_models(4, 1, "first-computer", catalogs()));
        assert!(!state.apply_models(3, 2, "second-computer", catalogs()));
        assert!(!state.apply_models(4, 1, "second-computer", catalogs()));
        let before = state.form.as_ref().unwrap().clone();
        assert!(state.apply_models(4, 2, "second-computer", catalogs()));
        let after = state.form.as_ref().unwrap();
        assert_eq!(after.spec, before.spec);
        assert_eq!(after.instance, before.instance);
        assert!(after.revision > before.revision);
        let view = form_view(after, false, &state.catalogs).unwrap();
        assert!(
            view.activate(&Activation {
                instance: before.instance,
                revision: before.revision,
                node: "model-0".into()
            })
            .is_err()
        );
        state = State {
            account_generation: 5,
            ..Default::default()
        };
        assert!(state.catalogs.is_empty());
        assert!(!state.apply_models(4, 2, "second-computer", catalogs()));
    }
    #[test]
    fn long_catalogs_keep_the_selected_model_and_invalid_labels_cannot_break_the_form() {
        let mut f = form();
        f.spec.model = Some("model-149".into());
        let mut models: Vec<_> = (0..150).map(|i| model(&format!("model-{i}"))).collect();
        models[0].label = "Huge title\0".repeat(10000);
        let catalogs = [("codex".into(), models)].into();
        let view = form_view(&f, false, &catalogs).unwrap();
        let json = String::from_utf8(view.to_json().unwrap()).unwrap();
        assert!(!json.contains("\\u0000"));
        assert!(
            matches!(view.activate(&Activation { instance: f.instance, revision: f.revision, node: "model-0".into() }).unwrap(), Intent::Model(Some(id)) if id=="model-149")
        );
    }

    #[test]
    fn form_labels_use_plain_copy() {
        fn check(value: &serde_json::Value) {
            match value {
                serde_json::Value::Object(map) => {
                    if let Some(label) = map.get("label").and_then(|v| v.as_str()) {
                        assert!(oa_copy::violations(label, &[]).is_empty(), "{label}");
                    }
                    for value in map.values() {
                        check(value);
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        check(value);
                    }
                }
                _ => {}
            }
        }
        check(
            &serde_json::from_slice::<serde_json::Value>(
                &form_view(&form(), false, &ModelCatalogs::new())
                    .unwrap()
                    .to_json()
                    .unwrap(),
            )
            .unwrap(),
        );
    }
    #[test]
    fn form_rejects_stale_callbacks_and_disables_edits_during_uncertain_save() {
        let f = form();
        let view = form_view(&f, false, &ModelCatalogs::new()).unwrap();
        let event = FieldChange {
            activation: Activation {
                instance: f.instance.clone(),
                revision: 1,
                node: "name".into(),
            },
            value: "Edited".into(),
        };
        assert!(matches!(view.change_field(&event).unwrap(), Intent::Name));
        let mut stale = event.clone();
        stale.activation.revision = 0;
        assert!(view.change_field(&stale).is_err());
        let mut foreign = event.clone();
        foreign.activation.instance = "old account form".into();
        assert!(view.change_field(&foreign).is_err());
        assert!(
            form_view(&f, true, &ModelCatalogs::new())
                .unwrap()
                .change_field(&event)
                .is_err()
        );
        let mut oversized = event;
        oversized.value = "é".repeat(41);
        assert!(view.change_field(&oversized).is_err());
    }
    #[test]
    fn sdk_runner_and_conflict_recovery_are_shared_form_intents() {
        let mut f = form();
        f.spec.runtime = "sdk".into();
        f.conflict = true;
        f.editing = Some(AgentDefinition {
            id: "a".repeat(64),
            revision: 1,
            spec: f.spec.clone(),
            deleted: false,
        });
        let view = form_view(&f, false, &ModelCatalogs::new()).unwrap();
        assert!(
            view.activate(&Activation {
                instance: f.instance.clone(),
                revision: 1,
                node: "model-default".into()
            })
            .is_err(),
            "An SDK runner name must not be replaced by a model choice"
        );
        assert!(matches!(
            view.activate(&Activation {
                instance: f.instance.clone(),
                revision: 1,
                node: "reload".into()
            })
            .unwrap(),
            Intent::Reload
        ));
        assert!(matches!(
            view.change_field(&FieldChange {
                activation: Activation {
                    instance: f.instance.clone(),
                    revision: 1,
                    node: "runner".into()
                },
                value: "reviewer".into()
            })
            .unwrap(),
            Intent::Runner
        ));
    }
}
