//! A static MetaHuman export rigged with MetaHuman's own body skeleton, so
//! Verse can pose and animate it.
//!
//! A MetaHuman leaves Unreal as geometry (`scripts/unreal/dna_dump.cpp`,
//! `scripts/blender/metahuman_import.py`): the head, groom, eyes and
//! clothes in one glTF with no skin. Its body DNA still carries the real
//! skeleton and skin weights; `dna_dump BODY.dna OUT --joints POSE.txt
//! --skin` writes them beside the posed body mesh:
//!
//! - `joints.txt`: every joint's name, parent and neutral local transform
//!   (centimetres, Euler XYZ degrees, the DNA's frame);
//! - `body_lod0_mesh.obj`: the body, posed by `POSE.txt`'s rotation deltas
//!   (the export's own pose, such as the arms lowered from the A pose);
//! - `body_lod0_mesh.skin.txt`: each body vertex's joint weights.
//!
//! [`Rig::build`] puts every vertex of the export on that skeleton:
//!
//! - **Body and clothes** take the weights of the body surface under them:
//!   the nearest body vertices, blended by inverse distance (a weight
//!   transfer, as clothing is skinned to a body in any DCC tool).
//! - **The head** (skin, teeth, lashes, brows, the wet lines) leaves the
//!   body's weights for the `head` joint's where it rises off the body's
//!   neck, and the groom is the head's alone.
//! - **The eyes and upper lids** get four joints of their own, found from
//!   the eye-shaded surfaces, so gaze and blinks need no face rig.
//!
//! The DNA has hundreds of joints (twist, corrective and face helpers), and
//! a palette holds 256, so the rig keeps MetaHuman's main skeleton
//! ([`MAIN`]: spine, neck, head, arms, every finger, legs) and gives every
//! other joint's weight to its nearest kept ancestor. Those helpers follow
//! their parents, which is what they do when nothing drives them.
//!
//! A [`Pose`] holds every DNA joint's world matrix. Poses are built in the
//! DNA's own frame by rotating a joint's subtree about the joint
//! ([`Pose::rotate`]), reaching a limb to a point ([`Pose::reach`]),
//! curling fingers ([`Pose::curl`]) and aiming the eyes ([`Pose::look`]);
//! [`Rig::palette`] turns a pose into the model-space skinning matrices
//! `Renderer::set_pose_override` takes.
//!
//! MetaHuman assets are private: this module names none and reads only
//! what the caller hands it.
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use verse_engine::assets::{Bone, Clip, Model};
use verse_engine::material::Shading;

/// MetaHuman's main body skeleton, kept as palette joints. Fingers are
/// `{thumb,index,middle,ring,pinky}`; the four fingers have metacarpals.
pub const MAIN: &[&str] = &[
    "pelvis", "spine_01", "spine_02", "spine_03", "spine_04", "spine_05", "neck_01", "neck_02",
    "head",
];
const SIDE: &[&str] = &[
    "clavicle",
    "upperarm",
    "lowerarm",
    "hand",
    "thumb_01",
    "thumb_02",
    "thumb_03",
    "index_metacarpal",
    "index_01",
    "index_02",
    "index_03",
    "middle_metacarpal",
    "middle_01",
    "middle_02",
    "middle_03",
    "ring_metacarpal",
    "ring_01",
    "ring_02",
    "ring_03",
    "pinky_metacarpal",
    "pinky_01",
    "pinky_02",
    "pinky_03",
    "thigh",
    "calf",
    "foot",
    "ball",
];
/// The four fingers, for [`Pose::curl`].
pub const FINGERS: [&str; 4] = ["index", "middle", "ring", "pinky"];
/// How far from an arm's bones a garment counts as its sleeve by default:
/// fully within the first distance, not at all past the second (cm).
pub const SLEEVE: (f32, f32) = (11.0, 16.0);

/// A DNA skeleton: names, parents (`-1` for a root) and neutral local
/// transforms in the DNA's frame (centimetres; Euler XYZ in degrees,
/// applied X, then Y, then Z, as RigLogic's joint attributes).
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub names: Vec<String>,
    pub parents: Vec<i32>,
    translation: Vec<Vec3>,
    rotation: Vec<Vec3>,
}

/// RigLogic's Euler rotation: degrees about X, then Y, then Z.
fn euler(r: Vec3) -> Quat {
    let r = r * (std::f32::consts::PI / 180.0);
    Quat::from_rotation_z(r.z) * Quat::from_rotation_y(r.y) * Quat::from_rotation_x(r.x)
}

