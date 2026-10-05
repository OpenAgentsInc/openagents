use super::*;
use crate::service::{
    Chamber,
    net::tests::key,
    wire::{Hello, Reply, Response, VERSION},
};
fn gateway(instance: u64, n: u8) -> Gateway {
    let scene = verse_engine::director::Scene::from_json(include_bytes!(
        "../../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let mut game = crate::play::Game::combat_in(scene, false, instance).unwrap();
    game.time = game.scene.cut_at;
    game.tick(0., [0.; 2]).unwrap();
    let mut gateway = Gateway::new(Chamber::new(game).unwrap())
        .unwrap()
        .with_content([9; 32])
        .unwrap();
    gateway
        .enroll_primary(key(n).x_only_public_key().0.serialize())
        .unwrap();
    gateway
}
#[test]
fn leases_fence_expiry_draining_restart_and_competing_coordinators() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let mut realm = Realm::open(&root).unwrap();
    assert!(Realm::open(&root).is_err());
    realm
        .create(gateway(1001, 11), "127.0.0.1:4001".parse().unwrap(), 1, 0)
        .unwrap();
    realm
        .create(gateway(1002, 12), "127.0.0.1:4002".parse().unwrap(), 1, 0)
        .unwrap();
    assert_eq!(realm.characters().count(), 2);
    let a = realm.acquire(1001, [1; 32], 0).unwrap();
    let b = realm.acquire(1002, [2; 32], 0).unwrap();
    assert!(realm.acquire(1001, [3; 32], 1).is_err());
    realm.tick(&a, 1, 1. / 30.).unwrap();
    realm.tick(&b, 1, 1. / 30.).unwrap();
    realm.drain(&a, 2).unwrap();
    assert!(realm.open_connection(&a, 2).is_err());
    realm.tick(&a, 3, 1. / 30.).unwrap();
    assert!(realm.restart(1001, 3).is_err());
    realm.stop(&a, 4).unwrap();
    assert!(realm.tick(&a, 4, 1. / 30.).is_err());
    realm.restart(1001, 5).unwrap();
    let current = realm.acquire(1001, [3; 32], 5).unwrap();
    assert!(realm.tick(&a, 6, 1. / 30.).is_err());
    realm.tick(&current, 6, 1. / 30.).unwrap();
    assert!(realm.tick(&b, 30_001, 1. / 30.).is_err());
    let new_b = realm.acquire(1002, [4; 32], 30_001).unwrap();
    realm.tick(&new_b, 30_002, 1. / 30.).unwrap();
    let before = realm.games[&1001].game().authority_tick;
    realm.checkpoint(&current, 30_003).unwrap();
    let old_route = realm.route(1, 30_003).unwrap();
    drop(realm);
    let mut recovered = Realm::open(&root).unwrap();
    assert_eq!(recovered.games[&1001].game().authority_tick, before);
    assert!(recovered.tick(&current, 30_004, 1. / 30.).is_err());
    assert!(recovered.route(1, 30_004).is_err());
    let fresh = recovered.acquire(1001, [3; 32], 30_004).unwrap();
    let route = recovered.route(1, 30_004).unwrap();
    assert!(route.epoch > old_route.epoch);
    recovered.tick(&fresh, 30_005, 1. / 30.).unwrap();
    assert_eq!(recovered.characters().count(), 2);
}
#[test]
fn owned_wire_dispatch_and_dirty_reads_select_durable_game_checkpoints() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let mut realm = Realm::open(&root).unwrap();
    realm
        .create(gateway(1001, 13), "127.0.0.1:4001".parse().unwrap(), 1, 0)
        .unwrap();
    let lease = realm.acquire(1001, [1; 32], 0).unwrap();
    let (connection, hello) = realm.open_connection(&lease, 1).unwrap();
    let hello: Hello = serde_json::from_slice(&hello).unwrap();
    let key = key(13);
    let public = key.x_only_public_key().0.serialize();
    let signature = secp256k1::Secp256k1::new()
        .sign_schnorr_no_aux_rand(&hello.challenge.signing_digest(public), &key)
        .to_byte_array()
        .to_vec();
    let request = Request {
        version: VERSION,
        request_id: 1,
        body: Body::Authenticate {
            public_key: public,
            signature,
        },
    };
    let reply: Response = serde_json::from_slice(
        &realm
            .dispatch(
                &lease,
                connection,
                1,
                &serde_json::to_vec(&request).unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(reply.body, Reply::Accepted));
    realm.tick(&lease, 2, 1. / 30.).unwrap();
    let tick = realm.games[&1001].game().authority_tick;
    let request = Request {
        version: VERSION,
        request_id: 2,
        body: Body::Replicate { ack: None },
    };
    let reply: Response = serde_json::from_slice(
        &realm
            .dispatch(
                &lease,
                connection,
                2,
                &serde_json::to_vec(&request).unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(reply.tick, tick);
    assert!(realm.dirty.is_empty());
    drop(realm);
    let recovered = Realm::open(&root).unwrap();
    assert_eq!(recovered.games[&1001].game().authority_tick, tick);
}

fn joining(g: &mut Gateway, n: u8) -> ConnectionId {
    let k = key(n);
    let public = k.x_only_public_key().0.serialize();
    let (id, challenge) = g.open(0).unwrap();
    let signature =
        secp256k1::Secp256k1::new().sign_schnorr_no_aux_rand(&challenge.signing_digest(public), &k);
    g.authenticate(id, 0, public, signature.to_byte_array())
        .unwrap();
    id
}
fn transfer_fixture(root: &Path) -> (Realm, Lease, Lease, u64, super::super::rewards::Receipt) {
    use crate::service::{
        items::{Catalog, Item},
        rewards::{Entry, Transaction},
    };
    let catalog = Catalog {
        version: 1,
        items: vec![Item {
            id: 1,
            name: "Mana draught".into(),
            health: 0,
            mana: 5,
        }],
    };
    let mut source = gateway(1001, 11).with_items(catalog.clone()).unwrap();
    let life = source
        .enroll_player(
            key(14).x_only_public_key().0.serialize(),
            [2., 0., 0.].into(),
        )
        .unwrap();
    source
        .grant_reward(Transaction {
            instance: 1001,
            actor: life.actor,
            source: [77; 32],
            experience: 900,
            items: vec![Entry { id: 1, count: 4 }],
            quests: vec![],
            spent: vec![],
            acceptance: None,
            outfit: None,
            equipment: None,
        })
        .unwrap();
    let source_id = source
        .chamber
        .game
        .simulation
        .player_ids()
        .find(|id| *id != 0)
        .unwrap();
    source
        .chamber
        .game
        .simulation
        .player_damage_for(source_id, 75)
        .unwrap();
    source
        .chamber
        .game
        .simulation
        .spend_mana_for(source_id, 10)
        .unwrap();
    source
        .chamber
        .game
        .simulation
        .cast_for(source_id, crate::rules::Spell::Firebolt, [1., 0., 0.])
        .unwrap();
    let connection = joining(&mut source, 14);
    let admission = source.admission(connection).unwrap();
    let receipt = source
        .use_item(connection, life, admission.epoch(), 1, [55; 16])
        .unwrap();
    source.close_all().unwrap();
    let destination = gateway(1002, 12).with_items(catalog).unwrap();
    let mut realm = Realm::open(root).unwrap();
    realm
        .create(source, "127.0.0.1:4001".parse().unwrap(), 4, 0)
        .unwrap();
    realm
        .create(destination, "127.0.0.1:4002".parse().unwrap(), 4, 0)
        .unwrap();
    let character = realm
        .characters()
        .find(|(_, p, _)| *p == key(14).x_only_public_key().0.serialize())
        .unwrap()
        .0;
    let a = realm.acquire(1001, [1; 32], 0).unwrap();
    let b = realm.acquire(1002, [2; 32], 0).unwrap();
    (realm, a, b, character, receipt)
}
#[test]
fn admission_and_transfer_preserve_unrelated_live_sessions_and_replication() {
    use crate::service::replication::{Packet, Receiver};
    let directory = tempfile::tempdir().unwrap();
    let (mut realm, a, b, character, _) = transfer_fixture(&directory.path().join("realm"));
    let source = joining(realm.games.get_mut(&1001).unwrap(), 11);
    let destination = joining(realm.games.get_mut(&1002).unwrap(), 12);
    let moving = joining(realm.games.get_mut(&1001).unwrap(), 14);
    let source_admission = realm.games[&1001].admission(source).unwrap();
    let destination_admission = realm.games[&1002].admission(destination).unwrap();
    let mut receiver = Receiver::default();
    let mut project = |realm: &mut Realm, request_id: u64| {
        let request = Request {
            version: VERSION,
            request_id,
            body: Body::Replicate {
                ack: receiver.ack(),
            },
        };
        let reply: Response = serde_json::from_slice(
            &realm
                .dispatch(&a, source, 1, &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        let Reply::Replicated { packet } = reply.body else {
            panic!("Replication refused")
        };
        receiver
            .admit(&packet, 1001, reply.tick, &reply.control)
            .unwrap();
        packet
    };
    assert!(matches!(project(&mut realm, 1), Packet::Full { .. }));
    realm
        .admit(
            &a,
            key(15).x_only_public_key().0.serialize(),
            [4., 0., 0.],
            1,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&realm.games[&1001].admission(source).unwrap()).unwrap(),
        serde_json::to_vec(&source_admission).unwrap()
    );
    assert!(matches!(project(&mut realm, 2), Packet::Full { .. }));
    realm
        .transfer(&a, &b, character, [61; 16], [3., 0., 0.], 1)
        .unwrap();
    assert!(realm.games[&1001].admission(moving).is_err());
    assert_eq!(
        serde_json::to_vec(&realm.games[&1001].admission(source).unwrap()).unwrap(),
        serde_json::to_vec(&source_admission).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&realm.games[&1002].admission(destination).unwrap()).unwrap(),
        serde_json::to_vec(&destination_admission).unwrap()
    );
    assert!(matches!(project(&mut realm, 3), Packet::Full { .. }));
    assert_eq!(realm.games[&1001].replication_stats().full, 3);
    let command = source_admission
        .command(
            realm.games[&1001].game().authority_tick,
            crate::Intent::Move {
                axes: [1., 0.],
                yaw: 0.,
            },
        )
        .unwrap();
    realm
        .games
        .get_mut(&1001)
        .unwrap()
        .submit(source, command)
        .unwrap();
}

#[test]
fn transfer_moves_resources_receipts_and_placement_once_across_restarts() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, a, b, character, original) = transfer_fixture(&root);
    let old_connection = joining(realm.games.get_mut(&1001).unwrap(), 14);
    let old_life = realm.games[&1001]
        .admission(old_connection)
        .unwrap()
        .actor();
    realm.drain(&a, 1).unwrap();
    let receipt = realm
        .transfer(&a, &b, character, [1; 16], [3., 0., 0.], 2)
        .unwrap();
    assert_eq!(receipt.source, old_life);
    assert_eq!(realm.characters().count(), 3);
    assert!(
        realm.games[&1001]
            .game()
            .player_admission(old_life.actor)
            .is_none()
    );
    assert!(
        realm.games[&1001]
            .character_rewards(old_life.actor)
            .is_none()
    );
    assert!(realm.games[&1001].admission(old_connection).is_err());
    let g = realm.games.get_mut(&1002).unwrap();
    let snapshot = g.game().player_snapshot(receipt.destination).unwrap();
    assert_eq!((snapshot.player.hp, snapshot.player.mana), (125, 15));
    assert!(
        snapshot
            .abilities
            .iter()
            .find(|a| a.id == crate::rules::Spell::Firebolt)
            .unwrap()
            .cooldown_remaining
            > 0.
    );
    assert_eq!(
        g.character_rewards(receipt.destination.actor)
            .unwrap()
            .experience,
        900
    );
    assert_eq!(
        g.character_rewards(receipt.destination.actor)
            .unwrap()
            .items[&1],
        3
    );
    let connection = joining(g, 14);
    let admission = g.admission(connection).unwrap();
    assert_eq!(
        g.use_item(
            connection,
            receipt.destination,
            admission.epoch(),
            1,
            [55; 16]
        )
        .unwrap(),
        original
    );
    assert_eq!(
        g.character_rewards(receipt.destination.actor)
            .unwrap()
            .items[&1],
        3
    );
    let source = g
        .chamber
        .game
        .simulation
        .player_ids()
        .find(|id| *id != 0)
        .unwrap();
    g.chamber.game.simulation.spend_mana_for(source, 5).unwrap();
    let fresh = g
        .use_item(
            connection,
            receipt.destination,
            admission.epoch(),
            1,
            [56; 16],
        )
        .unwrap();
    assert_eq!(fresh.revision, original.revision + 1);
    assert_eq!(
        g.character_rewards(receipt.destination.actor)
            .unwrap()
            .items[&1],
        2
    );
    realm.checkpoint(&b, 3).unwrap();
    assert_eq!(
        realm
            .transfer(&a, &b, character, [1; 16], [3., 0., 0.], 3)
            .unwrap(),
        receipt
    );
    assert!(
        realm
            .transfer(&a, &b, character, [1; 16], [4., 0., 0.], 3)
            .is_err()
    );
    assert_eq!(realm.route(character, 3).unwrap().instance, 1002);
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let a = realm.acquire(1001, [1; 32], 4).unwrap();
    let b = realm.acquire(1002, [2; 32], 4).unwrap();
    assert_eq!(
        realm
            .transfer(&a, &b, character, [1; 16], [3., 0., 0.], 4)
            .unwrap(),
        receipt
    );
    assert_eq!(
        realm.games[&1002]
            .character_rewards(receipt.destination.actor)
            .unwrap()
            .items[&1],
        2
    );
    realm.drain(&b, 5).unwrap();
    // Draining worlds can evacuate to an open destination after explicit restart.
    realm.stop(&a, 5).unwrap();
    realm.restart(1001, 6).unwrap();
    let a = realm.acquire(1001, [1; 32], 6).unwrap();
    let back = realm
        .transfer(&b, &a, character, [2; 16], [2., 0., 0.], 6)
        .unwrap();
    assert_ne!(back.destination, old_life);
    let g = realm.games.get_mut(&1001).unwrap();
    let connection = joining(g, 14);
    let admission = g.admission(connection).unwrap();
    assert_eq!(
        g.use_item(connection, back.destination, admission.epoch(), 1, [56; 16])
            .unwrap(),
        fresh
    );
    assert_eq!(
        g.character_rewards(back.destination.actor).unwrap().items[&1],
        2
    );
}
#[test]
fn admission_capacity_and_transfer_refusals_leave_owned_states_in_place() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, a, b, character, _) = transfer_fixture(&root);
    let first = realm
        .admit(
            &b,
            key(15).x_only_public_key().0.serialize(),
            [4., 0., 0.],
            1,
        )
        .unwrap();
    assert_eq!(
        realm
            .admit(
                &b,
                key(15).x_only_public_key().0.serialize(),
                [4., 0., 0.],
                1
            )
            .unwrap(),
        first
    );
    assert!(
        realm
            .admit(
                &a,
                key(15).x_only_public_key().0.serialize(),
                [4., 0., 0.],
                1
            )
            .is_err()
    );
    let before = realm.games[&1001].checkpoint().unwrap();
    assert!(
        realm
            .transfer(&a, &b, character, [3; 16], [f32::NAN, 0., 0.], 1)
            .is_err()
    );
    assert_eq!(realm.games[&1001].checkpoint().unwrap(), before);
    realm.manifest.instances.get_mut(&1002).unwrap().capacity = 2;
    assert!(
        realm
            .transfer(&a, &b, character, [3; 16], [3., 0., 0.], 1)
            .is_err()
    );
    realm.manifest.instances.get_mut(&1002).unwrap().capacity = 4;
    realm.games.get_mut(&1002).unwrap().chamber.items.items[0].mana = 6;
    assert!(
        realm
            .transfer(&a, &b, character, [3; 16], [3., 0., 0.], 1)
            .is_err()
    );
    assert_eq!(realm.games[&1001].checkpoint().unwrap(), before);
    assert_eq!(realm.manifest.characters[&character].instance, 1001);
}
#[test]
fn transfer_crash_child() {
    let Ok(root) = std::env::var("VERSE_REALM_CHILD_ROOT") else {
        return;
    };
    let mut realm = Realm::open(Path::new(&root)).unwrap();
    let character = realm
        .characters()
        .find(|(_, p, _)| *p == key(14).x_only_public_key().0.serialize())
        .unwrap()
        .0;
    let a = realm.acquire(1001, [1; 32], 1).unwrap();
    let b = realm.acquire(1002, [2; 32], 1).unwrap();
    realm
        .transfer(&a, &b, character, [1; 16], [3., 0., 0.], 2)
        .unwrap();
    panic!("Transfer crash boundary did not fire");
}
#[test]
fn every_transfer_publication_boundary_selects_one_owner_and_inventory() {
    for stage in [
        "before_snapshots",
        "after_snapshots",
        "before_seal",
        "after_seal",
        "after_directory_sync",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("realm");
        let (realm, _, _, character, original) = transfer_fixture(&root);
        drop(realm);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::realm::tests::transfer_crash_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("VERSE_REALM_CHILD_ROOT", &root)
            .env("VERSE_REALM_CRASH_AT", format!("transfer_{stage}"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(86),
            "{stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut realm = Realm::open(&root).unwrap();
        let sealed = matches!(stage, "after_seal" | "after_directory_sync");
        assert_eq!(
            realm.manifest.characters[&character].instance,
            if sealed { 1002 } else { 1001 },
            "{stage}"
        );
        assert_eq!(realm.characters().count(), 3);
        let placement = &realm.manifest.characters[&character];
        assert_eq!(
            realm.games[&placement.instance]
                .character_rewards(placement.actor)
                .unwrap()
                .items[&1],
            3
        );
        let a = realm.acquire(1001, [1; 32], 3).unwrap();
        let b = realm.acquire(1002, [2; 32], 3).unwrap();
        let receipt = realm
            .transfer(&a, &b, character, [1; 16], [3., 0., 0.], 4)
            .unwrap();
        assert_eq!(realm.manifest.transfers, 1);
        assert_eq!(
            realm
                .transfer(&a, &b, character, [1; 16], [3., 0., 0.], 4)
                .unwrap(),
            receipt
        );
        let g = realm.games.get_mut(&1002).unwrap();
        let connection = joining(g, 14);
        let admission = g.admission(connection).unwrap();
        assert_eq!(
            g.use_item(
                connection,
                receipt.destination,
                admission.epoch(),
                1,
                [55; 16]
            )
            .unwrap(),
            original
        );
        assert_eq!(
            g.character_rewards(receipt.destination.actor)
                .unwrap()
                .items[&1],
            3
        );
        println!(
            "realm transfer crash {stage}: {} owner, exact retry, three items",
            if sealed { "destination" } else { "source" }
        );
    }
}
#[tokio::test]
async fn real_tls_listeners_route_transfer_and_reauthenticate_the_sdk() {
    use crate::service::{client::Client, net::tests::tls};
    use rustls::pki_types::ServerName;
    use tokio::{net::TcpListener, sync::oneshot};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, _, _, character, original) = transfer_fixture(&root);
    let left = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let right = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let left_address = left.local_addr().unwrap();
    let right_address = right.local_addr().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let a = realm.acquire(1001, [1; 32], now).unwrap();
    let b = realm.acquire(1002, [2; 32], now).unwrap();
    realm.set_endpoint(&a, left_address, now).unwrap();
    realm.set_endpoint(&b, right_address, now).unwrap();
    let (tls, connector) = tls();
    let (stop, shutdown) = oneshot::channel();
    let (control, commands) = net::channel();
    let server = tokio::spawn(net::serve(
        realm,
        vec![(a, left), (b, right)],
        tls,
        commands,
        async {
            let _ = shutdown.await;
        },
    ));
    let mut source = Client::connect_with_content(
        left_address,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        1001,
        Some([9; 32]),
        &key(14),
    )
    .await
    .unwrap();
    assert_eq!(
        control.route(character).await.unwrap().endpoint,
        left_address
    );
    let old_life = source.control().unwrap().life;
    control.drain(1001).await.unwrap();
    assert!(control.route(character).await.is_err());
    let transfer = control
        .transfer(1001, 1002, character, [9; 16], [3., 0., 0.])
        .await
        .unwrap();
    assert_eq!(
        control.route(character).await.unwrap().endpoint,
        right_address
    );
    assert!(source.snapshot().await.is_err());
    let mut destination = Client::connect_with_content(
        right_address,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        1002,
        Some([9; 32]),
        &key(14),
    )
    .await
    .unwrap();
    assert_ne!(destination.control().unwrap().life, old_life);
    assert_eq!(
        LifeId::from(destination.control().unwrap().life),
        transfer.destination
    );
    assert!(destination.snapshot().await.unwrap().scope.is_some());
    let inventory = destination.inventory().await.unwrap();
    assert_eq!(inventory.items.iter().find(|i| i.id == 1).unwrap().count, 3);
    let retry = destination.use_item(1, [55; 16]).await.unwrap();
    assert!(
        matches!(retry.body, Reply::ItemUsed { revision, item: 1, operation } if revision == original.revision && operation == [55; 16])
    );
    assert_eq!(
        destination
            .inventory()
            .await
            .unwrap()
            .items
            .iter()
            .find(|i| i.id == 1)
            .unwrap()
            .count,
        3
    );
    destination.close().await.unwrap();
    stop.send(()).unwrap();
    let exit = server.await.unwrap().unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
    assert!(exit.stats.requests >= 7);
    assert!(exit.stats.queue_peak <= 128);
    assert_eq!(
        (exit.stats.admission.pending, exit.stats.admission.active),
        (0, 0)
    );
    assert!(
        exit.realm
            .manifest
            .instances
            .values()
            .all(|slot| slot.owner.is_none() && slot.expires_ms == 0)
    );
    println!("realm TLS acceptance: {:?}", exit.stats);
    if let Ok(path) = std::env::var("VERSE_REALM_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({ "schema": "verse.realm.tls.acceptance.v1", "instances": 2, "owned_characters": 3, "transfer_operation": "retained 16-byte identity", "source_instance": 1001, "destination_instance": 1002, "remaining_recovery_items": 3, "original_item_revision": original.revision, "source_sessions_fenced": true, "routed_sdk_reauthentication": true, "exact_retry_without_debit": true, "stats": exit.stats })).unwrap()).unwrap();
    }

    drop(exit);
    let realm = Realm::open(&root).unwrap();
    assert_eq!(realm.manifest.characters[&character].instance, 1002);
}
#[test]
fn transfer_index_branches_and_original_receipts_survive_repeated_placements() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, a, b, character, original) = transfer_fixture(&root);
    let mut last = None;
    for index in 1..=20u8 {
        let (source, destination) = if index % 2 == 1 { (&a, &b) } else { (&b, &a) };
        last = Some(
            realm
                .transfer(
                    source,
                    destination,
                    character,
                    [index; 16],
                    [3., 0., 0.],
                    index as u64,
                )
                .unwrap(),
        );
    }
    assert_eq!(realm.manifest.transfers, 20);
    transfer::validate(&realm).unwrap();
    let last = last.unwrap();
    assert_eq!(last.destination.instance, 1001);
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let a = realm.acquire(1001, [1; 32], 21).unwrap();
    let b = realm.acquire(1002, [2; 32], 21).unwrap();
    assert_eq!(
        realm
            .transfer(&b, &a, character, [20; 16], [3., 0., 0.], 21)
            .unwrap(),
        last
    );
    let g = realm.games.get_mut(&1001).unwrap();
    let connection = joining(g, 14);
    let admission = g.admission(connection).unwrap();
    assert_eq!(
        g.use_item(connection, last.destination, admission.epoch(), 1, [55; 16])
            .unwrap(),
        original
    );
    assert_eq!(
        g.character_rewards(last.destination.actor).unwrap().items[&1],
        3
    );
}
#[test]
fn character_receipt_branches_recover_original_revisions_after_transfer() {
    use crate::service::rewards::Transaction;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, a, b, character, _) = transfer_fixture(&root);
    let actor = realm.manifest.characters[&character].actor;
    let mut first = None;
    for index in 1..=40u8 {
        let transaction = Transaction {
            instance: 1001,
            actor,
            source: [index; 32],
            experience: 1,
            items: vec![],
            quests: vec![],
            spent: vec![],
            acceptance: None,
            outfit: None,
            equipment: None,
        };
        let receipt = realm
            .grant_reward(&a, character, transaction, index as u64)
            .unwrap();
        if index == 1 {
            first = Some(receipt);
        }
    }
    let transfer = realm
        .transfer(&a, &b, character, [41; 16], [3., 0., 0.], 41)
        .unwrap();
    let transaction = Transaction {
        actor: transfer.destination.actor,
        instance: 1002,
        ..first.as_ref().unwrap().transaction.clone()
    };
    assert_eq!(
        realm
            .grant_reward(&b, character, transaction.clone(), 42)
            .unwrap(),
        first.clone().unwrap()
    );
    assert_eq!(
        realm.games[&1002]
            .character_rewards(transfer.destination.actor)
            .unwrap()
            .experience,
        940
    );
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let b = realm.acquire(1002, [2; 32], 43).unwrap();
    assert_eq!(
        realm.grant_reward(&b, character, transaction, 43).unwrap(),
        first.unwrap()
    );
    assert_eq!(
        realm.games[&1002]
            .character_rewards(transfer.destination.actor)
            .unwrap()
            .experience,
        940
    );
}
#[test]
fn version_nine_uses_its_original_schema_and_refuses_new_realm_books() {
    let gateway = gateway(1001, 11);
    let mut saved: serde_json::Value =
        serde_json::from_slice(&gateway.checkpoint().unwrap()).unwrap();
    saved["version"] = 9.into();
    saved["character_schema"] = 2.into();
    assert!(
        super::super::save::decode(&serde_json::to_vec(&saved).unwrap(), [9; 32], 1001).is_ok()
    );
    saved["character_schema"] = 3.into();
    assert!(
        super::super::save::decode(&serde_json::to_vec(&saved).unwrap(), [9; 32], 1001).is_err()
    );
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (realm, _, _, _, _) = transfer_fixture(&root);
    let mut saved: serde_json::Value =
        serde_json::from_slice(&realm.games[&1001].checkpoint().unwrap()).unwrap();
    saved["version"] = 9.into();
    saved["character_schema"] = 2.into();
    assert!(
        super::super::save::decode_with_history(
            &serde_json::to_vec(&saved).unwrap(),
            [9; 32],
            1001,
            Some(realm.history.clone())
        )
        .is_err()
    );
}

