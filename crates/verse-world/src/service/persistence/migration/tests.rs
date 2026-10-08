use super::*;
use crate::service::{
    equipment::{Catalog as GearCatalog, Gear, Slot},
    host::Enrollment,
    items::{Catalog as ItemCatalog, Item},
    outfits::{Catalog as OutfitCatalog, Outfit},
    progression::{Config as Progression, Quest},
    rewards::{Entry, Receipt, Transaction},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use verse_engine::director::Scene;
fn key(n: u8) -> [u8; 32] {
    Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_byte_array([n; 32]).unwrap(),
    )
    .x_only_public_key()
    .0
    .serialize()
}
fn enrollment(n: u8, role: Role) -> Enrollment {
    Enrollment {
        public_key: key(n).iter().map(|v| format!("{v:02x}")).collect(),
        role,
    }
}
fn config(root: &Path) -> Config {
    Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        instance: 120,
        scene: "scene.json".into(),
        pack: "pack.json".into(),
        transport: Default::default(),
        certificate_der: "cert.der".into(),
        private_key_der: "key.der".into(),
        state_dir: Some(root.into()),
        authored_combat_health: false,
        authored: None,
        social_profile: None,
        guests: None,
        profile: None,
        enrollments: vec![
            enrollment(91, Role::Primary {}),
            enrollment(
                92,
                Role::Player {
                    spawn: [3., 0., -22.],
                },
            ),
            enrollment(93, Role::Spectator {}),
        ],
        rewards: vec![],
        items: ItemCatalog {
            version: 1,
            items: vec![Item {
                id: 1,
                name: "Recovery ember".into(),
                health: 45,
                mana: 5,
            }],
        },
        outfits: OutfitCatalog {
            version: 1,
            outfits: vec![Outfit {
                id: 2,
                name: "Ranger".into(),
                model: "ranger".into(),
            }],
        },
        equipment: GearCatalog {
            version: 1,
            gear: vec![Gear {
                id: 3,
                name: "Staff".into(),
                slot: Slot::MainHand,
                model: "staff".into(),
                offset: [0; 3],
                health: 20,
                mana: 2,
            }],
        },
        progression: Progression {
            version: 1,
            levels: vec![0, 100, 300],
            quests: vec![Quest {
                repeatable: false,
                id: 101,
                name: "Ritual".into(),
                objective: 2,
                goal: 2,
                experience: 75,
                items: vec![Entry { id: 1, count: 1 }],
                giver: None,
                dialogue: None,
                prerequisites: vec![],
            }],
        },
    }
}
fn game(config: &Config, changed: bool) -> Game {
    let mut scene = Scene::from_json(include_bytes!(
        "../../../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    if changed {
        scene.actors.iter_mut().find(|a| a.id == 1).unwrap().name = "Revised ritual".into();
        scene.actors.retain(|a| a.id != 6);
        let mut added = scene.actors.iter().find(|a| a.id == 2).unwrap().clone();
        added.id = 101;
        added.position.x += 8.;
        scene.actors.push(added);
    }
    let mut game = config.prepare_game(scene).unwrap();
    game.time = game.scene.cut_at;
    game.tick(0., [0.; 2]).unwrap();
    game
}
fn reward(actor: u64, n: u64) -> Transaction {
    let mut source = [5; 32];
    source[..8].copy_from_slice(&n.to_be_bytes());
    Transaction {
        instance: 120,
        actor,
        source,
        experience: 1,
        items: vec![],
        quests: vec![],
        spent: vec![],
        outfit: None,
        equipment: None,
        acceptance: None,
    }
}
struct Fixture {
    config: Config,
    target: Config,
    original: Transaction,
    original_receipt: Receipt,
    claim: Receipt,
    gear: Receipt,
    outfit: Receipt,
    item: Receipt,
    actor: u64,
    other: u64,
}
fn populate(root: &Path, legacy: bool) -> Fixture {
    let config = config(root);
    let mut gateway = config
        .gateway(game(&config, false))
        .unwrap()
        .with_content([8; 32])
        .unwrap();
    let mut store = Store::open(root, [8; 32], 120).unwrap();
    if !legacy {
        store.commit(&mut gateway).unwrap();
    }
    let actor = gateway.game().player_life().actor;
    let other = gateway.chamber.owners[&Principal(key(92))];
    let original = reward(actor, 1);
    let original_receipt = gateway.grant_reward(original.clone()).unwrap();
    let mut grant = reward(actor, 2);
    grant.items = vec![
        Entry { id: 1, count: 5 },
        Entry { id: 2, count: 1 },
        Entry { id: 3, count: 1 },
    ];
    grant.quests = vec![Entry { id: 2, count: 2 }];
    gateway.grant_reward(grant).unwrap();
    let session = gateway.chamber.connect(Principal(key(91))).unwrap();
    let admission = gateway
        .chamber
        .admission(Principal(key(91)), session)
        .unwrap();
    let gear = gateway
        .chamber
        .equip_gear(
            Principal(key(91)),
            session,
            admission.actor(),
            admission.epoch(),
            Slot::MainHand,
            3,
            [1; 16],
        )
        .unwrap();
    let outfit = gateway
        .chamber
        .equip_outfit(
            Principal(key(91)),
            session,
            admission.actor(),
            admission.epoch(),
            2,
            [2; 16],
        )
        .unwrap();
    let item = gateway
        .chamber
        .use_item(
            Principal(key(91)),
            session,
            admission.actor(),
            admission.epoch(),
            1,
            [3; 16],
        )
        .unwrap();
    let claim = gateway
        .chamber
        .claim_quest(
            Principal(key(91)),
            session,
            admission.actor(),
            admission.epoch(),
            101,
        )
        .unwrap();
    for n in 3..=150 {
        gateway.grant_reward(reward(actor, n)).unwrap();
    }
    gateway.grant_reward(reward(other, 151)).unwrap();
    store.commit(&mut gateway).unwrap();
    drop(store);
    if legacy {
        let mut committed: Committed =
            read(&root.join("chamber.json"), super::super::FILE_BYTES).unwrap();
        let mut state = journal::expand(committed.checkpoint.as_bytes()).unwrap();
        state["version"] = 8.into();
        state.as_object_mut().unwrap().remove("guests");
        state.as_object_mut().unwrap().remove("configured_players");
        state.as_object_mut().unwrap().remove("owners");
        state.as_object_mut().unwrap().remove("character_schema");
        state["world"]["rules_revision"] = "verse-chamber-owned-v18".into();
        state["world"]["world"]
            .as_object_mut()
            .unwrap()
            .remove("migration_generation");
        state["world"]["world"]["spells"]
            .as_object_mut()
            .unwrap()
            .remove("generation");
        committed.checkpoint = String::from_utf8(journal::contract(&state).unwrap()).unwrap();
        committed.digest = digest(committed.revision, &committed.checkpoint);
        std::fs::write(
            root.join("chamber.json"),
            serde_json::to_vec(&committed).unwrap(),
        )
        .unwrap();
    }
    let mut target = config.clone();
    target.items.items[0].name = "Revised ember".into();
    target.items.items[0].health = 60;
    target.outfits.outfits[0].model = "revised-ranger".into();
    target.equipment.gear[0].health = 40;
    target.progression.quests[0].experience = 90;
    target.progression.quests[0].goal = 1;
    target
        .enrollments
        .retain(|e| e.public_key != enrollment(92, Role::Spectator {}).public_key);
    target.enrollments.push(enrollment(
        94,
        Role::Player {
            spawn: [-3., 0., -22.],
        },
    ));
    Fixture {
        config,
        target,
        original,
        original_receipt,
        claim,
        gear,
        outfit,
        item,
        actor,
        other,
    }
}
fn plan(store: &Store, f: &Fixture) -> Review {
    store
        .plan_migration(&f.config, &f.target, game(&f.target, true), [9; 32])
        .unwrap()
}
#[test]
fn populated_legacy_save_migrates_preserving_receipts_ownership_and_completed_quests() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, true);
    let original = std::fs::read(root.join("chamber.json")).unwrap();
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let reviewed = plan(&store, &f);
    assert_eq!(reviewed.source.character_schema, 1);
    assert_eq!(reviewed.source.save_version, 8);
    assert_eq!(reviewed.target.character_schema, 4);
    assert_eq!(reviewed.target.save_version, 12);
    assert_eq!(reviewed.source.rules, "verse-chamber-owned-v18");
    assert_eq!(reviewed.target.rules, crate::play::RULES_REVISION);
    assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), original);
    assert!(Store::open(&root, [8; 32], 120).is_err());
    let record = store
        .apply_migration(
            &f.config,
            &f.target,
            game(&f.target, true),
            [9; 32],
            &reviewed,
        )
        .unwrap();
    assert_eq!(record.after_revision, 2);
    drop(store);
    assert!(Store::open(&root, [8; 32], 120).is_err());
    let mut store = Store::open(&root, [9; 32], 120).unwrap();
    let mut gateway = store.recover().unwrap();
    f.target.validate_recovered(&gateway).unwrap();
    assert_eq!(
        gateway.grant_reward(f.original.clone()).unwrap(),
        f.original_receipt
    );
    assert_eq!(gateway.chamber.owners[&Principal(key(92))], f.other);
    assert!(!gateway.chamber.grants.contains_key(&Principal(key(92))));
    assert!(gateway.chamber.owners[&Principal(key(94))] > f.other);
    let character = gateway.character_rewards(f.actor).unwrap();
    assert_eq!(character.experience, 225);
    assert_eq!(character.items[&1], 5);
    assert_eq!(character.outfit, 2);
    assert_eq!(character.equipment[&Slot::MainHand], 3);
    let quest = gateway.quest_log(f.actor).remove(0);
    assert!(quest.claimed && quest.accepted);
    assert_eq!(quest.progress, quest.goal);
    let session = gateway.chamber.connect(Principal(key(91))).unwrap();
    let admission = gateway
        .chamber
        .admission(Principal(key(91)), session)
        .unwrap();
    assert_eq!(
        gateway
            .chamber
            .claim_quest(
                Principal(key(91)),
                session,
                admission.actor(),
                admission.epoch(),
                101
            )
            .unwrap(),
        f.claim
    );
    assert_eq!(
        gateway
            .chamber
            .equip_gear(
                Principal(key(91)),
                session,
                admission.actor(),
                admission.epoch(),
                Slot::MainHand,
                3,
                [1; 16]
            )
            .unwrap(),
        f.gear
    );
    assert_eq!(
        gateway
            .chamber
            .equip_outfit(
                Principal(key(91)),
                session,
                admission.actor(),
                admission.epoch(),
                2,
                [2; 16]
            )
            .unwrap(),
        f.outfit
    );
    assert_eq!(
        gateway
            .chamber
            .use_item(
                Principal(key(91)),
                session,
                admission.actor(),
                admission.epoch(),
                1,
                [3; 16]
            )
            .unwrap(),
        f.item
    );
    assert_eq!(
        gateway
            .game()
            .player_snapshot(admission.actor())
            .unwrap()
            .player
            .max_hp,
        250
    );
    assert!(gateway.game().actor_life(101).is_some());
    assert!(gateway.game().actor_life(6).is_none());
    assert!(gateway.game().actor_life(2).unwrap().generation > 0);
    drop(gateway);
    drop(store);
    let mut store = Store::open(&root, [9; 32], 120).unwrap();
    let undo = store.rollback_migration(&record.id).unwrap();
    assert_eq!(undo.after_revision, 3);
    drop(store);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut restored = store.recover().unwrap();
    f.config.validate_recovered(&restored).unwrap();
    assert_eq!(
        restored.grant_reward(f.original).unwrap(),
        f.original_receipt
    );
}
#[test]
fn drift_unsafe_definitions_and_later_commit_refuse_without_discarding_source() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    let original = std::fs::read(root.join("chamber.json")).unwrap();
    let other_directory =
        tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let mut foreign_source = f.config.clone();
    let mut foreign_target = f.target.clone();
    foreign_source.state_dir = Some(other_directory.path().into());
    foreign_target.state_dir = foreign_source.state_dir.clone();
    assert!(
        store
            .plan_migration(
                &foreign_source,
                &foreign_target,
                game(&foreign_target, true),
                [9; 32]
            )
            .is_err()
    );
    let mut changed = f.target.clone();
    changed.items.items[0].health += 1;
    assert!(
        store
            .apply_migration(&f.config, &changed, game(&changed, true), [9; 32], &review)
            .is_err()
    );
    changed.items.items.clear();
    assert!(
        store
            .plan_migration(&f.config, &changed, game(&changed, true), [9; 32])
            .is_err()
    );
    changed = f.target.clone();
    changed.enrollments[0] = enrollment(95, Role::Primary {});
    assert!(
        store
            .plan_migration(&f.config, &changed, game(&changed, true), [9; 32])
            .is_err()
    );
    assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), original);
    let record = store
        .apply_migration(
            &f.config,
            &f.target,
            game(&f.target, true),
            [9; 32],
            &review,
        )
        .unwrap();
    let mut gateway = store.recover().unwrap();
    gateway.tick(1. / 30.).unwrap();
    store.commit(&mut gateway).unwrap();
    drop(store);
    let mut store = Store::open(&root, [9; 32], 120).unwrap();
    assert!(store.rollback_migration(&record.id).is_err());
    assert_eq!(
        store.recover().unwrap().game().authority_tick,
        gateway.game().authority_tick
    );
}
#[test]
fn revoked_character_reenrollment_keeps_its_actor_and_balances() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    store
        .apply_migration(
            &f.config,
            &f.target,
            game(&f.target, true),
            [9; 32],
            &review,
        )
        .unwrap();
    drop(store);
    let mut next = f.target.clone();
    next.enrollments.push(enrollment(
        92,
        Role::Player {
            spawn: [5., 0., -22.],
        },
    ));
    let mut store = Store::open(&root, [9; 32], 120).unwrap();
    let review = store
        .plan_migration(&f.target, &next, game(&next, true), [10; 32])
        .unwrap();
    store
        .apply_migration(&f.target, &next, game(&next, true), [10; 32], &review)
        .unwrap();
    let gateway = store.recover().unwrap();
    assert_eq!(gateway.chamber.owners[&Principal(key(92))], f.other);
    assert_eq!(gateway.character_rewards(f.other).unwrap().experience, 1);
    assert_eq!(
        gateway.game().player_spawn(f.other).unwrap().to_array(),
        [5., 0., -22.]
    );
}
#[test]
#[ignore = "Subprocess exits deliberately; launched by the crash recovery test."]
fn crash_child() {
    let Some(root) = std::env::var_os("VERSE_MIGRATION_CRASH_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    let stage = std::env::var("VERSE_MIGRATION_CRASH_STAGE").unwrap();
    let hook = std::sync::Arc::new(move |at: &str| {
        if at == stage {
            std::process::exit(86);
        }
    });
    if let Ok(id) = std::env::var("VERSE_MIGRATION_ROLLBACK") {
        let mut store = Store::open(root, [9; 32], 120).unwrap();
        store.inject(hook);
        store.rollback_migration(&id).unwrap();
        panic!("Missing rollback crash boundary");
    }
    let source = config(root);
    let mut target = source.clone();
    target.items.items[0].health = 60;
    let mut store = Store::open(root, [8; 32], 120).unwrap();
    let review = store
        .plan_migration(&source, &target, game(&target, true), [9; 32])
        .unwrap();
    store.inject(hook);
    store
        .apply_migration(&source, &target, game(&target, true), [9; 32], &review)
        .unwrap();
    panic!("Missing crash boundary");
}
#[test]
fn crash_recovery_selects_source_before_seal_and_target_after_seal() {
    for stage in [
        "migration_prepared",
        "before_snapshot_sync",
        "after_snapshot_sync",
        "after_snapshot_rename",
        "migration_snapshot",
        "migration_journal",
        "migration_sealed",
    ] {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = dir.path().join("state");
        let f = populate(&root, false);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::persistence::migration::tests::crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("VERSE_MIGRATION_CRASH_ROOT", &root)
            .env("VERSE_MIGRATION_CRASH_STAGE", stage)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "{stage}");
        let content = if stage == "migration_sealed" {
            [9; 32]
        } else {
            [8; 32]
        };
        let mut store = Store::open(&root, content, 120).unwrap_or_else(|e| panic!("{stage}: {e}"));
        let mut gateway = store.recover().unwrap();
        assert_eq!(
            gateway.grant_reward(f.original).unwrap(),
            f.original_receipt,
            "{stage}"
        );
        assert!(!root.join("migration.pending").exists());
    }
}

