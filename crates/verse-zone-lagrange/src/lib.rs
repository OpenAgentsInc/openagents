//! Lagrange 1: a construction station on a Lissajous orbit about Sun–Earth L1.
//! Physics lives in `verse-lagrange`; this crate maps input and draws the
//! scene, and `verse` re-exports it as `zones::lagrange`, so an edit here
//! recompiles this crate and what depends on it rather than all of Verse.
//!
//! Scene axes follow the station frame: -Z faces the Sun, +Z the Earth, +Y the
//! ecliptic north pole. Structure, parts, and the suit are generated here with
//! physical materials and drawn by the renderer's physical path
//! ([`crate::pbr`]); the Sun, Earth, Moon, and stars come from real data at
//! infinity. Guides and the forces overlay stay as lines.

mod draw;
mod light;

// The paths this zone was written against inside `crates/verse`.
use verse_core::world;
use verse_pbr::{mesh, pbr};
use verse_world::social::controller;

use std::sync::OnceLock;

use crate::{
    controller::{InputState, PlayerController, TURN_SPEED},
    mesh::{Mesh, Vertex},
    pbr::{Camera, GlowVertex, LitVertex, Material, bake},
    world::World,
};
use draw::{Surface, band, block, cuboid, cuboid_faces, frustum, lattice, member};
use glam::{DVec3, Mat4, Quat, Vec3};
use verse_lagrange::{
    Command, PartKind, PartState, Station,
    physics::DebugKind,
    station::{self, AIRLOCK},
};

/// Distance of guide marks for sky bodies, inside the camera's 2 km far plane.
const SKY: f32 = 1_850.0;
/// Camera pitch that thrusts level; tilting beyond the band climbs or dives.
const LEVEL_PITCH: f32 = 0.28;
const LEVEL_BAND: f32 = 0.12;
/// The return portal, beside the airlock.
pub const RETURN_PORTAL: Vec3 = Vec3::new(-5.0, 4.4, 22.5);

pub struct Lagrange {
    pub station: Station,
    /// Draw contacts, their impulses, joints, and thrust.
    pub overlay: bool,
    /// Use the readable art exposure instead of the photographic one.
    pub art: bool,
    /// Draw guides and overlays in the neutral palette: white and grays
    /// instead of cyan, amber, and green. Set when entered from the Grid.
    pub neutral: bool,
    rendered: Mesh,
    light: light::Light,
}

impl Lagrange {
    pub fn new() -> Self {
        let mut zone = Self {
            station: Station::new(),
            overlay: false,
            art: false,
            neutral: false,
            rendered: Mesh::default(),
            light: light::Light::new(structure_with_wings()),
        };
        zone.tick();
        zone
    }

    /// Feet position for the shared player controller.
    pub fn spawn() -> Vec3 {
        feet(station::SPAWN)
    }

    /// Face the station about 65° off the Sun line: modules read as
    /// half-lit cylinders with hard terminators, as in EVA photographs,
    /// instead of silhouettes against the Sun.
    pub fn spawn_yaw() -> f32 {
        -2.0
    }

    pub fn move_player(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        camera_pitch: f32,
        dt: f32,
    ) {
        if !input.mouse_look {
            if input.left {
                player.yaw = crate::controller::wrap(player.yaw + TURN_SPEED * dt);
            }
            if input.right {
                player.yaw = crate::controller::wrap(player.yaw - TURN_SPEED * dt);
            }
        }
        let strafe_left = input.strafe_left || (input.mouse_look && input.left);
        let strafe_right = input.strafe_right || (input.mouse_look && input.right);
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let ahead = axis(input.forward, input.backward);
        let side = axis(strafe_right, strafe_left);
        // Tilting the view past the level band flies along the view.
        let tilt = camera_pitch - LEVEL_PITCH;
        let elevation = if tilt.abs() > LEVEL_BAND {
            -(tilt - LEVEL_BAND * tilt.signum())
        } else {
            0.0
        };
        let forward =
            crate::controller::forward(player.yaw) * elevation.cos() + Vec3::Y * elevation.sin();
        let right = Vec3::new(-player.yaw.cos(), 0.0, player.yaw.sin());
        let direction = forward * ahead + right * side;
        self.station.step(
            f64::from(dt),
            &Command {
                direction: direction.as_dvec3(),
                yaw: f64::from(player.yaw),
                climb: input.jump,
            },
        );
        let astronaut = self.station.astronaut();
        player.pos = feet(astronaut.interpolated(self.station.alpha()).0);
        player.speed = astronaut.vel.length() as f32;
        player.set_surface_height(player.pos.y);
    }

    pub fn tick(&mut self) {
        let station = &self.station;
        self.light
            .update(station, structure_with_wings(), || resting_parts(station));
        self.rendered = self.build_dynamic();
    }