impl Skeleton {
    /// Parses `dna_dump`'s `joints.txt`: `name parent tx ty tz rx ry rz`
    /// per line, parents before children.
    ///
    /// # Errors
    /// A malformed line, or a parent that does not come first.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut s = Self {
            names: vec![],
            parents: vec![],
            translation: vec![],
            rotation: vec![],
        };
        for (n, line) in text.lines().enumerate() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.is_empty() {
                continue;
            }
            let bad = || {
                format!(
                    "joints line {}: expected name parent tx ty tz rx ry rz",
                    n + 1
                )
            };
            if f.len() != 8 {
                return Err(bad());
            }
            let parent: i32 = f[1].parse().map_err(|_| bad())?;
            let x: Vec<f32> = f[2..]
                .iter()
                .map(|v| v.parse::<f32>().map_err(|_| bad()))
                .collect::<Result<_, _>>()?;
            if parent >= s.names.len() as i32 || x.iter().any(|v| !v.is_finite()) {
                return Err(bad());
            }
            s.names.push(f[0].to_string());
            s.parents.push(parent);
            s.translation.push(Vec3::new(x[0], x[1], x[2]));
            s.rotation.push(Vec3::new(x[3], x[4], x[5]));
        }
        if s.names.is_empty() {
            return Err("The skeleton has no joints".into());
        }
        Ok(s)
    }

    #[must_use]
    pub fn index(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }

    /// Every joint's world matrix with `deltas` (degrees, by joint name)
    /// added to its neutral rotation, as `dna_dump --joints` poses it.
    #[must_use]
    pub fn world(&self, deltas: &HashMap<String, Vec3>) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.names.len());
        for j in 0..self.names.len() {
            let r = self.rotation[j] + deltas.get(&self.names[j]).copied().unwrap_or(Vec3::ZERO);
            let local = Mat4::from_rotation_translation(euler(r), self.translation[j]);
            out.push(match self.parents[j] {
                p if p >= 0 => out[p as usize] * local,
                _ => local,
            });
        }
        out
    }
}

/// Parses a pose file of rotation deltas (`joint rx ry rz` per line, in
/// degrees; `#` comments), as `dna_dump --joints` reads it.
///
/// # Errors
/// A malformed line.
pub fn parse_deltas(text: &str) -> Result<HashMap<String, Vec3>, String> {
    let mut out = HashMap::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let v: Option<Vec<f32>> = f
            .get(1..4)
            .map(|x| x.iter().filter_map(|v| v.parse().ok()).collect());
        match v {
            Some(v) if v.len() == 3 && f.len() == 4 => {
                out.insert(f[0].to_string(), Vec3::new(v[0], v[1], v[2]));
            }
            _ => return Err(format!("pose line {}: expected joint rx ry rz", n + 1)),
        }
    }
    Ok(out)
}

/// The body mesh's vertex positions (DNA frame, centimetres) and their
/// skin weights (joint index, weight).
#[derive(Clone, Debug)]
pub struct BodySkin {
    pub positions: Vec<Vec3>,
    pub weights: Vec<Vec<(u32, f32)>>,
}

impl BodySkin {
    /// Reads `dna_dump`'s posed body OBJ (only its `v` lines) and its
    /// `.skin.txt` (`count joint weight ...` per vertex).
    ///
    /// # Errors
    /// Malformed lines, or counts that disagree.
    pub fn parse(obj: &str, skin: &str) -> Result<Self, String> {
        let positions: Vec<Vec3> = obj
            .lines()
            .filter_map(|l| l.strip_prefix("v "))
            .map(|l| {
                let v: Vec<f32> = l
                    .split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect();
                (v.len() == 3)
                    .then(|| Vec3::new(v[0], v[1], v[2]))
                    .ok_or_else(|| "A body OBJ vertex is not three numbers".to_string())
            })
            .collect::<Result<_, _>>()?;
        let weights: Vec<Vec<(u32, f32)>> = skin
            .lines()
            .enumerate()
            .map(|(n, l)| {
                let f: Vec<&str> = l.split_whitespace().collect();
                let count: usize = f.first().and_then(|c| c.parse().ok()).unwrap_or(usize::MAX);
                if count == usize::MAX || f.len() != 1 + 2 * count {
                    return Err(format!(
                        "skin line {}: expected count (joint weight)*",
                        n + 1
                    ));
                }
                (0..count)
                    .map(|k| {
                        let j = f[1 + 2 * k].parse::<u32>();
                        let w = f[2 + 2 * k].parse::<f32>();
                        match (j, w) {
                            (Ok(j), Ok(w)) if w.is_finite() => Ok((j, w)),
                            _ => Err(format!("skin line {}: a bad joint or weight", n + 1)),
                        }
                    })
                    .collect()
            })
            .collect::<Result<_, _>>()?;
        if positions.is_empty() || positions.len() != weights.len() {
            return Err(format!(
                "The body has {} vertices and {} skin lines",
                positions.len(),
                weights.len()
            ));
        }
        Ok(Self { positions, weights })
    }
}

