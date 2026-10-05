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
    session.prediction.advance(4. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    assert!(inputs.try_recv().is_err());
    session.send(Input::Command(Intent::Move {
        axes: [0., 1.],
        yaw: 0.,
    }));
    session.prediction.advance(8. / 120.).unwrap();
    session.send_movement_interval().unwrap();
    let Input::MovementFrame { token, mut frame } = inputs.try_recv().unwrap() else {
        panic!("Missing complete interval")
    };
    assert_eq!(frame.steps, verse_world::movement::frames::MAX_STEPS);
    assert_eq!(frame.segments.len(), 2);
    assert_eq!(frame.segments[0].axes, [1., 0.]);
    assert_eq!(frame.segments[1].offset, 4);
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
    let response = request(&mut gateway, connection, 302, Body::MovementFrame { frame });
    assert!(matches!(
        response.body,
        verse_world::service::wire::Reply::Accepted
    ));
    updates.try_send(Update::Outcome(response)).unwrap();
    session.consume(&scene).unwrap();
    gateway.tick(0.1).unwrap();
    updates
        .try_send(Update::Snapshot(request(
            &mut gateway,
            connection,
            303,
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
    let context = session.prediction.context().unwrap();
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
