use super::*;
use crate::imported::chamber_loopback::Loopback;

/// Steps `session` until `done` holds or five seconds pass.
fn until(session: &mut Session, scene: &Scene, mut done: impl FnMut(&Session) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if session.step(scene, Held::default()).is_err() {
            return done(session);
        }
        if done(session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

fn joined(host: &Loopback, scene: &Scene) -> Session {
    let (client, runtime) = host.connect().unwrap();
    let mut session = Session::start(client, runtime, scene).unwrap();
    assert!(until(&mut session, scene, |s| s.hud().is_some()));
    session
}

#[test]
fn a_session_plays_the_loopback_chamber_and_stops_cleanly() {
    let host = Loopback::start(false).unwrap();
    let scene = Loopback::scene().unwrap();
    let mut session = joined(&host, &scene);
    assert!(session.alive());
    assert!(session.controlled(&scene));
    let start = session.position().unwrap();
    let forward = Held {
        forward: true,
        ..Held::default()
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && session.position().unwrap().distance(start) < 0.5 {
        session.step(&scene, forward).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(session.position().unwrap().distance(start) >= 0.5);
    let dir = tempfile::tempdir().unwrap();
    let pack = super::super::original::generate(dir.path()).unwrap();
    let atlas = super::super::original::atlas().unwrap();
    let frame = session.frame(&pack, &atlas, &scene, [640, 360]).unwrap();
    assert!(!frame.instances.is_empty());
    assert_eq!(frame.overlay, [640., 360.]);
    assert_eq!(session.stop(), Stopped::Closed);
}

#[test]
fn held_movement_reaches_the_authority_without_retiring_control() {
    let host = Loopback::start(false).unwrap();
    let scene = Loopback::scene().unwrap();
    let mut session = joined(&host, &scene);
    session.observe();
    assert!(until(&mut session, &scene, |s| {
        s.prediction.movement_profile() == Some(verse_world::movement::Profile::Frames)
    }));
    let (start, epoch) = host.with(|g| {
        let a = g.game().player_admission(14).unwrap();
        (g.game().actor_position(14).unwrap(), a.epoch())
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        session
            .step(
                &scene,
                Held {
                    strafe_left: true,
                    ..Held::default()
                },
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(16));
    }
    let (end, final_epoch, sequence) = host.with(|g| {
        let a = g.game().player_admission(14).unwrap();
        (
            g.game().actor_position(14).unwrap(),
            a.epoch(),
            a.accepted_sequence(),
        )
    });
    assert!(
        end.distance(start) > 0.1,
        "{start:?} -> {end:?}; {:?}",
        session.take_notes()
    );
    assert_eq!(epoch, final_epoch);
    assert!(sequence > 0);
    assert!(session.status.is_empty(), "{}", session.status);
}

#[test]
fn a_host_that_goes_away_fails_the_session() {
    let host = Loopback::start(false).unwrap();
    let scene = Loopback::scene().unwrap();
    let mut session = joined(&host, &scene);
    host.sever();
    assert!(until(&mut session, &scene, |s| !s.alive()));
    let error = loop {
        match session.step(&scene, Held::default()) {
            Ok(_) => std::thread::sleep(Duration::from_millis(5)),
            Err(error) => break error,
        }
    };
    assert_eq!(error, "Chamber update stream stopped");
    assert!(matches!(session.stop(), Stopped::Failed(message) if !message.is_empty()));
}

#[test]
fn a_new_session_after_a_stop_takes_the_same_character_back() {
    let host = Loopback::start(false).unwrap();
    let scene = Loopback::scene().unwrap();
    let session = joined(&host, &scene);
    let life = session.owned_life().unwrap();
    assert_eq!(session.stop(), Stopped::Closed);
    // The suspended phone resumes: a second connection to the same instance.
    let session = joined(&host, &scene);
    assert_eq!(session.owned_life(), Some(life));
    assert_eq!(session.stop(), Stopped::Closed);
}

#[test]
fn a_dead_player_respawns_once_into_a_new_life() {
    let host = Loopback::start(true).unwrap();
    let scene = Loopback::scene().unwrap();
    let mut session = joined(&host, &scene);
    assert!(session.dead());
    assert!(!session.controlled(&scene));
    let dead = session.owned_life().unwrap();
    session.respawn();
    session.respawn();
    assert_eq!(session.respawn_attempts(), &[dead]);
    assert_eq!(session.pending.len(), 1);
    // A life still dead after the retry interval asks again (the authority
    // refuses a respawn onto an obstructed spawn point), without growing the
    // record of lives asked for.
    session.respawn_asked = Some(Instant::now() - RESPAWN_RETRY);
    session.respawn();
    session.respawn();
    assert_eq!(session.pending.len(), 2);
    assert_eq!(session.respawn_attempts(), &[dead]);
    assert!(until(&mut session, &scene, |s| s.life_changes == 1 && !s.dead()));
    assert_ne!(session.owned_life(), Some(dead));
    assert!(session.controlled(&scene));
    assert_eq!(session.stop(), Stopped::Closed);
}

#[test]
fn native_prediction_binds_local_input_renders_it_and_retires_acknowledgments() {
    use secp256k1::{Keypair, Secp256k1, SecretKey};
    use verse_world::service::{
        Chamber,
        auth::{ConnectionId, Gateway},
        wire::{Body, Request, Response, VERSION},
    };
    fn request(g: &mut Gateway, id: ConnectionId, serial: u64, body: Body) -> Response {
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id: serial,
            body,
        })
        .unwrap();
        serde_json::from_slice(&g.dispatch_json(id, 0, &bytes).unwrap()).unwrap()
    }
    let dir = tempfile::tempdir().unwrap();
    let pack = super::super::original::generate(dir.path()).unwrap();
    let atlas = super::super::original::atlas().unwrap();
    let scene = Scene::from_json(include_bytes!(
        "../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let mut game = verse_world::play::Game::combat_in(scene.clone(), false, 160).unwrap();
    game.time = scene.cut_at;
    game.tick(1. / 30., [0.; 2]).unwrap();
    let mut gateway = Gateway::new(Chamber::new(game).unwrap()).unwrap();
    let key = Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_byte_array([101; 32]).unwrap(),
    );
    let public = key.x_only_public_key().0.serialize();
    gateway.enroll_primary(public).unwrap();
    let (connection, challenge) = gateway.open(0).unwrap();
    let signature = Secp256k1::new()
        .sign_schnorr_no_aux_rand(&challenge.signing_digest(public), &key)
        .to_byte_array();
    gateway
        .authenticate(connection, 0, public, signature)
        .unwrap();
    let (input, mut inputs, updates, output) = worker::channels();
    let _ = atlas;
    let mut session = Session::attached(View::new(160, 10., 0).unwrap(), input, output);
    session.observe();
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            1,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    // The first owned-life observation releases pointer input with a tracked stop.
    let Input::TrackedCommand { token, intent, .. } = inputs.try_recv().unwrap() else {
        panic!("Missing tracked stop");
    };
    let command = gateway
        .admission(connection)
        .unwrap()
        .command(gateway.game().authority_tick, intent)
        .unwrap();
    updates
        .try_send(Update::CommandBound {
            token,
            binding: Ok(command.clone()),
        })
        .unwrap();
    updates
        .try_send(Update::Outcome(request(
            &mut gateway,
            connection,
            2,
            Body::Command {
                command: command.into(),
            },
        )))
        .unwrap();
    gateway.tick(1. / 30.).unwrap();
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            3,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    let life = gateway.admission(connection).unwrap().actor();
    let start = gateway.game().actor_position(life.actor).unwrap();
    session.send(Input::Command(Intent::Move {
        axes: [1., 0.],
        yaw: 0.,
    }));
    let Input::TrackedCommand {
        token,
        life: submitted,
        epoch,
        intent,
    } = inputs.try_recv().unwrap()
    else {
        panic!("Missing tracked movement");
    };
    assert_eq!(submitted, life);
    assert_eq!(epoch, gateway.admission(connection).unwrap().epoch());
    session.prediction.advance(1. / 30.).unwrap();
    let pose = session.prediction.pose().unwrap();
    assert!(pose.position.x > start.x);
    assert_eq!(gateway.game().actor_position(life.actor).unwrap(), start);
    let camera = Camera {
        eye: pose.position + Vec3::new(0., 3., 8.),
        target: pose.position + Vec3::Y * 1.5,
        fov: 60.,
    };
    let rendered = chamber::remote_scene_predicted(
        &pack,
        &session.view,
        1.,
        camera,
        Vec3::ZERO,
        false,
        pose.position,
        Some(pose),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        rendered
            .frame
            .actors
            .iter()
            .find(|a| a.life == Some(life))
            .unwrap()
            .actor
            .position,
        pose.position
    );
    let command = gateway
        .admission(connection)
        .unwrap()
        .command(gateway.game().authority_tick, intent)
        .unwrap();
    updates
        .try_send(Update::CommandBound {
            token,
            binding: Ok(command.clone()),
        })
        .unwrap();
    updates
        .try_send(Update::Outcome(request(
            &mut gateway,
            connection,
            4,
            Body::Command {
                command: command.into(),
            },
        )))
        .unwrap();
    let pending = request(&mut gateway, connection, 5, Body::Snapshot {});
    let verse_world::service::wire::Reply::Snapshot { state } = &pending.body else {
        panic!("Missing pending snapshot");
    };
    assert!(state.movement.is_none());
    assert!(state.collision.is_some());
    updates.try_send(Update::Snapshot(pending)).unwrap();
    session.consume(&scene).unwrap();
    session.prediction.advance(0.).unwrap();
    assert_eq!(session.prediction.observation(), 5);
    assert_eq!(session.prediction.pending(), 1);
    gateway.tick(1. / 30.).unwrap();
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            6,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    session.prediction.advance(0.).unwrap();
    assert_eq!(session.prediction.pending(), 0);
    assert!(session.pending.is_empty());
    assert!(
        session
            .prediction
            .pose()
            .unwrap()
            .position
            .distance(gateway.game().actor_position(life.actor).unwrap())
            < 0.0001
    );
    // Teleport while another movement input is admitted but not yet applied.
    session.send(Input::Command(Intent::Move {
        axes: [0., 1.],
        yaw: 0.,
    }));
    let Input::TrackedCommand { token, intent, .. } = inputs.try_recv().unwrap() else {
        panic!("Missing pending move");
    };
    let command = gateway
        .admission(connection)
        .unwrap()
        .command(gateway.game().authority_tick, intent)
        .unwrap();
    updates
        .try_send(Update::CommandBound {
            token,
            binding: Ok(command.clone()),
        })
        .unwrap();
    updates
        .try_send(Update::Outcome(request(
            &mut gateway,
            connection,
            7,
            Body::Command {
                command: command.into(),
            },
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    session.prediction.advance(1. / 30.).unwrap();
    assert_eq!(session.prediction.pending(), 1);
    session.send(Input::Command(Intent::Cast {
        ability: Ability::MistyStep,
        target: None,
        aim: [1., 0., 0.],
    }));
    let Input::Command(intent) = inputs.try_recv().unwrap() else {
        panic!("Missing teleport cast");
    };
    let command = gateway
        .admission(connection)
        .unwrap()
        .command(gateway.game().authority_tick, intent)
        .unwrap();
    let outcome = request(
        &mut gateway,
        connection,
        8,
        Body::Command {
            command: command.into(),
        },
    );
    assert!(matches!(
        outcome.body,
        verse_world::service::wire::Reply::Accepted
    ));
    updates.try_send(Update::Outcome(outcome)).unwrap();
    let snapshot = request(&mut gateway, connection, 9, Body::Snapshot {});
    let verse_world::service::wire::Reply::Snapshot { state } = &snapshot.body else {
        panic!("Missing teleport snapshot");
    };
    assert!(state.movement.is_none());
    assert!(
        state
            .presentation
            .actors
            .iter()
            .find(|a| a.life.actor == life.actor)
            .unwrap()
            .teleport_stamp
            .is_some()
    );
    updates.try_send(Update::Snapshot(snapshot)).unwrap();
    session.consume(&scene).unwrap();
    assert!(session.prediction.pose().is_none());
    assert_eq!(session.prediction.pending(), 0);
    let notes = session.take_notes();
    assert_eq!(
        notes.iter().filter(|n| matches!(n, Note::Reset(_))).count(),
        1
    );
    let bound: Vec<_> = notes
        .iter()
        .filter_map(|n| match n {
            Note::Bound { token, .. } => Some(*token),
            _ => None,
        })
        .collect();
    assert!(!bound.is_empty());
    for token in bound {
        assert!(
            notes
                .iter()
                .any(|n| matches!(n, Note::Outcome { token: Some(t), .. } if *t == token)),
            "bound input {token} reached no outcome"
        );
    }
    gateway.tick(1. / 30.).unwrap();
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            10,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    session.prediction.advance(0.).unwrap();
    assert!(
        session
            .prediction
            .pose()
            .unwrap()
            .position
            .distance(gateway.game().actor_position(life.actor).unwrap())
            < 0.0001
    );
    // The native interval path preserves local event times until transport binding.
    while inputs.try_recv().is_ok() {}
    let life = gateway.admission(connection).unwrap().actor();
    let stop_command = gateway
        .admission(connection)
        .unwrap()
        .command(
            gateway.game().authority_tick,
            Intent::Move {
                axes: [0.; 2],
                yaw: 0.,
            },
        )
        .unwrap();
    gateway.submit(connection, stop_command).unwrap();
    gateway.tick(1. / 30.).unwrap();
    let entry_epoch = gateway.admission(connection).unwrap().epoch();
    let begin = request(
        &mut gateway,
        connection,
        300,
        Body::BeginMovementFrames {
            life: life.into(),
            epoch: entry_epoch,
        },
    );
    assert!(matches!(
        begin.body,
        verse_world::service::wire::Reply::Snapshot { .. }
    ));
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            301,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    assert_eq!(
        session.prediction.movement_profile(),
        Some(verse_world::movement::Profile::Frames)
    );
    session.send(Input::Command(Intent::Move {
        axes: [1., 0.],
        yaw: 0.,
    }));
    assert!(inputs.try_recv().is_err());
    session.prediction.advance(2. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    assert!(inputs.try_recv().is_err());
    session.send(Input::Command(Intent::Move {
        axes: [0., 1.],
        yaw: 0.,
    }));
    session.prediction.advance(2. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    let Input::MovementFrame { token, mut frame } = inputs.try_recv().unwrap() else {
        panic!("Missing complete interval")
    };
    // A 30 Hz update sends its complete four-step history without another wake-up.
    assert_eq!(frame.steps, 4);
    assert_eq!(frame.segments.len(), 2);
    assert_eq!(frame.segments[0].axes, [1., 0.]);
    assert_eq!(frame.segments[1].offset, 2);
    assert_eq!(frame.segments[1].axes, [0., 1.]);
    assert_eq!(frame.sequence, 0);
    frame.sequence = gateway.admission(connection).unwrap().accepted_sequence() + 1;
    frame.tick = gateway.game().authority_tick;
    updates
        .try_send(Update::FrameBound {
            token,
            binding: Ok(frame.clone()),
        })
        .unwrap();
    session.consume(&scene).unwrap();
    let mut response = request(&mut gateway, connection, 302, Body::MovementFrame { frame });
    assert!(matches!(
        response.body,
        verse_world::service::wire::Reply::Accepted
    ));
    // Capture an older body, then deliver movement only after it actually applies.
    let mut older = request(&mut gateway, connection, 303, Body::Snapshot {});
    gateway.tick(0.05).unwrap();
    let applied = gateway.game().movement_baseline(life).unwrap().unwrap();
    response.control.as_mut().unwrap().credit_step = gateway.game().physics_steps;
    older.control.as_mut().unwrap().credit_step = gateway.game().physics_steps;
    response.control.as_mut().unwrap().applied_movement = Some(applied);
    updates.try_send(Update::Outcome(response)).unwrap();
    session.consume(&scene).unwrap();
    session.prediction.advance(0.).unwrap();
    assert_eq!(
        session.prediction.confirmed().unwrap().applied_sequence,
        applied.applied_sequence
    );
    assert_eq!(session.prediction.pending(), 0);
    assert!(
        session
            .prediction
            .pose()
            .unwrap()
            .position
            .distance(gateway.game().actor_position(life.actor).unwrap())
            < 0.0001
    );
    assert!(session.take_notes().iter().any(
        |n| matches!(n, Note::Correction { detail, discontinuity:false, .. }
        if serde_json::to_value(detail).unwrap()["stage"] == "applied_movement_confirmation")
    ));
    updates.try_send(Update::Snapshot(older)).unwrap();
    session.consume(&scene).unwrap();
    assert_eq!(
        session.prediction.confirmed().unwrap().applied_sequence,
        applied.applied_sequence
    );
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            304,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    session.prediction.advance(0.).unwrap();
    assert!(
        session
            .prediction
            .pose()
            .unwrap()
            .position
            .distance(gateway.game().actor_position(life.actor).unwrap())
            < 0.0001
    );
    // A delayed render wake-up combines both turns into one bounded request.
    gateway.tick(0.1).unwrap();
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            305,
            Body::Snapshot {},
        )))
        .unwrap();
    session.consume(&scene).unwrap();
    let start = session.frame_cursor.unwrap().2;
    session.send(Input::Command(Intent::Move {
        axes: [1., 0.],
        yaw: 0.,
    }));
    session.prediction.advance(6. / 120.).unwrap();
    session.send(Input::Command(Intent::Move {
        axes: [0., 1.],
        yaw: 0.,
    }));
    session.prediction.advance(6. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    let Input::MovementFrame { frame: first, .. } = inputs.try_recv().unwrap() else {
        panic!("Missing first catch-up interval");
    };
    assert_eq!(first.start, start);
    assert_eq!(first.steps, verse_world::movement::frames::MAX_STEPS);
    assert_eq!(first.end().unwrap(), start + 12);
    assert_eq!(first.segments.len(), 2);
    assert_eq!(first.segments[1].offset, 6);
    assert_eq!(first.segments[0].axes, [1., 0.]);
    assert_eq!(first.segments[1].axes, [0., 1.]);
    assert!(inputs.try_recv().is_err());
    // Credit can end between packet boundaries; the final four steps must not wait for an ACK.
    let context = session.prediction.context().unwrap();
    let credit = session.prediction.physics_step() + 4;
    session
        .prediction
        .grant_world_credit(context.0, context.1, credit)
        .unwrap();
    session.prediction.advance(12. / 120.).unwrap();
    session.prediction.advance(10. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    let Input::MovementFrame { frame: full_a, .. } = inputs.try_recv().unwrap() else {
        panic!("Missing full interval")
    };
    assert_eq!(full_a.start, first.end().unwrap());
    assert_eq!(full_a.steps, verse_world::movement::frames::MAX_STEPS);
    assert!(inputs.try_recv().is_err());
    session.send_movement_interval().unwrap();
    let Input::MovementFrame {
        frame: remainder, ..
    } = inputs.try_recv().unwrap()
    else {
        panic!("Missing credit remainder")
    };
    assert_eq!(remainder.start, full_a.end().unwrap());
    assert_eq!(remainder.steps, 4);
    assert_eq!(
        remainder.end().unwrap(),
        credit + u64::from(verse_world::movement::frames::MAX_STEPS)
    );
    assert_eq!(remainder.segments[0].axes, [0., 1.]);
    let local_step = session.prediction.physics_step();
    session.send_movement_interval().unwrap();
    assert!(inputs.try_recv().is_err());
    assert_eq!(session.prediction.physics_step(), local_step);
    let cursor = session.frame_cursor;
    let token = session.input_token + 1;
    session
        .frame_bindings
        .insert(token, (context.0, context.1 - 1));
    session.pending.push_back((None, Some(token)));
    updates
        .try_send(Update::FrameBound {
            token,
            binding: Err("Movement interval control changed before transmission".into()),
        })
        .unwrap();
    session.consume(&scene).unwrap();
    assert_eq!(session.prediction.context(), Some(context));
    assert_eq!(session.frame_cursor, cursor);
    assert!(!session.frame_bindings.contains_key(&token));
    assert!(
        !session
            .pending
            .iter()
            .any(|(_, pending)| *pending == Some(token))
    );
    // A full local history is backpressure, not a new movement timeline.
    for _ in 0..verse_world::prediction::CAPACITY {
        if session.prediction.pending() == verse_world::prediction::CAPACITY {
            break;
        }
        session.send(Input::Command(Intent::Jump));
    }
    assert_eq!(
        session.prediction.pending(),
        verse_world::prediction::CAPACITY
    );
    let before_token = session.input_token;
    let before_failures = session.prediction_failures;
    session.send(Input::Command(Intent::Move {
        axes: [0.; 2],
        yaw: 0.,
    }));
    assert_eq!(session.status, "Input queue is busy");
    assert_eq!(session.prediction.context(), Some(context));
    assert_eq!(
        session.prediction.pending(),
        verse_world::prediction::CAPACITY
    );
    assert_eq!(session.frame_cursor, cursor);
    assert_eq!(session.input_token, before_token);
    assert_eq!(session.prediction_failures, before_failures);
    session.frame_bindings.insert(token + 1, context);
    updates
        .try_send(Update::FrameBound {
            token: token + 1,
            binding: Err("Current interval binding failed".into()),
        })
        .unwrap();
    session.consume(&scene).unwrap();
    assert!(session.prediction.context().is_none());
    assert!(session.frame_cursor.is_none());
    drop(updates);
    assert!(session.consume(&scene).is_err());
    assert!(session.prediction.pose().is_none());
}

#[test]
fn bounded_native_input_reports_pressure_and_closed_update_streams() {
    let dir = tempfile::tempdir().unwrap();
    let pack = super::super::original::generate(dir.path()).unwrap();
    let atlas = super::super::original::atlas().unwrap();
    let scene = Scene::from_json(include_bytes!(
        "../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let _ = (pack, atlas);
    let view = View::new(160, 10., 0).unwrap();
    let (input, mut inputs, updates, output) = worker::channels();
    let mut session = Session::attached(view, input, output);
    assert!(!session.controlled(&scene));
    session.respawn();
    assert!(session.respawn_attempts.is_empty());
    session.cast(&scene, Ability::Fireball);
    assert!(inputs.try_recv().is_err());
    for _ in 0..worker::INPUT_CAPACITY {
        session.send(Input::Command(Intent::Jump));
    }
    session.send(Input::Command(Intent::Jump));
    assert_eq!(session.status, "Input queue is busy");
    for _ in 0..worker::INPUT_CAPACITY {
        assert!(matches!(
            inputs.try_recv(),
            Ok(Input::Command(Intent::Jump))
        ));
    }
    assert!(inputs.try_recv().is_err());
    drop(inputs);
    session.send(Input::Respawn);
    assert_eq!(session.status, "Chamber connection stopped");
    assert!(session.consume(&scene).is_ok());
    session.pending.clear();
    session.pending.push_back((Some(Ability::Shield), None));
    session.pending.push_back((None, Some(17)));
    session.pending.push_back((None, Some(18)));
    session.status.clear();
    updates
        .try_send(Update::MovementSuperseded {
            token: 17,
            replacement: 18,
        })
        .unwrap();
    session.consume(&scene).unwrap();
    assert_eq!(
        session.pending.iter().copied().collect::<Vec<_>>(),
        vec![(Some(Ability::Shield), None), (None, Some(18))]
    );
    assert!(session.status.is_empty());
    drop(updates);
    assert!(session.consume(&scene).is_err());
}

#[test]
fn read_credit_advances_intervals_without_acknowledging_input_and_fences_old_epochs() {
    use verse_world::service::{
        event_cursor::Delivery,
        wire::{Response, VERSION},
    };
    let host = Loopback::start(false).unwrap();
    let scene = Loopback::scene().unwrap();
    let mut connected = joined(&host, &scene);
    assert!(until(&mut connected, &scene, |s| s.movement_frames()));
    let replica = connected.view.replica();
    let snapshot = Response {
        version: VERSION,
        request_id: 1,
        instance: replica
            .latest()
            .unwrap()
            .hud
            .as_ref()
            .unwrap()
            .life
            .instance,
        tick: replica.tick().unwrap(),
        control: replica.control().cloned(),
        body: Reply::Snapshot {
            state: replica.latest().unwrap().clone(),
        },
    };
    connected.stop();
    let (input, mut inputs, updates, output) = worker::channels();
    let mut session =
        Session::attached(View::new(snapshot.instance, 10., 0).unwrap(), input, output);
    updates
        .try_send(Update::Snapshot(snapshot.clone()))
        .unwrap();
    session.consume(&scene).unwrap();
    let baseline = session.prediction.confirmed().unwrap();
    let mut control = snapshot.control.clone().unwrap();
    control.credit_step = control.credit_step.max(session.prediction.physics_step()) + 12;
    let deliver = |control| Update::Events {
        delivery: Delivery {
            events: Vec::new(),
            gap: None,
        },
        checkpoint: Vec::new(),
        control: Some(control),
    };
    updates.try_send(deliver(control.clone())).unwrap();
    session.consume(&scene).unwrap();
    session.prediction.recover_world_credit().unwrap();
    let after = session.prediction.physics_step();
    assert!(after > baseline.physics_step);
    assert_eq!(session.prediction.confirmed().unwrap(), baseline);
    session.prediction.advance(4. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    let Input::MovementFrame { token, .. } = inputs.try_recv().unwrap() else {
        panic!("Missing pending movement frame");
    };
    let pending = session.pending.len();
    control.credit_step += 12;
    let mut credit = snapshot.clone();
    credit.body = Reply::Accepted;
    credit.control = Some(control.clone());
    let before = session.prediction.physics_step();
    updates
        .try_send(Update::MovementCredit(credit.clone()))
        .unwrap();
    session.consume(&scene).unwrap();
    assert_eq!(session.pending.len(), pending);
    assert_eq!(session.pending.front().unwrap().1, Some(token));
    assert_eq!(session.prediction.physics_step(), before);
    session.prediction.recover_world_credit().unwrap();
    let after = session.prediction.physics_step();
    assert!(after > before);
    assert_eq!(session.prediction.confirmed().unwrap(), baseline);
    control.epoch += 1;
    control.credit_step += 120;
    credit.control = Some(control.clone());
    updates.try_send(Update::MovementCredit(credit)).unwrap();
    updates.try_send(deliver(control)).unwrap();
    session.consume(&scene).unwrap();
    assert_eq!(session.prediction.physics_step(), after);
    assert_eq!(
        session.prediction.context(),
        Some((baseline.life, baseline.epoch))
    );
}