#[tokio::test]
async fn real_tls_social_profiles_share_interactions_and_fence_zone_transfers() {
    use crate::{
        play::social::{Action, SeatActor, Zone, tests::game},
        service::{
            client::Client,
            net::tests::tls,
            wire::{Body, Input},
        },
    };
    use rustls::pki_types::ServerName;
    use tokio::{net::TcpListener, sync::oneshot};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let mut realm = Realm::open(&root).unwrap();
    for (instance, zone, n) in [(2001, Zone::Plaza, 11), (2002, Zone::Everglade, 12)] {
        let mut g = Gateway::new(Chamber::new(game(instance, zone)).unwrap())
            .unwrap()
            .with_content([8; 32])
            .unwrap();
        g.enroll_primary(key(n).x_only_public_key().0.serialize())
            .unwrap();
        g.enroll_spectator(key(15).x_only_public_key().0.serialize())
            .unwrap();
        realm
            .create(g, "127.0.0.1:1".parse().unwrap(), 8, 0)
            .unwrap();
    }
    let initial = realm.acquire(2001, [3; 32], 1).unwrap();
    let (character, original_life) = realm
        .admit(
            &initial,
            key(14).x_only_public_key().0.serialize(),
            [0., 0., 1.],
            1,
        )
        .unwrap();
    realm.release(&initial, 1).unwrap();
    let left = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let right = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let left_address = left.local_addr().unwrap();
    let right_address = right.local_addr().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let a = realm.acquire(2001, [1; 32], now).unwrap();
    let b = realm.acquire(2002, [2; 32], now).unwrap();
    realm.set_endpoint(&a, left_address, now).unwrap();
    realm.set_endpoint(&b, right_address, now).unwrap();
    let (tls, connector) = tls();
    let (stop, shutdown) = oneshot::channel();
    let (control, commands) = net::channel();
    let server = tokio::spawn(net::serve(
        realm,
        vec![(a, left), (b, right)],
        tls,
        commands,
        async {
            let _ = shutdown.await;
        },
    ));
    let mut player = Client::connect_with_content(
        left_address,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        2001,
        Some([8; 32]),
        &key(14),
    )
    .await
    .unwrap();
    let mut viewer = Client::connect_with_content(
        left_address,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        2001,
        Some([8; 32]),
        &key(15),
    )
    .await
    .unwrap();
    player.snapshot().await.unwrap();
    assert!(matches!(
        player.social(Action::Sit { object: 1 }).await.unwrap().body,
        Reply::Accepted
    ));
    let first = player.snapshot().await.unwrap();
    let other = viewer.snapshot().await.unwrap();
    assert_eq!(first.social, other.social);
    assert_eq!(
        first.social.as_ref().unwrap().occupant(1),
        Some(original_life)
    );
    let foreign = Body::Social {
        input: crate::play::social::Input {
            life: original_life,
            epoch: player.control().unwrap().epoch,
            sequence: 1,
            tick: player.tick(),
            action: Action::Stand {},
        },
    };
    assert!(matches!(
        viewer.request(foreign).await.unwrap().body,
        Reply::Refused { .. }
    ));
    assert!(matches!(
        player
            .social(Action::Toggle { object: 2 })
            .await
            .unwrap()
            .body,
        Reply::Accepted
    ));
    control
        .publish_social_studio(
            2001,
            vec![SeatActor {
                seat: 3,
                feet: [-1., 0., 0.],
                yaw: 1.,
            }],
        )
        .await
        .unwrap();
    let current = player.snapshot().await.unwrap();
    let other = viewer.snapshot().await.unwrap();
    assert_eq!(current.social, other.social);
    assert!(current.social.as_ref().unwrap().switch_on(2));
    assert_eq!(current.social.as_ref().unwrap().studio.len(), 1);
    let old_input = Input {
        actor: player.control().unwrap().life,
        epoch: player.control().unwrap().epoch,
        sequence: player.control().unwrap().accepted_sequence + 1,
        tick: player.tick(),
        intent: crate::service::wire::Action::Move {
            axes: [1., 0.],
            yaw: 0.,
        },
    };
    let transfer = control
        .transfer(2001, 2002, character, [42; 16], [0., 0., 1.])
        .await
        .unwrap();
    assert!(player.snapshot().await.is_err());
    viewer.snapshot().await.unwrap();
    let mut destination = Client::connect_with_content(
        right_address,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        2002,
        Some([8; 32]),
        &key(14),
    )
    .await
    .unwrap();
    let mut destination_viewer = Client::connect_with_content(
        right_address,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        2002,
        Some([8; 32]),
        &key(15),
    )
    .await
    .unwrap();
    assert!(matches!(
        destination
            .request(Body::Command { command: old_input })
            .await
            .unwrap()
            .body,
        Reply::Refused { .. }
    ));
    let state = destination.snapshot().await.unwrap();
    let other = destination_viewer.snapshot().await.unwrap();
    assert_eq!(state.social, other.social);
    assert_eq!(state.social.as_ref().unwrap().profile.zone, Zone::Everglade);
    assert_eq!(
        LifeId::from(destination.control().unwrap().life),
        transfer.destination
    );
    assert_eq!(
        control.route(character).await.unwrap().endpoint,
        right_address
    );
    assert!(matches!(
        destination
            .social(Action::Sit { object: 1 })
            .await
            .unwrap()
            .body,
        Reply::Accepted
    ));
    assert_eq!(
        destination.snapshot().await.unwrap().social,
        destination_viewer.snapshot().await.unwrap().social
    );
    destination.close().await.unwrap();
    destination_viewer.close().await.unwrap();
    stop.send(()).unwrap();
    let exit = server.await.unwrap().unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
    assert_eq!(
        (exit.stats.admission.pending, exit.stats.admission.active),
        (0, 0)
    );
    println!(
        "social TLS acceptance: two profiles/viewers, seat/switch/Studio convergence, spectator refusal, stable character {character}, transfer fencing; {:?}",
        exit.stats
    );
    drop(exit);
    let recovered = Realm::open(&root).unwrap();
    assert_eq!(
        recovered
            .characters()
            .find(|(id, _, _)| *id == character)
            .unwrap()
            .2,
        transfer.destination
    );
    assert_eq!(
        recovered.games[&2001]
            .game()
            .social_state()
            .unwrap()
            .studio
            .len(),
        1
    );
    assert!(
        recovered.games[&2001]
            .game()
            .social_state()
            .unwrap()
            .switch_on(2)
    );
}

