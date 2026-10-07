use super::*;
use std::{collections::BTreeMap, path::Path};
use verse_engine::{
    assets::{Model, Texture},
    director::{Action, Actor, Cue},
    motion::{Binding, Mode, State},
};
pub(super) fn input(path: &Path) {
    std::fs::create_dir(path).unwrap();
    let states = State::ALL
        .into_iter()
        .map(|state| {
            (
                state,
                Binding {
                    clip: 0,
                    mode: Mode::Loop,
                    transition_seconds: 0.1,
                },
            )
        })
        .collect();
    let model = Model {
        graph: None,
        markers: vec![],
        states,
        skin: None,
        source: "authored-fixture".into(),
        source_sha256: String::new(),
        surfaces: vec![],
        bones: vec![],
        clips: vec![verse_engine::assets::Clip {
            id: 0,
            duration: 1.,
            bones: vec![],
        }],
        height: 1.,
        attachments: vec![],
    };
    let mut texture = vec![];
    {
        let mut encoder = png::Encoder::new(&mut texture, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255; 4])
            .unwrap();
    }
    let pack = Pack {
        inventory: None,
        version: 1,
        source_revision: "author-fixture".into(),
        models: ["claude", "cultist", "adventurer"]
            .into_iter()
            .map(|key| (key.into(), model.clone()))
            .collect(),
        textures: vec![Texture {
            file: "fixture.png".into(),
            sha256: workspace::hash(&texture),
            width: 1,
            height: 1,
        }],
        placements: vec![],
    };
    let actor = |id, name: &str, model: &str, position: [f32; 3], nameplate| Actor {
        id,
        name: name.into(),
        model: model.into(),
        position: position.into(),
        yaw: 0.,
        scale: 1.,
        health: 100,
        nameplate,
        friendly: false,
    };
    let scene = Scene {
        collision_profile: None,
        version: 1,
        duration: 60.,
        origin: [0.; 3],
        cut_at: 0.,
        actors: vec![
            actor(1, "Boss", "claude", [0., 0., 0.], true),
            actor(2, "Guardian", "cultist", [0., 0., -8.], true),
            actor(3, "Caller", "cultist", [4., 0., -8.], true),
            actor(14, "Visitor", "adventurer", [0., 0., -22.], false),
        ],
        cues: vec![],
    };
    std::fs::write(path.join("pack.json"), serde_json::to_vec(&pack).unwrap()).unwrap();
    std::fs::write(path.join("scene.json"), serde_json::to_vec(&scene).unwrap()).unwrap();
    std::fs::write(path.join("fixture.png"), texture).unwrap();
}
fn tx(revision: u64, edits: Vec<Edit>) -> Transaction {
    Transaction {
        expected_revision: revision,
        label: "Author change".into(),
        edits,
    }
}
#[test]
fn atomic_transactions_persist_undo_redo_and_exclusive_ownership() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    assert!(Workspace::open(&path).is_err());
    let mut guardian = work
        .document()
        .scene
        .actors
        .iter()
        .find(|a| a.id == 2)
        .unwrap()
        .clone();
    guardian.health = 175;
    assert_eq!(
        work.transact(&tx(
            1,
            vec![
                Edit::Zone {
                    name: "second-zone".into()
                },
                Edit::Actor { actor: guardian }
            ]
        ))
        .unwrap(),
        2
    );
    let journal = std::fs::read(path.join("journal.json")).unwrap();
    assert!(
        work.transact(&tx(1, vec![Edit::RemoveActor { id: 14 }]))
            .is_err()
    );
    assert!(
        work.transact(&tx(
            2,
            vec![
                Edit::Zone {
                    name: "third-zone".into()
                },
                Edit::RemoveActor { id: 14 }
            ]
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(path.join("journal.json")).unwrap(), journal);
    assert_eq!(work.revision(), 2);
    drop(work);
    let mut work = Workspace::open(&path).unwrap();
    assert_eq!(work.undo(2).unwrap(), 3);
    assert_eq!(work.document().zone, "first-zone");
    assert!(work.redo(2).is_err());
    assert_eq!(work.redo(3).unwrap(), 4);
    assert_eq!(work.document().zone, "second-zone");
    assert_eq!(
        work.document()
            .scene
            .actors
            .iter()
            .find(|a| a.id == 2)
            .unwrap()
            .health,
        175
    );
    work.undo(4).unwrap();
    work.transact(&tx(
        5,
        vec![Edit::Zone {
            name: "fork-zone".into(),
        }],
    ))
    .unwrap();
    assert!(work.redo(6).is_err());
}
#[test]
fn field_diagnostics_preserve_journal_and_running_generation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    let mut active = work.preview().unwrap();
    active.step(4).unwrap();
    let original = active.content();
    let before = active.report(None).unwrap();
    let mut invalid = work.document().clone();
    invalid.scene.actors[0].model = "missing-model".into();
    let error = active
        .reload(&invalid, &work.base, &path.join("assets"))
        .err()
        .unwrap();
    assert!(error.field.contains("scene.actors[id=1].model"));
    assert_eq!(active.content(), original);
    assert_eq!(active.report(None).unwrap().tick, before.tick);
    let invalid_cue = Edit::Cue {
        cue: TimelineCue {
            id: 8,
            cue: Cue {
                at: 1.,
                actor: 999,
                action: Action::CameraCut,
            },
        },
    };
    let error = work.transact(&tx(1, vec![invalid_cue])).err().unwrap();
    assert_eq!(error.field, "timeline[id=8].cue");
    let mut malformed = serde_json::to_value(work.document()).unwrap();
    malformed["authored"]["blockers"] = serde_json::json!({"1":{"min":[0,0,"bad"],"max":[1,1,1]}});
    let error = parse::<Document>(
        "input.json",
        &serde_json::to_vec(&malformed).unwrap(),
        2 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert_eq!(error.source, "input.json");
    assert!(
        error.field.contains("authored.blockers.1.min[2]"),
        "{error}"
    );
    assert_eq!(work.revision(), 1);
    let mut settings = work.document().authored.clone();
    let mut character = verse_world::content::Character::default();
    character.catalog.get_mut(&1).unwrap().cooldown = f32::NAN;
    settings.character = Some(character);
    assert!(
        work.transact(&tx(1, vec![Edit::Authored { settings }]))
            .is_err()
    );
    assert_eq!(work.revision(), 1);
}
#[test]
fn bounded_history_and_session_errors_leave_the_editor_usable() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    for i in 0..35 {
        work.transact(&tx(
            work.revision(),
            vec![Edit::Zone {
                name: format!("zone-{i}"),
            }],
        ))
        .unwrap();
    }
    assert_eq!(work.journal().undo.len(), 32);
    let mut commands = std::io::Cursor::new(
        b"{\"action\":\"undo\",\"expected_revision\":1}\n{\"action\":\"preview\",\"ticks\":1}\n"
            .to_vec(),
    );
    let mut output = vec![];
    cli::session(&mut work, &mut commands, &mut output).unwrap();
    let rows = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["ok"], false);
    assert_eq!(rows[1]["ok"], true);
    let mut oversized = std::io::Cursor::new(vec![b'x'; 2 * 1024 * 1024 + 1]);
    assert!(cli::session(&mut work, &mut oversized, &mut vec![]).is_err());
}
#[test]
fn immutable_build_reuse_and_tamper_refusal_keep_current_generation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    let first = work.build().unwrap();
    assert!(!first.reused);
    let again = work.build().unwrap();
    assert!(again.reused);
    assert_eq!(first.content, again.content);
    let current = std::fs::read(path.join("current.json")).unwrap();
    std::fs::write(first.path.join("scene.json"), b"{}").unwrap();
    assert!(work.build().is_err());
    assert_eq!(std::fs::read(path.join("current.json")).unwrap(), current);
    work.transact(&tx(
        1,
        vec![Edit::Zone {
            name: "second-zone".into(),
        }],
    ))
    .unwrap();
    let second = work.build().unwrap();
    assert_ne!(first.generation, second.generation);
    let current = std::fs::read(path.join("current.json")).unwrap();
    std::fs::write(path.join("assets/fixture.png"), b"corrupt").unwrap();
    assert!(work.build().is_err());
    assert_eq!(std::fs::read(path.join("current.json")).unwrap(), current);
}
#[test]
fn quest_references_catalog_tuning_and_collision_use_runtime_contracts() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    let mut giver = work.document().scene.actors[1].clone();
    giver.id = 100;
    giver.name = "Archivist".into();
    giver.friendly = true;
    giver.position = [2., 0., -22.].into();
    let quest = verse_world::service::progression::Quest {
        repeatable: false,
        dialogue: None,
        giver: Some(100),
        prerequisites: vec![],
        id: 1,
        name: "Secure outpost".into(),
        objective: 10,
        goal: 1,
        experience: 25,
        items: vec![],
    };
    let failed = work
        .transact(&tx(
            1,
            vec![Edit::Quest {
                quest: quest.clone(),
            }],
        ))
        .err()
        .unwrap();
    assert!(failed.field.ends_with(".giver"));
    let policies = vec![verse_world::service::rewards::Policy {
        participation: Default::default(),
        target: 2,
        experience: 10,
        items: vec![],
        quests: vec![verse_world::service::rewards::Entry { id: 10, count: 1 }],
    }];
    let mut character = verse_world::content::Character::default();
    character.catalog.get_mut(&1).unwrap().cost = 3;
    let settings = verse_world::content::Authored {
        navigation: Some(verse_world::content::NavigationRegion {
            min: [-12., -1., -26.],
            max: [12., 4., 12.],
            cell: 1.,
        }),
        character: Some(character),
        blockers: BTreeMap::from([
            (
                1,
                verse_world::content::Bounds {
                    min: [-10., -1., -10.],
                    max: [10., 0., 10.],
                },
            ),
            (
                2,
                verse_world::content::Bounds {
                    min: [-1., 0., -1.],
                    max: [1., 3., 1.],
                },
            ),
        ]),
    };
    work.transact(&tx(
        1,
        vec![
            Edit::Actor { actor: giver },
            Edit::Quest { quest },
            Edit::Rewards { policies },
            Edit::Authored { settings },
        ],
    ))
    .unwrap();
    let preview = work.preview().unwrap();
    assert_eq!(preview.gateway().quest_log(14)[0].giver, Some(100));
    let report = preview.report(Some(10.)).unwrap();
    let nodes = report.navigation.as_ref().unwrap()["nodes"]
        .as_array()
        .unwrap();
    assert!(!nodes.is_empty());
    assert!(nodes.iter().all(|n| {
        let p = &n["feet"];
        p[1].as_f64().unwrap() > 2.
            || p[0].as_f64().unwrap().abs() > 1.
            || p[2].as_f64().unwrap().abs() > 1.
    }));
    assert!(
        report
            .geometry
            .colliders
            .iter()
            .any(|s| s.key.life.entity == 2_000_002)
    );
    assert!(preview.svg(Some(10.)).unwrap().contains("navigation spans"));
    let previous = preview.content();
    let mut doc = work.document().clone();
    doc.progression.quests[0].experience = 30;
    assert_ne!(
        previous,
        Preview::new(&doc, &work.base, &path.join("assets"))
            .unwrap()
            .content()
    );
}
#[test]
#[cfg(unix)]
fn symlinks_and_interrupted_builds_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    std::fs::create_dir_all(path.join("generations/building")).unwrap();
    assert!(work.build().is_err());
    assert!(!path.join("current.json").exists());
    std::fs::create_dir(path.join("outside")).unwrap();
    std::os::unix::fs::symlink(path.join("outside"), path.join("linked")).unwrap();
    assert!(workspace::write_preview(&path.join("linked/view.svg"), b"<svg/>").is_err());
    assert!(!path.join("outside/view.svg").exists());
    let mut active = Some(work.preview().unwrap());
    let identity = active.as_ref().unwrap().content();
    let mut work = work;
    assert!(
        cli::command(
            &mut work,
            &mut active,
            &cli::Command::Preview {
                ticks: 1,
                navigation: Some(1000.)
            }
        )
        .is_err()
    );
    assert_eq!(active.unwrap().content(), identity);
}

