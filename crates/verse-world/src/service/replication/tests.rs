use super::*;
use crate::service::{
    auth::{ConnectionId, Gateway},
    net::tests::{gateway, key},
    replica::Buffer,
    wire::{Body, Reply, Request, Response, VERSION},
};
fn join(g: &mut Gateway, n: u8) -> ConnectionId {
    let key = key(n);
    let (id, challenge) = g.open(0).unwrap();
    let public = key.x_only_public_key().0.serialize();
    let signature = secp256k1::Secp256k1::new()
        .sign_schnorr_no_aux_rand(&challenge.signing_digest(public), &key)
        .to_byte_array();
    g.authenticate(id, 0, public, signature).unwrap();
    id
}
fn request(g: &mut Gateway, id: ConnectionId, number: u64, body: Body) -> Response {
    let bytes = serde_json::to_vec(&Request {
        version: VERSION,
        request_id: number,
        body,
    })
    .unwrap();
    serde_json::from_slice(&g.dispatch_json(id, 0, &bytes).unwrap()).unwrap()
}
fn state(g: &mut Gateway, id: ConnectionId) -> State {
    let Reply::Snapshot { state } = request(g, id, 1, Body::Snapshot {}).body else {
        panic!()
    };
    state
}
fn admit(receiver: &mut Receiver, r: &Response) -> State {
    let Reply::Replicated { packet } = &r.body else {
        panic!("{:?}", r.body)
    };
    receiver
        .admit(packet, r.instance, r.tick, &r.control)
        .unwrap()
}
#[test]
fn canonical_full_delta_resync_and_control_fences() {
    let keys = [key(1), key(2), key(3)];
    let mut g = gateway(&keys);
    let id = join(&mut g, 1);
    for _ in 0..12 {
        g.tick(1. / 30.).unwrap();
    }
    let mut receiver = Receiver::default();
    let first = request(&mut g, id, 1, Body::Replicate { ack: None });
    let a = admit(&mut receiver, &first);
    assert!(a.scope.is_some());
    assert_eq!(
        a.hud.as_ref().unwrap().life,
        first.control.as_ref().unwrap().life.into()
    );
    let second = request(
        &mut g,
        id,
        2,
        Body::Replicate {
            ack: receiver.ack(),
        },
    );
    assert!(matches!(
        second.body,
        Reply::Replicated {
            packet: Packet::Delta { .. }
        }
    ));
    let b = admit(&mut receiver, &second);
    assert_eq!(encode(&a).unwrap(), encode(&b).unwrap());
    let bad = Baseline {
        revision: 999,
        tick: 0,
        digest: [0; 32],
    };
    let full = request(&mut g, id, 3, Body::Replicate { ack: Some(bad) });
    assert!(matches!(
        full.body,
        Reply::Replicated {
            packet: Packet::Full { .. }
        }
    ));
    admit(&mut receiver, &full);
    let restored = request(&mut g, id, 4, Body::Replicate { ack: None });
    admit(&mut receiver, &restored);
    let ack = receiver.ack();
    let control = g.admission(id).unwrap();
    g.begin_movement_frames(id, control.actor(), control.epoch())
        .unwrap();
    let fenced = request(&mut g, id, 5, Body::Replicate { ack });
    assert!(matches!(
        fenced.body,
        Reply::Replicated {
            packet: Packet::Full { .. }
        }
    ));
    admit(&mut receiver, &fenced);
    assert_eq!(g.replication[&id].saved.len(), 2);
    assert!(g.replication[&id].stats.retained_bytes <= 2 * MAX_BASELINE_BYTES);
    g.close(id).unwrap();
    assert!(g.replication.is_empty());
}
#[test]
fn malformed_deltas_never_advance_acknowledgement() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 1);
    let mut sender = Sender::default();
    let mut receiver = Receiver::default();
    let state = scope(
        state(&mut g, connection),
        &request(&mut g, connection, 2, Body::Snapshot {}).control,
        None,
        0,
    )
    .unwrap();
    let control = request(&mut g, connection, 3, Body::Snapshot {}).control;
    let first = sender
        .packet(state.clone(), &control, 120, 0, None)
        .unwrap();
    receiver.admit(&first, 120, 0, &control).unwrap();
    let ack = receiver.ack().unwrap();
    let mut newer = state.clone();
    newer.presentation.time += 0.1;
    newer.snapshot.elapsed += 0.1;
    newer.hud.as_mut().unwrap().time += 0.1;
    let delta = sender.packet(newer, &control, 120, 1, Some(ack)).unwrap();
    assert!(matches!(delta, Packet::Delta { .. }));
    let mut wrong = delta.clone();
    let Packet::Delta { baseline, .. } = &mut wrong else {
        unreachable!()
    };
    baseline.digest = [9; 32];
    assert!(receiver.admit(&wrong, 120, 1, &control).is_err());
    assert_eq!(receiver.ack(), Some(ack));
    let mut wrong = delta.clone();
    let Packet::Delta { edits, .. } = &mut wrong else {
        unreachable!()
    };
    edits.push(Edit {
        path: vec![Part::Field("missing".into())],
        value: Value::Null,
    });
    assert!(receiver.admit(&wrong, 120, 1, &control).is_err());
    assert_eq!(receiver.ack(), Some(ack));
    let mut wrong = delta.clone();
    let Packet::Delta { edits, .. } = &mut wrong else {
        unreachable!()
    };
    edits.push(Edit {
        path: vec![],
        value: serde_json::json!({"unexpected":true}),
    });
    assert!(receiver.admit(&wrong, 120, 1, &control).is_err());
    assert_eq!(receiver.ack(), Some(ack));
    let mut empty = Receiver::default();
    assert!(empty.admit(&delta, 120, 1, &control).is_err());
    receiver.admit(&delta, 120, 1, &control).unwrap();
    assert!(receiver.admit(&first, 120, 0, &control).is_err());
    assert!(receiver.admit(&delta, 120, 2, &control).is_err());
    receiver.clear();
    assert!(receiver.admit(&first, 120, 0, &control).is_err());
    assert!(receiver.ack().is_none());
}
fn add_far(state: &mut State, count: usize) {
    let actor = state
        .snapshot
        .actors
        .iter()
        .find(|a| a.kind != "adventurer")
        .unwrap()
        .clone();
    let pose = state
        .presentation
        .actors
        .iter()
        .find(|p| p.actor.model != "adventurer")
        .unwrap()
        .clone();
    for i in 0..count {
        let id = 1000 + i as u32;
        let mut a = actor.clone();
        a.id = id;
        a.pos = [500. + i as f32 * 40., 0., 500.];
        let life = crate::service::wire::Life {
            instance: 120,
            actor: id as u64,
            generation: 0,
        };
        let mut p = pose.clone();
        p.life = life;
        p.actor.id = id as u64;
        p.actor.position = a.pos.into();
        state.snapshot.actors.push(a);
        state.presentation.actors.push(p);
        state
            .actors
            .push(crate::service::wire::ActorBinding { source: id, life });
    }
}
#[test]
fn distant_population_keeps_steady_bytes_constant_and_records_cost() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 1);
    let initial = state(&mut g, connection);
    let control = request(&mut g, connection, 2, Body::Snapshot {}).control;
    let mut steady = None;
    for population in [0, 32, 128] {
        let mut source = initial.clone();
        add_far(&mut source, population);
        let mut sender = Sender::default();
        let mut receiver = Receiver::default();
        let mut bytes = vec![];
        let mut scoped_actors = 0;
        let start = std::time::Instant::now();
        for tick in 0..60 {
            source.snapshot.elapsed += 1. / 30.;
            source.presentation.time += 1. / 30.;
            source.hud.as_mut().unwrap().time = source.presentation.time;
            let index = Index::new(&source);
            let packet = sender
                .project(source.clone(), &control, 120, tick, receiver.ack(), &index)
                .unwrap();
            let serialized = serde_json::to_vec(&packet).unwrap();
            bytes.push(serialized.len());
            let packet: Packet = serde_json::from_slice(&serialized).unwrap();
            scoped_actors = receiver
                .admit(&packet, 120, tick, &control)
                .unwrap()
                .actors
                .len();
        }
        assert_eq!(
            sender.stats.encoded_bytes,
            bytes.iter().sum::<usize>() as u64
        );
        assert_eq!(sender.stats.max_packet_bytes, *bytes.iter().max().unwrap());
        assert_eq!(sender.stats.deltas, 59);
        if let Some(expected) = &steady {
            assert_eq!(&bytes[1..], expected);
        } else {
            steady = Some(bytes[1..].to_vec());
        }
        println!(
            "VERSE_V05_EVIDENCE {}",
            serde_json::json!({"distant_actors":population,"scoped_actors":scoped_actors,"full_bytes":bytes[0],"steady_total_bytes":bytes[1..].iter().sum::<usize>(),"steady_samples":59,"encode_micros":sender.stats.encode_micros,"elapsed_micros":start.elapsed().as_micros(),"deltas":sender.stats.deltas,"full":sender.stats.full,"retained_bytes":sender.stats.retained_bytes})
        );
    }
}
#[test]
fn spatial_entry_exit_reentry_and_frequency_classes_preserve_life() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 3);
    let mut source = state(&mut g, connection);
    add_far(&mut source, 1);
    let extra = source.presentation.actors.last().unwrap().life;
    let mut sender = Sender::default();
    let mut receiver = Receiver::default();
    let mut replica = Buffer::new(120, 10.).unwrap();
    for (tick, x, expected) in [
        (0, 500., false),
        (6, 50., true),
        (7, 51., true),
        (12, 51., true),
        (18, 500., false),
        (24, 50., true),
    ] {
        source.snapshot.actors.last_mut().unwrap().pos = [x, 0., -22.];
        source
            .presentation
            .actors
            .last_mut()
            .unwrap()
            .actor
            .position = glam::Vec3::new(x, 0., -22.);
        let previous = sender.previous(receiver.ack(), &None);
        let scoped = scope(source.clone(), &None, previous.as_ref(), tick).unwrap();
        let pose = scoped.presentation.actors.iter().find(|p| p.life == extra);
        assert_eq!(pose.is_some(), expected);
        if tick == 7 {
            assert_eq!(pose.unwrap().actor.position.x, 50.);
        }
        if tick == 12 {
            assert_eq!(pose.unwrap().actor.position.x, 51.);
        }
        assert!(scoped.snapshot.abilities.is_empty());
        assert_eq!(scoped.snapshot.player.max_hp, 0);
        assert!(scoped.hud.is_none());
        let packet = sender
            .packet(scoped, &None, 120, tick, receiver.ack())
            .unwrap();
        let state = receiver.admit(&packet, 120, tick, &None).unwrap();
        replica
            .push(&Response {
                version: VERSION,
                instance: 120,
                request_id: tick + 1,
                tick,
                control: None,
                body: Reply::Snapshot { state },
            })
            .unwrap();
    }
}
#[test]
fn full_state_numeric_round_trip_preserves_canonical_digest() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 1);
    let original = state(&mut g, connection);
    let restored: State = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    fn compare(a: &Value, b: &Value, path: String) {
        match (a, b) {
            (Value::Object(a), Value::Object(b)) => {
                for (k, v) in a {
                    compare(v, &b[k], format!("{path}/{k}"));
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                for (i, v) in a.iter().enumerate() {
                    compare(v, &b[i], format!("{path}/{i}"));
                }
            }
            _ => assert_eq!(a, b, "{path}"),
        }
    }
    compare(
        &serde_json::to_value(&original).unwrap(),
        &serde_json::to_value(&restored).unwrap(),
        String::new(),
    );
    assert_eq!(encode(&original).unwrap(), encode(&restored).unwrap());
    let (value, bytes) = encode_parts(&original).unwrap();
    assert_eq!(value, serde_json::from_slice::<Value>(&bytes).unwrap());
    assert_eq!(bytes, encode(&original).unwrap());
}

