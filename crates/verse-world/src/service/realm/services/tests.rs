use super::*;
use crate::service::{
    Chamber,
    net::tests::key,
    wire::{Hello, Reply, Response, VERSION},
};
fn gateway(instance: u64, who: u8) -> Gateway {
    let scene = verse_engine::director::Scene::from_json(include_bytes!(
        "../../../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let mut game = crate::play::Game::combat_in(scene, false, instance).unwrap();
    game.time = game.scene.cut_at;
    game.tick(0., [0.; 2]).unwrap();
    game.encounter
        .as_mut()
        .unwrap()
        .postpone_casts_until(600.)
        .unwrap();
    if who == 11 {
        let mut class = crate::content::Character::default();
        class.key = "guardian".into();
        class.health = 300;
        class.mana = 40;
        game.configure_character(game.player_life(), class).unwrap();
    }
    let mut gateway = Gateway::new(Chamber::new(game).unwrap())
        .unwrap()
        .with_content([9; 32])
        .unwrap()
        .with_progression(crate::service::progression::Config {
            version: 1,
            levels: vec![0, 100, 300],
            quests: vec![crate::service::progression::Quest {
                repeatable: true,
                dialogue: None,
                giver: None,
                prerequisites: vec![],
                id: 1,
                name: "Party expedition".into(),
                objective: 1,
                goal: 2,
                experience: 75,
                items: vec![],
            }],
        })
        .unwrap()
        .with_equipment(crate::service::equipment::Catalog {
            version: 1,
            gear: vec![
                crate::service::equipment::Gear {
                    id: 10,
                    name: "Hat".into(),
                    slot: crate::service::equipment::Slot::Head,
                    model: "gear-hat".into(),
                    offset: [0; 3],
                    health: 100,
                    mana: 5,
                },
                crate::service::equipment::Gear {
                    id: 11,
                    name: "Wand".into(),
                    slot: crate::service::equipment::Slot::MainHand,
                    model: "gear-wand".into(),
                    offset: [0; 3],
                    health: 25,
                    mana: 10,
                },
            ],
        })
        .unwrap();
    gateway
        .enroll_primary(key(who).x_only_public_key().0.serialize())
        .unwrap();
    gateway
}
fn setup(root: &Path, now: u64) -> (Realm, Lease, Lease, u64, u64) {
    let mut realm = Realm::open(root).unwrap();
    realm
        .create(gateway(1001, 11), "127.0.0.1:4001".parse().unwrap(), 4, now)
        .unwrap();
    realm
        .create(gateway(1002, 12), "127.0.0.1:4002".parse().unwrap(), 4, now)
        .unwrap();
    let alice = realm
        .characters()
        .find(|(_, k, _)| *k == key(11).x_only_public_key().0.serialize())
        .unwrap()
        .0;
    let bob = realm
        .characters()
        .find(|(_, k, _)| *k == key(12).x_only_public_key().0.serialize())
        .unwrap()
        .0;
    let a = realm.acquire(1001, [1; 32], now).unwrap();
    let b = realm.acquire(1002, [2; 32], now).unwrap();
    (realm, a, b, alice, bob)
}
fn connect(realm: &mut Realm, lease: &Lease, who: u8, now: u64) -> ConnectionId {
    let (connection, hello) = realm.open_connection(lease, now).unwrap();
    let hello: Hello = serde_json::from_slice(&hello).unwrap();
    let pair = key(who);
    let public = pair.x_only_public_key().0.serialize();
    let signature = secp256k1::Secp256k1::new()
        .sign_schnorr_no_aux_rand(&hello.challenge.signing_digest(public), &pair)
        .to_byte_array()
        .to_vec();
    let response = dispatch(
        realm,
        lease,
        connection,
        Body::Authenticate {
            public_key: public,
            signature,
        },
        now,
    );
    assert!(matches!(response.body, Reply::Accepted));
    connection
}
fn dispatch(
    realm: &mut Realm,
    lease: &Lease,
    connection: ConnectionId,
    body: Body,
    now: u64,
) -> Response {
    serde_json::from_slice(
        &realm
            .dispatch(
                lease,
                connection,
                now,
                &serde_json::to_vec(&Request {
                    version: VERSION,
                    request_id: 1,
                    body,
                })
                .unwrap(),
            )
            .unwrap(),
    )
    .unwrap()
}
fn apply(
    realm: &mut Realm,
    lease: &Lease,
    connection: ConnectionId,
    character: u64,
    nonce: u8,
    action: Action,
) -> api::Receipt {
    let response = dispatch(
        realm,
        lease,
        connection,
        Body::ServiceAction {
            realm: realm.manifest.id,
            character,
            operation: [nonce; 16],
            action,
        },
        1,
    );
    match response.body {
        Reply::ServiceApplied { receipt } => receipt,
        other => panic!("Service action failed: {other:?}"),
    }
}
fn group_id(receipt: api::Receipt) -> Id {
    match receipt.outcome {
        Outcome::Group { group } => group.id,
        _ => panic!("Expected group"),
    }
}
fn item(receipt: api::Receipt) -> Item {
    match receipt.outcome {
        Outcome::Item { item } => item,
        _ => panic!("Expected item"),
    }
}
fn offer(receipt: api::Receipt) -> Offer {
    match receipt.outcome {
        Outcome::Trade { offer } => offer,
        _ => panic!("Expected trade"),
    }
}
fn party(
    realm: &mut Realm,
    a: &Lease,
    b: &Lease,
    ca: ConnectionId,
    cb: ConnectionId,
    alice: u64,
    bob: u64,
) -> Id {
    let id = group_id(apply(
        realm,
        a,
        ca,
        alice,
        1,
        Action::Create {
            kind: Kind::Party,
            name: "Expedition".into(),
        },
    ));
    apply(
        realm,
        a,
        ca,
        alice,
        2,
        Action::Invite {
            group: id,
            target: bob,
        },
    );
    apply(realm, b, cb, bob, 1, Action::Join { group: id });
    id
}
fn loot(event: u8, party: Id) -> api::Loot {
    api::Loot {
        event: [event; 32],
        party,
        experience: 25,
        items: vec![
            crate::service::rewards::Entry { id: 10, count: 1 },
            crate::service::rewards::Entry { id: 11, count: 1 },
        ],
        quests: vec![crate::service::rewards::Entry { id: 1, count: 2 }],
    }
}
#[test]
fn authenticated_membership_loot_and_cross_instance_trade_are_durable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("realm");
    let (mut realm, a, b, alice, bob) = setup(&root, 0);
    let ca = connect(&mut realm, &a, 11, 1);
    let cb = connect(&mut realm, &b, 12, 1);
    let party = party(&mut realm, &a, &b, ca, cb, alice, bob);
    let guild = group_id(apply(
        &mut realm,
        &a,
        ca,
        alice,
        3,
        Action::Create {
            kind: Kind::Guild,
            name: "Explorers".into(),
        },
    ));
    apply(
        &mut realm,
        &a,
        ca,
        alice,
        4,
        Action::Invite {
            group: guild,
            target: bob,
        },
    );
    apply(&mut realm, &b, cb, bob, 2, Action::Decline { group: guild });
    apply(
        &mut realm,
        &a,
        ca,
        alice,
        5,
        Action::Invite {
            group: guild,
            target: bob,
        },
    );
    apply(&mut realm, &b, cb, bob, 3, Action::Join { group: guild });
    let grant = realm.party_loot(&a, loot(40, party), 1).unwrap();
    assert_eq!(grant.recipients, vec![alice, bob]);
    let hat = item(apply(
        &mut realm,
        &a,
        ca,
        alice,
        6,
        Action::Materialize { definition: 10 },
    ));
    let wand = item(apply(
        &mut realm,
        &b,
        cb,
        bob,
        4,
        Action::Materialize { definition: 11 },
    ));
    assert!(realm.services_view(&b, cb, alice, 1).is_err());
    assert!(
        realm
            .services_action(
                &a,
                ca,
                realm.manifest.id,
                alice,
                [6; 16],
                Action::Materialize { definition: 11 },
                1
            )
            .is_err()
    );
    let trade = offer(apply(
        &mut realm,
        &a,
        ca,
        alice,
        7,
        Action::Offer {
            to: bob,
            give: vec![api::Token {
                id: hat.id,
                version: 1,
            }],
            want: vec![api::Token {
                id: wand.id,
                version: 1,
            }],
            expires_ms: 1000,
        },
    ));
    assert!(
        realm
            .services_action(
                &a,
                ca,
                realm.manifest.id,
                alice,
                [8; 16],
                Action::Accept { offer: trade.id },
                1
            )
            .is_err()
    );
    let before = realm.games[&1001].checkpoint().unwrap();
    realm.games.get_mut(&1002).unwrap().chamber.equipment.gear[0].health += 1;
    assert!(
        realm
            .services_action(
                &b,
                cb,
                realm.manifest.id,
                bob,
                [5; 16],
                Action::Accept { offer: trade.id },
                1
            )
            .is_err()
    );
    assert_eq!(realm.games[&1001].checkpoint().unwrap(), before);
    assert_eq!(realm.item_instance(hat.id).unwrap().owner, alice);
    realm.games.get_mut(&1002).unwrap().chamber.equipment.gear[0].health -= 1;
    let accepted = apply(
        &mut realm,
        &b,
        cb,
        bob,
        5,
        Action::Accept { offer: trade.id },
    );
    assert_eq!(realm.item_instance(hat.id).unwrap().owner, bob);
    assert_eq!(realm.item_instance(wand.id).unwrap().owner, alice);
    assert_eq!(realm.item_instance(hat.id).unwrap().version, 2);
    assert_eq!(
        apply(
            &mut realm,
            &b,
            cb,
            bob,
            5,
            Action::Accept { offer: trade.id }
        ),
        accepted
    );
    assert!(
        realm
            .services_action(
                &a,
                ca,
                realm.manifest.id,
                alice,
                [9; 16],
                Action::Offer {
                    to: bob,
                    give: vec![api::Token {
                        id: hat.id,
                        version: 1
                    }],
                    want: vec![],
                    expires_ms: 1000
                },
                1
            )
            .is_err()
    );
    let actor = realm.manifest.characters[&alice].actor;
    assert!(
        !realm.games[&1001]
            .character_rewards(actor)
            .unwrap()
            .items
            .contains_key(&10)
    );
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let a = realm.acquire(1001, [3; 32], 2).unwrap();
    let b = realm.acquire(1002, [4; 32], 2).unwrap();
    let ca = connect(&mut realm, &a, 11, 2);
    let cb = connect(&mut realm, &b, 12, 2);
    assert_eq!(
        realm.services_view(&a, ca, alice, 2).unwrap().groups.len(),
        2
    );
    assert_eq!(
        realm
            .services_action(
                &b,
                cb,
                realm.manifest.id,
                bob,
                [5; 16],
                Action::Accept { offer: trade.id },
                2
            )
            .unwrap(),
        accepted
    );
    assert_eq!(realm.party_loot(&a, loot(40, party), 2).unwrap(), grant);
    let moved = realm
        .transfer(&a, &b, alice, [19; 16], [3., 0., 0.], 2)
        .unwrap();
    assert_eq!(moved.destination.instance, 1002);
    let connection = connect(&mut realm, &b, 11, 2);
    let view = realm.services_view(&b, connection, alice, 2).unwrap();
    assert_eq!(view.items[0].id, wand.id);
    assert!(realm.services_view(&a, ca, alice, 2).is_err());
    realm
        .logout(&b, key(11).x_only_public_key().0.serialize(), alice, 2)
        .unwrap();
    assert_eq!(realm.party_loot(&a, loot(40, party), 2).unwrap(), grant);
    realm
        .resume(
            &b,
            key(11).x_only_public_key().0.serialize(),
            alice,
            [3., 0., 0.],
            2,
        )
        .unwrap();
    let connection = connect(&mut realm, &b, 11, 2);
    assert_eq!(
        realm.services_view(&b, connection, alice, 2).unwrap().items[0].owner,
        alice
    );
}
#[tokio::test]
async fn real_tls_party_adventure_claims_trades_transfers_and_restarts() {
    use crate::service::{client::Client, net::tests::tls, progression::Action as QuestAction};
    use rustls::pki_types::ServerName;
    use tokio::{net::TcpListener, sync::oneshot};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("realm");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let (mut realm, a, b, alice, bob) = setup(&root, now);
    let alice_account = realm.character(alice).unwrap().account;
    let bob_account = realm.character(bob).unwrap().account;
    let left = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let right = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let left_addr = left.local_addr().unwrap();
    let right_addr = right.local_addr().unwrap();
    realm.set_endpoint(&a, left_addr, now).unwrap();
    realm.set_endpoint(&b, right_addr, now).unwrap();
    let (tls, connector) = tls();
    let (stop, shutdown) = oneshot::channel();
    let (control, commands) = super::super::net::channel();
    let server = tokio::spawn(super::super::net::serve(
        realm,
        vec![(a, left), (b, right)],
        tls,
        commands,
        async {
            let _ = shutdown.await;
        },
    ));
    let mut ca = Client::connect_with_content(
        left_addr,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        1001,
        Some([9; 32]),
        &key(11),
    )
    .await
    .unwrap();
    let mut cb = Client::connect_with_content(
        right_addr,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        1002,
        Some([9; 32]),
        &key(12),
    )
    .await
    .unwrap();
    let identity = ca.services(alice).await.unwrap().realm;
    let party = group_id(
        ca.service_action(
            identity,
            alice,
            [1; 16],
            Action::Create {
                kind: Kind::Party,
                name: "Expedition".into(),
            },
        )
        .await
        .unwrap(),
    );
    ca.service_action(
        identity,
        alice,
        [2; 16],
        Action::Invite {
            group: party,
            target: bob,
        },
    )
    .await
    .unwrap();
    cb.service_action(identity, bob, [1; 16], Action::Join { group: party })
        .await
        .unwrap();
    assert!(cb.services(alice).await.is_err());
    let grant = control.party_loot(1001, loot(50, party)).await.unwrap();
    assert_eq!(grant.recipients, vec![alice, bob]);
    for client in [&mut ca, &mut cb] {
        assert!(matches!(
            client
                .quest_cycle(1, 0, QuestAction::Claim)
                .await
                .unwrap()
                .body,
            Reply::QuestCycleChanged { .. }
        ));
        let inv = client.inventory().await.unwrap();
        assert_eq!(inv.level.level, 2);
        assert_eq!(inv.experience, 100);
    }
    let hat = item(
        ca.service_action(
            identity,
            alice,
            [3; 16],
            Action::Materialize { definition: 10 },
        )
        .await
        .unwrap(),
    );
    let wand = item(
        cb.service_action(
            identity,
            bob,
            [2; 16],
            Action::Materialize { definition: 11 },
        )
        .await
        .unwrap(),
    );
    let trade = offer(
        ca.service_action(
            identity,
            alice,
            [4; 16],
            Action::Offer {
                to: bob,
                give: vec![api::Token {
                    id: hat.id,
                    version: 1,
                }],
                want: vec![api::Token {
                    id: wand.id,
                    version: 1,
                }],
                expires_ms: now + 300_000,
            },
        )
        .await
        .unwrap(),
    );
    assert!(
        ca.service_action(identity, alice, [5; 16], Action::Accept { offer: trade.id })
            .await
            .is_err()
    );
    let accepted = cb
        .service_action(identity, bob, [3; 16], Action::Accept { offer: trade.id })
        .await
        .unwrap();
    assert_eq!(
        cb.service_action(identity, bob, [3; 16], Action::Accept { offer: trade.id })
            .await
            .unwrap(),
        accepted
    );
    ca.equip_gear(crate::service::equipment::Slot::MainHand, 11, [31; 16])
        .await
        .unwrap();
    let stats = ca.snapshot().await.unwrap().snapshot.player;
    assert_eq!((stats.max_hp, stats.max_mana), (335, 51));
    assert!(stats.hp <= 300);
    ca.quest_cycle(1, 0, QuestAction::Reset).await.unwrap();
    assert!(matches!(
        ca.quest_cycle(1, 1, QuestAction::Claim).await.unwrap().body,
        Reply::Refused { .. }
    ));
    ca.quest_cycle(1, 0, QuestAction::Claim).await.unwrap();
    assert_eq!(ca.inventory().await.unwrap().experience, 100);
    control.party_loot(1001, loot(51, party)).await.unwrap();
    ca.quest_cycle(1, 1, QuestAction::Claim).await.unwrap();
    assert_eq!(ca.inventory().await.unwrap().experience, 200);
    let moved = control
        .transfer(1001, 1002, alice, [19; 16], [3., 0., 0.])
        .await
        .unwrap();
    assert_eq!(moved.destination.instance, 1002);
    assert!(ca.snapshot().await.is_err());
    let mut ca = Client::connect_with_content(
        right_addr,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        1002,
        Some([9; 32]),
        &key(11),
    )
    .await
    .unwrap();
    assert_eq!(ca.services(alice).await.unwrap().items[0].id, wand.id);
    assert!(matches!(
        ca.logout().await.unwrap().body,
        Reply::LoggedOut { .. }
    ));
    assert!(!ca.connected());
    let mut ca = Client::connect_with_content(
        right_addr,
        ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        1002,
        Some([9; 32]),
        &key(11),
    )
    .await
    .unwrap();
    assert!(ca.control().is_none());
    assert!(ca.services(alice).await.is_err());
    assert!(ca.connected());
    ca.select_character(alice).await.unwrap();
    assert_eq!(ca.inventory().await.unwrap().experience, 200);
    ca.quest_cycle(1, 1, QuestAction::Claim).await.unwrap();
    assert_eq!(ca.inventory().await.unwrap().experience, 200);
    assert_eq!(
        control.party_loot(1001, loot(50, party)).await.unwrap(),
        grant
    );
    use crate::service::safety as safe;
    let block = ca
        .safety_action(
            identity,
            [80; 16],
            safe::Action::Block {
                account: bob_account,
                blocked: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(ca.safety().await.unwrap().blocked, vec![bob_account]);
    assert!(cb.safety().await.unwrap().blocked.is_empty());
    let report = cb
        .safety_action(
            identity,
            [81; 16],
            safe::Action::Report {
                account: alice_account,
                reason: safe::Reason::Harassment,
                evidence: Some([8; 32]),
            },
        )
        .await
        .unwrap();
    let report_id = match report.outcome {
        safe::Outcome::Report { id } => id,
        _ => panic!("Expected report"),
    };
    assert_eq!(control.pending_reports().await.unwrap().len(), 1);
    control
        .resolve_report(report_id, safe::Status::Dismissed)
        .await
        .unwrap();
    assert!(control.pending_reports().await.unwrap().is_empty());
    assert_eq!(
        cb.safety_action(
            identity,
            [81; 16],
            safe::Action::Report {
                account: alice_account,
                reason: safe::Reason::Harassment,
                evidence: Some([8; 32])
            }
        )
        .await
        .unwrap(),
        report
    );
    drop(ca);
    drop(cb);
    stop.send(()).unwrap();
    let exit = server.await.unwrap().unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
    let stats = exit.stats;
    drop(exit.realm);
    let mut realm = Realm::open(&root).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let a = realm.acquire(1001, [3; 32], now + 1000).unwrap();
    let b = realm.acquire(1002, [4; 32], now + 1000).unwrap();
    let ca = connect(&mut realm, &b, 11, now + 1000);
    let cb = connect(&mut realm, &b, 12, now + 1000);
    assert_eq!(
        realm
            .services_view(&b, ca, alice, now + 1000)
            .unwrap()
            .items[0]
            .id,
        wand.id
    );
    assert_eq!(
        realm
            .services_action(
                &b,
                cb,
                identity,
                bob,
                [3; 16],
                Action::Accept { offer: trade.id },
                now + 1000
            )
            .unwrap(),
        accepted
    );
    assert_eq!(
        realm.party_loot(&a, loot(50, party), now + 1000).unwrap(),
        grant
    );
    assert_eq!(
        realm.safety_view(&b, ca, now + 1000).unwrap().blocked,
        vec![bob_account]
    );
    assert!(realm.pending_reports().unwrap().is_empty());
    assert_eq!(
        realm
            .safety_action(
                &b,
                ca,
                identity,
                [80; 16],
                safe::Action::Block {
                    account: bob_account,
                    blocked: true
                },
                now + 1000
            )
            .unwrap(),
        block
    );
    assert_eq!(
        realm
            .resolve_report(report_id, safe::Status::Dismissed, now + 1000)
            .unwrap()
            .target,
        alice_account
    );
    let actor = realm.manifest.characters[&alice].actor;
    assert_eq!(
        realm.games[&1002]
            .character_rewards(actor)
            .unwrap()
            .experience,
        200
    );
    if let Ok(path) = std::env::var("VERSE_GAME_SERVICES_RECEIPT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "verse.realm.game-services.acceptance.v1", "instances": 2,
                "authenticated_characters": 2, "stable_party_members": 2, "quest_cycles": 2,
                "item_instances": 2, "trade_consent": "offer owner and target acceptance",
                "trade_exact_retry": true, "foreign_mutation_refused": true,
                "guardian_level_two_with_wand": { "max_hp": 335, "max_mana": 51 },
                "logout_resume": true, "zone_transfer": true, "restart": true,
                "party_event_recipients_frozen": true, "final_character_experience": 200,
                "host_loot": "trusted local authored outcome adapter", "stats": stats,
                "account_block_private":true, "typed_report_private_queue":true, "completed_report_exact_retry":true
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn services_crash_child() {
    let Ok(root) = std::env::var("VERSE_GAME_SERVICES_CHILD_ROOT") else {
        return;
    };
    let mut realm = Realm::open(Path::new(&root)).unwrap();
    let a = realm.acquire(1001, [1; 32], 2).unwrap();
    let b = realm.acquire(1002, [2; 32], 2).unwrap();
    let bob = realm
        .characters()
        .find(|(_, k, _)| *k == key(12).x_only_public_key().0.serialize())
        .unwrap()
        .0;
    let connection = connect(&mut realm, &b, 12, 2);
    let trade = realm.member(bob).unwrap().offers[0];
    realm
        .services_action(
            &b,
            connection,
            realm.manifest.id,
            bob,
            [5; 16],
            Action::Accept { offer: trade },
            2,
        )
        .unwrap();
    let _ = a;
    panic!("Service publication crash boundary did not fire");
}
#[test]
fn every_trade_publication_boundary_selects_both_inventories_and_owners() {
    for stage in [
        "before_snapshots",
        "after_snapshots",
        "before_seal",
        "after_seal",
        "after_directory_sync",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("realm");
        let (mut realm, a, b, alice, bob) = setup(&root, 0);
        let ca = connect(&mut realm, &a, 11, 1);
        let cb = connect(&mut realm, &b, 12, 1);
        let party = party(&mut realm, &a, &b, ca, cb, alice, bob);
        realm.party_loot(&a, loot(60, party), 1).unwrap();
        let hat = item(apply(
            &mut realm,
            &a,
            ca,
            alice,
            3,
            Action::Materialize { definition: 10 },
        ));
        let wand = item(apply(
            &mut realm,
            &b,
            cb,
            bob,
            2,
            Action::Materialize { definition: 11 },
        ));
        let trade = offer(apply(
            &mut realm,
            &a,
            ca,
            alice,
            4,
            Action::Offer {
                to: bob,
                give: vec![api::Token {
                    id: hat.id,
                    version: 1,
                }],
                want: vec![api::Token {
                    id: wand.id,
                    version: 1,
                }],
                expires_ms: 1000,
            },
        ));
        drop(realm);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::realm::services::tests::services_crash_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("VERSE_GAME_SERVICES_CHILD_ROOT", &root)
            .env("VERSE_REALM_CRASH_AT", format!("services_{stage}"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(86),
            "{stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut realm = Realm::open(&root).unwrap();
        let a = realm.acquire(1001, [3; 32], 3).unwrap();
        let b = realm.acquire(1002, [4; 32], 3).unwrap();
        let sealed = matches!(stage, "after_seal" | "after_directory_sync");
        assert_eq!(
            realm.item_instance(hat.id).unwrap().owner,
            if sealed { bob } else { alice }
        );
        assert_eq!(
            realm.item_instance(wand.id).unwrap().owner,
            if sealed { alice } else { bob }
        );
        let ca_actor = realm.manifest.characters[&alice].actor;
        let cb_actor = realm.manifest.characters[&bob].actor;
        let ca_items = &realm.games[&1001]
            .character_rewards(ca_actor)
            .unwrap()
            .items;
        let cb_items = &realm.games[&1002]
            .character_rewards(cb_actor)
            .unwrap()
            .items;
        assert_eq!(
            ca_items.get(&10).copied().unwrap_or(0),
            if sealed { 0 } else { 1 }
        );
        assert_eq!(
            cb_items.get(&11).copied().unwrap_or(0),
            if sealed { 0 } else { 1 }
        );
        assert_eq!(
            ca_items.get(&11).copied().unwrap_or(0),
            if sealed { 2 } else { 1 }
        );
        assert_eq!(
            cb_items.get(&10).copied().unwrap_or(0),
            if sealed { 2 } else { 1 }
        );
        let cb = connect(&mut realm, &b, 12, 3);
        let first = realm
            .services_action(
                &b,
                cb,
                realm.manifest.id,
                bob,
                [5; 16],
                Action::Accept { offer: trade.id },
                3,
            )
            .unwrap();
        assert_eq!(
            realm
                .services_action(
                    &b,
                    cb,
                    realm.manifest.id,
                    bob,
                    [5; 16],
                    Action::Accept { offer: trade.id },
                    3
                )
                .unwrap(),
            first
        );
        assert_eq!(realm.item_instance(hat.id).unwrap().owner, bob);
        let _ = a;
        println!("trade boundary {stage}: paired inventories and ownership, exact retry");
    }
}
#[test]
fn offers_lock_only_the_sender_and_cancel_after_requested_items_change_owner() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("realm");
    let (mut realm, a, b, alice, bob) = setup(&root, 0);
    let ca = connect(&mut realm, &a, 11, 1);
    let cb = connect(&mut realm, &b, 12, 1);
    let party = party(&mut realm, &a, &b, ca, cb, alice, bob);
    realm.party_loot(&a, loot(60, party), 1).unwrap();
    let hat = item(apply(
        &mut realm,
        &a,
        ca,
        alice,
        3,
        Action::Materialize { definition: 10 },
    ));
    let wand = item(apply(
        &mut realm,
        &b,
        cb,
        bob,
        2,
        Action::Materialize { definition: 11 },
    ));
    let first = offer(apply(
        &mut realm,
        &a,
        ca,
        alice,
        4,
        Action::Offer {
            to: bob,
            give: vec![api::Token {
                id: hat.id,
                version: 1,
            }],
            want: vec![api::Token {
                id: wand.id,
                version: 1,
            }],
            expires_ms: 1000,
        },
    ));
    assert_eq!(realm.item_instance(hat.id).unwrap().locked, Some(first.id));
    assert_eq!(realm.item_instance(wand.id).unwrap().locked, None);
    let gift = offer(apply(
        &mut realm,
        &b,
        cb,
        bob,
        3,
        Action::Offer {
            to: alice,
            give: vec![api::Token {
                id: wand.id,
                version: 1,
            }],
            want: vec![],
            expires_ms: 1000,
        },
    ));
    apply(
        &mut realm,
        &a,
        ca,
        alice,
        5,
        Action::Accept { offer: gift.id },
    );
    assert!(
        realm
            .services_action(
                &b,
                cb,
                realm.manifest.id,
                bob,
                [4; 16],
                Action::Accept { offer: first.id },
                1
            )
            .is_err()
    );
    apply(
        &mut realm,
        &b,
        cb,
        bob,
        4,
        Action::Cancel { offer: first.id },
    );
    assert_eq!(realm.item_instance(hat.id).unwrap().locked, None);
    assert_eq!(realm.item_instance(wand.id).unwrap().owner, alice);
    assert_eq!(realm.item_instance(wand.id).unwrap().version, 2);
    let expiring = offer(apply(
        &mut realm,
        &a,
        ca,
        alice,
        6,
        Action::Offer {
            to: bob,
            give: vec![api::Token {
                id: hat.id,
                version: 1,
            }],
            want: vec![],
            expires_ms: 1000,
        },
    ));
    assert!(
        realm
            .services_action(
                &b,
                cb,
                realm.manifest.id,
                bob,
                [5; 16],
                Action::Accept { offer: expiring.id },
                1000
            )
            .is_err()
    );
    realm
        .services_action(
            &b,
            cb,
            realm.manifest.id,
            bob,
            [5; 16],
            Action::Cancel { offer: expiring.id },
            1000,
        )
        .unwrap();
    assert_eq!(realm.item_instance(hat.id).unwrap().owner, alice);
    assert_eq!(realm.item_instance(hat.id).unwrap().locked, None);
}

#[test]
fn account_blocks_fence_pending_and_new_contact_across_restart() {
    use crate::service::safety as safe;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("realm");
    let (mut realm, a, b, alice, bob) = setup(&root, 0);
    let ca = connect(&mut realm, &a, 11, 1);
    let cb = connect(&mut realm, &b, 12, 1);
    let aa = realm.character(alice).unwrap().account;
    let ba = realm.character(bob).unwrap().account;
    let party = party(&mut realm, &a, &b, ca, cb, alice, bob);
    realm.party_loot(&a, loot(1, party), 1).unwrap();
    let ia = item(apply(
        &mut realm,
        &a,
        ca,
        alice,
        3,
        Action::Materialize { definition: 10 },
    ));
    let ib = item(apply(
        &mut realm,
        &b,
        cb,
        bob,
        2,
        Action::Materialize { definition: 11 },
    ));
    let pending = offer(apply(
        &mut realm,
        &a,
        ca,
        alice,
        4,
        Action::Offer {
            to: bob,
            give: vec![api::Token {
                id: ia.id,
                version: ia.version,
            }],
            want: vec![api::Token {
                id: ib.id,
                version: ib.version,
            }],
            expires_ms: 1000,
        },
    ));
    let block = safe::Action::Block {
        account: aa,
        blocked: true,
    };
    let first = realm
        .safety_action(&b, cb, realm.manifest.id, [1; 16], block.clone(), 1)
        .unwrap();
    assert_eq!(
        realm
            .safety_action(&b, cb, realm.manifest.id, [1; 16], block.clone(), 1)
            .unwrap(),
        first
    );
    assert_eq!(realm.safety_view(&b, cb, 1).unwrap().blocked, vec![aa]);
    assert_eq!(realm.safety_view(&a, ca, 1).unwrap().account, aa);
    assert!(realm.safety_view(&a, ca, 1).unwrap().blocked.is_empty());
    assert!(realm.contact(alice, bob).is_err());
    assert!(realm.contact(bob, alice).is_err());
    let refuse = dispatch(
        &mut realm,
        &b,
        cb,
        Body::ServiceAction {
            realm: first.realm,
            character: bob,
            operation: [3; 16],
            action: Action::Accept { offer: pending.id },
        },
        1,
    );
    assert!(matches!(refuse.body, Reply::Refused { .. }));
    // A block never prevents cancellation or strands the sender's locked gear.
    apply(
        &mut realm,
        &a,
        ca,
        alice,
        5,
        Action::Cancel { offer: pending.id },
    );
    assert!(realm.item_instance(ia.id).unwrap().locked.is_none());
    apply(&mut realm, &b, cb, bob, 4, Action::Leave { group: party });
    let refuse = dispatch(
        &mut realm,
        &a,
        ca,
        Body::ServiceAction {
            realm: first.realm,
            character: alice,
            operation: [6; 16],
            action: Action::Invite {
                group: party,
                target: bob,
            },
        },
        1,
    );
    assert!(matches!(refuse.body, Reply::Refused { .. }));
    realm
        .logout(&b, key(12).x_only_public_key().0.serialize(), bob, 1)
        .unwrap();
    let extra = realm
        .create_character(
            &b,
            key(12).x_only_public_key().0.serialize(),
            [2., 0., -22.],
            1,
        )
        .unwrap()
        .0;
    assert_eq!(realm.character(extra).unwrap().account, ba);
    assert!(realm.contact(alice, extra).is_err());
    let unauthenticated = realm.open_connection(&a, 1).unwrap().0;
    assert!(
        realm
            .safety_action(
                &a,
                unauthenticated,
                first.realm,
                [1; 16],
                safe::Action::Block {
                    account: ba,
                    blocked: true
                },
                1
            )
            .is_err()
    );
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let b = realm.acquire(1002, [2; 32], 2).unwrap();
    let cb = connect(&mut realm, &b, 12, 2);
    assert!(realm.contact(alice, bob).is_err());
    assert_eq!(
        realm
            .safety_action(&b, cb, first.realm, [1; 16], block, 2)
            .unwrap(),
        first
    );
    let unblock = safe::Action::Block {
        account: aa,
        blocked: false,
    };
    realm
        .safety_action(&b, cb, first.realm, [2; 16], unblock, 2)
        .unwrap();
    assert!(realm.contact(alice, bob).is_ok());
    // A delayed retry returns the original receipt without reinstating the block.
    realm
        .safety_action(
            &b,
            cb,
            first.realm,
            [1; 16],
            safe::Action::Block {
                account: aa,
                blocked: true,
            },
            2,
        )
        .unwrap();
    assert!(realm.contact(alice, bob).is_ok());
}

#[test]
fn private_reports_bound_submission_and_preserve_completed_exact_retries() {
    use crate::service::safety as safe;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("realm");
    let (mut realm, a, b, alice, bob) = setup(&root, 0);
    let ca = connect(&mut realm, &a, 11, 1);
    let cb = connect(&mut realm, &b, 12, 1);
    let account = realm.character(alice).unwrap().account;
    let target = realm.character(bob).unwrap().account;
    let realm_id = realm.manifest.id;
    let action = safe::Action::Report {
        account: target,
        reason: safe::Reason::Harassment,
        evidence: Some([9; 32]),
    };
    let receipt = realm
        .safety_action(&a, ca, realm_id, [1; 16], action.clone(), 1)
        .unwrap();
    let id = match receipt.outcome {
        safe::Outcome::Report { id } => id,
        _ => panic!("Expected report"),
    };
    assert_eq!(realm.pending_reports().unwrap().len(), 1);
    assert_eq!(realm.pending_reports().unwrap()[0].reporter, account);
    assert_eq!(
        realm
            .safety_action(&a, ca, realm_id, [1; 16], action.clone(), 1)
            .unwrap(),
        receipt
    );
    assert!(
        realm
            .safety_action(
                &a,
                ca,
                realm_id,
                [1; 16],
                safe::Action::Block {
                    account: target,
                    blocked: true
                },
                1
            )
            .is_err()
    );
    let foreign = dispatch(&mut realm, &b, cb, Body::Safety {}, 1);
    match foreign.body {
        Reply::Safety { view } => {
            assert_eq!(view.account, target);
            assert!(view.blocked.is_empty());
        }
        _ => panic!("Expected own safety projection"),
    }
    let finished = realm
        .resolve_report(id, safe::Status::Dismissed, 1)
        .unwrap();
    assert_eq!(
        realm
            .resolve_report(id, safe::Status::Dismissed, 1)
            .unwrap(),
        finished
    );
    assert!(realm.resolve_report(id, safe::Status::Actioned, 1).is_err());
    assert!(realm.pending_reports().unwrap().is_empty());
    for n in 2..=16 {
        realm
            .safety_action(&a, ca, realm_id, [n; 16], action.clone(), 1)
            .unwrap();
    }
    assert!(
        realm
            .safety_action(&a, ca, realm_id, [17; 16], action.clone(), 1)
            .is_err()
    );
    assert_eq!(realm.pending_reports().unwrap().len(), 15);
    assert_eq!(
        realm
            .safety_action(&a, ca, realm_id, [1; 16], action.clone(), 1)
            .unwrap(),
        receipt
    );
    assert_eq!(realm.pending_reports().unwrap().len(), 15);
    drop(realm);
    let mut realm = Realm::open(&root).unwrap();
    let a = realm.acquire(1001, [1; 32], 2).unwrap();
    let ca = connect(&mut realm, &a, 11, 2);
    assert_eq!(realm.pending_reports().unwrap().len(), 15);
    assert_eq!(
        realm
            .safety_action(&a, ca, realm_id, [1; 16], action.clone(), 2)
            .unwrap(),
        receipt
    );
    assert!(
        realm
            .safety_action(&a, ca, realm_id, [17; 16], action, 2)
            .is_err()
    );
    assert_eq!(
        realm
            .resolve_report(id, safe::Status::Dismissed, 2)
            .unwrap(),
        finished
    );
}

#[test]
fn safety_crash_child() {
    let Ok(root) = std::env::var("VERSE_SAFETY_CHILD_ROOT") else {
        return;
    };
    let mut realm = Realm::open(Path::new(&root)).unwrap();
    let a = realm.acquire(1001, [1; 32], 2).unwrap();
    let b = realm.acquire(1002, [2; 32], 2).unwrap();
    let connection = connect(&mut realm, &a, 11, 2);
    let target = realm
        .account_for_key(key(12).x_only_public_key().0.serialize())
        .unwrap()
        .unwrap()
        .id;
    realm
        .safety_action(
            &a,
            connection,
            realm.manifest.id,
            [70; 16],
            crate::service::safety::Action::Report {
                account: target,
                reason: crate::service::safety::Reason::Spam,
                evidence: None,
            },
            2,
        )
        .unwrap();
    let _ = b;
    panic!("Safety crash boundary did not fire");
}
#[test]
fn safety_receipt_preferences_and_queue_recover_at_every_publication_boundary() {
    use crate::service::safety as safe;
    for stage in [
        "before_snapshots",
        "after_snapshots",
        "before_seal",
        "after_seal",
        "after_directory_sync",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("realm");
        let (realm, _, _, _, bob) = setup(&root, 0);
        let target = realm.character(bob).unwrap().account;
        let identity = realm.manifest.id;
        drop(realm);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::realm::services::tests::safety_crash_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("VERSE_SAFETY_CHILD_ROOT", &root)
            .env("VERSE_REALM_CRASH_AT", format!("safety_{stage}"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(86),
            "{stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut realm = Realm::open(&root).unwrap();
        let before = realm.pending_reports().unwrap();
        assert_eq!(
            before.len(),
            if matches!(stage, "after_seal" | "after_directory_sync") {
                1
            } else {
                0
            }
        );
        let a = realm.acquire(1001, [1; 32], 3).unwrap();
        let ca = connect(&mut realm, &a, 11, 3);
        let action = safe::Action::Report {
            account: target,
            reason: safe::Reason::Spam,
            evidence: None,
        };
        let receipt = realm
            .safety_action(&a, ca, identity, [70; 16], action.clone(), 3)
            .unwrap();
        assert_eq!(
            realm
                .safety_action(&a, ca, identity, [70; 16], action, 3)
                .unwrap(),
            receipt
        );
        assert_eq!(realm.pending_reports().unwrap().len(), 1);
    }
}

#[test]
fn public_scene_and_wire_inputs_cannot_select_operator_or_studio_commands() {
    for action in [
        serde_json::json!({"type":"execute","command":"touch should-not-exist"}),
        serde_json::json!({"type":"studio","operation":{"type":"read_log"}}),
        serde_json::json!({"type":"toggle","object":1,"command":"touch should-not-exist"}),
    ] {
        assert!(serde_json::from_value::<crate::play::social::Action>(action).is_err());
    }
    for body in [
        serde_json::json!({"type":"resolve_report","id":vec![1;32],"status":"dismissed"}),
        serde_json::json!({"type":"publisher","public":vec![1;32],"enabled":true,"reason":"approve"}),
        serde_json::json!({"type":"studio_view"}),
    ] {
        let bytes =
            serde_json::to_vec(&serde_json::json!({"version":VERSION,"request_id":1,"body":body}))
                .unwrap();
        assert!(Request::decode(&bytes).is_err());
    }
}
