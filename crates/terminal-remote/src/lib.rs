//! Shared mounts over the paired-host shell client. Dropping a mount only detaches.
use coder_computers::terminal::Model;
use coder_computers::terminal::model::{Projection, ProjectionTap};
use coder_computers::terminal::session::{Links, Session};
use coder_pty::wire::TerminalRef;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use terminal_core::pty::{Attachment, Event, Program, Transport};

/// An admitted host and the existing connection supervisor for it.
/// Both window and Verse mounts can inject this transport into their application.
pub struct Remote {
    pub host: String,
    pub label: String,
    pub links: Links,
    pub runtime: tokio::runtime::Handle,
    pub local: Arc<dyn Transport>,
    /// Reattach this exact reference instead of opening a shell.
    pub saved: Mutex<Option<TerminalRef>>,
    _owner: Option<(coder_computers::live::Live, tokio::runtime::Runtime)>,
}
impl Remote {
    /// Uses an existing paired-device store. Host grants are checked by the existing client.
    pub fn paired(
        store: &Path,
        host: String,
        saved: Option<TerminalRef>,
        local: Arc<dyn Transport>,
    ) -> Result<Self, String> {
        use coder_computers::live::{FileStore, Live, Settings, load_or_create_key};
        if host.len() != 64
            || !host
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("A host key needs 64 lowercase hexadecimal digits.".into());
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let key = load_or_create_key(store)?;
        let files = FileStore::open(store)?;
        let live = Live::open(
            Settings::new(coder_computers::Platform::Terminal),
            key,
            Box::new(files),
            runtime.handle().clone(),
        )
        .map_err(|e| e.to_string())?;
        let links = live.terminals().links(&host);
        Ok(Self {
            label: format!("Host {}…", &host[..12]),
            host,
            links,
            runtime: runtime.handle().clone(),
            local,
            saved: Mutex::new(saved),
            _owner: Some((live, runtime)),
        })
    }
    /// Pins this explicit mount to the admitted host grant. Route repair is
    /// allowed; replacing its computer, rights, or disclosure needs a new mount.
    pub fn pin_admission(&mut self) {
        let links = self.links.clone();
        let expected = self.host.clone();
        let pinned = Arc::new(Mutex::new(None));
        self.links = Arc::new(move || {
            let link = links()?;
            if link.device().host() != expected {
                return Err(coder_host::access::Error::new(
                    coder_host::access::Code::Denied,
                    "The connection belongs to another computer.",
                ));
            }
            let grant = link.device().access().grant.clone();
            let mut pinned = pinned.lock().map_err(|_| {
                coder_host::access::Error::new(
                    coder_host::access::Code::Denied,
                    "The admission pin is unavailable.",
                )
            })?;
            if pinned.as_ref().is_some_and(|old| old != &grant) {
                return Err(coder_host::access::Error::new(
                    coder_host::access::Code::Denied,
                    "The grant or disclosure changed. Admit a new mount explicitly.",
                ));
            }
            *pinned = Some(grant);
            Ok(link)
        });
    }
    /// Injects the platform's already admitted and supervised host connection.
    pub fn injected(
        host: String,
        label: String,
        links: Links,
        runtime: tokio::runtime::Handle,
        saved: Option<TerminalRef>,
        local: Arc<dyn Transport>,
    ) -> Self {
        Self {
            host,
            label,
            links,
            runtime,
            saved: Mutex::new(saved),
            local,
            _owner: None,
        }
    }
}
impl Transport for Remote {
    fn shell(&self) -> &Path {
        self.local.shell()
    }
    fn open(&self, program: &Program, rows: u16, cols: u16) -> Result<Box<dyn Attachment>, String> {
        if !matches!(program, Program::Shell) {
            return Err(
                "A host terminal opens its admitted shell; local programs need a local pane."
                    .into(),
            );
        }
        let mut model = Model::new(&self.host, &self.label, rows, cols);
        let (sender, events) = mpsc::sync_channel(128);
        let overflow = Arc::new(std::sync::atomic::AtomicBool::new(false));
        model.projection = Some(ProjectionTap {
            sender,
            overflow: overflow.clone(),
        });
        let saved = self
            .saved
            .lock()
            .map_err(|_| "Terminal reference lock failed.")?
            .take();
        let session = match saved {
            Some(reference) => Session::attach(&self.runtime, self.links.clone(), model, reference),
            None => Session::start(&self.runtime, self.links.clone(), model),
        };
        Ok(Box::new(RemoteAttachment {
            session,
            events,
            overflow,
            pending: VecDeque::new(),
            streams: None,
            phase: None,
        }))
    }
    // The client owns the host connection; mount shutdown never stops the host.
    fn shutdown(&self) {}
    fn thread_program(&self) -> Option<Program> {
        self.local.thread_program()
    }
    fn resolve(&self, name: &str) -> Option<PathBuf> {
        self.local.resolve(name)
    }
    fn request(
        &self,
        request: &terminal_core::bridge::Request,
    ) -> Result<terminal_core::bridge::Connection, String> {
        self.local.request(request)
    }
    fn git_summary(&self, pane: u64, directory: String) -> mpsc::Receiver<(u64, String, String)> {
        let (sender, receiver) = mpsc::channel();
        let _ = sender.send((pane, directory, "Host terminal".into()));
        receiver
    }
    fn open_link(&self, target: &str) -> Result<(), String> {
        self.local.open_link(target)
    }
    fn clipboard(&self) -> Option<String> {
        self.local.clipboard()
    }
    fn copy(&self, text: &str) -> Result<(), String> {
        self.local.copy(text)
    }
}
struct RemoteAttachment {
    session: Session,
    events: mpsc::Receiver<Projection>,
    overflow: Arc<std::sync::atomic::AtomicBool>,
    pending: VecDeque<Event>,
    streams: Option<coder_vt::Streams>,
    phase: Option<String>,
}
impl Attachment for RemoteAttachment {
    fn sharing(
        &self,
        action: terminal_core::sharing::Action,
    ) -> Option<mpsc::Receiver<Result<coder_pty::wire::Value, String>>> {
        use coder_computers::terminal::session::SharingCommand;
        use terminal_core::sharing::Action;
        Some(self.session.owner_sharing(match action {
            Action::Read => SharingCommand::Read,
            Action::Issue {
                grantee,
                mode,
                expires_at,
            } => SharingCommand::Issue {
                grantee,
                mode,
                expires_at,
            },
            Action::Pause(paused) => SharingCommand::Pause(paused),
            Action::Revoke(share) => SharingCommand::Revoke(share),
        }))
    }

