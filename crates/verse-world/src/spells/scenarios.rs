//! Recorded chamber commands for every physics spell.
use super::{PROP_ENTITY_BASE, PropKind, PropSpec, command::Command};
use crate::{
    play::Ability,
    playground::{Cue, Scenario, Shot, Step, creature},
};
use glam::{DVec3, Vec3};

fn caster(scene: &mut verse_engine::director::Scene) {
    scene.actors[0].position = Vec3::new(0., 0., -8.);
    scene.actors[0].yaw = std::f32::consts::PI;
}
fn camera() -> Vec<Shot> {
    vec![
        Shot {
            at: 0.,
            eye: Vec3::new(15., 12., -17.),
            target: Vec3::new(0., 3., -2.),
        },
        Shot {
            at: 40.,
            eye: Vec3::new(13., 15., 10.),
            target: Vec3::new(0., 4., -2.),
        },
    ]
}
fn targets(slot: u8, target: u64) -> Step {
    Step::Cast(Ability::SpellCommand(Command::Targets {
        slot,
        targets: [target, 0, 0, 0, 0],
        count: 1,
    }))
}
fn cast(at: f32, step: Step) -> Cue {
    Cue { at, step }
}
fn props(game: &mut crate::play::Game, _: &crate::playground::Hall) -> Result<(), String> {
    game.spawn_prop(
        "Crate",
        PropSpec::reference(PropKind::Crate),
        Vec3::new(2., 0.3, -5.),
        0.,
    )?;
    game.spawn_prop(
        "Barrel",
        PropSpec::reference(PropKind::Barrel),
        Vec3::new(-1., 0.45, -4.),
        0.,
    )?;
    game.spawn_prop(
        "Anvil",
        PropSpec::reference(PropKind::Anvil),
        Vec3::new(1., 0.2, -2.),
        0.,
    )?;
    let mut stone = PropSpec::reference(PropKind::StoneBlock);
    stone.size = super::Size::Huge;
    game.spawn_prop("Stone block", stone, Vec3::new(-4., 0.5, -3.), 0.)?;
    Ok(())
}

