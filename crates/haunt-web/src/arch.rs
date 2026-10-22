//! The house's architecture as triangles: floors, walls with their doorways
//! cut, lintels, baseboards, ceilings, and, for the yard, the ground, the
//! cobbled path, the road, the house front with its roof and lit windows,
//! and the far mountains.
//!
//! A vertex carries a shader pattern (cobbles, wood, wallpaper stripes,
//! halftone, brick, tiles, grass) and two palette colours, so the house
//! needs no texture files: 15 floats, position, normal, the pattern's
//! surface coordinates in metres, colour A, colour B, and the pattern id.
//! Positions are in the house's frame: +x east, +z north, y up.

use glam::Vec3;
use haunt_assets::{Catalog, Pattern, colour};
use haunt_rules::doc::{Area, AreaKind, House, Wall};
use toon_mesh::rgb;

/// Floats per vertex.
pub const STRIDE: usize = 15;
/// How tall a doorway is, m.
pub const DOOR_HEIGHT: f32 = 2.3;
/// How tall the house front is to the eaves, m.
pub const FRONT_HEIGHT: f32 = 8.0;
/// A pattern id for glowing glass (windows), drawn unlit.
pub const GLOW: f32 = 100.0;

/// A surface's look: a pattern and two colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub pattern: f32,
    pub a: [f32; 3],
    pub b: [f32; 3],
}

impl Style {
    /// A library surface by id; plain plum when it isn't there.
    #[must_use]
    pub fn of(catalog: &Catalog, id: &str) -> Self {
        catalog.surface(id).map_or_else(
            || Self::plain("plum"),
            |s| Self {
                pattern: s.pattern.index() as f32,
                a: token(&s.colors[0]),
                b: token(&s.colors[1]),
            },
        )
    }

    /// One flat palette colour.
    #[must_use]
    pub fn plain(name: &str) -> Self {
        let c = token(name);
        Self {
            pattern: Pattern::Plain.index() as f32,
            a: c,
            b: c,
        }
    }

    /// Glowing glass in a palette colour.
    #[must_use]
    pub fn glow(name: &str) -> Self {
        let c = token(name);
        Self {
            pattern: GLOW,
            a: c,
            b: c,
        }
    }

    /// A pattern in two palette colours.
    #[must_use]
    pub fn pattern(pattern: Pattern, a: &str, b: &str) -> Self {
        Self {
            pattern: pattern.index() as f32,
            a: token(a),
            b: token(b),
        }
    }
}

/// A palette token's colour as floats.
#[must_use]
pub fn token(name: &str) -> [f32; 3] {
    rgb(colour(name).unwrap_or(0x4A3F5E))
}

/// A triangle list in the architecture layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArchMesh {
    pub data: Vec<f32>,
}

impl ArchMesh {
    #[must_use]
    pub fn vertices(&self) -> usize {
        self.data.len() / STRIDE
    }

    fn vertex(&mut self, p: Vec3, n: Vec3, uv: [f32; 2], s: Style) {
        self.data.extend_from_slice(&[
            p.x, p.y, p.z, n.x, n.y, n.z, uv[0], uv[1], s.a[0], s.a[1], s.a[2], s.b[0], s.b[1],
            s.b[2], s.pattern,
        ]);
    }

