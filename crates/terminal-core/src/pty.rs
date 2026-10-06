//! Injected local or remote sessions. The application owns grids and output budgets.
use crate::{
    blocks::Blocks,
    bridge::{Connection, Request},
    proposals::Binding,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc::Receiver};
use web_time::{Duration, Instant};

pub const SCROLLBACK: usize = 5000;
#[derive(Clone, Debug)]
pub enum Program {
    Shell,
    Command {
        program: PathBuf,
        args: Vec<String>,
        label: String,
    },
}
impl Program {
    pub fn label(&self, shell: &Path) -> String {
        match self {
            Self::Shell => shell.file_name().map_or_else(
                || "shell".into(),
                |name| name.to_string_lossy().into_owned(),
            ),
            Self::Command { label, .. } => label.clone(),
        }
    }
}

pub enum Event {
    Size(u16, u16),
    Blocks(coder_pty::ext::BlockPage),
    /// Replaces the projection with the host-authoritative emulator.
    Snapshot(Box<coder_vt::Terminal>),
    History {
        epoch: u64,
        page: coder_pty::ext::HistoryRecord,
    },
    /// Changes attachment status without claiming the process ended.
    Status(String),
    Output(Vec<u8>),
    Gap,
    End(String),
}
/// A terminal attachment; implementations retain their protocol and platform objects.
pub trait Attachment: Send {
    /// The owner confirms grid dimensions through snapshots or size events.
    fn host_grid(&self) -> bool {
        false
    }

    fn input_available(&self) -> bool {
        true
    }
    fn reference(&self) -> Option<coder_pty::wire::TerminalRef> {
        None
    }
    /// The terminal owner answers emulator queries once for all attachments.
    fn host_answers(&self) -> bool {
        false
    }

    /// Retains a proposal on the terminal owner when this mount supports it.
    fn offer_proposal(&self, proposal: &crate::proposals::Proposal) -> Option<Result<(), String>> {
        let _ = proposal;
        None
    }
    /// Decides an owner-held proposal. A result never asks the mount to replay input.
    fn decide_proposal(
        &self,
        proposal: &crate::proposals::Proposal,
        approve: bool,
    ) -> Option<Result<coder_pty::proposal::State, String>> {
        let _ = (proposal, approve);
        None
    }

    fn sharing(
        &self,
        action: crate::sharing::Action,
    ) -> Option<Receiver<Result<coder_pty::wire::Value, String>>> {
        let _ = action;
        None
    }

    fn input(&self, bytes: &[u8]);
    fn resize(&self, rows: u16, cols: u16);
    fn close(&self);
    fn poll(&mut self) -> Option<Event>;
    fn target(&self) -> Option<Binding>;
    fn directory(&self) -> Option<String>;
}