/// A uniform grid over points, for nearest-neighbour queries.
struct Grid {
    cell: f32,
    cells: HashMap<[i32; 3], Vec<u32>>,
}

impl Grid {
    fn new(points: &[Vec3], cell: f32) -> Self {
        let mut cells: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            cells.entry(Self::key(*p, cell)).or_default().push(i as u32);
        }
        Self { cell, cells }
    }

    fn key(p: Vec3, cell: f32) -> [i32; 3] {
        let k = (p / cell).floor();
        [k.x as i32, k.y as i32, k.z as i32]
    }

    /// The `k` nearest points to `p` (index, distance), searching rings of
    /// cells outward until `k` are found and no nearer one can remain.
    fn nearest(&self, points: &[Vec3], p: Vec3, k: usize) -> Vec<(u32, f32)> {
        let c = Self::key(p, self.cell);
        let mut found: Vec<(u32, f32)> = Vec::new();
        for ring in 0..64i32 {
            for x in -ring..=ring {
                for y in -ring..=ring {
                    for z in -ring..=ring {
                        if x.abs().max(y.abs()).max(z.abs()) != ring {
                            continue;
                        }
                        if let Some(list) = self.cells.get(&[c[0] + x, c[1] + y, c[2] + z]) {
                            found.extend(list.iter().map(|&i| (i, points[i as usize].distance(p))));
                        }
                    }
                }
            }
            if found.len() >= k {
                found.sort_by(|a, b| a.1.total_cmp(&b.1));
                // Anything outside the rings searched is at least this far.
                if found[k - 1].1 <= ring as f32 * self.cell {
                    break;
                }
            }
        }
        found.sort_by(|a, b| a.1.total_cmp(&b.1));
        found.truncate(k);
        found
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A rigged MetaHuman export: its palette joints and bind pose.
#[derive(Clone, Debug)]
pub struct Rig {
    pub skeleton: Skeleton,
    /// The DNA joint behind each palette joint, before the four eye joints.
    pub joints: Vec<usize>,
    /// Every DNA joint's world matrix in the export's pose.
    bind: Vec<Mat4>,
    /// The DNA's frame (centimetres) into the model's space.
    frame: Mat4,
    /// The eyeballs' centres in the DNA's frame (her left, +X, first).
    pub eyes: [Vec3; 2],
    /// How far from the arms' bones [`Self::skin`] lets a garment follow
    /// them (cm, full to none): a loose robe needs less than a sleeve's
    /// width, or its panels beside the arms go with them.
    pub sleeve: (f32, f32),
}

/// Every DNA joint's world matrix (DNA frame), with the eyes' turn and the
/// upper lids' close on top.
#[derive(Clone, Debug)]
pub struct Pose {
    pub world: Vec<Mat4>,
    pub eyes: [Quat; 2],
    /// 0 open to 1 shut.
    pub blink: f32,
}

impl Rig {
    /// The rig for `model` (a static MetaHuman export, in the importer's
    /// model space) on `skeleton` posed by `bind_deltas` (the export's own
    /// pose). `frame` maps the DNA's frame (centimetres) into model space.
    /// Reads the eyeballs' centres from the model; [`Self::skin`] weights
    /// its vertices.
    ///
    /// # Errors
    /// The skeleton lacks a main joint, or the export has no eyeballs.
    pub fn new(
        model: &Model,
        skeleton: Skeleton,
        bind_deltas: &HashMap<String, Vec3>,
        frame: Mat4,
    ) -> Result<Self, String> {
        let mut names: Vec<String> = MAIN.iter().map(|s| (*s).to_string()).collect();
        for side in ["l", "r"] {
            names.extend(SIDE.iter().map(|s| format!("{s}_{side}")));
        }
        let joints: Vec<usize> = names
            .iter()
            .map(|n| {
                skeleton
                    .index(n)
                    .ok_or_else(|| format!("The skeleton has no {n} joint"))
            })
            .collect::<Result<_, _>>()?;
        let bind = skeleton.world(bind_deltas);
        let to_dna = frame.inverse();
        // Eyeballs: small eye- or cornea-shaded surfaces.
        let mut eyes = [Vec3::ZERO; 2];
        let mut found = [false; 2];
        for s in &model.surfaces {
            let shading = s.material.shading;
            if !(shading == Shading::Eye as u8 || shading == Shading::Cornea as u8)
                || s.vertices.is_empty()
            {
                continue;
            }
            let (min, max) = bounds(
                s.vertices
                    .iter()
                    .map(|v| to_dna.transform_point3(v.position.into())),
            );
            if max.x - min.x > 4.5 {
                continue;
            }
            let side = usize::from((min.x + max.x) * 0.5 < 0.0);
            eyes[side] = (min + max) * 0.5;
            found[side] = true;
        }
        if !found[0] || !found[1] {
            return Err("The export has no eyeballs (eye-shaded surfaces)".into());
        }
        Ok(Self {
            skeleton,
            joints,
            bind,
            frame,
            eyes,
            sleeve: SLEEVE,
        })
    }

    /// Weights every vertex of `model` (the export [`Self::new`] read) on
    /// the rig, from `body` (the body mesh in the export's pose with its
    /// skin weights), and replaces the model's bones, clips and skin.
    ///
    /// # Errors
    /// The body does not sit where the export's body skin does (a wrong
    /// frame or pose).
    pub fn skin(&self, model: &mut Model, body: &BodySkin) -> Result<(), String> {
        let (skeleton, joints, bind, eyes) = (&self.skeleton, &self.joints, &self.bind, self.eyes);
        let to_dna = self.frame.inverse();
        // Every DNA joint's palette joint: itself if kept, else its nearest
        // kept ancestor (the pelvis for a root).
        let mut palette_of = vec![0u32; skeleton.names.len()];
        let kept: HashMap<usize, u32> = joints
            .iter()
            .enumerate()
            .map(|(b, &j)| (j, b as u32))
            .collect();
        for j in 0..skeleton.names.len() {
            palette_of[j] = match kept.get(&j) {
                Some(&b) => b,
                None => match skeleton.parents[j] {
                    p if p >= 0 => palette_of[p as usize],
                    _ => 0,
                },
            };
        }
        let head_dna = skeleton.index("head").ok_or("no head")?;
        let head = kept[&head_dna];
        let main = joints.len() as u32;
        let eye_joint = [main, main + 1];
        let lid_joint = [main + 2, main + 3];
        let grid = Grid::new(&body.positions, 2.0);
        // The arms: every palette joint from the upper arm down, each side.
        let arm_joints: Vec<u32> = ["l", "r"]
            .iter()
            .flat_map(|side| {
                SIDE.iter()
                    .take_while(|n| !n.starts_with("thigh"))
                    .filter(|n| **n != "clavicle")
                    .map(move |n| format!("{n}_{side}"))
            })
            .filter_map(|n| skeleton.index(&n).and_then(|j| kept.get(&j).copied()))
            .collect();
        // The body without its arms, for clothes that only lie near them.
        let trunk: Vec<usize> = (0..body.positions.len())
            .filter(|&i| {
                let on_arm: f32 = body.weights[i]
                    .iter()
                    .filter(|(j, _)| arm_joints.contains(&palette_of[*j as usize]))
                    .map(|(_, w)| *w)
                    .sum();
                on_arm < 0.5
            })
            .collect();
        let trunk_points: Vec<Vec3> = trunk.iter().map(|&i| body.positions[i]).collect();
        let trunk_grid = Grid::new(&trunk_points, 2.0);
        // Each arm's bones as segments (DNA frame), shoulder to fingertip.
        let arm_bones: Vec<(Vec3, Vec3)> = ["l", "r"]
            .iter()
            .flat_map(|side| {
                let chain = ["upperarm", "lowerarm", "hand", "middle_03"].map(|n| {
                    skeleton
                        .index(&format!("{n}_{side}"))
                        .map(|j| bind[j].w_axis.truncate())
                });
                (0..3).filter_map(move |k| Some((chain[k]?, chain[k + 1]?)))
            })
            .collect();
        let wrists: Vec<f32> = ["l", "r"]
            .iter()
            .filter_map(|side| {
                skeleton
                    .index(&format!("hand_{side}"))
                    .map(|j| bind[j].w_axis.y)
            })
            .collect();
        let wrist_y = wrists.iter().copied().fold(f32::MAX, f32::min);
        // Check the frame: the export's body skin must lie on the body.
        let neck_top = body.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let head_y = bind[head_dna].w_axis.y;
        let mut misses = Vec::new();
        let gather = |grid: &Grid,
                      points: &[Vec3],
                      index: &dyn Fn(u32) -> usize,
                      p: Vec3,
                      acc: &mut HashMap<u32, f32>,
                      share: f32|
         -> f32 {
            let near = grid.nearest(points, p, 6);
            let total: f32 = near
                .iter()
                .map(|&(_, d)| 1.0 / (d * d + 0.25))
                .sum::<f32>()
                .max(1e-9);
            for &(i, d) in &near {
                let w = share / (d * d + 0.25) / total;
                for &(j, jw) in &body.weights[index(i)] {
                    *acc.entry(palette_of[j as usize]).or_default() += w * jw;
                }
            }
            near.first().map_or(f32::MAX, |n| n.1)
        };
        // `arms`: how much the vertex may follow the arms (0 takes the
        // trunk's weights alone).
        let transfer = |p: Vec3, arms: f32| -> ([u32; 4], [f32; 4], f32) {
            let mut acc: HashMap<u32, f32> = HashMap::new();
            let mut d = f32::MAX;
            if arms > 0.0 {
                d = gather(&grid, &body.positions, &|i| i as usize, p, &mut acc, arms);
            }
            if arms < 1.0 {
                let dt = gather(
                    &trunk_grid,
                    &trunk_points,
                    &|i| trunk[i as usize],
                    p,
                    &mut acc,
                    1.0 - arms,
                );
                d = d.min(dt);
            }
            let mut list: Vec<(u32, f32)> = acc.into_iter().collect();
            list.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            list.truncate(4);
            let total: f32 = list.iter().map(|x| x.1).sum::<f32>().max(1e-9);
            let mut j = [0u32; 4];
            let mut w = [0f32; 4];
            for (k, (jj, ww)) in list.into_iter().enumerate() {
                j[k] = jj;
                w[k] = ww / total;
            }
            (j, w, d)
        };
        // A garment follows an arm only in its sleeve: within a sleeve's
        // reach of the arm's bones, and above the wrist (a robe's panels
        // hang beside the hands; carried with a raised arm they'd tear
        // into a sheet).
        let sleeve = |p: Vec3| -> f32 {
            let d = arm_bones
                .iter()
                .map(|&(a, b)| {
                    let ab = b - a;
                    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
                    p.distance(a + ab * t)
                })
                .fold(f32::MAX, f32::min);
            (1.0 - smooth(self.sleeve.0, self.sleeve.1, d))
                * smooth(wrist_y - 6.0, wrist_y + 2.0, p.y)
        };
        for s in &mut model.surfaces {
            let shading = s.material.shading;
            if s.vertices.is_empty() {
                continue;
            }
            let (min, max) = bounds(
                s.vertices
                    .iter()
                    .map(|v| to_dna.transform_point3(v.position.into())),
            );
            let small_eye = (shading == Shading::Eye as u8 || shading == Shading::Cornea as u8)
                && max.x - min.x < 4.5;
            let hair = shading == Shading::Hair as u8;
            let cloth = shading == Shading::Cloth as u8;
            // A head part: not cloth, and centred above the body's neck.
            let head_part = !cloth && (min.y + max.y) * 0.5 > neck_top;
            for v in &mut s.vertices {
                let p = to_dna.transform_point3(v.position.into());
                if small_eye {
                    let side = usize::from((min.x + max.x) * 0.5 < 0.0);
                    v.joints = [eye_joint[side], 0, 0, 0];
                    v.weights = [1.0, 0.0, 0.0, 0.0];
                    continue;
                }
                let lid = |j: &mut [u32; 4], w: &mut [f32; 4]| {
                    // The upper lids (skin and lashes) over the front of
                    // each eye.
                    for side in 0..2 {
                        let c = eyes[side];
                        let lid = (1.0 - smooth(1.45, 2.1, p.distance(c)))
                            * smooth(-0.2, 0.3, p.y - c.y)
                            * smooth(-0.4, 0.4, p.z - c.z);
                        if lid > 0.0 {
                            *j = [head, lid_joint[side], 0, 0];
                            *w = [1.0 - lid, lid, 0.0, 0.0];
                        }
                    }
                };
                if hair {
                    let (mut j, mut w) = ([head, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]);
                    lid(&mut j, &mut w);
                    v.joints = j;
                    v.weights = w;
                    continue;
                }
                let arms = if cloth { sleeve(p) } else { 1.0 };
                let (mut j, mut w, d) = transfer(p, arms);
                if shading == Shading::Skin as u8 && !head_part && misses.len() < 4096 {
                    misses.push(d);
                }
                if head_part {
                    // Off the body's neck, the head carries it; below the
                    // body's neck line (the head mesh's own neck and
                    // chest) the body does, or the chest's skin would ride
                    // up over the collar when she lifts her head.
                    let h = (smooth(0.8, 2.5, d) * smooth(neck_top - 3.0, neck_top + 1.0, p.y))
                        .max(smooth(neck_top, head_y, p.y));
                    let mut acc: Vec<(u32, f32)> =
                        (0..4).map(|k| (j[k], w[k] * (1.0 - h))).collect();
                    match acc.iter_mut().find(|x| x.0 == head && x.1 > 0.0) {
                        Some(x) => x.1 += h,
                        None => {
                            acc.sort_by(|a, b| b.1.total_cmp(&a.1));
                            acc[3] = (head, h);
                        }
                    }
                    let total: f32 = acc.iter().map(|x| x.1).sum::<f32>().max(1e-9);
                    for k in 0..4 {
                        j[k] = acc[k].0;
                        w[k] = acc[k].1 / total;
                    }
                    if h > 0.99 {
                        lid(&mut j, &mut w);
                    }
                }
                v.joints = j;
                v.weights = w;
            }
        }
        // The body's own skin (hands, legs) is the body mesh itself: it
        // must lie on it. A wrong frame or pose shows here.
        if !misses.is_empty() {
            misses.sort_by(f32::total_cmp);
            let median = misses[misses.len() / 2];
            if median > 0.5 {
                return Err(format!(
                    "The export's body skin lies {median:.2} cm off the DNA body: wrong frame or pose"
                ));
            }
        }
        let parents: Vec<i16> = joints
            .iter()
            .map(|&j| {
                let mut p = skeleton.parents[j];
                while p >= 0 {
                    if let Some(&b) = kept.get(&(p as usize)) {
                        return b as i16;
                    }
                    p = skeleton.parents[p as usize];
                }
                -1
            })
            .chain([head as i16; 4])
            .collect();
        model.skin = None;
        model.bones = parents
            .into_iter()
            .map(|parent| Bone {
                parent,
                pivot: [0.0; 3],
            })
            .collect();
        model.clips = vec![Clip {
            id: 0,
            duration: 1.0,
            bones: vec![],
        }];
        Ok(())
    }

    /// The export's own pose.
    #[must_use]
    pub fn rest(&self) -> Pose {
        Pose {
            world: self.bind.clone(),
            eyes: [Quat::IDENTITY; 2],
            blink: 0.0,
        }
    }

    /// The DNA's frame (centimetres) into model space.
    #[must_use]
    pub fn frame(&self) -> Mat4 {
        self.frame
    }

    /// The palette joints' model-space skinning matrices for `pose`.
    #[must_use]
    pub fn palette(&self, pose: &Pose) -> Vec<Mat4> {
        let inv = self.frame.inverse();
        let skin = |j: usize| pose.world[j] * self.bind[j].inverse();
        let mut out: Vec<Mat4> = self
            .joints
            .iter()
            .map(|&j| self.frame * skin(j) * inv)
            .collect();
        let head = skin(self.skeleton.index("head").unwrap_or(0));
        let around = |c: Vec3, q: Quat| {
            Mat4::from_translation(c) * Mat4::from_quat(q) * Mat4::from_translation(-c)
        };
        for side in 0..2 {
            out.push(self.frame * head * around(self.eyes[side], pose.eyes[side]) * inv);
        }
        for side in 0..2 {
            // The lid turns down about the eye's horizontal axis.
            let close = Quat::from_rotation_x(1.0 * pose.blink.clamp(0.0, 1.0));
            out.push(self.frame * head * around(self.eyes[side], close) * inv);
        }
        out
    }
}

fn bounds(points: impl Iterator<Item = Vec3>) -> (Vec3, Vec3) {
    points.fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(a, b), p| (a.min(p), b.max(p)),
    )
}

