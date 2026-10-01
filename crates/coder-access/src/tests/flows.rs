use super::*;
use crate::client::{finish_redeem, pending_enrollments, prepare_redeem};

#[tokio::test]
async fn redeem_then_same_device_retry_after_restart_and_other_device_refused() {
    let mut f = Fixture::served(0, false).await;
    let code = f.invite("standard");
    let phone = key();
    let access = client::redeem(&code, &phone, POLICY).await.unwrap();
    assert_eq!(f.next_handled().await.map(|_| ()), Ok(()));
    assert_eq!(access.grant.rights, Rights::standard());
    assert_eq!(access.grant.owner, pubkey(&f.owner));
    assert_eq!(access.grant.epoch, 0);
    assert!(access.verify(&phone, now(), POLICY).is_ok());

    // Every request opens a new Host over the private store: a restart.
    let again = client::redeem(&code, &phone, POLICY).await.unwrap();
    assert_eq!(again.grant.grant, access.grant.grant);
    assert_eq!(again.grant.expires_at, access.grant.expires_at);

    let other = client::redeem(&code, &key(), POLICY).await.unwrap_err();
    assert_eq!(other.code, Code::Forbidden);
    assert_eq!(f.host().devices(now()).unwrap().len(), 1);
}

#[tokio::test]
async fn expired_and_cancelled_invitations_refuse() {
    // The host clock runs ahead of the invitation's five-minute window.
    let mut f = Fixture::served(INVITATION_LIFETIME + 1, false).await;
    let phone = key();
    let invitation = HostInvitation::parse(&f.invite("standard"), now(), POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &phone, now(), POLICY).unwrap();
    // An expired request earns no signed reply at all.
    let lost = exchange(&f.relay, &phone, &pending, &f.host_key).await;
    assert_eq!(lost.unwrap_err().code, Code::Transport);
    assert_eq!(f.next_handled().await, Err(Code::Expired));
    assert_eq!(f.host().devices(now()).unwrap().len(), 0);

    let f = Fixture::served(0, false).await;
    let issued = f
        .host()
        .invite(&f.relay, Rights::standard(), now(), now() + 3600)
        .unwrap();
    f.host().cancel_invitation(&issued.id).unwrap();
    let refused = client::redeem(&issued.code, &phone, POLICY)
        .await
        .unwrap_err();
    assert_eq!(refused.code, Code::Revoked);
    assert_eq!(f.host().devices(now()).unwrap().len(), 0);
}

#[tokio::test]
async fn crash_between_consumption_and_reply_returns_the_retained_reply() {
    let mut f = Fixture::served(0, true).await;
    let phone = key();
    let invitation = HostInvitation::parse(&f.invite("standard"), now(), POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &phone, now(), POLICY).unwrap();
    // The host commits consumption, grant, and reply, then loses the reply.
    assert_eq!(
        exchange(&f.relay, &phone, &pending, &f.host_key)
            .await
            .unwrap_err()
            .code,
        Code::Transport
    );
    let committed = f.next_handled().await.unwrap();
    assert_eq!(f.host().devices(now()).unwrap().len(), 1);
    // The exact retry reaches a new Host and receives the same signed bytes.
    let reply = exchange(&f.relay, &phone, &pending, &f.host_key)
        .await
        .unwrap();
    assert_eq!(reply.id, committed);
    let access = finish_redeem(&invitation, &pending, &reply, &phone, now(), POLICY).unwrap();
    assert_eq!(
        f.host().devices(now()).unwrap()[0].grant,
        access.grant.grant
    );
    // A new request ID from the same device also recovers the same grant.
    let code = invitation.0.encode_prefixed(INVITATION_PREFIX).unwrap();
    let again = client::redeem(&code, &phone, POLICY).await.unwrap();
    assert_eq!(again.grant.grant, access.grant.grant);
}

