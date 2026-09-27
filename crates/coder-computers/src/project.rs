//! Pure projection from a snapshot and screen state to a Rust Native tree.
//!
//! Every disabled control is followed by a status text node keyed
//! `<control>-reason` that states why. Rows use stacks rather than lists so a
//! platform can place the whole screen in one scrolling container.
use crate::authority::{Action, Denial, check};
use crate::controller::{Confirm, Notice, UiState};
use crate::intent::{Intent, Screen};
use crate::model::{
    Capabilities, DataState, DeviceList, DeviceRow, DirectoryState, HostRecord, HostStatus,
    LocalHost, NotEnrolledCause, OfflineCause, OutOfDate, Platform, ServiceState, Snapshot,
    SshRemoval, SshStage, right_label,
};
use coder_access::Right;
use coder_access::protocol::{DeviceState, OriginKind};
use coder_link::{Failure, Stage};
use coder_reach::hints::Class;
use coder_ui::theme::{Intensity, NEAR_BLACK};
use nostr::activity_summary::{ActivitySummary, Attention, Phase, SubjectKind};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};

fn color(intensity: Intensity) -> Color {
    let rgb = intensity.color();
    Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn text(key: impl Into<String>, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    let intensity = match role {
        TextRole::Heading => Intensity::Full,
        TextRole::Status => Intensity::Half,
        _ => Intensity::ThreeQuarters,
    };
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(color(intensity)),
            weight: (role == TextRole::Heading).then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

/// A button whose availability comes from one authority check. A denied
/// button is disabled and followed by its reason.
fn control(
    nodes: &mut Vec<Node<Intent>>,
    key: impl Into<String>,
    label: impl Into<String>,
    intent: Intent,
    allowed: Result<(), Denial>,
) {
    let key = key.into();
    let enabled = allowed.is_ok();
    nodes.push(Node {
        key: key.clone(),
        style: Style {
            foreground: Some(color(if enabled {
                Intensity::Full
            } else {
                Intensity::Quarter
            })),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled,
            intent,
        },
    });
    if let Err(denial) = allowed {
        nodes.push(text(
            format!("{key}-reason"),
            format!("Unavailable: {}", denial.reason()),
            TextRole::Status,
        ));
    }
}

fn stack(
    key: impl Into<String>,
    axis: Axis,
    gap: Space,
    children: Vec<Node<Intent>>,
) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(gap),
            ..Style::default()
        },
        element: Element::Stack { axis, children },
    }
}

fn section(key: impl Into<String>, children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack(key, Axis::Vertical, Space::Xs, children);
    node.style.padding_top = Some(Space::Sm);
    node
}

/// Relative time for a past moment.
pub(crate) fn ago(now: u64, at: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..60 => "just now".into(),
        60..3_600 => format!("{} min ago", seconds / 60),
        3_600..86_400 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

/// Relative time for a future moment.
pub(crate) fn until(now: u64, at: u64) -> String {
    let seconds = at.saturating_sub(now);
    match seconds {
        0 => "expired".into(),
        1..120 => format!("expires in {seconds} s"),
        120..7_200 => format!("expires in {} min", seconds / 60),
        7_200..172_800 => format!("expires in {} h", seconds / 3_600),
        _ => format!("expires in {} days", seconds / 86_400),
    }
}

fn short_key(key: &str) -> String {
    if key.len() > 16 && key.is_ascii() {
        format!("{}…{}", &key[..8], &key[key.len() - 8..])
    } else {
        key.into()
    }
}

fn route_label(class: Class) -> &'static str {
    match class {
        Class::Loopback => "on this computer",
        Class::Lan => "over the local network",
        Class::Tailnet => "over your tailnet",
        Class::Public => "over a public address",
        Class::Relay => "through a relay",
    }
}

