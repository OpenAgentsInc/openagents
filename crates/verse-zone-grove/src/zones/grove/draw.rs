//! How the Grove draws: the dummies as the pack's `props/Dummy` beside the
//! player's character in one figure, the flying target's post, each
//! dummy's name and colored health bar facing the camera, floating damage
//! numbers, the soft target's ring, and the spells' effects.

use super::dummies::{Condition, Dummy, HEIGHT};
use super::kit::Spell;
use crate::mesh::{Mesh, Vertex};
use crate::pbr::textured::{Figure, TexturedMesh, TexturedScene, TexturedVertex};
use crate::zones::everglade::scene::{Copied, copy_material};
use crate::zones::everglade_pack::ZonePack;
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::TAU;
use std::sync::Arc;

/// The pack's training dummy.
pub const DUMMY_MODEL: &str = "props/Dummy";
/// Width and height of a health bar at scale one, m.
const BAR: [f32; 2] = [1.3, 0.16];
/// Height of a dummy's name, m.
const NAME: f32 = 0.2;
/// How long a number floats, s, and a number floating up from a hit:
/// Everglade's demolition yard draws them too.
pub use crate::zones::everglade::floaters::{FLOAT, Floater};
/// How long a change of shape's swirl lasts, s.
const SHIFT: f32 = 0.7;

/// The figure the player's character, the Wild Shape beasts, and the
/// dummies share: the character's scene with each beast's images,
/// materials, and primitives, then the dummy's added once for each dummy,
/// so the renderer uploads it once and rewrites only the vertices each
/// frame.
pub struct Model {
    scene: Arc<TexturedScene>,
    /// How many vertices the character contributes, and the character
    /// scene they belong to.
    cast: Option<(Arc<TexturedScene>, usize)>,
    /// How many vertices each beast contributes, in form order; zero for a
    /// form the pack lacks.
    forms: Vec<usize>,
    /// The dummy's vertices in model space, every primitive's in turn.
    dummy: Vec<TexturedVertex>,
}

/// Where a vertex goes that draws nothing: the character's or a beast's
/// while another stands in its place.
const FOLDED: TexturedVertex = TexturedVertex {
    pos: [0.0, -100.0, 0.0],
    normal: [0.0, 1.0, 0.0],
    uv: [0.0; 2],
    color: [0; 4],
    light: crate::pbr::textured::UNBAKED,
};

impl Model {
    /// The shared figure for `cast`'s characters, the beasts in `forms`
    /// (each in its bind pose, `None` for a form the pack lacks), and
    /// `count` dummies.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack has no dummy.
    pub fn new(
        pack: &ZonePack,
        cast: Option<&Figure>,
        forms: &[Option<Figure>],
        count: usize,
    ) -> Result<Self, String> {
        let model = pack
            .model(DUMMY_MODEL)
            .ok_or_else(|| format!("The Everglade pack has no {DUMMY_MODEL}"))?;
        let mut scene = TexturedScene::default();
        let mut mesh = TexturedMesh::default();
        let mut cast_vertices = None;
        if let Some(figure) = cast {
            scene.images.clone_from(&figure.scene.images);
            scene.materials.clone_from(&figure.scene.materials);
            if let Some(first) = figure.scene.meshes.first() {
                mesh.primitives.clone_from(&first.primitives);
            }
            let count = mesh.primitives.iter().map(|p| p.vertices.len()).sum();
            cast_vertices = Some((figure.scene.clone(), count));
        }
        // Each beast's images and materials after those before it.
        let mut form_counts = Vec::with_capacity(forms.len());
        for form in forms {
            let Some(figure) = form else {
                form_counts.push(0);
                continue;
            };
            let (images, materials) = (scene.images.len(), scene.materials.len());
            scene.images.extend(figure.scene.images.iter().cloned());
            scene
                .materials
                .extend(figure.scene.materials.iter().map(|m| {
                    crate::pbr::textured::TexturedMaterial {
                        image: m.image.map(|i| i + images),
                        ..m.clone()
                    }
                }));
            let mut vertices = 0;
            for primitive in figure.scene.meshes.iter().flat_map(|m| &m.primitives) {
                vertices += primitive.vertices.len();
                mesh.primitives.push(crate::pbr::textured::Primitive {
                    material: primitive.material + materials,
                    ..primitive.clone()
                });
            }
            form_counts.push(vertices);
        }
        let mut copied = Copied::default();
        let mut primitives = Vec::with_capacity(model.primitives.len());
        let mut dummy = Vec::new();
        for primitive in &model.primitives {
            let material = copy_material(pack, primitive.material, &mut scene, &mut copied)?;
            let vertices: Vec<TexturedVertex> = primitive
                .vertices
                .iter()
                .map(|v| TexturedVertex {
                    pos: v.position,
                    normal: v.normal,
                    uv: v.uv,
                    color: v.color,
                    light: crate::pbr::textured::UNBAKED,
                })
                .collect();
            dummy.extend(vertices.iter().copied());
            primitives.push(crate::pbr::textured::Primitive {
                vertices,
                indices: primitive.indices.clone(),
                material,
            });
        }
        for _ in 0..count {
            mesh.primitives.extend(primitives.iter().cloned());
        }
        scene.add_mesh(mesh);
        scene.validate()?;
        Ok(Self {
            scene: Arc::new(scene),
            cast: cast_vertices,
            forms: form_counts,
            dummy,
        })
    }