#[tokio::test]
async fn observe_only_device_cannot_create_a_task_or_open_a_terminal() {
    let f = Fixture::served(0, false).await;
    let (_, observer) = f.enroll("observe").await;
    for (op, right) in [
        (task(), Right::Operate),
        (
            Operation::SteerTask {
                task: random_id(),
                revision: 1,
                prompt: "Use the smaller fixture".into(),
            },
            Right::Operate,
        ),
        (
            Operation::CancelTask {
                task: random_id(),
                revision: 1,
                reason: "No longer needed".into(),
            },
            Right::Operate,
        ),
        (command(), Right::Operate),
        (terminal(), Right::Terminal),
        (Operation::ListDevices {}, Right::AccessRead),
        (
            Operation::Revoke {
                device: pubkey(&key()),
            },
            Right::AccessAdmin,
        ),
    ] {
        let error = observer.call(op).await.unwrap_err();
        assert_eq!(error.code, Code::MissingRight);
        assert_eq!(error.missing, Some(right));
    }
    assert_eq!(f.recorder.count(), 0);

    let (_, operator) = f.enroll("standard").await;
    let Outcome::Dispatched { receipt } = operator.call(task()).await.unwrap() else {
        panic!("dispatch expected")
    };
    assert_eq!(receipt.operation, "task.create");
    assert_eq!(f.recorder.count(), 1);
}

#[tokio::test]
async fn exact_retry_is_idempotent_and_a_reused_request_id_conflicts() {
    let f = Fixture::served(0, false).await;
    let (phone, operator) = f.enroll("standard").await;
    let pending = operator.prepare(task(), now()).unwrap();
    let first = exchange(&f.relay, &phone, &pending, &f.host_key)
        .await
        .unwrap();
    let second = exchange(&f.relay, &phone, &pending, &f.host_key)
        .await
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(f.recorder.count(), 1);
    assert!(operator.verify_reply(&pending, &first, now()).is_ok());

    // Same request ID, different signed bytes.
    let mut changed = pending.request.clone();
    changed.op = terminal();
    let event = seal(
        &changed,
        REQUEST,
        &phone,
        &f.host_key,
        &changed.request,
        changed.issued_at,
        changed.expires_at,
    )
    .unwrap();
    let conflicting = Pending {
        request: changed,
        event,
    };
    let reply = exchange(&f.relay, &phone, &conflicting, &f.host_key)
        .await
        .unwrap();
    let error = operator
        .verify_reply(&conflicting, &reply, now())
        .unwrap_err();
    assert_eq!(error.code, Code::Conflict);
}

#[tokio::test]
async fn reverse_enrollment_approve_and_deny() {
    let f = Fixture::served(0, false).await;
    let host = f.host();
    let pending = host
        .request_enrollment(&f.relay, Rights::standard(), now())
        .unwrap();
    assert_eq!(pending.events.len(), 1, "only the owner is addressed");
    host.publish(&f.relay, &pending.events).await.unwrap();
    let found = pending_enrollments(&f.relay, &f.owner, &f.host_key, POLICY)
        .await
        .unwrap();
    let [request] = found.as_slice() else {
        panic!("one enrollment request expected")
    };
    // The request carries no code; the approver types what the host shows.
    let body = serde_json::to_string(&request.enrollment).unwrap();
    assert!(!body.contains(&pending.code));

    let laptop = key();
    let op = request.approve(
        &pending.code.to_lowercase(),
        &pubkey(&laptop),
        Rights::standard(),
        now() + 3600,
    );
    let owner = f.owner_client();
    let Outcome::Granted { authorization } = owner.call(op.clone()).await.unwrap() else {
        panic!("grant expected")
    };
    let access =
        Access::from_authorization(*authorization, &laptop, &f.host_key, now(), POLICY).unwrap();
    assert_eq!(access.grant.origin.kind, OriginKind::Approval);
    assert_eq!(access.grant.origin.issuer, pubkey(&f.owner));
    // Approving the same request for the same device returns the same grant.
    let Outcome::Granted { authorization } = owner.call(op).await.unwrap() else {
        panic!("grant expected")
    };
    assert_eq!(
        authorization.tag_values("h").next(),
        Some(access.grant.grant.as_str())
    );
    let other = request.approve(
        &pending.code,
        &pubkey(&key()),
        Rights::standard(),
        now() + 3600,
    );
    assert_eq!(owner.call(other).await.unwrap_err().code, Code::Conflict);
    let laptop = Client::device(access, laptop, POLICY).unwrap();
    assert!(matches!(
        laptop.call(task()).await.unwrap(),
        Outcome::Dispatched { .. }
    ));

    // A second request goes to the owner and to the current administrator too.
    let (admin_key, admin) = f.enroll("observe,access_read,access_admin").await;
    let pending = host
        .request_enrollment(&f.relay, Rights::standard(), now())
        .unwrap();
    assert_eq!(pending.events.len(), 2);
    host.publish(&f.relay, &pending.events).await.unwrap();
    let found = pending_enrollments(&f.relay, &admin_key, &f.host_key, POLICY)
        .await
        .unwrap();
    let request = found
        .iter()
        .find(|e| e.enrollment.enrollment == pending.id)
        .unwrap();
    assert_eq!(
        admin.call(request.deny()).await.unwrap(),
        Outcome::Denied {}
    );
    let late = request.approve(
        &pending.code,
        &pubkey(&key()),
        Rights::standard(),
        now() + 60,
    );
    assert_eq!(owner.call(late).await.unwrap_err().code, Code::Denied);
    assert_eq!(
        host.enrollment_status(&pending.id, now()).unwrap(),
        crate::host::EnrollmentStatus::Denied
    );
}