/// The status line: a word that names the state, then detail. The words
/// carry the meaning; no state depends on color or brightness alone.
pub fn status_line(host: &HostRecord, now: u64) -> String {
    match HostStatus::derive(host, now) {
        HostStatus::Online { data } => {
            let route = if host.tunnel.is_some_and(|tunnel| tunnel.in_use) {
                " through the SSH tunnel".to_owned()
            } else {
                host.route.map_or(String::new(), |class| {
                    format!(" {}", route_label(class))
                })
            };
            let data = match data {
                DataState::Current => "Up to date.",
                DataState::CatchingUp => "Catching up.",
                DataState::UpdatesFailed => "Updates failed; what you see may be out of date.",
            };
            format!("Online{route}. {data}")
        }
        HostStatus::Connecting { stage } => match stage {
            Stage::Establishing => "Connecting.".into(),
            Stage::Probing => "Connecting: checking the connection.".into(),
            Stage::Replacing => "Connecting: replacing the connection.".into(),
        },
        HostStatus::Offline { cause } => match cause {
            OfflineCause::SwitchedOff => "Offline: switched off. It stays in your list.".into(),
            OfflineCause::NotConnected => "Offline: not connected.".into(),
            OfflineCause::NoNetwork => "Offline: this device has no network.".into(),
            OfflineCause::Retrying { failure } => format!(
                "Offline: {} Retrying automatically.",
                match failure {
                    Some(Failure::Timeout) => "the last attempt timed out.",
                    Some(Failure::Closed) => "the connection closed.",
                    Some(Failure::NetworkUnavailable) => "the network dropped.",
                    _ => "the computer didn't answer.",
                }
            ),
            OfflineCause::Refused => {
                "Blocked: the computer didn't accept this device's key. Try again, or add the computer again.".into()
            }
            OfflineCause::Misconfigured => {
                "Blocked: this device's settings for the computer are invalid. Add the computer again.".into()
            }
        },
        HostStatus::OutOfDate { side } => match side {
            OutOfDate::Host => {
                "Out of date: the computer runs an older Coder. Update Coder on the computer.".into()
            }
            OutOfDate::ThisApp => {
                "Out of date: this app is older than the computer supports. Update this app.".into()
            }
            OutOfDate::Unknown => {
                "Out of date: this app and the computer share no protocol version. Update both.".into()
            }
        },
        HostStatus::NotEnrolled { cause } => match cause {
            NotEnrolledCause::NoAccess if host.listing.is_some() => {
                "Not enrolled: it's in your directory, and this device has no access yet. Add it with an invitation.".into()
            }
            NotEnrolledCause::NoAccess => {
                "Not enrolled: this device has no access. Add it with an invitation.".into()
            }
            NotEnrolledCause::Expired => {
                "Not enrolled: this device's access expired. Add it again with a new invitation.".into()
            }
        },
        HostStatus::Revoked => {
            "Revoked: this computer removed this device's access. Add it again with a new invitation.".into()
        }
    }
}

pub(crate) fn root(snapshot: &Snapshot, caps: Capabilities, ui: &UiState) -> Node<Intent> {
    let mut nodes = Vec::new();
    if ui.screen != Screen::FirstRun {
        let mut tabs = Vec::new();
        for (key, label, screen) in [
            ("tab-computers", "Computers", Screen::Computers),
            ("tab-add", "Add a computer", Screen::Add),
            ("tab-activity", "Activity", Screen::Activity),
        ] {
            control(&mut tabs, key, label, Intent::Show { screen }, Ok(()));
        }
        nodes.push(stack("tabs", Axis::Horizontal, Space::Md, tabs));
    }
    if let Some(Notice { text: value, .. }) = &ui.notice {
        nodes.push(text("notice", value, TextRole::Status));
    }
    if let ServiceState::Unavailable { reason } = &snapshot.service {
        nodes.push(text("service", reason, TextRole::Status));
    }
    if let Some(input) = &ui.input {
        let mut children = vec![text(
            "input-prompt",
            format!("Waiting for input: {}", input.request.prompt),
            TextRole::Status,
        )];
        control(
            &mut children,
            "input-cancel",
            "Cancel",
            Intent::Cancel,
            Ok(()),
        );
        nodes.push(section("input", children));
    }
    match &ui.screen {
        Screen::FirstRun => first_run(&mut nodes, snapshot, caps),
        Screen::Computers => computers(&mut nodes, snapshot, caps, ui),
        Screen::Add => {
            nodes.push(text("add-title", "Add a computer", TextRole::Heading));
            add(&mut nodes, snapshot, caps);
        }
        Screen::Access { host } => match snapshot.host(host) {
            Some(record) => access(&mut nodes, snapshot, caps, ui, record),
            None => nodes.push(text(
                "access-missing",
                "This computer is no longer in your list.",
                TextRole::Body,
            )),
        },
        Screen::Activity => activity(&mut nodes, snapshot),
    }
    let mut node = stack("computers-screen", Axis::Vertical, Space::Sm, nodes);
    node.style.background = Some(Color::rgb(
        (NEAR_BLACK >> 16) as u8,
        (NEAR_BLACK >> 8) as u8,
        NEAR_BLACK as u8,
    ));
    node.style.padding_start = Some(Space::Sm);
    node.style.padding_end = Some(Space::Sm);
    node
}

fn first_run(nodes: &mut Vec<Node<Intent>>, snapshot: &Snapshot, caps: Capabilities) {
    nodes.push(text(
        "first-run-title",
        "Connect a computer",
        TextRole::Heading,
    ));
    nodes.push(text(
        "first-run-body",
        "Coder works on your computers. Add at least one, then continue.",
        TextRole::Body,
    ));
    let enrolled = snapshot
        .hosts
        .iter()
        .filter(|host| host.enrollment.rights(snapshot.now).is_some())
        .count();
    nodes.push(text(
        "first-run-count",
        match enrolled {
            0 => "No computers added yet.".to_owned(),
            1 => "1 computer added.".to_owned(),
            n => format!("{n} computers added."),
        },
        TextRole::Status,
    ));
    add(nodes, snapshot, caps);
    let mut end = Vec::new();
    control(
        &mut end,
        "first-run-continue",
        "Continue",
        Intent::ContinueOnboarding,
        check(snapshot, caps, Action::ContinueFirstRun),
    );
    nodes.push(section("first-run-end", end));
}