    /// Waits for the light bake so an offline capture sees bounce light.
    pub fn settle_light(&mut self) {
        self.light.wait();
        self.rendered = self.build_dynamic();
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// Fixed station structure.
    pub fn world() -> World {
        World {
            mesh: structure().clone(),
            blockers: Vec::new(),
        }
    }

    /// Fixed station structure with its guide (the airlock's refill ring)
    /// in the neutral palette, at the same brightness.
    pub fn neutral_world() -> World {
        let mut world = Self::world();
        for vertex in &mut world.mesh.lines {
            let [r, g, b] = vertex.color;
            vertex.color = [r.max(g).max(b); 3];
        }
        world
    }

    fn camera(&self) -> Camera {
        if self.art {
            Camera::art()
        } else {
            Camera::helmet()
        }
    }

    /// A guide's color: as given, or in the neutral palette its brightest
    /// channel in white light, so it stays as bright.
    fn guide(&self, color: [f32; 3]) -> [f32; 3] {
        if self.neutral {
            [color[0].max(color[1]).max(color[2]); 3]
        } else {
            color
        }
    }

    fn build_dynamic(&self) -> Mesh {
        let mut mesh = Mesh::default();
        let s = &self.station;
        let alpha = s.alpha();
        let astronaut_pos = s.astronaut().interpolated(alpha).0;
        let astronaut = astronaut_pos.as_vec3();
        let head = astronaut + Vec3::Y * 0.7;
        let time = (s.world.tick as f64 * station::PHYSICS_DT) as f32;
        let sky = self.light.sky(s, head, self.camera(), time);
        // A small reticle marks the Earth, which is only nine pixels across.
        let at = sky.earth.dir * SKY;
        let r = 22.0;
        let (u, v) = basis(at);
        let reticle = self.guide([0.3, 0.8, 0.9]);
        for (du, dv) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let corner = at + (u * du + v * dv) * r;
            line(&mut mesh, corner, corner - u * du * 7.0, reticle);
            line(&mut mesh, corner, corner - v * dv * 7.0, reticle);
        }
        let carrying = s.parts.iter().any(|p| p.state == PartState::Carried);
        suit(
            &mut mesh.lit,
            feet(astronaut_pos),
            s.heading_yaw() as f32,
            carrying,
        );
        flexed_wings(&mut mesh.lit, s);
        ropes(&mut mesh.lit, s);
        let sunlit = |p: Vec3| self.light.sunlit(p, sky.sun_dir);
        plume_glows(&mut mesh.glow, s, sky.sun_dir, &sunlit);
        ice_flakes(&mut mesh.glow, s, sky.sun_dir, &sunlit);
        for part in &s.parts {
            let (pos, orientation) = s.body(part).interpolated(alpha);
            let transform = Mat4::from_rotation_translation(orientation.as_quat(), pos.as_vec3());
            part_mesh(&mut mesh.lit, part.kind, &transform);
            if part.state == PartState::Carried {
                let slot = part.kind.slot().as_vec3();
                let ready = s.snapshot().latch_ready;
                // Neutral guides keep the cue in brightness: gray until
                // aligned, then white.
                let color = match (self.neutral, ready) {
                    (false, true) => [0.2, 1.3, 0.5],
                    (false, false) => [1.2, 0.7, 0.15],
                    (true, true) => [1.3; 3],
                    (true, false) => [0.45; 3],
                };
                outline(&mut mesh, slot, part.kind.size().as_vec3() * 0.5, color);
                dashed(&mut mesh, pos.as_vec3(), slot, color);
            }
        }
        if let Some(next) = s.next_part()
            && !carrying
        {
            outline(
                &mut mesh,
                next.stowage().as_vec3(),
                next.size().as_vec3() * 0.55,
                self.guide([0.9, 0.8, 0.3]),
            );
        }
        if let Some(target) = s.target {
            dashed(&mut mesh, astronaut, target.as_vec3(), reticle);
        }
        if self.overlay {
            for l in s.debug_lines() {
                let color = match l.kind {
                    DebugKind::ContactNormal => [0.3, 0.9, 1.2],
                    DebugKind::ContactImpulse => [1.4, 0.3, 0.2],
                    DebugKind::Joint => [0.9, 0.9, 0.3],
                    DebugKind::Strained => [1.5, 0.2, 0.9],
                    DebugKind::Thrust => [1.2, 0.7, 1.4],
                };
                line(
                    &mut mesh,
                    l.from.as_vec3(),
                    l.to.as_vec3(),
                    self.guide(color),
                );
            }
        }
        mesh.sky = Some(sky);
        mesh
    }
}

/// The fixed structure, built once: lit geometry with baked occlusion, and
/// the airlock's refill ring as a guide. The solar wings flex, so they are
/// drawn each frame from [`wings`] and only join the structure for baking.
fn structure() -> &'static Mesh {
    baked().0
}

/// The undeflected wings, with occlusion baked against the whole station.
fn wings() -> &'static [Vec<LitVertex>; 2] {
    baked().1
}

/// The fixed structure and wings together, as the light bake and sun
/// visibility see them.
fn structure_with_wings() -> &'static [LitVertex] {
    baked().2
}

