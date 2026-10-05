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
    assert!(viewer.snapshot().await.is_err());
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