    /// How many vertices the player's character contributes, first in the
    /// figure.
    #[must_use]
    pub fn cast_count(&self) -> usize {
        self.cast.as_ref().map_or(0, |c| c.1)
    }

    /// This frame's figure: `cast` posed, or, while the druid wears a
    /// beast's shape, `worn` (the form's index and its posed vertices) in
    /// its place; then each of `dummies` standing where it is, wobbling
    /// after a hit and lying over while down. When the character's scene
    /// changed under it, the dummies draw without the character.
    /// The scene every figure of the field belongs to.
    #[must_use]
    pub fn scene(&self) -> Arc<TexturedScene> {
        self.scene.clone()
    }

    pub fn figure(
        &self,
        cast: Option<&Figure>,
        worn: Option<(usize, &[TexturedVertex])>,
        dummies: &[Dummy],
        now: f32,
    ) -> Figure {
        let mut vertices = Vec::with_capacity(
            self.dummy.len() * dummies.len()
                + self.cast.as_ref().map_or(0, |c| c.1)
                + self.forms.iter().sum::<usize>(),
        );
        if let Some((scene, count)) = &self.cast {
            match cast {
                Some(figure)
                    if worn.is_none()
                        && Arc::ptr_eq(scene, &figure.scene)
                        && figure.vertices.len() == *count =>
                {
                    vertices.extend(figure.vertices.iter().copied());
                }
                // Keeps the vertex count the scene expects, folded away.
                _ => vertices.extend(std::iter::repeat_n(FOLDED, *count)),
            }
        }
        for (index, &count) in self.forms.iter().enumerate() {
            match worn {
                Some((i, posed)) if i == index && posed.len() == count => {
                    vertices.extend_from_slice(posed);
                }
                _ => vertices.extend(std::iter::repeat_n(FOLDED, count)),
            }
        }
        for dummy in dummies {
            let transform = pose(dummy, now);
            let normals = Mat4::from_quat(Quat::from_mat4(&transform).normalize());
            vertices.extend(self.dummy.iter().map(|v| {
                TexturedVertex {
                    pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
                    normal: normals
                        .transform_vector3(Vec3::from(v.normal))
                        .normalize_or(Vec3::Y)
                        .to_array(),
                    ..*v
                }
            }));
        }
        Figure {
            scene: self.scene.clone(),
            vertices: Arc::new(vertices),
        }
    }
}

/// A dummy's model-to-world transform: its place and facing, a short
/// wobble after a hit, and lying on its back while down.
fn pose(dummy: &Dummy, now: f32) -> Mat4 {
    let since = now - dummy.hit_at;
    let wobble = if (0.0..0.6).contains(&since) {
        0.18 * (1.0 - since / 0.6) * (since * 30.0).sin()
    } else {
        0.0
    };
    let tilt = if dummy.down() || dummy.has(Condition::Prone, now) {
        -1.35
    } else {
        wobble
    };
    // Polymorph leaves a small, harmless thing where the dummy stood.
    let shrink = if dummy.has(Condition::Polymorphed, now) {
        0.4
    } else {
        1.0
    };
    Mat4::from_translation(dummy.pos)
        * Mat4::from_rotation_y(dummy.yaw)
        * Mat4::from_rotation_x(tilt)
        * Mat4::from_scale(Vec3::splat(dummy.kind.scale() * shrink))
}

