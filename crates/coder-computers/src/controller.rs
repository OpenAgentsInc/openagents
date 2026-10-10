//! The Computers application host: current view identity, screen state,
//! intent handling, and input requests.
use crate::authority::{Action, Denial, check};
use crate::intent::{Intent, Screen};
use crate::model::{
    Capabilities, CreatedInvitation, HostRecord, ListingChange, Snapshot, SshStage,
};
use crate::service::ComputersService;
use coder_access::protocol::{INVITATION_PREFIX, MAX_GRANT_LIFETIME, normalize_code};
use coder_access::{Code, Error, Right, Rights};
use rust_native::{Activation, ValidatedView, View, ViewError};
use serde::Serialize;
use std::collections::BTreeMap;

/// The largest value an input request accepts, in bytes.
pub const MAX_INPUT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Confirm {
    Forget(String),
    Revoke(String, String),
    /// Remove a host from the owner directory at a revision.
    Delist(String, u64),
    /// Remove a host this device set up over SSH.
    RemoveSsh(String),
    /// Stop a task: host, task, and the revision the screen showed.
    CancelTask(String, String, u64),
}

/// An order being written on one host's order screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OrderDraft {
    pub workspace: Option<String>,
    pub prompt: Option<String>,
}

/// The title a task gets from its prompt: the first line with text, with
/// control characters replaced, at most 80 characters.
pub(crate) fn task_title(prompt: &str) -> String {
    coder_access::client::tasks::title(prompt)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Done,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
}

/// What the platform adapter should collect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPurpose {
    Invitation,
    ApprovalCode,
    SshDestination,
    /// An answer `ssh` asks for, such as a password or key passphrase.
    SshPassword,
    /// The owner's secret key, to read the owner directory.
    OwnerKey,
    /// The label the owner directory gives a computer.
    DirectoryLabel,
    /// The placement weight the owner directory gives a computer, 0 to
    /// 1,000.
    DirectoryWeight,
    /// The prompt of a task to order.
    TaskPrompt,
    /// A workspace label, for a host that lists none.
    TaskWorkspace,
    /// Replacement instructions for a task.
    SteerPrompt,
    /// The path of a file on the computer to copy here.
    FilePath,
}

/// The largest file the Files control copies from a computer, in bytes.
/// A screenshot may be larger: it is bounded by
/// `coder_access::computer::MAX_SCREENSHOT_BYTES` on the host's side and
/// by [`MAX_CAPTURE_BYTES`] here.
pub const MAX_PULL_BYTES: u64 = 8 * 1024 * 1024;

/// The most bytes a capture keeps for showing on this device.
pub const MAX_CAPTURE_BYTES: usize = 32 * 1024 * 1024;

/// The most of a text file the screen shows.
pub const MAX_PREVIEW_BYTES: usize = 8 * 1024;

/// What a capture holds, decided from its bytes, never its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureKind {
    /// A PNG or JPEG the platform draws.
    Image,
    /// UTF-8 text the screen shows.
    Text,
    /// Anything else: only its size is shown.
    Other,
}

impl CaptureKind {
    /// Sniff the kind from the first bytes and the encoding.
    pub fn of(bytes: &[u8]) -> Self {
        const PNG: &[u8] = &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        const JPEG: &[u8] = &[0xff, 0xd8, 0xff];
        if bytes.starts_with(PNG) || bytes.starts_with(JPEG) {
            Self::Image
        } else if std::str::from_utf8(bytes).is_ok_and(|text| {
            !text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        }) {
            Self::Text
        } else {
            Self::Other
        }
    }
}

/// What the Screenshot or Files control last brought back from a
/// computer. It lives only in this surface's memory; nothing writes it to
/// disk.
#[derive(Clone, PartialEq, Eq)]
pub struct Capture {
    pub host: String,
    /// The path the computer read, or `None` for a screenshot.
    pub path: Option<String>,
    pub kind: CaptureKind,
    pub bytes: Vec<u8>,
    /// Counts captures in this surface, so each one has its own surface
    /// resource and a platform redraws a new one.
    pub serial: u64,
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field("host", &self.host)
            .field("path", &self.path)
            .field("kind", &self.kind)
            .field("bytes", &self.bytes.len())
            .field("serial", &self.serial)
            .finish()
    }
}

impl Capture {
    /// The `image:` surface resource a platform draws this capture's image
    /// under; `None` when it is not an image.
    pub fn resource(&self) -> Option<String> {
        (self.kind == CaptureKind::Image).then(|| format!("image:computer-capture-{}", self.serial))
    }
}

/// A request for one value the Rust Native tree cannot collect yet: Rust
/// Native's input request with this application's purposes. The adapter
/// shows its native field or scanner and answers with the token. A `secret`
/// request gets a masked field whose value the adapter never echoes, logs,
/// or keeps. Rust validates the value; the adapter only carries it.
pub type InputRequest = rust_native::InputRequest<InputPurpose>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Invitation,
    Code {
        host: String,
        enrollment: String,
    },
    Ssh,
    SshPrompt {
        id: u64,
    },
    OwnerKey,
    DirectoryLabel {
        host: String,
    },
    EditLabel {
        host: String,
        revision: u64,
    },
    EditWeight {
        host: String,
        revision: u64,
    },
    Prompt {
        host: String,
    },
    Workspace {
        host: String,
    },
    Steer {
        host: String,
        task: String,
        revision: u64,
    },
    /// A file to copy from a computer.
    Pull {
        host: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingInput {
    pub request: InputRequest,
    target: Target,
}

/// Screen state that is not part of the domain snapshot.
#[derive(Debug)]
pub(crate) struct UiState {
    pub screen: Screen,
    pub confirm: Option<Confirm>,
    /// Rights chosen for the next invitation, per host.
    pub drafts: BTreeMap<String, Vec<Right>>,
    /// Invitations created here and not yet dismissed, per host.
    pub invitations: BTreeMap<String, CreatedInvitation>,
    pub input: Option<PendingInput>,
    pub notice: Option<Notice>,
    /// Orders being written, per host.
    pub orders: BTreeMap<String, OrderDraft>,
    /// A terminal the person asked to open, until the client takes it.
    pub terminal: Option<String>,
    /// What a Screenshot or Files control last brought back.
    pub capture: Option<Capture>,
    /// How many captures this surface has made.
    pub captures: u64,
}

impl UiState {
    /// The invitation draft: the person's choice, or by default the standard
    /// rights this device holds.
    pub fn draft(&self, host: &HostRecord, now: u64) -> Vec<Right> {
        if let Some(draft) = self.drafts.get(&host.key) {
            return draft.clone();
        }
        let held = host.enrollment.rights(now);
        Rights::standard()
            .iter()
            .filter(|right| held.is_some_and(|held| held.contains(*right)))
            .collect()
    }
}

/// What an accepted activation or input did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The view changed.
    Updated,
    /// The adapter should collect the value in [`Computers::input`].
    InputRequested,
    /// First run finished; the client continues to its existing onboarding.
    ContinueOnboarding,
    /// The person asked to open a terminal. The client takes the host from
    /// [`Computers::take_terminal`] and shows its terminal screen.
    Terminal,
}