#[test]
fn generic_encounter_ids_and_timeline_survive_restart_and_restore() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "first-zone".into()).unwrap();
    let mut guardian = work
        .document()
        .scene
        .actors
        .iter()
        .find(|a| a.id == 2)
        .unwrap()
        .clone();
    guardian.id = 41;
    let settings = verse_world::content::Authored {
        navigation: Some(verse_world::content::NavigationRegion {
            min: [-12., -1., -26.],
            max: [12., 4., 12.],
            cell: 1.,
        }),
        character: Some(verse_world::content::Character::default()),
        blockers: BTreeMap::from([(
            7,
            verse_world::content::Bounds {
                min: [-4., 0., -16.],
                max: [-2., 2., -14.],
            },
        )]),
    };
    work.transact(&tx(
        1,
        vec![
            Edit::RemoveActor { id: 1 },
            Edit::RemoveActor { id: 2 },
            Edit::RemoveActor { id: 3 },
            Edit::Actor { actor: guardian },
            Edit::Cue {
                cue: TimelineCue {
                    id: 91,
                    cue: Cue {
                        at: 0.,
                        actor: 41,
                        action: Action::Yell {
                            text: "Authored greeting".into(),
                            animation: State::Yell.into(),
                        },
                    },
                },
            },
            Edit::Authored { settings },
        ],
    ))
    .unwrap();
    let mut preview = work.preview().unwrap();
    preview.step(5).unwrap();
    assert_eq!(preview.gateway().game().frame().yell.unwrap().actor, 41);
    let before = preview.gateway().checkpoint().unwrap();
    let content = preview.content();
    let mut restored = verse_world::service::auth::Gateway::restore(&before, content, 1).unwrap();
    restored.reset().unwrap();
    let game = restored.game();
    assert_eq!(
        game.actor_state(game.player_life()).unwrap().definition.key,
        "chamber-wizard"
    );
    assert!(
        game.collision_geometry()
            .unwrap()
            .colliders
            .iter()
            .any(|c| c.key.life.entity == 2_000_007)
    );
    restored.tick(1. / 30.).unwrap();
    assert_eq!(restored.game().frame().yell.unwrap().actor, 41);
    assert!(
        verse_world::service::auth::Gateway::restore(&restored.checkpoint().unwrap(), content, 1)
            .is_ok()
    );
}

