//! Everglade's generated ground, drawn through the textured path
//! (`pbr::textured`) so it is lit and shadowed like the pack's models. The
//! workshop, the glade, and the stations are textured placements from the
//! zone pack (`layout`), and the boards are drawn by `boards`.
//!
//! The ground is two layers over the heightfield:
//!
//! - The grass: an opaque grid over the whole walkable square, with a tiling
//!   detail texture generated here and a per-vertex tint that darkens from
//!   grass into forest floor toward the tree ring.
//! - The dirt: a blended sheet just above the grass over the town: the
//!   yard, the approach path, the roads (`layout::ROADS`), and the ponds'
//!   muddy banks, with cobbles on the paved streets and the Fountain Plaza
//!   (`layout::PAVED`). Its image holds the color and a smooth mask in
//!   alpha, so the edge against the grass is soft at any grid size.
//!
//! Where the ponds' bowls and Glade Run's bed are carved into the
//! heightfield, both layers subdivide each 2 m cell into [`FINE`]² cells,
//! so the banks and beds read as curves. The water itself is not part of
//! this scene: [`super::water`] draws it with the shared water shader.
//!
//! The pinned pack has no tiling ground texture (its `Grass` image is a
//! strip atlas for grass cards, and `PathRocks_Diffuse` is a stone atlas),
//! so both images are generated here from tileable value noise.

use super::{HALF_EXTENT, PATH_HALF_WIDTH, RETURN_PORTAL, YARD, height, land};
use crate::pbr::textured::{
    AlphaMode, BaseColorImage, Primitive, TexturedMaterial, TexturedMesh, TexturedScene,
    TexturedVertex, UNBAKED,
};
use glam::{Mat4, Vec3};

/// Ground grid cell size, m.
const CELL: f32 = 2.0;
/// Grid cells along a side of the walkable square.
const CELLS: i32 = (2.0 * HALF_EXTENT / CELL) as i32;
/// Grid cells along a side of one grass tile; each tile is its own
/// placement, so it is culled on its own.
const TILE_CELLS: i32 = 15;
/// Meters of ground one repeat of the grass texture covers.
const GRASS_REPEAT: f32 = 3.0;
/// Side of the grass detail texture, texels.
const GRASS_EDGE: u32 = 256;
/// Linear albedo of the grass in the clearing and of the forest floor under
/// the tree ring.
const GRASS: [f32; 3] = [0.09, 0.2, 0.04];
const FOREST: [f32; 3] = [0.055, 0.085, 0.032];
/// Mean linear value of the grass detail texture, which the vertex tint
/// divides out so the textured ground keeps the albedo above.
const DETAIL_MEAN: f32 = 0.55;
/// Roughness of both ground layers.
const ROUGHNESS: f32 = 0.95;

/// The dirt sheet's extent, min and max x and z, m. It lies on the grass
/// grid, so its triangles run parallel to the grass under them, and it
/// reaches past the yard, the path, and the roads so its mask fades to
/// nothing before its border.
const DIRT_MIN: [f32; 2] = [-125.0, -135.0];
const DIRT_MAX: [f32; 2] = [121.0, 101.0];
/// The dirt image's size, texels: about 15 cm on the ground.
const DIRT_SIZE: [u32; 2] = [1640, 1573];
/// Side of the square buckets the dirt's roads are indexed in, m.
const ROAD_BUCKET: f32 = 8.0;
/// Subdivisions a side of each 2 m ground cell over the carved water.
const FINE: i32 = 4;
/// Width of a pond's muddy bank beyond its water, m.
const BANK: f32 = 1.2;
/// How far the bank's dirt reaches under the water's edge, m.
const SHALLOWS: f32 = 0.4;
/// Water deeper than this clears the dirt sheet entirely, m.
const DRY_DEPTH: f32 = 0.06;
/// Linear albedo of the silt on the ponds' and the stream's beds: light
/// enough that the water's depth tint shows over it.
const MUD: [f32; 3] = [0.3, 0.25, 0.16];
/// Height of the dirt sheet above the grass, m: below the hall's and the
/// strongroom's floors, which sit 0.02 m up.
pub(super) const DIRT_LIFT: f32 = 0.015;
/// Width of the soft edge between dirt and grass, m.
const FADE: f32 = 0.8;
/// How far the dirt's edge wanders from the yard's and the path's lines, m.
const WANDER: f32 = 0.3;
/// Corner radii of the yard and the path, m.
const YARD_ROUND: f32 = 2.5;
const PATH_ROUND: f32 = 1.2;
/// Linear albedo of the trodden path and of the yard.
const PATH: [f32; 3] = [0.3, 0.24, 0.14];
/// Where a trail's wear on the grass is whole and where it fades out, m
/// from the trail's middle, and how much of the grass it covers.
const TRAIL_WORN: (f32, f32) = (0.6, 2.4);
const TRAIL_COVER: f32 = 0.85;
const YARD_DIRT: [f32; 3] = [0.24, 0.2, 0.13];
/// Linear albedo of the cobbles, lightest and darkest, and of the joints
/// between them.
const COBBLE_LIGHT: [f32; 3] = [0.36, 0.31, 0.25];
const COBBLE_DARK: [f32; 3] = [0.21, 0.18, 0.15];
const JOINT: [f32; 3] = [0.09, 0.085, 0.075];
/// A cobble's length and width, m; rows run east to west, each offset by
/// half a cobble.
const COBBLE: [f32; 2] = [0.75, 0.5];