fn baked() -> (
    &'static Mesh,
    &'static [Vec<LitVertex>; 2],
    &'static [LitVertex],
) {
    static BAKED: OnceLock<(Mesh, [Vec<LitVertex>; 2], Vec<LitVertex>)> = OnceLock::new();
    let (mesh, wings, all) = BAKED.get_or_init(|| {
        let mut fixed = Vec::new();
        build_structure(&mut fixed);
        let mut wings = [-1.0_f32, 1.0].map(|side| {
            let mut wing = Vec::new();
            solar_wing(&mut wing, side);
            wing
        });
        let mut all = fixed.clone();
        all.extend(wings.iter().flatten().copied());
        let bvh = bake::Bvh::new(&all);
        bake::bake_occlusion(&mut fixed, &bvh, 2.5, 24);
        for wing in &mut wings {
            bake::bake_occlusion(wing, &bvh, 2.5, 24);
        }
        let mut all = fixed.clone();
        all.extend(wings.iter().flatten().copied());
        let mut mesh = Mesh {
            lit: fixed,
            ..Mesh::default()
        };
        ring(
            &mut mesh,
            AIRLOCK.as_vec3(),
            Vec3::Z,
            station::REFILL_RANGE as f32,
            [0.2, 0.9, 0.5],
        );
        (mesh, wings, all)
    });
    (mesh, wings, all)
}

/// Positions of every drawn vertex of the fixed structure and the
/// undeflected wings.
#[cfg(test)]
pub fn structure_vertices(out: &mut Vec<[f32; 3]>) {
    out.extend(structure_with_wings().iter().map(|v| v.pos));
}

/// The wings bent by their structural modes this frame.
fn flexed_wings(out: &mut Vec<LitVertex>, s: &Station) {
    for (index, wing) in wings().iter().enumerate() {
        let flex = s.array_flex(index);
        out.extend(wing.iter().map(|v| {
            let p = flex.deflect(DVec3::from(v.pos.map(f64::from)));
            LitVertex {
                pos: p.as_vec3().to_array(),
                ..*v
            }
        }));
    }
}

/// Tethers and depot lines as round tubes along their ropes, with frames
/// carried along the curve so the tube never twists (Wang et al. 2008). The
/// tubes have the radius each rope keeps from the structure, so a line lies
/// on what it wraps. An unclipped line ends in its clip.
fn ropes(out: &mut Vec<LitVertex>, s: &Station) {
    let alpha = s.alpha();
    for (index, rope) in s.ropes().iter().enumerate() {
        let (radius, surface) = if index == 0 {
            (station::TETHER_RADIUS, Surface::from(Material::Fabric))
        } else {
            (station::LINE_RADIUS, Surface::from(Material::SafetyPaint))
        };
        let points: Vec<Vec3> = (0..rope.points.len())
            .map(|i| rope.point(i, alpha).as_vec3())
            .collect();
        tube(out, &points, radius as f32, surface);
        if !rope.clipped
            && let [.., before, end] = points.as_slice()
        {
            let along = (*end - *before).try_normalize().unwrap_or(Vec3::Z);
            block(
                out,
                *end,
                Vec3::new(0.025, 0.025, 0.05),
                Quat::from_rotation_arc(Vec3::Z, along),
                Material::Aluminium,
            );
        }
    }
}

fn tube(out: &mut Vec<LitVertex>, points: &[Vec3], radius: f32, surface: Surface) {
    const SIDES: usize = 6;
    if points.len() < 2 {
        return;
    }
    let tangent = |i: usize| {
        let a = points[i.saturating_sub(1)];
        let b = points[(i + 1).min(points.len() - 1)];
        (b - a).try_normalize().unwrap_or(Vec3::Z)
    };
    let mut t0 = tangent(0);
    let mut normal = t0.any_orthonormal_vector();
    let mut rings = Vec::with_capacity(points.len());
    for (i, &point) in points.iter().enumerate() {
        let t1 = tangent(i);
        // Rotate the frame by the minimal rotation from the last tangent.
        if let Some(axis) = t0.cross(t1).try_normalize() {
            let angle = t0.angle_between(t1);
            normal = Quat::from_axis_angle(axis, angle) * normal;
        }
        normal = (normal - t1 * normal.dot(t1))
            .try_normalize()
            .unwrap_or(normal);
        let binormal = t1.cross(normal);
        rings.push((point, t1, normal, binormal));
        t0 = t1;
    }
    let id = Mat4::IDENTITY;
    for pair in rings.windows(2) {
        let (a, ta, na, ba) = pair[0];
        let (b, tb, nb, bb) = pair[1];
        for k in 0..SIDES {
            let angle = |k: usize| k as f32 / SIDES as f32 * std::f32::consts::TAU;
            let dir = |n: Vec3, bn: Vec3, k: usize| n * angle(k).cos() + bn * angle(k).sin();
            let quad = [
                (a + dir(na, ba, k) * radius, dir(na, ba, k), ta),
                (a + dir(na, ba, k + 1) * radius, dir(na, ba, k + 1), ta),
                (b + dir(nb, bb, k + 1) * radius, dir(nb, bb, k + 1), tb),
                (b + dir(nb, bb, k) * radius, dir(nb, bb, k), tb),
            ];
            for i in [0, 1, 2, 0, 2, 3] {
                let (p, n, t) = quad[i];
                let mut v = draw::vertex(surface, &id, p, n, t);
                v.local = p.to_array();
                out.push(v);
            }
        }
    }
}

