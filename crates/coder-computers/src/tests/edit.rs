//! Owner edits to the directory, conflict settlement, removed hosts in
//! placement, SSH removal, and the SSH tunnel route, as the screens project
//! and check them.
use super::*;

fn studio() -> String {
    "a1".repeat(32)
}

fn laptop() -> String {
    "a2".repeat(32)
}

fn build() -> String {
    "a3".repeat(32)
}

fn owner_screens(snapshot: Snapshot, platform: Platform) -> (Fixed, Computers) {
    let fixed = Fixed::new(snapshot);
    let computers = Computers::new(Box::new(fixed.clone()), caps(platform), "c:edit").unwrap();
    (fixed, computers)
}

fn pressed_kinds() -> std::collections::BTreeSet<String> {
    PRESSED.with(|pressed| pressed.borrow().clone())
}

#[test]
fn the_owner_relabels_reweighs_and_removes_listed_hosts() {
    let (fixed, mut computers) = owner_screens(directory_snapshot(), Platform::Desktop);
    // Listed rows offer the owner's edits; an unlisted row offers none.
    for row in ["host-0", "host-2"] {
        for control in ["rename", "weight", "delist"] {
            assert!(enabled(&computers, &format!("{row}-{control}")), "{row}");
        }
    }
    for control in ["rename", "weight", "delist"] {
        assert!(find(&computers, &format!("host-1-{control}")).is_none());
    }

    // Rename: the label follows the directory's bounds.
    press(&mut computers, "host-0-rename").unwrap();
    let asked = computers.input().unwrap().clone();
    assert_eq!(asked.purpose, InputPurpose::DirectoryLabel);
    assert!(!asked.secret);
    for bad in ["", "   ", &"x".repeat(65), "two\nlines"] {
        assert!(
            matches!(computers.submit(&asked.token, bad), Err(Refusal::Input(_))),
            "{bad:?}"
        );
    }
    computers.submit(&asked.token, "  Studio Mac ").unwrap();
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("edit {} 2 Label(\"Studio Mac\")", studio())
    );
    assert_eq!(
        computers.notice().unwrap().text,
        "Renamed the computer to Studio Mac in your directory."
    );

    // Change weight: a whole number from 0 to 1000.
    press(&mut computers, "host-0-weight").unwrap();
    let asked = computers.input().unwrap().clone();
    assert_eq!(asked.purpose, InputPurpose::DirectoryWeight);
    assert!(!asked.secret);
    for bad in ["1001", "-1", "abc", "2.5", ""] {
        assert!(
            matches!(computers.submit(&asked.token, bad), Err(Refusal::Input(_))),
            "{bad:?}"
        );
    }
    computers.submit(&asked.token, "0").unwrap();
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("edit {} 2 Weight(0)", studio())
    );
    assert!(
        computers
            .notice()
            .unwrap()
            .text
            .ends_with("stays in your directory with weight 0. It gets no new work.")
    );
    press(&mut computers, "host-0-weight").unwrap();
    let asked = computers.input().unwrap().clone();
    computers.submit(&asked.token, "1000").unwrap();
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("edit {} 2 Weight(1000)", studio())
    );

    // Remove asks first. An enrolled host keeps this device's access; a
    // host this device has no grant for leaves the list.
    press(&mut computers, "host-0-delist").unwrap();
    assert!(text_of(&computers, "host-0-delist-confirm").contains("keeps its access"));
    press(&mut computers, "host-0-delist-no").unwrap();
    assert!(find(&computers, "host-0-delist-confirm").is_none());
    press(&mut computers, "host-2-delist").unwrap();
    assert!(text_of(&computers, "host-2-delist-confirm").contains("it leaves this list"));
    let calls = fixed.calls().len();
    press(&mut computers, "host-2-delist-yes").unwrap();
    assert_eq!(fixed.calls().len(), calls + 1);
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("delist {} 2", build())
    );
    assert_eq!(
        computers.notice().unwrap().text,
        "Removed Build box from your directory. It no longer gets new work."
    );
    assert!(find(&computers, "host-2-delist-confirm").is_none());

    let pressed = pressed_kinds();
    for kind in [
        "edit_label",
        "edit_weight",
        "remove_from_directory",
        "confirm_remove_from_directory",
    ] {
        assert!(pressed.contains(kind), "{kind} was never pressed");
    }
}

