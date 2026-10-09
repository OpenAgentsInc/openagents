//! The Grid as an engine content pack: the ground lattice, the Gym, the
//! portal arches, the line-figure avatar, and the Grid robot, compiled once from the same
//! generators the legacy line pass draws from and pinned under
//! `assets/verse/grid/`. The engine renderer draws the pack through
//! [`verse_engine::render_world::RenderWorld`]; nothing here reaches
//! `render.rs`.
//!
//! Regenerate the pinned pack after changing a generator with
//! `cargo run -p verse --example grid_pack`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};
use sha2::{Digest, Sha256};
use verse_engine::animation_graph::Authored;
use verse_engine::assets::{Bone, BoneKeys, Clip, Model, Pack, Placement, Surface, Topology};
use verse_engine::inventory::{Asset, Binding, Inventory, License, Purpose};
use verse_engine::markers::{ClipTrack, FOOTSTEP_LEFT, FOOTSTEP_RIGHT, Marker, Track};
use verse_engine::motion::{Binding as MotionBinding, Mode, State};

use crate::imported::{flat, inventory};
use crate::mesh::Mesh;
use crate::palette;
use crate::zones::{ZoneId, gate};

/// The pinned pack's directory, relative to the repository root.
pub const PINNED: &str = "assets/verse/grid";
/// The pack manifest's file name inside its directory.
pub const MANIFEST: &str = "pack.json";

/// The ground lattice.
pub const FLOOR: &str = "grid/floor";
/// The Gym hall at its Grid site, with its signs and boards.
pub const GYM: &str = "grid/gym";
/// The Everglade arch, at the origin facing +Z.
pub const ARCH_EVERGLADE: &str = "grid/arch-everglade";
/// The Lagrange 1 arch, at the origin facing +Z.
pub const ARCH_LAGRANGE: &str = "grid/arch-lagrange-1";
/// The RITUAL arch, at the origin facing +Z.
pub const ARCH_RITUAL: &str = "grid/arch-ritual";
/// The line-figure avatar, feet at the origin facing +Z.
pub const FIGURE: &str = "grid/figure";
/// The Gym's RESULTS, EVALS, and board lettering at the Grid site.
pub const BOARDS: &str = "grid/boards";
/// A companion agent's spade, at the origin facing +Z.
pub const SPADE: &str = "grid/spade";

/// The figure's walk clip is one leg-swing cycle, which the legacy gait
/// advances by distance: this many meters of travel play the clip once.
pub const WALK_CYCLE_METERS: f32 = 1.0 / crate::avatar::STRIDE;

const CLIP_IDLE: u16 = 0;
const CLIP_WALK: u16 = 2;
const CLIP_RUN: u16 = 3;
const CLIP_AIRBORNE: u16 = 4;
/// The legacy figure's full-swing amplitude, radians.
const SWING: f32 = 0.75;

/// The figure's bones. Every limb hangs from the root so only it rotates,
/// as the legacy figure's limbs do.
#[derive(Clone, Copy)]
enum Joint {
    Root,
    LegLeft,
    LegRight,
    Torso,
    ArmLeft,
    ArmRight,
    Head,
}

impl Joint {
    const ALL: [Self; 7] = [
        Self::Root,
        Self::LegLeft,
        Self::LegRight,
        Self::Torso,
        Self::ArmLeft,
        Self::ArmRight,
        Self::Head,
    ];

    fn pivot(self) -> Vec3 {
        match self {
            Self::Root => Vec3::ZERO,
            Self::LegLeft => Vec3::new(0.14, 0.92, 0.0),
            Self::LegRight => Vec3::new(-0.14, 0.92, 0.0),
            Self::Torso => Vec3::new(0.0, 1.27, 0.0),
            Self::ArmLeft => Vec3::new(0.36, 1.58, 0.0),
            Self::ArmRight => Vec3::new(-0.36, 1.58, 0.0),
            Self::Head => Vec3::new(0.0, 1.78, 0.0),
        }
    }

    /// The limb's swing sign against the shared swing angle.
    fn swing(self) -> f32 {
        match self {
            Self::LegLeft | Self::ArmRight => 1.0,
            Self::LegRight | Self::ArmLeft => -1.0,
            Self::Root | Self::Torso | Self::Head => 0.0,
        }
    }
}

/// The directory the pinned pack lives in, found from this crate.
#[must_use]
pub fn pinned_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PINNED)
}

/// Reads and validates the pinned pack.
pub fn load_pinned() -> Result<Pack, String> {
    Pack::read(&pinned_dir().join(MANIFEST))
}