fn build_structure(out: &mut Vec<LitVertex>) {
    let aluminium = Surface::from(Material::Aluminium);
    let paint = Surface::from(Material::WhitePaint);
    let safety = Surface::from(Material::SafetyPaint);
    // Main truss along x.
    lattice(
        out,
        Vec3::new(-30.0, 6.0, 0.0),
        Vec3::new(30.0, 6.0, 0.0),
        1.4,
        aluminium,
    );
    for side in [-1.0_f32, 1.0] {
        // Radiators stand edge-on to the Sun so they reject heat to space.
        let x = 8.0 * side;
        block(
            out,
            Vec3::new(x, 10.85, 4.2),
            Vec3::new(0.04, 4.15, 4.8),
            Quat::IDENTITY,
            paint,
        );
        member(
            out,
            Vec3::new(x, 6.6, 4.2),
            Vec3::new(x, 6.7, 4.2),
            0.3,
            aluminium,
        );
        // Station-keeping thruster pods at the truss tips.
        block(
            out,
            Vec3::new(30.8 * side, 6.0, 0.0),
            Vec3::splat(0.6),
            Quat::IDENTITY,
            Material::Mli,
        );
        for d in [Vec3::Y, -Vec3::Y, Vec3::Z] {
            let t = Mat4::from_rotation_translation(
                Quat::from_rotation_arc(Vec3::Z, d),
                Vec3::new(30.8 * side, 6.0, 0.0) + d * 0.7,
            );
            frustum(out, &t, 0.08, 0.16, 0.25, 12, Material::Dark, false);
        }
    }
    // Habitat wrapped in beta cloth, with insulation bands; node and airlock.
    let hab = Mat4::from_translation(Vec3::new(0.0, 6.0, 8.0));
    frustum(out, &hab, 2.1, 2.1, 12.0, 32, Material::Fabric, true);
    for z in [3.0, 8.0, 13.0] {
        band(
            out,
            Vec3::new(0.0, 6.0, z),
            Vec3::Z,
            2.14,
            0.5,
            Material::Mli,
        );
    }
    block(
        out,
        Vec3::new(0.0, 6.0, 15.5),
        Vec3::new(1.6, 1.6, 1.5),
        Quat::IDENTITY,
        paint,
    );
    // Hatch frame in safety yellow.
    for (a, b) in [
        ((-0.7, -0.7), (0.7, -0.7)),
        ((0.7, -0.7), (0.7, 0.7)),
        ((0.7, 0.7), (-0.7, 0.7)),
        ((-0.7, 0.7), (-0.7, -0.7)),
    ] {
        let p = |x: f32, y: f32| Vec3::new(x, 6.0 + y, 17.04);
        member(out, p(a.0, a.1), p(b.0, b.1), 0.08, safety);
    }
    block(
        out,
        Vec3::new(0.0, 6.0, 17.01),
        Vec3::new(0.66, 0.66, 0.02),
        Quat::IDENTITY,
        Material::Aluminium,
    );
    // Robotic arm on the truss.
    let shoulder = Vec3::new(-6.0, 6.8, 0.0);
    let elbow = Vec3::new(-6.0, 12.5, 4.5);
    let wrist = Vec3::new(-2.5, 9.5, 8.0);
    arm_boom(out, shoulder, elbow, 0.35);
    arm_boom(out, elbow, wrist, 0.3);
    block(out, wrist, Vec3::splat(0.35), Quat::IDENTITY, aluminium);
    block(out, elbow, Vec3::splat(0.3), Quat::IDENTITY, Material::Mli);
    // Keel jig: an open frame below the truss where the ship is built.
    let jig = station::JIG.as_vec3();
    for (x, y) in [(-3.5, -3.5), (3.5, -3.5), (3.5, 3.5), (-3.5, 3.5)] {
        member(
            out,
            jig + Vec3::new(x, y, -12.5),
            jig + Vec3::new(x, y, 9.5),
            0.14,
            safety,
        );
    }
    for i in 0..=7 {
        let z = -12.5 + i as f32 * 22.0 / 7.0;
        let c = |x: f32, y: f32| jig + Vec3::new(x, y, z);
        for (a, b) in [
            ((-3.5, -3.5), (3.5, -3.5)),
            ((3.5, -3.5), (3.5, 3.5)),
            ((3.5, 3.5), (-3.5, 3.5)),
            ((-3.5, 3.5), (-3.5, -3.5)),
        ] {
            member(out, c(a.0, a.1), c(b.0, b.1), 0.1, safety);
        }
    }
    for x in [-3.5, 3.5] {
        member(
            out,
            Vec3::new(x, 5.3, 0.0),
            jig + Vec3::new(x, 3.5, 0.0),
            0.16,
            aluminium,
        );
    }
    // Parts depot backboard and rack arms.
    block(
        out,
        Vec3::new(-15.3, -6.0, 1.25),
        Vec3::new(0.3, 3.0, 7.75),
        Quat::IDENTITY,
        aluminium,
    );
    for kind in PartKind::ALL {
        let at = kind.stowage().as_vec3();
        member(
            out,
            Vec3::new(-15.0, at.y, at.z),
            at - Vec3::X * 1.1,
            0.08,
            safety,
        );
    }
    member(
        out,
        Vec3::new(-15.0, -6.0, 1.0),
        Vec3::new(-3.5, -6.0, 1.0),
        0.12,
        aluminium,
    );
}