#[test]
fn owner_edits_need_the_owner_key_and_a_current_read() {
    let key = studio();
    let edit = |state: DirectoryState, revision: u64| {
        let mut snapshot = directory_snapshot();
        snapshot.directory = state;
        check(
            &snapshot,
            caps(Platform::Desktop),
            Action::EditListing {
                host: &key,
                revision,
            },
        )
    };
    let current = |revision| DirectoryState::Current {
        revision: Some(revision),
        as_of: NOW,
    };
    assert_eq!(edit(current(2), 2), Ok(()));
    assert_eq!(edit(DirectoryState::NoOwnerKey, 2), Err(Denial::NotOwner));
    assert_eq!(
        edit(DirectoryState::Loading, 2),
        Err(Denial::DirectoryNotRead)
    );
    assert_eq!(
        edit(DirectoryState::Failed { revision: Some(2) }, 2),
        Err(Denial::DirectoryNotRead)
    );
    assert_eq!(
        edit(DirectoryState::Conflict { revision: 2 }, 2),
        Err(Denial::DirectoryConflict)
    );
    // An edit made against another revision is stale, older or newer.
    assert_eq!(edit(current(3), 2), Err(Denial::StaleDirectory));
    assert_eq!(edit(current(1), 2), Err(Denial::StaleDirectory));
    // Only a listed host has an entry to edit.
    assert_eq!(
        check(
            &directory_snapshot(),
            caps(Platform::Desktop),
            Action::EditListing {
                host: &laptop(),
                revision: 2
            }
        ),
        Err(Denial::NotListed)
    );
    assert_eq!(
        check(
            &directory_snapshot(),
            caps(Platform::Desktop),
            Action::EditListing {
                host: &"ff".repeat(32),
                revision: 2
            }
        ),
        Err(Denial::UnknownHost)
    );

    // Without the owner key the rows show no owner controls. A phone that
    // reads the directory with its device key edits it like any client.
    let mut snapshot = directory_snapshot();
    snapshot.directory = DirectoryState::NoOwnerKey;
    let (_, computers) = owner_screens(snapshot, Platform::Phone);
    for control in ["rename", "weight", "delist"] {
        assert!(find(&computers, &format!("host-0-{control}")).is_none());
    }
    let (_, computers) = owner_screens(directory_snapshot(), Platform::Phone);
    assert!(enabled(&computers, "host-0-rename"));

    // A conflict or a failed read disables the controls with a reason.
    let mut snapshot = directory_snapshot();
    snapshot.directory = DirectoryState::Conflict { revision: 2 };
    let (fixed, mut computers) = owner_screens(snapshot, Platform::Desktop);
    assert!(!enabled(&computers, "host-0-rename"));
    assert!(text_of(&computers, "host-0-rename-reason").contains("two different versions"));
    assert_eq!(
        press(&mut computers, "host-0-weight"),
        Err(Refusal::Disabled)
    );
    let mut snapshot = directory_snapshot();
    snapshot.directory = DirectoryState::Failed { revision: Some(2) };
    fixed.0.lock().unwrap().snapshot = snapshot;
    computers.refresh().unwrap();
    assert!(text_of(&computers, "host-0-delist-reason").contains("hasn't been read"));
    assert!(fixed.calls().is_empty());
}