/// The pinned manifest, built into the binary for hosts without the
/// repository: phones and browsers.
const EMBEDDED_MANIFEST: &str = include_str!("../../../assets/verse/grid/pack.json");
/// The pinned pack's one texture file, the white texel.
const EMBEDDED_WHITE: (&str, &[u8]) = (
    "verse-flat-white.png",
    include_bytes!("../../../assets/verse/grid/verse-flat-white.png"),
);

/// Validates the pinned pack built into the binary.
pub fn embedded() -> Result<Pack, String> {
    let pack: Pack = serde_json::from_str(EMBEDDED_MANIFEST).map_err(|e| e.to_string())?;
    pack.validate()?;
    Ok(pack)
}

/// Admits the built-in pack for a renderer, with no file system: the same
/// digests and budgets the directory load checks.
pub fn prepare_embedded() -> Result<verse_engine::loading::Prepared, String> {
    verse_engine::loading::Prepared::from_bytes(embedded()?, &[EMBEDDED_WHITE], Default::default())
}

/// Compiles the Grid into `dir`: the white texel, every model, the floor
/// placement, and the manifest. Arches are placed per frame by
/// [`gates`], because the runtime decides which portals stand. Returns the admitted pack.
pub fn compile(dir: &Path) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut pack = Pack {
        inventory: None,
        version: 1,
        source_revision: String::new(),
        models: BTreeMap::new(),
        textures: vec![],
        placements: vec![],
    };
    let white = flat::white_texture(&mut pack, dir)?;
    let mut floor = crate::world::ground_mesh();
    floor.neutralize();
    pack.models
        .insert(FLOOR.into(), mesh_model(FLOOR, &floor, white, 0.0));
    let mut gym = crate::world::gym_mesh(crate::world::GymSite::GRID, true);
    gym.neutralize();
    pack.models
        .insert(GYM.into(), mesh_model(GYM, &gym, white, 7.0));
    for (name, zone, sign) in [
        (ARCH_EVERGLADE, ZoneId::Plaza, ZoneId::Everglade.sign()),
        (ARCH_LAGRANGE, ZoneId::Plaza, ZoneId::Lagrange1.sign()),
        (ARCH_RITUAL, ZoneId::Plaza, gate::RITUAL_SIGN),
    ] {
        let mut arch = Mesh::default();
        crate::zones::arch(&mut arch, zone, sign, Vec3::ZERO, 0.0);
        arch.neutralize();
        pack.models
            .insert(name.into(), mesh_model(name, &arch, white, 5.2));
    }
    pack.models.insert(FIGURE.into(), figure(white));
    let mut boards = crate::world::gym_display(crate::world::GymSite::GRID, None);
    boards.extend(&crate::world::results_display(
        crate::world::GymSite::GRID,
        None,
    ));
    boards.extend(&crate::world::evals_display(
        crate::world::GymSite::GRID,
        None,
    ));
    boards.neutralize();
    pack.models
        .insert(BOARDS.into(), mesh_model(BOARDS, &boards, white, 7.0));
    let mut spade = crate::agent::spade(Mat4::IDENTITY, Intensity::Full);
    spade.neutralize();
    pack.models
        .insert(SPADE.into(), mesh_model(SPADE, &spade, white, 2.0));
    for (name, variant) in [
        (crate::grid_robot::ROBOT, "lod0"),
        (crate::grid_robot::ROBOT_FAR, "lod1"),
    ] {
        pack.models.insert(
            name.into(),
            crate::grid_robot::model(dir, white, variant, name)?,
        );
    }
    let workstation = crate::grid_workstation::mesh()?;
    pack.models.insert(
        crate::grid_workstation::MODEL.into(),
        mesh_model(crate::grid_workstation::MODEL, &workstation, white, 1.7),
    );
    for at in crate::grid_workstation::SITES {
        pack.placements.push(Placement {
            model: crate::grid_workstation::MODEL.into(),
            position: at,
            rotation: Quat::IDENTITY.to_array(),
            scale: crate::grid_workstation::SCALE,
        });
    }
    for model in [FLOOR] {
        pack.placements.push(Placement {
            model: model.into(),
            position: [0.0; 3],
            rotation: Quat::IDENTITY.to_array(),
            scale: 1.0,
        });
    }
    for model in pack.models.values_mut() {
        model.source_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(model).map_err(|e| e.to_string())?)
        );
    }
    admit(&mut pack, dir)?;
    pack.validate()?;
    std::fs::write(
        dir.join(MANIFEST),
        serde_json::to_vec(&pack).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(pack)
}

/// The pack's placements as engine instances, for a renderer's static set.
#[must_use]
pub fn placements(pack: &Pack) -> Vec<verse_engine::presentation::Instance> {
    pack.placements
        .iter()
        .map(|p| verse_engine::presentation::Instance {
            mount: None,
            actor: None,
            model: p.model.clone(),
            transform: Mat4::from_scale_rotation_translation(
                Vec3::splat(p.scale),
                Quat::from_array(p.rotation),
                Vec3::from_array(p.position),
            ),
            animation: 0.into(),
            time: 0.,
            animation_epoch: None,
            emission: Vec3::ZERO,
        })
        .collect()
}

