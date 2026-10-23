use aethyrforge_host::*;
fn project() -> Project {
    Project {
        id: "fixture-world".into(),
        branch: "main".into(),
        revision: 1,
    }
}
fn status() -> Status {
    Status {
        project: project(),
        generation: "fixture-generation-1".into(),
        frame: 1,
        save: SaveState::Unsaved,
        selection: None,
        authoring: None,
        view: None,
        checkpoint: Some(Checkpoint {
            eye: [1., 2., 3.],
            target: [1., 2., 2.],
            mode: CameraMode::Walk,
        }),
    }
}
fn outcome() -> Outcome {
    Outcome {
        state: ResultState::Applied,
        message: "Opened.".into(),
        status: Some(status()),
        detail: None,
    }
}
fn pair() -> (Host, Engine) {
    let mut h = Host::new(
        "https://engine.example/run",
        "fixture-session".into(),
        "fixture-release".into(),
        "fixture-abi".into(),
        1,
        project(),
    )
    .unwrap();
    let hello = h.hello().unwrap().encode().unwrap();
    let (e, ready) = Engine::accept(
        &hello,
        "fixture-release".into(),
        "fixture-abi".into(),
        1,
        vec![Capability::Inspect, Capability::Observe],
    )
    .unwrap();
    h.receive(&ready.encode().unwrap()).unwrap();
    (h, e)
}
fn exchange(h: &mut Host, e: &mut Engine, a: Action, o: Outcome) -> String {
    let request = h.request(a, 0).unwrap();
    let Message::Request { request_id, .. } = e.receive(&request.encode().unwrap()).unwrap() else {
        panic!()
    };
    h.receive(&e.reply(&request_id, o).unwrap().encode().unwrap())
        .unwrap();
    request_id
}
#[test]
fn lifecycle_keeps_session_for_panels_and_restores_checkpoint_on_reload() {
    let (mut h, mut e) = pair();
    let session = h.session.clone();
    exchange(&mut h, &mut e, Action::Open { checkpoint: None }, outcome());
    exchange(&mut h, &mut e, Action::Observe { enabled: true }, outcome());
    for i in 2..200 {
        let mut s = status();
        s.frame = i;
        h.receive(&e.status(s).unwrap().encode().unwrap()).unwrap();
    }
    assert_eq!(h.session, session);
    assert_eq!(h.status.as_ref().unwrap().frame, 199);
    let checkpoint = h.status.as_ref().unwrap().checkpoint.clone();
    exchange(&mut h, &mut e, Action::Close, outcome());
    assert_eq!(h.phase, Phase::Closed);
    assert!(e.status(status()).is_err());
    let (mut reload, mut engine) = pair();
    exchange(
        &mut reload,
        &mut engine,
        Action::Open {
            checkpoint: checkpoint.clone(),
        },
        outcome(),
    );
    assert_eq!(reload.status.unwrap().checkpoint, checkpoint);
}
#[test]
fn forged_peer_version_session_payload_and_replay_fail() {
    let (mut h, mut e) = pair();
    assert_eq!(h.check_peer("https://evil.example", true), Err(Error::Peer));
    assert_eq!(
        h.check_peer("https://engine.example", false),
        Err(Error::Peer)
    );
    let req = h.request(Action::Open { checkpoint: None }, 0).unwrap();
    let mut forged = req.clone();
    forged.session = "other-session".into();
    assert_eq!(e.receive(&forged.encode().unwrap()), Err(Error::Session));
    assert!(
        e.receive(
            &req.encode()
                .unwrap()
                .replace("\"action\":", "\"unexpected\":true,\"action\":")
        )
        .is_err()
    );
    e.receive(&req.encode().unwrap()).unwrap();
    assert_eq!(e.receive(&req.encode().unwrap()), Err(Error::Replay));
    let Message::Request { request_id, .. } = req.message else {
        panic!()
    };
    let reply = e.reply(&request_id, outcome()).unwrap();
    h.receive(&reply.encode().unwrap()).unwrap();
    assert_eq!(h.receive(&reply.encode().unwrap()), Err(Error::Replay));
    let mut invalid = reply;
    invalid.version = VERSION + 1;
    assert_eq!(invalid.encode(), Err(Error::Version));
    let mut new = Host::new(
        "https://engine.example",
        "session".into(),
        "expected".into(),
        "abi".into(),
        1,
        project(),
    )
    .unwrap();
    new.hello().unwrap();
    let (_, ready) = Engine::accept(
        &Envelope {
            protocol: PROTOCOL.into(),
            version: VERSION,
            session: "session".into(),
            sequence: 1,
            message: Message::Hello {
                versions: vec![VERSION],
            },
        }
        .encode()
        .unwrap(),
        "wrong".into(),
        "abi".into(),
        1,
        vec![],
    )
    .unwrap();
    assert_eq!(new.receive(&ready.encode().unwrap()), Err(Error::Version));
    assert_eq!(new.phase, Phase::Connecting);
}
#[test]
fn bounds_stale_status_and_timeout_do_not_retry_mutations() {
    let (mut h, mut e) = pair();
    exchange(&mut h, &mut e, Action::Open { checkpoint: None }, outcome());
    assert_eq!(h.request(Action::Save, 0), Err(Error::Unsupported));
    let request = h.request(Action::Inspect, 0).unwrap();
    let Message::Request { request_id, .. } = e.receive(&request.encode().unwrap()).unwrap() else {
        panic!()
    };
    h.expire(DEADLINE_MS);
    assert_eq!(
        h.receipt(&request_id).unwrap().outcome.state,
        ResultState::Unknown
    );
    assert_eq!(h.pending(), 0);
    let late = e.reply(&request_id, outcome()).unwrap();
    h.receive(&late.encode().unwrap()).unwrap();
    assert_eq!(
        h.receipt(&request_id).unwrap().outcome.state,
        ResultState::Applied
    );
    let mut stale = status();
    stale.project.revision = 0;
    assert!(e.status(stale).is_err());
    for _ in 0..MAX_PENDING {
        h.request(Action::Inspect, 0).unwrap();
    }
    assert_eq!(h.request(Action::Inspect, 0), Err(Error::Busy));
    h.expire(DEADLINE_MS);
    for _ in 0..100 {
        exchange(&mut h, &mut e, Action::Inspect, outcome());
    }
    assert_eq!(h.receipts().len(), MAX_RESULTS);
    h.receive(&e.revoke("Access ended.".into()).unwrap().encode().unwrap())
        .unwrap();
    assert_eq!(h.phase, Phase::Revoked);
    assert!(h.status.is_none());
    assert!(h.request(Action::Inspect, 0).is_err());
}
#[test]
fn schema_and_numeric_limits() {
    let (mut h, _) = pair();
    let text = h
        .request(Action::Open { checkpoint: None }, 0)
        .unwrap()
        .encode()
        .unwrap();
    assert!(
        Envelope::decode(&text.replacen("\"version\":5", "\"version\":5,\"version\":5", 1))
            .is_err()
    );
    assert!(Envelope::decode(&" ".repeat(MAX_BYTES + 1)).is_err());
    assert!(
        Action::Draft {
            asset: "shader".into(),
            source: "💡".repeat(12001)
        }
        .validate()
        .is_err()
    );
    assert!(
        Action::Propose {
            operations: vec![Edit::Move {
                target: "entity".into(),
                position: [f64::NAN, 0., 0.]
            }]
        }
        .validate()
        .is_err()
    );
    assert!(
        Project {
            revision: MAX_SAFE_INTEGER + 1,
            ..project()
        }
        .validate()
        .is_err()
    );
    assert!(endpoint("http://evil.example").is_err());
    assert!(endpoint("https://user:secret@engine.example").is_err());
    assert!(endpoint("https://engine.example?token=secret").is_err());
    assert!(endpoint("http://127.0.0.1:1234/engine").is_ok());
    let expected = include_str!("../schema/host-v5.json");
    let actual = serde_json::to_string_pretty(&schemars::schema_for!(Envelope)).unwrap();
    assert_eq!(expected.trim(), actual.trim());
}