impl Pose {
    /// Where `joint` is (DNA frame).
    ///
    /// # Panics
    /// `joint` is not in the skeleton.
    #[must_use]
    pub fn position(&self, rig: &Rig, joint: &str) -> Vec3 {
        let j = rig
            .skeleton
            .index(joint)
            .unwrap_or_else(|| panic!("no joint {joint}"));
        self.world[j].w_axis.truncate()
    }

    /// Turns `joint` and everything below it by `q` (a world rotation)
    /// about the joint.
    pub fn rotate(&mut self, rig: &Rig, joint: &str, q: Quat) {
        let Some(root) = rig.skeleton.index(joint) else {
            return;
        };
        let c = self.world[root].w_axis.truncate();
        let m = Mat4::from_translation(c) * Mat4::from_quat(q) * Mat4::from_translation(-c);
        let mut inside = vec![false; self.world.len()];
        for j in root..self.world.len() {
            let p = rig.skeleton.parents[j];
            inside[j] = j == root || (p >= 0 && inside[p as usize]);
            if inside[j] {
                self.world[j] = m * self.world[j];
            }
        }
    }

    /// Reaches a two-joint limb (`upper` → `lower` → `end`, such as
    /// upperarm, lowerarm, hand) so `end` lands on `target`, its middle
    /// joint bending toward `pole`. Out of reach, the limb points straight
    /// at the target.
    pub fn reach(&mut self, rig: &Rig, [upper, lower, end]: [&str; 3], target: Vec3, pole: Vec3) {
        let a = self.position(rig, upper);
        let b = self.position(rig, lower);
        let c = self.position(rig, end);
        let (l1, l2) = (a.distance(b), b.distance(c));
        let to = target - a;
        let d = to.length().clamp((l1 - l2).abs() + 1e-3, l1 + l2 - 1e-3);
        let dir = to.normalize_or(Vec3::NEG_Y);
        let side = (pole - a) - dir * (pole - a).dot(dir);
        let side = side.normalize_or(dir.any_orthonormal_vector());
        let cos = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
        let elbow = a + dir * l1 * cos + side * l1 * (1.0 - cos * cos).sqrt();
        self.rotate(
            rig,
            upper,
            Quat::from_rotation_arc((b - a).normalize(), (elbow - a).normalize()),
        );
        let b = self.position(rig, lower);
        let c = self.position(rig, end);
        let goal = a + dir * d;
        self.rotate(
            rig,
            lower,
            Quat::from_rotation_arc((c - b).normalize(), (goal - b).normalize()),
        );
    }

