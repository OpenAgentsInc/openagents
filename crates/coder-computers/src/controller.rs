//! The Computers application host: current view identity, screen state,
//! intent handling, and input requests.
use crate::authority::{Action, Denial, check};
use crate::intent::{Intent, Screen};
use crate::model::{Capabilities, CreatedInvitation, HostRecord, Snapshot};
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
}

/// A request for one value the Rust Native tree cannot collect yet. The
/// adapter shows its native field or scanner and answers with the token.
/// Rust validates the value; the adapter only carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputRequest {
    pub token: String,
    pub purpose: InputPurpose,
    /// The field's accessible label.
    pub label: String,
    /// One sentence that says what to enter.
    pub prompt: String,
    /// Open the scanner first.
    pub scan: bool,
    pub max_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Invitation,
    Code { host: String, enrollment: String },
    Ssh,
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

    /// The value the adapter should collect, if any.
    pub fn input(&self) -> Option<&InputRequest> {
        self.ui.input.as_ref().map(|input| &input.request)
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

    /// Accept the value for the current input request.
    pub fn submit(&mut self, token: &str, value: &str) -> Result<Outcome, Refusal> {
        let result = self.accept(token, value);
        self.finish(result)
    }

    /// Close the current input request if `token` names it.
    pub fn cancel_input(&mut self, token: &str) -> Result<Outcome, Refusal> {
        let result = match &self.ui.input {
            Some(input) if input.request.token == token => {
                self.ui.input = None;
                Ok(Outcome::Updated)
            }
            _ => Err(Refusal::Stale),
        };
        self.finish(result)
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
                if let Screen::Access { host } = &self.ui.screen
                    && !hosts.contains(host)
                {
                    self.ui.screen = Screen::Computers;
                }
            }
            Err(error) => {
                self.ui.notice = Some(Notice {
                    kind: NoticeKind::Refused,
                    text: describe(&error),
                });
            }
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
        };
        self.ui.input = Some(PendingInput {
            request: InputRequest {
                token: format!("{}-input-{}", self.instance, self.inputs),
                purpose,
                label: label.into(),
                prompt: prompt.into(),
                scan,
                max_bytes: MAX_INPUT_BYTES,
            },
            target,
        });
        self.ui.notice = None;
        Ok(Outcome::InputRequested)
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
                if let Screen::Access { host } = &screen
                    && self.snapshot.host(host).is_none()
                {
                    return Err(Refusal::Denied(Denial::UnknownHost));
                }
                if screen == Screen::FirstRun && self.snapshot.first_run_complete {
                    return Err(Refusal::NotInteractive);
                }
                self.ui.screen = screen;
                self.ui.confirm = None;
                self.ui.input = None;
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
                self.ui.input = None;
                self.ui.notice = None;
                Ok(Outcome::Updated)
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
        let target = match &self.ui.input {
            Some(input) if input.request.token == token => input.target.clone(),
            _ => return Err(Refusal::Stale),
        };
        if value.len() > MAX_INPUT_BYTES {
            return Err(Refusal::Input("That's too long. Copy it again.".into()));
        }
        let value = value.trim();
        match target {
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
                self.done(format!("Connecting to {value} over SSH."))
            }
        }
    }
}
