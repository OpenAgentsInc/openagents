//! One check decides both whether a control is enabled and whether its
//! intent may run.
//!
//! The projection disables a control with [`Denial::reason`] as its visible
//! reason. The controller runs the same check against the current snapshot
//! before it calls the service, so a stale or forged activation cannot skip
//! it. Passing this check grants nothing: the host still checks its own grant
//! record for every operation.
use crate::model::{
    Capabilities, DirectoryState, Enrollment, HostRecord, HostStatus, LocalHost, Platform,
    ServiceState, Snapshot, right_label,
};
use coder_access::protocol::DeviceState;
use coder_access::{Right, Rights};

/// An action a Computers control offers.
#[derive(Clone, Copy, Debug)]
pub enum Action<'a> {
    SetEnabled {
        host: &'a str,
    },
    RetryNow {
        host: &'a str,
    },
    Forget {
        host: &'a str,
    },
    ScanInvitation,
    PasteInvitation,
    Approve {
        host: &'a str,
        enrollment: &'a str,
    },
    ConnectSsh,
    RunWithoutHost,
    ReadDevices {
        host: &'a str,
    },
    /// Include one right in a new invitation.
    IncludeRight {
        host: &'a str,
        right: Right,
    },
    Invite {
        host: &'a str,
        rights: Option<&'a Rights>,
    },
    Revoke {
        host: &'a str,
        device: &'a str,
    },
    ContinueFirstRun,
    /// Hold the owner key on this device to read the owner directory.
    ImportOwnerKey,
    /// Add an enrolled host to the owner directory. An owner action.
    ListInDirectory {
        host: &'a str,
    },
    /// Change or remove a listed host's directory entry, against the
    /// directory revision the screen showed. An owner action.
    EditListing {
        host: &'a str,
        revision: u64,
    },
    /// Publish this device's version above a directory conflict at
    /// `revision`. An owner action.
    KeepDirectory {
        revision: u64,
    },
    /// Remove a host this device set up over SSH.
    RemoveSsh {
        host: &'a str,
    },
    /// Order, steer, or cancel work on a host (`task.create`, `task.steer`,
    /// `task.cancel`, and `workspace.list`), which need `operate`.
    Operate {
        host: &'a str,
    },
    /// Open a terminal on a host (`terminal.open`), which needs `terminal`.
    Terminal {
        host: &'a str,
    },
    /// Read what a task changed (`task.review`), which needs `observe`.
    /// Publishing it is [`Action::Operate`].
    Review {
        host: &'a str,
    },
}

/// Why a control is unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Denial {
    Unavailable(String),
    UnknownHost,
    UnknownRequest,
    NoCamera,
    SshNeedsComputer,
    PhoneRunsNoHost,
    LocalHostRunning,
    AlreadyClientOnly,
    NotEnrolled,
    Revoked,
    OutOfDate,
    Offline,
    MissingRight(Right),
    NotApprover,
    NoRightsChosen,
    RightNotHeld(Right),
    ThisDevice,
    NotActive,
    NeedsComputer,
    SshNotSetUp,
    SshRunning,
    OwnerKeyNeedsComputer,
    OwnerKeyHeld,
    NotOwner,
    DirectoryNotRead,
    DirectoryConflict,
    AlreadyListed,
    NotListed,
    StaleDirectory,
    NoConflict,
    NotSetUpOverSsh,
}