#[test]
fn an_edit_against_a_stale_revision_never_reaches_the_service() {
    let (fixed, mut computers) = owner_screens(directory_snapshot(), Platform::Desktop);
    // The rename was asked at revision 2; revision 3 arrives before the
    // person submits.
    press(&mut computers, "host-0-rename").unwrap();
    let asked = computers.input().unwrap().clone();
    let newer = |fixed: &Fixed| {
        fixed.0.lock().unwrap().snapshot.directory = DirectoryState::Current {
            revision: Some(3),
            as_of: NOW,
        };
    };
    newer(&fixed);
    computers.refresh().unwrap();
    assert_eq!(
        computers.submit(&asked.token, "Studio Mac"),
        Err(Refusal::Denied(Denial::StaleDirectory))
    );
    assert_eq!(
        computers.notice().unwrap().text,
        "Your directory changed since this screen was drawn. Check it, then try again."
    );
    assert!(fixed.calls().is_empty());
    // The redrawn row binds its controls to revision 3.
    press(&mut computers, "input-cancel").unwrap();
    press(&mut computers, "host-0-weight").unwrap();
    let asked = computers.input().unwrap().clone();
    computers.submit(&asked.token, "250").unwrap();
    assert_eq!(
        fixed.calls().last().unwrap(),
        &format!("edit {} 3 Weight(250)", studio())
    );

    // A removal confirmed after the directory moved on is stale too.
    press(&mut computers, "host-0-delist").unwrap();
    fixed.0.lock().unwrap().snapshot.directory = DirectoryState::Current {
        revision: Some(4),
        as_of: NOW,
    };
    computers.refresh().unwrap();
    assert!(!enabled(&computers, "host-0-delist-yes"));
    assert!(text_of(&computers, "host-0-delist-yes-reason").contains("changed since"));
    let calls = fixed.calls().len();
    assert_eq!(
        press(&mut computers, "host-0-delist-yes"),
        Err(Refusal::Disabled)
    );
    assert_eq!(fixed.calls().len(), calls);

    // The service's own stale refusal is shown too.
    fixed.0.lock().unwrap().fail = Some(Error::new(Code::Stale, "moved"));
    press(&mut computers, "host-0-weight").unwrap();
    let asked = computers.input().unwrap().clone();
    assert!(matches!(
        computers.submit(&asked.token, "5"),
        Err(Refusal::Failed(ref error)) if error.code == Code::Stale
    ));
    assert_eq!(
        computers.notice().unwrap().text,
        "The computer's records changed. Refresh and try again."
    );
}

#[test]
fn a_conflict_stays_visible_until_the_owner_keeps_a_version() {
    let mut snapshot = directory_snapshot();
    snapshot.directory = DirectoryState::Conflict { revision: 2 };
    let (fixed, mut computers) = owner_screens(snapshot, Platform::Desktop);
    assert!(
        text_of(&computers, "directory-status").contains("two different versions at revision 2")
    );
    assert!(enabled(&computers, "directory-keep"));
    press(&mut computers, "directory-keep").unwrap();
    assert_eq!(fixed.calls(), vec!["keep 2".to_owned()]);
    assert_eq!(
        computers.notice().unwrap().text,
        "Published this device's version of your directory as revision 3."
    );
    assert!(pressed_kinds().contains("keep_directory"));

    let keep = |state: DirectoryState, revision: u64| {
        let mut snapshot = directory_snapshot();
        snapshot.directory = state;
        check(
            &snapshot,
            caps(Platform::Desktop),
            Action::KeepDirectory { revision },
        )
    };
    assert_eq!(keep(DirectoryState::Conflict { revision: 2 }, 2), Ok(()));
    assert_eq!(
        keep(DirectoryState::Conflict { revision: 3 }, 2),
        Err(Denial::StaleDirectory)
    );
    assert_eq!(keep(DirectoryState::NoOwnerKey, 2), Err(Denial::NotOwner));
    assert_eq!(
        keep(
            DirectoryState::Current {
                revision: Some(2),
                as_of: NOW
            },
            2
        ),
        Err(Denial::NoConflict)
    );
    // No conflict, no control.
    let (_, computers) = owner_screens(directory_snapshot(), Platform::Desktop);
    assert!(find(&computers, "directory-keep").is_none());
}

#[test]
fn a_removed_host_stays_reachable_and_leaves_placement() {
    let client = ClientProfile {
        protocol: coder_reach::PROTOCOL_VERSION,
        accepts: VersionRange {
            min: coder_reach::PROTOCOL_VERSION,
            max: coder_reach::PROTOCOL_VERSION,
        },
    };
    let mut snapshot = directory_snapshot();
    // The owner removed Studio; this device still holds its grant.
    snapshot.hosts[0].listing = None;
    snapshot.hosts[0].delisted = true;
    assert_eq!(snapshot.hosts[0].weight(), 0);
    assert_eq!(snapshot.place(&client), Some(laptop().as_str()));
    let studio_key = studio();
    assert!(snapshot.assess_placement(&client).contains(
        &coder_reach::placement::Assessment::Skipped {
            host: &studio_key,
            reason: coder_reach::placement::Skip::ZeroWeight,
        }
    ));
    let (_, computers) = owner_screens(snapshot.clone(), Platform::Desktop);
    assert!(text_of(&computers, "host-0-status").starts_with("Online"));
    assert_eq!(
        text_of(&computers, "host-0-directory"),
        "Removed from your directory. This device can still reach it; it gets no new work."
    );
    // It keeps its connection controls and can be added back.
    for control in ["switch", "access", "forget", "list"] {
        assert!(
            enabled(&computers, &format!("host-0-{control}")),
            "{control}"
        );
    }
    for control in ["rename", "weight", "delist"] {
        assert!(find(&computers, &format!("host-0-{control}")).is_none());
    }
    // A host the directory never listed keeps the local weight.
    assert_eq!(snapshot.hosts[1].weight(), LOCAL_WEIGHT);
}

