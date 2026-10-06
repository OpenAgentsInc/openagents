use super::layout::{Collision, DESKS};
use super::*;
use crate::{
    pbr::textured::TexturedScene,
    zones::everglade_pack::{self, MERGED_TRIANGLE_BUDGET, PLACED_TRIANGLE_BUDGET},
};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The committed, pinned pack.
fn pack_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ))
}

/// The pinned pack, decoded once for every test.
pub(super) fn pack() -> &'static ZonePack {
    static PACK: OnceLock<ZonePack> = OnceLock::new();
    PACK.get_or_init(|| ZonePack::load_local(&pack_path()).expect("the committed pack loads"))
}

/// The zone's static world, built once for every test.
pub(super) fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| Everglade::world(pack()).expect("the layout builds from the pack"))
}

#[test]
fn the_ground_is_flat_in_the_clearing_and_rises_gently_within_bounds() {
    let step = 0.5;
    let n = (2.0 * HALF_EXTENT / step) as i32;
    for i in 0..=n {
        for j in 0..=n {
            let (x, z) = (
                -HALF_EXTENT + i as f32 * step,
                -HALF_EXTENT + j as f32 * step,
            );
            let h = height(x, z);
            assert!(
                h.is_finite() && (0.0..=MAX_HEIGHT).contains(&h),
                "{x},{z}: {h}"
            );
            if x.hypot(z) <= CLEARING_RADIUS && !verse_world::social::everglade::on_hill(x, z) {
                assert_eq!(h, 0.0, "the clearing is flat at {x},{z}");
            }
            // Gentle: no step steeper than 0.6 m per meter.
            let dx = (height(x + step, z) - h).abs();
            let dz = (height(x, z + step) - h).abs();
            assert!(dx.max(dz) <= 0.6 * step, "{x},{z} is too steep");
        }
    }
    // The ground has risen by the tree ring in every direction.
    for k in 0..36 {
        let angle = k as f32 / 36.0 * std::f32::consts::TAU;
        let (x, z) = (angle.cos() * RING_RADIUS, angle.sin() * RING_RADIUS);
        assert!(height(x, z) >= RING_RISE - UNDULATION - 1e-4);
    }
    for (x, z) in [(f32::NAN, 0.0), (0.0, f32::INFINITY)] {
        assert_eq!(height(x, z), 0.0);
    }
}

#[test]
fn the_world_is_ground_textured_placements_and_boards() {
    let world = world();
    assert!(world.mesh.faces.len() % 3 == 0 && world.mesh.lines.len() % 2 == 0);
    for v in world.mesh.faces.iter().chain(&world.mesh.lines) {
        assert!(v.pos.iter().chain(&v.color).all(|x| x.is_finite()));
        assert!(v.color.iter().all(|c| (0.0..=1.0).contains(c)));
    }
    // The boards are the only vertex-color faces; the ground is textured
    // (`draw`'s tests check it lies on the height function).
    assert!(!world.mesh.faces.is_empty());
    let mut ground = TexturedScene::default();
    super::draw::ground(&mut ground);
    assert!(!ground.placements.is_empty());
    // Every placement, the far level of each that has one, and the ground
    // are in the scene, which validates and merges into cells, with
    // base-color images within the pack's texture budget.
    let scene = world.mesh.textured.as_ref().expect("a textured scene");
    let placements = layout::placements();
    let fars = super::detail::far_placements(pack(), &placements);
    let far_count = fars.iter().flatten().count();
    assert!(far_count > 600, "{far_count}");
    assert_eq!(
        scene.placements.len(),
        placements.len() + far_count + ground.placements.len()
    );
    // A far level stands where its placement does, which draws near.
    for (i, far) in fars.iter().enumerate() {
        if let Some((_, at)) = far {
            let (near, far) = (scene.placements[i], scene.placements[*at]);
            assert_eq!(near.transform, far.transform);
            assert_eq!(near.detail.switch(), far.detail.switch());
            assert!(matches!(near.detail, crate::pbr::textured::Detail::Near(_)));
            assert!(matches!(far.detail, crate::pbr::textured::Detail::Far(_)));
        }
    }
    scene.validate().unwrap();
    let merged = scene.merge().unwrap();
    assert!(!merged.batches.is_empty());
    let images: u64 = scene.images.iter().map(|i| i.rgba.len() as u64).sum();
    assert!(images <= everglade_pack::Limits::EVERGLADE.decoded_texture_bytes);
    // The greybox markers are gone: no station posts stand in the world.
    assert!(world.mesh.lines.is_empty());
}