/// Why an activation or input did nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The activation or input named a view or request that is not current.
    Stale,
    /// The control is disabled.
    Disabled,
    /// The node is not a control.
    NotInteractive,
    /// The current snapshot does not allow the intent.
    Denied(Denial),
    /// The value the person entered is not acceptable.
    Input(String),
    /// The service or host refused or failed.
    Failed(Error),
}

impl Refusal {
    pub fn reason(&self) -> String {
        match self {
            Self::Stale => "The screen changed. Check it and try again.".into(),
            Self::Disabled | Self::NotInteractive => "That control isn't available.".into(),
            Self::Denied(denial) => denial.reason(),
            Self::Input(reason) => reason.clone(),
            Self::Failed(error) => describe(error),
        }
    }
}

/// User-facing copy for a service or host refusal. Diagnostic messages stay
/// out of the screen; the stable code decides the text.
pub fn describe(error: &Error) -> String {
    match error.code {
        Code::MissingRight => match error.missing {
            Some(right) => format!(
                "The computer refused: {}",
                Denial::MissingRight(right).reason()
            ),
            None => "The computer refused: this device lacks a needed right.".into(),
        },
        Code::Malformed | Code::Unsupported | Code::Bounds => {
            "The computer couldn't accept this. Check what you entered and try again.".into()
        }
        Code::Forbidden => "The computer refused this request.".into(),
        Code::Expired => "It expired. Create a new invitation or code on the computer.".into(),
        Code::Revoked => "This computer revoked this device's access.".into(),
        Code::Stale => "The computer's records changed. Refresh and try again.".into(),
        Code::Conflict => "Another device already used it.".into(),
        Code::Unavailable => "The computer can't do this right now.".into(),
        Code::Transport => {
            "Couldn't reach the computer. Check that it's online, then try again.".into()
        }
        Code::RateLimited => "Too many attempts. Wait, then start again on the computer.".into(),
        Code::WrongCode => {
            "That code doesn't match the one on the computer. Check it and try again.".into()
        }
        Code::Denied => "This request was denied.".into(),
    }
}

/// The Computers screens for one surface lifetime.
pub struct Computers {
    service: Box<dyn ComputersService + Send>,
    caps: Capabilities,
    snapshot: Snapshot,
    pub(crate) ui: UiState,
    instance: String,
    revision: u64,
    inputs: u64,
    current: Option<ValidatedView<Intent>>,
}

impl Computers {
    /// Open the screens for a new surface lifetime. `instance` must be fresh
    /// for each mount. First run shows until the service records it done.
    pub fn new(
        mut service: Box<dyn ComputersService + Send>,
        caps: Capabilities,
        instance: impl Into<String>,
    ) -> Result<Self, Error> {
        let snapshot = service.snapshot()?;
        let screen = if snapshot.first_run_complete {
            Screen::Computers
        } else {
            Screen::FirstRun
        };
        let mut computers = Self {
            service,
            caps,
            snapshot,
            ui: UiState {
                screen,
                confirm: None,
                drafts: BTreeMap::new(),
                invitations: BTreeMap::new(),
                input: None,
                notice: None,
                orders: BTreeMap::new(),
                terminal: None,
                capture: None,
                captures: 0,
            },
            instance: instance.into(),
            revision: 0,
            inputs: 0,
            current: None,
        };
        computers.rebuild()?;
        Ok(computers)
    }

