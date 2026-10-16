//! Account-bound Agent Lab state and durable SDK requests shared by native shells.
use crate::{Controller, Lease};
use openagents::{
    self,
    agent_lab::{
        self as api,
        view::{Action, Command, Workbench},
        *,
    },
};
use rust_native::{Activation, FieldChange, View};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Clone, Serialize, Deserialize)]
enum Pending {
    Save {
        document: Option<String>,
        request: api::Request<SaveDocument>,
    },
    Start {
        document: String,
        request: api::Request<StartRun>,
    },
    Promote {
        document: String,
        request: api::Request<Promote>,
    },
    Change {
        run: String,
        request: api::Request<RunChange>,
    },
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    #[default]
    None,
    Session {
        id: String,
    },
}
pub struct Lab {
    account: Option<String>,
    controller: Arc<Controller>,
    root: PathBuf,
    state: Mutex<Workbench>,
    updates: tokio::sync::watch::Sender<u64>,
    io: tokio::sync::Mutex<()>,
}
impl Lab {
    pub fn new(controller: Arc<Controller>, root: PathBuf) -> Arc<Self> {
        let (tx, _) = tokio::sync::watch::channel(1);
        Arc::new(Self {
            account: controller.lease().map(|l| l.session().account().id.clone()),
            controller,
            root,
            state: Mutex::new(Workbench::new(
                openagents::agents::request_id().unwrap_or_else(|_| "lab".into()),
            )),
            updates: tx,
            io: tokio::sync::Mutex::new(()),
        })
    }
    pub fn watch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.updates.subscribe()
    }
    fn publish(&self) {
        self.updates.send_modify(|r| *r = r.wrapping_add(1));
    }
    pub fn view(&self) -> View<Action> {
        if self.lease().is_err() {
            let mut s = Workbench::new("signed-out".into());
            s.notice = Some("Sign in to use Agent Lab.".into());
            s.busy = true;
            return s.view();
        }
        self.state.lock().unwrap().view()
    }
    pub fn snapshot(&self) -> Workbench {
        self.state.lock().unwrap().clone()
    }
    fn lease(&self) -> Result<Lease, String> {
        self.controller
            .lease()
            .filter(|l| Some(&l.session().account().id) == self.account.as_ref())
            .ok_or_else(|| "Sign in to use Agent Lab.".into())
    }
    fn path(&self, lease: &Lease) -> PathBuf {
        let identity =
            serde_json::to_vec(&(lease.client().deployment(), &lease.session().account().id))
                .unwrap_or_default();
        self.root
            .join(format!("{:x}", Sha256::digest(identity)))
            .join("pending.json")
    }
    pub fn change_field(&self, event: FieldChange) -> Result<(), String> {
        self.lease()?;
        let mut state = self.state.lock().unwrap();
        let action = state
            .view()
            .validate()
            .map_err(|_| "Couldn't open Agent Lab.")?
            .change_field(&event)
            .map_err(|_| "This field changed. Try it again.")?
            .clone();
        state.field(&action.0, event.value)?;
        drop(state);
        self.publish();
        Ok(())
    }
    pub async fn refresh(&self) -> Result<(), String> {
        let lease = self.lease()?;
        let labs = lease
            .client()
            .agent_lab(lease.session())
            .map_err(|_| "Sign in again.")?;
        let mut documents = vec![];
        let mut after = None;
        loop {
            let page = labs
                .documents(after.as_deref(), &lease.cancel)
                .await
                .map_err(|_| "Couldn't load your experiments.")?;
            documents.extend(page.data);
            after = page.next;
            if after.is_none() || documents.len() >= 64 {
                break;
            }
        }
        let mut runs = vec![];
        after = None;
        loop {
            let page = labs
                .runs(None, after.as_deref(), &lease.cancel)
                .await
                .map_err(|_| "Couldn't load your results.")?;
            runs.extend(page.data);
            after = page.next;
            if after.is_none() || runs.len() >= 128 {
                break;
            }
        }
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| "Sign in again.")?;
        let mut definitions = vec![];
        after = None;
        loop {
            let page = agents
                .list(after.as_deref(), 100, &lease.cancel)
                .await
                .map_err(|_| "Couldn't load your agents.")?;
            definitions.extend(page.data);
            after = page.next;
            if after.is_none() {
                break;
            }
        }
        let hosts = labs
            .hosts(&lease.cancel)
            .await
            .map_err(|_| "Couldn't load your computers.")?;
        if !lease.is_current() {
            return Ok(());
        }
        let mut s = self.state.lock().unwrap();
        // Keep the full opened document for version browsing; catalog pages may contain only its latest version.
        if let Some(id) = &s.document {
            if let Some(old) = s.documents.iter().find(|d| &d.id == id) {
                if let Some(item) = documents.iter_mut().find(|d| &d.id == id) {
                    if old.versions.len() > item.versions.len() {
                        *item = old.clone();
                    }
                }
            }
        }
        s.pending = self.path(&lease).exists();
        s.documents = documents;
        s.runs = runs;
        s.agents = definitions;
        s.hosts = hosts;
        if s.draft.host == "choose" {
            if let Some(h) = s.hosts.first().cloned() {
                s.draft.host = h.id;
                s.draft.project = h
                    .projects
                    .first()
                    .map(|p| p.id.clone())
                    .unwrap_or_else(|| "choose".into());
            }
        }
        s.bump();
        drop(s);
        self.publish();
        Ok(())
    }
    pub async fn activate(&self, event: Activation) -> Result<Effect, String> {
        let lease = self.lease()?;
        let _gate = self.io.lock().await;
        let command = {
            let mut s = self.state.lock().unwrap();
            let action = s
                .view()
                .validate()
                .map_err(|_| "Couldn't open Agent Lab.")?
                .activate(&event)
                .map_err(|_| "This view changed. Try that action again.")?
                .clone();
            s.action(&action.0)?
        };
        let Some(command) = command else {
            self.publish();
            return Ok(Effect::None);
        };
        if let Command::Session(id) = command {
            return Ok(Effect::Session { id });
        }
        if let Command::Refresh = command {
            self.refresh().await?;
            return Ok(Effect::None);
        }
        let labs = lease
            .client()
            .agent_lab(lease.session())
            .map_err(|_| "Sign in again.")?;
        if let Command::Load(id) = command {
            let d = labs
                .document(&id, &lease.cancel)
                .await
                .map_err(|_| "Couldn't open this experiment.")?;
            if !lease.is_current() {
                return Ok(Effect::None);
            }
            let mut s = self.state.lock().unwrap();
            let v = d
                .versions
                .last()
                .ok_or("This experiment has no saved version.")?;
            if let Content::Experiment { experiment } = serde_json::from_value(v.content.clone())
                .map_err(|_| "Couldn't open this experiment.")?
            {
                s.draft = experiment;
                s.inputs.clear();
                s.document = Some(id.clone());
                s.base = v.number;
                s.dirty = false;
                s.documents.retain(|d| d.id != id);
                s.documents.push(d);
                s.bump();
            }
            drop(s);
            self.publish();
            return Ok(Effect::None);
        }
        let path = self.path(&lease);
        let stopping = matches!(command, Command::Stop(_));
        let pending = if path.exists() {
            serde_json::from_slice::<Pending>(
                &std::fs::read(&path).map_err(|_| "Couldn't open your saved request.")?,
            )
            .map_err(|_| "Couldn't open your saved request.")?
        } else {
            let s = self.state.lock().unwrap();
            let id = s.document.clone();
            let request_id =
                openagents::agents::request_id().map_err(|_| "Try this action again.")?;
            match command {
                Command::Save => {
                    s.draft.validate().map_err(|e| e.message)?;
                    Pending::Save {
                        document: id,
                        request: labs.bind(SaveDocument {
                            request_id,
                            expected_revision: s
                                .documents
                                .iter()
                                .find(|d| Some(&d.id) == s.document.as_ref())
                                .and_then(|d| d.versions.last())
                                .map(|v| v.number)
                                .unwrap_or(0),
                            content: Content::Experiment {
                                experiment: s.draft.clone(),
                            },
                            summary: if s.base == 0 {
                                "Created experiment"
                            } else {
                                "Updated experiment"
                            }
                            .into(),
                        }),
                    }
                }
                Command::Start => Pending::Start {
                    document: id.ok_or("Save this experiment first.")?,
                    request: labs.bind(StartRun {
                        request_id,
                        version: s.base,
                    }),
                },
                Command::Promote => Pending::Promote {
                    document: id.ok_or("Save this experiment first.")?,
                    request: labs.bind(Promote {
                        request_id,
                        version: s.base,
                    }),
                },
                Command::Stop(id) | Command::Archive(id) => {
                    let r = s
                        .runs
                        .iter()
                        .find(|r| r.id == id)
                        .ok_or("Choose an experiment run.")?;
                    Pending::Change {
                        run: id,
                        request: labs.bind(RunChange {
                            request_id,
                            expected_revision: r.revision,
                            action: if stopping {
                                RunAction::Stop
                            } else {
                                RunAction::Archive
                            },
                        }),
                    }
                }
                Command::Review { run, trial, passed } => {
                    let r = s
                        .runs
                        .iter()
                        .find(|r| r.id == run)
                        .ok_or("Choose an experiment run.")?;
                    let note = s
                        .review_notes
                        .get(&format!("review-note:{run}:{trial}"))
                        .cloned()
                        .unwrap_or_default();
                    Pending::Change {
                        run,
                        request: labs.bind(RunChange {
                            request_id,
                            expected_revision: r.revision,
                            action: RunAction::Review {
                                trial,
                                passed,
                                note,
                            },
                        }),
                    }
                }
                _ => return Err("Choose an available action.".into()),
            }
        };
        private_write(
            &path,
            &serde_json::to_vec(&pending).map_err(|_| "Couldn't save this request.")?,
        )?;
        {
            let mut s = self.state.lock().unwrap();
            s.busy = true;
            s.pending = true;
            s.notice = None;
            s.bump();
        }
        self.publish();
        let result: Result<(), openagents::Error> = async {
            match pending {
                Pending::Save { document, request } => {
                    let d = labs
                        .save(document.as_deref(), &request, &lease.cancel)
                        .await?;
                    if lease.is_current() {
                        let mut s = self.state.lock().unwrap();
                        s.document = Some(d.id.clone());
                        s.base = d.versions.last().map(|v| v.number).unwrap_or_default();
                        s.dirty = false;
                        if let Content::Experiment { experiment } = &request.body.content {
                            s.draft = experiment.clone();
                        }
                        s.documents.retain(|v| v.id != d.id);
                        s.documents.push(d);
                    }
                }
                Pending::Start { document, request } => {
                    let r = labs.start(&document, &request, &lease.cancel).await?;
                    if lease.is_current() {
                        let mut s = self.state.lock().unwrap();
                        s.selected_run = Some(r.id.clone());
                        s.runs.retain(|v| v.id != r.id);
                        s.runs.push(r);
                    }
                }
                Pending::Promote { document, request } => {
                    let d = labs.promote(&document, &request, &lease.cancel).await?;
                    if lease.is_current() {
                        let mut s = self.state.lock().unwrap();
                        s.notice = Some(format!("{} is ready in your agent picker.", d.spec.name));
                    }
                }
                Pending::Change { run, request } => {
                    let r = labs.change(&run, &request, &lease.cancel).await?;
                    if lease.is_current() {
                        let mut s = self.state.lock().unwrap();
                        s.runs.retain(|v| v.id != r.id);
                        if !r.archived {
                            s.runs.push(r)
                        }
                    }
                }
            }
            Ok(())
        }
        .await;
        if !lease.is_current() {
            return Ok(Effect::None);
        }
        let mut s = self.state.lock().unwrap();
        s.busy = false;
        match result {
            Ok(()) => {
                let _ = std::fs::remove_file(&path);
                s.pending = false;
            }
            Err(e) => {
                if matches!(
                    e,
                    openagents::Error::Refused { .. }
                        | openagents::Error::Forbidden
                        | openagents::Error::InvalidReference
                ) {
                    let _ = std::fs::remove_file(&path);
                    s.pending = false;
                }
                s.notice = Some(e.to_string());
            }
        }
        s.bump();
        drop(s);
        self.publish();
        Ok(Effect::None)
    }
}
fn private_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("Couldn't save this request.")?;
    std::fs::create_dir_all(parent).map_err(|_| "Couldn't save this request.")?;
    let temporary = path.with_extension("new");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "Couldn't save this request.")?;
    file.write_all(bytes)
        .map_err(|_| "Couldn't save this request.")?;
    file.sync_all().map_err(|_| "Couldn't save this request.")?;
    std::fs::rename(temporary, path).map_err(|_| "Couldn't save this request.")?;
    Ok(())
}