/// Collision footprints for placed workstations and any placed Gym walls,
/// carried through each placement's transform.
#[must_use]
pub fn blockers(pack: &Pack) -> Vec<crate::controller::Footprint> {
    let snap = |v: f32| (v * 1000.0).round() / 1000.0;
    pack.placements
        .iter()
        .filter(|p| p.model == GYM || p.model == crate::grid_workstation::MODEL)
        .flat_map(|p| {
            let transform = Mat4::from_scale_rotation_translation(
                Vec3::splat(p.scale),
                Quat::from_array(p.rotation),
                Vec3::from_array(p.position),
            );
            let walls = if p.model == GYM {
                crate::world::GymSite::GRID.walls().to_vec()
            } else {
                let points = pack.models[crate::grid_workstation::MODEL]
                    .surfaces
                    .iter()
                    .flat_map(|s| &s.vertices);
                let mut min = [f32::INFINITY; 2];
                let mut max = [f32::NEG_INFINITY; 2];
                for v in points {
                    for (axis, i) in [0, 2].into_iter().enumerate() {
                        min[axis] = min[axis].min(v.position[i]);
                        max[axis] = max[axis].max(v.position[i]);
                    }
                }
                vec![crate::controller::Footprint { min, max }]
            };
            walls.into_iter().map(move |wall| {
                let corners = [
                    [wall.min[0], wall.min[1]],
                    [wall.max[0], wall.min[1]],
                    [wall.max[0], wall.max[1]],
                    [wall.min[0], wall.max[1]],
                ]
                .map(|[x, z]| transform.transform_point3(Vec3::new(x, 0.0, z)));
                let fold = |f: fn(f32, f32) -> f32, pick: fn(&Vec3) -> f32| {
                    snap(corners.iter().map(pick).reduce(f).unwrap_or(0.0))
                };
                crate::controller::Footprint {
                    min: [fold(f32::min, |p| p.x), fold(f32::min, |p| p.z)],
                    max: [fold(f32::max, |p| p.x), fold(f32::max, |p| p.z)],
                }
            })
        })
        .collect()
}

/// The arches the runtime currently stands on the Grid, as engine instances.
#[must_use]
pub fn gates(runtime: &crate::runtime::WorldRuntime) -> Vec<verse_engine::presentation::Instance> {
    [
        (ARCH_EVERGLADE, runtime.everglade_gate()),
        (ARCH_LAGRANGE, runtime.grid_gate()),
        (ARCH_RITUAL, runtime.ritual_gate()),
    ]
    .into_iter()
    .filter_map(|(model, gate)| {
        let gate = gate?;
        Some(verse_engine::presentation::Instance {
            mount: None,
            actor: None,
            model: model.into(),
            transform: Mat4::from_rotation_translation(Quat::from_rotation_y(gate.yaw), gate.at),
            animation: 0.into(),
            time: 0.,
            animation_epoch: None,
            emission: Vec3::ZERO,
        })
    })
    .collect()
}