#[tokio::test]
async fn wrong_codes_close_the_request_and_a_forged_digest_refuses() {
    let f = Fixture::served(0, false).await;
    let host = f.host();
    let pending = host
        .request_enrollment(&f.relay, Rights::standard(), now())
        .unwrap();
    host.publish(&f.relay, &pending.events).await.unwrap();
    let found = pending_enrollments(&f.relay, &f.owner, &f.host_key, POLICY)
        .await
        .unwrap();
    let request = &found[0];
    let owner = f.owner_client();
    let device = pubkey(&key());

    let mut forged = request.clone();
    forged.digest = nostr::contracts::digest_bytes(b"another request");
    let op = forged.approve(&pending.code, &device, Rights::standard(), now() + 60);
    assert_eq!(owner.call(op).await.unwrap_err().code, Code::Forbidden);

    let wrong = if pending.code.starts_with('0') {
        "1111-1111"
    } else {
        "0000-0000"
    };
    for _ in 0..MAX_CODE_ATTEMPTS {
        let op = request.approve(wrong, &device, Rights::standard(), now() + 60);
        assert_eq!(owner.call(op).await.unwrap_err().code, Code::WrongCode);
    }
    let op = request.approve(&pending.code, &device, Rights::standard(), now() + 60);
    assert_eq!(owner.call(op).await.unwrap_err().code, Code::RateLimited);
    assert_eq!(
        host.enrollment_status(&pending.id, now()).unwrap(),
        crate::host::EnrollmentStatus::Closed
    );
    assert!(f.host().devices(now()).unwrap().is_empty());
}

#[tokio::test]
async fn delegation_cannot_exceed_held_rights() {
    let f = Fixture::served(0, false).await;
    let (_, admin) = f.enroll("observe,access_admin").await;
    let expires = admin.access().unwrap().grant.expires_at;

    let wide = Operation::Invite {
        rights: Rights::parse_list("observe,operate").unwrap(),
        grant_expires_at: expires,
    };
    let error = admin.call(wide).await.unwrap_err();
    assert_eq!(
        (error.code, error.missing),
        (Code::MissingRight, Some(Right::Operate))
    );
    let late = Operation::Invite {
        rights: Rights::parse_list("observe").unwrap(),
        grant_expires_at: expires + 1,
    };
    assert_eq!(admin.call(late).await.unwrap_err().code, Code::Forbidden);

    let narrow = Operation::Invite {
        rights: Rights::parse_list("observe").unwrap(),
        grant_expires_at: expires,
    };
    let Outcome::Invitation { code, .. } = admin.call(narrow).await.unwrap() else {
        panic!("invitation expected")
    };
    let device = key();
    let access = client::redeem(&code, &device, POLICY).await.unwrap();
    assert_eq!(access.grant.rights, Rights::parse_list("observe").unwrap());
    assert_eq!(
        access.grant.origin.issuer,
        admin.access().unwrap().grant.device
    );
}