    /// A quad from four corners in order around its face, with the surface
    /// coordinates of each corner. Its normal is `n`; the winding follows.
    pub fn quad(&mut self, corners: [Vec3; 4], uvs: [[f32; 2]; 4], n: Vec3, s: Style) {
        let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        let order = if face.dot(n) >= 0.0 {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        for i in order {
            self.vertex(corners[i], n, uvs[i], s);
        }
    }

    /// A triangle with normal `n`.
    pub fn tri(&mut self, corners: [Vec3; 3], uvs: [[f32; 2]; 3], n: Vec3, s: Style) {
        let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        let order = if face.dot(n) >= 0.0 {
            [0, 1, 2]
        } else {
            [0, 2, 1]
        };
        for i in order {
            self.vertex(corners[i], n, uvs[i], s);
        }
    }

    /// A horizontal rectangle at height `y`, facing up (or down).
    pub fn flat(&mut self, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, up: bool, s: Style) {
        let n = if up { Vec3::Y } else { Vec3::NEG_Y };
        self.quad(
            [
                Vec3::new(x0, y, z0),
                Vec3::new(x1, y, z0),
                Vec3::new(x1, y, z1),
                Vec3::new(x0, y, z1),
            ],
            [[x0, z0], [x1, z0], [x1, z1], [x0, z1]],
            n,
            s,
        );
    }

    /// A vertical rectangle along a wall line: `fixed` is its x (for a
    /// north-south wall) or z, from `a` to `b` along it, `y0` to `y1` high,
    /// facing `n`.
    #[allow(clippy::too_many_arguments)]
    pub fn upright(
        &mut self,
        along_x: bool,
        fixed: f32,
        a: f32,
        b: f32,
        y0: f32,
        y1: f32,
        n: Vec3,
        s: Style,
    ) {
        let at = |t: f32, y: f32| {
            if along_x {
                Vec3::new(t, y, fixed)
            } else {
                Vec3::new(fixed, y, t)
            }
        };
        self.quad(
            [at(a, y0), at(b, y0), at(b, y1), at(a, y1)],
            [[a, y0], [b, y0], [b, y1], [a, y1]],
            n,
            s,
        );
    }

    /// A box's visible sides (not its bottom), for steps and chimneys.
    pub fn solid(&mut self, min: Vec3, max: Vec3, s: Style) {
        self.flat(min.x, min.z, max.x, max.z, max.y, true, s);
        self.upright(true, min.z, min.x, max.x, min.y, max.y, Vec3::NEG_Z, s);
        self.upright(true, max.z, min.x, max.x, min.y, max.y, Vec3::Z, s);
        self.upright(false, min.x, min.z, max.z, min.y, max.y, Vec3::NEG_X, s);
        self.upright(false, max.x, min.z, max.z, min.y, max.y, Vec3::X, s);
    }

    pub fn extend(&mut self, other: &ArchMesh) {
        self.data.extend_from_slice(&other.data);
    }
}

/// The inward normal of an area's wall.
fn inward(wall: Wall) -> Vec3 {
    match wall {
        Wall::North => Vec3::NEG_Z,
        Wall::South => Vec3::Z,
        Wall::East => Vec3::NEG_X,
        Wall::West => Vec3::X,
    }
}

/// An indoor area: floor, ceiling, walls with doorways, lintels over them,
/// and a baseboard.
fn indoor(area: &Area, catalog: &Catalog) -> ArchMesh {
    let mut m = ArchMesh::default();
    let floor = Style::of(catalog, &area.surfaces.floor);
    let walls = Style::of(catalog, &area.surfaces.walls);
    let trim = area
        .surfaces
        .trim
        .as_deref()
        .map_or(Style::plain("licorice"), |t| Style::of(catalog, t));
    let ceiling_height = area.ceiling.unwrap_or(3.0);
    m.flat(area.x0(), area.z0(), area.x1(), area.z1(), 0.0, true, floor);
    let ceiling = Style {
        pattern: Pattern::Halftone.index() as f32,
        a: walls.b,
        b: token("plum"),
    };
    m.flat(
        area.x0(),
        area.z0(),
        area.x1(),
        area.z1(),
        ceiling_height,
        false,
        ceiling,
    );
    for wall in Wall::ALL {
        let (fixed, start, end) = area.wall_line(wall);
        let n = inward(wall);
        // Solid stretches between doorways, full height.
        let mut spans = vec![(start, end)];
        for door in area.doors.iter().filter(|d| d.wall == wall) {
            let (s, e) = area.door_span(door);
            let mut next = Vec::new();
            for (a, b) in spans {
                if e <= a || s >= b {
                    next.push((a, b));
                    continue;
                }
                if s > a {
                    next.push((a, s));
                }
                if e < b {
                    next.push((e, b));
                }
            }
            spans = next;
            // The lintel over the doorway.
            m.upright(
                wall.along_x(),
                fixed,
                s,
                e,
                DOOR_HEIGHT,
                ceiling_height,
                n,
                walls,
            );
        }
        for (a, b) in spans {
            m.upright(wall.along_x(), fixed, a, b, 0.0, ceiling_height, n, walls);
            // The baseboard sits a hair inside the wall.
            let inset = fixed
                + 0.01
                    * if matches!(wall, Wall::North | Wall::East) {
                        -1.0
                    } else {
                        1.0
                    };
            m.upright(wall.along_x(), inset, a, b, 0.0, 0.18, n, trim);
            // And a picture rail near the top.
            m.upright(
                wall.along_x(),
                inset,
                a,
                b,
                ceiling_height - 0.12,
                ceiling_height - 0.04,
                n,
                trim,
            );
        }
    }
    m
}

/// The yard: its ground, the path to the door, and the house front.
fn yard(house: &House, area: &Area, catalog: &Catalog) -> ArchMesh {
    let mut m = ArchMesh::default();
    let ground = Style::of(catalog, &area.surfaces.floor);
    m.flat(
        area.x0(),
        area.z0(),
        area.x1(),
        area.z1(),
        0.0,
        true,
        ground,
    );
    // The cobbled path from where you arrive to the front door, and the
    // moonlit road beyond the gate.
    let path = Style::of(catalog, "cobble");
    if let Some(door) = area.doors.iter().find(|d| d.wall == Wall::North) {
        let [dx, dz] = area.door_centre(door);
        m.flat(dx - 1.1, area.z0(), dx + 1.1, dz - 0.6, 0.006, true, path);
        // Steps up to the door.
        let step = Style::of(catalog, "night-cobble");
        m.solid(
            Vec3::new(dx - 1.6, 0.0, dz - 0.9),
            Vec3::new(dx + 1.6, 0.15, dz),
            step,
        );
        m.solid(
            Vec3::new(dx - 1.3, 0.0, dz - 0.45),
            Vec3::new(dx + 1.3, 0.3, dz),
            step,
        );
    }
    let road = Style::pattern(Pattern::Cobble, "dusk", "night_teal");
    let mid = area.centre()[0];
    m.flat(
        mid - 1.6,
        area.z0() - 90.0,
        mid + 1.6,
        area.z0(),
        0.006,
        true,
        road,
    );
    house_front(&mut m, house, area, catalog);
    m
}

/// The house's front, standing on the yard's north wall and wider than
/// the yard: clapboard walls, a door, lit windows, a porch roof, a gabled
/// roof, and a chimney, against the sky.
fn house_front(m: &mut ArchMesh, _house: &House, area: &Area, catalog: &Catalog) {
    let z = area.z1();
    let (x0, x1) = (area.x0() - 3.0, area.x1() + 3.0);
    let face = Vec3::NEG_Z;
    let wall = Style::of(catalog, &area.surfaces.walls);
    let wood = Style::pattern(Pattern::Wood, "plum", "licorice");
    let trim = Style::plain("licorice");
    let door = area.doors.iter().find(|d| d.wall == Wall::North);
    let (ds, de) = door.map_or((0.0, 0.0), |d| area.door_span(d));
    // The ground floor around the doorway, then the upper floor.
    m.upright(true, z, x0, ds, 0.0, DOOR_HEIGHT, face, wall);
    m.upright(true, z, de, x1, 0.0, DOOR_HEIGHT, face, wall);
    m.upright(true, z, x0, x1, DOOR_HEIGHT, 4.0, face, wall);
    m.upright(true, z - 0.02, x0, x1, 3.9, 4.15, face, trim);
    m.upright(true, z, x0, x1, 4.0, FRONT_HEIGHT, face, wood);
    // The door frame.
    if door.is_some() {
        for x in [ds - 0.12, de] {
            m.solid(
                Vec3::new(x, 0.0, z - 0.12),
                Vec3::new(x + 0.12, DOOR_HEIGHT + 0.12, z),
                trim,
            );
        }
        m.solid(
            Vec3::new(ds - 0.12, DOOR_HEIGHT, z - 0.12),
            Vec3::new(de + 0.12, DOOR_HEIGHT + 0.14, z),
            trim,
        );
    }
    // Lit windows: two downstairs, four upstairs, one in the gable.
    let window = |m: &mut ArchMesh, cx: f32, y: f32, w: f32, h: f32, lit: &str| {
        m.upright(
            true,
            z - 0.03,
            cx - w / 2.0,
            cx + w / 2.0,
            y,
            y + h,
            face,
            Style::glow(lit),
        );
        // Frame and cross bars.
        for (a, b, ya, yb) in [
            (cx - w / 2.0 - 0.1, cx + w / 2.0 + 0.1, y - 0.12, y),
            (cx - w / 2.0 - 0.1, cx + w / 2.0 + 0.1, y + h, y + h + 0.12),
            (cx - w / 2.0 - 0.1, cx - w / 2.0, y, y + h),
            (cx + w / 2.0, cx + w / 2.0 + 0.1, y, y + h),
            (cx - 0.04, cx + 0.04, y, y + h),
            (
                cx - w / 2.0,
                cx + w / 2.0,
                y + h / 2.0 - 0.04,
                y + h / 2.0 + 0.04,
            ),
        ] {
            m.upright(true, z - 0.05, a, b, ya, yb, face, Style::plain("licorice"));
        }
    };
    let mid = (x0 + x1) / 2.0;
    window(m, mid - 5.5, 0.9, 1.4, 1.2, "candle");
    window(m, mid + 5.5, 0.9, 1.4, 1.2, "pumpkin");
    for (k, x) in [-7.5_f32, -3.0, 3.0, 7.5].iter().enumerate() {
        let lit = if k == 1 { "pumpkin" } else { "candle" };
        window(m, mid + x, 5.0, 1.2, 1.6, lit);
    }
    // The porch roof over the door.
    let porch = Style::of(catalog, "night-cobble");
    let (px0, px1) = (mid - 3.2, mid + 3.2);
    m.quad(
        [
            Vec3::new(px0, 3.1, z - 2.2),
            Vec3::new(px1, 3.1, z - 2.2),
            Vec3::new(px1, 3.7, z),
            Vec3::new(px0, 3.7, z),
        ],
        [[px0, 0.0], [px1, 0.0], [px1, 2.3], [px0, 2.3]],
        Vec3::new(0.0, 0.96, -0.26).normalize(),
        porch,
    );
    for x in [px0 + 0.15, px1 - 0.15] {
        m.solid(
            Vec3::new(x - 0.1, 0.0, z - 2.2),
            Vec3::new(x + 0.1, 3.1, z - 2.0),
            Style::plain("licorice"),
        );
    }
    // The main roof slopes back from the eaves; the gable rises over the
    // middle with a round window.
    let roof = Style::pattern(Pattern::Brick, "night_teal", "licorice");
    let ridge = FRONT_HEIGHT + 4.5;
    m.quad(
        [
            Vec3::new(x0 - 0.4, FRONT_HEIGHT, z - 0.6),
            Vec3::new(x1 + 0.4, FRONT_HEIGHT, z - 0.6),
            Vec3::new(x1 + 0.4, ridge, z + 5.0),
            Vec3::new(x0 - 0.4, ridge, z + 5.0),
        ],
        [[x0, 0.0], [x1, 0.0], [x1, 7.0], [x0, 7.0]],
        Vec3::new(0.0, 0.78, -0.62).normalize(),
        roof,
    );
    let (gx0, gx1, gtop) = (mid - 3.5, mid + 3.5, ridge + 2.6);
    m.tri(
        [
            Vec3::new(gx0, FRONT_HEIGHT, z - 0.3),
            Vec3::new(gx1, FRONT_HEIGHT, z - 0.3),
            Vec3::new(mid, gtop, z - 0.3),
        ],
        [[gx0, FRONT_HEIGHT], [gx1, FRONT_HEIGHT], [mid, gtop]],
        face,
        wood,
    );
    for (a, b) in [(gx0 - 0.5, mid), (mid, gx1 + 0.5)] {
        let (ya, yb) = if a < mid {
            (FRONT_HEIGHT - 0.3, gtop + 0.2)
        } else {
            (gtop + 0.2, FRONT_HEIGHT - 0.3)
        };
        m.quad(
            [
                Vec3::new(a, ya, z - 0.5),
                Vec3::new(b, yb, z - 0.5),
                Vec3::new(b, yb, z + 4.0),
                Vec3::new(a, ya, z + 4.0),
            ],
            [[0.0, 0.0], [4.0, 0.0], [4.0, 4.5], [0.0, 4.5]],
            Vec3::new(if a < mid { -0.6 } else { 0.6 }, 0.8, 0.0).normalize(),
            roof,
        );
    }
    window(m, mid, FRONT_HEIGHT + 0.9, 1.0, 1.0, "candle");
    // A crooked chimney.
    m.solid(
        Vec3::new(x1 - 4.0, ridge - 2.0, z + 2.0),
        Vec3::new(x1 - 3.0, ridge + 2.2, z + 3.0),
        Style::of(catalog, "brick"),
    );
}

/// Far scenery around the yard: the dark ground beyond it and flat
/// triangle mountains to the south, against the sky.
#[must_use]
pub fn scenery(house: &House) -> ArchMesh {
    let mut m = ArchMesh::default();
    let Some(yard) = house.areas.iter().find(|a| a.kind == AreaKind::Yard) else {
        return m;
    };
    let [cx, _] = yard.centre();
    let ground = Style::pattern(Pattern::Grass, "night_teal", "plum");
    // Ground everywhere below the house, a hair lower than any floor.
    m.flat(
        cx - 160.0,
        yard.z0() - 160.0,
        cx + 160.0,
        yard.z1() + 160.0,
        -0.02,
        true,
        ground,
    );
    // Two rows of mountains far to the south, and lower hills north.
    for (row, (z, h, a, b)) in [
        (yard.z0() - 140.0, 34.0, "dusk", "plum"),
        (yard.z0() - 110.0, 20.0, "plum", "night_teal"),
        (yard.z1() + 150.0, 18.0, "plum", "night_teal"),
    ]
    .into_iter()
    .enumerate()
    {
        let style = Style::pattern(Pattern::Halftone, a, b);
        let face = if z < yard.z0() { Vec3::Z } else { Vec3::NEG_Z };
        let mut x = cx - 170.0;
        let mut k = 0u32;
        while x < cx + 170.0 {
            let w = 26.0 + ((k * 7 + row as u32 * 3) % 5) as f32 * 7.0;
            let peak = h * (0.6 + ((k * 5 + row as u32) % 4) as f32 * 0.17);
            m.tri(
                [
                    Vec3::new(x, -0.5, z),
                    Vec3::new(x + w, -0.5, z),
                    Vec3::new(x + w * 0.5, peak, z),
                ],
                [[0.0, 0.0], [w, 0.0], [w * 0.5, peak]],
                face,
                style,
            );
            x += w * 0.7;
            k += 1;
        }
    }
    m
}

/// One area's architecture.
#[must_use]
pub fn area(house: &House, area: &Area, catalog: &Catalog) -> ArchMesh {
    match area.kind {
        AreaKind::Yard => yard(house, area, catalog),
        AreaKind::Hallway | AreaKind::Room => indoor(area, catalog),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_area_has_a_floor_walls_and_open_doorways() {
        let house = haunt_rules::v1();
        let catalog = haunt_assets::catalog();
        for a in &house.areas {
            let mesh = area(&house, a, catalog);
            assert!(mesh.vertices() > 0 && mesh.vertices() % 3 == 0, "{}", a.id);
            // Normals are unit length and patterns known.
            for v in mesh.data.chunks(STRIDE) {
                let n = Vec3::new(v[3], v[4], v[5]);
                assert!((n.length() - 1.0).abs() < 1e-3, "{}", a.id);
                assert!(v[14] <= 7.0 || v[14] == GLOW, "{}", a.id);
            }
            if a.kind == AreaKind::Yard {
                continue;
            }
            // No wall triangle covers the middle of a doorway below its
            // lintel.
            for door in &a.doors {
                let [dx, dz] = a.door_centre(door);
                for tri in mesh.data.chunks(STRIDE * 3) {
                    let ys: Vec<f32> = (0..3).map(|k| tri[k * STRIDE + 1]).collect();
                    let n = Vec3::new(tri[3], tri[4], tri[5]);
                    if n.y.abs() > 0.5 || ys.iter().all(|y| *y >= DOOR_HEIGHT - 1e-3) {
                        continue;
                    }
                    let xs: Vec<f32> = (0..3).map(|k| tri[k * STRIDE]).collect();
                    let zs: Vec<f32> = (0..3).map(|k| tri[k * STRIDE + 2]).collect();
                    let inside = |v: &[f32], c: f32| {
                        v.iter().cloned().fold(f32::MAX, f32::min) < c - 0.05
                            && v.iter().cloned().fold(f32::MIN, f32::max) > c + 0.05
                    };
                    let on_line = if door.wall.along_x() {
                        zs.iter().all(|z| (z - dz).abs() < 0.02) && inside(&xs, dx)
                    } else {
                        xs.iter().all(|x| (x - dx).abs() < 0.02) && inside(&zs, dz)
                    };
                    assert!(!on_line, "{} door {} is walled up", a.id, door.id);
                }
            }
        }
        assert!(scenery(&house).vertices() > 0);
    }
}