    /// Curls the four fingers of the `side` ("l" or "r") hand by `angle`
    /// radians at each knuckle, toward the palm; a negative angle opens
    /// them. `spread` fans them apart (radians between neighbours).
    pub fn curl(&mut self, rig: &Rig, side: &str, angle: f32, spread: f32) {
        for (n, finger) in FINGERS.iter().enumerate() {
            // Fan from the middle finger out, about the palm's normal.
            let fan = (n as f32 - 1.0) * spread;
            for k in 1..=3 {
                let joint = format!("{finger}_0{k}_{side}");
                let next = if k < 3 {
                    format!("{finger}_0{}_{side}", k + 1)
                } else {
                    joint.clone()
                };
                let (Some(_), Some(_)) = (rig.skeleton.index(&joint), rig.skeleton.index(&next))
                else {
                    continue;
                };
                let p = self.position(rig, &joint);
                let along = if k < 3 {
                    (self.position(rig, &next) - p).normalize()
                } else {
                    (p - self.position(rig, &format!("{finger}_02_{side}"))).normalize()
                };
                // The palm's side of the finger: MetaHuman's palm helper.
                let palm = format!("{finger}_01_palm_{side}");
                let normal = match rig.skeleton.index(&palm) {
                    Some(_) => {
                        let n = self.position(rig, &palm)
                            - self.position(rig, &format!("{finger}_01_{side}"));
                        (n - along * n.dot(along)).normalize_or(Vec3::Y)
                    }
                    None => Vec3::Y,
                };
                if k == 1 && fan != 0.0 {
                    self.rotate(rig, &joint, Quat::from_axis_angle(normal, fan));
                }
                self.rotate(
                    rig,
                    &joint,
                    Quat::from_axis_angle(along.cross(normal).normalize(), angle),
                );
            }
        }
    }