    fn host_grid(&self) -> bool {
        true
    }
    fn input_available(&self) -> bool {
        let model = self.session.model();
        model.phase == coder_computers::terminal::Phase::Attached
            && !model.watch
            && model.typing != coder_computers::terminal::Typing::Elsewhere
    }
    fn reference(&self) -> Option<TerminalRef> {
        let model = self.session.model();
        let (generation, terminal) = model.reference.as_ref()?;
        Some(TerminalRef {
            generation: generation.clone(),
            terminal: terminal.clone(),
        })
    }
    fn host_answers(&self) -> bool {
        true
    }
    fn input(&self, bytes: &[u8]) {
        self.session.send(bytes.to_vec());
    }
    fn resize(&self, rows: u16, cols: u16) {
        self.session.resize(rows, cols);
    }
    fn close(&self) {
        self.session.close();
    }
    fn target(&self) -> Option<terminal_core::proposals::Binding> {
        let model = self.session.model();
        if model.phase != coder_computers::terminal::Phase::Attached {
            return None;
        }
        let (generation, terminal) = model.reference.as_ref()?;
        let coder_computers::terminal::Blocks::Page { rows, .. } = &model.blocks else {
            return None;
        };
        let cwd = rows.first()?.dir.clone();
        Some(terminal_core::proposals::Binding {
            terminal: terminal.clone(),
            generation: generation.clone(),
            cwd,
            shell_directory: None,
            context_digest: String::new(),
        })
    }
    fn offer_proposal(
        &self,
        proposal: &terminal_core::proposals::Proposal,
    ) -> Option<Result<(), String>> {
        Some(
            self.session
                .owner_proposal(coder_pty::proposal::Action::Offer {
                    proposal: proposal.clone(),
                })
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| {
                    "Proposal disposition is unknown. Reconcile before acting again.".to_owned()
                })
                .and_then(|result| result)
                .map(|_| ()),
        )
    }
    fn decide_proposal(
        &self,
        proposal: &terminal_core::proposals::Proposal,
        approve: bool,
    ) -> Option<Result<coder_pty::proposal::State, String>> {
        Some(
            self.session
                .owner_proposal(coder_pty::proposal::Action::Decide {
                    thread: proposal.thread.clone(),
                    proposal: proposal.id.clone(),
                    revision: proposal.revision,
                    approve,
                    attachment: "0".repeat(64),
                })
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| {
                    "Proposal disposition is unknown. Reconcile before acting again.".to_owned()
                })
                .and_then(|result| result)
                .and_then(|page| {
                    page.entries
                        .first()
                        .map(|entry| entry.state.clone())
                        .ok_or_else(|| "The host returned no proposal disposition.".into())
                }),
        )
    }
    fn directory(&self) -> Option<String> {
        None
    }
    fn poll(&mut self) -> Option<Event> {
        if self.overflow.load(std::sync::atomic::Ordering::Acquire) {
            while self.events.try_recv().is_ok() {}
            self.pending.clear();
            self.streams = None;
            self.overflow
                .store(false, std::sync::atomic::Ordering::Release);
            self.session.reconcile();
            return Some(Event::Gap);
        }
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        let model = self.session.model();
        let status = format!(
            "{} · {} · {} · {} {}",
            model.route.as_deref().unwrap_or("unavailable"),
            model.phase.describe(),
            model.label,
            model
                .reference
                .as_ref()
                .map(|r| r.0.get(..12).unwrap_or(&r.0))
                .unwrap_or("unavailable"),
            model.notice.as_deref().unwrap_or("")
        );
        if self.phase.as_ref() != Some(&status) {
            self.phase = Some(status.clone());
            return Some(if model.phase.ended() {
                Event::End(status)
            } else {
                Event::Status(status)
            });
        }
        if self.streams.is_none() {
            if let Some((generation, terminal)) = &model.reference {
                self.streams = Some(coder_vt::Streams::new(
                    TerminalRef {
                        generation: generation.clone(),
                        terminal: terminal.clone(),
                    },
                    terminal_core::pty::SCROLLBACK,
                ));
            }
        }
        drop(model);
        match self.events.try_recv().ok()? {
            Projection::Size(rows, cols) => Some(Event::Size(rows, cols)),
            Projection::Output(bytes) => Some(Event::Output(bytes)),
            Projection::Gap => Some(Event::Gap),
            Projection::Blocks(page) => Some(Event::Blocks(page)),
            Projection::Records(part) => {
                let events = self.streams.as_mut()?.push(&part).ok()?;
                for event in events {
                    match event {
                        coder_vt::StreamEvent::Ready { terminal, .. } => {
                            self.session.blocks(None);
                            self.pending.push_back(Event::Snapshot(terminal));
                        }
                        coder_vt::StreamEvent::History { epoch, page } => {
                            self.pending.push_back(Event::History { epoch, page })
                        }
                        coder_vt::StreamEvent::Finished { .. } => {}
                    }
                }
                self.pending.pop_front()
            }
        }
    }
}

/// The paired store's actual device identity; a layout cannot choose another client.
pub fn device_id(store: &Path) -> Result<String, String> {
    let key = coder_computers::live::load_or_create_key(store)?;
    Ok(coder_host::reach::pubkey(&key))
}

/// Validates an exact retained reference without opening a replacement terminal.
pub fn reference(generation: &str, terminal: &str) -> Result<TerminalRef, String> {
    if [generation, terminal].iter().any(|part| {
        part.len() != 64
            || !part
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }) {
        return Err(
            "A terminal reference needs two lowercase 64-digit hexadecimal identifiers.".into(),
        );
    }
    Ok(TerminalRef {
        generation: generation.into(),
        terminal: terminal.into(),
    })
}

impl Drop for RemoteAttachment {
    fn drop(&mut self) {
        self.session.leave_wait(std::time::Duration::from_secs(3));
    }
}
