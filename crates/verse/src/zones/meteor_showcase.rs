//! The Meteor Showcase: two large houses from the medieval kit on an open
//! lot at golden hour, for an eight-meteor swarm to bring down
//! (`verse --meteor-showcase`, issue #10926).
//!
//! The houses are kit houses ([`everglade::layout::kit_house`]), built and
//! broken as Stoop Lane's are: every kit piece is one block of the town's
//! demolition, so the swarm breaks them piece by piece and what stood on a
//! broken wall falls through the support graph. The lot is grass with a
//! dirt path to both doors and a few trees, on the flat, uncarved land of
//! Everglade's clearing, so debris rests on the ground it is drawn on.
//!
//! The player casts Meteor Swarm with `1` and calls down
//! [`Volley::SHOWCASE`]: eight meteors fanned across the sky on arcs of
//! their own. Nothing casts on its own; `R` rebuilds the houses. The film
//! (`meteor_showcase_capture`) stages a caster west of the houses who sends
//! one meteor at each of [`targets`] ([`stage`]). The town clock is pinned
//! to golden hour, [`HOUR`], and the light is baked for it when the zone
//! loads.

use super::{Everglade, everglade, everglade_pack::ZonePack};
use crate::{controller::PlayerController, world::World};
use everglade::demolition::meteor::Volley;
use everglade::demolition::town::Debris;
use everglade::layout::kit_house::{KitHouse, KitStyle};
use everglade::layout::{Collision, Placement};
use everglade::studio::{Posture, SeatFigure};
use glam::Vec3;
use std::sync::Arc;

/// The middle of the lot, between the two houses, x and z, m: on the flat
/// land west of Lantern Pond, clear of the ponds and Glade Run.
pub const LOT: [f32; 2] = [-56.0, -5.0];
/// Where the player stands when the zone opens: south of the lot, behind
/// where the establishing view looks from.
pub const SPAWN: Vec3 = Vec3::new(-50.0, 0.0, -42.0);
/// The way back to the plaza, behind the spawn.
pub const RETURN_PORTAL: Vec3 = Vec3::new(-46.0, 0.0, -94.0);
/// Where the caster stands, west of the houses, so the meteors come in
/// from the low Sun's side and cross the view.
pub const CASTER: [f32; 2] = [-25.0, -13.0];
/// The town clock's pinned hour: the Sun low in the west-south-west.
pub const HOUR: f64 = 17.35;
/// How long after the houses stand whole the caster casts, s.
pub const DELAY: f32 = 0.5;
/// How long after the houses stand whole they rebuild, s.
pub const REBUILD: f32 = 45.0;
/// The stage's bloom, so fire glows into what surrounds it.
pub const BLOOM: f32 = 0.09;
/// Where the haze starts and where it closes, m: clearer than the town's,
/// so the houses stand sharp from across the lot.
pub const FOG: (f32, f32) = (90.0, 360.0);
/// The houses' debris: at most 700 chunks, each lasting for minutes, with
/// kit pieces broken as the town breaks them. A finer break and more
/// chunks cost 90 to 200 ms of rigid bodies a frame
/// (`docs/verse/meteor-showcase-handoff.md`).
pub const DEBRIS: Debris = Debris {
    chunks: 700,
    lifetime: 600.0,
    shards: 2,
};

/// Which way the houses' fronts face, as the controller's yaw: toward the
/// south-south-west, so the low Sun lights their fronts and west sides.
pub const FACING: f32 = 2.8;

/// A point `along` the houses' front line from the lot's middle and `out`
/// in front of it, x and z, m.
#[must_use]
pub fn on_lot(along: f32, out: f32) -> [f32; 2] {
    let (s, c) = FACING.sin_cos();
    [LOT[0] + along * c + out * s, LOT[1] - along * s + out * c]
}

/// The two houses, side by side facing the path: a three-story timber
/// house of four bays and one of three.
#[must_use]
pub fn houses() -> [KitHouse; 2] {
    [
        KitHouse {
            name: "showcase large house",
            center: on_lot(-10.0, 0.0),
            width: 16.0,
            depth: 10.0,
            facing: FACING,
            stories: 3,
            style: KitStyle::Timber,
            door_bay: 1,
            door_at: None,
            seed: 13,
        },
        KitHouse {
            name: "showcase small house",
            center: on_lot(10.0, -2.0),
            width: 12.0,
            depth: 10.0,
            facing: FACING,
            stories: 3,
            style: KitStyle::Timber,
            door_bay: 0,
            door_at: None,
            seed: 6,
        },
    ]
}