/// One solar array wing: a thin blanket of cells in a frame, facing the Sun.
fn solar_wing(out: &mut Vec<LitVertex>, side: f32) {
    let (a, b) = (13.0 * side, 30.0 * side);
    let (x0, x1) = (a.min(b), a.max(b));
    let (y0, y1) = (-0.5, 12.5);
    let z = -1.1;
    let center = Vec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0, z + 0.02);
    let half = Vec3::new((x1 - x0) / 2.0, (y1 - y0) / 2.0, 0.02);
    let back = Surface::from(Material::WhitePaint);
    let frame = Surface::from(Material::Aluminium);
    cuboid_faces(
        out,
        &Mat4::from_translation(center),
        half,
        [
            frame,
            frame,
            frame,
            frame,
            back,
            Surface::from(Material::Dark),
        ],
    );
    // Cells: 17 × 13 panels, each inset with a thin metal gap.
    let (nx, ny) = (17, 13);
    let (cw, ch) = ((x1 - x0) / nx as f32, (y1 - y0) / ny as f32);
    for i in 0..nx {
        for j in 0..ny {
            let cx = x0 + (i as f32 + 0.5) * cw;
            let cy = y0 + (j as f32 + 0.5) * ch;
            // Cells vary slightly in their coating, as real strings do.
            let h = ((i * 7 + j * 13) % 5) as f32 / 5.0;
            let color = [0.03 + h * 0.006, 0.04 + h * 0.008, 0.10 + h * 0.02];
            let q = [
                Vec3::new(cx - cw * 0.46, cy - ch * 0.46, z - 0.001),
                Vec3::new(cx + cw * 0.46, cy - ch * 0.46, z - 0.001),
                Vec3::new(cx + cw * 0.46, cy + ch * 0.46, z - 0.001),
                Vec3::new(cx - cw * 0.46, cy + ch * 0.46, z - 0.001),
            ];
            draw::quad(
                out,
                &Mat4::IDENTITY,
                q,
                -Vec3::Z,
                Vec3::X,
                Surface::tinted(Material::SolarCell, color),
            );
        }
    }
    // The mast that holds the wing to the truss.
    member(
        out,
        Vec3::new(a, 6.0, 0.0),
        Vec3::new(a, 6.0, -1.08),
        0.2,
        frame,
    );
}

fn arm_boom(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, radius: f32) {
    let d = b - a;
    let t = Mat4::from_rotation_translation(
        Quat::from_rotation_arc(Vec3::Z, d.normalize()),
        (a + b) / 2.0,
    );
    frustum(
        out,
        &t,
        radius,
        radius,
        d.length(),
        16,
        Material::WhitePaint,
        true,
    );
}

/// Lit geometry for one part in its own frame, with baked self-occlusion.
fn part_geometry(kind: PartKind) -> &'static [LitVertex] {
    static PARTS: OnceLock<Vec<Vec<LitVertex>>> = OnceLock::new();
    let all = PARTS.get_or_init(|| {
        PartKind::ALL
            .iter()
            .map(|&kind| {
                let mut out = Vec::new();
                build_part(&mut out, kind);
                let bvh = bake::Bvh::new(&out);
                bake::bake_occlusion(&mut out, &bvh, 1.0, 16);
                out
            })
            .collect()
    });
    let index = PartKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
    &all[index]
}