#[test]
fn interrupted_rollback_keeps_the_target_until_its_own_seal() {
    for stage in [
        "migration_prepared",
        "migration_snapshot",
        "migration_journal",
        "migration_sealed",
    ] {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = dir.path().join("state");
        let f = populate(&root, false);
        let mut store = Store::open(&root, [8; 32], 120).unwrap();
        let review = plan(&store, &f);
        let record = store
            .apply_migration(
                &f.config,
                &f.target,
                game(&f.target, true),
                [9; 32],
                &review,
            )
            .unwrap();
        drop(store);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::persistence::migration::tests::crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("VERSE_MIGRATION_CRASH_ROOT", &root)
            .env("VERSE_MIGRATION_CRASH_STAGE", stage)
            .env("VERSE_MIGRATION_ROLLBACK", &record.id)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "{stage}");
        let content = if stage == "migration_sealed" {
            [8; 32]
        } else {
            [9; 32]
        };
        let mut store = Store::open(&root, content, 120).unwrap();
        let mut gateway = store.recover().unwrap();
        assert_eq!(
            gateway.grant_reward(f.original).unwrap(),
            f.original_receipt
        );
        if stage == "migration_sealed" {
            f.config.validate_recovered(&gateway).unwrap();
        } else {
            f.target.validate_recovered(&gateway).unwrap();
        }
    }
}
#[test]
fn a_storage_failure_recovers_the_source_after_storage_becomes_available() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    let obstruction = root.join("next.json");
    let injected = obstruction.clone();
    store.inject(std::sync::Arc::new(move |stage| {
        if stage == "migration_prepared" {
            std::fs::create_dir(&injected).unwrap();
        }
    }));
    assert!(
        store
            .apply_migration(
                &f.config,
                &f.target,
                game(&f.target, true),
                [9; 32],
                &review
            )
            .is_err()
    );
    drop(store);
    std::fs::remove_dir(obstruction).unwrap();
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut gateway = store.recover().unwrap();
    f.config.validate_recovered(&gateway).unwrap();
    assert_eq!(
        gateway.grant_reward(f.original).unwrap(),
        f.original_receipt
    );
}
#[test]
fn changed_source_revision_and_active_objective_are_refused() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    let mut gateway = store.recover().unwrap();
    gateway.tick(1. / 30.).unwrap();
    store.commit(&mut gateway).unwrap();
    drop(store);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    assert!(
        store
            .apply_migration(
                &f.config,
                &f.target,
                game(&f.target, true),
                [9; 32],
                &review
            )
            .is_err()
    );
    let mut gateway = store.recover().unwrap();
    let mut quest = f.config.progression.quests[0].clone();
    quest.id = 102;
    quest.giver = Some(2);
    quest.goal = 3;
    gateway.chamber.progression.quests.push(quest.clone());
    gateway
        .chamber
        .restore_reward(quest.acceptance(120, f.actor, 2))
        .unwrap();
    store.commit(&mut gateway).unwrap();
    drop(store);
    let mut source = f.config.clone();
    source.progression.quests.push(quest.clone());
    let mut target = source.clone();
    target.progression.quests[1].objective = 3;
    let store = Store::open(&root, [8; 32], 120).unwrap();
    assert!(
        store
            .plan_migration(&source, &target, game(&target, true), [9; 32])
            .unwrap_err()
            .contains("progress adapter")
    );
}