impl Denial {
    /// User-facing copy for a disabled control or a refused intent.
    pub fn reason(&self) -> String {
        match self {
            Self::Unavailable(reason) => reason.clone(),
            Self::UnknownHost => "This computer is no longer in your list.".into(),
            Self::UnknownRequest => "This request is no longer waiting.".into(),
            Self::NoCamera => "This device can't scan. Paste the invitation instead.".into(),
            Self::SshNeedsComputer => {
                "SSH connections start from the desktop or terminal app.".into()
            }
            Self::PhoneRunsNoHost => "Phones connect to computers and never run a host.".into(),
            Self::LocalHostRunning => "A host already runs on this computer.".into(),
            Self::AlreadyClientOnly => "This computer already runs with no local host.".into(),
            Self::NotEnrolled => "This device has no access to this computer.".into(),
            Self::Revoked => "This computer revoked this device's access.".into(),
            Self::OutOfDate => format!(
                "This app and the computer don't match. Update this app, or update Coder: {}",
                crate::project::UPDATE_CODER
            ),
            Self::Offline => "The computer is offline. Connect it first.".into(),
            Self::MissingRight(right) => format!(
                "This device doesn't have the \"{}\" right on this computer.",
                right_label(*right)
            ),
            Self::NotApprover => {
                "Only the owner or a device that can manage access can approve.".into()
            }
            Self::NoRightsChosen => "Choose at least one right.".into(),
            Self::RightNotHeld(right) => format!(
                "You can share only rights this device holds. It lacks \"{}\".",
                right_label(*right)
            ),
            Self::ThisDevice => {
                "This is the device you're using. Revoke it from another device.".into()
            }
            Self::NotActive => "This device's access already ended.".into(),
            Self::NeedsComputer => "Add at least one computer to continue.".into(),
            Self::SshNotSetUp => "This app has no Coder release to install over SSH.".into(),
            Self::SshRunning => "An SSH setup is already running. Wait for it to finish.".into(),
            Self::OwnerKeyNeedsComputer => {
                "Add a computer first. The owner key must be the one your computers name.".into()
            }
            Self::OwnerKeyHeld => "This device already holds your owner key.".into(),
            Self::NotOwner => {
                "Only a device that holds your owner key can change your directory.".into()
            }
            Self::DirectoryNotRead => {
                "Your directory hasn't been read yet. Refresh, then try again.".into()
            }
            Self::DirectoryConflict => {
                "Your directory has two different versions at one revision. Publish a newer one from the device that made the change.".into()
            }
            Self::AlreadyListed => "Your directory already lists this computer.".into(),
            Self::NotListed => "Your directory doesn't list this computer.".into(),
            Self::StaleDirectory => {
                "Your directory changed since this screen was drawn. Check it, then try again.".into()
            }
            Self::NoConflict => "Your directory has no conflict to settle.".into(),
            Self::NotSetUpOverSsh => {
                "This device didn't set up this computer over SSH. Stop its host on the computer.".into()
            }
        }
    }
}

fn ready(snapshot: &Snapshot) -> Result<(), Denial> {
    match &snapshot.service {
        ServiceState::Ready => Ok(()),
        ServiceState::Unavailable { reason } => Err(Denial::Unavailable(reason.clone())),
    }
}

fn host<'a>(snapshot: &'a Snapshot, key: &str) -> Result<&'a HostRecord, Denial> {
    snapshot.host(key).ok_or(Denial::UnknownHost)
}

/// The host is enrolled, current, compatible, and online, and this device
/// holds `right` on it. Returns the held rights.
fn live<'a>(snapshot: &'a Snapshot, key: &str, right: Right) -> Result<&'a Rights, Denial> {
    let record = host(snapshot, key)?;
    match HostStatus::derive(record, snapshot.now) {
        HostStatus::Revoked => return Err(Denial::Revoked),
        HostStatus::NotEnrolled { .. } => return Err(Denial::NotEnrolled),
        HostStatus::OutOfDate { .. } => return Err(Denial::OutOfDate),
        HostStatus::Online { .. } => {}
        HostStatus::Offline { .. } | HostStatus::Connecting { .. } => return Err(Denial::Offline),
    }
    let held = record
        .enrollment
        .rights(snapshot.now)
        .ok_or(Denial::NotEnrolled)?;
    if held.contains(right) {
        Ok(held)
    } else {
        Err(Denial::MissingRight(right))
    }
}