fn build_part(out: &mut Vec<LitVertex>, kind: PartKind) {
    let half = kind.size().as_vec3() * 0.5;
    let id = Mat4::IDENTITY;
    let aluminium = Surface::from(Material::Aluminium);
    match kind {
        PartKind::MainEngine => {
            // Thrust structure, then a regeneratively cooled copper-alloy bell.
            cuboid(
                out,
                &Mat4::from_translation(Vec3::Z * 1.0),
                Vec3::new(0.8, 0.8, 0.5),
                Material::Mli,
            );
            frustum(
                out,
                &Mat4::from_translation(Vec3::Z * 0.35),
                0.3,
                0.35,
                0.3,
                16,
                aluminium,
                false,
            );
            let copper = Surface {
                material: Material::Aluminium,
                color: Some([0.95, 0.64, 0.54]),
                roughness: Some(0.45),
            };
            frustum(
                out,
                &Mat4::from_translation(Vec3::Z * -0.4),
                1.05,
                0.4,
                2.2,
                32,
                copper,
                false,
            );
        }
        PartKind::PropellantTank => {
            frustum(out, &id, 1.3, 1.3, 3.2, 32, Material::Mli, false);
            frustum(
                out,
                &Mat4::from_translation(Vec3::Z * 1.8),
                1.3,
                0.5,
                0.4,
                32,
                Material::Mli,
                true,
            );
            frustum(
                out,
                &Mat4::from_translation(Vec3::Z * -1.8),
                0.5,
                1.3,
                0.4,
                32,
                Material::Mli,
                true,
            );
            for z in [-1.0, 1.0] {
                frustum(
                    out,
                    &Mat4::from_translation(Vec3::Z * z * 1.4),
                    1.32,
                    1.32,
                    0.08,
                    32,
                    aluminium,
                    false,
                );
            }
        }
        PartKind::KeelTrussAft | PartKind::KeelTrussFore => {
            let t = |p: Vec3| p;
            for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                cuboid(
                    out,
                    &Mat4::from_translation(Vec3::new(
                        x * (half.x - 0.06),
                        y * (half.y - 0.06),
                        0.0,
                    )),
                    Vec3::new(0.06, 0.06, half.z),
                    aluminium,
                );
            }
            for i in 0..4 {
                let z0 = -half.z + i as f32 * half.z / 2.0;
                let z1 = z0 + half.z / 2.0;
                let p = |x: f32, y: f32, z: f32| {
                    t(Vec3::new(x * (half.x - 0.06), y * (half.y - 0.06), z))
                };
                member(out, p(1.0, 1.0, z0), p(-1.0, 1.0, z1), 0.04, aluminium);
                member(out, p(1.0, -1.0, z0), p(-1.0, -1.0, z1), 0.04, aluminium);
                member(out, p(1.0, 1.0, z0), p(1.0, -1.0, z1), 0.04, aluminium);
                member(out, p(-1.0, 1.0, z0), p(-1.0, -1.0, z1), 0.04, aluminium);
            }
        }
        PartKind::RcsPod => {
            cuboid(out, &id, Vec3::new(0.5, 0.4, 0.5), Material::WhitePaint);
            for x in [-1.0, 1.0] {
                cuboid(
                    out,
                    &Mat4::from_translation(Vec3::new(x * 0.9, 0.0, 0.0)),
                    Vec3::new(0.3, 0.25, 0.25),
                    aluminium,
                );
                for d in [Vec3::Y, -Vec3::Y, Vec3::Z] {
                    let t = Mat4::from_rotation_translation(
                        Quat::from_rotation_arc(Vec3::Z, d),
                        Vec3::new(x * 0.9, 0.0, 0.0) + d * 0.3,
                    );
                    frustum(out, &t, 0.04, 0.09, 0.14, 10, Material::Dark, false);
                }
            }
        }
        PartKind::AvionicsBay => {
            cuboid(out, &id, half * 0.95, Material::Mli);
            // A silvered-Teflon radiator plate on top.
            cuboid(
                out,
                &Mat4::from_translation(Vec3::Y * 0.7),
                Vec3::new(0.6, 0.02, 0.6),
                Material::Radiator,
            );
        }
    }
}

/// Transforms part-local geometry into the world.
fn place(out: &mut Vec<LitVertex>, geometry: &[LitVertex], t: &Mat4) {
    out.extend(geometry.iter().map(|v| LitVertex {
        pos: t.transform_point3(Vec3::from(v.pos)).to_array(),
        normal: t.transform_vector3(Vec3::from(v.normal)).to_array(),
        tangent: t.transform_vector3(Vec3::from(v.tangent)).to_array(),
        ..*v
    }));
}

fn part_mesh(out: &mut Vec<LitVertex>, kind: PartKind, t: &Mat4) {
    place(out, part_geometry(kind), t);
}

/// Parts resting in the rack or latched in the jig, for the light bake.
fn resting_parts(s: &Station) -> Vec<LitVertex> {
    let mut out = Vec::new();
    for part in &s.parts {
        if matches!(part.state, PartState::Stowed | PartState::Installed) {
            let body = s.body(part);
            let t = Mat4::from_rotation_translation(body.orientation.as_quat(), body.pos.as_vec3());
            part_mesh(&mut out, part.kind, &t);
        }
    }
    out
}

/// The suit in its own frame for both arm poses, with baked occlusion.
fn suit_geometry(carrying: bool) -> &'static [LitVertex] {
    static SUIT: OnceLock<[Vec<LitVertex>; 2]> = OnceLock::new();
    let both = SUIT.get_or_init(|| {
        [false, true].map(|carrying| {
            let mut out = Vec::new();
            build_suit(&mut out, carrying);
            let bvh = bake::Bvh::new(&out);
            bake::bake_occlusion(&mut out, &bvh, 0.5, 16);
            out
        })
    });
    &both[usize::from(carrying)]
}