fn computers(nodes: &mut Vec<Node<Intent>>, snapshot: &Snapshot, caps: Capabilities, ui: &UiState) {
    nodes.push(text("computers-title", "Computers", TextRole::Heading));
    if snapshot.hosts.is_empty() {
        nodes.push(text(
            "computers-empty",
            "No computers yet. Add one to reach it from this device.",
            TextRole::Body,
        ));
    }
    for (index, host) in snapshot.hosts.iter().enumerate() {
        let prefix = format!("host-{index}");
        let mut row = vec![
            text(format!("{prefix}-label"), &host.label, TextRole::Heading),
            text(
                format!("{prefix}-status"),
                status_line(host, snapshot.now),
                TextRole::Status,
            ),
        ];
        if let Some(listing) = host.listing {
            row.push(text(
                format!("{prefix}-directory"),
                if listing.weight == 0 {
                    "In your directory. Weight 0: not used for new work.".to_owned()
                } else {
                    format!("In your directory. Weight {}.", listing.weight)
                },
                TextRole::Status,
            ));
        }
        if host.listing.is_none() && host.delisted {
            row.push(text(
                format!("{prefix}-directory"),
                "Removed from your directory. This device can still reach it; it gets no new work.",
                TextRole::Status,
            ));
        }
        if let Some(destination) = &host.ssh {
            row.push(text(
                format!("{prefix}-ssh"),
                format!("Set up over SSH on {destination}."),
                TextRole::Status,
            ));
        }
        if let Some(tunnel) = host.tunnel {
            row.push(text(
                format!("{prefix}-tunnel"),
                if tunnel.open {
                    "SSH tunnel open. This device connects through it."
                } else {
                    "SSH tunnel closed. This device uses the relay; the computer keeps running."
                },
                TextRole::Status,
            ));
        }
        let status = HostStatus::derive(host, snapshot.now);
        let mut actions = Vec::new();
        if host.directory_only() {
            control(
                &mut actions,
                format!("{prefix}-access"),
                "Access",
                Intent::Show {
                    screen: Screen::Access {
                        host: host.key.clone(),
                    },
                },
                Ok(()),
            );
            listing_controls(&mut actions, &prefix, host, snapshot, caps);
            row.push(stack(
                format!("{prefix}-actions"),
                Axis::Horizontal,
                Space::Md,
                actions,
            ));
            confirmations(&mut row, &prefix, host, snapshot, caps, ui);
            nodes.push(section(prefix, row));
            continue;
        }
        let enabled = host.link.is_none_or(|link| link.enabled);
        control(
            &mut actions,
            format!("{prefix}-switch"),
            if enabled { "Switch off" } else { "Switch on" },
            Intent::SetEnabled {
                host: host.key.clone(),
                enabled: !enabled,
            },
            check(snapshot, caps, Action::SetEnabled { host: &host.key }),
        );
        if enabled && matches!(status, HostStatus::Offline { .. }) {
            control(
                &mut actions,
                format!("{prefix}-retry"),
                "Try now",
                Intent::RetryNow {
                    host: host.key.clone(),
                },
                check(snapshot, caps, Action::RetryNow { host: &host.key }),
            );
        }
        control(
            &mut actions,
            format!("{prefix}-access"),
            "Access",
            Intent::Show {
                screen: Screen::Access {
                    host: host.key.clone(),
                },
            },
            Ok(()),
        );
        control(
            &mut actions,
            format!("{prefix}-forget"),
            "Forget",
            Intent::Forget {
                host: host.key.clone(),
            },
            check(snapshot, caps, Action::Forget { host: &host.key }),
        );
        if host.listing.is_none() && snapshot.directory != DirectoryState::NoOwnerKey {
            control(
                &mut actions,
                format!("{prefix}-list"),
                "Add to directory",
                Intent::ListInDirectory {
                    host: host.key.clone(),
                },
                check(snapshot, caps, Action::ListInDirectory { host: &host.key }),
            );
        }
        listing_controls(&mut actions, &prefix, host, snapshot, caps);
        // Only a client that can run ssh offers the explicit remove.
        if host.ssh.is_some() && caps.ssh() {
            control(
                &mut actions,
                format!("{prefix}-ssh-remove"),
                "Remove over SSH",
                Intent::RemoveSshHost {
                    host: host.key.clone(),
                },
                check(snapshot, caps, Action::RemoveSsh { host: &host.key }),
            );
        }
        row.push(stack(
            format!("{prefix}-actions"),
            Axis::Horizontal,
            Space::Md,
            actions,
        ));
        confirmations(&mut row, &prefix, host, snapshot, caps, ui);
        if ui.confirm == Some(Confirm::Forget(host.key.clone())) {
            row.push(text(
                format!("{prefix}-forget-confirm"),
                format!(
                    "Forget {}? This device stops connecting and removes it from this list. The computer keeps this device's access until you revoke it.",
                    host.label
                ),
                TextRole::Body,
            ));
            let mut confirm = Vec::new();
            control(
                &mut confirm,
                format!("{prefix}-forget-yes"),
                "Forget computer",
                Intent::ConfirmForget {
                    host: host.key.clone(),
                },
                check(snapshot, caps, Action::Forget { host: &host.key }),
            );
            control(
                &mut confirm,
                format!("{prefix}-forget-no"),
                "Keep",
                Intent::Cancel,
                Ok(()),
            );
            row.push(stack(
                format!("{prefix}-forget-actions"),
                Axis::Horizontal,
                Space::Md,
                confirm,
            ));
        }
        nodes.push(section(prefix, row));
    }
    if let Some(line) = snapshot
        .ssh
        .as_ref()
        .and_then(|attempt| removal_line(&attempt.destination, &attempt.stage))
    {
        nodes.push(text("computers-ssh", line, TextRole::Status));
    }
    let mut end = Vec::new();
    control(
        &mut end,
        "computers-add",
        "Add a computer",
        Intent::Show {
            screen: Screen::Add,
        },
        Ok(()),
    );
    control(
        &mut end,
        "computers-refresh",
        "Refresh",
        Intent::Refresh,
        Ok(()),
    );
    nodes.push(stack("computers-end", Axis::Horizontal, Space::Md, end));
    directory(nodes, snapshot, caps);
}