#[test]
fn primary_character_logout_resume_and_key_recovery_preserve_owned_receipts() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, a, b, character, original) = transfer_fixture(&root);
    let principal = key(14).x_only_public_key().0.serialize();
    let owner = realm.account_for_key(principal).unwrap().unwrap();
    let stale = joining(realm.games.get_mut(&1001).unwrap(), 14);
    let old_life = realm.games[&1001].admission(stale).unwrap().actor();
    let other = joining(realm.games.get_mut(&1001).unwrap(), 11);
    let other_admission = realm.games[&1001].admission(other).unwrap();
    assert!(
        realm
            .logout(&a, key(12).x_only_public_key().0.serialize(), character, 1)
            .is_err()
    );
    realm.logout(&a, principal, character, 1).unwrap();
    realm.logout(&a, principal, character, 1).unwrap();
    assert_eq!(realm.characters().count(), 2);
    assert!(realm.games[&1001].admission(stale).is_err());
    assert_eq!(
        realm.games[&1001].admission(other).unwrap().epoch(),
        other_admission.epoch()
    );
    assert!(
        realm.games[&1001]
            .character_rewards(old_life.actor)
            .is_none()
    );
    assert!(matches!(
        realm.character(character).unwrap().residence,
        Residence::Dormant { .. }
    ));
    // Another account can occupy the freed physical slot without receiving its inventory.
    let (fresh_id, fresh_life) = realm
        .admit(
            &a,
            key(15).x_only_public_key().0.serialize(),
            [4., 0., 0.],
            1,
        )
        .unwrap();
    assert_ne!(fresh_id, character);
    assert_eq!(fresh_life.actor, old_life.actor);
    assert!(fresh_life.generation > old_life.generation);
    assert!(
        realm.games[&1001]
            .character_rewards(fresh_life.actor)
            .unwrap()
            .items
            .is_empty()
    );
    let new_key = key(16).x_only_public_key().0.serialize();
    let recovered = realm
        .recover_account(&[a.clone()], owner.id, owner.epoch, new_key, 1)
        .unwrap();
    assert_eq!(recovered.id, owner.id);
    assert_eq!(recovered.characters, vec![character]);
    assert_eq!(recovered.epoch, owner.epoch + 1);
    assert!(realm.account_for_key(principal).is_err());
    assert!(
        realm
            .resume(&b, principal, character, [3., 0., 0.], 1)
            .is_err()
    );
    assert!(realm.admit(&b, principal, [3., 0., 0.], 1).is_err());
    assert!(
        realm
            .recover_account(
                &[a.clone()],
                owner.id,
                owner.epoch,
                key(17).x_only_public_key().0.serialize(),
                1
            )
            .is_err()
    );
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let a = realm.acquire(1001, [1; 32], 2).unwrap();
    let b = realm.acquire(1002, [2; 32], 2).unwrap();
    let life = realm
        .resume(&b, new_key, character, [3., 0., 0.], 2)
        .unwrap();
    assert_eq!(
        realm
            .resume(&b, new_key, character, [3., 0., 0.], 2)
            .unwrap(),
        life
    );
    let g = realm.games.get_mut(&1002).unwrap();
    let connection = joining(g, 16);
    let admission = g.admission(connection).unwrap();
    let snapshot = g.game().player_snapshot(life).unwrap();
    assert_eq!((snapshot.player.hp, snapshot.player.mana), (125, 15));
    assert_eq!(
        g.use_item(connection, life, admission.epoch(), 1, [55; 16])
            .unwrap(),
        original
    );
    assert_eq!(g.character_rewards(life.actor).unwrap().items[&1], 3);
    // The authored primary is now an ordinary persistent character as well.
    let primary_key = key(11).x_only_public_key().0.serialize();
    let primary = realm
        .characters()
        .find(|(_, p, _)| *p == primary_key)
        .unwrap()
        .0;
    let primary_actor = realm.manifest.characters[&primary].actor;
    let transfer = realm
        .transfer(&a, &b, primary, [62; 16], [5., 0., 0.], 2)
        .unwrap();
    assert_eq!(transfer.source.actor, primary_actor);
    assert!(
        realm.games[&1001]
            .game()
            .player_admission(transfer.source.actor)
            .is_none()
    );
    assert!(
        !realm.games[&1001]
            .game()
            .frame()
            .actors
            .iter()
            .any(|a| a.actor.id == transfer.source.actor)
    );
    assert!(
        !realm.games[&1001]
            .game()
            .snapshot()
            .actors
            .iter()
            .any(|a| a.id == 0)
    );
    realm.tick(&a, 3, 1. / 30.).unwrap();
    realm.checkpoint(&a, 3).unwrap();
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let a = realm.acquire(1001, [1; 32], 4).unwrap();
    assert!(
        realm.games[&1001]
            .game()
            .player_admission(transfer.source.actor)
            .is_none()
    );
    realm.games.get_mut(&1001).unwrap().reset().unwrap();
    realm.checkpoint(&a, 4).unwrap();
    assert!(
        realm.games[&1001]
            .game()
            .player_admission(transfer.source.actor)
            .is_none()
    );
    assert_eq!(realm.character(primary).unwrap().id, primary);
}