fn build_suit(out: &mut Vec<LitVertex>, carrying: bool) {
    let fabric = Surface::from(Material::Fabric);
    let part = |out: &mut Vec<LitVertex>, center: Vec3, half: Vec3, s: Surface| {
        cuboid(out, &Mat4::from_translation(center), half, s);
    };
    for x in [-0.14, 0.14] {
        part(
            out,
            Vec3::new(x, 0.42, 0.0),
            Vec3::new(0.11, 0.42, 0.12),
            fabric,
        );
        // Boots.
        part(
            out,
            Vec3::new(x, 0.06, 0.03),
            Vec3::new(0.12, 0.07, 0.16),
            Surface::from(Material::WhitePaint),
        );
    }
    part(
        out,
        Vec3::new(0.0, 1.12, 0.0),
        Vec3::new(0.3, 0.33, 0.18),
        fabric,
    );
    // Hard upper torso and helmet shell.
    part(
        out,
        Vec3::new(0.0, 1.3, 0.0),
        Vec3::new(0.26, 0.16, 0.19),
        Surface::from(Material::WhitePaint),
    );
    let helmet = Mat4::from_translation(Vec3::new(0.0, 1.62, 0.02));
    cuboid(out, &helmet, Vec3::splat(0.17), Material::WhitePaint);
    // Gold sun visor across the face.
    part(
        out,
        Vec3::new(0.0, 1.62, 0.18),
        Vec3::new(0.14, 0.11, 0.02),
        Surface::from(Material::Visor),
    );
    // Maneuvering pack with its nitrogen tanks and thruster blocks.
    part(
        out,
        Vec3::new(0.0, 1.18, -0.36),
        Vec3::new(0.34, 0.42, 0.17),
        Surface::from(Material::WhitePaint),
    );
    for x in [-0.4, 0.4] {
        let t = Mat4::from_translation(Vec3::new(x, 1.35, -0.3));
        frustum(out, &t, 0.06, 0.06, 0.4, 12, Material::Aluminium, true);
        for y in [0.85, 1.55] {
            part(
                out,
                Vec3::new(x * 0.95, y, -0.36),
                Vec3::splat(0.05),
                Surface::from(Material::Dark),
            );
        }
    }
    for x in [-0.4, 0.4] {
        let (center, half) = if carrying {
            (Vec3::new(x, 1.2, 0.35), Vec3::new(0.08, 0.08, 0.36))
        } else {
            (Vec3::new(x, 1.0, 0.02), Vec3::new(0.08, 0.32, 0.08))
        };
        part(out, center, half, fabric);
        // Gloves.
        let glove = if carrying {
            center + Vec3::Z * (half.z + 0.05)
        } else {
            center - Vec3::Y * (half.y + 0.05)
        };
        part(
            out,
            glove,
            Vec3::splat(0.06),
            Surface::from(Material::WhitePaint),
        );
    }
}

fn suit(out: &mut Vec<LitVertex>, feet: Vec3, yaw: f32, carrying: bool) {
    let t = Mat4::from_translation(feet) * Mat4::from_rotation_y(yaw);
    place(out, suit_geometry(carrying), &t);
}

/// Cold-gas plumes are nearly invisible: nitrogen leaves at about 690 m/s.
/// What a camera sees is a brief sparkle of sunlit condensate at the start of
/// each pulse, and only when the nozzle is in sunlight. Every speck is a
/// closed-form function of its pulse's seed and the rendered instant, so a
/// replay draws the same ones with no stored particle state.
fn plume_glows(
    out: &mut Vec<GlowVertex>,
    s: &Station,
    sun_dir: Vec3,
    sunlit: &dyn Fn(Vec3) -> bool,
) {
    let bright = crate::pbr::SUN_ILLUMINANCE * 0.8 / std::f32::consts::PI;
    let now = (s.world.tick as f64 - 1.0 + s.alpha()) * station::PHYSICS_DT;
    for pulse in s.plume_pulses() {
        let age = (now - pulse.tick as f64 * station::PHYSICS_DT) as f32;
        let station_keeping = pulse.thruster >= 24;
        let life = if station_keeping { 1.2 } else { 0.25 };
        if !(0.0..life).contains(&age) {
            continue;
        }
        let pos = pulse.pos.as_vec3();
        if !sunlit(pos) {
            continue;
        }
        let dir = pulse.dir.as_vec3();
        let (u, v) = basis(dir);
        let count = if station_keeping { 24 } else { 3 };
        for i in 0..count {
            let h = seeded(pulse.seed, i);
            // Most pulses shed nothing visible; a few flakes glint.
            if !station_keeping && h[0] > 0.5 {
                continue;
            }
            let spread = Vec3::new(h[1] - 0.5, h[2] - 0.5, 0.0) * 0.4;
            let heading = (dir + u * spread.x + v * spread.y).normalize();
            let speed = 4.0 + h[3] * 30.0;
            let at = pos + heading * (0.05 + speed * age);
            let fade = (1.0 - age / life).max(0.0);
            speck(out, at, 0.006, [bright * fade; 3], sun_dir);
        }
    }
}