/// The owner directory's state, and the way to read it on this device.
fn directory(nodes: &mut Vec<Node<Intent>>, snapshot: &Snapshot, caps: Capabilities) {
    let line = match snapshot.directory {
        DirectoryState::NoOwnerKey => {
            "This device lists the computers it was added to. Enter your owner key to see every computer in your directory.".to_owned()
        }
        DirectoryState::Loading => "Reading your directory.".to_owned(),
        DirectoryState::Current { revision: None, .. } => {
            "Your directory is empty. Add a computer to it from its row.".to_owned()
        }
        DirectoryState::Current {
            revision: Some(revision),
            as_of,
        } => format!(
            "Your directory, revision {revision}, read {}.",
            ago(snapshot.now, as_of)
        ),
        DirectoryState::Conflict { revision } => format!(
            "Your directory has two different versions at revision {revision}. This list shows the last version this device trusted."
        ),
        DirectoryState::Failed { revision: Some(revision) } => format!(
            "Couldn't read your directory. This list shows revision {revision}."
        ),
        DirectoryState::Failed { revision: None } => {
            "Couldn't read your directory. Refresh to try again.".to_owned()
        }
    };
    let mut children = vec![
        text("directory-title", "Your directory", TextRole::Heading),
        text("directory-status", line, TextRole::Status),
    ];
    if let DirectoryState::Conflict { revision } = snapshot.directory {
        control(
            &mut children,
            "directory-keep",
            "Keep this device's version",
            Intent::KeepDirectory { revision },
            check(snapshot, caps, Action::KeepDirectory { revision }),
        );
    }
    if snapshot.directory == DirectoryState::NoOwnerKey {
        control(
            &mut children,
            "directory-owner-key",
            "Enter owner key",
            Intent::ImportOwnerKey,
            check(snapshot, caps, Action::ImportOwnerKey),
        );
    }
    nodes.push(section("directory", children));
}