/// Records the pack's provenance: this compiler's sources, the white texel,
/// and every model, then admits the pack for local use.
fn admit(pack: &mut Pack, dir: &Path) -> Result<(), String> {
    let project = inventory::id("verse:source:project")?;
    let (revision, bytes) = inventory::bundle(&[
        include_bytes!("grid_pack.rs"),
        include_bytes!("grid_robot.rs"),
        include_bytes!("grid_workstation.rs"),
        include_bytes!("../../verse-zone-everglade/src/zones/everglade/boards.rs"),
        include_bytes!("world.rs"),
        include_bytes!("../../verse-core/src/world.rs"),
        include_bytes!("../../verse-core/src/avatar.rs"),
        include_bytes!("../../verse-core/src/agent.rs"),
        include_bytes!("../../verse-pbr/src/mesh.rs"),
        include_bytes!("../../verse-gfx/src/palette.rs"),
        include_bytes!("zones/mod.rs"),
        include_bytes!("zones/gate.rs"),
        include_bytes!("doors/mesh.rs"),
        include_bytes!("../../verse-core/src/label.rs"),
        include_bytes!("../../verse-pbr/src/imported/flat.rs"),
    ]);
    let mut assets = vec![inventory::source(
        project.as_str(),
        "OpenAgents contributors",
        License::Apache2,
        "rust",
        revision.clone(),
        bytes,
    )?];
    // The Grid robot's admitted glTF sources and the clips it plays.
    let robot = inventory::id("verse:source:grid-robot")?;
    let (digest, size) = crate::grid_robot::sources()?;
    assets.push(inventory::source(
        robot.as_str(),
        "OpenAgents; rig and clips by Quaternius",
        License::Cc0,
        "gltf",
        digest,
        size,
    )?);
    let workstation = inventory::id("verse:source:grid-workstation")?;
    let (digest, size) = crate::grid_workstation::sources()?;
    assets.push(inventory::source(
        workstation.as_str(),
        "Quaternius",
        License::Cc0,
        "gltf",
        digest,
        size,
    )?);
    let mut texture_ids = BTreeMap::new();
    for (slot, texture) in pack.textures.iter().enumerate() {
        let id = inventory::id(&format!("verse:texture:grid/{slot}"))?;
        texture_ids.insert(slot, id.clone());
        assets.push(Asset {
            id,
            binding: Binding::Texture { slot },
            sha256: texture.sha256.clone(),
            bytes: std::fs::metadata(dir.join(&texture.file))
                .map_err(|e| e.to_string())?
                .len(),
            dependencies: vec![project.clone()],
        });
    }
    for (name, model) in &pack.models {
        let mut dependencies = vec![project.clone()];
        if name.starts_with("grid/robot") {
            dependencies.push(robot.clone());
        }
        if name == crate::grid_workstation::MODEL {
            dependencies.push(workstation.clone());
        }
        for slot in model.surfaces.iter().flat_map(|s| s.texture_slots()) {
            if !dependencies.contains(&texture_ids[&slot]) {
                dependencies.push(texture_ids[&slot].clone());
            }
        }
        let (sha256, bytes) = verse_engine::inventory::fingerprint(model)?;
        assets.push(Asset {
            id: inventory::id(&format!("verse:model:{name}"))?,
            binding: Binding::Model { key: name.clone() },
            sha256,
            bytes,
            dependencies,
        });
    }
    let inventory = Inventory {
        version: 1,
        compiler: inventory::id("verse:compiler:grid")?,
        compiler_revision: revision.clone(),
        assets,
    };
    inventory.verify(pack)?;
    inventory.admit(pack, Purpose::OriginalLocal, &inventory.roots())?;
    pack.source_revision = revision;
    pack.inventory = Some(inventory);
    Ok(())
}

/// Converts a legacy mesh into flat engine surfaces, one lines surface per
/// line color and one unlit triangle surface per face color.
/// Longest line segment the pack stores, in meters. Long lines are split so
/// rasterizers keep segments whose ends lie far outside the viewport.
const SEGMENT: f32 = crate::world::LOT;

fn mesh_model(name: &str, mesh: &Mesh, white: usize, height: f32) -> Model {
    let mut surfaces = Vec::new();
    for (pairs, topology) in [
        (&mesh.lines, Topology::Lines),
        (&mesh.faces, Topology::Triangles),
    ] {
        let stride = topology.stride();
        let mut by_color: BTreeMap<[u32; 3], Surface> = BTreeMap::new();
        for primitive in pairs.chunks_exact(stride) {
            let color = primitive[0].color;
            let surface = by_color
                .entry(color.map(f32::to_bits))
                .or_insert_with(|| flat::surface(white, color, topology));
            if topology == Topology::Lines {
                let (a, b) = (
                    Vec3::from_array(primitive[0].pos),
                    Vec3::from_array(primitive[1].pos),
                );
                let pieces = (a.distance(b) / SEGMENT).ceil().max(1.0) as u32;
                for piece in 0..pieces {
                    let t0 = piece as f32 / pieces as f32;
                    let t1 = (piece + 1) as f32 / pieces as f32;
                    flat::line(surface, a.lerp(b, t0), a.lerp(b, t1));
                }
                continue;
            }
            let base = surface.vertices.len() as u32;
            for (i, v) in primitive.iter().enumerate() {
                surface
                    .vertices
                    .push(flat_vertex(Vec3::from_array(v.pos), 0));
                surface.indices.push(base + i as u32);
            }
        }
        surfaces.extend(by_color.into_values());
    }
    flat::model(name, surfaces, height)
}

fn flat_vertex(p: Vec3, joint: u32) -> verse_engine::assets::Vertex {
    verse_engine::assets::Vertex {
        position: p.to_array(),
        normal: [0., 1., 0.],
        uv: [0.5, 0.5],
        joints: [joint, 0, 0, 0],
        weights: [1., 0., 0., 0.],
    }
}