pub fn telekinesis() -> Scenario {
    Scenario {
        key: "telekinesis",
        title: "Telekinesis",
        srd: crate::telekinesis::SRD_LINE,
        seed: 452,
        live: 60.,
        speed: 3,
        replay: (1.2, 2.2),
        setup: |scene, _| {
            caster(scene);
            for (id, x) in [(101, 0.), (102, -2.)] {
                scene.actors.push(creature(
                    id,
                    "Dummy",
                    "dummy",
                    Vec3::new(x, 0., 0.),
                    0.,
                    200,
                ));
            }
            Ok(())
        },
        populate: |game, hall| {
            props(game, hall)?;
            for i in 0..8 {
                let mut spec = PropSpec::reference(PropKind::Crate);
                spec.dimensions = DVec3::new(0.1, 1.2, 0.6);
                spec.mass = 8.;
                game.spawn_prop(
                    "Standing plank",
                    spec,
                    Vec3::new(3. + i as f32 * 0.7, 0.6, -5.),
                    0.,
                )?;
            }
            for i in 0..4 {
                game.spawn_prop(
                    "Tower crate",
                    PropSpec::reference(PropKind::Crate),
                    Vec3::new(-6. + i as f32 * 0.75, 0.3, -5.),
                    0.,
                )?;
            }
            game.spawn_prop(
                "Range crate",
                PropSpec::reference(PropKind::Crate),
                Vec3::new(0., 0.3, -6.),
                0.,
            )?;
            for i in 0..4 {
                game.spawn_prop(
                    "Throw target stack",
                    PropSpec::reference(PropKind::Crate),
                    Vec3::new(8.7 + (i % 2) as f32 * 0.61, 0.3 + (i / 2) as f32 * 0.6, -5.),
                    0.,
                )?;
            }
            game.spells.dice.force_save(101, 2)?;
            game.spells.dice.force_save(102, 20)?;
            Ok(())
        },
        script: || {
            let mut cues = vec![
                cast(0.5, targets(0, PROP_ENTITY_BASE)),
                cast(
                    0.6,
                    Step::Cast(Ability::SpellCommand(Command::Hand([2000, 900, -5000]))),
                ),
                cast(
                    1.2,
                    Step::Cast(Ability::SpellCommand(Command::Hand([9000, 900, -5000]))),
                ),
                cast(2.2, Step::Cast(Ability::SpellCommand(Command::Release))),
                cast(7., targets(0, 101)),
                cast(14., targets(0, 102)),
                cast(21., targets(0, PROP_ENTITY_BASE + 3)),
                cast(
                    25.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
            ];
            for i in 0..4 {
                let at = 27. + i as f32 * 6.1;
                cues.push(cast(at, targets(0, PROP_ENTITY_BASE + 12 + i)));
                cues.push(cast(
                    at + 0.1,
                    Step::Cast(Ability::SpellCommand(Command::Hand([
                        -4000,
                        300 + i as i32 * 600,
                        -4000,
                    ]))),
                ));
                cues.push(cast(
                    at + 2.,
                    Step::Cast(Ability::SpellCommand(Command::Release)),
                ));
            }
            cues.push(cast(51.5, targets(0, PROP_ENTITY_BASE + 16)));
            cues.push(cast(
                51.6,
                Step::Cast(Ability::SpellCommand(Command::Hand([0, 1500, 1000]))),
            ));
            for i in 0..50 {
                cues.push(cast(53. + i as f32 * 0.1, Step::Move([-1., 0.])));
            }
            cues.push(cast(
                58.,
                Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
            ));
            cues
        },
        camera,
        replay_camera: (Vec3::new(8., 5., -12.), Vec3::new(5., 1., -5.)),
        check: |game| {
            if game
                .spells
                .telekinesis
                .iter()
                .any(|e| e.grip.grip.is_some())
            {
                return Err("A Telekinesis grip remained after concentration ended".into());
            }
            if !game.spells.log.iter().any(|r| r.spell == "Falling") {
                return Err("The released dummy did not take falling damage".into());
            }
            Ok(())
        },
    }
}

pub fn levitate() -> Scenario {
    Scenario {
        key: "levitate",
        title: "Levitate",
        srd: "Level 2 Transmutation | Range 60 ft | CON save | 20-ft rise | concentration 10 min",
        seed: 454,
        live: 57.,
        speed: 3,
        replay: (3., 4.5),
        setup: |scene, _| {
            caster(scene);
            scene.actors.push(creature(
                102,
                "Successful-save dummy",
                "dummy",
                Vec3::new(2., 0., -3.),
                0.,
                200,
            ));
            scene.actors.push(creature(
                101,
                "Floating dummy",
                "dummy",
                Vec3::new(-4.8, 0., -12.),
                0.,
                200,
            ));
            Ok(())
        },
        populate: |game, hall| {
            props(game, hall)?;
            let mut load = PropSpec::reference(PropKind::Anvil);
            load.mass = 300.;
            game.spawn_prop(
                "Anvil plus load (300 kg)",
                load,
                Vec3::new(4., 0.3, -6.),
                0.,
            )?;
            let body = game.spells.props[0].body;
            game.spells.world[body].pos.x = 1.5;
            let mut wall = PropSpec::reference(PropKind::StoneBlock).secured();
            wall.dimensions = DVec3::new(0.3, 6., 4.);
            game.spawn_prop("Drift bounce wall", wall, Vec3::new(8., 3., -12.), 0.)?;
            game.spells.dice.force_save(101, 2)?;
            game.spells.dice.force_save(102, 20)?;
            Ok(())
        },
        script: || {
            vec![
                cast(0.5, targets(2, 101)),
                cast(
                    3.,
                    Step::Walk {
                        actor: 101,
                        target: [20., 0., -12.],
                        speed: 3.,
                    },
                ),
                cast(
                    14.,
                    Step::Cast(Ability::SpellCommand(Command::Altitude(-6096))),
                ),
                cast(
                    20.1,
                    Step::Cast(Ability::SpellCommand(Command::Altitude(6096))),
                ),
                cast(
                    26.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
                cast(32., targets(2, PROP_ENTITY_BASE)),
                cast(
                    32.1,
                    Step::Cast(Ability::SpellCommand(Command::Altitude(-4000))),
                ),
                cast(38., Step::Cast(Ability::Thunderwave)),
                cast(
                    40.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
                cast(
                    46.,
                    Step::Refused(Ability::SpellCommand(Command::Targets {
                        slot: 2,
                        targets: [PROP_ENTITY_BASE + 4, 0, 0, 0, 0],
                        count: 1,
                    })),
                ),
                cast(47., targets(2, 102)),
            ]
        },
        camera: levitate_camera,
        replay_camera: (Vec3::new(4., 8., -20.), Vec3::new(-3., 4., -12.)),
        check: |game| {
            if game.spells.log.iter().any(|r| r.spell == "Falling") {
                return Err("A levitated target took falling damage".into());
            }
            Ok(())
        },
    }
}

pub fn area(slot: u8) -> Scenario {
    if slot == 1 {
        return stone();
    }
    if slot == 4 {
        return wind();
    }
    if slot == 8 {
        return gravity();
    }
    let (key, title, srd) = match slot {
        1 => (
            "wall-of-stone",
            "Wall of Stone",
            crate::wall_of_stone::SRD_LINE,
        ),
        4 => (
            "gust-of-wind",
            "Gust of Wind",
            "Level 2 Evocation | Self: 60 by 10 ft Line | STR save | push 15 ft",
        ),
        5 => (
            "wind-wall",
            "Wind Wall",
            "Level 3 Evocation | range 120 ft | STR save | 4d8 appearance damage",
        ),
        6 => (
            "black-tentacles",
            "Black Tentacles",
            "Level 4 Conjuration | range 90 ft | STR save | 20-ft square | 3d6",
        ),
        7 => (
            "meteor-swarm",
            "Meteor Swarm",
            "Level 9 Evocation | range 1 mile | DEX save | four 40-ft spheres | 20d6 fire + 20d6 bludgeoning",
        ),
        8 => (
            "reverse-gravity",
            "Reverse Gravity",
            crate::reverse_gravity::SRD_LINE,
        ),
        _ => unreachable!(),
    };
    let script: fn() -> Vec<Cue> = match slot {
        1 => || {
            vec![
                cast(0.5, Step::Cast(Ability::Spell(1))),
                cast(
                    12.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
            ]
        },
        4 => || {
            vec![
                cast(0.5, Step::Face(std::f32::consts::PI)),
                cast(0.6, Step::Cast(Ability::Spell(4))),
                cast(
                    12.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
            ]
        },
        5 => || {
            let mut cues = vec![
                cast(0.5, Step::Face(std::f32::consts::PI)),
                cast(0.6, Step::Cast(Ability::Spell(5))),
            ];
            for i in 0..20 {
                cues.push(cast(1. + i as f32 * 1.05, Step::Cast(Ability::Bow)));
            }
            cues.push(cast(
                2.,
                Step::Impulse {
                    prop: 4,
                    impulse: [0., 180., 200.],
                },
            ));
            cues.push(cast(
                2.,
                Step::Impulse {
                    prop: 5,
                    impulse: [0., 0., 160.],
                },
            ));
            cues.push(cast(
                22.,
                Step::Flight {
                    origin: [-1.8, 1.1, -8.],
                    direction: [0., 0., 1.],
                    siege: true,
                },
            ));
            cues.push(cast(
                24.,
                Step::Cast(Ability::SpellCommand(Command::WindWall {
                    points: [
                        [-5000, -8000],
                        [-3500, -4500],
                        [0, -3000],
                        [3500, -4500],
                        [5000, -8000],
                        [0, 0],
                        [0, 0],
                        [0, 0],
                    ],
                    count: 5,
                })),
            ));
            for (origin, direction) in [
                ([0.8, 1.1, 3.], [0., 0., -1.]),
                ([8., 1.1, -6.], [-1., 0., 0.]),
                ([-8., 1.1, -6.], [1., 0., 0.]),
            ] {
                cues.push(cast(
                    25.,
                    Step::Flight {
                        origin,
                        direction,
                        siege: false,
                    },
                ));
            }
            cues.push(cast(
                29.,
                Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
            ));
            cues
        },
        6 => || {
            let mut cues = vec![
                cast(
                    0.5,
                    Step::Cast(Ability::SpellCommand(Command::Point {
                        slot: 6,
                        point: [0, 0, -3000],
                    })),
                ),
                cast(
                    2.,
                    Step::Impulse {
                        prop: 0,
                        impulse: [0., 0., -480.],
                    },
                ),
            ];
            for actor in 101..=103 {
                cues.push(cast(
                    2.,
                    Step::Walk {
                        actor,
                        target: [(actor as f32 - 102.) * 1.8, 0., -5.],
                        speed: 3.,
                    },
                ));
            }
            cues.push(cast(5., Step::Cast(Ability::Thunderwave)));
            cues.push(cast(
                8.,
                Step::Save {
                    actor: 101,
                    roll: 20,
                },
            ));
            cues.push(cast(9.1, Step::Escape(101)));
            cues.push(cast(
                8.2,
                Step::Walk {
                    actor: 101,
                    target: [-8., 0., -3.],
                    speed: 3.,
                },
            ));
            cues.push(cast(
                13.,
                Step::ExpectActor {
                    actor: 101,
                    min: [-8.5, -0.1, -3.5],
                    max: [-7.5, 0.5, -2.5],
                },
            ));
            cues.push(cast(
                16.,
                Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
            ));
            cues
        },
        7 => || vec![cast(0.5, Step::Cast(Ability::Spell(7)))],
        8 => || {
            vec![
                cast(0.5, Step::Cast(Ability::Spell(8))),
                cast(
                    10.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
            ]
        },
        _ => unreachable!(),
    };
    Scenario {
        key,
        title,
        srd,
        seed: 451 + u64::from(slot),
        live: if slot == 5 { 32. } else { 18. },
        speed: if slot == 5 { 2 } else { 1 },
        replay: if slot == 5 {
            (1., 2.5)
        } else if slot == 6 {
            (3., 4.)
        } else if slot == 7 {
            (2.2, 3.2)
        } else {
            (0.4, 1.4)
        },
        setup: if slot == 6 {
            tentacle_setup
        } else if slot == 7 {
            meteor_setup
        } else {
            |scene, _| {
                caster(scene);
                for i in 0..3 {
                    scene.actors.push(creature(
                        101 + i,
                        "Dummy",
                        "dummy",
                        Vec3::new((i as f32 - 1.) * 1.8, 0., -3.),
                        0.,
                        400,
                    ));
                }
                Ok(())
            }
        },
        populate: match slot {
            5 => wind_wall_props,
            6 => tentacle_props,
            7 => meteor_props,
            _ => props,
        },
        script,
        camera: if slot == 7 { meteor_camera } else { camera },
        replay_camera: (Vec3::new(11., 9., -12.), Vec3::new(0., 3., -3.)),
        check: |game| {
            let spells = &game.spells;
            if spells.walls.is_empty()
                && spells.gusts.is_empty()
                && spells.wind_walls.is_empty()
                && spells.tentacles.is_empty()
                && spells.meteors.is_empty()
                && spells.reversed.is_empty()
            {
                return Err("The scenario did not create its spell effect".into());
            }
            if !spells.concentration.is_empty() {
                return Err("Concentration remained after the scripted end".into());
            }
            if !spells.wind_walls.is_empty() && game.snapshot().counters.deflections != 23 {
                return Err(format!(
                    "Expected 23 straight and arc arrow deflections; got {}",
                    game.snapshot().counters.deflections
                ));
            }
            for meteor in &spells.meteors {
                if meteor.swarm.impacts.len() != 4 {
                    return Err("Meteor Swarm did not produce four impacts".into());
                }
                if !(101..=106).all(|id| meteor.swarm.affected.contains(&id)) {
                    return Err("Meteor Swarm did not damage each overlapping dummy once".into());
                }
            }
            for tentacles in &spells.tentacles {
                if !tentacles.spell.ended
                    || tentacles
                        .spell
                        .tentacles
                        .iter()
                        .flat_map(|t| &t.segments)
                        .any(|b| !spells.world[*b].removed)
                {
                    return Err("Tentacle bodies remained after concentration ended".into());
                }
            }
            for wall in &spells.walls {
                if !wall.wall.vanished {
                    return Err("Stone panels remained after concentration ended".into());
                }
            }
            Ok(())
        },
    }
}

fn meteor_props(
    game: &mut crate::play::Game,
    hall: &crate::playground::Hall,
) -> Result<(), String> {
    props(game, hall)?;
    for i in 0..4 {
        let mut spec = PropSpec::reference(PropKind::Crate);
        spec.hit_points = Some(30);
        game.spawn_prop(
            "Crate stack",
            spec,
            Vec3::new(3. + (i % 2) as f32 * 0.61, 0.3 + (i / 2) as f32 * 0.6, 2.),
            0.,
        )?;
    }
    for (x, y) in [(-0.31, 0.45), (0.31, 0.45), (0., 1.35)] {
        let mut spec = PropSpec::reference(PropKind::Barrel);
        spec.hit_points = Some(40);
        game.spawn_prop("Barrel pyramid", spec, Vec3::new(-4. + x, y, 1.), 0.)?;
    }
    for i in 0..3 {
        let mut spec = PropSpec::reference(PropKind::Crate);
        spec.dimensions = DVec3::new(0.15, 1.2, 0.6);
        spec.mass = 8.;
        spec.hit_points = Some(20);
        game.spawn_prop(
            "Wooden fence",
            spec,
            Vec3::new(-1. + i as f32 * 0.6, 0.6, 3.),
            0.,
        )?;
    }
    for level in 0..3 {
        let mut spec = PropSpec::reference(PropKind::StoneBlock);
        spec.hit_points = Some(100);
        game.spawn_prop(
            "Stone-block tower",
            spec,
            Vec3::new(6., 0.5 + level as f32, -1.),
            0.,
        )?;
    }
    let mut roof = PropSpec::reference(PropKind::StoneBlock).secured();
    roof.dimensions = DVec3::new(4., 0.3, 4.);
    roof.hit_points = Some(500);
    game.spawn_prop("Meteor overhang", roof, Vec3::new(0., 8., -3.), 0.)?;
    Ok(())
}

fn tentacle_setup(
    scene: &mut verse_engine::director::Scene,
    _: &crate::playground::Hall,
) -> Result<(), String> {
    caster(scene);
    for i in 0..3 {
        scene.actors.push(creature(
            101 + i,
            "Entering dummy",
            "dummy",
            Vec3::new((i as f32 - 1.) * 1.8, 0., 2.),
            0.,
            400,
        ));
    }
    Ok(())
}
fn tentacle_props(game: &mut crate::play::Game, _: &crate::playground::Hall) -> Result<(), String> {
    game.spawn_prop(
        "Rolling barrel",
        PropSpec::reference(PropKind::Barrel),
        Vec3::new(0., 0.45, 2.),
        0.,
    )?;
    game.spells.dice.force_save(101, 2)?;
    game.spells.dice.force_save(102, 2)?;
    game.spells.dice.force_save(103, 20)?;
    Ok(())
}

fn stone_command(shape: u8, from: [i32; 3], to: [i32; 3], count: u8) -> Step {
    Step::Cast(Ability::SpellCommand(Command::Stone {
        shape,
        from,
        to,
        count,
        thin: false,
    }))
}
fn stone() -> Scenario {
    Scenario {
        key: "wall-of-stone",
        title: "Wall of Stone",
        srd: crate::wall_of_stone::SRD_LINE,
        seed: 453,
        live: 66.,
        speed: 3,
        replay: (47.3, 47.8),
        setup: |scene, _| {
            caster(scene);
            scene.actors[0].position.y = 2.;
            scene.actors.push(creature(
                101,
                "Bridge walker",
                "dummy",
                Vec3::new(-6., 4., -2.),
                0.,
                400,
            ));
            scene.actors.push(creature(
                102,
                "Ramp walker",
                "dummy",
                Vec3::new(7., 0., -8.),
                0.,
                400,
            ));
            scene.actors.push(creature(
                103,
                "Tower rider",
                "dummy",
                Vec3::new(-8., 3.3, -7.),
                0.,
                400,
            ));
            scene.actors.push(creature(
                104,
                "Enclosure escape",
                "dummy",
                Vec3::new(-10., 0., 5.),
                0.,
                400,
            ));
            scene.actors.push(creature(
                105,
                "Wall footprint dummy",
                "dummy",
                Vec3::new(-4., 0., 3.),
                0.,
                400,
            ));
            Ok(())
        },
        populate: |game, _| {
            for (name, dimensions, center) in [
                (
                    "West bridge bank",
                    DVec3::new(3., 4., 3.),
                    Vec3::new(-6., 2., -2.),
                ),
                (
                    "East bridge bank",
                    DVec3::new(3., 4., 3.),
                    Vec3::new(0., 2., -2.),
                ),
                (
                    "Ramp ledge",
                    DVec3::new(12., 6., 8.),
                    Vec3::new(7., 3., 8.5),
                ),
            ] {
                let mut spec = PropSpec::reference(PropKind::StoneBlock).secured();
                spec.dimensions = dimensions;
                game.spawn_prop(name, spec, center, 0.)?;
            }
            for x in [-9., 0.] {
                let mut bank = PropSpec::reference(PropKind::StoneBlock).secured();
                bank.dimensions = DVec3::new(6., 4., 4.);
                game.spawn_prop("Collapse bridge bank", bank, Vec3::new(x, 2., 10.), 0.)?;
            }
            let mut platform = PropSpec::reference(PropKind::StoneBlock).secured();
            platform.dimensions = DVec3::new(2., 2., 2.);
            game.spawn_prop(
                "Caster shooting platform",
                platform,
                Vec3::new(0., 1., -8.),
                0.,
            )?;
            game.spells.dice.force_save(104, 20)?;
            Ok(())
        },
        script: || {
            let mut cues = vec![
                cast(0., stone_command(3, [-8000, 0, -7000], [0; 3], 1)),
                cast(
                    6.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
                cast(
                    10.,
                    stone_command(1, [-4500, 4000, -2000], [-1500, 4000, -2000], 1),
                ),
                cast(
                    11.,
                    Step::Walk {
                        actor: 101,
                        target: [0., 4., -2.],
                        speed: 2.,
                    },
                ),
                cast(
                    16.,
                    Step::ExpectActor {
                        actor: 101,
                        min: [-0.5, 3.9, -2.5],
                        max: [0.5, 4.5, -1.5],
                    },
                ),
                cast(17., Step::Stop(101)),
                cast(
                    20.,
                    stone_command(2, [7000, 0, -6100], [7000, 6000, 4500], 2),
                ),
                cast(
                    21.,
                    Step::Walk {
                        actor: 102,
                        target: [7., 6., 6.],
                        speed: 2.,
                    },
                ),
                cast(
                    28.5,
                    Step::ExpectActor {
                        actor: 102,
                        min: [6.5, 5.5, 3.],
                        max: [7.5, 6.5, 7.],
                    },
                ),
                cast(29., Step::Stop(102)),
                cast(
                    30.,
                    stone_command(1, [-9000, 4000, 10000], [0, 4000, 10000], 1),
                ),
                cast(30.1, Step::SelectPanel(1)),
            ];
            for i in 0..23 {
                cues.push(cast(31. + i as f32 * 0.7, Step::Cast(Ability::FireBolt)));
            }
            cues.push(cast(
                52.,
                stone_command(0, [-5500, 0, 3000], [-2500, 0, 3000], 1),
            ));
            cues.push(cast(58.1, stone_command(4, [-10000, 0, 5000], [0; 3], 1)));
            cues.push(cast(
                64.5,
                Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
            ));
            cues
        },
        camera: stone_camera,
        replay_camera: (Vec3::new(-2., 10., 2.), Vec3::new(-4.5, 4., 10.)),
        check: |game| {
            if game.spells.walls.len() != 6 || !game.spells.walls[3].wall.panels[1].destroyed {
                return Err(format!(
                    "Stone layouts or repeated projectile collapse were incomplete: {:?}",
                    game.spells
                        .walls
                        .iter()
                        .map(|e| e
                            .wall
                            .panels
                            .iter()
                            .map(|p| (p.hit_points, p.destroyed))
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                ));
            }
            if !game.spells.log.iter().any(|r| r.spell == "Falling") {
                return Err("The tower rider did not fall".into());
            }
            Ok(())
        },
    }
}
fn wind() -> Scenario {
    Scenario {
        key: "gust-of-wind",
        title: "Gust of Wind",
        srd: "Level 2 Evocation | 60 by 10 ft Line | STR save | push 15 ft",
        seed: 455,
        live: 30.,
        speed: 2,
        replay: (0.5, 2.),
        setup: |scene, _| {
            caster(scene);
            for i in 0..3 {
                scene.actors.push(creature(
                    101 + i,
                    "Wind save dummy",
                    "dummy",
                    Vec3::new((i as f32 - 1.) * 0.8, 0., 4.),
                    0.,
                    400,
                ));
            }
            Ok(())
        },
        populate: |game, hall| {
            props(game, hall)?;
            for (index, position) in [
                (0, DVec3::new(0.8, 0.3, -3.)),
                (1, DVec3::new(-0.8, 0.45, -1.)),
                (2, DVec3::new(0., 0.2, 1.)),
            ] {
                let body = game.spells.props[index].body;
                game.spells.world[body].pos = position;
            }
            for (name, mass, half, x) in [("Paper", 0.01, 0.15, 0.), ("Basket", 1., 0.3, -0.8)] {
                let mut spec = PropSpec::reference(PropKind::Crate);
                spec.mass = mass;
                spec.dimensions = DVec3::splat(half * 2.);
                game.spawn_prop(name, spec, Vec3::new(x, half as f32, -5.), 0.)?;
            }
            for i in 0..4 {
                game.spells.flames.push(crate::gust::Flame {
                    id: 500 + i,
                    position: DVec3::new(0.5, 1., -6. + f64::from(i)),
                    protected: i >= 2,
                    lit: true,
                });
            }
            game.spells.dice.force_save(101, 2)?;
            game.spells.dice.force_save(102, 2)?;
            game.spells.dice.force_save(103, 20)?;
            Ok(())
        },
        script: || {
            vec![
                cast(0.5, Step::Cast(Ability::Spell(4))),
                cast(
                    2.,
                    Step::Walk {
                        actor: 103,
                        target: [0.8, 0., -8.],
                        speed: 2.,
                    },
                ),
                cast(
                    4.,
                    Step::Flight {
                        origin: [0.8, 1.1, 10.],
                        direction: [0., 0., -1.],
                        siege: false,
                    },
                ),
                cast(
                    7.,
                    Step::Cast(Ability::SpellCommand(Command::Wind([1000, 0, 0]))),
                ),
                cast(
                    20.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
            ]
        },
        camera,
        replay_camera: (Vec3::new(10., 8., -10.), Vec3::new(0., 1., -2.)),
        check: |game| {
            if game
                .spells
                .flames
                .iter()
                .filter(|f| !f.protected)
                .any(|f| f.lit)
            {
                return Err("Unprotected flames remained lit in the wind".into());
            }
            if game.spells.gusts[0].gust.line.direction != DVec3::X {
                return Err("The wind did not turn 90 degrees".into());
            }
            Ok(())
        },
    }
}
fn meteor_setup(
    scene: &mut verse_engine::director::Scene,
    _: &crate::playground::Hall,
) -> Result<(), String> {
    caster(scene);
    for i in 0..6 {
        scene.actors.push(creature(
            101 + i,
            "Overlapping blast dummy",
            "dummy",
            Vec3::new((i % 3) as f32 * 1.8 - 1.8, 0., (i / 3) as f32 * 2. - 3.),
            0.,
            400,
        ));
    }
    Ok(())
}
fn gravity() -> Scenario {
    Scenario {
        key: "reverse-gravity",
        title: "Reverse Gravity",
        srd: crate::reverse_gravity::SRD_LINE,
        seed: 459,
        live: 42.,
        speed: 2,
        replay: (6., 7.),
        setup: |scene, _| {
            caster(scene);
            for (id, x, z) in [
                (101, -5., -3.),
                (102, 0., -3.),
                (103, 5., -3.),
                (104, 8., -3.),
            ] {
                scene.actors.push(creature(
                    id,
                    "Gravity dummy",
                    "dummy",
                    Vec3::new(x, 0., z),
                    0.,
                    400,
                ));
            }
            Ok(())
        },
        populate: |game, hall| {
            props(game, hall)?;
            let mut fixed = PropSpec::reference(PropKind::Crate).secured();
            fixed.dimensions = DVec3::new(0.6, 2., 0.6);
            game.spawn_prop("Fixed grab post", fixed, Vec3::new(8.9, 1., -3.), 0.)?;
            let mut ceiling = PropSpec::reference(PropKind::StoneBlock).secured();
            ceiling.dimensions = DVec3::new(4., 0.4, 4.);
            game.spawn_prop("40-foot ceiling", ceiling, Vec3::new(-5., 12.392, -3.), 0.)?;
            let mut crate_spec = PropSpec::reference(PropKind::Crate);
            crate_spec.hit_points = Some(10);
            game.spawn_prop(
                "Ceiling-strike crate",
                crate_spec,
                Vec3::new(-6.1, 0.3, -3.),
                0.,
            )?;
            for i in 0..3 {
                let mut plank = PropSpec::reference(PropKind::Crate);
                plank.dimensions = DVec3::new(1.2, 0.1, 0.6);
                plank.center_of_mass = DVec3::ZERO;
                plank.mass = 8.;
                game.spawn_prop(
                    "Loose plank pile",
                    plank,
                    Vec3::new(3., 0.05 + i as f32 * 0.11, 0.),
                    0.,
                )?;
            }
            game.spells.dice.force_save(104, 20)?;
            for id in 101..=103 {
                game.spells.dice.force_save(id, 2)?;
            }
            Ok(())
        },
        script: || {
            vec![
                cast(
                    0.5,
                    Step::Cast(Ability::SpellCommand(Command::Point {
                        slot: 8,
                        point: [0, 0, -3000],
                    })),
                ),
                cast(
                    10.,
                    Step::Impulse {
                        prop: 0,
                        impulse: [600., 0., 0.],
                    },
                ),
                cast(
                    16.,
                    Step::Cast(Ability::SpellCommand(Command::EndConcentration)),
                ),
            ]
        },
        camera: || {
            vec![
                Shot {
                    at: 0.,
                    eye: Vec3::new(22., 25., -22.),
                    target: Vec3::new(0., 16., -3.),
                },
                Shot {
                    at: 20.,
                    eye: Vec3::new(20., 18., -20.),
                    target: Vec3::new(0., 8., -3.),
                },
            ]
        },
        replay_camera: (Vec3::new(22., 25., -22.), Vec3::new(0., 16., -3.)),
        check: |game| {
            if !game.spells.reversed[0]
                .spell
                .holds
                .iter()
                .any(|h| matches!(h.grab, crate::reverse_gravity::Grab::Held { .. }))
            {
                return Err("No successful fixed-object grab was demonstrated".into());
            }
            if !game
                .spells
                .log
                .iter()
                .any(|r| r.spell == "Reverse Gravity impact" && r.text.contains("damage"))
            {
                return Err("The ceiling strike did not deal damage".into());
            }
            Ok(())
        },
    }
}

fn meteor_camera() -> Vec<Shot> {
    vec![
        Shot {
            at: 0.,
            eye: Vec3::new(75., 70., -70.),
            target: Vec3::new(0., 45., -3.),
        },
        Shot {
            at: 1.4,
            eye: Vec3::new(25., 18., -25.),
            target: Vec3::new(0., 4., -3.),
        },
    ]
}

fn wind_wall_props(
    game: &mut crate::play::Game,
    hall: &crate::playground::Hall,
) -> Result<(), String> {
    props(game, hall)?;
    for x in [-3., 3.] {
        game.spawn_prop(
            "Wall crossing crate",
            PropSpec::reference(PropKind::Crate),
            Vec3::new(x, 0.3, -6.),
            0.,
        )?;
    }
    for i in 0..3 {
        let mut paper = PropSpec::reference(PropKind::Crate);
        paper.mass = 0.01;
        paper.dimensions = DVec3::new(0.3, 0.05, 0.3);
        paper.size = super::Size::Tiny;
        game.spawn_prop(
            "Updraft paper",
            paper,
            Vec3::new(4. + i as f32 * 0.15, 0.05, -4.),
            0.,
        )?;
    }
    Ok(())
}

fn stone_camera() -> Vec<Shot> {
    vec![
        Shot {
            at: 0.,
            eye: Vec3::new(2., 8., -15.),
            target: Vec3::new(-8., 2., -7.),
        },
        Shot {
            at: 10.,
            eye: Vec3::new(4., 9., -10.),
            target: Vec3::new(-3., 4., -2.),
        },
        Shot {
            at: 20.,
            eye: Vec3::new(19., 12., -10.),
            target: Vec3::new(7., 3., 0.),
        },
        Shot {
            at: 30.,
            eye: Vec3::new(10., 8., 8.),
            target: Vec3::new(-4.5, 4., 10.),
        },
        Shot {
            at: 52.,
            eye: Vec3::new(-3., 8., -10.),
            target: Vec3::new(-4., 1., 3.),
        },
        Shot {
            at: 56.,
            eye: Vec3::new(-4., 8., 0.),
            target: Vec3::new(-10., 2., 5.),
        },
    ]
}

fn levitate_camera() -> Vec<Shot> {
    vec![
        Shot {
            at: 0.,
            eye: Vec3::new(10., 9., -19.),
            target: Vec3::new(-1., 4., -12.),
        },
        Shot {
            at: 26.,
            eye: Vec3::new(10., 9., -19.),
            target: Vec3::new(-1., 4., -12.),
        },
        Shot {
            at: 32.,
            eye: Vec3::new(12., 8., -14.),
            target: Vec3::new(1.5, 2., -4.),
        },
    ]
}