/// Mount services are injected; this crate has no window, network, or native clipboard.
pub trait Transport: Send + Sync {
    fn shell(&self) -> &Path;
    fn open(&self, program: &Program, rows: u16, cols: u16) -> Result<Box<dyn Attachment>, String>;
    fn shutdown(&self);
    fn thread_program(&self) -> Option<Program>;
    fn resolve(&self, name: &str) -> Option<PathBuf>;
    fn request(&self, request: &Request) -> Result<Connection, String>;
    /// Reads thread `thread` through the shared chat client, only reading.
    /// A mount without a client answers that it cannot.
    fn read_thread(&self, thread: &str) -> Receiver<crate::thread::Read> {
        let _ = thread;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err(crate::thread::Unread::Unavailable(
            "this mount has no chat client".into(),
        )));
        receiver
    }
    /// Reads Coder run `task` from the task owner on this computer, only
    /// reading. A mount without a task owner answers that it cannot.
    fn read_run(&self, task: &str) -> Receiver<crate::run::Read> {
        let _ = task;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err(crate::run::Unread::Unavailable(
            "this mount has no task owner".into(),
        )));
        receiver
    }
    /// Reads retained file `path` of Coder run `task` through the task
    /// owner, which reads only by manifest path; `digest` is what the
    /// manifest names. A mount without a task owner answers that it cannot.
    fn read_artifact(&self, task: &str, path: &str, digest: &str) -> Receiver<crate::files::Read> {
        let _ = (task, path, digest);
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err(crate::files::Unread::Unavailable(
            "this mount has no task owner".into(),
        )));
        receiver
    }
    /// Reads this computer's background rules from the host's own store.
    /// A mount without one answers that it cannot.
    fn read_rules(&self) -> Receiver<crate::rules::Read> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no background rules".into()));
        receiver
    }
    /// Lists the plugin test results under directory `root`, reading each
    /// report without checking it. A mount without the helper answers that
    /// it cannot.
    fn read_studies(&self, root: &str) -> Receiver<crate::gym::ListRead> {
        let _ = root;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no plugin test results".into()));
        receiver
    }
    /// Reads the installed plugins by exact release, with the test results
    /// under `root` for each. A mount without the helper answers that it
    /// cannot.
    fn read_components(&self, root: &str) -> Receiver<crate::gym::ComponentsRead> {
        let _ = root;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no installed plugins".into()));
        receiver
    }
    /// Uses installed plugin `id` once, only while exactly `version` with
    /// package `digest` is installed and on, through the shared route on
    /// `workspace`. The same request again follows the first run.
    fn plugin_use(
        &self,
        id: &str,
        version: &str,
        digest: &str,
        request: &str,
        workspace: &str,
    ) -> Receiver<crate::gym::UseRead> {
        let _ = (id, version, digest, request, workspace);
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount runs no plugins".into()));
        receiver
    }
    /// Searches local and trusted knowledge entries for `query`, lexically,
    /// with no model.
    fn search_knowledge(&self, query: &str) -> Receiver<crate::knowledge::HitsRead> {
        let _ = query;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no knowledge".into()));
        receiver
    }
    /// Reads knowledge entry `id` at its current version.
    fn read_entry(&self, id: &str) -> Receiver<crate::knowledge::ShownRead> {
        let _ = id;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no knowledge".into()));
        receiver
    }
    /// Reads the studio's goals and their plans.
    fn read_goals(&self) -> Receiver<crate::knowledge::GoalsRead> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no studio".into()));
        receiver
    }
    /// Recomputes the retained plugin test result in `dir` from its
    /// attempts, only reading.
    fn read_study(&self, dir: &str) -> Receiver<crate::gym::Read> {
        let _ = dir;
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reads no plugin test results".into()));
        receiver
    }
    /// Pauses or resumes background rule `id` (`verb` is `pause` or
    /// `resume`) through the host's existing command.
    fn rule_command(&self, verb: &str, id: &str) -> Receiver<Result<(), String>> {
        let _ = (verb, id);
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount changes no background rules".into()));
        receiver
    }
    /// Sends task command `bytes` to the task owner's `verb` (`cancel` or
    /// `correct`) and answers its receipt or refusal.
    fn task_command(&self, verb: &str, bytes: &[u8]) -> Receiver<crate::run::Sent> {
        let _ = (verb, bytes);
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(crate::run::Sent::Refused(
            "this mount has no task owner".into(),
        ));
        receiver
    }
    /// Hands `text` to workshop agent `agent` through the host
    /// (`studio.agent.ask`), asked from `directory`, and answers what the
    /// host said. A mount without a host answers that it cannot.
    fn ask_agent(
        &self,
        agent: &str,
        text: &str,
        directory: Option<&str>,
    ) -> Receiver<Result<String, String>> {
        let _ = (agent, text, directory);
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(Err("this mount reaches no workshop agent".into()));
        receiver
    }
    fn git_summary(&self, pane: u64, directory: String) -> Receiver<(u64, String, String)>;
    fn open_link(&self, target: &str) -> Result<(), String>;
    fn clipboard(&self) -> Option<String>;
    fn copy(&self, text: &str) -> Result<(), String>;
}