/// Sunlight from above and slightly behind the approach, so slopes facing
/// the arriving player read lighter.
pub fn shade(color: [f32; 3], a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let light = Vec3::new(0.35, 0.85, -0.4).normalize();
    let k = 0.62 + 0.45 * normal.dot(light).abs();
    color.map(|v| v * k)
}

/// Triangles the ground adds to the zone: the grass grid and the dirt
/// sheet, each carved cell subdivided.
#[cfg(test)]
#[must_use]
pub(super) fn triangles() -> u64 {
    let carved = carved_cells();
    let fine = (FINE * FINE) as u64;
    let grass = CELLS as u64 * CELLS as u64 + carved.len() as u64 * (fine - 1);
    let in_dirt = |(i, j): &(i32, i32)| {
        let [x, z] = grid_point(*i, *j);
        x >= DIRT_MIN[0] && x < DIRT_MAX[0] && z >= DIRT_MIN[1] && z < DIRT_MAX[1]
    };
    let dirt = ((DIRT_MAX[0] - DIRT_MIN[0]) / CELL) as u64
        * ((DIRT_MAX[1] - DIRT_MIN[1]) / CELL) as u64
        + carved.iter().filter(|c| in_dirt(c)).count() as u64 * (fine - 1);
    2 * (grass + dirt)
}

/// The 2 m ground cells, as grid indices, that a pond's bowl, the stream's
/// bed, or their banks reach into: they draw subdivided.
#[must_use]
pub fn carved_cells() -> &'static std::collections::BTreeSet<(i32, i32)> {
    static CARVED: std::sync::OnceLock<std::collections::BTreeSet<(i32, i32)>> =
        std::sync::OnceLock::new();
    CARVED.get_or_init(|| {
        let carved = verse_world::social::everglade_water::carved;
        let mut out = std::collections::BTreeSet::new();
        for i in 0..CELLS {
            for j in 0..CELLS {
                let [x, z] = grid_point(i, j);
                // A cell with any carved sample, its corners included.
                let any = (0..=FINE).any(|a| {
                    (0..=FINE).any(|b| {
                        carved(
                            x + CELL * a as f32 / FINE as f32,
                            z + CELL * b as f32 / FINE as f32,
                        )
                    })
                });
                if any {
                    out.insert((i, j));
                }
            }
        }
        out
    })
}

/// Adds the ground to `scene`: its two images and materials, a grass mesh
/// per tile, and the dirt sheet, each placed at its own corner, over the
/// carved heightfield.
pub(super) fn ground(scene: &mut TexturedScene) {
    ground_with(scene, true);
}