#[test]
fn every_placement_names_an_admitted_model_and_stays_in_the_glade() {
    let placements = layout::placements();
    assert!(placements.len() > 100);
    for placement in &placements {
        let model = pack()
            .model(placement.model)
            .unwrap_or_else(|| panic!("{} is not in the pack", placement.model));
        let set = placement.model.split('/').next().unwrap();
        assert!(
            ["nature", "village", "props", "generated", "foliage"].contains(&set),
            "{}",
            placement.model
        );
        assert!(placement.scale > 0.0 && placement.scale.is_finite());
        let transform = placement.transform();
        assert!(transform.is_finite(), "{}", placement.model);
        let (min, max) = model.bounds();
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { min[0] } else { max[0] },
                if i & 2 == 0 { min[1] } else { max[1] },
                if i & 4 == 0 { min[2] } else { max[2] },
            );
            let p = transform.transform_point3(corner);
            assert!(
                p.x.abs() < HALF_EXTENT - 1.0 && p.z.abs() < HALF_EXTENT - 1.0,
                "{} reaches {p}",
                placement.model
            );
        }
    }
    // The strongroom keeps the metal crate; the rigged chest is not admitted.
    assert!(placements.iter().any(|p| p.model == "props/Crate_Metal"));
    assert!(pack().model("props/Chest_Wood").is_none());
}

/// Triangles of the merged cells that draw from `eye` at their levels of
/// detail, in every direction and at every distance.
fn levelled(merged: &crate::pbr::textured::Merged, eye: Vec3) -> u64 {
    merged
        .batches
        .iter()
        .filter(|b| b.level.drawn_from(eye))
        .map(|b| u64::from(b.count / 3))
        .sum()
}

#[test]
fn the_layout_stays_within_the_placed_triangle_budget() {
    let scene = world().mesh.textured.as_ref().unwrap();
    let merged = scene.merge().unwrap();
    // Every level is merged and uploaded: the geometry bound.
    let all = merged.indices.len() as u64 / 3;
    let near: u64 = merged
        .batches
        .iter()
        .filter(|b| !matches!(b.level, crate::pbr::textured::Level::Far { .. }))
        .map(|b| u64::from(b.count / 3))
        .sum();
    eprintln!(
        "Everglade merges {all} triangles of {MERGED_TRIANGLE_BUDGET}, {near} at their near levels"
    );
    assert!(all <= MERGED_TRIANGLE_BUDGET, "{all}");
    // What a frame can reach: from anywhere in the clearing, each cell at
    // the level it draws at from there.
    let mut most = (0, Vec3::ZERO);
    for x in (-136..=136).step_by(8) {
        for z in (-136..=136).step_by(8) {
            let eye = Vec3::new(x as f32, 2.0, z as f32);
            let triangles = levelled(&merged, eye);
            if triangles > most.0 {
                most = (triangles, eye);
            }
        }
    }
    eprintln!(
        "Everglade places at most {} triangles of {PLACED_TRIANGLE_BUDGET} at their levels, from {}",
        most.0, most.1
    );
    assert!(most.0 <= PLACED_TRIANGLE_BUDGET, "{}", most.0);
    // The player is drawn, not placed; it has its own budget in the pack.
    let player = pack().character.as_ref().expect("the player's character");
    assert!(player.triangles() <= everglade_pack::Limits::EVERGLADE.character_triangles);
}

#[test]
fn prop_bounds_become_navigation_blockers() {
    let placements = layout::placements();
    let expected: Vec<_> = placements
        .iter()
        .flat_map(|p| p.footprints(pack().model(p.model).unwrap().bounds()))
        .chain(layout::board_blockers())
        .chain(layout::pond_blockers())
        .chain(layout::city::blocks().into_iter().map(|(f, _)| f))
        .chain(super::npcs::blocks().into_iter().map(|(f, _)| f))
        .collect();
    assert_eq!(world().blockers, expected);
    for placement in &placements {
        let footprints = placement.footprints(pack().model(placement.model).unwrap().bounds());
        match placement.collision {
            Collision::None => assert!(footprints.is_empty()),
            Collision::Bounds | Collision::Core(_) => {
                assert_eq!(footprints.len(), 1, "{}", placement.model);
                let [x, z] = placement.at;
                let f = footprints[0];
                assert!(f.max[0] > f.min[0] && f.max[1] > f.min[1]);
                if matches!(placement.collision, Collision::Core(_)) {
                    assert!(f.contains(x, z, 0.0), "{} core", placement.model);
                }
            }
            // A doorway or arch leaves two jambs with the opening between.
            Collision::Opening(_) => {
                assert_eq!(footprints.len(), 2, "{}", placement.model);
                let [x, z] = placement.at;
                assert!(footprints.iter().all(|f| !f.contains(x, z, 0.0)));
            }
        }
    }
    // The hall's walls, the strongroom's fence, and the workbenches all
    // block.
    let blocks = |x: f32, z: f32| world().blockers.iter().any(|b| b.contains(x, z, 0.0));
    let ([cx, cz], [hx, hz]) = HALL;
    assert!(blocks(cx - hx, cz) && blocks(cx + hx, cz) && blocks(-5.0, cz + hz));
    assert!(blocks(14.0, 7.0));
    for desk in DESKS {
        assert!(blocks(desk.seat[0], desk.monitor.center.z));
    }
}