#[tokio::test]
async fn real_tls_sdk_account_selection_logout_and_operator_recovery() {
    use crate::service::{client::Client, net::tests::tls};
    use rustls::pki_types::ServerName;
    use tokio::{net::TcpListener, sync::oneshot};
    let directory = tempfile::tempdir().unwrap();
    let (mut realm, _, _, character, original) = transfer_fixture(&directory.path().join("realm"));
    let account = realm
        .account_for_key(key(14).x_only_public_key().0.serialize())
        .unwrap()
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let lease = realm.acquire(1001, [1; 32], now).unwrap();
    realm.set_endpoint(&lease, address, now).unwrap();
    let (tls, connector) = tls();
    let (stop, shutdown) = oneshot::channel();
    let (control, commands) = net::channel();
    let server = tokio::spawn(net::serve(
        realm,
        vec![(lease, listener)],
        tls,
        commands,
        async {
            let _ = shutdown.await;
        },
    ));
    let connect = |n| {
        let config = connector.config().clone();
        async move {
            Client::connect_with_content(
                address,
                ServerName::try_from("localhost").unwrap(),
                config,
                1001,
                Some([9; 32]),
                &key(n),
            )
            .await
        }
    };
    let mut resident = connect(14).await.unwrap();
    let old = resident.control().unwrap().clone();
    assert!(
        matches!(resident.logout().await.unwrap().body, Reply::LoggedOut { character: c } if c == character)
    );
    assert!(!resident.connected());
    assert!(control.route(character).await.is_err());
    let mut observer = connect(11).await.unwrap();
    let observer_epoch = observer.control().unwrap().epoch;
    observer.snapshot().await.unwrap();
    let mut account_client = connect(14).await.unwrap();
    assert!(account_client.control().is_none());
    assert_eq!(
        account_client.account().await.unwrap().characters,
        vec![character]
    );
    account_client.snapshot().await.unwrap();
    assert!(matches!(
        account_client.select_character(1).await.unwrap().body,
        Reply::Refused { .. }
    ));
    assert!(account_client.connected());
    let selected = account_client.select_character(character).await.unwrap();
    assert!(
        matches!(selected.body, Reply::CharacterSelected { character: c } if c == character),
        "{:?}",
        selected.body
    );
    let new = account_client.control().unwrap().clone();
    assert!(new.life.actor != old.life.actor || new.life.generation > old.life.generation);
    assert_eq!(
        account_client
            .inventory()
            .await
            .unwrap()
            .items
            .iter()
            .find(|i| i.id == 1)
            .unwrap()
            .count,
        3
    );
    assert!(
        matches!(account_client.use_item(1, [55; 16]).await.unwrap().body, Reply::ItemUsed { revision, .. } if revision == original.revision)
    );
    assert!(matches!(
        account_client
            .request(Body::Logout {
                life: old.life,
                epoch: old.epoch
            })
            .await
            .unwrap()
            .body,
        Reply::Refused { .. }
    ));
    observer.snapshot().await.unwrap();
    assert_eq!(observer.control().unwrap().epoch, observer_epoch);
    let recovered = control
        .recover_account(
            account.id,
            account.epoch,
            key(16).x_only_public_key().0.serialize(),
        )
        .await
        .unwrap();
    assert_eq!(recovered.characters, vec![character]);
    assert_eq!(
        control.account(account.id).await.unwrap().epoch,
        recovered.epoch
    );
    assert!(account_client.snapshot().await.is_err());
    assert!(connect(14).await.is_err());
    let mut fresh = connect(16).await.unwrap();
    assert_eq!(
        fresh
            .inventory()
            .await
            .unwrap()
            .items
            .iter()
            .find(|i| i.id == 1)
            .unwrap()
            .count,
        3
    );
    fresh.logout().await.unwrap();
    let (second, _) = control
        .create_character(
            1001,
            key(16).x_only_public_key().0.serialize(),
            [4., 0., 0.],
        )
        .await
        .unwrap();
    assert_ne!(second, character);
    let mut fresh = connect(16).await.unwrap();
    assert!(fresh.inventory().await.unwrap().items.is_empty());
    fresh.logout().await.unwrap();
    let mut fresh = connect(16).await.unwrap();
    fresh.select_character(character).await.unwrap();
    assert_eq!(
        fresh
            .inventory()
            .await
            .unwrap()
            .items
            .iter()
            .find(|i| i.id == 1)
            .unwrap()
            .count,
        3
    );
    fresh.close().await.unwrap();
    observer.close().await.unwrap();
    stop.send(()).unwrap();
    let exit = server.await.unwrap().unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
    assert_eq!(exit.realm.account(account.id).unwrap().characters.len(), 2);
    println!(
        "TLS lifecycle: durable logout, owned selection, stale command refusal, live recovery, retired key refusal, two independent character inventories"
    );
}