/// Adds the grass to `scene`, and the dirt sheet over the town when `dirt`
/// is set. With `dirt` the ground is Everglade's own, with its ponds and
/// stream carved; without it, as for the Grove, it is the uncarved land.
pub fn ground_with(scene: &mut TexturedScene, dirt: bool) {
    let ground: fn(f32, f32) -> f32 = if dirt { height } else { land };
    let empty = std::collections::BTreeSet::new();
    let carved = if dirt { carved_cells() } else { &empty };
    let grass_image = scene.add_image(grass_image());
    let grass = scene.add_material(TexturedMaterial {
        image: Some(grass_image),
        roughness: ROUGHNESS,
        ..TexturedMaterial::default()
    });
    let grass_attributes = |p: Vec3| {
        let tint = grass_tint(p.x, p.z);
        (
            [p.x / GRASS_REPEAT, p.z / GRASS_REPEAT],
            if dirt { mud(tint, p) } else { tint },
        )
    };
    let tiles = CELLS / TILE_CELLS;
    for ti in 0..tiles {
        for tj in 0..tiles {
            let start = [ti * TILE_CELLS, tj * TILE_CELLS];
            let corner = grid_point(start[0], start[1]);
            let shape = Shape {
                ground,
                carved,
                lift: 0.0,
            };
            let mesh = grid(
                start,
                [TILE_CELLS; 2],
                corner,
                &shape,
                grass,
                grass_attributes,
            );
            let mesh = scene.add_mesh(mesh);
            scene.place(
                mesh,
                Mat4::from_translation(Vec3::new(corner[0], 0.0, corner[1])),
            );
            // The tile's carved cells, subdivided.
            let cells: Vec<(i32, i32)> = carved
                .iter()
                .copied()
                .filter(|&(i, j)| {
                    (start[0]..start[0] + TILE_CELLS).contains(&i)
                        && (start[1]..start[1] + TILE_CELLS).contains(&j)
                })
                .collect();
            if !cells.is_empty() {
                let mesh = fine(&cells, corner, &shape, grass, grass_attributes);
                let mesh = scene.add_mesh(mesh);
                scene.place(
                    mesh,
                    Mat4::from_translation(Vec3::new(corner[0], 0.0, corner[1])),
                );
            }
        }
    }
    if !dirt {
        return;
    }
    let dirt_image = scene.add_image(dirt_image());
    let dirt = scene.add_material(TexturedMaterial {
        image: Some(dirt_image),
        roughness: ROUGHNESS,
        alpha: AlphaMode::Blend,
        ..TexturedMaterial::default()
    });
    let start = [
        ((DIRT_MIN[0] + HALF_EXTENT) / CELL) as i32,
        ((DIRT_MIN[1] + HALF_EXTENT) / CELL) as i32,
    ];
    let cells = [
        ((DIRT_MAX[0] - DIRT_MIN[0]) / CELL) as i32,
        ((DIRT_MAX[1] - DIRT_MIN[1]) / CELL) as i32,
    ];
    let shape = Shape {
        ground,
        carved,
        lift: DIRT_LIFT,
    };
    let dirt_attributes = |p: Vec3| ([dirt_u(p.x), dirt_v(p.z)], [255; 4]);
    let mesh = grid(start, cells, DIRT_MIN, &shape, dirt, dirt_attributes);
    let mesh = scene.add_mesh(mesh);
    scene.place(
        mesh,
        Mat4::from_translation(Vec3::new(DIRT_MIN[0], 0.0, DIRT_MIN[1])),
    );
    let inside: Vec<(i32, i32)> = carved
        .iter()
        .copied()
        .filter(|&(i, j)| {
            (start[0]..start[0] + cells[0]).contains(&i)
                && (start[1]..start[1] + cells[1]).contains(&j)
        })
        .collect();
    if !inside.is_empty() {
        let mesh = fine(&inside, DIRT_MIN, &shape, dirt, dirt_attributes);
        let mesh = scene.add_mesh(mesh);
        scene.place(
            mesh,
            Mat4::from_translation(Vec3::new(DIRT_MIN[0], 0.0, DIRT_MIN[1])),
        );
    }
}

/// What a ground layer stands on: the heightfield, the cells drawn
/// subdivided instead, and the layer's height over the ground, m.
struct Shape<'a> {
    ground: fn(f32, f32) -> f32,
    carved: &'a std::collections::BTreeSet<(i32, i32)>,
    lift: f32,
}

/// The world x and z of grid point `(i, j)`.
fn grid_point(i: i32, j: i32) -> [f32; 2] {
    [
        -HALF_EXTENT + i as f32 * CELL,
        -HALF_EXTENT + j as f32 * CELL,
    ]
}

/// The unit normal of `ground` at `(x, z)`, from its slope.
fn normal(ground: fn(f32, f32) -> f32, x: f32, z: f32) -> Vec3 {
    let e = 0.25;
    let dx = (ground(x + e, z) - ground(x - e, z)) / (2.0 * e);
    let dz = (ground(x, z + e) - ground(x, z - e)) / (2.0 * e);
    Vec3::new(-dx, 1.0, -dz).normalize()
}

