//! Bounded saved-history reads. This module never creates or runs a conversation.
use crate::Lease;
use openagents::{
    CancellationToken, ChatReader, ChatRef, ChatSummary, Error, HistoryIndex, HistoryProject,
    ProjectReader,
};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed {
        message: String,
        stale: bool,
    },
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub signed_in: bool,
    pub login_required: bool,
    pub rows: Vec<ChatSummary>,
    pub projects: Vec<HistoryProject>,
    pub project: Option<HistoryProject>,
    pub state: LoadState,
    pub projects_state: LoadState,
    pub has_more: bool,
    pub more_projects: bool,
}
impl Snapshot {
    /// The same saved-chat ordering on desktop and mobile.
    pub fn visible_rows(&self, archived: bool) -> Vec<ChatSummary> {
        let mut rows = self
            .rows
            .iter()
            .filter(|r| r.archived_unix.is_some() == archived)
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            a.pinned_unix
                .is_none()
                .cmp(&b.pinned_unix.is_none())
                .then_with(|| {
                    if a.pinned_unix.is_some() && b.pinned_unix.is_some() {
                        b.pinned_unix.cmp(&a.pinned_unix)
                    } else {
                        a.project.cmp(&b.project)
                    }
                })
                .then_with(|| b.updated_unix.cmp(&a.updated_unix))
                .then_with(|| a.chat.id().cmp(b.chat.id()))
        });
        rows
    }
}
/// An existing chat in the account that supplied the row. It grants no execution authority.
#[derive(Clone)]
pub struct ExistingChat {
    pub chat: ChatRef,
    pub summary: ChatSummary,
    pub lease: Lease,
}
impl ExistingChat {
    pub fn is_current(&self) -> bool {
        self.lease.is_current()
    }
}
struct State {
    epoch: u64,
    query: u64,
    projects_query: u64,
    replace_projects: bool,
    lease: Option<Lease>,
    cancel: CancellationToken,
    query_cancel: CancellationToken,
    projects_cancel: CancellationToken,
    reader: Option<ChatReader>,
    projects_reader: Option<ProjectReader>,
    index: Option<HistoryIndex>,
    replace: bool,
    snapshot: Snapshot,
}
pub struct History {
    state: Mutex<State>,
    updates: watch::Sender<Snapshot>,
}
impl History {
    pub fn new() -> Arc<Self> {
        let (updates, _) = watch::channel(Snapshot::default());
        Arc::new(Self {
            state: Mutex::new(State {
                epoch: 0,
                query: 0,
                projects_query: 0,
                replace_projects: false,
                lease: None,
                cancel: CancellationToken::new(),
                query_cancel: CancellationToken::new(),
                projects_cancel: CancellationToken::new(),
                reader: None,
                projects_reader: None,
                index: None,
                replace: false,
                snapshot: Snapshot::default(),
            }),
            updates,
        })
    }
    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.updates.subscribe()
    }
    pub fn snapshot(&self) -> Snapshot {
        let state = self.state.lock().unwrap();
        if state.lease.as_ref().is_some_and(|l| !l.is_current()) {
            return Snapshot::default();
        }
        state.snapshot.clone()
    }
    fn publish(&self, state: &State) {
        self.updates.send_replace(state.snapshot.clone());
    }
    /// Replace the complete account context before starting any new reads.
    pub fn reset(&self, lease: Option<Lease>) {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.query_cancel.cancel();
        s.projects_cancel.cancel();
        s.epoch += 1;
        s.query += 1;
        s.projects_query += 1;
        s.replace_projects = false;
        s.snapshot = Snapshot::default();
        s.reader = None;
        s.projects_reader = None;
        s.index = None;
        s.replace = false;
        s.lease = lease.filter(Lease::is_current);
        if let Some(lease) = s.lease.clone() {
            s.cancel = lease.cancel.child_token();
            s.query_cancel = s.cancel.child_token();
            s.projects_cancel = s.cancel.child_token();
            match (
                lease
                    .client()
                    .history(lease.session())
                    .and_then(|r| r.with_page_size(200)),
                lease
                    .client()
                    .history_projects(lease.session())
                    .and_then(|r| r.with_page_size(200)),
                lease.client().history_index(lease.session()),
            ) {
                (Ok(reader), Ok(projects), Ok(index)) => {
                    s.reader = Some(reader);
                    s.projects_reader = Some(projects);
                    s.index = Some(index);
                    s.snapshot.signed_in = true;
                    s.snapshot.has_more = true;
                    s.snapshot.more_projects = true;
                }
                _ => s.lease = None,
            }
        }
        self.publish(&s);
    }
    pub fn existing(&self, id: &str) -> Option<ExistingChat> {
        let s = self.state.lock().unwrap();
        let lease = s.lease.as_ref()?.clone();
        if !lease.is_current() {
            return None;
        }
        let summary = s.index.as_ref()?.get(id)?.clone();
        Some(ExistingChat {
            chat: summary.chat.clone(),
            summary,
            lease,
        })
    }
    /// Open an authorized shared project's saved index, or return to owned chats.
    pub fn project(&self, project: Option<&HistoryProject>) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        let lease = s
            .lease
            .clone()
            .filter(Lease::is_current)
            .ok_or(Error::Cancelled)?;
        let reader = match project {
            Some(p) => lease.client().shared_history(lease.session(), p)?,
            None => lease.client().history(lease.session())?,
        };
        let reader = reader.with_page_size(200)?;
        s.query_cancel.cancel();
        s.query_cancel = s.cancel.child_token();
        s.query += 1;
        s.reader = Some(reader);
        s.index = Some(lease.client().history_index(lease.session())?);
        s.snapshot.rows.clear();
        s.snapshot.project = project.cloned();
        s.snapshot.state = LoadState::Idle;
        s.snapshot.has_more = true;
        s.replace = false;
        self.publish(&s);
        Ok(())
    }
    /// Keep old rows explicitly stale during an outage; replace them on the first fresh page.
    pub fn refresh(&self) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        let lease = s
            .lease
            .clone()
            .filter(Lease::is_current)
            .ok_or(Error::Cancelled)?;
        let reader = match &s.snapshot.project {
            Some(p) => lease.client().shared_history(lease.session(), p)?,
            None => lease.client().history(lease.session())?,
        };
        let reader = reader.with_page_size(200)?;
        s.query_cancel.cancel();
        s.query_cancel = s.cancel.child_token();
        s.query += 1;
        s.reader = Some(reader);
        s.replace = true;
        s.snapshot.state = LoadState::Idle;
        s.snapshot.has_more = true;
        self.publish(&s);
        Ok(())
    }
    pub fn refresh_projects(&self) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        let lease = s
            .lease
            .clone()
            .filter(Lease::is_current)
            .ok_or(Error::Cancelled)?;
        s.projects_reader = Some(
            lease
                .client()
                .history_projects(lease.session())?
                .with_page_size(200)?,
        );
        s.projects_cancel.cancel();
        s.projects_cancel = s.cancel.child_token();
        s.projects_query += 1;
        s.replace_projects = true;
        s.snapshot.more_projects = true;
        s.snapshot.projects_state = LoadState::Idle;
        self.publish(&s);
        Ok(())
    }
    /// Fetch at most one 200-row page. Failed cursors advance only after an explicit retry succeeds.
    pub fn more(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let request = {
            let mut s = self.state.lock().unwrap();
            if !s.snapshot.has_more || s.snapshot.state == LoadState::Loading {
                None
            } else if let (Some(lease), Some(reader)) =
                (s.lease.clone().filter(Lease::is_current), s.reader.clone())
            {
                s.snapshot.state = LoadState::Loading;
                this.publish(&s);
                Some((s.epoch, s.query, lease, reader, s.query_cancel.clone()))
            } else {
                None
            }
        };
        async move {
            let Some((epoch, query, lease, mut reader, cancel)) = request else {
                return;
            };
            let result = reader
                .next_page(lease.client(), lease.session(), &cancel)
                .await;
            let mut s = this.state.lock().unwrap();
            if s.epoch != epoch || s.query != query || !lease.is_current() || cancel.is_cancelled()
            {
                return;
            }
            match result {
                Ok(page) => {
                    if s.replace {
                        s.index = lease.client().history_index(lease.session()).ok();
                        s.replace = false;
                    }
                    let more = page.as_ref().is_some_and(|p| p.has_more);
                    if let Some(page) = page {
                        if let Some(index) = &mut s.index {
                            if let Err(e) = index.extend(page.data) {
                                s.snapshot.state = LoadState::Failed {
                                    message: e.to_string(),
                                    stale: false,
                                };
                                s.snapshot.rows.clear();
                                this.publish(&s);
                                return;
                            }
                        }
                    }
                    s.snapshot.rows = s
                        .index
                        .as_ref()
                        .map(|index| {
                            index
                                .sorted(false)
                                .into_iter()
                                .chain(index.sorted(true))
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    s.reader = Some(reader);
                    s.snapshot.has_more = more;
                    s.snapshot.state = LoadState::Ready;
                }
                Err(e) => {
                    s.snapshot.login_required = matches!(
                        e,
                        Error::Unauthorized
                            | Error::ExpiredSession
                            | Error::WrongAccount
                            | Error::WrongDeployment
                            | Error::InvalidCredential
                    );
                    if lost_access(&e) {
                        s.index = lease.client().history_index(lease.session()).ok();
                        s.snapshot.rows.clear();
                        if let Some(project) = s.snapshot.project.take() {
                            s.snapshot.projects.retain(|p| p.id() != project.id());
                            s.reader = lease
                                .client()
                                .history(lease.session())
                                .and_then(|r| r.with_page_size(200))
                                .ok();
                        }
                        if s.snapshot.login_required {
                            s.snapshot.projects.clear();
                        }
                    }
                    s.snapshot.state = LoadState::Failed {
                        message: e.to_string(),
                        stale: !s.snapshot.rows.is_empty(),
                    };
                }
            }
            this.publish(&s);
        }
    }
    /// Read project names and shared-project access without scanning transcripts.
    pub fn more_projects(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let request = {
            let mut s = self.state.lock().unwrap();
            if !s.snapshot.more_projects || s.snapshot.projects_state == LoadState::Loading {
                None
            } else if let (Some(lease), Some(reader)) = (
                s.lease.clone().filter(Lease::is_current),
                s.projects_reader.clone(),
            ) {
                s.snapshot.projects_state = LoadState::Loading;
                this.publish(&s);
                Some((
                    s.epoch,
                    s.projects_query,
                    lease,
                    reader,
                    s.projects_cancel.clone(),
                ))
            } else {
                None
            }
        };
        async move {
            let Some((epoch, projects_query, lease, mut reader, cancel)) = request else {
                return;
            };
            let result = reader
                .next_page(lease.client(), lease.session(), &cancel)
                .await;
            let mut s = this.state.lock().unwrap();
            if s.epoch != epoch
                || s.projects_query != projects_query
                || !lease.is_current()
                || cancel.is_cancelled()
            {
                return;
            }
            match result {
                Ok(page) => {
                    if s.replace_projects {
                        s.snapshot.projects.clear();
                        s.replace_projects = false;
                    }
                    s.snapshot.more_projects = page.as_ref().is_some_and(|p| p.has_more);
                    if let Some(page) = page {
                        for project in page.data {
                            if !s.snapshot.projects.iter().any(|p| p.id() == project.id()) {
                                s.snapshot.projects.push(project);
                            }
                        }
                    }
                    s.projects_reader = Some(reader);
                    s.snapshot.projects_state = LoadState::Ready;
                }
                Err(e) => {
                    s.snapshot.login_required = matches!(
                        e,
                        Error::Unauthorized
                            | Error::ExpiredSession
                            | Error::WrongAccount
                            | Error::WrongDeployment
                            | Error::InvalidCredential
                    );
                    if lost_access(&e) {
                        s.snapshot.projects.clear();
                        s.snapshot.project = None;
                        s.query_cancel.cancel();
                        s.query_cancel = s.cancel.child_token();
                        s.query += 1;
                        s.reader = lease
                            .client()
                            .history(lease.session())
                            .and_then(|r| r.with_page_size(200))
                            .ok();
                        s.snapshot.has_more = true;
                        s.replace = true;
                        s.index = lease.client().history_index(lease.session()).ok();
                        s.snapshot.rows.clear();
                        s.snapshot.state = LoadState::Failed {
                            message: e.to_string(),
                            stale: false,
                        };
                    }
                    s.snapshot.projects_state = LoadState::Failed {
                        message: e.to_string(),
                        stale: !s.snapshot.projects.is_empty(),
                    };
                }
            }
            this.publish(&s);
        }
    }
}
impl Drop for History {
    fn drop(&mut self) {
        self.state.lock().unwrap().cancel.cancel();
    }
}
fn lost_access(error: &Error) -> bool {
    matches!(
        error,
        Error::Unauthorized
            | Error::Forbidden
            | Error::ExpiredSession
            | Error::WrongAccount
            | Error::WrongDeployment
            | Error::NotFound
    )
}