    /// Turns `joint` (and everything below it) so its face's forward
    /// direction (DNA +Z in the export's pose: where she looks) moves
    /// `share` of the way toward `target`, at most `limit` radians.
    /// Splitting a look between neck and head reads as a person turning.
    pub fn aim(&mut self, rig: &Rig, joint: &str, target: Vec3, share: f32, limit: f32) {
        let Some(j) = rig.skeleton.index(joint) else {
            return;
        };
        let skin = self.world[j] * rig.bind[j].inverse();
        let from = skin.transform_vector3(Vec3::Z).normalize();
        let at = self.world[j].w_axis.truncate();
        let to = (target - at).normalize_or(from);
        let (axis, angle) = Quat::from_rotation_arc(from, to).to_axis_angle();
        if angle > 1e-5 {
            self.rotate(
                rig,
                joint,
                Quat::from_axis_angle(axis, (angle * share).min(limit)),
            );
        }
    }

    /// Turns the `side` hand about its forearm so its palm faces `toward`
    /// (a direction, DNA frame) as nearly as the forearm allows.
    pub fn turn_palm(&mut self, rig: &Rig, side: &str, toward: Vec3) {
        let hand = format!("hand_{side}");
        let (Some(_), Some(_)) = (
            rig.skeleton.index(&format!("middle_01_palm_{side}")),
            rig.skeleton.index(&format!("lowerarm_{side}")),
        ) else {
            return;
        };
        let axis = (self.position(rig, &hand) - self.position(rig, &format!("lowerarm_{side}")))
            .normalize_or(Vec3::Y);
        let normal = self.position(rig, &format!("middle_01_palm_{side}"))
            - self.position(rig, &format!("middle_01_{side}"));
        let flat = |v: Vec3| (v - axis * v.dot(axis)).normalize_or(Vec3::ZERO);
        let (a, b) = (flat(normal), flat(toward));
        if a == Vec3::ZERO || b == Vec3::ZERO {
            return;
        }
        let angle = a.cross(b).dot(axis).atan2(a.dot(b));
        self.rotate(rig, &hand, Quat::from_axis_angle(axis, angle));
    }