#[tokio::test]
async fn delegated_approval_is_bounded_by_the_approver() {
    let f = Fixture::served(0, false).await;
    let admin_key = key();
    let access = client::redeem(&f.invite("observe,access_admin"), &admin_key, POLICY)
        .await
        .unwrap();
    let admin = Client::device(access, admin_key, POLICY).unwrap();
    let host = f.host();
    let pending = host
        .request_enrollment(&f.relay, Rights::standard(), now())
        .unwrap();
    host.publish(&f.relay, &pending.events).await.unwrap();
    let found = pending_enrollments(&f.relay, &admin_key, &f.host_key, POLICY)
        .await
        .unwrap();
    let request = &found[0];
    let device = pubkey(&key());
    let op = request.approve(&pending.code, &device, Rights::standard(), now() + 60);
    let error = admin.call(op).await.unwrap_err();
    assert_eq!(
        (error.code, error.missing),
        (Code::MissingRight, Some(Right::Operate))
    );
    let op = request.approve(
        &pending.code,
        &device,
        Rights::parse_list("observe").unwrap(),
        now() + 60,
    );
    assert!(matches!(
        admin.call(op).await.unwrap(),
        Outcome::Granted { .. }
    ));
}

#[tokio::test]
async fn revocation_during_a_pending_request_refuses() {
    let f = Fixture::served(0, false).await;
    let (phone, operator) = f.enroll("standard").await;
    let pending = operator.prepare(task(), now()).unwrap();
    // The owner revokes while the signed request is still in flight.
    f.host().revoke(&pubkey(&phone), now()).unwrap();
    let reply = exchange(&f.relay, &phone, &pending, &f.host_key)
        .await
        .unwrap();
    let error = operator.verify_reply(&pending, &reply, now()).unwrap_err();
    assert_eq!(error.code, Code::Revoked);
    assert_eq!(f.recorder.count(), 0);

    // An administrator's pending invitation dies with the administrator's grant.
    let (admin_key, admin) = f.enroll("observe,access_admin").await;
    let invite = Operation::Invite {
        rights: Rights::parse_list("observe").unwrap(),
        grant_expires_at: now() + 600,
    };
    let Outcome::Invitation { code, .. } = admin.call(invite).await.unwrap() else {
        panic!("invitation expected")
    };
    let owner = f.owner_client();
    let revoked = owner
        .call(Operation::Revoke {
            device: pubkey(&admin_key),
        })
        .await
        .unwrap();
    assert!(matches!(revoked, Outcome::Revoked { epoch: 1, .. }));
    let error = client::redeem(&code, &key(), POLICY).await.unwrap_err();
    assert_eq!(error.code, Code::Revoked);
}

#[tokio::test]
async fn stale_epoch_and_copied_grants_refuse() {
    let f = Fixture::served(0, false).await;
    let (phone, first) = f.enroll("standard").await;
    let old = first.access().unwrap().grant.clone();
    f.host().revoke(&pubkey(&phone), now()).unwrap();
    // Re-enrollment issues a new grant at the device's new epoch.
    let access = client::redeem(&f.invite("standard"), &phone, POLICY)
        .await
        .unwrap();
    assert_eq!(access.grant.epoch, 1);
    let current = Client::device(access.clone(), phone, POLICY).unwrap();

    for (grant, epoch, code) in [
        (old.grant.as_str(), 0, Code::Revoked),
        (access.grant.grant.as_str(), 0, Code::Stale),
    ] {
        let pending = forge(&phone, &f.host_key, &f.relay, Some((grant, epoch)), task());
        let reply = exchange(&f.relay, &phone, &pending, &f.host_key)
            .await
            .unwrap();
        assert_eq!(
            current
                .verify_reply(&pending, &reply, now())
                .unwrap_err()
                .code,
            code
        );
    }
    assert!(current.call(task()).await.is_ok());

    // A copied grant is not a bearer credential.
    let thief = key();
    let pending = forge(
        &thief,
        &f.host_key,
        &f.relay,
        Some((&access.grant.grant, 1)),
        task(),
    );
    let reply = exchange(&f.relay, &thief, &pending, &f.host_key)
        .await
        .unwrap();
    let stranger = Client::owner(&f.host_key, &f.relay, thief, POLICY).unwrap();
    assert_eq!(
        stranger
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    // Neither is a key without any grant, unless the host established it as owner.
    assert_eq!(
        stranger.call(task()).await.unwrap_err().code,
        Code::Forbidden
    );
}

#[tokio::test]
async fn administrator_lists_and_revokes_devices() {
    let f = Fixture::served(0, false).await;
    let (_, admin) = f.enroll("admin").await;
    let (phone, operator) = f.enroll("standard").await;
    let Outcome::Devices { devices } = admin.call(Operation::ListDevices {}).await.unwrap() else {
        panic!("devices expected")
    };
    assert_eq!(devices.len(), 2);
    assert!(devices.iter().all(|d| d.state == DeviceState::Active));
    // The host records its own observation of each device. The listing
    // admin was seen by this request; the phone has made none yet.
    let seen = |key: &str| devices.iter().find(|d| d.device == key).unwrap().last_seen;
    let admin_key = admin.access().unwrap().grant.device.clone();
    assert!(seen(&admin_key).is_some_and(|at| at + 5 >= now() && at <= now()));
    assert_eq!(seen(&pubkey(&phone)), None);
    // A channel admission records the phone once per resolution interval.
    let grant = operator.access().unwrap().grant.grant.clone();
    let at = now();
    f.host().touch(&pubkey(&phone), &grant, at).unwrap();
    f.host().touch(&pubkey(&phone), &grant, at + 1).unwrap();
    // A key that does not hold the grant records nothing.
    f.host().touch(&admin_key, &grant, at + 2).unwrap();
    let listed = f.host().devices(now()).unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|d| d.device == pubkey(&phone))
            .unwrap()
            .last_seen,
        Some(at)
    );
    let Outcome::Revoked { grants, .. } = admin
        .call(Operation::Revoke {
            device: pubkey(&phone),
        })
        .await
        .unwrap()
    else {
        panic!("revocation expected")
    };
    assert_eq!(grants, vec![operator.access().unwrap().grant.grant.clone()]);
    assert_eq!(operator.call(task()).await.unwrap_err().code, Code::Revoked);
    let listed = f.host().devices(now()).unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|d| d.device == pubkey(&phone))
            .unwrap()
            .state,
        DeviceState::Revoked
    );
}