#[test]
fn evicted_ids_cannot_reexecute_and_lookup_does_not_replay() {
    let (mut h, mut e) = pair();
    let old = exchange(&mut h, &mut e, Action::Open { checkpoint: None }, outcome());
    for _ in 0..70 {
        exchange(&mut h, &mut e, Action::Inspect, outcome());
    }
    assert!(e.lookup(&old).is_none());
    let lookup = h
        .request(
            Action::Lookup {
                target: old.clone(),
            },
            0,
        )
        .unwrap();
    let mut forged = lookup.clone();
    if let Message::Request { request_id, .. } = &mut forged.message {
        *request_id = old;
    }
    assert_eq!(e.receive(&forged.encode().unwrap()), Err(Error::Replay));
    e.receive(&lookup.encode().unwrap()).unwrap();
    assert!(
        Action::Publish {
            visibility: Visibility::Public
        }
        .validate_outcome(&outcome())
        .is_err()
    );
    let hello = Envelope {
        protocol: PROTOCOL.into(),
        version: VERSION,
        session: "new-session".into(),
        sequence: 1,
        message: Message::Hello {
            versions: vec![VERSION],
        },
    };
    let (_, ready) = Engine::accept(
        &hello.encode().unwrap(),
        "fixture-release".into(),
        "fixture-abi".into(),
        1,
        vec![],
    )
    .unwrap();
    for variant in 0..2 {
        let mut host = Host::new(
            "https://engine.example",
            "new-session".into(),
            "fixture-release".into(),
            "fixture-abi".into(),
            1,
            project(),
        )
        .unwrap();
        host.hello().unwrap();
        let mut wrong = ready.clone();
        if let Message::Ready {
            engine_abi,
            project_format,
            ..
        } = &mut wrong.message
        {
            if variant == 0 {
                *engine_abi = "wrong".into()
            } else {
                *project_format = 2
            }
        }
        assert_eq!(host.receive(&wrong.encode().unwrap()), Err(Error::Version));
    }
}