fn ssh_snapshot(tunnel: Option<Tunnel>, route: Option<Class>) -> Snapshot {
    let mut snapshot = Synthetic::empty(Platform::Terminal, now)
        .snapshot()
        .unwrap();
    snapshot.first_run_complete = true;
    snapshot.hosts = vec![HostRecord {
        label: "devbox".into(),
        ssh: Some("me@devbox".into()),
        tunnel,
        route,
        ..host(
            enrolled(Rights::all()),
            Some(Status {
                freshness: Freshness::Current { as_of: Moment(1) },
                ..link(Phase::Connected)
            }),
        )
    }];
    snapshot
}

#[test]
fn the_ssh_tunnel_route_shows_when_used_and_when_closed() {
    let (_, computers) = owner_screens(
        ssh_snapshot(
            Some(Tunnel {
                open: true,
                in_use: true,
            }),
            Some(Class::Loopback),
        ),
        Platform::Terminal,
    );
    assert_eq!(
        text_of(&computers, "host-0-status"),
        "Online through the SSH tunnel. Up to date."
    );
    assert_eq!(
        text_of(&computers, "host-0-tunnel"),
        "SSH tunnel open. This device connects through it."
    );
    // The tunnel closed: the relay carries the connection, and the host
    // keeps running.
    let (_, computers) = owner_screens(
        ssh_snapshot(
            Some(Tunnel {
                open: false,
                in_use: false,
            }),
            Some(Class::Relay),
        ),
        Platform::Terminal,
    );
    assert_eq!(
        text_of(&computers, "host-0-status"),
        "Online through a relay. Up to date."
    );
    assert_eq!(
        text_of(&computers, "host-0-tunnel"),
        "SSH tunnel closed. This device uses the relay; the computer keeps running."
    );
    // No tunnel from this process: no tunnel line.
    let (_, computers) = owner_screens(ssh_snapshot(None, Some(Class::Relay)), Platform::Terminal);
    assert!(find(&computers, "host-0-tunnel").is_none());
}

#[test]
fn remove_over_ssh_asks_first_and_shows_the_outcome() {
    let key = "ab".repeat(32);
    let (fixed, mut computers) = owner_screens(ssh_snapshot(None, None), Platform::Terminal);
    assert!(enabled(&computers, "host-0-ssh-remove"));
    press(&mut computers, "host-0-ssh-remove").unwrap();
    let question = text_of(&computers, "host-0-ssh-remove-confirm");
    assert!(question.contains("me@devbox"), "{question}");
    assert!(question.contains("keeps running and this device detaches"));
    press(&mut computers, "host-0-ssh-remove-no").unwrap();
    assert!(fixed.calls().is_empty());
    press(&mut computers, "host-0-ssh-remove").unwrap();
    press(&mut computers, "host-0-ssh-remove-yes").unwrap();
    assert_eq!(fixed.calls(), vec![format!("ssh_remove {key}")]);
    assert_eq!(
        computers.notice().unwrap().text,
        "Removing devbox over SSH."
    );
    let pressed = pressed_kinds();
    assert!(pressed.contains("remove_ssh_host"));
    assert!(pressed.contains("confirm_remove_ssh_host"));

    let set = |fixed: &Fixed, stage: SshStage| {
        fixed.0.lock().unwrap().snapshot.ssh = Some(SshAttempt {
            destination: "me@devbox".into(),
            stage,
        });
    };
    set(
        &fixed,
        SshStage::Removing {
            label: "devbox".into(),
        },
    );
    computers.refresh().unwrap();
    assert_eq!(
        text_of(&computers, "computers-ssh"),
        "Removing devbox from me@devbox."
    );
    // One SSH run at a time.
    assert!(!enabled(&computers, "host-0-ssh-remove"));
    assert!(text_of(&computers, "host-0-ssh-remove-reason").contains("already running"));
    // A prompt during the remove is a masked input request.
    set(
        &fixed,
        SshStage::Prompt {
            id: 3,
            text: "me@devbox's password:".into(),
        },
    );
    computers.refresh().unwrap();
    assert!(computers.input().unwrap().secret);
    set(
        &fixed,
        SshStage::Removing {
            label: "devbox".into(),
        },
    );
    computers.refresh().unwrap();
    assert!(computers.input().is_none());

    for (removal, expected) in [
        (
            SshRemoval::Stopped,
            "Removed devbox. Its host on me@devbox stopped, and this device forgot it.",
        ),
        (
            SshRemoval::Detached,
            "Removed devbox. Its host on me@devbox was already running before setup, so it keeps running; this device detached and forgot it.",
        ),
        (
            SshRemoval::Absent,
            "Removed devbox. No host was running on me@devbox; this device forgot it.",
        ),
    ] {
        let mut state = fixed.0.lock().unwrap();
        state.snapshot.hosts.clear();
        state.snapshot.ssh = Some(SshAttempt {
            destination: "me@devbox".into(),
            stage: SshStage::Removed {
                label: "devbox".into(),
                removal,
            },
        });
        drop(state);
        computers.refresh().unwrap();
        assert_eq!(text_of(&computers, "computers-ssh"), expected);
        // The Add screen's SSH line says the same.
        press(&mut computers, "tab-add").unwrap();
        assert_eq!(text_of(&computers, "ssh-status"), expected);
        assert!(enabled(&computers, "ssh-connect"));
        press(&mut computers, "tab-computers").unwrap();
    }
    set(
        &fixed,
        SshStage::RemoveFailed {
            label: "devbox".into(),
            reason: "ssh couldn't connect or sign in.".into(),
        },
    );
    computers.refresh().unwrap();
    assert_eq!(
        text_of(&computers, "computers-ssh"),
        "Couldn't remove devbox from me@devbox: ssh couldn't connect or sign in. It stays in your list."
    );
    // A setup stage never shows on the Computers screen.
    set(&fixed, SshStage::Starting);
    computers.refresh().unwrap();
    assert!(find(&computers, "computers-ssh").is_none());
}