    /// Turns both eyes toward `target` (DNA frame), each within `limit`
    /// radians of where the head points them.
    pub fn look(&mut self, rig: &Rig, target: Vec3, limit: f32) {
        let head = rig.skeleton.index("head").unwrap_or(0);
        let skin = self.world[head] * rig.bind[head].inverse();
        for side in 0..2 {
            let c = skin.transform_point3(rig.eyes[side]);
            let ahead = skin.transform_vector3(Vec3::Z).normalize();
            let want = (target - c).normalize_or(ahead);
            let q = Quat::from_rotation_arc(ahead, want);
            let (axis, angle) = q.to_axis_angle();
            // The rotation is in world; the eye joint applies it under the
            // head's, so take it into the head's bind frame.
            let local = Quat::from_mat4(&skin).inverse()
                * Quat::from_axis_angle(axis, angle.min(limit))
                * Quat::from_mat4(&skin);
            self.eyes[side] = local;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOINTS: &str = "root -1 0 0 0 0 0 0\n\
        upper 0 0 10 0 0 0 0\n\
        lower 1 10 0 0 0 0 0\n\
        hand 2 10 0 0 0 0 0\n";

    #[test]
    fn a_skeleton_parses_and_poses_by_its_deltas() {
        let s = Skeleton::parse(JOINTS).unwrap();
        let rest = s.world(&HashMap::new());
        assert!(
            rest[3]
                .w_axis
                .truncate()
                .distance(Vec3::new(20.0, 10.0, 0.0))
                < 1e-4
        );
        // 90 degrees about Z at the upper joint swings the arm up.
        let deltas = parse_deltas("# up\nupper 0 0 90\n").unwrap();
        let up = s.world(&deltas);
        assert!(up[3].w_axis.truncate().distance(Vec3::new(0.0, 30.0, 0.0)) < 1e-3);
        assert!(Skeleton::parse("a 3 0 0 0 0 0 0").is_err());
        assert!(parse_deltas("upper 1 2").is_err());
    }

    #[test]
    fn euler_angles_compose_x_then_y_then_z() {
        let q = euler(Vec3::new(90.0, 90.0, 0.0));
        // X first takes +Y to +Z, then Y takes +Z to +X.
        assert!((q * Vec3::Y).distance(Vec3::X) < 1e-5);
    }

    #[test]
    fn a_limb_reaches_its_target_and_bends_toward_the_pole() {
        let s = Skeleton::parse(JOINTS).unwrap();
        let rig = Rig {
            joints: vec![0, 1, 2, 3],
            bind: s.world(&HashMap::new()),
            skeleton: s,
            frame: Mat4::IDENTITY,
            eyes: [Vec3::ZERO; 2],
            sleeve: SLEEVE,
        };
        let mut pose = rig.rest();
        let target = Vec3::new(5.0, 22.0, 3.0);
        pose.reach(
            &rig,
            ["upper", "lower", "hand"],
            target,
            Vec3::new(0.0, 0.0, 50.0),
        );
        assert!(pose.position(&rig, "hand").distance(target) < 1e-3);
        assert!(pose.position(&rig, "lower").z > 3.0);
        // Bone lengths hold.
        assert!(
            (pose
                .position(&rig, "upper")
                .distance(pose.position(&rig, "lower"))
                - 10.0)
                .abs()
                < 1e-3
        );
        // Rest palette is the identity.
        for m in rig.palette(&rig.rest()) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-4));
        }
    }

    #[test]
    fn the_grid_finds_the_nearest_points() {
        let points: Vec<Vec3> = (0..1000)
            .map(|i| Vec3::new((i % 10) as f32, ((i / 10) % 10) as f32, (i / 100) as f32))
            .collect();
        let grid = Grid::new(&points, 2.0);
        let near = grid.nearest(&points, Vec3::new(3.2, 4.1, 5.0), 3);
        assert_eq!(near[0].0, 543);
        assert!(near.windows(2).all(|w| w[0].1 <= w[1].1));
        let far = grid.nearest(&points, Vec3::new(40.0, 4.0, 5.0), 1);
        assert_eq!(far[0].0, 549);
    }
}