/// Appends a box's twelve edges to `edges` and six faces to `faces`, every
/// vertex bound to `joint`. `transform` maps the unit cube into the figure.
fn figure_box(edges: &mut Surface, faces: &mut Surface, transform: Mat4, joint: Joint) {
    let joint = joint as u32;
    let c: [Vec3; 8] = std::array::from_fn(|i| {
        transform.transform_point3(Vec3::new(
            if i & 1 == 0 { -0.5 } else { 0.5 },
            if i & 2 == 0 { -0.5 } else { 0.5 },
            if i & 4 == 0 { -0.5 } else { 0.5 },
        ))
    });
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                let base = edges.vertices.len() as u32;
                edges
                    .vertices
                    .extend([flat_vertex(c[i], joint), flat_vertex(c[i | bit], joint)]);
                edges.indices.extend([base, base + 1]);
            }
        }
    }
    for [a, b, d, e] in [
        [0, 1, 3, 2],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 3, 7, 5],
    ] {
        let base = faces.vertices.len() as u32;
        faces
            .vertices
            .extend([a, b, d, e].map(|i| flat_vertex(c[i], joint)));
        faces
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// The line-figure avatar: the legacy figure's boxes bound to swinging
/// limb bones, with idle, walk, run, and airborne clips and footstep
/// markers on the walk.
fn figure(white: usize) -> Model {
    let bright = palette::neutral(palette::amber(Intensity::Full));
    let dim = palette::neutral(palette::amber(crate::avatar::dim(Intensity::Full)));
    let mut edges = flat::surface(white, bright, Topology::Lines);
    let mut faces = flat::surface(
        white,
        palette::neutral(palette::field()),
        Topology::Triangles,
    );
    let mut marks = flat::surface(white, dim, Topology::Lines);

    let limb = |joint: Joint, size: Vec3| {
        Mat4::from_translation(joint.pivot())
            * Mat4::from_translation(Vec3::new(0.0, -size.y / 2.0, 0.0))
            * Mat4::from_scale(size)
    };
    let part =
        |joint: Joint, size: Vec3| Mat4::from_translation(joint.pivot()) * Mat4::from_scale(size);
    let leg = Vec3::new(0.2, 0.92, 0.22);
    let arm = Vec3::new(0.15, 0.68, 0.17);
    figure_box(
        &mut edges,
        &mut faces,
        limb(Joint::LegLeft, leg),
        Joint::LegLeft,
    );
    figure_box(
        &mut edges,
        &mut faces,
        limb(Joint::LegRight, leg),
        Joint::LegRight,
    );
    figure_box(
        &mut edges,
        &mut faces,
        part(Joint::Torso, Vec3::new(0.54, 0.7, 0.3)),
        Joint::Torso,
    );
    figure_box(
        &mut edges,
        &mut faces,
        limb(Joint::ArmLeft, arm),
        Joint::ArmLeft,
    );
    figure_box(
        &mut edges,
        &mut faces,
        limb(Joint::ArmRight, arm),
        Joint::ArmRight,
    );
    figure_box(
        &mut edges,
        &mut faces,
        part(Joint::Head, Vec3::new(0.28, 0.3, 0.28)),
        Joint::Head,
    );
    let segment = |surface: &mut Surface, a: Vec3, b: Vec3, joint: Joint| {
        let base = surface.vertices.len() as u32;
        surface
            .vertices
            .extend([flat_vertex(a, joint as u32), flat_vertex(b, joint as u32)]);
        surface.indices.extend([base, base + 1]);
    };
    segment(
        &mut edges,
        Vec3::new(-0.1, 1.8, 0.145),
        Vec3::new(0.1, 1.8, 0.145),
        Joint::Head,
    );
    let ground = Vec3::new(0.0, 0.02, 0.0);
    let ring: Vec<Vec3> = (0..32)
        .map(|i| {
            let a = i as f32 / 32.0 * std::f32::consts::TAU;
            ground + Vec3::new(a.cos() * 0.7, 0.0, a.sin() * 0.7)
        })
        .collect();
    for i in 0..ring.len() {
        segment(&mut marks, ring[i], ring[(i + 1) % ring.len()], Joint::Root);
    }
    segment(
        &mut marks,
        ground + Vec3::Z * 0.7,
        ground + Vec3::Z * 1.1,
        Joint::Root,
    );

    let swing_clip = |id: u16| Clip {
        id,
        duration: 1.0,
        bones: Joint::ALL
            .iter()
            .filter(|joint| joint.swing() != 0.0)
            .map(|joint| BoneKeys {
                bone: *joint as usize,
                translation: vec![],
                rotation: (0..=16)
                    .map(|i| {
                        let t = i as f32 / 16.0;
                        let angle = (t * std::f32::consts::TAU).sin() * SWING * joint.swing();
                        (t, Quat::from_rotation_x(angle).to_array())
                    })
                    .collect(),
                scale: vec![],
            })
            .collect(),
    };
    let still = |id: u16| Clip {
        id,
        duration: 1.0,
        bones: vec![],
    };
    let bind = |clip: u16| MotionBinding {
        clip,
        mode: Mode::Loop,
        transition_seconds: 0.2,
    };
    let mut model = Model {
        graph: None,
        markers: vec![ClipTrack {
            clip: CLIP_WALK,
            track: Track {
                duration: 1.0,
                markers: vec![
                    Marker {
                        id: FOOTSTEP_RIGHT,
                        seconds: 0.0,
                    },
                    Marker {
                        id: FOOTSTEP_LEFT,
                        seconds: 0.5,
                    },
                ],
            },
        }],
        states: BTreeMap::from([
            (State::Idle, bind(CLIP_IDLE)),
            (State::Walk, bind(CLIP_WALK)),
            (State::Run, bind(CLIP_RUN)),
            (State::Airborne, bind(CLIP_AIRBORNE)),
        ]),
        skin: None,
        source: FIGURE.into(),
        source_sha256: String::new(),
        surfaces: vec![edges, faces, marks],
        bones: Joint::ALL
            .iter()
            .map(|joint| Bone {
                parent: match joint {
                    Joint::Root => -1,
                    _ => Joint::Root as i16,
                },
                pivot: joint.pivot().to_array(),
            })
            .collect(),
        clips: vec![
            still(CLIP_IDLE),
            swing_clip(CLIP_WALK),
            swing_clip(CLIP_RUN),
            still(CLIP_AIRBORNE),
        ],
        height: 1.93,
        attachments: vec![],
    };
    model.graph = Some(Authored::from_bindings(&model));
    model
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_hides_the_gym_and_its_boards_without_leaving_walls() {
        let pack = embedded().unwrap();
        assert!(
            pack.placements
                .iter()
                .all(|p| p.model != GYM && p.model != BOARDS)
        );
        let empty = crate::world::GymSite::GRID.point(crate::world::GYM_CENTER);
        assert!(
            blockers(&pack)
                .iter()
                .all(|f| !f.contains(empty.x, empty.z, 0.0))
        );
        let mut moved = pack.clone();
        moved
            .placements
            .retain(|p| p.model != crate::grid_workstation::MODEL);
        moved.placements.push(Placement {
            model: GYM.into(),
            position: [10.0, 0.0, -4.0],
            rotation: Quat::IDENTITY.to_array(),
            scale: 1.0,
        });
        let walls = blockers(&moved);
        assert_eq!(walls.len(), 5);
        for (a, b) in walls.iter().zip(crate::world::GymSite::GRID.walls()) {
            assert!((a.min[0] - b.min[0] - 10.0).abs() < 1e-3);
            assert!((a.max[1] - b.max[1] + 4.0).abs() < 1e-3);
        }
    }

    #[test]
    fn the_pinned_pack_is_what_the_sources_compile_to() {
        let dir = tempfile::tempdir().unwrap();
        let pack = compile(dir.path()).unwrap();
        let compiled = std::fs::read(dir.path().join(MANIFEST)).unwrap();
        let pinned = std::fs::read(pinned_dir().join(MANIFEST)).unwrap();
        assert!(
            compiled == pinned,
            "assets/verse/grid is stale; run `cargo run -p verse --example grid_pack`"
        );
        let texel = std::fs::read(pinned_dir().join(flat::WHITE_TEXTURE)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&texel)),
            pack.textures[0].sha256
        );
    }

    #[test]
    fn the_pinned_pack_verifies_and_admits() {
        let pack = load_pinned().unwrap();
        let inventory = pack.inventory.as_ref().unwrap();
        inventory.verify(&pack).unwrap();
        inventory
            .admit(&pack, Purpose::OriginalLocal, &inventory.roots())
            .unwrap();
        for name in [
            FLOOR,
            GYM,
            ARCH_EVERGLADE,
            ARCH_LAGRANGE,
            ARCH_RITUAL,
            FIGURE,
            BOARDS,
            SPADE,
            crate::grid_robot::ROBOT,
            crate::grid_robot::ROBOT_FAR,
            crate::grid_workstation::MODEL,
        ] {
            assert!(pack.models.contains_key(name), "{name}");
        }
        assert_eq!(
            pack.placements.len(),
            1 + crate::grid_workstation::SITES.len()
        );
        verse_engine::loading::Prepared::load(pack, &pinned_dir(), Default::default()).unwrap();
    }

    #[test]
    fn the_built_in_pack_is_the_pinned_one() {
        let pinned = load_pinned().unwrap();
        let built_in = embedded().unwrap();
        assert_eq!(
            serde_json::to_string(&built_in).unwrap(),
            serde_json::to_string(&pinned).unwrap()
        );
        let from_dir =
            verse_engine::loading::Prepared::load(pinned, &pinned_dir(), Default::default())
                .unwrap();
        let from_memory = prepare_embedded().unwrap();
        assert_eq!(
            from_memory.receipt().manifest_sha256,
            from_dir.receipt().manifest_sha256
        );
        assert_eq!(
            from_memory.receipt().encoded_bytes,
            from_dir.receipt().encoded_bytes
        );
        assert_eq!(from_memory.textures().len(), 1);
    }

    #[test]
    fn the_floor_keeps_the_legacy_lattice() {
        let pack = load_pinned().unwrap();
        let floor = &pack.models[FLOOR];
        let legacy = crate::world::ground_mesh();
        let lines: usize = floor
            .surfaces
            .iter()
            .filter(|s| s.topology == Topology::Lines)
            .map(|s| s.indices.len() / 2)
            .sum();
        // Every legacy line spans the whole Grid and is stored in LOT-long
        // pieces; no piece is lost or merged.
        let pieces = (2.0 * crate::world::HALF / SEGMENT).ceil() as usize;
        assert_eq!(lines, legacy.lines.len() / 2 * pieces);
        assert!(floor.surfaces.iter().all(|s| s.unlit));
        assert!(floor.surfaces.iter().all(|s| {
            s.indices.chunks_exact(2).all(|i| {
                let a = Vec3::from_array(s.vertices[i[0] as usize].position);
                let b = Vec3::from_array(s.vertices[i[1] as usize].position);
                a.distance(b) <= SEGMENT + 1e-3
            })
        }));
        // Streets are brighter than the fine grid, in the neutral palette.
        let tints: Vec<[f32; 3]> = floor.surfaces.iter().map(|s| s.tint).collect();
        assert_eq!(tints.len(), 2);
        assert!(tints.iter().all(|t| t[0] == t[1] && t[1] == t[2]));
    }

    #[test]
    fn the_figure_swings_its_limbs_like_the_legacy_gait() {
        let pack = load_pinned().unwrap();
        let model = &pack.models[FIGURE];
        let selection = verse_engine::motion::Selection::Named(State::Walk);
        let rest = verse_engine::animation::pose_selected(model, selection, 0.0).unwrap();
        let quarter = verse_engine::animation::pose_selected(model, selection, 0.25).unwrap();
        let leg = Joint::LegLeft as usize;
        let foot = Vec3::new(0.14, 0.0, 0.0);
        assert!(rest[leg].transform_point3(foot).abs_diff_eq(foot, 1e-4));
        let swung = quarter[leg].transform_point3(foot);
        // A full swing of 0.75 rad about the hip at 0.92 m lifts the foot
        // and carries it along Z, the way the legacy figure's does.
        assert!(
            (swung.y - 0.92 * (1.0 - SWING.cos())).abs() < 1e-3,
            "{swung}"
        );
        assert!((swung.z.abs() - 0.92 * SWING.sin()).abs() < 1e-3, "{swung}");
        let other = quarter[Joint::LegRight as usize].transform_point3(Vec3::new(-0.14, 0.0, 0.0));
        assert!((other.z + swung.z).abs() < 1e-4, "legs swing opposite");
        assert_eq!(quarter[Joint::Torso as usize], Mat4::IDENTITY);
        let feet: f32 = model.surfaces[0]
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MAX, f32::min);
        assert!(feet.abs() < 0.03, "feet at {feet}");
        assert!(model.graph.is_some());
        assert_eq!(model.markers[0].track.markers.len(), 2);
    }

    #[test]
    fn a_gait_maps_onto_the_walk_clip_by_distance() {
        let mut gait = crate::avatar::Gait::default();
        gait.advance(WALK_CYCLE_METERS, false, 1.0);
        assert!(gait.cycle().abs() < 1e-3 || (1.0 - gait.cycle()).abs() < 1e-3);
        gait.advance(WALK_CYCLE_METERS / 4.0, false, 1.0);
        assert!((gait.cycle() - 0.25).abs() < 1e-3, "{}", gait.cycle());
    }
}

