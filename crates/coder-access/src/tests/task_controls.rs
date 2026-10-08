//! Synthetic admission and recovery checks for exact task controls.
use super::*;

fn enrolled(f: &Fixture, rights: &str, recorder: &mut Recorder) -> (SecretKey, Client) {
    let device = key();
    let invitation = HostInvitation::parse(&f.invite(rights), now(), POLICY).unwrap();
    let pending = client::prepare_redeem(&invitation, &device, now(), POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, now(), recorder)
        .unwrap();
    let access =
        client::finish_redeem(&invitation, &pending, &reply, &device, now(), POLICY).unwrap();
    (device, Client::device(access, device, POLICY).unwrap())
}

fn exact_command() -> Operation {
    Operation::CommandTaskAtRevision {
        revision: 1,
        command: TaskCommand {
            command: random_id(),
            task: random_id(),
            action: CommandAction::Steer,
            based_on: 1,
            text: "Synthetic exact control.".into(),
            emulate: false,
            issued_at: now(),
        },
    }
}

fn recovery(original: &Pending) -> Operation {
    Operation::RequestOperation {
        request: original.request.request.clone(),
        request_event: original.event.id.clone(),
    }
}

#[test]
fn exact_controls_require_current_operate_and_never_dispatch_on_refusal() {
    let fixture = Fixture::local();
    let mut recorder = Recorder::default();
    let (_, observer) = enrolled(&fixture, "observe", &mut recorder);
    let operations = [
        exact_command(),
        Operation::QueueTaskAtRevision {
            task: random_id(),
            revision: 1,
            edit: QueueEdit::List {},
            queue_digest: None,
        },
    ];
    for operation in operations {
        let pending = observer.prepare(operation, now()).unwrap();
        let reply = fixture
            .host()
            .handle(&pending.event, &fixture.relay, now(), &mut recorder)
            .unwrap();
        let error = observer.verify_reply(&pending, &reply, now()).unwrap_err();
        assert_eq!(
            (error.code, error.missing),
            (Code::MissingRight, Some(Right::Operate))
        );
    }
    assert_eq!(recorder.count(), 0);
    let (device, operator) = enrolled(&fixture, "standard", &mut recorder);
    let pending = operator.prepare(exact_command(), now()).unwrap();
    fixture.host().revoke(&pubkey(&device), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut recorder)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!(recorder.count(), 0);
}

#[test]
fn retained_effect_recovery_survives_envelope_expiry_without_dispatch() {
    let fixture = Fixture::local();
    let mut recorder = Recorder::default();
    let (device, operator) = enrolled(&fixture, "standard", &mut recorder);
    let original = operator.prepare(task(), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&original.event, &fixture.relay, now(), &mut recorder)
        .unwrap();
    let outcome = operator.verify_reply(&original, &reply, now()).unwrap();
    let later = original.request.expires_at + 1;
    assert_eq!(
        fixture
            .host()
            .handle(&original.event, &fixture.relay, later, &mut recorder)
            .unwrap_err()
            .code,
        Code::Expired
    );
    let pending = operator.prepare(recovery(&original), later).unwrap();
    for _ in 0..2 {
        let reply = fixture
            .host()
            .handle(&pending.event, &fixture.relay, later, &mut recorder)
            .unwrap();
        let Outcome::RequestOperation {
            result: Some(result),
            ..
        } = operator.verify_reply(&pending, &reply, later).unwrap()
        else {
            panic!("known original effect required")
        };
        assert_eq!(
            *result,
            ReplyResult::Ok {
                outcome: outcome.clone()
            }
        );
    }
    assert_eq!(recorder.count(), 1);
    let (_, other) = enrolled(&fixture, "standard", &mut recorder);
    let pending = other.prepare(recovery(&original), later).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, later, &mut recorder)
        .unwrap();
    assert_eq!(
        other
            .verify_reply(&pending, &reply, later)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!(recorder.count(), 1);
    let mut mismatch = recovery(&original);
    if let Operation::RequestOperation { request_event, .. } = &mut mismatch {
        *request_event = random_id();
    }
    let pending = operator.prepare(mismatch, later).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, later, &mut recorder)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&pending, &reply, later)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    fixture.host().revoke(&pubkey(&device), later).unwrap();
    let pending = operator.prepare(recovery(&original), later).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, later, &mut recorder)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&pending, &reply, later)
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!(recorder.count(), 1);
}