/// A spell's drawn effect.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// A bolt in flight from `from` toward a dummy, landing at `start +
    /// flight`.
    Bolt {
        from: Vec3,
        target: usize,
        start: f32,
        flight: f32,
        /// Fire Bolt, Fireball, or another bolt spell.
        spell: Spell,
        /// The particles trailing it.
        trail: Option<crate::fx::Handle>,
    },
    /// A jagged lightning stroke from `from` to `to`.
    Strike { from: Vec3, to: Vec3, start: f32 },
    /// Thunderwave's blast erupting from the caster through its cube
    /// ([`super::thunder`]); `origin` is the cube's origin.
    Wave {
        origin: Vec3,
        forward: Vec3,
        start: f32,
    },
    /// Gust of Wind's streaks along its line.
    Gust {
        origin: Vec3,
        forward: Vec3,
        start: f32,
    },
    /// A puff of silver mist where Misty Step left or arrived.
    Mist { at: Vec3, start: f32 },
    /// A web over the ground at `at`, until `until`.
    Web { at: Vec3, start: f32, until: f32 },
    /// Long Rest's green rings rising around the druid.
    Rest { at: Vec3, start: f32 },
    /// A Wild Shape or Return to Form: a green swirl closing on the druid.
    Shift { at: Vec3, start: f32 },
}

impl Effect {
    /// Whether it has finished at `now`.
    #[must_use]
    pub fn done(&self, now: f32) -> bool {
        let (start, length) = match self {
            Self::Bolt { start, flight, .. } => (*start, *flight),
            Self::Wave { start, .. } => (*start, super::thunder::LENGTH),
            Self::Gust { start, .. } => (*start, 1.4),
            Self::Mist { start, .. } => (*start, 0.8),
            Self::Web { until, .. } => return now >= *until,
            Self::Rest { start, .. } => (*start, 1.2),
            Self::Shift { start, .. } => (*start, SHIFT),
            Self::Strike { start, .. } => (*start, 0.35),
        };
        now - start >= length
    }

    /// How many of its kind may be live at once; past it the oldest goes.
    #[must_use]
    pub const fn cap(&self) -> usize {
        match self {
            Self::Bolt { .. } => 24,
            Self::Mist { .. } => 8,
            Self::Wave { .. } => 6,
            Self::Gust { .. } | Self::Web { .. } => 6,
            Self::Rest { .. } | Self::Shift { .. } => 1,
            Self::Strike { .. } => 12,
        }
    }
}

/// The camera's jolt `age` seconds after a Thunderwave, m.
#[must_use]
pub fn shake(age: f32) -> Vec3 {
    super::thunder::shake(age)
}

/// Builds the Grove's lines and faces: the post, the bars, the numbers,
/// the target ring, and the effects, seen from `eye`.
pub struct Painter {
    pub mesh: Mesh,
    eye: Vec3,
}

impl Painter {
    pub fn new(eye: Vec3) -> Self {
        Self {
            mesh: Mesh::default(),
            eye,
        }
    }

    fn line(&mut self, a: Vec3, b: Vec3, color: [f32; 3]) {
        for p in [a, b] {
            self.mesh.lines.push(Vertex {
                pos: p.to_array(),
                color,
                fog: 1.0,
            });
        }
    }

    fn quad(&mut self, corners: [Vec3; 4], color: [f32; 3]) {
        let [a, b, c, d] = corners;
        for p in [a, b, c, a, c, d] {
            self.mesh.faces.push(Vertex {
                pos: p.to_array(),
                color,
                fog: 1.0,
            });
        }
    }

    /// The turn that makes text and bars at `at` face the camera.
    fn facing(&self, at: Vec3) -> Mat4 {
        crate::zones::everglade::floaters::facing(self.eye, at)
    }

    /// A camera-facing rectangle at `at` (its center), `size` m, pushed
    /// `depth` m toward the camera so later layers draw over earlier ones.
    fn panel(&mut self, at: Vec3, offset: [f32; 2], size: [f32; 2], depth: f32, color: [f32; 3]) {
        let m = self.facing(at);
        let [x, y] = offset;
        let [w, h] = size;
        // Board space faces -Z; the viewer's right is -X.
        let p = |u: f32, v: f32| m.transform_point3(Vec3::new(-(x + u), y + v, -depth));
        self.quad([p(0.0, 0.0), p(w, 0.0), p(w, h), p(0.0, h)], color);
    }

    /// Text centered over `at`, facing the camera.
    fn text(&mut self, at: Vec3, text: &str, height: f32, color: [f32; 3]) {
        crate::zones::everglade::floaters::text(&mut self.mesh, self.eye, at, text, height, color);
    }