#[test]
fn lifecycle_crash_child() {
    let Ok(root) = std::env::var("VERSE_LIFECYCLE_CHILD_ROOT") else {
        return;
    };
    let mut realm = Realm::open(Path::new(&root)).unwrap();
    let principal = key(14).x_only_public_key().0.serialize();
    let account = realm.account_for_key(principal).unwrap().unwrap();
    let character = account.characters[0];
    let a = realm.acquire(1001, [1; 32], 1).unwrap();
    let b = realm.acquire(1002, [2; 32], 1).unwrap();
    match std::env::var("VERSE_LIFECYCLE_ACTION").unwrap().as_str() {
        "logout" => realm.logout(&a, principal, character, 2).unwrap(),
        "resume" => {
            realm
                .resume(&b, principal, character, [3., 0., 0.], 2)
                .unwrap();
        }
        "recover" => {
            realm
                .recover_account(
                    &[a],
                    account.id,
                    account.epoch,
                    key(16).x_only_public_key().0.serialize(),
                    2,
                )
                .unwrap();
        }
        _ => panic!("Unknown lifecycle crash action"),
    }
    panic!("Lifecycle crash boundary did not fire");
}

#[test]
fn lifecycle_crashes_select_one_account_character_and_exact_inventory() {
    for action in ["logout", "resume", "recover"] {
        for stage in [
            "before_snapshots",
            "after_snapshots",
            "before_seal",
            "after_seal",
            "after_directory_sync",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().join("realm");
            let (mut realm, a, _, character, original) = transfer_fixture(&root);
            let principal = key(14).x_only_public_key().0.serialize();
            let account = realm.account_for_key(principal).unwrap().unwrap();
            if action == "resume" {
                realm.logout(&a, principal, character, 0).unwrap();
            }
            drop(realm);
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "service::realm::tests::lifecycle_crash_child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("VERSE_LIFECYCLE_CHILD_ROOT", &root)
                .env("VERSE_LIFECYCLE_ACTION", action)
                .env("VERSE_REALM_CRASH_AT", format!("lifecycle_{stage}"))
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(86),
                "{action}/{stage}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let mut realm = Realm::open(&root).unwrap();
            let sealed = matches!(stage, "after_seal" | "after_directory_sync");
            let a = realm.acquire(1001, [1; 32], 3).unwrap();
            let b = realm.acquire(1002, [2; 32], 3).unwrap();
            let restored = realm.character(character).unwrap();
            let mut current_key = principal;
            match action {
                "logout" => {
                    assert_eq!(
                        matches!(restored.residence, Residence::Dormant { .. }),
                        sealed
                    );
                    realm.logout(&a, principal, character, 4).unwrap();
                    realm.logout(&a, principal, character, 4).unwrap();
                    realm
                        .resume(&b, principal, character, [3., 0., 0.], 4)
                        .unwrap();
                }
                "resume" => {
                    assert_eq!(
                        matches!(
                            restored.residence,
                            Residence::Resident { instance: 1002, .. }
                        ),
                        sealed
                    );
                    let life = realm
                        .resume(&b, principal, character, [3., 0., 0.], 4)
                        .unwrap();
                    assert_eq!(
                        realm
                            .resume(&b, principal, character, [3., 0., 0.], 4)
                            .unwrap(),
                        life
                    );
                }
                "recover" => {
                    let selected = realm.account(account.id).unwrap();
                    assert_eq!(selected.epoch, account.epoch + u64::from(sealed));
                    current_key = key(16).x_only_public_key().0.serialize();
                    if !sealed {
                        realm
                            .recover_account(
                                &[a.clone()],
                                account.id,
                                account.epoch,
                                current_key,
                                4,
                            )
                            .unwrap();
                    }
                    assert!(
                        realm
                            .recover_account(
                                &[a.clone()],
                                account.id,
                                account.epoch,
                                current_key,
                                4
                            )
                            .is_err()
                    );
                    assert!(realm.account_for_key(principal).is_err());
                }
                _ => unreachable!(),
            }
            assert_eq!(
                realm.account_for_key(current_key).unwrap().unwrap().id,
                account.id
            );
            let placement = &realm.manifest.characters[&character];
            let gateway = realm.games.get_mut(&placement.instance).unwrap();
            let connection = joining(gateway, if action == "recover" { 16 } else { 14 });
            let admission = gateway.admission(connection).unwrap();
            assert_eq!(
                gateway
                    .use_item(
                        connection,
                        admission.actor(),
                        admission.epoch(),
                        1,
                        [55; 16]
                    )
                    .unwrap(),
                original
            );
            assert_eq!(
                gateway
                    .character_rewards(admission.actor().actor)
                    .unwrap()
                    .items[&1],
                3
            );
            println!(
                "lifecycle crash {action}/{stage}: one account, one character owner, original receipt, three items"
            );
        }
    }
}