#[test]
fn remove_over_ssh_follows_the_platform_and_the_setup() {
    let key = "ab".repeat(32);
    let remove = |snapshot: &Snapshot, platform: Platform| {
        check(snapshot, caps(platform), Action::RemoveSsh { host: &key })
    };
    let snapshot = ssh_snapshot(None, None);
    assert_eq!(remove(&snapshot, Platform::Terminal), Ok(()));
    assert_eq!(remove(&snapshot, Platform::Desktop), Ok(()));
    assert_eq!(
        remove(&snapshot, Platform::Phone),
        Err(Denial::SshNeedsComputer)
    );
    let (_, phone) = owner_screens(snapshot.clone(), Platform::Phone);
    assert!(find(&phone, "host-0-ssh-remove").is_none());
    let mut plain = snapshot.clone();
    plain.hosts[0].ssh = None;
    assert_eq!(
        remove(&plain, Platform::Terminal),
        Err(Denial::NotSetUpOverSsh)
    );
    let (_, computers) = owner_screens(plain, Platform::Terminal);
    assert!(find(&computers, "host-0-ssh-remove").is_none());
    let mut unready = snapshot.clone();
    unready.ssh_ready = false;
    assert_eq!(
        remove(&unready, Platform::Terminal),
        Err(Denial::SshNotSetUp)
    );
    // A revoked or expired host can still be removed from its machine.
    let mut revoked = snapshot;
    revoked.hosts[0].enrollment = Enrollment::Revoked;
    assert_eq!(remove(&revoked, Platform::Terminal), Ok(()));
    // SSH setup and remove exclude each other.
    let mut busy = ssh_snapshot(None, None);
    busy.ssh = Some(SshAttempt {
        destination: "me@devbox".into(),
        stage: SshStage::Removing {
            label: "devbox".into(),
        },
    });
    assert_eq!(
        check(&busy, caps(Platform::Terminal), Action::ConnectSsh),
        Err(Denial::SshRunning)
    );
}

#[test]
fn a_service_without_directory_or_ssh_support_refuses_the_new_effects() {
    let mut service = Unavailable::new(device(), LocalHost::Undecided, now);
    for result in [
        service.edit_listing("ab", 1, &ListingChange::Weight(5)),
        service.remove_from_directory("ab", 1),
        service.keep_directory(1),
        service.remove_ssh("ab"),
    ] {
        assert_eq!(result.unwrap_err().code, Code::Unavailable);
    }
}