    /// The flying target's post: a stone footing, a stout pole braced at
    /// its foot, and a platform under the target. It stands from the
    /// ground to the target's home, which a post never leaves.
    pub fn post(&mut self, dummy: &Dummy) {
        let (x, z) = (dummy.home.x, dummy.home.z);
        let ground = crate::zones::everglade::land(x, z);
        let top = dummy.home.y;
        let wood = [0.32, 0.2, 0.1];
        let stone = [0.46, 0.44, 0.4];
        boxed(
            &mut self.mesh,
            Vec3::new(x, ground - 0.05, z),
            [0.55, 0.35, 0.55],
            stone,
        );
        boxed(
            &mut self.mesh,
            Vec3::new(x, ground + 0.3, z),
            [0.22, top - ground - 0.3, 0.22],
            wood,
        );
        // Four short braces from the footing up the pole.
        for (dx, dz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            boxed(
                &mut self.mesh,
                Vec3::new(x + dx * 0.32, ground + 0.3, z + dz * 0.32),
                [0.07 + 0.05 * dz.abs(), 0.6, 0.07 + 0.05 * dx.abs()],
                [0.27, 0.17, 0.09],
            );
        }
        boxed(
            &mut self.mesh,
            Vec3::new(x, top - 0.16, z),
            [0.65, 0.16, 0.65],
            [0.42, 0.28, 0.14],
        );
    }

    /// A dummy's name, its health bar (green when healthy, through amber,
    /// to red), and a tag for each of its `conditions` with the seconds it
    /// has left.
    pub fn bar(&mut self, dummy: &Dummy, conditions: &[(Condition, f32)], targeted: bool) {
        let scale = dummy.kind.scale().max(1.0);
        let at = dummy.pos + Vec3::Y * (HEIGHT * dummy.kind.scale() + 0.35);
        let [w, h] = [BAR[0] * scale.sqrt(), BAR[1]];
        let frame = if targeted {
            [1.0, 0.82, 0.3]
        } else {
            [0.55, 0.42, 0.18]
        };
        let pad = 0.03;
        self.panel(
            at,
            [-w / 2.0 - pad, -pad],
            [w + 2.0 * pad, h + 2.0 * pad],
            0.0,
            frame,
        );
        self.panel(at, [-w / 2.0, 0.0], [w, h], 0.01, [0.22, 0.04, 0.05]);
        let k = dummy.fraction();
        if k > 0.0 {
            self.panel(at, [-w / 2.0, 0.0], [w * k, h], 0.02, health_color(k));
        }
        // Conditions in a row over the name, each a colored chip and its
        // tag.
        let row = at + Vec3::Y * (h + NAME + 0.2);
        let count = conditions.len().min(4);
        for (k, (condition, left)) in conditions.iter().take(4).enumerate() {
            let x = (k as f32 - (count as f32 - 1.0) / 2.0) * TAG;
            let color = condition.color();
            self.panel(
                row,
                [x - TAG / 2.0 + 0.03, 0.0],
                [TAG - 0.06, 0.2],
                0.0,
                color.map(|c| c * 0.35),
            );
            let label = format!("{} {}", condition.tag(), left.ceil() as i32);
            let m = self.facing(row);
            let center = m.transform_point3(Vec3::new(-x, 0.04, -0.02));
            self.text(center, &label, 0.13, color);
        }
        let name = if dummy.down() {
            format!("{} down", dummy.kind.name())
        } else {
            format!(
                "{} {}/{}",
                dummy.kind.name(),
                dummy.hp.ceil() as i32,
                dummy.kind.max_hp() as i32
            )
        };
        self.text(at + Vec3::Y * (h + 0.08), &name, NAME, [0.98, 0.92, 0.72]);
    }

    /// A number rising and fading over [`FLOAT`] seconds.
    pub fn floater(&mut self, floater: &Floater, now: f32) {
        crate::zones::everglade::floaters::paint(&mut self.mesh, self.eye, floater, now);
    }

    /// A gold ring on the ground under the soft target.
    pub fn target(&mut self, dummy: &Dummy, now: f32) {
        let r = 0.75 * dummy.kind.scale();
        let base = Vec3::new(dummy.pos.x, dummy.pos.y + 0.05, dummy.pos.z);
        let spin = now * 0.8;
        ring(self, base, r, 24, spin, [1.0, 0.8, 0.25]);
    }

