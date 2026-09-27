//! The closed intent and screen types the Computers views carry.
//!
//! An intent names what a person asked for. It is resolved only from the
//! current validated view, then checked again by the controller against the
//! current snapshot. It never grants authority.
use coder_access::Right;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case", deny_unknown_fields)]
pub enum Screen {
    /// Connect at least one computer, then continue to onboarding.
    FirstRun,
    /// One row per host.
    Computers,
    /// Ways to add a computer.
    Add,
    /// Enrolled devices and invitations for one host.
    Access { host: String },
    /// Hosts and tasks that need attention.
    Activity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    Show {
        screen: Screen,
    },
    Refresh,
    /// Switch a host on or off. Switching off keeps it in the list.
    SetEnabled {
        host: String,
        enabled: bool,
    },
    RetryNow {
        host: String,
    },
    Forget {
        host: String,
    },
    ConfirmForget {
        host: String,
    },
    /// Ask the platform adapter to scan an invitation.
    ScanInvitation,
    /// Ask the platform adapter for a pasted invitation.
    PasteInvitation,
    /// Ask for the code a headless host shows.
    EnterCode {
        host: String,
        enrollment: String,
    },
    Deny {
        host: String,
        enrollment: String,
    },
    ConnectSsh,
    RunWithoutHost,
    RefreshDevices {
        host: String,
    },
    /// Include or exclude one right in the next invitation.
    ToggleRight {
        host: String,
        right: Right,
    },
    CreateInvitation {
        host: String,
    },
    CancelInvitation {
        host: String,
        invitation: String,
    },
    /// Hide a created invitation. It stays valid until it expires or is
    /// redeemed; cancel it to end it.
    DismissInvitation {
        host: String,
    },
    Revoke {
        host: String,
        device: String,
    },
    ConfirmRevoke {
        host: String,
        device: String,
    },
    /// Close a confirmation or an input request.
    Cancel,
    ContinueOnboarding,
}