/// A heightfield mesh over `cells` grid cells from grid point `start`, in
/// coordinates relative to `corner` and `shape.lift` meters above
/// `shape.ground`, leaving out the cells `shape.carved` draws subdivided.
/// `attributes` gives each vertex's image coordinate and color from its
/// world position. Front faces point up.
fn grid(
    start: [i32; 2],
    cells: [i32; 2],
    corner: [f32; 2],
    shape: &Shape,
    material: usize,
    attributes: impl Fn(Vec3) -> ([f32; 2], [u8; 4]),
) -> TexturedMesh {
    let columns = cells[0] + 1;
    let mut vertices = Vec::with_capacity((columns * (cells[1] + 1)) as usize);
    for j in 0..=cells[1] {
        for i in 0..=cells[0] {
            let [x, z] = grid_point(start[0] + i, start[1] + j);
            vertices.push(ground_vertex(shape, x, z, corner, &attributes));
        }
    }
    let mut indices = Vec::with_capacity((cells[0] * cells[1] * 6) as usize);
    for j in 0..cells[1] {
        for i in 0..cells[0] {
            if shape.carved.contains(&(start[0] + i, start[1] + j)) {
                continue;
            }
            let at = |i: i32, j: i32| (j * columns + i) as u32;
            let [a, b, c, d] = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            // Seen from above, with x east and z north, a-c-b and a-d-c turn
            // counterclockwise.
            indices.extend_from_slice(&[a, c, b, a, d, c]);
        }
    }
    TexturedMesh {
        primitives: vec![Primitive {
            vertices,
            indices,
            material,
        }],
    }
}

/// The grid `cells`, each split into [`FINE`]² cells, relative to `corner`.
fn fine(
    cells: &[(i32, i32)],
    corner: [f32; 2],
    shape: &Shape,
    material: usize,
    attributes: impl Fn(Vec3) -> ([f32; 2], [u8; 4]),
) -> TexturedMesh {
    let side = (FINE + 1) as u32;
    let mut vertices = Vec::with_capacity(cells.len() * (side * side) as usize);
    let mut indices = Vec::with_capacity(cells.len() * (FINE * FINE * 6) as usize);
    let step = CELL / FINE as f32;
    for &(ci, cj) in cells {
        let [x0, z0] = grid_point(ci, cj);
        let base = vertices.len() as u32;
        for j in 0..=FINE {
            for i in 0..=FINE {
                let (x, z) = (x0 + i as f32 * step, z0 + j as f32 * step);
                vertices.push(ground_vertex(shape, x, z, corner, &attributes));
            }
        }
        for j in 0..FINE as u32 {
            for i in 0..FINE as u32 {
                let at = |i: u32, j: u32| base + j * side + i;
                let [a, b, c, d] = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                indices.extend_from_slice(&[a, c, b, a, d, c]);
            }
        }
    }
    TexturedMesh {
        primitives: vec![Primitive {
            vertices,
            indices,
            material,
        }],
    }
}

/// One ground vertex at world `(x, z)`, relative to `corner`.
fn ground_vertex(
    shape: &Shape,
    x: f32,
    z: f32,
    corner: [f32; 2],
    attributes: &impl Fn(Vec3) -> ([f32; 2], [u8; 4]),
) -> TexturedVertex {
    let world = Vec3::new(x, (shape.ground)(x, z), z);
    let (uv, color) = attributes(world);
    TexturedVertex {
        pos: [x - corner[0], world.y + shape.lift, z - corner[1]],
        normal: normal(shape.ground, x, z).to_array(),
        uv,
        color,
        light: UNBAKED,
    }
}

/// The grass's per-vertex tint at `(x, z)`: grass in the clearing,
/// darkening into forest floor toward the tree ring.
fn grass_tint(x: f32, z: f32) -> [u8; 4] {
    let t = ((x.hypot(z) - super::CLEARING_RADIUS) / (super::RING_RADIUS - super::CLEARING_RADIUS))
        .clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    // Broad patches of lusher and drier grass, so the lawns do not read
    // as one flat green.
    let lush = value_noise(x * 0.045 + 7.0, z * 0.045 - 3.0, 1 << 12, 71) - 0.5;
    let dry = (value_noise(x * 0.11, z * 0.11, 1 << 12, 73) - 0.55).max(0.0);
    let k = 1.0 + 0.35 * lush;
    // The trails to the zone's edges are worn into the turf, so they
    // follow the rising ground (`layout::trails`).
    let d = super::layout::trails::distance(x, z);
    let worn = 1.0 - ((d - TRAIL_WORN.0) / (TRAIL_WORN.1 - TRAIL_WORN.0)).clamp(0.0, 1.0);
    let worn = worn * worn * (3.0 - 2.0 * worn) * TRAIL_COVER;
    let rgb: [u8; 3] = std::array::from_fn(|i| {
        let albedo = (GRASS[i] + (FOREST[i] - GRASS[i]) * t) * k;
        let straw = [0.2, 0.17, 0.05][i];
        let albedo = albedo + (straw - albedo) * dry * 0.9 * (1.0 - t);
        let albedo = albedo + (PATH[i] * 0.75 - albedo) * worn;
        unorm(albedo / DETAIL_MEAN)
    });
    [rgb[0], rgb[1], rgb[2], 255]
}

