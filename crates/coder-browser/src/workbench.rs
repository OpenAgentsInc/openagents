//! The shared terminal application over an existing admitted host attachment.
use super::{Error, Incoming, Result, State};
use coder_host_wire::TermRequest;
use coder_pty::{
    client::{Applied, TerminalState},
    ext::{Features, RecordsFrame},
    proposal::{self, Action, Page},
    wire::{self, Body, Mode, TerminalRef, TerminalResult, Value},
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
};
use terminal_core::{
    Application,
    bridge::{Connection, Request},
    pty::{Attachment, Event, Program, Sessions, Transport},
};
const OUTPUT_MAX: usize = 256 * 1024;
struct IO {
    terminal: TerminalRef,
    attachment: String,
    state: State,
    typist: bool,
    interactive: bool,
    uses_typist: bool,
    resize: Option<wire::Size>,
    opened: bool,
    outbound: Option<TermRequest>,
    incoming: VecDeque<Event>,
    bytes: usize,
    clipboard: Option<String>,
}
impl IO {
    fn send(&mut self, request: TermRequest) -> bool {
        let read = matches!(
            &request,
            TermRequest::Seat(_)
                | TermRequest::BlockPage(_)
                | TermRequest::SessionRead(_)
                | TermRequest::SessionList(_)
                | TermRequest::Proposal(proposal::Request {
                    action: Action::Read { .. },
                    ..
                })
        );
        if self.state != State::Ready || (!self.typist && !read) || self.outbound.is_some() {
            return false;
        }
        self.outbound = Some(request);
        self.state = State::Unknown;
        true
    }
}
#[derive(Clone)]
struct Remote(Arc<Mutex<IO>>);
impl Attachment for Remote {
    fn host_grid(&self) -> bool {
        true
    }
    fn host_answers(&self) -> bool {
        true
    }
    fn input(&self, bytes: &[u8]) {
        let mut io = self.0.lock().unwrap();
        let mut input =
            wire::Input::new(coder_reach::new_id(), io.terminal.clone(), bytes.to_vec());
        if io.uses_typist {
            input.requires = vec![coder_pty::ext::TYPIST.into()];
            input.attachment = Some(io.attachment.clone());
        }
        io.send(TermRequest::Input(input));
    }
    fn resize(&self, rows: u16, cols: u16) {
        let mut io = self.0.lock().unwrap();
        let mut resize = wire::Resize::new(
            coder_reach::new_id(),
            io.terminal.clone(),
            wire::Size { rows, cols },
        );
        if io.uses_typist {
            resize.requires = vec![coder_pty::ext::TYPIST.into()];
            resize.attachment = Some(io.attachment.clone());
        }
        if io.send(TermRequest::Resize(resize)) {
            io.resize = Some(wire::Size { rows, cols });
        }
    }
    fn close(&self) {
        let mut io = self.0.lock().unwrap();
        // A browser pane detaches; closing the page never kills its host terminal.
        io.outbound = None;
        io.state = State::Disconnected;
    }
    fn poll(&mut self) -> Option<Event> {
        let mut io = self.0.lock().unwrap();
        let event = io.incoming.pop_front()?;
        if let Event::Output(bytes) = &event {
            io.bytes = io.bytes.saturating_sub(bytes.len());
        }
        Some(event)
    }
    fn target(&self) -> Option<terminal_core::proposals::Binding> {
        None
    }
    fn directory(&self) -> Option<String> {
        None
    }
}
impl Transport for Remote {
    fn shell(&self) -> &Path {
        Path::new("host-terminal")
    }
    fn open(
        &self,
        program: &Program,
        _: u16,
        _: u16,
    ) -> std::result::Result<Box<dyn Attachment>, String> {
        let mut io = self.0.lock().unwrap();
        if io.opened || !matches!(program, Program::Shell) {
            return Err(
                "Select an admitted host session; browser-local programs are unavailable.".into(),
            );
        }
        io.opened = true;
        Ok(Box::new(self.clone()))
    }
    fn shutdown(&self) {
        Attachment::close(self);
    }
    fn thread_program(&self) -> Option<Program> {
        None
    }
    fn resolve(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn request(&self, _: &Request) -> std::result::Result<Connection, String> {
        Err("Select a host-admitted thread; local helpers are unavailable.".into())
    }
    fn git_summary(&self, _: u64, _: String) -> Receiver<(u64, String, String)> {
        mpsc::channel().1
    }
    fn open_link(&self, _: &str) -> std::result::Result<(), String> {
        Err("Open terminal links with the browser's explicit link control.".into())
    }
    fn clipboard(&self) -> Option<String> {
        None
    }
    fn copy(&self, text: &str) -> std::result::Result<(), String> {
        self.0.lock().unwrap().clipboard = Some(text.into());
        Ok(())
    }
}
/// The browser mounts the native application state and grid, with no local process adapter.
pub struct Workbench {
    pub core: Application,
    io: Arc<Mutex<IO>>,
    state: TerminalState,
    restore: coder_vt::Restore,
    records: coder_pty::ext::Assembler,
    pub features: Features,
    pub proposals: Option<Page>,
    pub blocks: Vec<coder_pty::ext::Block>,
    pub session: Option<coder_pty::ext::SessionRecord>,
    pub notice: Option<String>,
    composing: bool,
}
impl Workbench {
    pub fn new(
        terminal: TerminalRef,
        attachment: String,
        mode: Mode,
        size: wire::Size,
        features: Features,
    ) -> Self {
        let io = Arc::new(Mutex::new(IO {
            terminal: terminal.clone(),
            attachment,
            state: if features.snapshot {
                State::Behind
            } else {
                State::Ready
            },
            typist: mode == Mode::Interact && !features.typist,
            interactive: mode == Mode::Interact,
            uses_typist: features.typist,
            resize: None,
            opened: false,
            outbound: None,
            incoming: VecDeque::new(),
            bytes: 0,
            clipboard: None,
        }));
        let mut core = Application::new(Sessions(Arc::new(Remote(io.clone()))));
        core.cell = [8.0, 16.0];
        core.area = terminal_core::layout::Rect {
            x: 0.0,
            y: 0.0,
            w: f32::from(size.cols) * 8.0 + 8.0,
            h: f32::from(size.rows) * 16.0 + 26.0,
        };
        core.new_tab(&Program::Shell);
        core.open = true;
        core.focused = true;
        core.paper.on = false;
        Self {
            core,
            io,
            state: TerminalState::new(
                terminal.clone(),
                usize::from(size.rows),
                usize::from(size.cols),
            ),
            restore: coder_vt::Restore::new(5000),
            records: coder_pty::ext::Assembler::new(coder_pty::ext::StreamKind::Snapshot, terminal),
            features,
            proposals: None,
            blocks: vec![],
            session: None,
            notice: None,
            composing: false,
        }
    }
    pub fn state(&self) -> State {
        self.io.lock().unwrap().state
    }
    pub fn can_type(&self) -> bool {
        let io = self.io.lock().unwrap();
        io.state == State::Ready && io.typist
    }
    pub fn share_status(&self) -> &'static str {
        if self.features.shares {
            "Host terminal sharing available"
        } else {
            "Terminal sharing unavailable on this host"
        }
    }
    pub fn dispatch(&mut self) -> Option<TermRequest> {
        self.io.lock().unwrap().outbound.take()
    }
    pub fn result(&mut self, result: &TerminalResult) {
        let mut io = self.io.lock().unwrap();
        io.state = match result.reason {
            Some(wire::Reason::Revoked | wire::Reason::NotAdmitted) => State::Revoked,
            Some(wire::Reason::Lost | wire::Reason::Stale) => State::Stale,
            _ => State::Ready,
        };
        if result.reason == Some(wire::Reason::NotTypist) {
            io.typist = false;
        }
        let resize = io.resize.take();
        if result.status != wire::Status::Refused {
            if let Some(size) = resize {
                if let Some(pane) = self.core.focused_pane() {
                    pane.session.vt.resize(size.rows.into(), size.cols.into());
                }
            }
        }
        match &result.value {
            Some(Value::Proposals { page }) => self.proposals = Some(page.clone()),
            Some(Value::Blocks { page }) => self.blocks = page.blocks.clone(),
            Some(Value::Session { record }) => self.session = Some(record.clone()),
            _ => {}
        }
    }
    pub fn disconnect(&mut self) {
        let mut io = self.io.lock().unwrap();
        io.state = State::Disconnected;
        io.outbound = None;
        io.typist = false;
        self.proposals = None;
        self.notice =
            Some("Connection lost. Reattach and read the current host state before typing.".into());
    }
    pub fn input(&mut self, text: &str) {
        if self.can_type() && !self.composing {
            self.core.send(text.as_bytes());
        }
    }
    pub fn composition(&mut self, active: bool) {
        self.composing = active;
    }
    pub fn commit_composition(&mut self, text: &str) {
        self.composing = false;
        self.input(text);
    }
    pub fn paste(&mut self, text: &str) {
        if self.can_type() {
            self.core.paste(text);
        } else {
            self.notice = Some("This attachment cannot type.".into());
        }
    }
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if !self.can_type() {
            return;
        }
        let Some(sessions) = self.core.sessions.as_ref().map(|s| Sessions(s.0.clone())) else {
            return;
        };
        if let Some(pane) = self.core.focused_pane() {
            sessions.resize(&mut pane.session, rows, cols);
        }
    }
    pub fn key(&mut self, key: &terminal_core::input::KeyIn) -> bool {
        if self.composing || !self.can_type() {
            return false;
        }
        self.core.key(key)
    }
    pub fn take_typist(&mut self) -> Result<()> {
        let mut io = self.io.lock().unwrap();
        if !self.features.typist || !io.interactive {
            return Err(Error::NotAdmitted);
        }
        let request = TermRequest::Seat(coder_pty::ext::Seat::take(
            coder_reach::new_id(),
            io.terminal.clone(),
            io.attachment.clone(),
        ));
        if io.send(request) {
            Ok(())
        } else {
            Err(Error::Disconnected)
        }
    }
    pub fn read_blocks(&mut self, before: Option<u64>) -> Result<()> {
        if !self.features.blocks {
            return Err(Error::NotAdmitted);
        }
        let mut io = self.io.lock().unwrap();
        let request = TermRequest::BlockPage(coder_pty::ext::BlockPageRead::new(
            coder_reach::new_id(),
            io.terminal.clone(),
            before,
            16,
        ));
        if io.send(request) {
            Ok(())
        } else {
            Err(Error::Disconnected)
        }
    }
    pub fn read_proposals(&mut self) -> Result<()> {
        if !self.features.proposals {
            return Err(Error::NotAdmitted);
        }
        self.proposal(Action::Read { limit: 8 })
    }
    pub fn decide(
        &mut self,
        thread: &str,
        proposal: &str,
        revision: u64,
        approve: bool,
    ) -> Result<()> {
        let page = self.proposals.as_ref().ok_or(Error::Stale)?;
        let entry = page
            .entries
            .iter()
            .find(|e| {
                e.proposal.thread == thread
                    && e.proposal.id == proposal
                    && e.proposal.revision == revision
                    && matches!(
                        e.state,
                        proposal::State::Pending | proposal::State::Warned { .. }
                    )
            })
            .ok_or(Error::Stale)?;
        let attachment = self.io.lock().unwrap().attachment.clone();
        let action = Action::Decide {
            thread: entry.proposal.thread.clone(),
            proposal: entry.proposal.id.clone(),
            revision: entry.proposal.revision,
            approve,
            attachment,
        };
        self.proposal(action)
    }
    fn proposal(&mut self, action: Action) -> Result<()> {
        let mut io = self.io.lock().unwrap();
        let request = TermRequest::Proposal(proposal::Request::new(
            coder_reach::new_id(),
            io.terminal.clone(),
            action,
        ));
        if io.send(request) {
            Ok(())
        } else {
            Err(Error::Disconnected)
        }
    }
    pub fn incoming(&mut self, incoming: Incoming) -> Result<()> {
        match incoming {
            Incoming::Frame(frame) => {
                if frame.attachment != self.io.lock().unwrap().attachment {
                    return Err(Error::Malformed);
                }
                let applied = self.state.apply(&frame);
                match applied {
                    Applied::Output { .. } => {
                        if let Body::Output { data, .. } = frame.body {
                            let mut io = self.io.lock().unwrap();
                            if io.bytes.saturating_add(data.len()) > OUTPUT_MAX {
                                drop(io);
                                self.disconnect();
                                return Err(Error::Limit);
                            }
                            io.bytes += data.len();
                            io.incoming.push_back(Event::Output(data));
                        }
                    }
                    Applied::Typist { typist, size } => {
                        if let Some(pane) = self.core.focused_pane() {
                            pane.session.vt.resize(size.rows.into(), size.cols.into());
                        }
                        let mut io = self.io.lock().unwrap();
                        io.typist = typist.as_ref() == Some(&io.attachment);
                    }
                    Applied::Gap { .. } | Applied::Behind { .. } => {
                        self.notice =
                            Some("Output gap. Reattach with a current screen snapshot.".into());
                        self.io.lock().unwrap().state = State::Behind;
                    }
                    Applied::Detached(_) => self.disconnect(),
                    Applied::Exit(_) => {
                        self.disconnect();
                        self.notice = Some("The host terminal exited.".into());
                    }
                    Applied::Refused(_) => return Err(Error::Malformed),
                    _ => {}
                }
            }
            Incoming::Records(records) => self.records(records)?,
        }
        self.core.tick();
        Ok(())
    }
    fn records(&mut self, frame: RecordsFrame) -> Result<()> {
        let records = self.records.push(&frame).map_err(|_| Error::Malformed)?;
        for record in records {
            match &record {
                coder_pty::ext::Record::History(page) => {
                    let epoch = self
                        .restore
                        .binding()
                        .map(|b| b.epoch)
                        .ok_or(Error::Malformed)?;
                    if let Some(pane) = self.core.focused_pane() {
                        pane.session
                            .vt
                            .attach_history(epoch, page)
                            .map_err(|_| Error::Malformed)?;
                    }
                    continue;
                }
                coder_pty::ext::Record::Finish(_) => continue,
                _ => {}
            }
            if let Some(vt) = self.restore.push(&record).map_err(|_| Error::Malformed)? {
                if let Some(pane) = self.core.focused_pane() {
                    pane.session.vt = vt;
                }
                if let Some(binding) = self.restore.binding() {
                    self.state = TerminalState::new(
                        TerminalRef {
                            generation: binding.generation.clone(),
                            terminal: binding.terminal.clone(),
                        },
                        binding.size.rows as usize,
                        binding.size.cols as usize,
                    )
                    .starting_after(binding.through);
                }
                self.io.lock().unwrap().state = State::Ready;
            }
        }
        Ok(())
    }
    pub fn clipboard(&mut self) -> Option<String> {
        self.io
            .lock()
            .unwrap()
            .clipboard
            .take()
            .or_else(|| self.core.copied.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(mode: Mode) -> Workbench {
        Workbench::new(
            TerminalRef {
                generation: "a".repeat(64),
                terminal: "b".repeat(64),
            },
            "c".repeat(64),
            mode,
            wire::Size::new(24, 80),
            Features {
                snapshot: false,
                typist: false,
                ..Features::ALL
            },
        )
    }
    #[test]
    fn watch_revoked_and_disconnected_input_is_never_queued() {
        let mut m = model(Mode::Observe);
        m.input("x");
        m.paste("p");
        assert!(m.dispatch().is_none());
        let mut m = model(Mode::Interact);
        m.disconnect();
        m.input("x");
        assert!(m.dispatch().is_none());
        m.io.lock().unwrap().state = State::Revoked;
        m.input("x");
        assert!(m.dispatch().is_none());
    }
    #[test]
    fn composition_commits_unicode_once_and_unknown_dispatch_cannot_queue_more_input() {
        let mut m = model(Mode::Interact);
        m.composition(true);
        m.input("a");
        assert!(m.dispatch().is_none());
        m.commit_composition("日本語🦀");
        let TermRequest::Input(first) = m.dispatch().unwrap() else {
            panic!()
        };
        assert_eq!(first.data, "日本語🦀".as_bytes());
        m.input("second");
        assert!(m.dispatch().is_none());
        assert_eq!(m.state(), State::Unknown);
        m.disconnect();
        assert!(m.dispatch().is_none());
    }
    #[test]
    fn host_snapshot_restores_full_screen_and_resource_identity() {
        let mut m = model(Mode::Interact);
        let terminal = m.io.lock().unwrap().terminal.clone();
        let mut host = coder_vt::Terminal::new(24, 80, 5000);
        host.feed(b"\x1b[?1049h\x1b[2J\x1b[3;4Hfull-screen fixture");
        let records = host
            .snapshot(&coder_vt::Binding {
                terminal: terminal.clone(),
                through: 8,
                exit: None,
            })
            .unwrap();
        let bytes = coder_pty::ext::encode_stream(&records);
        for frame in
            coder_pty::ext::frames(&terminal, &"c".repeat(64), &"d".repeat(64), &bytes, 1024)
        {
            m.incoming(Incoming::Records(frame)).unwrap();
        }
        let pane = m.core.focused_pane().unwrap();
        assert!(pane.session.vt.alternate_screen());
        assert_eq!(pane.session.vt.text(), host.text());
        assert_eq!(m.state.resume_after(), 8);
        assert_eq!(m.io.lock().unwrap().terminal, terminal);
    }
    #[test]
    fn exact_proposal_revision_controls_dispatch_and_watch_cannot_approve() {
        let mut m = model(Mode::Interact);
        m.proposals = Some(Page {
            entries: vec![proposal::Entry {
                proposal: proposal::Proposal {
                    thread: "t".into(),
                    id: "p".into(),
                    revision: 7,
                    command: "echo fixture".into(),
                    binding: proposal::Binding {
                        terminal: "b".repeat(64),
                        generation: "a".repeat(64),
                        cwd: "/scratch".into(),
                        shell_directory: None,
                        context_digest: "sha256:00".into(),
                    },
                },
                effect: proposal::Effect::Ordinary,
                state: proposal::State::Pending,
            }],
            more: false,
        });
        assert_eq!(m.decide("t", "p", 6, true), Err(Error::Stale));
        assert!(m.dispatch().is_none());
        m.decide("t", "p", 7, true).unwrap();
        let TermRequest::Proposal(request) = m.dispatch().unwrap() else {
            panic!()
        };
        assert!(matches!(
            request.action,
            Action::Decide {
                revision: 7,
                approve: true,
                ..
            }
        ));
        m.io.lock().unwrap().state = State::Ready;
        m.io.lock().unwrap().typist = false;
        assert_eq!(m.decide("t", "p", 7, true), Err(Error::Disconnected));
    }
    #[test]
    fn gaps_disable_input_and_renderer_state_contains_no_world_presence() {
        let mut m = model(Mode::Interact);
        let terminal = m.io.lock().unwrap().terminal.clone();
        m.incoming(Incoming::Frame(wire::Frame::new(
            terminal,
            "c".repeat(64),
            Body::Gap {
                from: 1,
                to: 5,
                bytes: None,
            },
        )))
        .unwrap();
        assert_eq!(m.state(), State::Behind);
        m.input("x");
        assert!(m.dispatch().is_none());
        assert!(m.notice.unwrap().contains("snapshot"));
    }
}

#[cfg(test)]
mod resize_tests {
    use super::*;
    #[test]
    fn remote_grid_size_changes_only_after_an_accepted_owner_resize() {
        let mut model = Workbench::new(
            TerminalRef {
                generation: "a".repeat(64),
                terminal: "b".repeat(64),
            },
            "c".repeat(64),
            Mode::Interact,
            wire::Size::new(24, 80),
            Features::NONE,
        );
        model.resize(40, 100);
        assert_eq!(model.core.focused_pane().unwrap().session.vt.rows(), 24);
        let request = model.dispatch().unwrap();
        model.result(&TerminalResult::from_outcome(
            request.request(),
            Ok((wire::Status::Accepted, Value::Done)),
        ));
        assert_eq!(model.core.focused_pane().unwrap().session.vt.rows(), 40);
        model.resize(50, 120);
        let request = model.dispatch().unwrap();
        model.result(&TerminalResult::from_outcome(
            request.request(),
            Err(wire::Refusal::new(wire::Reason::NotTypist, "fixture")),
        ));
        assert_eq!(model.core.focused_pane().unwrap().session.vt.rows(), 40);
        assert!(!model.can_type());
    }
}