#[test]
fn authentication_request_has_a_result_and_shutdown_clears_views() {
    let (mut h, mut e) = pair();
    let request = h.request(Action::Open { checkpoint: None }, 0).unwrap();
    let Message::Request { request_id, .. } = e.receive(&request.encode().unwrap()).unwrap() else {
        panic!()
    };
    h.receive(
        &e.auth_required(&request_id, "Sign in to open this private world.".into())
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(h.phase, Phase::Ready);
    assert_eq!(e.lookup(&request_id).unwrap().state, ResultState::Rejected);
    assert_eq!(
        h.receipt(&request_id).unwrap().outcome.state,
        ResultState::Rejected
    );
    exchange(&mut h, &mut e, Action::Open { checkpoint: None }, outcome());
    h.request(Action::Inspect, 0).unwrap();
    h.shutdown();
    assert_eq!(h.phase, Phase::Closed);
    assert!(h.status.is_none());
    assert!(h.capabilities.is_empty());
    assert_eq!(h.pending(), 0);
}

#[test]
fn initial_world_loading_has_a_bounded_deadline_separate_from_commands() {
    let (mut host, _) = pair();
    let open = host.request(Action::Open { checkpoint: None }, 0).unwrap();
    let Message::Request { request_id, .. } = open.message else {
        panic!()
    };
    host.expire(DEADLINE_MS);
    assert_eq!(host.pending(), 1);
    host.expire(OPEN_DEADLINE_MS);
    assert_eq!(host.pending(), 0);
    assert_eq!(
        host.receipt(&request_id).unwrap().outcome.state,
        ResultState::Unknown
    );
}