/// `tint` turned to mud where the ground at `p` lies under the water.
fn mud(tint: [u8; 4], p: Vec3) -> [u8; 4] {
    let Some(top) = verse_world::social::everglade_water::rest_surface(p.x, p.z) else {
        return tint;
    };
    let under = smoothstep((top - p.y) / 0.3);
    let rgb: [u8; 3] = std::array::from_fn(|i| {
        let grass = f32::from(tint[i]) / 255.0;
        unorm(grass + (MUD[i] / DETAIL_MEAN - grass) * under)
    });
    [rgb[0], rgb[1], rgb[2], 255]
}

fn dirt_u(x: f32) -> f32 {
    (x - DIRT_MIN[0]) / (DIRT_MAX[0] - DIRT_MIN[0])
}

fn dirt_v(z: f32) -> f32 {
    (z - DIRT_MIN[1]) / (DIRT_MAX[1] - DIRT_MIN[1])
}

/// How much of the dirt sheet may show at `(x, z)`, 0 to 1: none where
/// water stands over the carved ground. The physical renderer draws its
/// water before blended textured surfaces, so dirt over a pond or the run
/// would hide the water under a flat sheet.
pub(super) fn dry(x: f32, z: f32) -> f32 {
    verse_world::social::everglade_water::rest_surface(x, z).map_or(1.0, |top| {
        1.0 - smoothstep((top - height(x, z)) / DRY_DEPTH)
    })
}