fn add(nodes: &mut Vec<Node<Intent>>, snapshot: &Snapshot, caps: Capabilities) {
    let mut invite = vec![
        text("invite-title", "Use an invitation", TextRole::Heading),
        text(
            "invite-body",
            "On the computer, run `coder host invite`. Scan its QR code, or paste its complete coder-host: string.",
            TextRole::Body,
        ),
    ];
    let mut buttons = Vec::new();
    control(
        &mut buttons,
        "invite-scan",
        "Scan invitation",
        Intent::ScanInvitation,
        check(snapshot, caps, Action::ScanInvitation),
    );
    control(
        &mut buttons,
        "invite-paste",
        "Paste invitation",
        Intent::PasteInvitation,
        check(snapshot, caps, Action::PasteInvitation),
    );
    invite.push(stack(
        "invite-actions",
        Axis::Horizontal,
        Space::Md,
        buttons,
    ));
    nodes.push(section("invite", invite));

    let mut approve = vec![text(
        "approve-title",
        "Approve a computer's code",
        TextRole::Heading,
    )];
    let mut waiting = 0;
    for (index, host) in snapshot.hosts.iter().enumerate() {
        for (request_index, request) in host
            .enrollments
            .iter()
            .filter(|request| request.expires_at > snapshot.now)
            .enumerate()
        {
            waiting += 1;
            let prefix = format!("approve-{index}-{request_index}");
            approve.push(text(
                format!("{prefix}-label"),
                format!(
                    "{} asks to connect. It asks for: {}. {}.",
                    host.label,
                    request
                        .rights
                        .iter()
                        .map(right_label)
                        .collect::<Vec<_>>()
                        .join(", "),
                    capitalize(&until(snapshot.now, request.expires_at))
                ),
                TextRole::Body,
            ));
            let allowed = check(
                snapshot,
                caps,
                Action::Approve {
                    host: &host.key,
                    enrollment: &request.enrollment,
                },
            );
            let mut actions = Vec::new();
            control(
                &mut actions,
                format!("{prefix}-code"),
                "Enter code",
                Intent::EnterCode {
                    host: host.key.clone(),
                    enrollment: request.enrollment.clone(),
                },
                allowed.clone(),
            );
            control(
                &mut actions,
                format!("{prefix}-deny"),
                "Deny",
                Intent::Deny {
                    host: host.key.clone(),
                    enrollment: request.enrollment.clone(),
                },
                allowed,
            );
            approve.push(stack(
                format!("{prefix}-actions"),
                Axis::Horizontal,
                Space::Md,
                actions,
            ));
        }
    }
    if waiting == 0 {
        approve.push(text(
            "approve-empty",
            "No computers are waiting. A computer without a screen you can scan shows an 8-character code; its request appears here.",
            TextRole::Body,
        ));
    }
    nodes.push(section("approve", approve));

    // Only a client that can run ssh offers SSH; a phone never does.
    if caps.ssh() {
        let mut ssh = vec![
            text("ssh-title", "Connect over SSH", TextRole::Heading),
            text(
                "ssh-body",
                "Start or adopt a host on a machine you can reach with ssh. SSH authorizes the setup once; after that, the host's grant decides access.",
                TextRole::Body,
            ),
        ];
        if let Some(attempt) = &snapshot.ssh {
            let destination = &attempt.destination;
            let line = match &attempt.stage {
                SshStage::Starting => format!("Setting up a host on {destination}."),
                SshStage::Prompt { .. } => format!("{destination} asks for an answer."),
                SshStage::Enrolling => format!("Adding this device to the host on {destination}."),
                SshStage::Added { host } => format!(
                    "Added {} over SSH.",
                    snapshot
                        .host(host)
                        .map_or("the computer", |record| record.label.as_str())
                ),
                SshStage::Failed { reason } => {
                    format!("Couldn't set up a host on {destination}: {reason}")
                }
                stage => removal_line(destination, stage).unwrap_or_default(),
            };
            ssh.push(text("ssh-status", line, TextRole::Status));
        }
        control(
            &mut ssh,
            "ssh-connect",
            "Connect over SSH",
            Intent::ConnectSsh,
            check(snapshot, caps, Action::ConnectSsh),
        );
        nodes.push(section("ssh", ssh));
    }

    let mut local = vec![text("local-title", "No local host", TextRole::Heading)];
    local.push(text(
        "local-body",
        match (&snapshot.local_host, caps.platform) {
            (LocalHost::Running { .. }, _) => "A host runs on this computer.",
            (LocalHost::ClientOnly, _) => {
                "This computer runs with no local host and connects to your other computers."
            }
            (_, Platform::Phone) | (LocalHost::NotSupported, _) => {
                "This phone connects to your computers. It never runs a host."
            }
            (LocalHost::Undecided, _) => "Use this computer only to reach your other computers.",
        },
        TextRole::Body,
    ));
    control(
        &mut local,
        "local-client-only",
        "Run with no local host",
        Intent::RunWithoutHost,
        check(snapshot, caps, Action::RunWithoutHost),
    );
    nodes.push(section("local", local));
}

/// The owner's controls for a listed host's directory entry. A device
/// without the owner key shows none.
fn listing_controls(
    actions: &mut Vec<Node<Intent>>,
    prefix: &str,
    host: &HostRecord,
    snapshot: &Snapshot,
    caps: Capabilities,
) {
    if host.listing.is_none() || snapshot.directory == DirectoryState::NoOwnerKey {
        return;
    }
    let revision = snapshot.directory.revision().unwrap_or(0);
    let allowed = check(
        snapshot,
        caps,
        Action::EditListing {
            host: &host.key,
            revision,
        },
    );
    for (key, label, intent) in [
        (
            "rename",
            "Rename",
            Intent::EditLabel {
                host: host.key.clone(),
                revision,
            },
        ),
        (
            "weight",
            "Change weight",
            Intent::EditWeight {
                host: host.key.clone(),
                revision,
            },
        ),
        (
            "delist",
            "Remove from directory",
            Intent::RemoveFromDirectory {
                host: host.key.clone(),
                revision,
            },
        ),
    ] {
        control(
            actions,
            format!("{prefix}-{key}"),
            label,
            intent,
            allowed.clone(),
        );
    }
}