/// The engine frame of the Grid and its comparison with the legacy pass.
#[cfg(feature = "capture")]
pub mod capture {
    use crate::grid_frame;
    use crate::imported::Renderer;
    use crate::runtime::WorldRuntime;

    /// Channel difference, 0 to 255, below which two pixels match.
    pub const CHANNEL_TOLERANCE: u8 = 48;

    /// One rendered engine frame.
    pub struct Comparison {
        pub width: u32,
        pub height: u32,
        pub pixels: Vec<u8>,
    }

    impl Comparison {
        /// Renders the bare world's current frame from the pinned pack with
        /// the legacy camera and fog, assembled as the desktop app does.
        pub fn render(
            runtime: &WorldRuntime,
            width: u32,
            height: u32,
            atlas: &crate::ui::Atlas,
        ) -> Result<Self, String> {
            let pack = super::load_pinned()?;
            let statics = grid_frame::statics(&pack);
            let mut renderer =
                Renderer::new(pack, &super::pinned_dir(), width, height, atlas, &statics)?;
            let aspect = width as f32 / height as f32;
            let lighting = grid_frame::lighting(&runtime.atmosphere());
            let dynamic = grid_frame::dynamic(runtime, &[], &[]);
            let pixels = renderer.draw(
                runtime.view(aspect),
                &dynamic,
                &crate::ui::UiBatch::default(),
                &lighting,
            )?;
            Ok(Self {
                width,
                height,
                pixels,
            })
        }

