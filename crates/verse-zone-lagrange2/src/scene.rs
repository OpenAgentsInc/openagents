//! The factory's geometry: the deck truss, pedestals, racks, cradle, CMG
//! pods, and solar wings (fixed); the arms and parts (moving); and the orbit
//! display above the deck. Built in the factory frame, which is the scene.

use glam::{DQuat, DVec3, Mat4, Quat, Vec3};
use verse_lagrange2::{
    factory::{self, CRADLE, DECK, DECK_SIZE, Factory, PEDESTAL, RACK, TOOL},
    ship::{Part, PartKind, Shape, Subsystem},
};
use verse_zone_lagrange::draw::{Surface, band, block, cuboid, frustum, lattice, member};

use crate::pbr::{LitVertex, Material};

/// A part's mesh in its own frame, placed by `t`.
pub fn part_mesh(out: &mut Vec<LitVertex>, part: &Part, t: &Mat4) {
    let along_x = Mat4::from_quat(Quat::from_rotation_arc(Vec3::Z, Vec3::X));
    match (part.kind, part.shape) {
        (PartKind::Truss, Shape::Box { size }) => {
            let h = size.as_vec3() / 2.0;
            // Four longerons with bays of diagonals, end frames, and a
            // white grapple fixture on top.
            let corners = [
                Vec3::new(0.0, h.y, h.z),
                Vec3::new(0.0, h.y, -h.z),
                Vec3::new(0.0, -h.y, -h.z),
                Vec3::new(0.0, -h.y, h.z),
            ];
            let local = |p: Vec3| t.transform_point3(p);
            for c in corners {
                member(
                    out,
                    local(c - Vec3::X * h.x),
                    local(c + Vec3::X * h.x),
                    0.28,
                    Material::Aluminium,
                );
            }
            let bays = (size.x / 2.0).round().max(1.0) as usize;
            for b in 0..=bays {
                let x = -h.x + size.x as f32 * b as f32 / bays as f32;
                for k in 0..4 {
                    let (a, c) = (corners[k], corners[(k + 1) % 4]);
                    member(
                        out,
                        local(a + Vec3::X * x),
                        local(c + Vec3::X * x),
                        0.14,
                        Material::Aluminium,
                    );
                    if b < bays {
                        let next = x + size.x as f32 / bays as f32;
                        let (p, q) = if b % 2 == 0 {
                            (a + Vec3::X * x, c + Vec3::X * next)
                        } else {
                            (c + Vec3::X * x, a + Vec3::X * next)
                        };
                        member(out, local(p), local(q), 0.08, Material::Aluminium);
                    }
                }
            }
            block_at(
                out,
                t,
                Vec3::new(0.0, h.y + 0.12, 0.0),
                Vec3::new(0.6, 0.12, 0.6),
                Material::WhitePaint,
            );
        }
        (PartKind::Tank { .. }, Shape::Cylinder { radius, length }) => {
            let (r, l) = (radius as f32, length as f32);
            frustum(
                out,
                &(*t * along_x),
                r,
                r,
                l - 2.0 * r * 0.5,
                28,
                Material::Mli,
                false,
            );
            for side in [-1.0f32, 1.0] {
                let dome = Mat4::from_translation(Vec3::X * side * (l / 2.0 - r * 0.25));
                let flip = if side > 0.0 {
                    Mat4::IDENTITY
                } else {
                    Mat4::from_rotation_y(std::f32::consts::PI)
                };
                frustum(
                    out,
                    &(*t * dome * flip * along_x),
                    r,
                    r * 0.45,
                    r * 0.5,
                    28,
                    Material::Mli,
                    true,
                );
            }
            for x in [-l / 4.0, l / 4.0] {
                band_at(
                    out,
                    t,
                    Vec3::X * x,
                    Vec3::X,
                    r + 0.03,
                    0.3,
                    Material::Aluminium,
                );
            }
            block_at(
                out,
                t,
                Vec3::new(0.0, r + 0.1, 0.0),
                Vec3::new(0.6, 0.12, 0.6),
                Material::WhitePaint,
            );
        }
        (PartKind::Engine { .. }, Shape::Cylinder { radius, length }) => {
            let (r, l) = (radius as f32, length as f32);
            // A thrust structure up front and a bell aft.
            let body = Mat4::from_translation(Vec3::X * (l * 0.25));
            frustum(
                out,
                &(*t * body * along_x),
                r * 0.8,
                r * 0.8,
                l * 0.5,
                24,
                Material::Mli,
                true,
            );
            let bell = Mat4::from_translation(-Vec3::X * (l * 0.25))
                * Mat4::from_rotation_y(std::f32::consts::PI);
            frustum(
                out,
                &(*t * bell * along_x),
                r * 0.35,
                r,
                l * 0.5,
                32,
                Material::Dark,
                false,
            );
            block_at(
                out,
                t,
                Vec3::new(l * 0.25, r * 0.8 + 0.1, 0.0),
                Vec3::new(0.6, 0.12, 0.6),
                Material::WhitePaint,
            );
        }
        (_, Shape::Cylinder { radius, length }) => {
            let (r, l) = (radius as f32, length as f32);
            frustum(
                out,
                &(*t * along_x),
                r,
                r,
                l,
                32,
                Material::WhitePaint,
                true,
            );
            for x in [-l / 2.0 + 0.4, 0.0, l / 2.0 - 0.4] {
                band_at(
                    out,
                    t,
                    Vec3::X * x,
                    Vec3::X,
                    r + 0.04,
                    0.25,
                    Material::Aluminium,
                );
            }
            // A window band and a docking ring forward.
            band_at(
                out,
                t,
                Vec3::X * (l / 4.0),
                Vec3::X,
                r + 0.02,
                0.5,
                Material::Dark,
            );
            let ring = Mat4::from_translation(Vec3::X * (l / 2.0 + 0.3));
            frustum(
                out,
                &(*t * ring * along_x),
                r * 0.5,
                r * 0.45,
                0.6,
                24,
                Material::Aluminium,
                true,
            );
            block_at(
                out,
                t,
                Vec3::new(0.0, r + 0.1, 0.0),
                Vec3::new(0.6, 0.12, 0.6),
                Material::Dark,
            );
        }
        (_, Shape::Box { size }) => {
            let h = size.as_vec3() / 2.0;
            let surface = match part.subsystem {
                Subsystem::Power => Material::SolarCell,
                Subsystem::Thermal => Material::Radiator,
                _ => Material::WhitePaint,
            };
            if part.subsystem == Subsystem::Power {
                // A battery box with a cell-faced top.
                cuboid(out, t, h - Vec3::Y * 0.05, Material::Mli);
                block_at(
                    out,
                    t,
                    Vec3::Y * (h.y - 0.02),
                    Vec3::new(h.x, 0.04, h.z),
                    surface,
                );
            } else {
                cuboid(out, t, h, surface);
            }
            block_at(
                out,
                t,
                Vec3::Y * (h.y + 0.1),
                Vec3::new(0.5, 0.1, 0.5),
                Material::Dark,
            );
        }
    }
}

