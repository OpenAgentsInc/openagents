//! Inland patches over the clipmap: a sloping estuary, marsh, and pools.

use crate::{ground, terrain, water};
use glam::{Vec2, Vec3};
use std::sync::Arc;
use verse_pbr::water::{Body, Kind, Preset, Water, WaterPatch, WaterSurface, WaterVertex};

pub fn surface(field: Arc<verse_pbr::water::field::Field>) -> WaterSurface {
    let mut surface = verse_zone_water::coast::surface(field);
    let direction = (terrain::ESTUARY_GATE - terrain::ESTUARY_MOUTH).normalize();
    let side = direction.perp();
    let mut river = WaterPatch {
        cols: 5,
        rows: 129,
        decimate: false,
        dry: 0.0,
        ..WaterPatch::default()
    };
    for row in 0..river.rows {
        let along = row as f32 / (river.rows - 1) as f32;
        let center = terrain::ESTUARY_MOUTH.lerp(terrain::ESTUARY_GATE, along);
        for col in 0..river.cols {
            let p = center + side * ((col as f32 / (river.cols - 1) as f32 * 2.0 - 1.0) * 3.0);
            let height = terrain::river_level(p);
            let mut vertex = WaterVertex::new(
                Vec3::new(p.x, height, p.y),
                height - ground(p.x, p.y),
                Kind::Stream,
            )
            .in_body(1);
            vertex.flow = water::current(p).to_array();
            river.vertices.push(vertex);
        }
    }
    river.bake_shore();
    surface.patches.push(river);
    surface.patches.push(pool(
        terrain::MARSH,
        Vec2::new(20.0, 26.0),
        terrain::river_level(terrain::MARSH),
        2,
    ));
    for (i, center) in terrain::POOLS.into_iter().enumerate() {
        surface
            .patches
            .push(pool(center, Vec2::splat(4.0), terrain::POOL_RIM, i + 3));
    }
    surface
}

fn pool(center: Vec2, radius: Vec2, level: f32, body: usize) -> WaterPatch {
    let mut patch = WaterPatch {
        cols: 25,
        rows: 25,
        decimate: false,
        dry: 0.0,
        ..WaterPatch::default()
    };
    for z in 0..patch.rows {
        for x in 0..patch.cols {
            let uv = Vec2::new(x as f32, z as f32) / 24.0 * 2.0 - Vec2::ONE;
            let p = center + uv * radius;
            let river = terrain::segment(p, terrain::ESTUARY_MOUTH, terrain::ESTUARY_GATE).0;
            let depth = if uv.length() < 1.0 && (body != 2 || river > 3.0) {
                level - ground(p.x, p.y)
            } else {
                -1.0
            };
            patch.vertices.push(
                WaterVertex::new(Vec3::new(p.x, level, p.y), depth, Kind::Stream).in_body(body),
            );
        }
    }
    patch.bake_shore();
    patch
}

pub fn frame(tick: u64) -> Water {
    let mut frame =
        verse_zone_water::coast::frame_water("calm", tick).expect("the calm sea state is bundled");
    let tide = water::tide(tick) as f32;
    frame.sea_body_mut().level = tide;
    frame.shelter = Some(water::HARBOR_SHELTER);
    let preset = Preset::named("ocean").cloned().unwrap_or_default();
    let river = terrain::ESTUARY_GATE - terrain::ESTUARY_MOUTH;
    let gradient = -river / river.length_squared() * tide;
    frame.bodies[1] = Body::still(0.0, &preset);
    frame.bodies[1].level = tide - gradient.dot(terrain::ESTUARY_MOUTH);
    frame.bodies[1].level_gradient = gradient.to_array();
    frame.bodies[2] = Body::still(terrain::river_level(terrain::MARSH), &preset);
    for body in &mut frame.bodies[3..6] {
        *body = Body::still(terrain::POOL_RIM, &preset);
    }
    frame.count = if tide < terrain::POOL_RIM { 6 } else { 3 };
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::water::Water as _;

    #[test]
    fn rendered_inland_levels_match_gameplay_at_both_tides() {
        let physics = water::CoastalWater::new("calm").unwrap();
        let surfaces = surface(Arc::new(water::fields().unwrap().optical));
        surfaces.validate().unwrap();
        for tick in [water::TIDE_PERIOD / 4, water::TIDE_PERIOD * 3 / 4] {
            let frame = frame(tick);
            assert!(frame.valid());
            assert_eq!(
                frame.count,
                if water::tide(tick) < f64::from(terrain::POOL_RIM) {
                    6
                } else {
                    3
                }
            );
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let p = terrain::ESTUARY_MOUTH.lerp(terrain::ESTUARY_GATE, fraction);
                let body = frame.bodies[1];
                let drawn = terrain::river_level(p) + body.level - body.rest
                    + Vec2::from(body.level_gradient).dot(p);
                if let Some(sample) = physics.sample(p.x as f64, p.y as f64, tick) {
                    assert_eq!(sample.body, water::ESTUARY);
                    assert!(
                        (drawn as f64 - sample.height).abs() < 1e-5,
                        "{p}: {drawn} versus {}",
                        sample.height
                    );
                }
            }
            for patch in &surfaces.patches[1..] {
                let vertex = patch.vertices[12 * 25 + 12];
                let body = vertex.body as usize;
                if body >= frame.count {
                    continue;
                }
                let sample = physics
                    .sample(vertex.pos[0] as f64, vertex.pos[2] as f64, tick)
                    .unwrap();
                assert_eq!(sample.body.0 as usize, body);
                assert!((vertex.pos[1] as f64 - sample.height).abs() < 1e-5);
            }
        }
    }
}