    /// Faerie Fire's violet outline around a dummy.
    pub fn outline(&mut self, dummy: &Dummy, now: f32) {
        let scale = dummy.kind.scale();
        let color = [0.85, 0.55, 1.0].map(|c| c * (0.75 + 0.25 * (now * 6.0).sin()));
        let r = 0.45 * scale;
        let top = HEIGHT * scale;
        for i in 0..6 {
            let a = i as f32 / 6.0 * TAU + now * 0.8;
            let b = a + TAU / 6.0;
            let p = |a: f32, y: f32| dummy.pos + Vec3::new(a.cos() * r, y, a.sin() * r);
            self.line(p(a, 0.1), p(a, top), color);
            self.line(p(a, top), p(b, top), color);
            self.line(p(a, 0.1), p(b, 0.1), color);
        }
    }

    /// Web strands from the ground to a rooted dummy.
    pub fn strands(&mut self, dummy: &Dummy, now: f32) {
        let ground = crate::zones::everglade::land(dummy.pos.x, dummy.pos.z);
        let color = [0.88, 0.86, 1.0];
        for i in 0..8 {
            let a = i as f32 / 8.0 * TAU + 0.2 * (now * 0.5).sin();
            let foot = Vec3::new(
                dummy.pos.x + a.cos() * 1.1,
                ground + 0.03,
                dummy.pos.z + a.sin() * 1.1,
            );
            let body = dummy.pos + Vec3::Y * (0.3 + 0.18 * (i % 4) as f32 * dummy.kind.scale());
            self.line(foot, body, color);
        }
    }

    /// One effect at `now`; `target` is where a bolt's dummy is now.
    pub fn effect(&mut self, effect: &Effect, now: f32, target: Option<Vec3>) {
        match *effect {
            Effect::Bolt {
                from,
                start,
                flight,
                spell,
                trail,
                ..
            } => {
                let Some(to) = target else { return };
                let k = ((now - start) / flight.max(1e-3)).clamp(0.0, 1.0);
                let at = from.lerp(to, k);
                let back = from.lerp(to, (k - 0.08).max(0.0));
                let color = spell.def().kind.color();
                // A bolt with particles is drawn by them; the line keeps a
                // streak for when the particle budget runs out.
                if trail.is_none() {
                    let size = if spell == Spell::Fireball { 0.35 } else { 0.18 };
                    boxed(
                        &mut self.mesh,
                        at - Vec3::Y * size,
                        [size, size * 2.0, size],
                        [1.0, 0.62, 0.18],
                    );
                }
                self.line(back, at, color);
            }
            Effect::Strike { from, to, start } => {
                let k = ((now - start) / 0.35).clamp(0.0, 1.0);
                let color = [0.8, 0.88, 1.0].map(|c| c * (1.0 - k * k));
                let seed = (start * 1000.0) as u32;
                let mut last = from;
                for s in 1..=8 {
                    let t = s as f32 / 8.0;
                    let jitter = if s == 8 {
                        Vec3::ZERO
                    } else {
                        let h = |n: u32| {
                            ((seed.wrapping_mul(2_654_435_761).wrapping_add(n * 97)) % 1000) as f32
                                / 500.0
                                - 1.0
                        };
                        Vec3::new(h(s), 0.0, h(s + 50)) * 0.6
                    };
                    let p = from.lerp(to, t) + jitter;
                    self.line(last, p, color);
                    last = p;
                }
            }
            Effect::Wave {
                origin,
                forward,
                start,
            } => {
                // Each blast its own scatter, the same on every frame.
                let seed = u64::from(start.to_bits()) << 32
                    | u64::from((origin.x + origin.z * 7.0).to_bits());
                super::thunder::draw(&mut self.mesh, origin, forward, now - start, self.eye, seed);
            }
            Effect::Gust {
                origin,
                forward,
                start,
            } => {
                let t = now - start;
                let length = verse_world::gust::LENGTH as f32;
                let width = verse_world::gust::WIDTH as f32;
                let side = Vec3::new(-forward.z, 0.0, forward.x);
                let fade = (1.0 - t / 1.4).clamp(0.0, 1.0);
                for i in 0..14 {
                    let lane = (i as f32 / 13.0 - 0.5) * width;
                    let up = 0.3 + (i % 5) as f32 * 0.45;
                    let s = ((t * 22.0 + i as f32 * 3.7) % length).max(0.0);
                    let a = origin + side * lane + Vec3::Y * up + forward * s;
                    let b = a + forward * 1.6;
                    self.line(a, b, [0.7, 1.0, 0.95].map(|c| c * fade));
                }
            }
            Effect::Mist { at, start } => {
                let k = ((now - start) / 0.8).clamp(0.0, 1.0);
                for i in 0..3 {
                    let r = 0.4 + k * (0.8 + i as f32 * 0.4);
                    ring(
                        self,
                        at + Vec3::Y * (0.2 + i as f32 * 0.6 + k),
                        r,
                        16,
                        k,
                        [0.75, 0.88, 1.0].map(|c| c * (1.0 - k)),
                    );
                }
            }
            Effect::Web { at, start, until } => {
                let fade = ((until - now) / 1.0).clamp(0.0, 1.0);
                let grow = ((now - start) / 0.3).clamp(0.0, 1.0);
                let r = 3.0 * grow;
                let ground =
                    Vec3::new(at.x, crate::zones::everglade::land(at.x, at.z) + 0.05, at.z);
                let color = [0.86, 0.84, 1.0].map(|c| c * fade);
                for i in 0..10 {
                    let a = i as f32 / 10.0 * TAU;
                    self.line(ground, ground + Vec3::new(a.cos(), 0.0, a.sin()) * r, color);
                }
                for k in 1..=3 {
                    ring(self, ground, r * k as f32 / 3.0, 10, 0.0, color);
                }
            }
            Effect::Shift { at, start } => {
                let k = ((now - start) / SHIFT).clamp(0.0, 1.0);
                let color = [0.55, 1.0, 0.4].map(|c| c * (1.0 - k));
                for i in 0..4 {
                    let y = 0.2 + i as f32 * 0.5;
                    let r = 2.2 * (1.0 - k) + 0.3;
                    ring(self, at + Vec3::Y * y, r, 18, k * 6.0 + i as f32, color);
                }
                for i in 0..8 {
                    let a = i as f32 / 8.0 * TAU + k * 4.0;
                    let r = 2.0 * (1.0 - k) + 0.2;
                    let foot = at + Vec3::new(a.cos() * r, 0.05, a.sin() * r);
                    self.line(foot, foot + Vec3::Y * (1.8 * (1.0 - k) + 0.2), color);
                }
            }
            Effect::Rest { at, start } => {
                let k = ((now - start) / 1.2).clamp(0.0, 1.0);
                for i in 0..3 {
                    let y = (k * 2.4 + i as f32 * 0.6) % 2.4;
                    ring(
                        self,
                        at + Vec3::Y * y,
                        0.9,
                        20,
                        k * 2.0,
                        [0.35, 1.0, 0.45].map(|c| c * (1.0 - k)),
                    );
                }
            }
        }
    }
}