    /// The current validated view.
    pub fn view(&self) -> Option<&ValidatedView<Intent>> {
        self.current.as_ref()
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn screen(&self) -> &Screen {
        &self.ui.screen
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.ui.notice.as_ref()
    }

    /// The host whose terminal the person asked to open, once. A client
    /// with a terminal screen shows it after an [`Outcome::Terminal`].
    pub fn take_terminal(&mut self) -> Option<String> {
        self.ui.terminal.take()
    }

    /// What the Screenshot or Files control last brought back, if the
    /// screen still shows it.
    pub fn capture(&self) -> Option<&Capture> {
        self.ui.capture.as_ref()
    }

    /// The encoded image bytes of the capture's `image:` surface
    /// `resource`, for a platform that draws the image itself.
    pub fn capture_image(&self, resource: &str) -> Option<&[u8]> {
        let capture = self.ui.capture.as_ref()?;
        (capture.resource().as_deref() == Some(resource)).then_some(capture.bytes.as_slice())
    }

    /// The value the adapter should collect, if any.
    pub fn input(&self) -> Option<&InputRequest> {
        self.ui.input.as_ref().map(|input| &input.request)
    }

    /// The QR modules of the invitation the current Access screen shows, for
    /// a platform host that draws the code itself. Rendered locally; `true`
    /// is a dark module.
    pub fn invitation_qr(&self) -> Option<Vec<Vec<bool>>> {
        let Screen::Access { host } = &self.ui.screen else {
            return None;
        };
        crate::qr::modules(&self.ui.invitations.get(host)?.code)
    }

    /// Pass an application lifecycle change to the service's supervisors,
    /// then reload. `active` is `false` when the application moves to the
    /// background.
    pub fn set_active(&mut self, active: bool) -> Result<(), Error> {
        self.service.application(active)?;
        self.refresh()
    }

    /// Reload the snapshot and draw a new revision.
    pub fn refresh(&mut self) -> Result<(), Error> {
        self.reload();
        self.rebuild()
    }

    /// Resolve a native callback against the current view, then check and
    /// run its intent. A refusal is also shown as the screen's notice.
    pub fn activate(&mut self, event: &Activation) -> Result<Outcome, Refusal> {
        let resolved = match self.current.as_ref() {
            None => Err(Refusal::Stale),
            Some(view) => view.activate(event).cloned().map_err(|error| match error {
                ViewError::StaleActivation => Refusal::Stale,
                ViewError::Disabled => Refusal::Disabled,
                _ => Refusal::NotInteractive,
            }),
        };
        let result = resolved.and_then(|intent| self.apply(intent));
        self.finish(result)
    }

    /// Check and run an intent that a platform's own native control chose,
    /// such as a row in a native list, rather than a button in the current
    /// view. It runs through the same authority check as [`Self::activate`];
    /// a refusal is also shown as the screen's notice.
    pub fn perform(&mut self, intent: Intent) -> Result<Outcome, Refusal> {
        let result = self.apply(intent);
        self.finish(result)
    }

    /// Whether this device may order work on `host` now: the same check the
    /// Order work screen makes.
    pub fn can_operate(&self, host: &str) -> bool {
        self.allow(Action::Operate { host }).is_ok()
    }

    /// Ask `host` again for the workspace labels it accepts.
    pub fn refresh_workspaces(&mut self, host: &str) -> Result<(), Refusal> {
        self.allow(Action::Operate { host })?;
        self.service
            .refresh_workspaces(host)
            .map_err(Refusal::Failed)?;
        self.reload();
        self.rebuild().map_err(Refusal::Failed)
    }

    /// Ask `host` to stop task `task` at `revision`, as the Stop task
    /// control does, without its confirmation step: a chat client's stop
    /// control is the confirmation.
    pub fn stop_task(&mut self, host: &str, task: &str, revision: u64) -> Result<(), Refusal> {
        self.allow(Action::Operate { host })?;
        let reason = match self.caps.platform {
            crate::model::Platform::Phone => "Stopped from a phone.",
            crate::model::Platform::Desktop => "Stopped from the desktop app.",
            crate::model::Platform::Terminal => "Stopped from the terminal app.",
        };
        self.service
            .cancel_task(host, task, revision, reason)
            .map_err(Refusal::Failed)?;
        self.reload();
        self.rebuild().map_err(Refusal::Failed)
    }

    /// Send a durable task command to `host` (`task.command`). A chat
    /// client mints the command once and calls this again with the same
    /// command after a transport failure; the host never runs it twice.
    pub fn command_task(
        &mut self,
        host: &str,
        command: &coder_access::protocol::TaskCommand,
    ) -> Result<(), Refusal> {
        self.allow(Action::Operate { host })?;
        self.service
            .command_task(host, command)
            .map_err(Refusal::Failed)?;
        self.reload();
        self.rebuild().map_err(Refusal::Failed)
    }

    /// List or edit a task's held messages on `host` (`task.queue`).
    pub fn queue_task(
        &mut self,
        host: &str,
        task: &str,
        edit: &coder_access::protocol::QueueEdit,
    ) -> Result<coder_access::protocol::TaskQueue, Refusal> {
        self.allow(Action::Operate { host })?;
        let queue = self
            .service
            .queue_task(host, task, edit)
            .map_err(Refusal::Failed)?;
        if !matches!(edit, coder_access::protocol::QueueEdit::List {}) {
            self.reload();
            self.rebuild().map_err(Refusal::Failed)?;
        }
        Ok(queue)
    }

    /// Read what `task` on `host` changed (`task.review`), which needs
    /// `observe`.
    pub fn review_task(
        &mut self,
        host: &str,
        task: &str,
    ) -> Result<coder_access::review::TaskReview, Refusal> {
        self.allow(Action::Review { host })?;
        self.service
            .review_task(host, task)
            .map_err(Refusal::Failed)
    }

    /// Publish the reviewed change of `task` on `host` once
    /// (`task.publish`), which needs `operate`: a mutation, under the same
    /// grant check as ordering work.
    pub fn publish_task(
        &mut self,
        host: &str,
        task: &str,
        base: &str,
        head_commit: &str,
        head: &str,
    ) -> Result<coder_access::review::Publication, Refusal> {
        self.allow(Action::Operate { host })?;
        self.service
            .publish_task(host, task, base, head_commit, head)
            .map_err(Refusal::Failed)
    }

    /// Leave a nudge for `host`, which this device could not reach with a
    /// waiting command. It carries nothing and grants nothing.
    pub fn nudge_host(&mut self, host: &str) -> Result<(), Refusal> {
        self.allow(Action::Operate { host })
            .or_else(|denial| match denial {
                // A host that is offline is the one to nudge.
                Refusal::Denied(crate::Denial::Offline) => Ok(()),
                other => Err(other),
            })?;
        self.service.nudge_host(host).map_err(Refusal::Failed)
    }

    /// Take a finished or cancelled task on `host` off every device's
    /// lists (`task.archive`). The host keeps its record and transcript.
    pub fn archive_task(&mut self, host: &str, task: &str) -> Result<(), Refusal> {
        self.allow(Action::Operate { host })?;
        self.service
            .archive_task(host, task)
            .map_err(Refusal::Failed)?;
        self.reload();
        self.rebuild().map_err(Refusal::Failed)
    }

    /// Order work on `host` without the Order work screen, as a chat client
    /// does: NIP-HOST `task.create` with the prompt's first line as title.
    /// It records the task; the host's own policy decides whether it runs.
    pub fn start_task(
        &mut self,
        host: &str,
        workspace: &str,
        prompt: &str,
    ) -> Result<String, Refusal> {
        self.allow(Action::Operate { host })?;
        let task = coder_access::client::tasks::input(prompt, workspace);
        let id = self
            .service
            .create_task(host, &task)
            .map_err(Refusal::Failed)?;
        self.reload();
        self.rebuild().map_err(Refusal::Failed)?;
        Ok(id)
    }

    /// [`Computers::start_task`] with images: each image's exact bytes go
    /// to `host` first (`artifact.put`, resuming where the host holds), and
    /// the task names them by digest. Nothing is created when any image
    /// fails to arrive; the caller keeps its draft.
    ///
    /// # Errors
    /// As [`Computers::start_task`], or an image the host refused or did
    /// not hold whole.
    pub fn start_task_with_images(
        &mut self,
        host: &str,
        workspace: &str,
        prompt: &str,
        images: &[coder_access::media::Upload],
    ) -> Result<String, Refusal> {
        if images.is_empty() {
            return self.start_task(host, workspace, prompt);
        }
        self.start_task_requesting(host, workspace, prompt, images, None)
    }

    /// [`Computers::start_task_with_images`] for a person who asked for
    /// `engine` (#10081), the chat offer's typed engine. The request names
    /// it only when `host`'s presence advertises
    /// [`coder_access::protocol::TASK_ENGINE`]: an older host rejects the
    /// field, so it gets the request it always got and runs its default.
    /// The host puts the engine first only among the routes its owner's
    /// policy admits; it is a request, never permission.
    ///
    /// # Errors
    /// As [`Computers::start_task_with_images`].
    pub fn start_task_requesting(
        &mut self,
        host: &str,
        workspace: &str,
        prompt: &str,
        images: &[coder_access::media::Upload],
        engine: Option<nostr::cj_conversation::Engine>,
    ) -> Result<String, Refusal> {
        self.allow(Action::Operate { host })?;
        let accepts_engine = self
            .snapshot()
            .host(host)
            .and_then(|record| record.presence.as_ref())
            .is_some_and(|received| {
                received
                    .presence
                    .supports(coder_access::protocol::TASK_ENGINE)
            });
        let references = if images.is_empty() {
            Vec::new()
        } else {
            let service = &mut self.service;
            coder_access::media::send(images, |put| service.put_artifact(host, put))
                .map_err(Refusal::Failed)?
        };
        let mut task = coder_access::client::tasks::input(prompt, workspace);
        task.images = references;
        task.engine = engine.filter(|_| accepts_engine);
        let id = self
            .service
            .create_task(host, &task)
            .map_err(Refusal::Failed)?;
        self.reload();
        self.rebuild().map_err(Refusal::Failed)?;
        Ok(id)
    }

    /// Leave first run for the Computers screen, for a client that adds hosts
    /// on its own, such as through NIP-HOST tailnet admission.
    pub fn finish_first_run(&mut self) -> Result<(), Refusal> {
        if self.ui.screen != Screen::FirstRun {
            return Ok(());
        }
        self.service.complete_first_run().map_err(Refusal::Failed)?;
        self.reload();
        self.ui.screen = Screen::Computers;
        self.rebuild().map_err(Refusal::Failed)
    }

    /// Add a host from an invitation a trusted local path delivered, such as
    /// NIP-HOST tailnet admission, without an input request. Adding a host
    /// finishes first run. The host still signs the grant on redemption.
    pub fn admit(&mut self, invitation: &str, label: &str) -> Result<String, Refusal> {
        let result = (|| {
            if !invitation.starts_with(INVITATION_PREFIX) {
                return Err(Refusal::Input(
                    "This isn't a computer invitation. Computer invitations start with coder-host:."
                        .into(),
                ));
            }
            let host = self
                .service
                .redeem_labeled(invitation, label)
                .map_err(Refusal::Failed)?;
            self.service.complete_first_run().map_err(Refusal::Failed)?;
            self.reload();
            if matches!(self.ui.screen, Screen::FirstRun | Screen::Add) {
                self.ui.screen = Screen::Computers;
            }
            let label = self.label(&host);
            self.ui.notice = Some(Notice {
                kind: NoticeKind::Done,
                text: format!("Added {label}."),
            });
            Ok(host)
        })();
        let outcome = result
            .as_ref()
            .map(|_| Outcome::Updated)
            .map_err(Clone::clone);
        self.finish(outcome)?;
        result
    }

    /// Accept the value for the current input request.
    pub fn submit(&mut self, token: &str, value: &str) -> Result<Outcome, Refusal> {
        let result = self.accept(token, value);
        self.finish(result)
    }

    /// Close the current input request if `token` names it.
    pub fn cancel_input(&mut self, token: &str) -> Result<Outcome, Refusal> {
        let result = match &self.ui.input {
            Some(input) if input.request.token == token => {
                self.close_input();
                Ok(Outcome::Updated)
            }
            _ => Err(Refusal::Stale),
        };
        self.finish(result)
    }

    /// Close the current input request. Closing an SSH prompt refuses it, so
    /// `ssh` fails authentication rather than wait.
    fn close_input(&mut self) {
        if let Some(PendingInput {
            target: Target::SshPrompt { id },
            ..
        }) = self.ui.input.take()
        {
            let _ = self.service.answer_ssh_prompt(id, None);
        }
    }

    fn finish(&mut self, result: Result<Outcome, Refusal>) -> Result<Outcome, Refusal> {
        if let Err(refusal) = &result {
            self.ui.notice = Some(Notice {
                kind: NoticeKind::Refused,
                text: refusal.reason(),
            });
        }
        if let Err(error) = self.rebuild() {
            return Err(Refusal::Failed(error));
        }
        result
    }

    fn reload(&mut self) {
        match self.service.snapshot() {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                let hosts: Vec<String> =
                    self.snapshot.hosts.iter().map(|h| h.key.clone()).collect();
                self.ui.drafts.retain(|host, _| hosts.contains(host));
                self.ui.invitations.retain(|host, _| hosts.contains(host));
                self.ui.orders.retain(|host, _| hosts.contains(host));
                if self
                    .ui
                    .capture
                    .as_ref()
                    .is_some_and(|capture| !hosts.contains(&capture.host))
                {
                    self.ui.capture = None;
                }
                if let Screen::Access { host } | Screen::Host { host } | Screen::Order { host } =
                    &self.ui.screen
                    && !hosts.contains(host)
                {
                    self.ui.screen = Screen::Computers;
                }
                self.sync_ssh_prompt();
            }
            Err(error) => {
                self.ui.notice = Some(Notice {
                    kind: NoticeKind::Refused,
                    text: describe(&error),
                });
            }
        }
    }