/// A value in 0 to 1 as an 8-bit unorm.
fn unorm(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A linear value as an 8-bit sRGB code.
fn srgb(linear: f32) -> u8 {
    let l = linear.clamp(0.0, 1.0);
    let encoded = if l <= 0.003_130_8 {
        12.92 * l
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    unorm(encoded)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A hash of a lattice point to 0 to 1.
fn hash(i: i32, j: i32, seed: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x8da6_b343)
        ^ (j as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    f32::from((h & 0xffff) as u16) / 65_535.0
}

/// Smooth value noise in 0 to 1 at `(x, y)` lattice units, repeating every
/// `period` units in both directions.
fn value_noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (sx, sy) = (smoothstep(x - x0), smoothstep(y - y0));
    let (i, j) = (x0 as i32, y0 as i32);
    let at = |a: i32, b: i32| hash(a.rem_euclid(period), b.rem_euclid(period), seed);
    let near = at(i, j) + (at(i + 1, j) - at(i, j)) * sx;
    let far = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * sx;
    near + (far - near) * sy
}

/// Octaves of value noise over one repeat, `u` and `v` in 0 to 1, starting
/// at `base` lattice cells per repeat; tileable, in 0 to 1.
fn fractal(u: f32, v: f32, base: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut weight, mut total) = (0.0, 1.0, 0.0);
    for octave in 0..octaves {
        let period = base << octave;
        sum += weight * value_noise(u * period as f32, v * period as f32, period, seed + octave);
        total += weight;
        weight *= 0.55;
    }
    sum / total
}

/// The grass detail texture: tiling light and dark patches, fine
/// blade-scale speckle, and a slight yellow-green drift, near neutral so the
/// vertex tint carries the grass's color.
pub(super) fn grass_image() -> BaseColorImage {
    let edge = GRASS_EDGE;
    let mut rgba = Vec::with_capacity((edge * edge * 4) as usize);
    for y in 0..edge {
        for x in 0..edge {
            let (u, v) = (x as f32 / edge as f32, y as f32 / edge as f32);
            let patches = fractal(u, v, 4, 3, 11);
            let speckle = value_noise(u * 128.0, v * 128.0, 128, 29);
            let drift = fractal(u, v, 2, 2, 41) - 0.5;
            let l = 0.35 + 0.25 * patches + 0.15 * speckle;
            rgba.extend_from_slice(&[
                srgb(l * (1.0 + 0.3 * drift)),
                srgb(l),
                srgb(l * (0.9 - 0.2 * drift)),
                255,
            ]);
        }
    }
    BaseColorImage {
        name: "everglade/ground/grass".into(),
        width: edge,
        height: edge,
        rgba,
    }
}

/// Signed distance from `(x, z)` to a rectangle with rounded corners:
/// negative inside, m.
fn rounded_rect(center: [f32; 2], half: [f32; 2], radius: f32, x: f32, z: f32) -> f32 {
    let qx = (x - center[0]).abs() - half[0] + radius;
    let qz = (z - center[1]).abs() - half[1] + radius;
    qx.max(0.0).hypot(qz.max(0.0)) + qx.max(qz).min(0.0) - radius
}

/// How much of the yard's and the paths' dirt covers `(x, z)`, 0 to 1 each.
/// The paths are the approach, the roads, and the ponds' banks. Each fades
/// smoothly over [`FADE`] meters centered on its edge, and the edge wanders
/// a little so it does not read as a ruled line.
#[must_use]
pub(super) fn dirt_cover(x: f32, z: f32) -> (f32, f32) {
    let wander = WANDER * (2.0 * value_noise(x * 0.7, z * 0.7, 1 << 12, 53) - 1.0);
    let cover = |distance: f32| smoothstep(0.5 - (distance + wander) / FADE);
    let yard = rounded_rect(YARD.0, YARD.1, YARD_ROUND, x, z);
    // The path runs from behind the return portal into the yard.
    let start = RETURN_PORTAL.z - 2.0;
    let end = YARD.0[1] - YARD.1[1] + 1.0;
    let path = rounded_rect(
        [0.0, (start + end) / 2.0],
        [PATH_HALF_WIDTH, (end - start) / 2.0],
        PATH_ROUND,
        x,
        z,
    );
    let road = near_roads(x, z)
        .iter()
        .map(|&i| {
            let (a, b, half) = super::layout::roads()[i as usize];
            super::layout::segment_distance(a, b, x, z) - half
        })
        .fold(f32::INFINITY, f32::min);
    // The banks are a ring from just under the water's edge to the land.
    // The water pass draws before this blended sheet, so [`dirt_image`]
    // clears it wherever water stands ([`dry`]); the grass layer muddies
    // the beds instead ([`mud`]).
    let ring = |d: f32, edge: f32| (edge - SHALLOWS - d).max(d - edge - BANK);
    let run = verse_world::social::everglade_water::run();
    let stream = if run.near(x, z) {
        let s = run.locate(x, z);
        ring(s.across, run.half_at(s.along))
    } else {
        f32::INFINITY
    };
    let bank = super::layout::PONDS
        .iter()
        .map(|&([cx, cz], r)| ring((cx - x).hypot(cz - z), r))
        .fold(f32::INFINITY, f32::min)
        .min(stream);
    (cover(yard), cover(path.min(road).min(bank)))
}

/// How much of the cobbled paving covers `(x, z)`, 0 to 1: the paved
/// streets and squares (`layout::PAVED`, `layout::PAVED_SQUARES`), fading
/// over [`FADE`] meters at their edges.
#[must_use]
pub(super) fn paved_cover(x: f32, z: f32) -> f32 {
    let wander = 0.5 * WANDER * (2.0 * value_noise(x * 0.9, z * 0.9, 1 << 12, 59) - 1.0);
    let street = super::layout::PAVED
        .iter()
        .map(|&(a, b, half)| super::layout::segment_distance(a, b, x, z) - half)
        .fold(f32::INFINITY, f32::min);
    let square = super::layout::PAVED_SQUARES
        .iter()
        .map(|&(c, h)| rounded_rect(c, h, 1.5, x, z))
        .fold(f32::INFINITY, f32::min);
    smoothstep(0.5 - (street.min(square) + wander) / (0.6 * FADE))
}

/// The color of the cobbles at `(x, z)`: stones in running rows, each its
/// own shade, with dark joints between them.
fn cobble(x: f32, z: f32) -> [f32; 3] {
    let row = (z / COBBLE[1]).floor();
    let along = x / COBBLE[0] + 0.5 * row.rem_euclid(2.0);
    let column = along.floor();
    let (u, v) = (along - column, z / COBBLE[1] - row);
    let joint = u.min(1.0 - u) * COBBLE[0] < 0.06 || v.min(1.0 - v) * COBBLE[1] < 0.05;
    if joint {
        return JOINT;
    }
    let shade = hash(column as i32, row as i32, 83);
    let grain = value_noise(x * 6.0, z * 6.0, 1 << 14, 89);
    std::array::from_fn(|i| {
        (COBBLE_DARK[i] + (COBBLE_LIGHT[i] - COBBLE_DARK[i]) * shade) * (0.85 + 0.3 * grain)
    })
}

/// The roads whose dirt may reach `(x, z)`: those within a bucket of the
/// sheet's [`ROAD_BUCKET`] grid, indexed once. Beyond a road's half width
/// plus the fade and the wander its cover is nothing, so the others are
/// left out.
fn near_roads(x: f32, z: f32) -> &'static [u16] {
    static INDEX: std::sync::OnceLock<(usize, Vec<Vec<u16>>)> = std::sync::OnceLock::new();
    let (side, lists) = INDEX.get_or_init(|| {
        let side = ((HALF_EXTENT * 2.0) / ROAD_BUCKET).ceil() as usize + 1;
        let mut lists = vec![Vec::new(); side * side];
        let reach = FADE + WANDER + ROAD_BUCKET;
        for (i, &(a, b, half)) in super::layout::roads().iter().enumerate() {
            let cell = |v: f32| {
                (((v + HALF_EXTENT) / ROAD_BUCKET).floor().max(0.0) as usize).min(side - 1)
            };
            let r = half + reach;
            for j in cell(a[1].min(b[1]) - r)..=cell(a[1].max(b[1]) + r) {
                for k in cell(a[0].min(b[0]) - r)..=cell(a[0].max(b[0]) + r) {
                    lists[j * side + k].push(i as u16);
                }
            }
        }
        (side, lists)
    });
    let cell = |v: f32| {
        if v.is_finite() {
            (((v + HALF_EXTENT) / ROAD_BUCKET).floor().max(0.0) as usize).min(side - 1)
        } else {
            0
        }
    };
    &lists[cell(z) * side + cell(x)]
}