#[test]
fn signed_zero_is_equal_for_delta_and_digest() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 1);
    let control = request(&mut g, connection, 1, Body::Snapshot {}).control;
    let mut state = scope(state(&mut g, connection), &control, None, 0).unwrap();
    state.movement.as_mut().unwrap().character.vertical_speed = 0.;
    let mut sender = Sender::default();
    let mut receiver = Receiver::default();
    let first = sender
        .packet(state.clone(), &control, 120, 0, None)
        .unwrap();
    receiver.admit(&first, 120, 0, &control).unwrap();
    state.movement.as_mut().unwrap().character.vertical_speed = -0.;
    let packet = sender
        .packet(state, &control, 120, 1, receiver.ack())
        .unwrap();
    assert!(matches!(packet, Packet::Delta { .. }));
    receiver.admit(&packet, 120, 1, &control).unwrap();
}

#[path = "transport.rs"]
mod transport;

#[test]
fn skipped_world_ticks_do_not_starve_outer_band_updates() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 3);
    let mut source = state(&mut g, connection);
    add_far(&mut source, 1);
    let mut sender = Sender::default();
    let mut receiver = Receiver::default();
    for (tick, x, expected) in [(5, 50., 50.), (8, 51., 50.), (11, 52., 52.), (17, 53., 53.)] {
        source.snapshot.actors.last_mut().unwrap().pos = [x, 0., -22.];
        source
            .presentation
            .actors
            .last_mut()
            .unwrap()
            .actor
            .position = glam::Vec3::new(x, 0., -22.);
        let index = Index::new(&source);
        let packet = sender
            .project(source.clone(), &None, 120, tick, receiver.ack(), &index)
            .unwrap();
        let state = receiver.admit(&packet, 120, tick, &None).unwrap();
        assert_eq!(
            state.presentation.actors.last().unwrap().actor.position.x,
            expected
        );
    }
}
#[test]
fn scoped_collision_keeps_large_rotated_support_and_drops_distant_small_shape() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 1);
    let source = state(&mut g, connection);
    let control = request(&mut g, connection, 2, Body::Snapshot {}).control;
    let mut expanded = source.clone();
    let mut large = source.collision.as_ref().unwrap().colliders[0].clone();
    large.key.life.entity = 9000;
    large.pose.position = glam::DVec3::new(500., 0., -22.);
    large.pose.rotation = glam::DQuat::from_rotation_y(0.6);
    large.geometry = physics::queries::GeometrySnapshot::Box {
        min: glam::DVec3::new(-600., -1., -600.),
        max: glam::DVec3::new(600., 0., 600.),
    };
    let mut small = large.clone();
    small.key.life.entity = 9001;
    small.geometry = physics::queries::GeometrySnapshot::Box {
        min: glam::DVec3::splat(-1.),
        max: glam::DVec3::splat(1.),
    };
    expanded
        .collision
        .as_mut()
        .unwrap()
        .colliders
        .extend([large, small]);
    let scoped = scope(expanded, &control, None, 0).unwrap();
    scoped.validate_control(120, &control).unwrap();
    let collision = scoped.collision.unwrap();
    assert!(
        collision
            .colliders
            .iter()
            .any(|c| c.key.life.entity == 9000)
    );
    assert!(
        !collision
            .colliders
            .iter()
            .any(|c| c.key.life.entity == 9001)
    );
}
#[test]
fn valid_but_oversized_geometry_refuses_without_retaining_baseline() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut g, 1);
    let control = request(&mut g, connection, 1, Body::Snapshot {}).control;
    let mut state = scope(state(&mut g, connection), &control, None, 0).unwrap();
    let triangles = (0..16384)
        .map(|i| {
            let p = glam::DVec3::new(i as f64 * 0.001, 0.123456789, 0.);
            physics::queries::Triangle([p, p + glam::DVec3::Y, p + glam::DVec3::Z])
        })
        .collect();
    state.collision.as_mut().unwrap().colliders[0].geometry =
        physics::queries::GeometrySnapshot::Triangles { triangles };
    state.validate_control(120, &control).unwrap();
    let mut sender = Sender::default();
    assert!(
        sender
            .packet(state.clone(), &control, 120, 0, None)
            .unwrap_err()
            .contains("byte budget")
    );
    assert!(sender.saved.is_empty());
    assert_eq!(sender.revision, 0);
    let mut receiver = Receiver::default();
    let packet = Packet::Full {
        baseline: Baseline {
            revision: 1,
            tick: 0,
            digest: [0; 32],
        },
        state,
    };
    assert!(
        receiver
            .admit(&packet, 120, 0, &control)
            .unwrap_err()
            .contains("byte budget")
    );
    assert!(receiver.ack().is_none());
}