#[test]
fn an_uncertain_native_effect_recovers_unknown_and_never_reenvelopes() {
    let fixture = Fixture::local();
    let mut recorder = Recorder::default();
    let (_, operator) = enrolled(&fixture, "standard", &mut recorder);
    let original = operator.prepare(task(), now()).unwrap();
    let mut tick = 0;
    let result = fixture.host().handle_with_clock(
        &original.event,
        &fixture.relay,
        || {
            tick += 1;
            Ok(if tick == 1 {
                original.request.issued_at
            } else {
                original.request.expires_at + 1
            })
        },
        &mut recorder,
    );
    assert_eq!(result.unwrap_err().code, Code::Expired);
    let later = original.request.expires_at + 1;
    let pending = operator.prepare(recovery(&original), later).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, later, &mut recorder)
        .unwrap();
    assert!(matches!(
        operator.verify_reply(&pending, &reply, later),
        Ok(Outcome::RequestOperation { result: None, .. })
    ));
    assert_eq!(recorder.count(), 1);
}

#[test]
fn recovery_never_discloses_effects_under_a_read_only_original_grant() {
    let fixture = Fixture::local();
    let mut recorder = Recorder::default();
    let (_, observer) = enrolled(&fixture, "observe", &mut recorder);
    let original = observer.prepare(task(), now()).unwrap();
    fixture
        .host()
        .handle(&original.event, &fixture.relay, now(), &mut recorder)
        .unwrap();
    let pending = observer.prepare(recovery(&original), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut recorder)
        .unwrap();
    let error = observer.verify_reply(&pending, &reply, now()).unwrap_err();
    assert_eq!(
        (error.code, error.missing),
        (Code::MissingRight, Some(Right::Operate))
    );
    assert_eq!(recorder.count(), 0);
}

#[test]
fn exact_queue_and_command_validation_bind_every_fence() {
    let mut command = exact_command();
    if let Operation::CommandTaskAtRevision { revision, .. } = &mut command {
        *revision += 1;
    }
    assert_eq!(command.validate().unwrap_err().code, Code::Malformed);
    let mut queue = Operation::QueueTaskAtRevision {
        task: random_id(),
        revision: 1,
        edit: QueueEdit::Lease {},
        queue_digest: None,
    };
    assert_eq!(queue.validate().unwrap_err().code, Code::Malformed);
    if let Operation::QueueTaskAtRevision { queue_digest, .. } = &mut queue {
        *queue_digest = Some(format!("sha256:{}", "a".repeat(64)));
    }
    assert!(queue.validate().is_ok());
    assert!(!queue.reads_only() && queue.retains_reply());
    if let Operation::QueueTaskAtRevision { edit, .. } = &mut queue {
        *edit = QueueEdit::List {};
    }
    assert!(queue.reads_only() && !queue.retains_reply());
    assert_eq!(queue.required(), Some(Right::Operate));
}

#[test]
fn recovery_retention_is_bounded_and_unknown_reads_do_not_extend_it() {
    let fixture = Fixture::local();
    let mut recorder = Recorder::default();
    let owner = fixture.owner_client();
    let original = owner.prepare(task(), now()).unwrap();
    fixture
        .host()
        .handle(&original.event, &fixture.relay, now(), &mut recorder)
        .unwrap();
    for (later, known) in [
        (original.request.expires_at + 48 * 60 * 60 - 1, true),
        (original.request.expires_at + 48 * 60 * 60 + 1, false),
    ] {
        let pending = owner.prepare(recovery(&original), later).unwrap();
        let reply = fixture
            .host()
            .handle(&pending.event, &fixture.relay, later, &mut recorder)
            .unwrap();
        let Outcome::RequestOperation { result, .. } =
            owner.verify_reply(&pending, &reply, later).unwrap()
        else {
            panic!("native recovery required")
        };
        assert_eq!(result.is_some(), known);
    }
    assert_eq!(recorder.count(), 1);
}

#[test]
fn exact_control_replies_cannot_substitute_a_task_or_earlier_revision() {
    let op = exact_command();
    let Operation::CommandTaskAtRevision { command, .. } = &op else {
        unreachable!()
    };
    let mut reply = Outcome::Dispatched {
        receipt: Receipt {
            operation: op.name().into(),
            reference: command.task.clone(),
        },
    };
    assert!(reply.answers(&op));
    if let Outcome::Dispatched { receipt } = &mut reply {
        receipt.reference = random_id();
    }
    assert!(!reply.answers(&op));
    let op = Operation::QueueTaskAtRevision {
        task: random_id(),
        revision: 4,
        edit: QueueEdit::Lease {},
        queue_digest: Some(format!("sha256:{}", "a".repeat(64))),
    };
    let Operation::QueueTaskAtRevision { task, .. } = &op else {
        unreachable!()
    };
    let reply = Outcome::QueueAtRevision {
        queue: TaskQueue {
            task: task.clone(),
            revision: 3,
            lease: None,
            items: vec![],
        },
        revision: 3,
        queue_digest: format!("sha256:{}", "b".repeat(64)),
    };
    assert!(!reply.answers(&op));
}