#[test]
fn authored_geometry_has_world_scale_outward_faces_and_stable_identity() {
    let shape = BoxGeometry {
        min: [-4., 0., -16.],
        max: [-2., 2., -14.],
        texture: 0,
        tint: [0.4, 0.3, 0.2],
    };
    let model = geometry::compile("author/barricade", &shape, 1).unwrap();
    for face in model.surfaces[0].indices.chunks_exact(3) {
        let vertices = &model.surfaces[0].vertices;
        let point = |i: u32| crate::basis().transform_point3(vertices[i as usize].position.into());
        let a = point(face[0]);
        let b = point(face[1]);
        let c = point(face[2]);
        let normal = crate::basis()
            .transform_vector3(vertices[face[0] as usize].normal.into())
            .normalize();
        assert!((b - a).cross(c - a).dot(normal) > 0.);
        assert!(
            a.cmpge(glam::Vec3::from_array(shape.min) - glam::Vec3::splat(1e-4))
                .all()
        );
        assert!(
            a.cmple(glam::Vec3::from_array(shape.max) + glam::Vec3::splat(1e-4))
                .all()
        );
    }
    assert_eq!(
        model.source_sha256,
        geometry::compile("author/barricade", &shape, 1)
            .unwrap()
            .source_sha256
    );
    assert!(geometry::compile("author/barricade", &shape, 0).is_err());
}