fn block_at(out: &mut Vec<LitVertex>, t: &Mat4, center: Vec3, half: Vec3, s: impl Into<Surface>) {
    cuboid(out, &(*t * Mat4::from_translation(center)), half, s);
}

fn band_at(
    out: &mut Vec<LitVertex>,
    t: &Mat4,
    center: Vec3,
    axis: Vec3,
    radius: f32,
    width: f32,
    s: impl Into<Surface>,
) {
    let mut ring = Vec::new();
    band(&mut ring, center, axis, radius, width, s);
    for v in &mut ring {
        let p = t.transform_point3(Vec3::from_array(v.pos));
        let n = t
            .transform_vector3(Vec3::from_array(v.normal))
            .normalize_or_zero();
        let tg = t
            .transform_vector3(Vec3::from_array(v.tangent))
            .normalize_or_zero();
        v.pos = p.to_array();
        v.normal = n.to_array();
        v.tangent = tg.to_array();
    }
    out.extend(ring);
}

/// The fixed factory: deck truss, plating under the racks, pedestals,
/// racks, the cradle, CMG pods, and two solar wings.
pub fn structure(out: &mut Vec<LitVertex>, ship: &verse_lagrange2::Spacecraft) {
    let top = DECK as f32;
    let half = (DECK_SIZE / 2.0).as_vec3();
    let depth = DECK_SIZE.y as f32;
    // The deck: a two-layer truss grid on 12 m bays.
    let bay = 12.0;
    let n = (half.x * 2.0 / bay).round() as i32;
    for layer in [top - 0.3, top - depth + 0.3] {
        for i in 0..=n {
            let c = -half.x + i as f32 * bay;
            lattice(
                out,
                Vec3::new(c, layer, -half.z),
                Vec3::new(c, layer, half.z),
                0.6,
                Material::Aluminium,
            );
            lattice(
                out,
                Vec3::new(-half.x, layer, c),
                Vec3::new(half.x, layer, c),
                0.6,
                Material::Aluminium,
            );
        }
    }
    for i in 0..=n {
        for j in 0..=n {
            let (x, z) = (-half.x + i as f32 * bay, -half.z + j as f32 * bay);
            member(
                out,
                Vec3::new(x, top - depth + 0.3, z),
                Vec3::new(x, top - 0.3, z),
                0.3,
                Material::Aluminium,
            );
        }
    }
    // Plating along the rack rows and under the cradle, where people and
    // parts work.
    for side in [-1.0f32, 1.0] {
        block(
            out,
            Vec3::new(side * factory::RACK_X as f32, top - 0.1, 0.0),
            Vec3::new(10.0, 0.1, half.z - 4.0),
            Quat::IDENTITY,
            Surface::tinted(Material::WhitePaint, [0.55, 0.56, 0.58]),
        );
    }
    block(
        out,
        Vec3::new(0.0, top - 0.1, 0.0),
        Vec3::new(30.0, 0.1, 7.0),
        Quat::IDENTITY,
        Surface::tinted(Material::WhitePaint, [0.5, 0.5, 0.52]),
    );
    // Pedestals: lattice towers with a ring at the top.
    for &[x, z] in &factory::ARM_BASES {
        let base = Vec3::new(x as f32, top, z as f32);
        let up = base + Vec3::Y * PEDESTAL as f32;
        for (dx, dz) in [(-1.2, -1.2), (1.2, -1.2), (1.2, 1.2), (-1.2, 1.2)] {
            member(
                out,
                base + Vec3::new(dx, 0.0, dz),
                up + Vec3::new(dx * 0.5, 0.0, dz * 0.5),
                0.3,
                Material::SafetyPaint,
            );
        }
        for h in [0.3, 0.5, 0.75] {
            let y = base + Vec3::Y * (PEDESTAL as f32 * h);
            band(out, y, Vec3::Y, 1.2 - h * 0.6, 0.25, Material::Dark);
        }
        frustum(
            out,
            &Mat4::from_rotation_translation(
                Quat::from_rotation_arc(Vec3::Z, Vec3::Y),
                up - Vec3::Y * 0.3,
            ),
            1.4,
            1.4,
            0.6,
            24,
            Material::Dark,
            true,
        );
    }
    // Racks: a dark beam frame under each slot.
    for &[x, z] in &factory::ARM_BASES {
        for slot in factory::RACK_SLOTS.iter().take(2) {
            let center = Vec3::new(
                x.signum() as f32 * factory::RACK_X as f32,
                top + RACK as f32 / 2.0,
                (z + z.signum() * slot) as f32,
            );
            for dx in [-6.0, 0.0, 6.0] {
                block(
                    out,
                    center + Vec3::X * dx,
                    Vec3::new(0.3, RACK as f32 / 2.0, 2.2),
                    Quat::IDENTITY,
                    Material::Dark,
                );
            }
            block(
                out,
                center + Vec3::Y * 0.45,
                Vec3::new(7.0, 0.05, 0.3),
                Quat::IDENTITY,
                Material::SafetyPaint,
            );
        }
    }
    // The cradle: saddles under the keel at its latch, and a spine.
    let keel = ship.parts.iter().find(|p| p.attach.parent.is_none());
    if let Some(keel) = keel {
        let latch = (CRADLE + keel.attach.at).as_vec3();
        for dx in [-6.0, 0.0, 6.0] {
            let at = Vec3::new(latch.x + dx, (top + latch.y) / 2.0, 0.0);
            block(
                out,
                at,
                Vec3::new(0.4, (latch.y - top) / 2.0, 1.6),
                Quat::IDENTITY,
                Material::SafetyPaint,
            );
        }
    }
    lattice(
        out,
        Vec3::new(-28.0, top + 0.6, 0.0),
        Vec3::new(30.0, top + 0.6, 0.0),
        1.0,
        Material::Aluminium,
    );
    // CMG pods at the deck's corners, under it.
    for (x, z) in [(-36.0, -36.0), (36.0, -36.0), (-36.0, 36.0), (36.0, 36.0)] {
        let at = Vec3::new(x, top - depth - 1.5, z);
        block(
            out,
            at,
            Vec3::new(2.5, 1.5, 2.5),
            Quat::IDENTITY,
            Material::Mli,
        );
        frustum(
            out,
            &Mat4::from_rotation_translation(
                Quat::from_rotation_arc(Vec3::Z, Vec3::Y),
                at - Vec3::Y * 1.8,
            ),
            1.8,
            1.8,
            0.6,
            24,
            Material::WhitePaint,
            true,
        );
    }
    // Two solar wings off the aft edge, tilted toward the Sun's side.
    for side in [-1.0f32, 1.0] {
        let root = Vec3::new(-half.x, top - depth / 2.0, side * 20.0);
        let tip = root + Vec3::new(-6.0, 0.0, side * 50.0);
        member(out, root, tip, 0.5, Material::Aluminium);
        for k in 0..6 {
            let t0 = k as f32 / 6.0 + 0.04;
            let center = root.lerp(tip, t0 + 0.06);
            block(
                out,
                center,
                Vec3::new(9.0, 0.05, 3.6),
                Quat::from_rotation_z(0.45),
                Material::SolarCell,
            );
        }
    }
}