/// The confirmation a row asks for before removing a host from the
/// directory or over SSH.
fn confirmations(
    row: &mut Vec<Node<Intent>>,
    prefix: &str,
    host: &HostRecord,
    snapshot: &Snapshot,
    caps: Capabilities,
    ui: &UiState,
) {
    let (key, question, yes, intent, allowed) = match &ui.confirm {
        Some(Confirm::Delist(key, revision)) if *key == host.key => (
            "delist",
            if host.enrollment.rights(snapshot.now).is_some() {
                format!(
                    "Remove {} from your directory? It gets no new work. This device keeps its access, and the computer keeps running.",
                    host.label
                )
            } else {
                format!(
                    "Remove {} from your directory? This device has no access to it, so it leaves this list.",
                    host.label
                )
            },
            "Remove from directory",
            Intent::ConfirmRemoveFromDirectory {
                host: host.key.clone(),
                revision: *revision,
            },
            check(
                snapshot,
                caps,
                Action::EditListing {
                    host: &host.key,
                    revision: *revision,
                },
            ),
        ),
        Some(Confirm::RemoveSsh(key)) if *key == host.key => (
            "ssh-remove",
            format!(
                "Remove {} from {}? If this app's setup started its host, the host stops. If the host was already running, it keeps running and this device detaches. Then this device forgets the computer.",
                host.label,
                host.ssh.as_deref().unwrap_or("its SSH destination")
            ),
            "Remove computer",
            Intent::ConfirmRemoveSshHost {
                host: host.key.clone(),
            },
            check(snapshot, caps, Action::RemoveSsh { host: &host.key }),
        ),
        _ => return,
    };
    row.push(text(
        format!("{prefix}-{key}-confirm"),
        question,
        TextRole::Body,
    ));
    let mut confirm = Vec::new();
    control(
        &mut confirm,
        format!("{prefix}-{key}-yes"),
        yes,
        intent,
        allowed,
    );
    control(
        &mut confirm,
        format!("{prefix}-{key}-no"),
        "Keep",
        Intent::Cancel,
        Ok(()),
    );
    row.push(stack(
        format!("{prefix}-{key}-actions"),
        Axis::Horizontal,
        Space::Md,
        confirm,
    ));
}