/// Whether `action` may run now.
pub fn check(snapshot: &Snapshot, caps: Capabilities, action: Action<'_>) -> Result<(), Denial> {
    ready(snapshot)?;
    match action {
        Action::SetEnabled { host: key }
        | Action::RetryNow { host: key }
        | Action::Forget { host: key } => {
            if host(snapshot, key)?.directory_only() {
                Err(Denial::NotEnrolled)
            } else {
                Ok(())
            }
        }
        Action::ScanInvitation => {
            if caps.camera {
                Ok(())
            } else {
                Err(Denial::NoCamera)
            }
        }
        Action::PasteInvitation => Ok(()),
        Action::Approve {
            host: key,
            enrollment,
        } => {
            let record = host(snapshot, key)?;
            if !record.enrollments.iter().any(|request| {
                request.enrollment == enrollment && request.expires_at > snapshot.now
            }) {
                return Err(Denial::UnknownRequest);
            }
            let admin = record
                .enrollment
                .rights(snapshot.now)
                .is_some_and(|rights| rights.contains(Right::AccessAdmin));
            if snapshot.owner || admin {
                Ok(())
            } else {
                Err(Denial::NotApprover)
            }
        }
        Action::ConnectSsh => {
            if !caps.ssh() {
                Err(Denial::SshNeedsComputer)
            } else if !snapshot.ssh_ready {
                Err(Denial::SshNotSetUp)
            } else if snapshot
                .ssh
                .as_ref()
                .is_some_and(|attempt| attempt.stage.running())
            {
                Err(Denial::SshRunning)
            } else {
                Ok(())
            }
        }
        Action::RunWithoutHost => match (&snapshot.local_host, caps.platform) {
            (_, Platform::Phone) | (LocalHost::NotSupported, _) => Err(Denial::PhoneRunsNoHost),
            (LocalHost::Running { .. }, _) => Err(Denial::LocalHostRunning),
            (LocalHost::ClientOnly, _) => Err(Denial::AlreadyClientOnly),
            (LocalHost::Undecided, _) => Ok(()),
        },
        Action::ReadDevices { host: key } => live(snapshot, key, Right::AccessRead).map(|_| ()),
        Action::Operate { host: key } => live(snapshot, key, Right::Operate).map(|_| ()),
        Action::Terminal { host: key } => live(snapshot, key, Right::Terminal).map(|_| ()),
        Action::Review { host: key } => live(snapshot, key, Right::Observe).map(|_| ()),
        Action::IncludeRight { host: key, right } => {
            let held = live(snapshot, key, Right::AccessAdmin)?;
            if held.contains(right) {
                Ok(())
            } else {
                Err(Denial::RightNotHeld(right))
            }
        }
        Action::Invite { host: key, rights } => {
            let held = live(snapshot, key, Right::AccessAdmin)?;
            match rights {
                None => Err(Denial::NoRightsChosen),
                Some(rights) => match rights.first_missing(held) {
                    Some(right) => Err(Denial::RightNotHeld(right)),
                    None => Ok(()),
                },
            }
        }
        Action::Revoke { host: key, device } => {
            live(snapshot, key, Right::AccessAdmin)?;
            if device == snapshot.device {
                return Err(Denial::ThisDevice);
            }
            let active = matches!(
                &host(snapshot, key)?.devices,
                crate::model::DeviceList::Loaded { devices, .. }
                    if devices.iter().any(|row| row.device == device && row.state == DeviceState::Active)
            );
            if active {
                Ok(())
            } else {
                Err(Denial::NotActive)
            }
        }
        Action::ContinueFirstRun => {
            if snapshot.has_enrolled_host() {
                Ok(())
            } else {
                Err(Denial::NeedsComputer)
            }
        }
        Action::ImportOwnerKey => {
            // Every platform offers it: each adapter masks a secret input
            // request. The service keeps the rest of the NIP-REACH rule.
            if snapshot.directory != DirectoryState::NoOwnerKey {
                Err(Denial::OwnerKeyHeld)
            } else if !snapshot
                .hosts
                .iter()
                .any(|host| matches!(host.enrollment, Enrollment::Enrolled { .. }))
            {
                Err(Denial::OwnerKeyNeedsComputer)
            } else {
                Ok(())
            }
        }
        Action::EditListing {
            host: key,
            revision,
        } => {
            let record = host(snapshot, key)?;
            match snapshot.directory {
                DirectoryState::NoOwnerKey => return Err(Denial::NotOwner),
                DirectoryState::Conflict { .. } => return Err(Denial::DirectoryConflict),
                DirectoryState::Loading | DirectoryState::Failed { .. } => {
                    return Err(Denial::DirectoryNotRead);
                }
                DirectoryState::Current {
                    revision: shown, ..
                } if shown != Some(revision) => {
                    return Err(Denial::StaleDirectory);
                }
                DirectoryState::Current { .. } => {}
            }
            if record.listing.is_some() {
                Ok(())
            } else {
                Err(Denial::NotListed)
            }
        }
        Action::KeepDirectory { revision } => match snapshot.directory {
            DirectoryState::NoOwnerKey => Err(Denial::NotOwner),
            DirectoryState::Conflict { revision: shown } if shown == revision => Ok(()),
            DirectoryState::Conflict { .. } => Err(Denial::StaleDirectory),
            _ => Err(Denial::NoConflict),
        },
        Action::RemoveSsh { host: key } => {
            let record = host(snapshot, key)?;
            if !caps.ssh() {
                Err(Denial::SshNeedsComputer)
            } else if record.ssh.is_none() {
                Err(Denial::NotSetUpOverSsh)
            } else if !snapshot.ssh_ready {
                Err(Denial::SshNotSetUp)
            } else if snapshot
                .ssh
                .as_ref()
                .is_some_and(|attempt| attempt.stage.running())
            {
                Err(Denial::SshRunning)
            } else {
                Ok(())
            }
        }
        Action::ListInDirectory { host: key } => {
            let record = host(snapshot, key)?;
            match snapshot.directory {
                DirectoryState::NoOwnerKey => return Err(Denial::NotOwner),
                DirectoryState::Conflict { .. } => return Err(Denial::DirectoryConflict),
                DirectoryState::Loading | DirectoryState::Failed { .. } => {
                    return Err(Denial::DirectoryNotRead);
                }
                DirectoryState::Current { .. } => {}
            }
            if record.listing.is_some() {
                return Err(Denial::AlreadyListed);
            }
            match HostStatus::derive(record, snapshot.now) {
                HostStatus::Revoked => Err(Denial::Revoked),
                HostStatus::NotEnrolled { .. } => Err(Denial::NotEnrolled),
                _ => Ok(()),
            }
        }
    }
}