/// An arm's links from their bodies, in the factory frame.
pub fn arm_mesh(
    out: &mut Vec<LitVertex>,
    f: &Factory,
    index: usize,
    frame: &dyn Fn(DVec3, DQuat) -> Mat4,
) {
    let a = &f.arms[index];
    let lengths = [
        a.spec.shoulder,
        a.spec.upper,
        a.spec.forearm,
        a.spec.wrist - TOOL,
        TOOL,
    ];
    let widths = [1.3, 1.0, 0.85, 0.7, 0.6];
    for (i, &link) in a.links.iter().enumerate() {
        let b = &f.world[link];
        let t = frame(b.pos, b.orientation);
        let half = lengths[i] as f32 / 2.0;
        let w = widths[i];
        if i == 0 {
            // The turret: a column on a slewing ring.
            frustum(
                out,
                &(t * Mat4::from_quat(Quat::from_rotation_arc(Vec3::Z, Vec3::Y))),
                w * 0.8,
                w * 0.65,
                half * 2.0,
                20,
                Material::WhitePaint,
                false,
            );
            band(
                out,
                t.transform_point3(-Vec3::Y * (half - 0.3)),
                t.transform_vector3(Vec3::Y).normalize(),
                w * 1.05,
                0.6,
                Material::Dark,
            );
        } else if i == 4 {
            // The tool: a grapple housing with three snare fingers.
            cuboid(out, &t, Vec3::new(w, half, w), Material::Dark);
            for k in 0..3 {
                let angle = k as f32 * std::f32::consts::TAU / 3.0;
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * (w * 0.8);
                block(
                    out,
                    t.transform_point3(offset + Vec3::Y * (half + 0.2)),
                    Vec3::new(0.12, 0.3, 0.12),
                    Quat::IDENTITY,
                    Material::Aluminium,
                );
            }
        } else {
            // A boom: white composite with a dark joint housing at its root.
            cuboid(
                out,
                &t,
                Vec3::new(w * 0.5, half - 0.6, w * 0.5),
                Material::WhitePaint,
            );
            let joint = t * Mat4::from_translation(-Vec3::Y * (half - 0.4));
            frustum(
                out,
                &joint,
                w * 0.8,
                w * 0.8,
                w * 1.6,
                20,
                Material::Dark,
                true,
            );
            band(
                out,
                t.transform_point3(Vec3::Y * (half * 0.2)),
                t.transform_vector3(Vec3::Y).normalize(),
                w * 0.52,
                0.3,
                Material::SafetyPaint,
            );
        }
    }
}

/// A guide line pair.
pub fn line(mesh: &mut crate::mesh::Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    let v = |p: Vec3| crate::mesh::Vertex {
        pos: p.to_array(),
        color,
        fog: 0.0,
    };
    mesh.lines.push(v(a));
    mesh.lines.push(v(b));
}

/// A circle of guide lines.
pub fn circle(
    mesh: &mut crate::mesh::Mesh,
    center: Vec3,
    axis: Vec3,
    radius: f32,
    color: [f32; 3],
) {
    let a = axis.normalize();
    let u = a.any_orthonormal_vector();
    let v = a.cross(u);
    let n = 40;
    for i in 0..n {
        let p = |k: usize| {
            let t = k as f32 / n as f32 * std::f32::consts::TAU;
            center + (u * t.cos() + v * t.sin()) * radius
        };
        line(mesh, p(i), p(i + 1), color);
    }
}