/// The dirt path: up from the south-west to a fork before the houses, and
/// on to each front door.
#[must_use]
pub fn paths() -> Vec<Vec<[f32; 2]>> {
    let [big, small] = houses();
    let fork = on_lot(0.5, 13.0);
    let door = |house: &KitHouse| house.door_points().0;
    vec![
        vec![on_lot(-3.0, 70.0), on_lot(1.5, 38.0), fork],
        vec![fork, door(&big)],
        vec![fork, door(&small)],
    ]
}

/// Every placement: the houses' kit pieces, and the trees, shrubs, and
/// grass around the lot, clear of the houses and the path.
#[must_use]
pub fn placements() -> Vec<Placement> {
    let mut out = Vec::new();
    for house in houses() {
        house.raise(&mut out);
    }
    // A few trees close by, framing the houses, and a loose line of them
    // well behind.
    let trees: [(&'static str, [f32; 2], f32); 11] = [
        ("foliage/oak_old", [LOT[0] - 30.0, LOT[1] + 6.0], 1.0),
        ("foliage/linden_broad", [LOT[0] + 26.0, LOT[1] + 9.0], 1.0),
        ("foliage/beech_tall", [LOT[0] - 24.0, LOT[1] - 14.0], 1.1),
        ("foliage/oak_forked", [LOT[0] + 32.0, LOT[1] + 6.0], 1.0),
        ("foliage/beech_tall", [LOT[0] - 6.0, LOT[1] + 22.0], 1.2),
        ("foliage/oak_old", [LOT[0] + 10.0, LOT[1] + 30.0], 1.1),
        ("foliage/linden_broad", [LOT[0] - 22.0, LOT[1] + 32.0], 1.0),
        ("foliage/oak_forked", [LOT[0] - 40.0, LOT[1] + 26.0], 1.2),
        ("foliage/beech_tall", [LOT[0] + 36.0, LOT[1] + 28.0], 1.1),
        ("foliage/linden_broad", [LOT[0] + 4.0, LOT[1] + 44.0], 1.2),
        ("foliage/oak_old", [LOT[0] - 50.0, LOT[1] - 6.0], 1.0),
    ];
    for (i, (model, at, scale)) in trees.into_iter().enumerate() {
        out.push(Placement::new(model, at, i as f32 * 1.7, Collision::Core(0.4)).scale(scale));
    }
    let shrubs: [(&'static str, [f32; 2]); 6] = [
        ("foliage/shrub_mound", [LOT[0] - 21.0, LOT[1] - 3.0]),
        ("foliage/shrub_flowering", [LOT[0] + 20.0, LOT[1] + 2.0]),
        ("foliage/shrub_tall", [LOT[0] - 15.0, LOT[1] + 12.0]),
        ("foliage/shrub_mound", [LOT[0] + 17.0, LOT[1] + 14.0]),
        ("foliage/shrub_flowering", [LOT[0] - 9.0, LOT[1] - 24.0]),
        ("foliage/shrub_tall", [LOT[0] + 13.0, LOT[1] - 26.0]),
    ];
    for (i, (model, at)) in shrubs.into_iter().enumerate() {
        out.push(Placement::new(model, at, i as f32 * 2.3, Collision::None));
    }
    // Tufts of tall grass and wildflowers over the lawns, off the path.
    let paths = paths();
    let on_path = |x: f32, z: f32| {
        paths
            .iter()
            .flat_map(|p| p.windows(2))
            .any(|w| everglade::layout::segment_distance(w[0], w[1], x, z) < 2.5)
    };
    let mut planted = 0;
    let mut n = 0_u32;
    while planted < 70 && n < 600 {
        n += 1;
        let x = LOT[0] + (everglade::layout::noise(n, 501) - 0.5) * 84.0;
        let z = LOT[1] + (everglade::layout::noise(n, 502) - 0.5) * 84.0;
        let near_house = houses()
            .iter()
            .any(|h| (x - h.center[0]).hypot(z - h.center[1]) < h.width.hypot(h.depth) / 2.0 + 4.0);
        if near_house || on_path(x, z) {
            continue;
        }
        let model = if everglade::layout::noise(n, 503) < 0.7 {
            "foliage/grass_tall"
        } else {
            "foliage/wildflower_clump"
        };
        out.push(Placement::new(
            model,
            [x, z],
            everglade::layout::noise(n, 504) * 6.3,
            Collision::None,
        ));
        planted += 1;
    }
    out
}

/// Where the caster's eight meteors land, one each in turn, alternating
/// between the houses: the roofs, the upper stories' fronts and sides, and
/// the ground floors' corners, so the upper floors lose their footing and
/// fall as the roofs burst.
#[must_use]
pub fn targets() -> Vec<Vec3> {
    let [big, small] = houses();
    let at = |house: &KitHouse, u: f32, w: f32, up: f32| {
        let [x, z] = house.world([u, w]);
        Vec3::new(x, house.floor() + up, z)
    };
    let (bd, sd) = (big.depth / 2.0, small.depth / 2.0);
    vec![
        at(&big, -3.0, 0.0, 14.5),
        at(&small, 1.0, 0.0, 14.5),
        at(&big, 4.5, bd, 9.5),
        at(&small, -4.0, sd, 5.0),
        at(&big, -6.5, bd, 1.5),
        at(&small, 4.5, -sd, 9.5),
        at(&big, 7.5, -2.0, 5.0),
        at(&small, -5.5, -1.0, 1.5),
    ]
}

/// The circle the caster's targeting ring lies on: the ground between the
/// houses.
#[must_use]
pub fn aim() -> Vec3 {
    Vec3::new(LOT[0], everglade::land(LOT[0], LOT[1]), LOT[1] - 2.0)
}

/// The caster, gesturing at the houses as the cast gathers.
#[must_use]
pub fn figures() -> Vec<SeatFigure> {
    let [x, z] = CASTER;
    let aim = aim();
    vec![SeatFigure {
        name: "Meteor caster".into(),
        pos: Vec3::new(x, everglade::land(x, z), z),
        yaw: (aim.x - x).atan2(aim.z - z),
        speed: 0.0,
        posture: Posture::Talk,
        look: Some(aim + Vec3::Y * 14.0),
        tint: [0.75, 0.2, 0.08],
        form: None,
    }]
}

/// The zone's air at golden hour: Everglade's haze in the low Sun's
/// horizon color.
#[must_use]
pub fn atmosphere() -> super::Atmosphere {
    Everglade::air_at(HOUR as f32)
}

/// Builds the lot, the houses' demolition, the caster, and the golden-hour
/// stage, and starts the light's bake.
///
/// # Errors
/// Returns a message if a required model or destruction scene cannot load.
pub fn build(pack: &ZonePack, player: &PlayerController) -> Result<(World, Everglade), String> {
    let placements = placements();
    let (mut scene, blockers) = everglade::scene::build(pack, &placements)?;
    everglade::draw::ground_with_paths(&mut scene, &paths());
    scene.validate()?;
    let scene = Arc::new(scene);
    let mut world = World::default();
    world.mesh.textured = Some(scene.clone());
    world.blockers = blockers;
    let solids = everglade::solids::build_with(pack, &placements, &[])?;
    let mut glade = Everglade::with_solids(pack, player, solids)?;
    glade.set_free_casting();
    glade.set_clock(town_clock::Clock::DAYTIME.pinned(Some(HOUR)));
    glade.set_bloom(BLOOM);
    glade.set_fog(FOG.0, FOG.1);
    // What the swarm breaks is relit, so no baked shade floats where the
    // houses stood (#10938).
    glade.relight_destruction();
    glade.bake_light(scene.clone());
    glade.start_wreckage(pack, &placements, scene)?;
    let town = glade
        .town_mut()
        .ok_or("The showcase has no destructible houses")?;
    town.set_volley(Volley::SHOWCASE);
    town.set_debris(DEBRIS);
    town.set_numbers(false);
    Ok((world, glade))
}

/// Stands the caster west of the houses and has them cast the showcase's
/// volley at [`targets`] `delay` seconds from now, and again after each
/// rebuild: the film's staged cast. The zone never casts on its own; the
/// player casts with Meteor Swarm.
pub fn stage(glade: &mut Everglade, delay: f32) -> Result<(), String> {
    let town = glade
        .town_mut()
        .ok_or("The showcase has no destructible houses")?;
    let [x, z] = CASTER;
    town.start_showcase(
        Vec3::new(x, everglade::land(x, z), z),
        aim(),
        targets(),
        Volley::SHOWCASE,
        delay,
        REBUILD,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use everglade::layout::kit_house::STORY;
    use verse_world::social::everglade_water;

    #[test]
    fn the_lot_is_flat_uncarved_land_clear_of_the_water() {
        for i in -45..=45 {
            for j in -45..=45 {
                let (x, z) = (LOT[0] + i as f32, LOT[1] + j as f32);
                assert!(!everglade_water::carved(x, z), "carved at {x}, {z}");
                assert_eq!(everglade::height(x, z), 0.0, "uneven at {x}, {z}");
                assert_eq!(everglade::land(x, z), 0.0, "uneven at {x}, {z}");
            }
        }
    }

    #[test]
    fn two_big_timber_kit_houses_with_chimneys_stand_apart() {
        let [big, small] = houses();
        for house in [big, small] {
            assert!(house.stories >= 2 && house.stories <= 3);
            assert_eq!(house.style, KitStyle::Timber);
            assert!(house.eaves() - house.floor() >= 2.0 * STORY);
            let mut pieces = Vec::new();
            house.raise(&mut pieces);
            let count = |m: &str| pieces.iter().filter(|p| p.model == m).count();
            assert!(count("kit/chimney") >= 1, "{} has no chimney", house.name);
            assert!(count("kit/roof-end") >= 2, "{} has no gables", house.name);
            assert!(count("kit/wall-4-timber") + count("kit/wall-2-timber") > 0);
            assert!(
                pieces
                    .iter()
                    .all(|p| everglade_pack_kit_piece(p.model) && p.collision == Collision::None)
            );
        }
        // A street's width between them.
        let gap = (big.center[0] - small.center[0]).hypot(big.center[1] - small.center[1])
            - (big.width + small.width) / 2.0;
        assert!(gap > 4.0, "the houses are {gap} m apart");
    }

    fn everglade_pack_kit_piece(model: &str) -> bool {
        super::super::everglade_pack::kit::piece_of(model).is_some()
    }

    #[test]
    fn the_targets_lie_on_both_houses_and_the_caster_is_in_range() {
        let targets = targets();
        assert_eq!(targets.len(), Volley::SHOWCASE.count);
        for house in houses() {
            let on = targets
                .iter()
                .filter(|t| {
                    // In the house's own frame.
                    let (s, c) = house.facing.sin_cos();
                    let (dx, dz) = (t.x - house.center[0], t.z - house.center[1]);
                    let (u, w) = (dx * c - dz * s, dx * s + dz * c);
                    u.abs() <= house.width / 2.0 + 0.5 && w.abs() <= house.depth / 2.0 + 0.5
                })
                .count();
            assert_eq!(on, 4, "{} takes four meteors", house.name);
        }
        let [x, z] = CASTER;
        let aim = aim();
        assert!((aim.x - x).hypot(aim.z - z) < everglade::demolition::meteor::RANGE);
    }

    #[test]
    fn the_path_reaches_both_doors_and_nothing_grows_on_it() {
        let paths = paths();
        for house in houses() {
            let door = house.door_points().0;
            assert!(paths.iter().any(|p| p.last() == Some(&door)));
        }
        for p in placements()
            .iter()
            .filter(|p| p.model.starts_with("foliage/"))
        {
            let d = paths
                .iter()
                .flat_map(|path| path.windows(2))
                .map(|w| everglade::layout::segment_distance(w[0], w[1], p.at[0], p.at[1]))
                .fold(f32::INFINITY, f32::min);
            assert!(d > 2.0, "{} grows on the path", p.model);
        }
    }
}