        /// Writes the frame as a PNG.
        pub fn write(&self, path: &std::path::Path) -> Result<(), String> {
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut encoder = png::Encoder::new(file, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .map_err(|e| e.to_string())?
                .write_image_data(&self.pixels)
                .map_err(|e| e.to_string())
        }
    }

    /// Reads a PNG as RGBA bytes.
    pub fn read_png(path: &std::path::Path) -> Result<Vec<u8>, String> {
        let decoder = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(path).map_err(|e| e.to_string())?,
        ));
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buffer = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
        let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
        buffer.truncate(info.buffer_size());
        match info.color_type {
            png::ColorType::Rgba => Ok(buffer),
            png::ColorType::Rgb => Ok(buffer
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect()),
            other => Err(format!("Unsupported PNG color type {other:?}")),
        }
    }

    /// The fraction of pixels whose RGB channels differ by more than
    /// [`CHANNEL_TOLERANCE`] between two RGBA frames. Frames of different
    /// sizes, or buffers that are not whole RGBA pixels, mismatch entirely.
    #[must_use]
    pub fn mismatch(a: &[u8], b: &[u8]) -> f32 {
        if a.len() != b.len() || a.len() % 4 != 0 {
            return 1.0;
        }
        let pixels = a.len() / 4;
        if pixels == 0 {
            return 1.0;
        }
        let differing = (0..pixels)
            .filter(|i| (0..3).any(|c| a[i * 4 + c].abs_diff(b[i * 4 + c]) > CHANNEL_TOLERANCE))
            .count();
        differing as f32 / pixels as f32
    }
}