#[test]
fn shared_cache_keeps_owner_privacy_and_invalidates_on_mutation() {
    let mut g = gateway(&[key(1), key(2), key(3)]);
    let a = join(&mut g, 1);
    let b = join(&mut g, 2);
    let s = join(&mut g, 3);
    let first = state(&mut g, a);
    let cached = g.view_cache.as_ref().unwrap() as *const State;
    let second = state(&mut g, b);
    assert_eq!(cached, g.view_cache.as_ref().unwrap() as *const State);
    assert_ne!(first.hud.unwrap().life, second.hud.unwrap().life);
    let spectator = state(&mut g, s);
    assert!(spectator.hud.is_none());
    assert!(spectator.movement.is_none());
    assert!(spectator.collision.is_none());
    assert!(spectator.snapshot.abilities.is_empty());
    assert_eq!(spectator.snapshot.player.mana, 0);
    let admission = g.admission(a).unwrap();
    let command = admission
        .command(
            g.game().authority_tick,
            crate::Intent::Cast {
                ability: crate::play::Ability::Shield,
                target: None,
                aim: [0., 0., 1.],
            },
        )
        .unwrap();
    g.submit(a, command).unwrap();
    assert!(g.view_cache.is_none());
    let after = state(&mut g, a);
    assert!(
        after
            .presentation
            .effects
            .iter()
            .find(|e| e.life == admission.actor().into())
            .unwrap()
            .shield
            > 0
    );
    g.tick(1. / 30.).unwrap();
    assert!(g.view_cache.is_none());
    let mut receiver = Receiver::default();
    let full = request(&mut g, a, 10, Body::Replicate { ack: None });
    admit(&mut receiver, &full);
    let stats = g.replication_stats();
    assert_eq!(stats.full, 1);
    assert!(stats.retained_bytes > 0);
    g.revoke(key(1).x_only_public_key().0.serialize()).unwrap();
    assert_eq!(g.replication_stats().full, 1);
    assert_eq!(g.replication_stats().retained_bytes, 0);
    assert!(matches!(
        request(
            &mut g,
            a,
            11,
            Body::Replicate {
                ack: receiver.ack()
            }
        )
        .body,
        Reply::Refused { .. }
    ));
}
#[tokio::test]
async fn pipeline_refuses_second_replaceable_request_but_preserves_reliable_reads() {
    use crate::service::{client::Client, net::tests::start};
    use rustls::pki_types::ServerName;
    let keys = [key(241), key(242), key(243)];
    let (address, tls, stop, host) = start(&keys).await;
    let client = Client::connect(
        address,
        ServerName::try_from("localhost").unwrap(),
        tls.config().clone(),
        120,
        &keys[0],
    )
    .await
    .unwrap();
    let mut pipeline = client.pipeline().unwrap();
    let first = pipeline.send_snapshot().unwrap();
    assert!(
        pipeline
            .send_snapshot()
            .unwrap_err()
            .contains("already in flight")
    );
    let events = pipeline
        .send(Body::Events {
            after: 0,
            limit: 64,
        })
        .unwrap();
    assert_eq!(events, first + 1);
    let inventory = pipeline.send(Body::Inventory {}).unwrap();
    assert_eq!(inventory, events + 1);
    assert_eq!(pipeline.pending(), 3);
    let (_, snapshot) = pipeline.receive().await.unwrap();
    assert!(matches!(snapshot.body, Reply::Snapshot { .. }));
    let (_, events) = pipeline.receive().await.unwrap();
    assert!(matches!(events.body, Reply::Events { .. }));
    let (_, inventory) = pipeline.receive().await.unwrap();
    assert!(matches!(inventory.body, Reply::Inventory { .. }));
    pipeline.send_snapshot().unwrap();
    pipeline.receive().await.unwrap();
    drop(pipeline);
    stop.send(()).unwrap();
    let exit = host.await.unwrap();
    assert!(exit.failure.is_none());
}