#[test]
fn resident_slots_recycle_across_eighty_distinct_persistent_accounts() {
    let directory = tempfile::tempdir().unwrap();
    let (mut realm, a, _, _, _) = transfer_fixture(&directory.path().join("realm"));
    let records = realm.games[&1001].game().physics_bodies().records().count();
    let mut first = None;
    for n in 17..97 {
        let principal = key(n).x_only_public_key().0.serialize();
        let (character, life) = realm.admit(&a, principal, [4., 0., 0.], 1).unwrap();
        if first.is_none() {
            first = Some((character, principal));
        }
        assert!(realm.characters().count() <= 4);
        realm.logout(&a, principal, character, 1).unwrap();
        assert!(matches!(
            realm.character(character).unwrap().residence,
            Residence::Dormant { .. }
        ));
        assert!(
            realm.games[&1001]
                .game()
                .player_admission(life.actor)
                .is_none()
        );
        assert!(realm.games[&1001].game().physics_bodies().records().count() <= records + 1);
    }
    let (character, principal) = first.unwrap();
    let life = realm
        .resume(&a, principal, character, [4., 0., 0.], 1)
        .unwrap();
    assert_eq!(
        realm.games[&1001]
            .chamber
            .rewards
            .realm_character(life.actor),
        Some(character)
    );
    println!(
        "80 distinct accounts: bounded resident placements/body records; first dormant account resumes with its original character ID"
    );
}