#[test]
fn owner_is_established_locally_and_the_store_is_private() {
    let f = Fixture::local();
    let host = f.host();
    assert_eq!(host.init(&pubkey(&f.owner)).unwrap(), f.host_key);
    assert_eq!(host.init(&pubkey(&key())).unwrap_err().code, Code::Conflict);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: PathBuf| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(f.dir.clone()), 0o700);
        for file in ["access.json", "access.lock", "host.key"] {
            assert_eq!(mode(f.dir.join(file)), 0o600, "{file}");
        }
    }
    #[cfg(windows)]
    for path in [
        f.dir.clone(),
        f.dir.join("access.json"),
        f.dir.join("access.lock"),
        f.dir.join("host.key"),
    ] {
        assert!(
            private_fs::is_private_path(&path).unwrap(),
            "{}",
            path.display()
        );
    }
    let issued = host
        .invite(
            "wss://relay.example/",
            Rights::standard(),
            now(),
            now() + 3600,
        )
        .unwrap();
    let text = std::fs::read_to_string(f.dir.join("access.json")).unwrap();
    let invitation = HostInvitation::parse(&issued.code, now(), RelayPolicy::Production).unwrap();
    assert!(!text.contains(invitation.0.capability()));
    assert!(issued.code.starts_with(INVITATION_PREFIX));
    // A history-observer invitation is never host access, and vice versa.
    let observer = issued
        .code
        .replacen(INVITATION_PREFIX, coder_connect::pairing::PREFIX, 1);
    assert!(HostInvitation::parse(&observer, now(), RelayPolicy::Production).is_err());
    assert!(
        coder_connect::pairing::Invitation::parse(&issued.code, now(), RelayPolicy::Production)
            .is_err()
    );
    // The grant must outlive the invitation.
    assert!(
        host.invite(
            "wss://relay.example/",
            Rights::standard(),
            now(),
            now() + 10
        )
        .is_err()
    );
}

fn command() -> Operation {
    Operation::CommandTask {
        command: crate::TaskCommand {
            command: random_id(),
            task: random_id(),
            action: crate::CommandAction::Queue,
            based_on: 2,
            text: "Then update the changelog.".into(),
            emulate: false,
            issued_at: now(),
        },
    }
}