    /// Show an SSH prompt as an input request, and close a prompt request
    /// the setup no longer waits on.
    fn sync_ssh_prompt(&mut self) {
        let waiting = match self.snapshot.ssh.as_ref().map(|attempt| &attempt.stage) {
            Some(SshStage::Prompt { id, text }) => Some((*id, text.clone())),
            _ => None,
        };
        let shown = match &self.ui.input {
            Some(PendingInput {
                target: Target::SshPrompt { id },
                ..
            }) => Some(*id),
            _ => None,
        };
        match (waiting, shown) {
            (Some((id, _)), Some(current)) if id == current => {}
            (Some((id, text)), _) => {
                let _ = self.ask_with(
                    InputPurpose::SshPassword,
                    Target::SshPrompt { id },
                    false,
                    Some(text),
                );
            }
            (None, Some(_)) => self.ui.input = None,
            (None, None) => {}
        }
    }

    fn rebuild(&mut self) -> Result<(), Error> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| Error::new(Code::Bounds, "view revision exhausted"))?;
        let root = crate::project::root(&self.snapshot, self.caps, &self.ui);
        let view = View::new(self.instance.clone(), self.revision, root)
            .validate()
            .map_err(|error| Error::new(Code::Bounds, error.to_string()))?;
        self.current = Some(view);
        Ok(())
    }

    fn allow(&self, action: Action<'_>) -> Result<(), Refusal> {
        check(&self.snapshot, self.caps, action).map_err(Refusal::Denied)
    }

    fn label(&self, host: &str) -> String {
        self.snapshot
            .host(host)
            .map_or_else(|| "The computer".into(), |record| record.label.clone())
    }

    fn done(&mut self, text: impl Into<String>) -> Result<Outcome, Refusal> {
        self.reload();
        self.ui.notice = Some(Notice {
            kind: NoticeKind::Done,
            text: text.into(),
        });
        Ok(Outcome::Updated)
    }

    fn ask(
        &mut self,
        purpose: InputPurpose,
        target: Target,
        scan: bool,
    ) -> Result<Outcome, Refusal> {
        self.ask_with(purpose, target, scan, None)
    }

    fn ask_with(
        &mut self,
        purpose: InputPurpose,
        target: Target,
        scan: bool,
        asked: Option<String>,
    ) -> Result<Outcome, Refusal> {
        self.inputs += 1;
        let (label, prompt) = match purpose {
            InputPurpose::Invitation => (
                "Computer invitation",
                "Scan or paste the complete coder-host: invitation.",
            ),
            InputPurpose::ApprovalCode => (
                "Code shown on the computer",
                "Enter the 8-character code the computer shows.",
            ),
            InputPurpose::SshDestination => (
                "SSH destination",
                "Enter a destination ssh accepts, such as a configured alias or user@host.",
            ),
            InputPurpose::SshPassword => ("SSH answer", "Enter what ssh asks for."),
            InputPurpose::OwnerKey => (
                "Owner key",
                "Enter the secret key your computers name as their owner, as hex or nsec. It stays on this device.",
            ),
            InputPurpose::DirectoryLabel => (
                "Computer name",
                "Enter the name your directory shows for this computer, up to 64 bytes.",
            ),
            InputPurpose::DirectoryWeight => (
                "Placement weight",
                "Enter a weight from 0 to 1000. Higher weights get more new work; 0 keeps the computer listed and gives it none.",
            ),
            InputPurpose::TaskPrompt => (
                "Task prompt",
                "Describe the work for the computer, up to 16 KiB.",
            ),
            InputPurpose::TaskWorkspace => (
                "Workspace",
                "Enter the workspace name the computer shares, such as openagents.",
            ),
            InputPurpose::SteerPrompt => (
                "New instructions",
                "Enter the instructions that replace this task's prompt.",
            ),
            InputPurpose::FilePath => (
                "File on the computer",
                "Enter the file's path on the computer, such as ~/notes.txt or /tmp/log.txt.",
            ),
        };
        let prompt = asked.unwrap_or_else(|| prompt.to_owned());
        let secret = matches!(purpose, InputPurpose::SshPassword | InputPurpose::OwnerKey);
        self.ui.input = Some(PendingInput {
            request: InputRequest {
                token: format!("{}-input-{}", self.instance, self.inputs),
                purpose,
                label: label.into(),
                prompt,
                scan,
                secret,
                max_bytes: MAX_INPUT_BYTES,
            },
            target,
        });
        debug_assert!(self.input().is_some_and(|input| input.validate().is_ok()));
        self.ui.notice = None;
        Ok(Outcome::InputRequested)
    }

    /// Keep what a Screenshot or Files control brought back, replacing the
    /// last capture.
    fn keep_capture(
        &mut self,
        host: String,
        path: Option<String>,
        bytes: Vec<u8>,
    ) -> Result<(), Refusal> {
        if bytes.len() > MAX_CAPTURE_BYTES {
            return Err(Refusal::Failed(Error::new(
                Code::Bounds,
                "the capture is over this device's limit",
            )));
        }
        self.ui.captures += 1;
        self.ui.capture = Some(Capture {
            host,
            path,
            kind: CaptureKind::of(&bytes),
            bytes,
            serial: self.ui.captures,
        });
        Ok(())
    }

    /// The latest grant expiry this device may give: at most the protocol
    /// maximum and, unless this device is the owner, its own grant's expiry.
    fn grant_expiry(&self, host: &str) -> u64 {
        let limit = self.snapshot.now.saturating_add(MAX_GRANT_LIFETIME);
        match self.snapshot.host(host).map(|record| &record.enrollment) {
            Some(crate::model::Enrollment::Enrolled { expires_at, .. }) if !self.snapshot.owner => {
                limit.min(*expires_at)
            }
            _ => limit,
        }
    }

    fn apply(&mut self, intent: Intent) -> Result<Outcome, Refusal> {
        match intent {
            Intent::Show { screen } => {
                if let Screen::Access { host } | Screen::Host { host } | Screen::Order { host } =
                    &screen
                    && self.snapshot.host(host).is_none()
                {
                    return Err(Refusal::Denied(Denial::UnknownHost));
                }
                if screen == Screen::FirstRun && self.snapshot.first_run_complete {
                    return Err(Refusal::NotInteractive);
                }
                self.ui.confirm = None;
                self.ui.input = None;
                self.ui.notice = None;
                // Opening an order reads the workspaces the host shares
                // when they are not known yet. A failure leaves the form
                // usable: the person can enter a workspace name.
                if let Screen::Order { host } = &screen
                    && self
                        .snapshot
                        .host(host)
                        .is_some_and(|record| record.workspaces.is_none())
                    && self.allow(Action::Operate { host }).is_ok()
                {
                    if let Err(error) = self.service.refresh_workspaces(host) {
                        self.ui.notice = Some(Notice {
                            kind: NoticeKind::Refused,
                            text: format!(
                                "Couldn't list this computer's workspaces: {} Enter a workspace name instead.",
                                describe(&error)
                            ),
                        });
                    }
                    self.reload();
                }
                self.ui.screen = screen;
                Ok(Outcome::Updated)
            }
            Intent::RefreshWorkspaces { host } => {
                self.allow(Action::Operate { host: &host })?;
                self.service
                    .refresh_workspaces(&host)
                    .map_err(Refusal::Failed)?;
                self.done("Workspaces refreshed.")
            }
            Intent::ChooseWorkspace { host, workspace } => {
                self.allow(Action::Operate { host: &host })?;
                let listed = self
                    .snapshot
                    .host(&host)
                    .and_then(|record| record.workspaces.as_ref())
                    .is_some_and(|list| list.contains(&workspace));
                if !listed {
                    return Err(Refusal::Stale);
                }
                self.ui.orders.entry(host).or_default().workspace = Some(workspace);
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::EnterWorkspace { host } => {
                self.allow(Action::Operate { host: &host })?;
                self.ask(
                    InputPurpose::TaskWorkspace,
                    Target::Workspace { host },
                    false,
                )
            }
            Intent::WritePrompt { host } => {
                self.allow(Action::Operate { host: &host })?;
                self.ask(InputPurpose::TaskPrompt, Target::Prompt { host }, false)
            }
            Intent::SubmitTask { host } => {
                self.allow(Action::Operate { host: &host })?;
                let draft = self.ui.orders.get(&host).cloned().unwrap_or_default();
                let (Some(workspace), Some(prompt)) = (draft.workspace, draft.prompt) else {
                    return Err(Refusal::Input(
                        "Choose a workspace and write a prompt first.".into(),
                    ));
                };
                let task = coder_access::protocol::TaskCreate {
                    title: task_title(&prompt),
                    prompt,
                    workspace: workspace.clone(),
                    images: Vec::new(),
                    engine: None,
                };
                self.service
                    .create_task(&host, &task)
                    .map_err(Refusal::Failed)?;
                if let Some(order) = self.ui.orders.get_mut(&host) {
                    order.prompt = None;
                }
                let label = self.label(&host);
                self.ui.screen = Screen::Activity;
                self.done(format!(
                    "Sent \"{}\" to {label} in {workspace}. Follow it here.",
                    task.title
                ))
            }
            Intent::SteerTask {
                host,
                task,
                revision,
            } => {
                self.allow(Action::Operate { host: &host })?;
                self.ask(
                    InputPurpose::SteerPrompt,
                    Target::Steer {
                        host,
                        task,
                        revision,
                    },
                    false,
                )
            }
            Intent::CancelTask {
                host,
                task,
                revision,
            } => {
                self.allow(Action::Operate { host: &host })?;
                self.ui.confirm = Some(Confirm::CancelTask(host, task, revision));
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::ConfirmCancelTask {
                host,
                task,
                revision,
            } => {
                if self.ui.confirm
                    != Some(Confirm::CancelTask(host.clone(), task.clone(), revision))
                {
                    return Err(Refusal::Stale);
                }
                self.allow(Action::Operate { host: &host })?;
                let reason = match self.caps.platform {
                    crate::model::Platform::Phone => "Cancelled from a phone.",
                    crate::model::Platform::Desktop => "Cancelled from the desktop app.",
                    crate::model::Platform::Terminal => "Cancelled from the terminal app.",
                };
                self.service
                    .cancel_task(&host, &task, revision, reason)
                    .map_err(Refusal::Failed)?;
                self.ui.confirm = None;
                let label = self.label(&host);
                self.done(format!("Asked {label} to stop the task."))
            }
            Intent::OpenTerminal { host } => {
                self.allow(Action::Terminal { host: &host })?;
                self.ui.terminal = Some(host);
                self.ui.notice = None;
                Ok(Outcome::Terminal)
            }
            Intent::Screenshot { host } => {
                self.allow(Action::Terminal { host: &host })?;
                let bytes = self.service.screenshot(&host).map_err(Refusal::Failed)?;
                let label = self.label(&host);
                self.keep_capture(host, None, bytes)?;
                self.done(format!("Took a screenshot of {label}."))
            }
            Intent::PullFile { host } => {
                self.allow(Action::Terminal { host: &host })?;
                self.ask(InputPurpose::FilePath, Target::Pull { host }, false)
            }
            Intent::ClearCapture { host } => {
                if self
                    .ui
                    .capture
                    .as_ref()
                    .is_none_or(|capture| capture.host != host)
                {
                    return Err(Refusal::Stale);
                }
                self.ui.capture = None;
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::Refresh => {
                self.ui.notice = None;
                self.reload();
                Ok(Outcome::Updated)
            }
            Intent::SetEnabled { host, enabled } => {
                self.allow(Action::SetEnabled { host: &host })?;
                self.service
                    .set_enabled(&host, enabled)
                    .map_err(Refusal::Failed)?;
                let label = self.label(&host);
                self.done(if enabled {
                    format!("Switched on {label}.")
                } else {
                    format!("Switched off {label}. It stays in your list.")
                })
            }
            Intent::RetryNow { host } => {
                self.allow(Action::RetryNow { host: &host })?;
                self.service.retry_now(&host).map_err(Refusal::Failed)?;
                let label = self.label(&host);
                self.done(format!("Trying {label} now."))
            }
            Intent::Forget { host } => {
                self.allow(Action::Forget { host: &host })?;
                self.ui.confirm = Some(Confirm::Forget(host));
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::ConfirmForget { host } => {
                if self.ui.confirm != Some(Confirm::Forget(host.clone())) {
                    return Err(Refusal::Stale);
                }
                self.allow(Action::Forget { host: &host })?;
                let label = self.label(&host);
                self.service.forget(&host).map_err(Refusal::Failed)?;
                self.ui.confirm = None;
                self.done(format!(
                    "Forgot {label}. It keeps this device's access until someone revokes it."
                ))
            }
            Intent::ScanInvitation => {
                self.allow(Action::ScanInvitation)?;
                self.ask(InputPurpose::Invitation, Target::Invitation, true)
            }
            Intent::PasteInvitation => {
                self.allow(Action::PasteInvitation)?;
                self.ask(InputPurpose::Invitation, Target::Invitation, false)
            }
            Intent::EnterCode { host, enrollment } => {
                self.allow(Action::Approve {
                    host: &host,
                    enrollment: &enrollment,
                })?;
                self.ask(
                    InputPurpose::ApprovalCode,
                    Target::Code { host, enrollment },
                    false,
                )
            }
            Intent::Deny { host, enrollment } => {
                self.allow(Action::Approve {
                    host: &host,
                    enrollment: &enrollment,
                })?;
                self.service
                    .deny_enrollment(&host, &enrollment)
                    .map_err(Refusal::Failed)?;
                let label = self.label(&host);
                self.done(format!("Denied {label}'s request."))
            }
            Intent::ConnectSsh => {
                self.allow(Action::ConnectSsh)?;
                self.ask(InputPurpose::SshDestination, Target::Ssh, false)
            }
            Intent::RunWithoutHost => {
                self.allow(Action::RunWithoutHost)?;
                self.service
                    .run_without_local_host()
                    .map_err(Refusal::Failed)?;
                self.done("This computer now runs with no local host.")
            }
            Intent::RefreshDevices { host } => {
                self.allow(Action::ReadDevices { host: &host })?;
                self.service
                    .refresh_devices(&host)
                    .map_err(Refusal::Failed)?;
                self.done("Devices refreshed.")
            }
            Intent::ToggleRight { host, right } => {
                self.allow(Action::IncludeRight { host: &host, right })?;
                let record = self
                    .snapshot
                    .host(&host)
                    .ok_or(Refusal::Denied(Denial::UnknownHost))?;
                let mut draft = self.ui.draft(record, self.snapshot.now);
                if let Some(index) = draft.iter().position(|held| *held == right) {
                    draft.remove(index);
                } else {
                    draft.push(right);
                    draft.sort();
                }
                self.ui.drafts.insert(host, draft);
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::CreateInvitation { host } => {
                let record = self
                    .snapshot
                    .host(&host)
                    .ok_or(Refusal::Denied(Denial::UnknownHost))?;
                let rights = Rights::new(self.ui.draft(record, self.snapshot.now)).ok();
                self.allow(Action::Invite {
                    host: &host,
                    rights: rights.as_ref(),
                })?;
                let rights = rights.ok_or(Refusal::Denied(Denial::NoRightsChosen))?;
                let expiry = self.grant_expiry(&host);
                let created = self
                    .service
                    .create_invitation(&host, &rights, expiry)
                    .map_err(Refusal::Failed)?;
                if !created.code.starts_with(INVITATION_PREFIX) || created.rights != rights {
                    return Err(Refusal::Failed(Error::new(
                        Code::Malformed,
                        "created invitation differs from the request",
                    )));
                }
                self.ui.invitations.insert(host, created);
                self.done("Invitation created. Share it with the new device.")
            }
            Intent::CancelInvitation { host, invitation } => {
                let created = self
                    .ui
                    .invitations
                    .get(&host)
                    .filter(|created| created.invitation == invitation)
                    .ok_or(Refusal::Stale)?;
                let rights = created.rights.clone();
                self.allow(Action::Invite {
                    host: &host,
                    rights: Some(&rights),
                })?;
                self.service
                    .cancel_invitation(&host, &invitation)
                    .map_err(Refusal::Failed)?;
                self.ui.invitations.remove(&host);
                self.done("Invitation cancelled. It can no longer be used.")
            }
            Intent::DismissInvitation { host } => {
                self.ui.invitations.remove(&host).ok_or(Refusal::Stale)?;
                self.ui.notice = Some(Notice {
                    kind: NoticeKind::Done,
                    text: "The invitation stays valid until it's used or expires.".into(),
                });
                Ok(Outcome::Updated)
            }
            Intent::Revoke { host, device } => {
                self.allow(Action::Revoke {
                    host: &host,
                    device: &device,
                })?;
                self.ui.confirm = Some(Confirm::Revoke(host, device));
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::ConfirmRevoke { host, device } => {
                if self.ui.confirm != Some(Confirm::Revoke(host.clone(), device.clone())) {
                    return Err(Refusal::Stale);
                }
                self.allow(Action::Revoke {
                    host: &host,
                    device: &device,
                })?;
                self.service
                    .revoke(&host, &device)
                    .map_err(Refusal::Failed)?;
                self.ui.confirm = None;
                // The revocation stands even if the refreshed list fails.
                let _ = self.service.refresh_devices(&host);
                self.done("Device revoked. It can no longer reach this computer.")
            }
            Intent::Cancel => {
                self.ui.confirm = None;
                self.close_input();
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::ImportOwnerKey => {
                self.allow(Action::ImportOwnerKey)?;
                self.ask(InputPurpose::OwnerKey, Target::OwnerKey, false)
            }
            Intent::ListInDirectory { host } => {
                self.allow(Action::ListInDirectory { host: &host })?;
                self.ask(
                    InputPurpose::DirectoryLabel,
                    Target::DirectoryLabel { host },
                    false,
                )
            }
            Intent::EditLabel { host, revision } => {
                self.allow(Action::EditListing {
                    host: &host,
                    revision,
                })?;
                self.ask(
                    InputPurpose::DirectoryLabel,
                    Target::EditLabel { host, revision },
                    false,
                )
            }
            Intent::EditWeight { host, revision } => {
                self.allow(Action::EditListing {
                    host: &host,
                    revision,
                })?;
                self.ask(
                    InputPurpose::DirectoryWeight,
                    Target::EditWeight { host, revision },
                    false,
                )
            }
            Intent::RemoveFromDirectory { host, revision } => {
                self.allow(Action::EditListing {
                    host: &host,
                    revision,
                })?;
                self.ui.confirm = Some(Confirm::Delist(host, revision));
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::ConfirmRemoveFromDirectory { host, revision } => {
                if self.ui.confirm != Some(Confirm::Delist(host.clone(), revision)) {
                    return Err(Refusal::Stale);
                }
                self.allow(Action::EditListing {
                    host: &host,
                    revision,
                })?;
                let label = self.label(&host);
                self.service
                    .remove_from_directory(&host, revision)
                    .map_err(Refusal::Failed)?;
                self.ui.confirm = None;
                self.done(format!(
                    "Removed {label} from your directory. It no longer gets new work."
                ))
            }
            Intent::KeepDirectory { revision } => {
                self.allow(Action::KeepDirectory { revision })?;
                self.service
                    .keep_directory(revision)
                    .map_err(Refusal::Failed)?;
                self.done(format!(
                    "Published this device's version of your directory as revision {}.",
                    revision.saturating_add(1)
                ))
            }
            Intent::RemoveSshHost { host } => {
                self.allow(Action::RemoveSsh { host: &host })?;
                self.ui.confirm = Some(Confirm::RemoveSsh(host));
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Intent::ConfirmRemoveSshHost { host } => {
                if self.ui.confirm != Some(Confirm::RemoveSsh(host.clone())) {
                    return Err(Refusal::Stale);
                }
                self.allow(Action::RemoveSsh { host: &host })?;
                let label = self.label(&host);
                self.service.remove_ssh(&host).map_err(Refusal::Failed)?;
                self.ui.confirm = None;
                self.done(format!("Removing {label} over SSH."))
            }
            Intent::ContinueOnboarding => {
                self.allow(Action::ContinueFirstRun)?;
                self.service.complete_first_run().map_err(Refusal::Failed)?;
                self.reload();
                self.ui.screen = Screen::Computers;
                self.ui.input = None;
                self.ui.notice = None;
                Ok(Outcome::ContinueOnboarding)
            }
        }
    }

    fn accept(&mut self, token: &str, value: &str) -> Result<Outcome, Refusal> {
        let Some(input) = &self.ui.input else {
            return Err(Refusal::Stale);
        };
        let value = match input.request.accept(token, value) {
            Ok(value) => value,
            Err(rust_native::InputError::TooLong) => {
                return Err(Refusal::Input("That's too long. Copy it again.".into()));
            }
            Err(_) => return Err(Refusal::Stale),
        };
        let target = input.target.clone();
        if let Target::SshPrompt { id } = target {
            // A password is passed exactly as typed.
            self.service
                .answer_ssh_prompt(id, Some(value))
                .map_err(Refusal::Failed)?;
            self.ui.input = None;
            self.ui.notice = None;
            return Ok(Outcome::Updated);
        }
        let value = value.trim();
        match target {
            Target::SshPrompt { .. } => Err(Refusal::Stale),
            Target::Pull { host } => {
                self.allow(Action::Terminal { host: &host })?;
                if value.is_empty()
                    || value.len() > 4096
                    || value.chars().any(char::is_control)
                    || !(value.starts_with('/') || value.starts_with("~/"))
                {
                    return Err(Refusal::Input(
                        "Enter a path on one line that starts with / or ~/.".into(),
                    ));
                }
                let (path, bytes) = self
                    .service
                    .pull_file(&host, value, MAX_PULL_BYTES)
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                let size = bytes.len();
                self.keep_capture(host, Some(path.clone()), bytes)?;
                self.done(format!("Copied {path} ({}).", byte_size(size as u64)))
            }
            Target::Prompt { host } => {
                self.allow(Action::Operate { host: &host })?;
                if value.is_empty() {
                    return Err(Refusal::Input("Write what the computer should do.".into()));
                }
                self.ui.orders.entry(host).or_default().prompt = Some(value.to_owned());
                self.ui.input = None;
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Target::Workspace { host } => {
                self.allow(Action::Operate { host: &host })?;
                if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
                    return Err(Refusal::Input(
                        "Enter a workspace name of 1 to 128 bytes on one line.".into(),
                    ));
                }
                self.ui.orders.entry(host).or_default().workspace = Some(value.to_owned());
                self.ui.input = None;
                self.ui.notice = None;
                Ok(Outcome::Updated)
            }
            Target::Steer {
                host,
                task,
                revision,
            } => {
                self.allow(Action::Operate { host: &host })?;
                if value.is_empty() {
                    return Err(Refusal::Input("Enter the new instructions.".into()));
                }
                self.service
                    .steer_task(&host, &task, revision, value)
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                let label = self.label(&host);
                self.done(format!("Sent new instructions to {label}."))
            }
            Target::OwnerKey => {
                self.allow(Action::ImportOwnerKey)?;
                if value.is_empty() {
                    return Err(Refusal::Input(
                        "Enter the owner's secret key as 64 hex characters or nsec.".into(),
                    ));
                }
                self.service
                    .import_owner_key(value)
                    .map_err(|error| match error.code {
                        // Not a key, or not the owner a held grant names.
                        Code::Malformed => Refusal::Input(
                            "That isn't the owner key your computers name. Check it and try again."
                                .into(),
                        ),
                        _ => Refusal::Failed(error),
                    })?;
                self.ui.input = None;
                self.done("This device now holds your owner key. It reads your directory.")
            }
            Target::EditLabel { host, revision } => {
                self.allow(Action::EditListing {
                    host: &host,
                    revision,
                })?;
                directory_label(value)?;
                self.service
                    .edit_listing(&host, revision, &ListingChange::Label(value.to_owned()))
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                self.done(format!(
                    "Renamed the computer to {value} in your directory."
                ))
            }
            Target::EditWeight { host, revision } => {
                self.allow(Action::EditListing {
                    host: &host,
                    revision,
                })?;
                let weight = value
                    .parse::<u32>()
                    .ok()
                    .filter(|weight| *weight <= coder_reach::directory::MAX_WEIGHT)
                    .ok_or_else(|| Refusal::Input("Enter a whole number from 0 to 1000.".into()))?;
                let label = self.label(&host);
                self.service
                    .edit_listing(&host, revision, &ListingChange::Weight(weight))
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                self.done(if weight == 0 {
                    format!("{label} stays in your directory with weight 0. It gets no new work.")
                } else {
                    format!("{label} now has weight {weight} in your directory.")
                })
            }
            Target::DirectoryLabel { host } => {
                self.allow(Action::ListInDirectory { host: &host })?;
                directory_label(value)?;
                self.service
                    .list_in_directory(&host, value)
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                self.done(format!("Added {value} to your directory."))
            }
            Target::Invitation => {
                self.allow(Action::PasteInvitation)?;
                if value.starts_with("coder-pair:") {
                    return Err(Refusal::Input(
                        "This is a Chats pairing code. Use it on the Chats screen.".into(),
                    ));
                }
                if !value.starts_with(INVITATION_PREFIX) {
                    return Err(Refusal::Input(
                        "This isn't a computer invitation. Computer invitations start with coder-host:.".into(),
                    ));
                }
                let host = self
                    .service
                    .redeem_invitation(value)
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                self.reload();
                let label = self.label(&host);
                if self.ui.screen == Screen::Add {
                    self.ui.screen = Screen::Computers;
                }
                self.ui.notice = Some(Notice {
                    kind: NoticeKind::Done,
                    text: format!("Added {label}."),
                });
                Ok(Outcome::Updated)
            }
            Target::Code { host, enrollment } => {
                self.allow(Action::Approve {
                    host: &host,
                    enrollment: &enrollment,
                })?;
                let code = normalize_code(value).map_err(|_| {
                    Refusal::Input("Enter the 8-character code the computer shows.".into())
                })?;
                let record = self
                    .snapshot
                    .host(&host)
                    .ok_or(Refusal::Denied(Denial::UnknownHost))?;
                let request = record
                    .enrollments
                    .iter()
                    .find(|request| request.enrollment == enrollment)
                    .ok_or(Refusal::Denied(Denial::UnknownRequest))?;
                let held = record.enrollment.rights(self.snapshot.now);
                // Approval only narrows: the request's rights, and for an
                // administrator also its own.
                let rights = Rights::new(request.rights.iter().filter(|right| {
                    self.snapshot.owner || held.is_some_and(|held| held.contains(*right))
                }))
                .map_err(|_| Refusal::Denied(Denial::NoRightsChosen))?;
                let expiry = self.grant_expiry(&host);
                self.service
                    .approve_enrollment(&host, &enrollment, &code, &rights, expiry)
                    .map_err(Refusal::Failed)?;
                self.ui.input = None;
                let label = self.label(&host);
                self.done(format!("Approved {label}."))
            }
            Target::Ssh => {
                self.allow(Action::ConnectSsh)?;
                // The same rule coder-ssh applies: never an option, never
                // whitespace. The service checks again with coder-ssh.
                if value.is_empty()
                    || value.len() > 255
                    || value.starts_with('-')
                    || value.chars().any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err(Refusal::Input(
                        "Enter one SSH destination with no spaces, such as user@host.".into(),
                    ));
                }
                self.service.connect_ssh(value).map_err(Refusal::Failed)?;
                self.ui.input = None;
                self.done(format!("Setting up a host on {value} over SSH."))
            }
        }
    }
}

/// Check a directory label: 1 to 64 bytes with no control characters.
fn directory_label(value: &str) -> Result<(), Refusal> {
    if value.is_empty()
        || value.len() > coder_reach::directory::MAX_LABEL_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(Refusal::Input(
            "Enter a name of 1 to 64 bytes with no control characters.".into(),
        ));
    }
    Ok(())
}

/// A byte count as people read it: bytes, KB, or MB.
pub(crate) fn byte_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} bytes")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}