#[test]
fn a_hosted_instance_walks_the_pinned_packs_content_under_the_social_rules() {
    use verse_world::social::world::World;
    use verse_world::{Command, Controller, Intent};

    // One content digest: the loader's pin is the instance's identity.
    let mut profile = Everglade::social_profile(pack()).unwrap();
    let digest = everglade_pack::content_digest().unwrap();
    assert_eq!(profile.content, digest);
    let spelled: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(spelled, everglade_pack::PACK_SHA256);
    assert_eq!(everglade_pack::pinned().sha256, spelled);

    // A session joins without an adventurer or a hostile, and the hall's
    // south wall stops it where it stops a local player.
    profile.spawn = Vec3::new(-5.0, 0.0, -1.0);
    let mut world = World::new(profile, 1);
    let who = Controller(9);
    let life = world.join(who).unwrap();
    for _ in 0..120 {
        let admission = world.admission(life).unwrap().clone();
        let command: Command<()> = admission
            .command(
                world.tick(),
                Intent::Move {
                    axes: [0.0, 1.0],
                    yaw: 0.0,
                },
            )
            .unwrap();
        world.command(who, &command).unwrap();
        world.step();
    }
    let at = world.avatar(life).unwrap().pos;
    assert!(at.z < HALL.0[1] - HALL.1[1], "{at}");
    assert!(at.z > 0.0, "{at}");
}

#[test]
fn the_towns_creatures_live_by_the_water_the_trees_and_the_hives() {
    use super::player::Motion;
    use super::wildlife::{CULL, Creature, Route, Wildlife, creatures};
    let placements = layout::placements();
    let all = creatures(pack(), &placements);
    for form in [
        "beasts/songbird",
        "beasts/duck",
        "beasts/frog",
        "beasts/cat",
        "beasts/rat",
        "beasts/snake",
        "beasts/wasp",
    ] {
        assert!(pack().form(form).is_some(), "{form}");
        assert!(all.iter().any(|c| c.form == form), "{form}");
    }
    // Every route keeps its creature where it belongs over a minute: ducks
    // on the water, birds over the roofs while circling, the rest on the
    // ground or a little above it.
    for Creature { form, route, .. } in &all {
        for step in 0..120 {
            let m = route.at(step as f32 * 0.5);
            let ground = height(m.at.x, m.at.z);
            assert!(m.at.is_finite() && m.yaw.is_finite(), "{form}");
            match (*form, route) {
                ("beasts/duck", _) => {
                    let wet = layout::PONDS
                        .iter()
                        .any(|(c, r)| (m.at.x - c[0]).hypot(m.at.z - c[1]) < r - 0.5);
                    assert!(wet, "a duck leaves the water at {}", m.at);
                }
                ("beasts/songbird", Route::Circle { .. }) => {
                    assert!(m.at.y > ground + 10.0, "{form} at {}", m.at);
                }
                _ => assert!(
                    m.at.y >= ground - 0.05,
                    "{form} under the ground at {}",
                    m.at
                ),
            }
        }
    }
    // A pacing rat walks, then rests.
    let rat = all.iter().find(|c| c.form == "beasts/rat").unwrap();
    let motions: Vec<Motion> = (0..40).map(|i| rat.route.at(i as f32).motion).collect();
    assert!(motions.contains(&Motion::Walk) && motions.contains(&Motion::Idle));
    // Near Lantern Pond the ducks, the frog, and the circling birds pose;
    // far ones fold away.
    let mut wildlife = Wildlife::new(pack(), all.clone()).unwrap();
    assert_eq!(wildlife.creatures().len(), all.len());
    let [px, pz] = layout::PONDS[0].0;
    let eye = Vec3::new(px, height(px, pz) + 2.0, pz - 10.0);
    wildlife.tick(0.1, eye);
    let near = all
        .iter()
        .filter(|c| c.route.at(0.1 + c.phase).at.distance(eye) <= CULL)
        .count();
    assert!(near >= 4 && near < all.len(), "{near}");
    assert_eq!(wildlife.drawn(), near);
    // They join the characters' figure, which still validates.
    let zone = Everglade::new(
        pack(),
        &crate::controller::PlayerController::new(Vec3::ZERO, 0.0),
    )
    .unwrap();
    let cast = zone.cast_figure().unwrap();
    wildlife.prepare(&cast.scene);
    let figure = wildlife.figure(cast.clone(), None);
    figure.validate().unwrap();
    assert!(figure.vertices.len() > cast.vertices.len());
}