#[tokio::test]
async fn a_task_command_reaches_the_owner_with_its_grant_and_epoch() {
    let f = Fixture::served(0, false).await;
    let (phone, operator) = f.enroll("standard").await;
    let Outcome::Dispatched { receipt } = operator.call(command()).await.unwrap() else {
        panic!("dispatch expected")
    };
    assert_eq!(receipt.operation, "task.command");
    let entry = f.host().devices(now()).unwrap().remove(0);
    let seen = f.recorder.seen();
    assert_eq!(
        seen[0].1,
        format!("{} {} {}", pubkey(&phone), entry.grant, entry.epoch)
    );
    // Emulation belongs to a steer only, and text stays bounded.
    let Operation::CommandTask { mut command } = command() else {
        unreachable!()
    };
    command.emulate = true;
    assert!(
        Operation::CommandTask {
            command: command.clone()
        }
        .validate()
        .is_err()
    );
    command.action = crate::CommandAction::Steer;
    assert!(
        Operation::CommandTask {
            command: command.clone()
        }
        .validate()
        .is_ok()
    );
    command.text = "x".repeat(16 * 1024 + 1);
    assert!(Operation::CommandTask { command }.validate().is_err());
}

#[tokio::test]
async fn spend_lists_carry_the_senders_own_grant_and_need_operate() {
    let f = Fixture::served(0, false).await;
    let (phone, operator) = f.enroll("standard").await;
    let grant =
        crate::spend::Grant::request_mode(random_id(), &pubkey(&phone), &f.host_key, 0, now());
    let Outcome::Spends { spends } = operator
        .call(Operation::ListSpends {
            grant: Box::new(grant.clone()),
        })
        .await
        .unwrap()
    else {
        panic!("spends expected")
    };
    assert!(spends.is_empty());
    assert_eq!(
        f.recorder.1.0.lock().unwrap()[0],
        (pubkey(&phone), grant.grant.clone())
    );
    // A grant another key issued, or one naming another host, is refused.
    let mut foreign = grant.clone();
    foreign.issuer = pubkey(&key());
    let refused = operator
        .call(Operation::ListSpends {
            grant: Box::new(foreign),
        })
        .await
        .unwrap_err();
    assert_eq!(refused.code, Code::Forbidden);
    let mut elsewhere = grant.clone();
    elsewhere.grantee = pubkey(&key());
    let refused = operator
        .call(Operation::ListSpends {
            grant: Box::new(elsewhere),
        })
        .await
        .unwrap_err();
    assert_eq!(refused.code, Code::Forbidden);
    // A receipt reaches the host's book and comes back as recorded.
    let receipt = crate::spend::Receipt::refused(
        &random_id(),
        &grant.grant,
        crate::spend::Refusal::DeclinedByOwner,
        now(),
    );
    let Outcome::Settled { receipt: recorded } = operator
        .call(Operation::SettleSpend {
            receipt: Box::new(receipt.clone()),
        })
        .await
        .unwrap()
    else {
        panic!("settled expected")
    };
    assert_eq!(*recorded, receipt);
    // A device that may only observe cannot list or settle.
    let (watcher, observer) = f.enroll("observe").await;
    let own =
        crate::spend::Grant::request_mode(random_id(), &pubkey(&watcher), &f.host_key, 0, now());
    let refused = observer
        .call(Operation::ListSpends {
            grant: Box::new(own),
        })
        .await
        .unwrap_err();
    assert_eq!(refused.code, Code::MissingRight);
}

/// `task.review` is a read under `observe`; `task.publish` is a mutation
/// under `operate`. A device holding only `observe` is refused a
/// publication before the task owner sees it (#10067, #10068).
#[tokio::test]
async fn publishing_a_reviewed_change_needs_operate() {
    let f = Fixture::served(0, false).await;
    let (_, observer) = f.enroll("observe").await;
    let task = "a".repeat(64);
    let revision = "b".repeat(40);
    let publish = Operation::PublishTask {
        task: task.clone(),
        base: revision.clone(),
        head_commit: revision.clone(),
        head: revision.clone(),
    };
    let error = observer.call(publish.clone()).await.unwrap_err();
    assert_eq!(
        (error.code, error.missing),
        (Code::MissingRight, Some(Right::Operate))
    );
    // The read passes the grant check; this task owner reviews nothing.
    let error = observer
        .call(Operation::ReviewTask { task: task.clone() })
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::Unsupported);
    // With `operate`, the publication reaches the task owner, which here
    // publishes nothing.
    let (_, operator) = f.enroll("standard").await;
    let error = operator.call(publish).await.unwrap_err();
    assert_eq!(error.code, Code::Unsupported);
    assert_eq!(f.recorder.count(), 0);
    // A revision that is not a Git object ID is malformed.
    assert!(
        Operation::PublishTask {
            task,
            base: "main".into(),
            head_commit: revision.clone(),
            head: revision,
        }
        .validate()
        .is_err()
    );
}