#[test]
fn reintroduced_npc_and_new_props_use_a_generation_beyond_previous_content() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    store
        .apply_migration(
            &f.config,
            &f.target,
            game(&f.target, true),
            [9; 32],
            &review,
        )
        .unwrap();
    drop(store);
    let mut store = Store::open(&root, [9; 32], 120).unwrap();
    let mut target = game(&f.target, false);
    let index = target
        .spawn_prop(
            "Migration crate",
            crate::spells::PropSpec::reference(crate::spells::PropKind::Crate),
            glam::Vec3::new(5., 1., -20.),
            0.,
        )
        .unwrap();
    let review = store
        .plan_migration(&f.target, &f.target, target.clone(), [10; 32])
        .unwrap();
    store
        .apply_migration(&f.target, &f.target, target, [10; 32], &review)
        .unwrap();
    let mut gateway = store.recover().unwrap();
    let npc = gateway.game().actor_life(6).unwrap();
    assert!(npc.generation >= 2);
    assert_eq!(
        gateway.chamber.game.spells.props[index].life.generation,
        npc.generation
    );
    let next = gateway
        .chamber
        .game
        .spawn_prop(
            "Later crate",
            crate::spells::PropSpec::reference(crate::spells::PropKind::Crate),
            glam::Vec3::new(7., 1., -20.),
            0.,
        )
        .unwrap();
    assert_eq!(
        gateway.chamber.game.spells.props[next].life.generation,
        npc.generation
    );
    gateway.checkpoint().unwrap();
}

