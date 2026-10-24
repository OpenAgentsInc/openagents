//! Source seeded scenery selection. Art stays in the separately cooked catalog.
//! Source collision excludes a four-source-unit circle around tree roots.
use crate::terrain::{height, noise};
const CELL: f64 = 50. / 6.;
const TREE_RADIUS: f64 = 4. / 6.;
// JavaScript object insertion order in scenery-controller.js: biome bits are
// forest=1, arid=2, desert=4. Source model scales become metres once below.
const CATALOG: [(&str, u8, f64, bool); 36] = [
    ("birch1", 1, 0.075, true),
    ("tree1", 1, 0.1, true),
    ("rock1", 6, 0.025, false),
    ("rockMoss1", 1, 0.025, false),
    ("plant1", 3, 0.05, false),
    ("grass1", 3, 0.05, false),
    ("flowers1", 1, 0.05, false),
    ("birch2", 1, 0.075, true),
    ("birch3", 1, 0.075, true),
    ("birch4", 1, 0.075, true),
    ("birch5", 1, 0.075, true),
    ("birch6", 1, 0.075, true),
    ("birch7", 1, 0.075, true),
    ("birch8", 1, 0.075, true),
    ("birch9", 1, 0.075, true),
    ("tree2", 1, 0.1, true),
    ("tree3", 1, 0.1, true),
    ("tree4", 1, 0.1, true),
    ("tree5", 1, 0.1, true),
    ("tree6", 1, 0.1, true),
    ("tree7", 1, 0.1, true),
    ("tree8", 1, 0.1, true),
    ("tree9", 1, 0.1, true),
    ("rock2", 6, 0.025, false),
    ("rock3", 6, 0.025, false),
    ("rock4", 6, 0.025, false),
    ("rock5", 6, 0.025, false),
    ("rock6", 6, 0.025, false),
    ("rockMoss2", 1, 0.025, false),
    ("rockMoss3", 1, 0.025, false),
    ("rockMoss4", 1, 0.025, false),
    ("rockMoss5", 1, 0.025, false),
    ("rockMoss6", 1, 0.025, false),
    ("plant2", 3, 0.05, false),
    ("plant3", 3, 0.05, false),
    ("plant4", 3, 0.05, false),
];
#[derive(Clone, Debug)]
pub struct Placement {
    pub id: &'static str,
    pub position: [f64; 3],
    pub scale: f64,
    pub yaw: f64,
    pub collidable: bool,
}
fn random(x: f64, y: f64, z: f64) -> f64 {
    noise(x, y, z, 2, 1, 0.5, 2., 1., 1., 1.)
}
fn jitter(gx: i32, gz: i32) -> (f64, f64) {
    let (mut x, mut z) = (gx as f64 * 50., gz as f64 * 50.);
    x += (random(x, 0., z) * 2. - 1.) * 25.;
    z += (random(x, 1., z) * 2. - 1.) * 25.;
    (x, z)
}
fn at(gx: i32, gz: i32, x: f64, z: f64) -> Option<Placement> {
    let world_x = x / 6.;
    let world_z = z / 6.;
    let ground = height(world_x, world_z);
    let moisture = noise(x, 0., z, 2, 2, 0.5, 2., 2., 1024., 1.);
    let (biome, density) = if ground * 6. / 100. < 0.05 {
        (4, 0.1)
    } else if moisture > 0.5 {
        (1, 0.8)
    } else {
        (2, 0.6)
    };
    if random(gx as f64 * 50., 2., gz as f64 * 50.) > density {
        return None;
    }
    let mut matching = [0; 36];
    let mut count = 0;
    for (index, (_, biomes, _, _)) in CATALOG.iter().enumerate() {
        if biomes & biome != 0 {
            matching[count] = index;
            count += 1;
        }
    }
    let selection = (random(x, 3., z) * (count - 1) as f64 + 0.5).floor() as usize;
    let (id, _, scale, collidable) = CATALOG[matching[selection.min(count - 1)]];
    Some(Placement {
        id,
        position: [world_x, ground, world_z],
        scale: scale / 6. * (0.8 + random(x, 4., z) * 0.4),
        yaw: random(x, 5., z) * 360.,
        collidable,
    })
}
pub fn cell(gx: i32, gz: i32) -> Option<Placement> {
    let (x, z) = jitter(gx, gz);
    at(gx, gz, x, z)
}
/// Nine candidate cells cover half-cell jitter plus the source trunk radius.
/// Distant roots are rejected before evaluating height and biome noise.
pub fn tree_blocks(x: f32, z: f32) -> bool {
    let (x, z) = (x as f64, z as f64);
    let gx = (x / CELL).floor() as i32;
    let gz = (z / CELL).floor() as i32;
    for a in gx - 1..=gx + 1 {
        for b in gz - 1..=gz + 1 {
            let (px, pz) = jitter(a, b);
            if (x - px / 6.).hypot(z - pz / 6.) > TREE_RADIUS {
                continue;
            }
            if at(a, b, px, pz).is_some_and(|p| p.collidable) {
                return true;
            }
        }
    }
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_and_world_collision_use_tree_roots_only() {
        assert_eq!(CATALOG.iter().filter(|p| p.3).count(), 18);
        let mut trees = 0;
        for x in -12..=12 {
            for z in -12..=12 {
                if let Some(p) = cell(x, z) {
                    if p.collidable {
                        trees += 1;
                        assert!(tree_blocks(p.position[0] as f32, p.position[2] as f32));
                        assert!(!tree_blocks(
                            (p.position[0] + 1.) as f32,
                            p.position[2] as f32
                        ));
                    }
                }
            }
        }
        assert!(trees > 0);
    }
    #[test]
    fn matches_original_javascript_cell_oracle() {
        let mut placements = 0;
        let mut trees = 0;
        for x in -12..=12 {
            for z in -12..=12 {
                if let Some(p) = cell(x, z) {
                    placements += 1;
                    trees += usize::from(p.collidable);
                }
            }
        }
        assert_eq!((placements, trees), (240, 11));
        for (x, z, id, position, scale, yaw) in [
            (
                9,
                -7,
                "birch6",
                [74.55050754458169, 4.401285723179213, -58.33155036210277],
                0.012690755613445211,
                300.8291142990622,
            ),
            (
                9,
                -6,
                "tree3",
                [75., 5.500917619303238, -50.00000000000019],
                0.01920033196159856,
                179.99999999950026,
            ),
            (
                9,
                -5,
                "birch2",
                [77.71758916323715, 7.322751008673895, -39.93335026756806],
                0.014381633458797715,
                165.57912341229093,
            ),
            (
                -12,
                -4,
                "rock2",
                [-96.38342592592606, 0.8713500126036328, -33.53829077474328],
                0.004558220193686417,
                210.16553717691716,
            ),
            (
                -12,
                -1,
                "rock4",
                [-103.16708161865512, 1.1210301134745402, -8.331696231632668],
                0.0035287705250614428,
                43.41707182881536,
            ),
        ] {
            let p = cell(x, z).unwrap();
            assert_eq!(p.id, id);
            for axis in 0..3 {
                assert!((p.position[axis] - position[axis]).abs() < 1e-9);
            }
            assert!((p.scale - scale).abs() < 1e-12);
            assert!((p.yaw - yaw).abs() < 1e-7);
        }
    }
}
