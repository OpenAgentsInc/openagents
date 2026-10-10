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
    /// One host: its status and route, and what this device can do there.
    Host { host: String },
    /// Order work on one host: a workspace it shares and a prompt.
    Order { host: String },
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
    /// Ask for the owner key, so this device can read the owner directory.
    ImportOwnerKey,
    /// Ask for a label, then add an enrolled host to the owner directory.
    ListInDirectory {
        host: String,
    },
    /// Ask for a new directory label for a listed host. `revision` is the
    /// directory revision the screen showed; an edit against another
    /// revision is refused as stale.
    EditLabel {
        host: String,
        revision: u64,
    },
    /// Ask for a new placement weight for a listed host.
    EditWeight {
        host: String,
        revision: u64,
    },
    /// Ask to confirm removing a host from the owner directory.
    RemoveFromDirectory {
        host: String,
        revision: u64,
    },
    /// Publish the next revision without the host. A grant this device holds
    /// keeps the host reachable; it leaves the directory and placement.
    ConfirmRemoveFromDirectory {
        host: String,
        revision: u64,
    },
    /// End a directory conflict at `revision` by publishing this device's
    /// last trusted version above it.
    KeepDirectory {
        revision: u64,
    },
    /// Ask to confirm removing a host this device set up over SSH.
    RemoveSshHost {
        host: String,
    },
    /// Run `coder-ssh`'s explicit remove, which stops only a host its setup
    /// started, then forget the computer on this device.
    ConfirmRemoveSshHost {
        host: String,
    },
    /// Read the workspaces the host shares again (`workspace.list`).
    RefreshWorkspaces {
        host: String,
    },
    /// Choose one of the workspaces the host listed for the order.
    ChooseWorkspace {
        host: String,
        workspace: String,
    },
    /// Ask for a workspace label, for a host that lists none.
    EnterWorkspace {
        host: String,
    },
    /// Ask for the order's prompt.
    WritePrompt {
        host: String,
    },
    /// Send the order (`task.create`).
    SubmitTask {
        host: String,
    },
    /// Ask for replacement instructions (`task.steer`). `revision` is the
    /// task revision the screen showed.
    SteerTask {
        host: String,
        task: String,
        revision: u64,
    },
    /// Ask to confirm stopping a task.
    CancelTask {
        host: String,
        task: String,
        revision: u64,
    },
    /// Ask the host to stop the task (`task.cancel`).
    ConfirmCancelTask {
        host: String,
        task: String,
        revision: u64,
    },
    /// Open a terminal on the host. The client's terminal screen takes over
    /// from [`crate::Computers::take_terminal`].
    OpenTerminal {
        host: String,
    },
    /// Take a picture of the computer's screen and show it here
    /// (`computer`: `screenshot`). Needs `terminal`.
    Screenshot {
        host: String,
    },
    /// Ask for a path, then copy that file from the computer and show it
    /// here (`computer`: `stat` and `read`). Needs `terminal`.
    PullFile {
        host: String,
    },
    /// Stop showing what was last brought back from the computer.
    ClearCapture {
        host: String,
    },
}