/// The line for an explicit SSH remove, or `None` for a setup stage.
fn removal_line(destination: &str, stage: &SshStage) -> Option<String> {
    Some(match stage {
        SshStage::Removing { label } => format!("Removing {label} from {destination}."),
        SshStage::Removed { label, removal } => match removal {
            SshRemoval::Stopped => {
                format!(
                    "Removed {label}. Its host on {destination} stopped, and this device forgot it."
                )
            }
            SshRemoval::Detached => format!(
                "Removed {label}. Its host on {destination} was already running before setup, so it keeps running; this device detached and forgot it."
            ),
            SshRemoval::Absent => format!(
                "Removed {label}. No host was running on {destination}; this device forgot it."
            ),
        },
        SshStage::RemoveFailed { label, reason } => {
            format!("Couldn't remove {label} from {destination}: {reason} It stays in your list.")
        }
        SshStage::Starting
        | SshStage::Prompt { .. }
        | SshStage::Enrolling
        | SshStage::Added { .. }
        | SshStage::Failed { .. } => return None,
    })
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or(String::new(), |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn device_line(row: &DeviceRow, snapshot: &Snapshot) -> String {
    let who = match (&row.label, row.device == snapshot.device) {
        (_, true) => "This device".to_owned(),
        (Some(label), false) => label.clone(),
        (None, false) => format!("Device {}", short_key(&row.device)),
    };
    let state = match row.state {
        DeviceState::Active => "Active",
        DeviceState::Revoked => "Revoked",
        DeviceState::Expired => "Expired",
    };
    let origin = match row.origin {
        OriginKind::Invitation => "joined by invitation",
        OriginKind::Approval => "joined by approval",
    };
    let seen = row.last_seen.map_or("last seen: unknown".to_owned(), |at| {
        format!("last seen {}", ago(snapshot.now, at))
    });
    format!("{who}. {state}, {origin}, {seen}.")
}

fn access(
    nodes: &mut Vec<Node<Intent>>,
    snapshot: &Snapshot,
    caps: Capabilities,
    ui: &UiState,
    host: &HostRecord,
) {
    nodes.push(text(
        "access-title",
        format!("Access: {}", host.label),
        TextRole::Heading,
    ));
    nodes.push(text(
        "access-status",
        status_line(host, snapshot.now),
        TextRole::Status,
    ));
    match host.enrollment.rights(snapshot.now) {
        Some(rights) => nodes.push(text(
            "access-mine",
            format!(
                "This device can: {}.",
                rights
                    .iter()
                    .map(right_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TextRole::Body,
        )),
        None => nodes.push(text(
            "access-mine",
            "This device holds no current access to this computer.",
            TextRole::Body,
        )),
    }

    let mut devices = vec![text("devices-title", "Devices", TextRole::Heading)];
    let read = check(snapshot, caps, Action::ReadDevices { host: &host.key });
    match (&host.devices, &read) {
        (
            DeviceList::Loaded {
                devices: rows,
                as_of,
            },
            _,
        ) => {
            devices.push(text(
                "devices-as-of",
                format!(
                    "{} devices. Checked {}.",
                    rows.len(),
                    ago(snapshot.now, *as_of)
                ),
                TextRole::Status,
            ));
            for (index, row) in rows.iter().enumerate() {
                let prefix = format!("device-{index}");
                let mut entry = vec![
                    text(
                        format!("{prefix}-label"),
                        device_line(row, snapshot),
                        TextRole::Body,
                    ),
                    text(
                        format!("{prefix}-rights"),
                        format!(
                            "Can: {}.",
                            row.rights
                                .iter()
                                .map(right_label)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        TextRole::Status,
                    ),
                ];
                if row.state == DeviceState::Active {
                    let allowed = check(
                        snapshot,
                        caps,
                        Action::Revoke {
                            host: &host.key,
                            device: &row.device,
                        },
                    );
                    if ui.confirm == Some(Confirm::Revoke(host.key.clone(), row.device.clone())) {
                        entry.push(text(
                            format!("{prefix}-revoke-confirm"),
                            "Revoke this device? It loses access to this computer at once. Adding it again needs a new invitation.",
                            TextRole::Body,
                        ));
                        let mut confirm = Vec::new();
                        control(
                            &mut confirm,
                            format!("{prefix}-revoke-yes"),
                            "Revoke access",
                            Intent::ConfirmRevoke {
                                host: host.key.clone(),
                                device: row.device.clone(),
                            },
                            allowed,
                        );
                        control(
                            &mut confirm,
                            format!("{prefix}-revoke-no"),
                            "Keep",
                            Intent::Cancel,
                            Ok(()),
                        );
                        entry.push(stack(
                            format!("{prefix}-revoke-actions"),
                            Axis::Horizontal,
                            Space::Md,
                            confirm,
                        ));
                    } else {
                        control(
                            &mut entry,
                            format!("{prefix}-revoke"),
                            "Revoke",
                            Intent::Revoke {
                                host: host.key.clone(),
                                device: row.device.clone(),
                            },
                            allowed,
                        );
                    }
                }
                devices.push(section(prefix, entry));
            }
        }
        (DeviceList::NotLoaded, Ok(())) => devices.push(text(
            "devices-empty",
            "Refresh to load the devices that can reach this computer.",
            TextRole::Body,
        )),
        (DeviceList::NotLoaded, Err(_)) => {}
    }
    control(
        &mut devices,
        "devices-refresh",
        "Refresh devices",
        Intent::RefreshDevices {
            host: host.key.clone(),
        },
        read,
    );
    nodes.push(section("devices", devices));

    let mut invite = vec![
        text("share-title", "Invite a device", TextRole::Heading),
        text(
            "share-body",
            "Choose what the new device may do. An invitation works once, for one device, and expires soon.",
            TextRole::Body,
        ),
    ];
    let draft = ui.draft(host, snapshot.now);
    for right in Right::ALL {
        let included = draft.contains(&right);
        control(
            &mut invite,
            format!("share-right-{}", right.as_str()),
            format!(
                "{}: {}",
                right_label(right),
                if included { "included" } else { "not included" }
            ),
            Intent::ToggleRight {
                host: host.key.clone(),
                right,
            },
            check(
                snapshot,
                caps,
                Action::IncludeRight {
                    host: &host.key,
                    right,
                },
            ),
        );
    }
    let chosen = coder_access::Rights::new(draft.iter().copied()).ok();
    control(
        &mut invite,
        "share-create",
        "Create invitation",
        Intent::CreateInvitation {
            host: host.key.clone(),
        },
        check(
            snapshot,
            caps,
            Action::Invite {
                host: &host.key,
                rights: chosen.as_ref(),
            },
        ),
    );
    if let Some(created) = ui.invitations.get(&host.key) {
        invite.push(text("share-code-title", "Invitation", TextRole::Heading));
        invite.push(text("share-code", &created.code, TextRole::Code));
        // A phone draws the same locally rendered code natively beside the
        // tree; a terminal or desktop adapter draws it here as text.
        match (caps.platform, crate::qr::modules(&created.code)) {
            (Platform::Phone, Some(_)) => invite.push(text(
                "share-qr-hint",
                "The new device can scan the QR code below instead.",
                TextRole::Status,
            )),
            (_, Some(modules)) => {
                invite.push(text("share-qr", crate::qr::text(&modules), TextRole::Code));
            }
            (_, None) => {}
        }
        invite.push(text(
            "share-code-detail",
            format!(
                "Single use. {}. Grants: {}. Show it only to the device you're adding; anyone with this string can use it once.",
                capitalize(&until(snapshot.now, created.expires_at)),
                created.rights.iter().map(right_label).collect::<Vec<_>>().join(", ")
            ),
            TextRole::Status,
        ));
        let mut actions = Vec::new();
        control(
            &mut actions,
            "share-cancel",
            "Cancel invitation",
            Intent::CancelInvitation {
                host: host.key.clone(),
                invitation: created.invitation.clone(),
            },
            check(
                snapshot,
                caps,
                Action::Invite {
                    host: &host.key,
                    rights: Some(&created.rights),
                },
            ),
        );
        control(
            &mut actions,
            "share-done",
            "Done",
            Intent::DismissInvitation {
                host: host.key.clone(),
            },
            Ok(()),
        );
        invite.push(stack(
            "share-code-actions",
            Axis::Horizontal,
            Space::Md,
            actions,
        ));
    }
    nodes.push(section("share", invite));
}

/// The newest summary per subject, attention first, newest first.
pub(crate) fn current_activity(snapshot: &Snapshot) -> Vec<&ActivitySummary> {
    let mut newest: Vec<&ActivitySummary> = Vec::new();
    for summary in &snapshot.activity {
        match newest.iter_mut().find(|held| {
            held.host == summary.host
                && held.subject == summary.subject
                && held.subject_kind == summary.subject_kind
        }) {
            Some(held) => {
                // A conflicting body for one sequence keeps the held one.
                if nostr::activity_summary::supersedes(held, summary).unwrap_or(false) {
                    *held = summary;
                }
            }
            None => newest.push(summary),
        }
    }
    newest.sort_by(|a, b| {
        (a.attention == Attention::None)
            .cmp(&(b.attention == Attention::None))
            .then(b.updated_at.cmp(&a.updated_at))
            .then(a.subject.cmp(&b.subject))
    });
    newest
}

fn activity(nodes: &mut Vec<Node<Intent>>, snapshot: &Snapshot) {
    nodes.push(text("activity-title", "Activity", TextRole::Heading));
    let summaries = current_activity(snapshot);
    let attention = summaries
        .iter()
        .filter(|summary| summary.attention != Attention::None)
        .count();
    nodes.push(text(
        "activity-count",
        match attention {
            0 => "Nothing needs your attention.".to_owned(),
            1 => "1 item needs your attention.".to_owned(),
            n => format!("{n} items need your attention."),
        },
        TextRole::Status,
    ));
    for (index, summary) in summaries.iter().take(64).enumerate() {
        let prefix = format!("activity-{index}");
        let host = snapshot.host(&summary.host);
        let label = host.map_or_else(
            || format!("Computer {}", short_key(&summary.host)),
            |h| h.label.clone(),
        );
        let kind = match summary.subject_kind {
            SubjectKind::Task => "Task",
            SubjectKind::Session => "Session",
        };
        let phase = match summary.phase {
            Phase::Queued => "queued",
            Phase::Running => "running",
            Phase::Waiting => "waiting",
            Phase::Completed => "completed",
            Phase::Failed => "failed",
            Phase::Cancelled => "cancelled",
            Phase::Unknown => "state unknown",
        };
        let reason = match summary.attention {
            Attention::None => None,
            Attention::Approval => Some("Needs your approval."),
            Attention::Input => Some("Needs your input."),
            Attention::Completed => Some("Finished."),
            Attention::Failed => Some("Failed."),
        };
        let mut row = vec![
            text(
                format!("{prefix}-subject"),
                format!("{label}. {kind}, {phase}."),
                TextRole::Body,
            ),
            // The headline is host-redacted data. Show it literally.
            text(
                format!("{prefix}-headline"),
                &summary.headline,
                TextRole::Body,
            ),
            text(
                format!("{prefix}-time"),
                format!(
                    "{}Updated {}.",
                    reason.map_or(String::new(), |r| format!("{r} ")),
                    ago(snapshot.now, summary.updated_at)
                ),
                TextRole::Status,
            ),
        ];
        if host.is_some_and(|h| !HostStatus::derive(h, snapshot.now).online()) {
            row.push(text(
                format!("{prefix}-stale"),
                format!("From the last summary. {label} isn't online now."),
                TextRole::Status,
            ));
        }
        nodes.push(section(prefix, row));
    }
    let mut end = Vec::new();
    control(
        &mut end,
        "activity-refresh",
        "Refresh",
        Intent::Refresh,
        Ok(()),
    );
    nodes.push(stack("activity-end", Axis::Horizontal, Space::Md, end));
}