#[test]
fn near_band_history_skip_preserves_packets_across_distance_and_ack_changes() {
    let mut gateway = gateway(&[key(1), key(2), key(3)]);
    let connection = join(&mut gateway, 1);
    let mut source = state(&mut gateway, connection);
    let control = request(&mut gateway, connection, 2, Body::Snapshot {}).control;
    add_far(&mut source, 1);
    let center = source
        .presentation
        .actors
        .iter()
        .find(|pose| Some(pose.life) == control.as_ref().map(|c| c.life))
        .unwrap()
        .actor
        .position;
    let mut optimized = Sender::default();
    let mut reference = Sender::default();
    let mut receiver = Receiver::default();
    for tick in 0u64..24 {
        let distance = match tick % 8 {
            0..=2 => 31.,
            3..=5 => 33.,
            _ => 50.,
        };
        let position = center + glam::Vec3::X * distance;
        source.snapshot.actors.last_mut().unwrap().pos = position.to_array();
        source
            .presentation
            .actors
            .last_mut()
            .unwrap()
            .actor
            .position = position;
        if distance == 31. {
            assert!(!spatial::needs_outer_history(&source, &control));
        }
        let ack = if tick == 9 { None } else { receiver.ack() };
        let due = tick.saturating_sub(reference.outer_tick) >= 6;
        let previous = if due {
            None
        } else {
            reference.previous(ack, &control)
        };
        let refresh = due || previous.is_none();
        let index = Index::new(&source);
        let expected =
            scoped(source.clone(), &control, previous.as_ref(), refresh, &index).unwrap();
        let expected = reference
            .packet(expected, &control, 120, tick, ack)
            .unwrap();
        if refresh {
            reference.outer_tick = tick;
        }
        let actual = optimized
            .project(source.clone(), &control, 120, tick, ack, &index)
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&actual).unwrap(),
            serde_json::to_vec(&expected).unwrap(),
            "Packet changed at tick {tick}"
        );
        assert_eq!(optimized.outer_tick, reference.outer_tick);
        receiver.admit(&actual, 120, tick, &control).unwrap();
    }
}