/// Width of a condition tag over a health bar, m.
const TAG: f32 = 0.62;

/// The color of a health bar `k` full: green, amber at half, red low.
#[must_use]
pub fn health_color(k: f32) -> [f32; 3] {
    let k = k.clamp(0.0, 1.0);
    if k >= 0.5 {
        let t = (k - 0.5) * 2.0;
        [0.95 - 0.75 * t, 0.75 + 0.1 * t, 0.12]
    } else {
        let t = k * 2.0;
        [0.9 + 0.05 * t, 0.12 + 0.63 * t, 0.1]
    }
}

/// A horizontal ring of `segments` lines at `center`.
fn ring(p: &mut Painter, center: Vec3, r: f32, segments: usize, phase: f32, color: [f32; 3]) {
    for i in 0..segments {
        if i % 4 == 3 {
            continue;
        }
        let a = phase + i as f32 / segments as f32 * TAU;
        let b = phase + (i + 1) as f32 / segments as f32 * TAU;
        p.line(
            center + Vec3::new(a.cos() * r, 0.0, a.sin() * r),
            center + Vec3::new(b.cos() * r, 0.0, b.sin() * r),
            color,
        );
    }
}

/// An axis-aligned box standing on `base`, `half`-wide in x and z and
/// `size[1]` tall, as shaded faces.
fn boxed(mesh: &mut Mesh, base: Vec3, size: [f32; 3], color: [f32; 3]) {
    let [hx, h, hz] = size;
    let c = |i: usize| {
        Vec3::new(
            base.x + if i & 1 == 0 { -hx } else { hx },
            base.y + if i & 2 == 0 { 0.0 } else { h },
            base.z + if i & 4 == 0 { -hz } else { hz },
        )
    };
    let c: [Vec3; 8] = std::array::from_fn(c);
    for [a, b, cc, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        let shaded = crate::zones::everglade::draw::shade(color, c[a], c[b], c[cc]);
        for p in [c[a], c[b], c[cc], c[a], c[cc], c[d]] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color: shaded,
                fog: 1.0,
            });
        }
    }
}