#[test]
fn numeric_keys_survive_tagged_commands_and_noncanonical_keys_are_refused() {
    let mut settings = verse_world::content::Authored::default();
    settings.character = Some(verse_world::content::Character::default());
    settings.blockers.insert(
        1,
        verse_world::content::Bounds {
            min: [-1., 0., -1.],
            max: [1., 1., 1.],
        },
    );
    let command = cli::Command::Transaction {
        transaction: tx(
            1,
            vec![
                Edit::Authored { settings },
                Edit::Model {
                    key: "cultist".into(),
                    edit: ModelEdit {
                        graph: None,
                        states: None,
                        materials: BTreeMap::from([(0, Default::default())]),
                    },
                },
            ],
        ),
    };
    let bytes = serde_json::to_vec(&command).unwrap();
    let parsed: cli::Command = parse("command.json", &bytes, 2 * 1024 * 1024).unwrap();
    assert_eq!(serde_json::to_vec(&parsed).unwrap(), bytes);
    let invalid = String::from_utf8(bytes)
        .unwrap()
        .replace("\"1\":{\"min\"", "\"01\":{\"min\"");
    assert!(parse::<cli::Command>("command.json", invalid.as_bytes(), 2 * 1024 * 1024).is_err());
    let duplicate = br#"{"op":"authored","settings":{"character":null,"blockers":{"1":{"min":[0,0,0],"max":[1,1,1]},"1":{"min":[0,0,0],"max":[2,2,2]}}}}"#;
    assert!(parse::<Edit>("duplicate.json", duplicate, 2 * 1024 * 1024).is_err());
}