#[test]
fn a_corrupt_backup_cannot_replace_the_applied_world() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    let record = store
        .apply_migration(
            &f.config,
            &f.target,
            game(&f.target, true),
            [9; 32],
            &review,
        )
        .unwrap();
    let active = std::fs::read(root.join("chamber.json")).unwrap();
    let backup = root
        .join("migrations")
        .join(record.id.clone())
        .join("before.json");
    let mut saved: Committed = read(&backup, super::super::FILE_BYTES).unwrap();
    saved.revision += 1;
    std::fs::write(backup, serde_json::to_vec(&saved).unwrap()).unwrap();
    assert!(store.rollback_migration(&record.id).is_err());
    assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), active);
    f.target
        .validate_recovered(&store.recover().unwrap())
        .unwrap();
}

#[test]
fn restored_backup_preserves_reviewed_rollback_and_refuses_later_progress() {
    use super::super::backup::{self, Budget};
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("source");
    let f = populate(&root, false);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let review = plan(&store, &f);
    let record = store
        .apply_migration(
            &f.config,
            &f.target,
            game(&f.target, true),
            [9; 32],
            &review,
        )
        .unwrap();
    let archive = dir.path().join("backup");
    let report = store.export_backup(&archive, Budget::default()).unwrap();
    assert_eq!(report.migrations, 1);
    let reverted = dir.path().join("reverted");
    backup::restore(&archive, &reverted, [9; 32], 120, Budget::default()).unwrap();
    let mut restored = Store::open(&reverted, [9; 32], 120).unwrap();
    assert!(Store::open(&reverted, [9; 32], 120).is_err());
    restored.rollback_migration(&record.id).unwrap();
    drop(restored);
    let mut restored = Store::open(&reverted, [8; 32], 120).unwrap();
    assert_eq!(
        restored
            .recover()
            .unwrap()
            .grant_reward(f.original.clone())
            .unwrap(),
        f.original_receipt
    );
    let advanced = dir.path().join("advanced");
    backup::restore(&archive, &advanced, [9; 32], 120, Budget::default()).unwrap();
    let mut restored = Store::open(&advanced, [9; 32], 120).unwrap();
    let mut gateway = restored.recover().unwrap();
    gateway.tick(1. / 30.).unwrap();
    restored.commit(&mut gateway).unwrap();
    assert!(restored.rollback_migration(&record.id).is_err());
}