#[cfg(all(test, feature = "capture"))]
mod capture_tests {
    use super::capture::{CHANNEL_TOLERANCE, Comparison, mismatch, read_png};

    /// The largest fraction of pixels the engine frame may differ from the
    /// legacy line pass by: line rasterization and fog differ slightly.
    const FRAME_TOLERANCE: f32 = 0.06;

    #[test]
    fn mismatch_counts_pixels_past_the_channel_tolerance() {
        let a = [0, 0, 0, 255, 10, 10, 10, 255];
        let b = [
            0,
            0,
            CHANNEL_TOLERANCE,
            255,
            10,
            10,
            10 + CHANNEL_TOLERANCE + 1,
            255,
        ];
        assert_eq!(mismatch(&a, &b), 0.5);
        assert_eq!(mismatch(&a, &a), 0.0);
    }

    #[test]
    fn mismatch_refuses_frames_of_different_shape() {
        assert_eq!(mismatch(&[0; 8], &[0; 4]), 1.0);
        assert_eq!(mismatch(&[0; 6], &[0; 6]), 1.0);
        assert_eq!(mismatch(&[], &[]), 1.0);
    }

    #[test]
    fn the_engine_frame_matches_the_legacy_frame_at_spawn() {
        let mut runtime = crate::runtime::WorldRuntime::bare();
        for _ in 0..30 {
            runtime.tick(&crate::controller::InputState::default(), 1.0 / 60.0);
        }
        let (width, height) = (590u32, 1280u32);
        let aspect = width as f32 / height as f32;
        let atlas = crate::ui::Atlas::new(16.0);
        let dir = std::env::temp_dir().join(format!("verse-grid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("legacy.png");
        crate::render::capture_with_atmosphere(
            &legacy,
            width,
            height,
            &runtime.world.mesh,
            runtime.view(aspect),
            &runtime.dynamic_mesh(),
            &crate::ui::UiBatch::default(),
            &atlas,
            runtime.atmosphere(),
        )
        .unwrap();
        let engine = Comparison::render(&runtime, width, height, &atlas).unwrap();
        let fraction = mismatch(&engine.pixels, &read_png(&legacy).unwrap());
        println!("mismatch {fraction:.4} of pixels differ from the legacy frame");
        assert!(
            fraction <= FRAME_TOLERANCE,
            "{fraction} > {FRAME_TOLERANCE}"
        );
        let lit = engine.pixels.chunks_exact(4).filter(|p| p[0] > 128).count();
        assert!(lit > 0, "the engine frame is black");
        std::fs::remove_dir_all(&dir).ok();
    }
}