/// The dirt sheet's image: the yard's and the path's dirt with gravel
/// speckle, and their cover in alpha. Every texel carries the dirt's color,
/// so filtering and mipmaps never pull grass-colored texels into the edge.
pub(super) fn dirt_image() -> BaseColorImage {
    let [width, height] = DIRT_SIZE;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        for column in 0..width {
            let u = (column as f32 + 0.5) / width as f32;
            let v = (row as f32 + 0.5) / height as f32;
            let x = DIRT_MIN[0] + u * (DIRT_MAX[0] - DIRT_MIN[0]);
            let z = DIRT_MIN[1] + v * (DIRT_MAX[1] - DIRT_MIN[1]);
            let (yard, path) = dirt_cover(x, z);
            let paved = paved_cover(x, z);
            let cover = yard.max(path).max(paved) * dry(x, z);
            let trodden = if yard + path > 0.0 {
                path / (yard + path)
            } else {
                1.0
            };
            let patches = value_noise(x * 0.5, z * 0.5, 1 << 12, 61);
            let gravel = value_noise(x * 9.0, z * 9.0, 1 << 14, 67);
            let k = 0.75 + 0.3 * patches + 0.25 * (gravel - 0.5);
            let stone = if paved > 0.0 { cobble(x, z) } else { JOINT };
            let rgb: [u8; 3] = std::array::from_fn(|i| {
                let dirt = (YARD_DIRT[i] + (PATH[i] - YARD_DIRT[i]) * trodden) * k;
                srgb(dirt + (stone[i] - dirt) * paved)
            });
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], unorm(cover)]);
        }
    }
    BaseColorImage {
        name: "everglade/ground/dirt".into(),
        width,
        height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> TexturedScene {
        let mut scene = TexturedScene::default();
        ground(&mut scene);
        scene
    }

    /// Every triangle of `scene` in world space, with its material.
    fn world_triangles(scene: &TexturedScene) -> Vec<([Vec3; 3], [Vec3; 3], usize)> {
        let mut out = Vec::new();
        for placement in &scene.placements {
            for p in &scene.meshes[placement.mesh].primitives {
                for t in p.indices.chunks_exact(3) {
                    let v = [t[0], t[1], t[2]].map(|i| p.vertices[i as usize]);
                    let pos = v.map(|v| placement.transform.transform_point3(v.pos.into()));
                    let normal = v.map(|v| Vec3::from(v.normal));
                    out.push((pos, normal, p.material));
                }
            }
        }
        out
    }

    #[test]
    fn the_ground_covers_the_square_on_the_heightfield_facing_up() {
        let scene = scene();
        scene.validate().unwrap();
        let all = world_triangles(&scene);
        assert_eq!(all.len() as u64, triangles());
        let grass = scene
            .materials
            .iter()
            .position(|m| m.alpha == AlphaMode::Opaque)
            .unwrap();
        // No water in the scene: the water pass draws it over the carved
        // beds (`super::super::water`).
        assert_eq!(scene.materials.len(), 2);
        let mut extent = 0.0_f32;
        let mut deepest = 0.0_f32;
        for (pos, normals, material) in &all {
            let lift = if *material == grass { 0.0 } else { DIRT_LIFT };
            for p in pos {
                assert!((p.y - lift - height(p.x, p.z)).abs() < 1e-4, "{p}");
                extent = extent.max(p.x.abs()).max(p.z.abs());
                deepest = deepest.min(p.y - lift);
            }
            let face = (pos[1] - pos[0]).cross(pos[2] - pos[0]);
            assert!(face.y > 0.0, "a ground triangle faces down at {}", pos[0]);
            // The beds' and banks' slopes are steeper than the land's.
            let wet = verse_world::social::everglade_water::carved(pos[0].x, pos[0].z);
            for n in normals {
                assert!(n.y > if wet { 0.3 } else { 0.7 }, "{} at {}", n, pos[0]);
                assert!((n.length() - 1.0).abs() < 1e-4);
            }
        }
        // Lantern Pond's bed is drawn to its full depth.
        let lantern = verse_world::social::everglade_water::pond_level(0)
            - verse_world::social::everglade_water::POND_DEPTHS[0];
        assert!(deepest < lantern + 0.1, "{deepest}");
        assert!((extent - HALF_EXTENT).abs() < 1e-3);
        // Each grass tile lands in a merge cell of its own.
        let merged = scene.merge().unwrap();
        let tiles = (CELLS / TILE_CELLS).pow(2) as usize;
        assert_eq!(
            merged
                .batches
                .iter()
                .filter(|b| b.material == grass)
                .count(),
            tiles
        );
    }

    #[test]
    fn the_dirt_edge_is_soft_and_inside_its_sheet() {
        // Solid dirt on the path and in the yard, grass beside them.
        assert!(dirt_cover(0.0, -25.0).1 > 0.99);
        assert!(dirt_cover(0.0, -6.5).0 > 0.99);
        assert_eq!(dirt_cover(6.0, -25.0), (0.0, 0.0));
        assert_eq!(dirt_cover(0.0, 20.0), (0.0, 0.0));
        // The roads are dirt too.
        assert!(dirt_cover(-11.0, 30.0).1 > 0.99);
        assert!(dirt_cover(20.0, 46.0).1 > 0.99);
        // Crossing the path's edge passes through partial cover over more
        // than one sample 5 cm apart, never jumping.
        let mut partial = 0;
        let mut last = dirt_cover(0.0, -25.0).1;
        for k in 1..=80 {
            let (_, cover) = dirt_cover(k as f32 * 0.05, -25.0);
            partial += usize::from((0.05..0.95).contains(&cover));
            assert!(
                (cover - last).abs() < 0.2,
                "the edge jumps at {}",
                k as f32 * 0.05
            );
            last = cover;
        }
        assert!(partial >= 4, "{partial}");
        // The mask is clear along the sheet's border, so the repeating
        // sampler never wraps dirt across it.
        let image = dirt_image();
        let (w, h) = (image.width as usize, image.height as usize);
        for i in 0..w {
            for j in [0, h - 1] {
                assert_eq!(image.rgba[(j * w + i) * 4 + 3], 0);
            }
        }
        for j in 0..h {
            for i in [0, w - 1] {
                assert_eq!(image.rgba[(j * w + i) * 4 + 3], 0);
            }
        }
        // No dirt lies over standing water: the water draws before this
        // blended sheet, which would hide it.
        let alpha = |x: f32, z: f32| {
            let i = ((x - DIRT_MIN[0]) / (DIRT_MAX[0] - DIRT_MIN[0]) * w as f32) as usize;
            let j = ((z - DIRT_MIN[1]) / (DIRT_MAX[1] - DIRT_MIN[1]) * h as f32) as usize;
            image.rgba[(j * w + i) * 4 + 3]
        };
        for &([cx, cz], r) in &super::super::layout::PONDS {
            for k in 0..16 {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                for s in [0.0, 0.5, 0.8] {
                    let (x, z) = (cx + a.cos() * r * s, cz + a.sin() * r * s);
                    assert_eq!(alpha(x, z), 0, "dirt over the pond at ({x}, {z})");
                }
            }
        }
        let run = verse_world::social::everglade_water::run();
        for k in 0..40 {
            let [x, z] = run.point_at(run.length() * k as f32 / 40.0);
            if (DIRT_MIN[0]..DIRT_MAX[0]).contains(&x) && (DIRT_MIN[1]..DIRT_MAX[1]).contains(&z) {
                assert_eq!(alpha(x, z), 0, "dirt over Glade Run at ({x}, {z})");
            }
        }
    }

    #[test]
    fn the_grass_texture_tiles_and_stays_near_its_mean() {
        for (x, y) in [(0.3, 0.7), (5.5, 2.25), (31.9, 0.1)] {
            let a = value_noise(x, y, 32, 7);
            assert!((a - value_noise(x + 32.0, y - 64.0, 32, 7)).abs() < 1e-5);
        }
        let image = grass_image();
        assert_eq!(image.rgba.len(), (GRASS_EDGE * GRASS_EDGE * 4) as usize);
        let decode = |c: u8| {
            let s = f32::from(c) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        let green: f32 = image
            .rgba
            .chunks_exact(4)
            .map(|t| decode(t[1]))
            .sum::<f32>()
            / (GRASS_EDGE * GRASS_EDGE) as f32;
        assert!((green - DETAIL_MEAN).abs() < 0.08, "{green}");
    }
}