/// Ice flakes from the habitat's water vent: a burst every 40 s whose flakes
/// drift outward, pushed anti-sunward by solar radiation pressure (about
/// 10⁻⁴ m/s² for a 100 µm flake), glinting as they tumble and sublimating
/// after a minute and a half. Positions follow from the physics tick alone.
fn ice_flakes(
    out: &mut Vec<GlowVertex>,
    s: &Station,
    sun_dir: Vec3,
    sunlit: &dyn Fn(Vec3) -> bool,
) {
    const PERIOD: f64 = 40.0;
    const LIFE: f64 = 90.0;
    const VENT: Vec3 = Vec3::new(1.9, 7.2, 12.0);
    let now = (s.world.tick as f64 - 1.0 + s.alpha()) * station::PHYSICS_DT;
    let bright = crate::pbr::SUN_ILLUMINANCE * 0.6 / std::f32::consts::PI;
    let pressure = -sun_dir * 1.0e-4;
    let latest = (now / PERIOD).floor() as i64;
    for burst in (latest - (LIFE / PERIOD).ceil() as i64)..=latest {
        if burst < 0 {
            continue;
        }
        let age = (now - burst as f64 * PERIOD) as f32;
        if !(0.0..LIFE as f32).contains(&age) {
            continue;
        }
        for i in 0..14 {
            let h = seeded(0x1ce_f1a4e ^ burst as u64, i);
            let heading = Vec3::new(0.6 + h[0], h[1] - 0.5, h[2] - 0.5).normalize();
            let speed = 0.05 + h[3] * 0.25;
            let at = VENT + heading * speed * age + pressure * (0.5 * age * age);
            if !sunlit(at) {
                continue;
            }
            // Tumbling flakes flash when a face lines up with the Sun.
            let phase = h[1] * 40.0 + age * (1.0 + h[2] * 3.0);
            let glint = 0.15 + 0.85 * phase.sin().abs().powi(12);
            let fade = 1.0 - (age / LIFE as f32).powi(2);
            speck(out, at, 0.004, [bright * glint * fade; 3], sun_dir);
        }
    }
}

/// Four deterministic values in [0, 1) from a seed and an index.
fn seeded(seed: u64, i: u32) -> [f32; 4] {
    let mut x = seed ^ (u64::from(i).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let mut out = [0.0; 4];
    for o in &mut out {
        // SplitMix64.
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        *o = (z >> 40) as f32 / (1u64 << 24) as f32;
    }
    out
}

/// A camera-facing speck (the sun direction stands in for the view, since
/// specks are only drawn in sunlight and are nearly isotropic).
fn speck(out: &mut Vec<GlowVertex>, at: Vec3, size: f32, radiance: [f32; 3], facing: Vec3) {
    let (u, v) = basis(facing);
    let corners = [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ];
    for (x, y) in corners {
        out.push(GlowVertex {
            pos: (at + (u * x + v * y) * size).to_array(),
            radiance,
            uv: [x, y],
        });
    }
}

fn feet(center: DVec3) -> Vec3 {
    center.as_vec3() - Vec3::Y * 0.9
}

fn vertex(pos: Vec3, color: [f32; 3], fog: f32) -> Vertex {
    Vertex {
        pos: pos.to_array(),
        color,
        fog,
    }
}

fn line(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    mesh.lines.push(vertex(a, color, 0.0));
    mesh.lines.push(vertex(b, color, 0.0));
}

fn dashed(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    let steps = ((b - a).length() / 0.5).clamp(1.0, 400.0) as usize;
    for i in (0..steps).step_by(2) {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        line(mesh, a.lerp(b, t0), a.lerp(b, t1), color);
    }
}

fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.normalize();
    let helper = if a.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = a.cross(helper).normalize();
    (u, a.cross(u))
}

fn ring(mesh: &mut Mesh, center: Vec3, axis: Vec3, radius: f32, color: [f32; 3]) {
    let (u, v) = basis(axis);
    let n = 32;
    for i in 0..n {
        let p = |i: usize| {
            let t = i as f32 / n as f32 * std::f32::consts::TAU;
            center + (u * t.cos() + v * t.sin()) * radius
        };
        line(mesh, p(i), p(i + 1), color);
    }
}

fn outline(mesh: &mut Mesh, center: Vec3, half: Vec3, color: [f32; 3]) {
    let c = |x: f32, y: f32, z: f32| center + Vec3::new(x, y, z) * half;
    for (p, q) in [
        ((-1., -1., -1.), (1., -1., -1.)),
        ((1., -1., -1.), (1., 1., -1.)),
        ((1., 1., -1.), (-1., 1., -1.)),
        ((-1., 1., -1.), (-1., -1., -1.)),
        ((-1., -1., 1.), (1., -1., 1.)),
        ((1., -1., 1.), (1., 1., 1.)),
        ((1., 1., 1.), (-1., 1., 1.)),
        ((-1., 1., 1.), (-1., -1., 1.)),
        ((-1., -1., -1.), (-1., -1., 1.)),
        ((1., -1., -1.), (1., -1., 1.)),
        ((1., 1., -1.), (1., 1., 1.)),
        ((-1., 1., -1.), (-1., 1., 1.)),
    ] {
        line(mesh, c(p.0, p.1, p.2), c(q.0, q.1, q.2), color);
    }
}

#[cfg(test)]
mod tests {
    /// The solids the lines wrap around follow the drawn structure: every
    /// vertex of the station and its wings lies within a centimeter of one.
    #[test]
    fn the_lines_wrap_the_structure_as_it_is_drawn() {
        let solids = verse_lagrange::station::structure_solids();
        let mut drawn = Vec::new();
        super::structure_vertices(&mut drawn);
        assert!(drawn.len() > 1_000);
        for vertex in drawn {
            let p = glam::DVec3::from(vertex.map(f64::from));
            let nearest = solids
                .iter()
                .map(|solid| solid.distance(p).0)
                .fold(f64::INFINITY, f64::min);
            assert!(nearest < 0.01, "{p} is {nearest} m from every solid");
        }
    }
}