#[test]
fn authored_mips_survive_snapshot_and_new_material_variants_reuse_them() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let pack = verse_engine::assets::Pack::read(&source.join("pack.json")).unwrap();
    let prepared =
        verse_engine::loading::Prepared::load(pack.clone(), &source, Default::default()).unwrap();
    let original = verse_engine::mips::archive::Archive::cook(&prepared).unwrap();
    let custom = [200, 50, 120, 255];
    let mut manifest = serde_json::to_value(original.manifest()).unwrap();
    manifest["rgba_sha256"] = serde_json::json!(workspace::hash(&custom));
    let authored = verse_engine::mips::archive::Archive::from_bytes(
        &pack,
        &serde_json::to_vec(&manifest).unwrap(),
        &custom,
    )
    .unwrap();
    std::fs::write(
        source.join(verse_engine::mips::archive::MANIFEST),
        authored.encoded_manifest().unwrap(),
    )
    .unwrap();
    std::fs::write(
        source.join(verse_engine::mips::archive::PAYLOAD),
        authored.payload(),
    )
    .unwrap();
    let mut work =
        Workspace::init(&source, &root.path().join("work"), "authored-zone".into()).unwrap();
    assert_eq!(work.preview().unwrap().mips.payload(), custom);
    let edits = vec![
        Edit::Primitive {
            key: "author/box".into(),
            shape: BoxGeometry {
                min: [-1., 0., -1.],
                max: [1., 1., 1.],
                texture: 0,
                tint: [1.; 3],
            },
        },
        Edit::Model {
            key: "author/box".into(),
            edit: ModelEdit {
                graph: None,
                states: None,
                materials: BTreeMap::from([(
                    0,
                    verse_engine::material::Material {
                        normal_texture: Some(0),
                        ..Default::default()
                    },
                )]),
            },
        },
    ];
    work.transact(&tx(1, edits)).unwrap();
    drop(work);
    let work = Workspace::open(&root.path().join("work")).unwrap();
    let preview = work.preview().unwrap();
    assert_eq!(
        preview
            .mips
            .levels(
                verse_engine::mips::Variant {
                    texture: 0,
                    role: verse_engine::mips::Role::Color
                },
                8192
            )
            .unwrap()[0]
            .2,
        custom
    );
    assert_eq!(preview.mips.manifest().entries.len(), 2);
    let build = work.build().unwrap();
    assert_eq!(build.content, preview.content());
    let prepared = verse_engine::loading::Prepared::load(
        verse_engine::assets::Pack::read(&build.path.join("pack.json")).unwrap(),
        &build.path,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepared.mips().unwrap().identity().unwrap(),
        preview.mips.identity().unwrap()
    );
}

#[test]
fn graph_transactions_expose_controls_and_preserve_history_on_refusal() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    input(&source);
    let source_pack = source.join("pack.json");
    let mut pack: Pack = serde_json::from_slice(&std::fs::read(&source_pack).unwrap()).unwrap();
    for model in pack.models.values_mut() {
        model.bones.push(verse_engine::assets::Bone {
            parent: -1,
            pivot: [0.; 3],
        });
        model.graph = Some(verse_engine::animation_graph::Authored::from_bindings(
            model,
        ));
    }
    std::fs::write(source_pack, serde_json::to_vec(&pack).unwrap()).unwrap();

    let path = root.path().join("work");
    let mut work = Workspace::init(&source, &path, "graph-zone".into()).unwrap();
    let mut graph: verse_engine::animation_graph::Authored = serde_json::from_value(
        work.inspect().unwrap()["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["key"] == "adventurer")
            .unwrap()["graph"]
            .clone(),
    )
    .unwrap();
    graph.graph.states[0].transitions[0].seconds = 0.3;
    work.transact(&tx(
        1,
        vec![Edit::Model {
            key: "adventurer".into(),
            edit: ModelEdit {
                graph: Some(graph.clone()),
                ..Default::default()
            },
        }],
    ))
    .unwrap();
    assert_eq!(
        work.document().models["adventurer"].graph.as_ref(),
        Some(&graph)
    );
    let before = std::fs::read(path.join("journal.json")).unwrap();
    graph.locomotion = Some(verse_engine::locomotion::Definition::universal(0));
    let error = work
        .transact(&tx(
            2,
            vec![Edit::Model {
                key: "adventurer".into(),
                edit: ModelEdit {
                    graph: Some(graph),
                    ..Default::default()
                },
            }],
        ))
        .unwrap_err();
    assert_eq!(error.field, "models.adventurer.graph");
    assert_eq!(std::fs::read(path.join("journal.json")).unwrap(), before);
    let mut preview = work.preview().unwrap();
    let sampled = preview.step(15).unwrap();
    assert!(!sampled.animation.is_empty());
    assert_eq!(
        serde_json::to_value(&sampled.animation).unwrap(),
        serde_json::to_value(preview.report(None).unwrap().animation).unwrap()
    );
    work.undo(2).unwrap();
    assert!(!work.document().models.contains_key("adventurer"));
    work.redo(3).unwrap();
    assert!(work.document().models["adventurer"].graph.is_some());
}