pub struct Sessions(pub Arc<dyn Transport>);
impl Sessions {
    pub fn shell(&self) -> &Path {
        self.0.shell()
    }
    pub fn open(&self, program: &Program, rows: u16, cols: u16) -> Result<Session, String> {
        Ok(Session {
            vt: coder_vt::Terminal::new(rows.into(), cols.into(), SCROLLBACK),
            blocks: Blocks::default(),
            exited: None,
            status: None,
            journal: None,
            cwd: None,
            started: Instant::now(),
            checked: None,
            attachment: self.0.open(program, rows, cols)?,
        })
    }
    pub fn input(&self, session: &Session, bytes: &[u8]) {
        if session.exited.is_none() {
            session.attachment.input(bytes);
        }
    }
    pub fn resize(&self, session: &mut Session, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        if session.vt.rows() == rows as usize && session.vt.cols() == cols as usize {
            return;
        }
        if session.attachment.host_grid() {
            session.attachment.resize(rows, cols);
            return;
        }
        session.vt.resize(rows.into(), cols.into());
        session.attachment.resize(rows, cols);
    }
    pub fn close(&self, session: &Session) {
        session.attachment.close();
    }
    pub fn shutdown(&self) {
        self.0.shutdown();
    }
    pub fn pump(&self, session: &mut Session, maximum: usize, deadline: Instant) -> (bool, u64) {
        let mut changed = false;
        let mut bytes = 0u64;
        loop {
            if bytes as usize >= maximum || (changed && Instant::now() >= deadline) {
                break;
            }
            let Some(event) = session.attachment.poll() else {
                break;
            };
            changed = true;
            match event {
                Event::Size(rows, cols) => session.vt.resize(rows.into(), cols.into()),
                Event::Blocks(page) => {
                    session.blocks.restore_journal(&page, &session.vt);
                    session.journal = Some(page);
                }
                Event::Snapshot(vt) => session.vt = *vt,
                Event::History { epoch, page } => {
                    let _ = session.vt.attach_history(epoch, &page);
                }
                Event::Status(status) => session.status = Some(status),
                Event::Output(data) => {
                    bytes += data.len() as u64;
                    session.vt.feed(&data);
                }
                Event::Gap => session.vt.mark("[output skipped]"),
                Event::End(reason) => session.exited = Some(reason),
            }
            session.blocks.update(
                &mut session.vt,
                session.started.elapsed().as_millis() as u64,
            );
        }
        let replies = session.vt.take_replies();
        if !replies.is_empty() && !session.attachment.host_answers() {
            self.input(session, &replies);
        }
        let now = Instant::now();
        if session
            .checked
            .is_none_or(|at| now.duration_since(at) > Duration::from_secs(1))
        {
            session.checked = Some(now);
            session.cwd = session.attachment.directory();
        }
        (changed, bytes)
    }
}

pub struct Session {
    pub vt: coder_vt::Terminal,
    pub blocks: Blocks,
    pub exited: Option<String>,
    pub status: Option<String>,
    pub journal: Option<coder_pty::ext::BlockPage>,
    pub cwd: Option<String>,
    started: Instant,
    checked: Option<Instant>,
    attachment: Box<dyn Attachment>,
}
impl Session {
    pub fn sharing(
        &self,
        action: crate::sharing::Action,
    ) -> Option<Receiver<Result<coder_pty::wire::Value, String>>> {
        self.attachment.sharing(action)
    }
    pub fn reference(&self) -> Option<coder_pty::wire::TerminalRef> {
        self.attachment.reference()
    }
    pub fn input_available(&self) -> bool {
        self.exited.is_none() && self.attachment.input_available()
    }
    pub fn offer_proposal(
        &self,
        proposal: &crate::proposals::Proposal,
    ) -> Option<Result<(), String>> {
        self.attachment.offer_proposal(proposal)
    }
    pub fn decide_proposal(
        &self,
        proposal: &crate::proposals::Proposal,
        approve: bool,
    ) -> Option<Result<coder_pty::proposal::State, String>> {
        self.attachment.decide_proposal(proposal, approve)
    }
    pub fn binding(&self, context_digest: String) -> Option<Binding> {
        let mut binding = self.attachment.target()?;
        binding.context_digest = context_digest;
        binding.shell_directory = self.blocks.cwd.clone();
        Some(binding)
    }
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("vt", &self.vt)
            .field("exited", &self.exited)
            .finish_non_exhaustive()
    }
}