#[test]
fn legacy_realm_head_upgrades_ownership_without_relabeling_characters() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let (mut realm, _, _, character, original) = transfer_fixture(&root);
    let before: Vec<_> = realm
        .characters()
        .map(|(id, key, life)| (id, key, life.actor))
        .collect();
    realm.manifest.version = 1;
    realm.manifest.next_account = 0;
    realm.manifest.registry_root = None;
    for instance in [1001, 1002] {
        let mut saved: serde_json::Value =
            serde_json::from_slice(&realm.games[&instance].checkpoint().unwrap()).unwrap();
        let mut world: serde_json::Value =
            serde_json::from_str(saved["world"].as_str().unwrap()).unwrap();
        world["rules_revision"] = "verse-chamber-owned-v21".into();
        let scene = world["world"]["scene"].as_object_mut().unwrap();
        let origin = scene.remove("origin").unwrap();
        scene.insert("origin_wow".into(), origin);
        saved["world"] = serde_json::to_string(&world).unwrap().into();
        let hash = disk::store_snapshot(&realm, &serde_json::to_vec(&saved).unwrap()).unwrap();
        realm
            .manifest
            .instances
            .get_mut(&instance)
            .unwrap()
            .snapshot = hash;
    }
    realm.publish(&[]).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("realm.json")).unwrap()).unwrap();
    assert!(value["manifest"].get("registry_root").is_none());
    assert!(value["manifest"].get("next_account").is_none());
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    assert_eq!(realm.manifest.version, 2);
    assert_eq!(
        realm
            .characters()
            .map(|(id, key, life)| (id, key, life.actor))
            .collect::<Vec<_>>(),
        before
    );
    let account = realm
        .account_for_key(key(14).x_only_public_key().0.serialize())
        .unwrap()
        .unwrap();
    assert_eq!(account.characters, vec![character]);
    let gateway = realm.games.get_mut(&1001).unwrap();
    let connection = joining(gateway, 14);
    let a = gateway.admission(connection).unwrap();
    assert_eq!(
        gateway
            .use_item(connection, a.actor(), a.epoch(), 1, [55; 16])
            .unwrap(),
        original
    );
}

#[test]
fn registry_retains_more_than_the_resident_manifest_budget_with_bounded_records() {
    let directory = tempfile::tempdir().unwrap();
    let mut realm = Realm::open(&directory.path().join("realm")).unwrap();
    let principal = key(19).x_only_public_key().0.serialize();
    let records = (1..=4096)
        .map(|id| {
            registry::Record::Account(Account {
                id,
                epoch: 1,
                key: principal,
                characters: vec![id],
            })
        })
        .collect();
    let root = registry::put(&realm, None, records).unwrap();
    realm.manifest.registry_root = root;
    realm.manifest.next_account = 4097;
    realm.manifest.next_character = 4097;
    assert!(realm.manifest.characters.is_empty());
    for id in [1, 8, 2048, 2049, 4096] {
        assert_eq!(realm.account(id).unwrap().id, id);
    }
    let bytes = serde_json::to_vec(&realm.manifest).unwrap();
    assert!(bytes.len() < 1024);
    println!(
        "4,096 account records: bounded SHA-addressed registry nodes; empty resident table; realm head {} bytes",
        bytes.len()
    );
}

#[test]
fn account_observation_is_connection_scoped_and_does_not_retain_grants() {
    let directory = tempfile::tempdir().unwrap();
    let (mut realm, a, _, character, _) = transfer_fixture(&directory.path().join("realm"));
    let k = key(14);
    let public_key = k.x_only_public_key().0.serialize();
    realm.logout(&a, public_key, character, 1).unwrap();
    let principal = super::super::Principal(public_key);
    for request_id in 1..=130 {
        let (connection, hello) = realm.open_connection(&a, 1).unwrap();
        let hello: Hello = serde_json::from_slice(&hello).unwrap();
        let signature = secp256k1::Secp256k1::new()
            .sign_schnorr_no_aux_rand(&hello.challenge.signing_digest(public_key), &k)
            .to_byte_array()
            .to_vec();
        let request = Request {
            version: VERSION,
            request_id,
            body: Body::Authenticate {
                public_key,
                signature,
            },
        };
        let response: Response = serde_json::from_slice(
            &realm
                .dispatch(&a, connection, 1, &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(matches!(response.body, Reply::Accepted));
        assert!(response.control.is_none());
        let gateway = &realm.games[&1001];
        assert!(gateway.account_observers.contains(&principal));
        let restored = super::super::save::decode_with_history(
            &gateway.checkpoint().unwrap(),
            [9; 32],
            1001,
            Some(realm.history.clone()),
        )
        .unwrap();
        assert!(!restored.chamber.grants.contains_key(&principal));
        realm.close_connection(&a, connection, 1).unwrap();
        assert!(!realm.games[&1001].chamber.grants.contains_key(&principal));
    }
}

#[test]
fn defeated_logout_keeps_resources_and_account_character_limits_refuse_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let (mut realm, a, b, character, _) = transfer_fixture(&directory.path().join("realm"));
    let principal = key(14).x_only_public_key().0.serialize();
    let account = realm.account_for_key(principal).unwrap().unwrap();
    let gateway = realm.games.get_mut(&1001).unwrap();
    let source = gateway
        .chamber
        .game
        .simulation
        .player_ids()
        .find(|id| *id != 0)
        .unwrap();
    gateway
        .chamber
        .game
        .simulation
        .player_damage_for(source, 1000)
        .unwrap();
    gateway.tick(0.).unwrap();
    realm.logout(&a, principal, character, 1).unwrap();
    let life = realm
        .resume(&b, principal, character, [3., 0., 0.], 1)
        .unwrap();
    assert_eq!(
        realm.games[&1002]
            .game()
            .player_snapshot(life)
            .unwrap()
            .player
            .hp,
        0
    );
    assert_eq!(
        realm.games[&1002]
            .character_rewards(life.actor)
            .unwrap()
            .items[&1],
        3
    );
    realm.logout(&b, principal, character, 1).unwrap();
    for _ in 1..8 {
        let (id, _) = realm
            .create_character(&a, principal, [4., 0., 0.], 1)
            .unwrap();
        realm.logout(&a, principal, id, 1).unwrap();
    }
    assert_eq!(realm.account(account.id).unwrap().characters.len(), 8);
    let before = serde_json::to_vec(&realm.manifest).unwrap();
    let checkpoint = realm.games[&1001].checkpoint().unwrap();
    assert!(
        realm
            .create_character(&a, principal, [4., 0., 0.], 1)
            .is_err()
    );
    assert_eq!(serde_json::to_vec(&realm.manifest).unwrap(), before);
    assert_eq!(realm.games[&1001].checkpoint().unwrap(), checkpoint);
    realm.poisoned = true;
    assert!(realm.account(account.id).is_err());
    assert!(realm.account_for_key(principal).is_err());
    assert!(realm.character(character).is_err());
}

#[test]
fn primary_equipment_and_progression_survive_logout_recovery_and_another_instance() {
    use crate::service::{
        equipment::{Catalog, Gear, Slot},
        rewards::{Entry, Transaction},
    };
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let catalog = Catalog {
        version: 1,
        gear: vec![Gear {
            id: 2,
            name: "Traveler crown".into(),
            slot: Slot::Head,
            model: "adventurer".into(),
            offset: [0; 3],
            health: 100,
            mana: 10,
        }],
    };
    let mut source = gateway(1001, 11).with_equipment(catalog.clone()).unwrap();
    let primary = source.game().player_life();
    source
        .grant_reward(Transaction {
            acceptance: None,
            instance: 1001,
            actor: primary.actor,
            source: [88; 32],
            experience: 1000,
            items: vec![Entry { id: 2, count: 1 }],
            quests: vec![Entry { id: 1, count: 2 }],
            spent: vec![],
            outfit: None,
            equipment: None,
        })
        .unwrap();
    let connection = joining(&mut source, 11);
    let admission = source.admission(connection).unwrap();
    let original = source
        .equip_gear(
            connection,
            primary,
            admission.epoch(),
            Slot::Head,
            2,
            [64; 16],
        )
        .unwrap();
    source
        .chamber
        .game
        .recover_player_resources(primary.actor, 50, 5)
        .unwrap();
    let destination = gateway(1002, 12).with_equipment(catalog).unwrap();
    let mut realm = Realm::open(&root).unwrap();
    realm
        .create(source, "127.0.0.1:4001".parse().unwrap(), 4, 0)
        .unwrap();
    realm
        .create(destination, "127.0.0.1:4002".parse().unwrap(), 4, 0)
        .unwrap();
    let old_key = key(11).x_only_public_key().0.serialize();
    let account = realm.account_for_key(old_key).unwrap().unwrap();
    let character = account.characters[0];
    let a = realm.acquire(1001, [1; 32], 0).unwrap();
    let b = realm.acquire(1002, [2; 32], 0).unwrap();
    realm.logout(&a, old_key, character, 1).unwrap();
    assert_eq!(realm.games[&1001].game().snapshot().player.max_hp, 200);
    let new_key = key(17).x_only_public_key().0.serialize();
    realm
        .recover_account(&[a], account.id, account.epoch, new_key, 1)
        .unwrap();
    let life = realm
        .resume(&b, new_key, character, [3., 0., 0.], 1)
        .unwrap();
    realm.checkpoint(&b, 1).unwrap();
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let _b = realm.acquire(1002, [2; 32], 2).unwrap();
    let gateway = realm.games.get_mut(&1002).unwrap();
    let connection = joining(gateway, 17);
    let admission = gateway.admission(connection).unwrap();
    assert_eq!(admission.actor(), life);
    let resources = gateway.game().player_snapshot(life).unwrap().player;
    assert_eq!(
        (
            resources.hp,
            resources.max_hp,
            resources.mana,
            resources.max_mana
        ),
        (250, 300, 25, 30)
    );
    let state = gateway.character_rewards(life.actor).unwrap();
    assert_eq!(state.experience, 1000);
    assert_eq!(state.quests[&1], 2);
    assert_eq!(state.items[&2], 1);
    assert_eq!(state.equipment[&Slot::Head], 2);
    assert_eq!(
        gateway
            .equip_gear(connection, life, admission.epoch(), Slot::Head, 2, [64; 16])
            .unwrap(),
        original
    );
    assert_eq!(gateway.game().player_snapshot(life).unwrap().player.hp, 250);
}

#[test]
fn public_guest_admission_registers_verified_accounts_and_preserves_recovery() {
    fn authenticate(realm: &mut Realm, lease: &Lease, n: u8, valid: bool) -> Response {
        let (connection, hello) = realm.open_connection(lease, 1).unwrap();
        let hello: Hello = serde_json::from_slice(&hello).unwrap();
        let pair = key(n);
        let public = pair.x_only_public_key().0.serialize();
        let mut signature = secp256k1::Secp256k1::new()
            .sign_schnorr_no_aux_rand(&hello.challenge.signing_digest(public), &pair)
            .to_byte_array();
        if !valid {
            signature[0] ^= 1;
        }
        serde_json::from_slice(
            &realm
                .dispatch(
                    lease,
                    connection,
                    1,
                    &serde_json::to_vec(&Request {
                        version: VERSION,
                        request_id: 1,
                        body: Body::Authenticate {
                            public_key: public,
                            signature: signature.to_vec(),
                        },
                    })
                    .unwrap(),
                )
                .unwrap(),
        )
        .unwrap()
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("realm");
    let public = gateway(1001, 11)
        .with_guests(
            Some(crate::service::auth::Guests {
                cap: 1,
                ring: [3., 0., 0.],
                radius: 1.,
            }),
            0,
        )
        .unwrap();
    let mut realm = Realm::open(&root).unwrap();
    realm
        .create(public, "127.0.0.1:4001".parse().unwrap(), 2, 0)
        .unwrap();
    let lease = realm.acquire(1001, [1; 32], 0).unwrap();
    let before = realm.manifest.clone();
    assert!(matches!(
        authenticate(&mut realm, &lease, 12, false).body,
        Reply::Refused { .. }
    ));
    assert_eq!(realm.manifest.registry_root, before.registry_root);
    assert_eq!(realm.manifest.next_character, before.next_character);
    assert_eq!(realm.games[&1001].guest_count(), 0);
    assert!(matches!(
        authenticate(&mut realm, &lease, 12, true).body,
        Reply::Accepted
    ));
    let old_key = key(12).x_only_public_key().0.serialize();
    let account = realm.account_for_key(old_key).unwrap().unwrap();
    let character = account.characters[0];
    assert_eq!(realm.games[&1001].guest_count(), 1);
    assert!(matches!(
        authenticate(&mut realm, &lease, 13, true).body,
        Reply::Refused { .. }
    ));
    assert!(
        realm
            .account_for_key(key(13).x_only_public_key().0.serialize())
            .unwrap()
            .is_none()
    );
    realm.logout(&lease, old_key, character, 1).unwrap();
    assert_eq!(realm.games[&1001].guest_count(), 0);
    let new_key = key(14).x_only_public_key().0.serialize();
    realm
        .recover_account(&[lease.clone()], account.id, account.epoch, new_key, 1)
        .unwrap();
    assert!(matches!(
        authenticate(&mut realm, &lease, 12, true).body,
        Reply::Refused { .. }
    ));
    assert_eq!(realm.games[&1001].guest_count(), 0);
    assert!(matches!(
        authenticate(&mut realm, &lease, 13, true).body,
        Reply::Accepted
    ));
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let lease = realm.acquire(1001, [1; 32], 1).unwrap();
    assert_eq!(realm.games[&1001].guests().unwrap().cap, 1);
    assert_eq!(realm.games[&1001].guest_count(), 1);
    assert!(matches!(
        authenticate(&mut realm, &lease, 12, true).body,
        Reply::Refused { .. }
    ));
    assert_eq!(realm.account(account.id).unwrap().key, new_key);
    assert_eq!(realm.character(character).unwrap().account, account.id);
    assert!(matches!(
        authenticate(&mut realm, &lease, 14, true).body,
        Reply::Accepted
    ));
    assert_eq!(realm.games[&1001].guest_count(), 1);
}
