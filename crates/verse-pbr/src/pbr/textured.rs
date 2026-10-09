//! Textured static meshes for the physical path.
//!
//! A zone that draws authored, image-textured props builds one
//! [`TexturedScene`]: base-color images, materials, meshes, and placements.
//! It hands the scene to the renderer on its world mesh
//! ([`crate::mesh::Mesh::textured`]), so the scene uploads once with the
//! zone's other static geometry and is released when the zone's world is
//! replaced.
//!
//! The renderer merges placements into [`CELL`]-meter cells per material, as
//! `imported::merge` does for the lair, and draws each cell with one indexed
//! call. A mesh placed many times with [`TexturedScene::place_instanced`]
//! uploads once instead and draws its copies as instances, in runs per cell
//! ([`super::instanced`]); OpenGL ES 3.0, WebGL2, and WebGPU all draw
//! instances, so every backend shares that path. Materials follow glTF 2.0's
//! metallic-roughness model at its simplest: a base-color image times a
//! linear factor and the vertex color, uniform metallic and roughness, an
//! alpha mode (opaque, masked at a cutoff, or blended), and a double-sided
//! flag. Normal, occlusion, metallic-roughness, and emissive images are not
//! read.
//!
//! A placement may draw only at some distances ([`Detail`]): a model's near
//! level of detail within one of the scene's switch distances and a lighter
//! far level beyond it, or small ground cover only near. Each level merges
//! into cells of its own; the renderer picks each cell's level from the
//! eye every frame, with [`HYSTERESIS`], and its shadows and depth prepass
//! draw the same levels.
//!
//! Textured meshes draw only in physical frames: a frame with a
//! [`super::Sky`], or a [`super::Neon`] stage with a studio [`super::Key`].
//! Opaque cells draw first, then masked cells, then blended cells from the
//! farthest to the nearest. Triangles inside one blended cell are not sorted.
//! Opaque and masked cells cast sun or key shadows; blended cells do not.
//!
//! The shaders are the textured entries of `photo.wgsl`. They use only the
//! OpenGL ES 3.0 features every backend requests (see `verse_gfx::gles`), so
//! desktops and phones draw the same thing.

use std::collections::BTreeMap;
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};

/// Side of the square ground cells that placements merge into, in meters.
pub const CELL: f32 = 8.0;
/// Largest base-color image side. OpenGL ES 3.0 guarantees 2048 texels; the
/// importer halves larger images until they fit.
pub const MAX_IMAGE_SIZE: u32 = 2048;
/// Most base-color images in one scene: a zone pack's 64 and, in
/// Everglade, the medieval kit's.
pub const MAX_IMAGES: usize = 96;
/// Most materials in one scene.
pub const MAX_MATERIALS: usize = 1024;
/// Most placements in one scene.
pub const MAX_PLACEMENTS: usize = 1 << 16;
/// Most bytes a scene keeps on the GPU, the zone geometry bound: merged
/// cells, shared meshes, instance records, and the light texture
/// ([`super::instanced::Layout::bytes`]). The vertices and the indices are
/// separate buffers, each well under wgpu's default 256 MiB buffer limit,
/// which phones and browsers keep.
pub const MAX_BYTES: usize = 224 * 1024 * 1024;
/// Most bytes of [`TexturedScene::merge`], which the light bake builds on
/// the CPU: every placement's vertices as [`TexturedVertex`]es and its
/// 32-bit indices, instances counted once each. Everglade's city merged to
/// 209 MiB when the GPU drew that merge; instancing took the merge off the
/// GPU, and this bound keeps the bake's memory within what phones and
/// browsers spare a worker.
pub const MAX_MERGE_BYTES: usize = 448 * 1024 * 1024;
/// The fewest instanced placements of one mesh that draw as instances
/// ([`TexturedScene::instanced`]).
pub const MIN_INSTANCES: usize = 2;
/// Most switch distances in one scene ([`TexturedScene::switches`]).
pub const MAX_SWITCHES: usize = 8;
/// How far past its switch distance a cell must move before it changes
/// level, m: a cell drawn near stays near until it is this much farther than
/// the switch, and a far one stays far until it is this much nearer, so a
/// cell at the switch does not flicker between its levels
/// ([`Level::near`]).
pub const HYSTERESIS: f32 = 2.5;

/// One vertex of a textured mesh.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct TexturedVertex {
    /// Position in meters: mesh space in a [`TexturedMesh`], world space once
    /// merged.
    pub pos: [f32; 3],
    /// Unit surface normal.
    pub normal: [f32; 3],
    /// Base-color image coordinate; the image repeats outside 0 to 1.
    pub uv: [f32; 2],
    /// Linear RGBA multiplier (glTF `COLOR_0`); white is `[255; 4]`.
    pub color: [u8; 4],
    /// Baked ambient light ([`crate::pbr::textured_bake`]): red, green, and
    /// blue multiply the frame's ambient irradiance by `4 × (byte / 255)²`,
    /// and alpha is the open sky fraction, which also occludes ambient
    /// reflections. An alpha of zero, [`UNBAKED`], leaves the ambient as is.
    pub light: [u8; 4],
}

/// The [`TexturedVertex::light`] of a vertex no bake has reached.
pub const UNBAKED: [u8; 4] = [0; 4];

impl TexturedVertex {
    /// A white vertex.
    #[must_use]
    pub fn new(pos: Vec3, normal: Vec3, uv: [f32; 2]) -> Self {
        Self {
            pos: pos.to_array(),
            normal: normal.to_array(),
            uv,
            color: [255; 4],
            light: UNBAKED,
        }
    }

    fn finite(&self) -> bool {
        self.pos
            .iter()
            .chain(&self.normal)
            .chain(&self.uv)
            .all(|x| x.is_finite())
    }
}

/// A base-color image.
#[derive(Clone, PartialEq, Eq)]
pub struct BaseColorImage {
    /// Where the image came from, for diagnostics; importers also use it to
    /// share one image between materials.
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8: sRGB-encoded color with linear alpha, as glTF stores
    /// base color.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for BaseColorImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BaseColorImage({} {}x{})",
            self.name, self.width, self.height
        )
    }
}

impl BaseColorImage {
    /// A one-texel image of `rgba`.
    #[must_use]
    pub fn solid(name: impl Into<String>, rgba: [u8; 4]) -> Self {
        Self {
            name: name.into(),
            width: 1,
            height: 1,
            rgba: rgba.to_vec(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_IMAGE_SIZE
            || self.height > MAX_IMAGE_SIZE
            || self.rgba.len() != self.width as usize * self.height as usize * 4
        {
            return Err(format!(
                "base-color image {} is not {MAX_IMAGE_SIZE} texels or smaller with RGBA8 data",
                self.name
            ));
        }
        Ok(())
    }
}

/// How a material's alpha is used (glTF `alphaMode`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlphaMode {
    /// Alpha is ignored.
    Opaque,
    /// A fragment is fully opaque when its alpha reaches `cutoff` and absent
    /// otherwise: leaves, grass, fences, and other cutouts.
    Mask { cutoff: f32 },
    /// Alpha blends the surface over what is behind it: glass and water.
    Blend,
}

impl AlphaMode {
    /// Whether a fragment whose base alpha is `alpha` is drawn. The masked
    /// fragment shader applies the same rule; a blended fragment is drawn at
    /// its alpha.
    #[must_use]
    pub fn keeps(self, alpha: f32) -> bool {
        let blend = match self {
            Self::Opaque => 0,
            Self::Mask { .. } => 1,
            Self::Blend => 2,
        };
        verse_engine::material::coverage(alpha, blend, self.cutoff()).is_some()
    }

    /// The pass the material draws in.
    #[must_use]
    pub fn pass(self) -> Pass {
        match self {
            Self::Opaque => Pass::Opaque,
            Self::Mask { .. } => Pass::Masked,
            Self::Blend => Pass::Blended,
        }
    }

    /// The cutoff the masked shaders compare against: 0 keeps every fragment.
    fn cutoff(self) -> f32 {
        match self {
            Self::Mask { cutoff } => cutoff,
            Self::Opaque | Self::Blend => 0.0,
        }
    }
}

/// The three textured passes, in drawing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pass {
    Opaque,
    Masked,
    Blended,
}

impl Pass {
    pub const ALL: [Self; 3] = [Self::Opaque, Self::Masked, Self::Blended];
}

/// A textured material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedMaterial {
    /// Index of the base-color image, or `None` for white.
    pub image: Option<usize>,
    /// Linear RGBA factor multiplied with the image, 0 to 1.
    pub base_color: [f32; 4],
    /// 0 for a dielectric, 1 for a metal.
    pub metallic: f32,
    /// Perceptual roughness, 0 to 1.
    pub roughness: f32,
    pub alpha: AlphaMode,
    /// Draw back faces too: leaves, cloth, and other single-sheet surfaces.
    /// Back faces shade with the normal turned toward the eye.
    pub double_sided: bool,
    /// Emitted luminance per unit of base color, cd/m²: a flame, embers, or
    /// a glowing liquid. Zero emits nothing.
    pub emissive: f32,
}

impl Default for TexturedMaterial {
    /// A white, fully rough, opaque, single-sided dielectric.
    fn default() -> Self {
        Self {
            image: None,
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            alpha: AlphaMode::Opaque,
            double_sided: false,
            emissive: 0.0,
        }
    }
}

impl TexturedMaterial {
    fn validate(&self, images: usize) -> Result<(), String> {
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if self.image.is_some_and(|i| i >= images)
            || !self.base_color.iter().all(|&x| unit(x))
            || !unit(self.metallic)
            || !unit(self.roughness)
            || !unit(self.alpha.cutoff())
            || !(self.emissive.is_finite() && (0.0..=MAX_EMISSIVE).contains(&self.emissive))
        {
            return Err("textured material has an invalid image or factor".into());
        }
        Ok(())
    }
}

/// Triangles that share one material.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Primitive {
    pub vertices: Vec<TexturedVertex>,
    /// Triangle list indices into `vertices`, counterclockwise front faces.
    pub indices: Vec<u32>,
    /// Index of the material.
    pub material: usize,
}

/// A static mesh: its primitives in mesh space, meters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturedMesh {
    pub primitives: Vec<Primitive>,
}

impl TexturedMesh {
    fn validate(&self, materials: usize) -> Result<(), String> {
        for p in &self.primitives {
            if p.material >= materials
                || !p.indices.len().is_multiple_of(3)
                || p.indices.iter().any(|&i| i as usize >= p.vertices.len())
                || !p.vertices.iter().all(TexturedVertex::finite)
            {
                return Err("textured mesh has an invalid primitive".into());
            }
        }
        Ok(())
    }
}

/// One copy of a mesh in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Index of the mesh.
    pub mesh: usize,
    /// Mesh space to world space. Its translation picks the cell.
    pub transform: Mat4,
    /// The distances it draws at.
    pub detail: Detail,
    /// Whether it may draw as an instance of its mesh, sharing the mesh's
    /// uploaded triangles with the other copies, rather than merged into
    /// its cell ([`TexturedScene::place_instanced`]). An instanced
    /// placement has no index ranges, so a zone cannot hide or carve it.
    pub instanced: bool,
}

/// The distances a placement draws at, by its cell's distance from the eye
/// against one of the scene's [`TexturedScene::switches`]: a model's near
/// level of detail is `Near(i)` and its lighter far level `Far(i)`, so each
/// cell draws exactly one of them; small ground cover that is not worth
/// drawing far off is `Near(i)` alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Detail {
    /// At every distance.
    #[default]
    Always,
    /// While its cell is nearer than switch `i`.
    Near(u8),
    /// While its cell is at or beyond switch `i`.
    Far(u8),
    /// One whole object's near, middle, far, or original-piece fallback level.
    Group { group: u16, level: u8 },
}

impl Detail {
    /// The index of its switch distance, if it has one.
    #[must_use]
    pub fn switch(self) -> Option<usize> {
        match self {
            Self::Always => None,
            Self::Near(i) | Self::Far(i) => Some(usize::from(i)),
            Self::Group { .. } => None,
        }
    }
}

/// A merged cell's level of detail: [`Detail`] with its switch distance and
/// the center of its cell, which every level of that cell measures from, so
/// a cell's near and far levels always change together.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Always,
    Near {
        anchor: [f32; 2],
        switch: f32,
    },
    Far {
        anchor: [f32; 2],
        switch: f32,
    },
    Group {
        group: u16,
        anchor: [f32; 2],
        switches: [f32; 2],
        level: u8,
        fallback: u8,
    },
}

impl Level {
    /// Whether this level's cell counts as near from `eye`: within the switch
    /// distance across the ground. `was` is the answer the last frame gave,
    /// which holds until the eye crosses [`HYSTERESIS`] past the switch; `None`
    /// decides by the switch alone.
    #[must_use]
    pub fn near(self, eye: Vec3, was: Option<bool>) -> bool {
        let (anchor, switch) = match self {
            Self::Always => return true,
            Self::Near { anchor, switch } | Self::Far { anchor, switch } => (anchor, switch),
            Self::Group {
                anchor,
                switches,
                level,
                ..
            } => {
                // A group's history belongs to the shared selector, never to
                // separate batches. Without it, choose the exact distance.
                return DetailGroup {
                    anchor,
                    switches,
                    fallback: 0,
                }
                .selected(eye, None)
                    == level;
            }
        };
        let distance = (eye.x - anchor[0]).hypot(eye.z - anchor[1]);
        match was {
            None => distance < switch,
            Some(true) => distance < switch + HYSTERESIS,
            Some(false) => distance < switch - HYSTERESIS,
        }
    }

    /// Whether a cell of this level draws when its cell is `near`.
    #[must_use]
    pub fn drawn(self, near: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Near { .. } => near,
            Self::Far { .. } => !near,
            Self::Group { .. } => near,
        }
    }

    /// Whether the cell draws from `eye`, with no earlier frame to hold it.
    #[must_use]
    pub fn drawn_from(self, eye: Vec3) -> bool {
        self.drawn(self.near(eye, None))
    }

    /// Uses the original pieces at every distance after the object changes.
    #[must_use]
    pub fn drawn_with_fallback(
        self,
        selected: bool,
        groups: &std::collections::BTreeSet<u16>,
    ) -> bool {
        match self {
            Self::Group {
                group,
                level,
                fallback,
                ..
            } if groups.contains(&group) => level == fallback,
            _ => self.drawn(selected),
        }
    }
}

/// Distance levels that share one object anchor and one destruction fallback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailGroup {
    pub anchor: [f32; 2],
    pub switches: [f32; 2],
    /// The original pieces: near (0), or a separate fallback (3).
    pub fallback: u8,
}

impl DetailGroup {
    /// Chooses exactly one level, retaining the shared earlier level within
    /// its hysteresis band. A teleport can cross both switches at once.
    #[must_use]
    pub fn selected(self, eye: Vec3, was: Option<u8>) -> u8 {
        let distance = (eye.x - self.anchor[0]).hypot(eye.z - self.anchor[1]);
        match was {
            Some(0) if distance < self.switches[0] + HYSTERESIS => return 0,
            Some(1)
                if distance >= self.switches[0] - HYSTERESIS
                    && distance < self.switches[1] + HYSTERESIS =>
            {
                return 1;
            }
            Some(2) if distance >= self.switches[1] - HYSTERESIS => return 2,
            _ => {}
        }
        if distance < self.switches[0] {
            0
        } else if distance < self.switches[1] {
            1
        } else {
            2
        }
    }
}

/// Everything a zone's textured static geometry needs, in one value.
#[derive(Clone, Default, PartialEq)]
pub struct TexturedScene {
    pub images: Vec<BaseColorImage>,
    pub materials: Vec<TexturedMaterial>,
    pub meshes: Vec<TexturedMesh>,
    pub placements: Vec<Placement>,
    /// The distances, in meters across the ground, at which placements
    /// change level ([`Detail`]), at most [`MAX_SWITCHES`].
    pub switches: Vec<f32>,
    pub detail_groups: Vec<DetailGroup>,
    /// Where a background light bake delivers this scene's merged vertices
    /// with their light channel filled
    /// ([`crate::pbr::textured_bake::SceneBaker`]); the renderer writes them
    /// over the uploaded vertices once.
    pub baked: BakedVertices,
    /// Rewrites of ranges of the merged indices, such as a zone hiding the
    /// placements of a building it now draws itself
    /// ([`Self::index_ranges`]); the renderer applies each change once.
    pub edits: IndexEdits,
}

/// One placement's primitive in a merged scene: its triangles' indices
/// from `first`, `count` of them, which address the merged vertices from
/// `base` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexRange {
    pub first: u32,
    pub count: u32,
    pub base: u32,
    /// The primitive's index in its mesh.
    pub primitive: usize,
}

/// Ranges of a scene's merged indices rewritten after the upload, shared
/// between a zone and the renderer. Each range keeps its latest contents
/// and the revision that wrote them, so an upload of the same scene
/// replays every range and a renderer that has seen revision `n` applies
/// only the later ones.
///
/// Slots always compare equal: they carry changes, not scene content.
#[derive(Clone, Default)]
pub struct IndexEdits(std::sync::Arc<std::sync::Mutex<IndexEditState>>);

#[derive(Default)]
struct IndexEditState {
    revision: u64,
    /// The first merged index of each range, its revision, and its indices.
    ranges: BTreeMap<u32, (u64, Vec<u32>)>,
    fallback_groups: std::collections::BTreeSet<u16>,
}

impl IndexEdits {
    /// Replaces the object groups that must draw their original pieces.
    pub fn set_group_fallbacks(&self, groups: std::collections::BTreeSet<u16>) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.fallback_groups != groups {
            state.fallback_groups = groups;
            state.revision += 1;
        }
    }

    #[must_use]
    pub fn group_fallbacks(&self) -> std::collections::BTreeSet<u16> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fallback_groups
            .clone()
    }
    /// Sets the merged indices from `first` on to `indices`.
    pub fn write(&self, first: u32, indices: Vec<u32>) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.revision += 1;
        let revision = state.revision;
        state.ranges.insert(first, (revision, indices));
    }

    /// The latest revision written.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .revision
    }

    /// Every range written after revision `seen`, and the latest revision.
    #[must_use]
    pub fn since(&self, seen: u64) -> (Vec<(u32, Vec<u32>)>, u64) {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let changed = state
            .ranges
            .iter()
            .filter(|(_, (revision, _))| *revision > seen)
            .map(|(first, (_, indices))| (*first, indices.clone()))
            .collect();
        (changed, state.revision)
    }
}

impl PartialEq for IndexEdits {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for IndexEdits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IndexEdits")
    }
}

/// Deliveries of baked light, shared between a bake and the renderer: the
/// light texture's texels and the lamp texture's, one a vertex in
/// [`TexturedScene::merge`]'s order. A zone may deliver again, such as when
/// the sun moves to another baked direction; the renderer takes each
/// delivery once.
///
/// Slots always compare equal: they carry a delivery, not scene content.
#[derive(Clone, Default)]
pub struct BakedVertices(std::sync::Arc<std::sync::Mutex<Delivery>>);

#[derive(Default)]
struct Delivery {
    lights: Option<Vec<[u8; 4]>>,
    lamps: Option<Vec<[u8; 4]>>,
    /// Whether to keep a copy of the light channel delivered, and that
    /// copy ([`BakedVertices::keep_delivered`]).
    keep: bool,
    kept: Option<std::sync::Arc<Vec<[u8; 4]>>>,
}

impl BakedVertices {
    fn lock(&self) -> std::sync::MutexGuard<'_, Delivery> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Hands over the merged vertices with their light channel filled.
    pub fn deliver(&self, vertices: Vec<TexturedVertex>) {
        self.deliver_lights(vertices.into_iter().map(|v| v.light).collect());
    }

    /// Hands over the light channel of every merged vertex.
    pub fn deliver_lights(&self, lights: Vec<[u8; 4]>) {
        let mut delivery = self.lock();
        if delivery.keep {
            delivery.kept = Some(std::sync::Arc::new(lights.clone()));
        }
        delivery.lights = Some(lights);
    }

    /// Keeps a copy of each light channel delivered from now on, for a
    /// zone that relights what it hides ([`crate::pbr::relight`]).
    pub fn keep_delivered(&self) {
        self.lock().keep = true;
    }

    /// The last light channel delivered since [`Self::keep_delivered`],
    /// whether or not the renderer took it.
    #[must_use]
    pub fn delivered(&self) -> Option<std::sync::Arc<Vec<[u8; 4]>>> {
        self.lock().kept.clone()
    }

    /// Hands over the lamp light of every merged vertex
    /// ([`crate::pbr::baked_layers::encode_lamp`]).
    pub fn deliver_lamps(&self, lamps: Vec<[u8; 4]>) {
        self.lock().lamps = Some(lamps);
    }

    /// Takes the delivered light channel, if a bake has delivered one since
    /// the last take.
    #[must_use]
    pub fn take(&self) -> Option<Vec<[u8; 4]>> {
        self.lock().lights.take()
    }

    /// Takes the delivered lamp light, if any arrived since the last take.
    #[must_use]
    pub fn take_lamps(&self) -> Option<Vec<[u8; 4]>> {
        self.lock().lamps.take()
    }
}

impl PartialEq for BakedVertices {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for BakedVertices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BakedVertices")
    }
}

impl std::fmt::Debug for TexturedScene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TexturedScene({} images, {} materials, {} meshes, {} placements)",
            self.images.len(),
            self.materials.len(),
            self.meshes.len(),
            self.placements.len()
        )
    }
}

impl TexturedScene {
    /// Adds an image and returns its index.
    pub fn add_image(&mut self, image: BaseColorImage) -> usize {
        self.images.push(image);
        self.images.len() - 1
    }

    /// Adds a material and returns its index.
    pub fn add_material(&mut self, material: TexturedMaterial) -> usize {
        self.materials.push(material);
        self.materials.len() - 1
    }

    /// Adds a mesh and returns its index.
    pub fn add_mesh(&mut self, mesh: TexturedMesh) -> usize {
        self.meshes.push(mesh);
        self.meshes.len() - 1
    }

    /// Places a copy of mesh `mesh` in the world.
    pub fn place(&mut self, mesh: usize, transform: Mat4) {
        self.place_detail(mesh, transform, Detail::Always);
    }

    /// Places a copy of mesh `mesh` that draws only at `detail`'s distances.
    pub fn place_detail(&mut self, mesh: usize, transform: Mat4, detail: Detail) {
        self.placements.push(Placement {
            mesh,
            transform,
            detail,
            instanced: false,
        });
    }

    /// Places a copy of mesh `mesh`, as [`Self::place_detail`] does, that
    /// may draw as an instance: when at least [`MIN_INSTANCES`] instanced
    /// placements share its mesh, the renderer uploads the mesh once and
    /// draws every copy from it, so repeated trees, props, and kit pieces
    /// cost their triangles once ([`super::instanced`]). A zone that edits
    /// a placement's indices ([`Self::index_ranges`]) places it with
    /// [`Self::place_detail`] instead.
    pub fn place_instanced(&mut self, mesh: usize, transform: Mat4, detail: Detail) {
        self.placements.push(Placement {
            mesh,
            transform,
            detail,
            instanced: true,
        });
    }

    /// Whether each placement draws as an instance: it was placed with
    /// [`Self::place_instanced`], and at least [`MIN_INSTANCES`] such
    /// placements share its mesh. A mesh placed once gains nothing from
    /// instancing and merges into its cell.
    #[must_use]
    pub fn instanced(&self) -> Vec<bool> {
        let mut counts = vec![0usize; self.meshes.len()];
        for p in &self.placements {
            if p.instanced
                && let Some(count) = counts.get_mut(p.mesh)
            {
                *count += 1;
            }
        }
        self.placements
            .iter()
            .map(|p| p.instanced && counts.get(p.mesh).is_some_and(|&n| n >= MIN_INSTANCES))
            .collect()
    }

    /// The level a cell of `detail` at `cell` draws at.
    pub(crate) fn level(&self, detail: Detail, cell: (i32, i32)) -> Level {
        let anchor = [(cell.0 as f32 + 0.5) * CELL, (cell.1 as f32 + 0.5) * CELL];
        match detail {
            Detail::Always => Level::Always,
            Detail::Near(i) => Level::Near {
                anchor,
                switch: self.switches[usize::from(i)],
            },
            Detail::Far(i) => Level::Far {
                anchor,
                switch: self.switches[usize::from(i)],
            },
            Detail::Group { group, level } => {
                let g = self.detail_groups[usize::from(group)];
                Level::Group {
                    group,
                    anchor: g.anchor,
                    switches: g.switches,
                    level,
                    fallback: g.fallback,
                }
            }
        }
    }

    /// Merges one object's pieces at its anchor, while preserving their transforms.
    pub(crate) fn placement_cell(&self, placement: &Placement) -> (i32, i32) {
        match placement.detail {
            Detail::Group { group, .. } => {
                let anchor = self.detail_groups[usize::from(group)].anchor;
                (
                    (anchor[0] / CELL).floor() as i32,
                    (anchor[1] / CELL).floor() as i32,
                )
            }
            _ => cell_of(placement.transform),
        }
    }

    /// Checks every index, factor, and bound.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first part that is out of bounds.
    pub fn validate(&self) -> Result<(), String> {
        if self.images.len() > MAX_IMAGES
            || self.materials.len() > MAX_MATERIALS
            || self.placements.len() > MAX_PLACEMENTS
            || self.switches.len() > MAX_SWITCHES
            || self.detail_groups.len() > 384
        {
            return Err("textured scene exceeds its image, material, or placement bound".into());
        }
        if !self.switches.iter().all(|s| s.is_finite() && *s > 0.0) {
            return Err("textured scene has an invalid switch distance".into());
        }
        if !self.detail_groups.iter().all(|g| {
            g.anchor.iter().all(|a| a.is_finite())
                && g.switches.iter().all(|s| s.is_finite() && *s > 0.0)
                && g.switches[0] < g.switches[1]
                && matches!(g.fallback, 0 | 3)
        }) {
            return Err("textured scene has an invalid detail group".into());
        }
        for image in &self.images {
            image.validate()?;
        }
        for material in &self.materials {
            material.validate(self.images.len())?;
        }
        for mesh in &self.meshes {
            mesh.validate(self.materials.len())?;
        }
        for placement in &self.placements {
            if placement.mesh >= self.meshes.len()
                || !placement.transform.is_finite()
                || placement.transform.determinant().abs() < 1e-12
                || placement
                    .detail
                    .switch()
                    .is_some_and(|i| i >= self.switches.len())
                || matches!(placement.detail, Detail::Group { group, level }
                    if usize::from(group) >= self.detail_groups.len() || level > 3)
            {
                return Err("textured placement has an invalid mesh or transform".into());
            }
        }
        let layout = super::instanced::Layout::of(self);
        if layout.bytes() > MAX_BYTES as u64 || layout.merge_bytes() > MAX_MERGE_BYTES as u64 {
            return Err("textured scene exceeds its GPU bounds".into());
        }
        Ok(())
    }

    /// Distinct image/cutoff recipes; an opaque user never inherits another material's mask.
    pub fn mip_variants(&self) -> std::collections::BTreeSet<verse_engine::mips::Variant> {
        use verse_engine::mips::{Role, Variant};
        let mut variants: std::collections::BTreeSet<_> = self
            .materials
            .iter()
            .filter_map(|material| {
                material.image.map(|texture| Variant {
                    texture,
                    role: material_role(material),
                })
            })
            .collect();
        for texture in 0..self.images.len() {
            if !variants.iter().any(|v| v.texture == texture) {
                variants.insert(Variant {
                    texture,
                    role: Role::Color,
                });
            }
        }
        variants
    }

    /// Merges every placement into world-space cells, one per pass,
    /// material, [`CELL`], and [`Detail`]. A mirroring transform reverses its
    /// triangles' winding so front faces stay counterclockwise.
    ///
    /// The placements that draw merged come first, in the cells the renderer
    /// uploads ([`super::instanced::Prepared`]), so their indices are the
    /// ones [`Self::index_ranges`] finds. The instanced placements
    /// ([`Self::instanced`]) follow in cells of their own: the renderer
    /// draws them from their shared meshes, and this order numbers their
    /// vertices for the light bake, whose light each instance reads.
    ///
    /// # Errors
    ///
    /// Returns the validation error when the scene is out of bounds.
    pub fn merge(&self) -> Result<Merged, String> {
        self.validate()?;
        let instanced = self.instanced();
        let mut merged = Merged::default();
        self.merge_into(&mut merged, |i| !instanced[i]);
        self.merge_into(&mut merged, |i| instanced[i]);
        Ok(merged)
    }

    /// Merges the placements `which` keeps into world-space cells, appended
    /// to `merged`.
    pub(crate) fn merge_into(&self, merged: &mut Merged, which: impl Fn(usize) -> bool) {
        type Cell = (Vec<TexturedVertex>, Vec<u32>);
        let mut cells: BTreeMap<(Pass, usize, i32, i32, Detail), Cell> = BTreeMap::new();
        for (index, placement) in self.placements.iter().enumerate() {
            if !which(index) {
                continue;
            }
            let t = placement.transform;
            let normals = Mat3::from_mat4(t).inverse().transpose();
            let mirrored = t.determinant() < 0.0;
            let cell = self.placement_cell(placement);
            for p in &self.meshes[placement.mesh].primitives {
                let pass = self.materials[p.material].alpha.pass();
                let (vertices, indices) = cells
                    .entry((pass, p.material, cell.0, cell.1, placement.detail))
                    .or_default();
                let offset = vertices.len() as u32;
                vertices.extend(p.vertices.iter().map(|v| {
                    TexturedVertex {
                        pos: t.transform_point3(Vec3::from(v.pos)).to_array(),
                        normal: (normals * Vec3::from(v.normal))
                            .normalize_or(Vec3::Y)
                            .to_array(),
                        ..*v
                    }
                }));
                for triangle in p.indices.chunks_exact(3) {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| i + offset);
                    indices.extend(if mirrored { [a, c, b] } else { [a, b, c] });
                }
            }
        }
        for ((_, material, x, z, detail), (vertices, indices)) in cells {
            if indices.is_empty() {
                continue;
            }
            let base = merged.vertices.len() as u32;
            let first = merged.indices.len() as u32;
            let (min, max) = bounds(&vertices);
            merged.indices.extend(indices.iter().map(|i| i + base));
            merged.vertices.extend(vertices);
            merged.batches.push(Batch {
                material,
                first,
                count: merged.indices.len() as u32 - first,
                min,
                max,
                level: self.level(detail, (x, z)),
                run: None,
            });
        }
    }

    /// Bytes the renderer keeps on the GPU for this scene: vertices,
    /// indices, instance records, and the light texture
    /// ([`super::instanced::Layout`]).
    #[must_use]
    pub fn gpu_bytes(&self) -> u64 {
        super::instanced::Layout::of(self).bytes()
    }

    /// What a frame seen through `view_proj` from `eye` draws of this scene
    /// when fog is total at `far` meters, each cell and run of instances at
    /// the level it draws at from there, counted as the renderer draws it.
    #[must_use]
    pub fn frame_cost(&self, view_proj: Mat4, eye: Vec3, far: f32) -> FrameCost {
        super::instanced::Prepared::of_scene(self)
            .map(|prepared| prepared.frame_cost(view_proj, eye, far))
            .unwrap_or_default()
    }

    /// Where each placement's triangles land in [`Self::merge`]'s indices:
    /// for every placement that draws merged, one range per primitive with
    /// triangles, and none for an instanced one ([`Self::instanced`]). Only
    /// counts are taken, so this is cheap beside a merge.
    #[must_use]
    pub fn index_ranges(&self) -> Vec<Vec<IndexRange>> {
        type Key = (Pass, usize, i32, i32, Detail);
        // Each cell's index and vertex counts so far.
        let mut counts: BTreeMap<Key, (u32, u32)> = BTreeMap::new();
        let mut local: Vec<Vec<(Key, u32, u32, u32, usize)>> =
            Vec::with_capacity(self.placements.len());
        let instanced = self.instanced();
        for (placement, &instanced) in self.placements.iter().zip(&instanced) {
            let mut ranges = Vec::new();
            // An instance draws from its shared mesh: nothing to rewrite.
            if instanced {
                local.push(ranges);
                continue;
            }
            let cell = self.placement_cell(placement);
            let primitives = self
                .meshes
                .get(placement.mesh)
                .map_or(&[][..], |m| &m.primitives);
            for (index, p) in primitives.iter().enumerate() {
                let Some(material) = self.materials.get(p.material) else {
                    continue;
                };
                let key = (
                    material.alpha.pass(),
                    p.material,
                    cell.0,
                    cell.1,
                    placement.detail,
                );
                let count = (p.indices.len() / 3 * 3) as u32;
                let (indices, vertices) = counts.entry(key).or_default();
                if count > 0 {
                    ranges.push((key, *indices, count, *vertices, index));
                }
                *indices += count;
                *vertices += p.vertices.len() as u32;
            }
            local.push(ranges);
        }
        // Cells without triangles add nothing to the merge.
        let mut bases: BTreeMap<Key, (u32, u32)> = BTreeMap::new();
        let (mut first, mut base) = (0u32, 0u32);
        for (key, (indices, vertices)) in counts {
            if indices > 0 {
                bases.insert(key, (first, base));
                first += indices;
                base += vertices;
            }
        }
        local
            .into_iter()
            .map(|ranges| {
                ranges
                    .into_iter()
                    .map(|(key, at, count, offset, primitive)| {
                        let (first, base) = bases[&key];
                        IndexRange {
                            first: first + at,
                            count,
                            base: base + offset,
                            primitive,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// The merged indices `range` of placement `placement` holds, as
    /// [`Self::merge`] writes them.
    #[must_use]
    pub fn range_indices(&self, placement: usize, range: &IndexRange) -> Vec<u32> {
        let Some(p) = self.placements.get(placement) else {
            return Vec::new();
        };
        let mirrored = p.transform.determinant() < 0.0;
        let Some(primitive) = self
            .meshes
            .get(p.mesh)
            .and_then(|m| m.primitives.get(range.primitive))
        else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(range.count as usize);
        for triangle in primitive.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| i + range.base);
            out.extend(if mirrored { [a, c, b] } else { [a, b, c] });
        }
        out.truncate(range.count as usize);
        out
    }

    /// Imports a static glTF 2.0 file as one mesh and returns its index;
    /// place it with [`Self::place`]. Node transforms are applied, so the mesh
    /// is in the file's scene space (meters, Y up). Each glTF material adds a
    /// [`TexturedMaterial`] with its base-color image (PNG only, shared by
    /// path with earlier imports and halved to [`MAX_IMAGE_SIZE`]), factors,
    /// alpha mode, and double-sided flag.
    ///
    /// Metallic-roughness images are not read, so a material that has one
    /// imports as a dielectric at its roughness factor rather than as the
    /// uniform metal its factor alone would describe.
    ///
    /// # Errors
    ///
    /// Returns a message when the file cannot be read, a primitive is not a
    /// triangle list with normals, or an image is not a PNG.
    pub fn import_gltf(&mut self, path: &Path) -> Result<usize, String> {
        let gltf = gltf::Gltf::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.import_document(path, gltf, path.parent())
    }

    /// Imports a binary glTF file held in memory, as [`Self::import_gltf`]
    /// does from disk. `label` names the file in messages and keys its
    /// images; the file must carry its buffers and images inside it.
    ///
    /// # Errors
    ///
    /// Returns a message when the bytes are not a self-contained glTF file,
    /// a primitive is not a triangle list with normals, or an image is not a
    /// PNG.
    pub fn import_glb(&mut self, label: &str, bytes: &[u8]) -> Result<usize, String> {
        let path = Path::new(label);
        let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| format!("{label}: {e}"))?;
        let external = gltf
            .document
            .images()
            .any(|image| matches!(image.source(), gltf::image::Source::Uri { .. }))
            || gltf
                .document
                .buffers()
                .any(|buffer| matches!(buffer.source(), gltf::buffer::Source::Uri(_)));
        if external {
            return Err(format!("{label}: the file refers to files outside it"));
        }
        self.import_document(path, gltf, None)
    }

    fn import_document(
        &mut self,
        path: &Path,
        gltf: gltf::Gltf,
        base: Option<&Path>,
    ) -> Result<usize, String> {
        let fail = |message: &dyn std::fmt::Display| format!("{}: {message}", path.display());
        let buffers =
            gltf::import_buffers(&gltf.document, base, gltf.blob).map_err(|e| fail(&e))?;
        let document = gltf.document;
        let scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .ok_or_else(|| fail(&"the file has no scene"))?;
        let mut materials = BTreeMap::new();
        let mut mesh = TexturedMesh::default();
        let mut stack: Vec<_> = scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
        let mut visited = 0;
        while let Some((node, parent)) = stack.pop() {
            visited += 1;
            if visited > 4096 {
                return Err(fail(&"the node hierarchy exceeds 4096 nodes"));
            }
            let transform = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
            stack.extend(node.children().map(|child| (child, transform)));
            let Some(source) = node.mesh() else {
                continue;
            };
            let normals_to_world = Mat3::from_mat4(transform).inverse().transpose();
            let mirrored = transform.determinant() < 0.0;
            for primitive in source.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(fail(&"a primitive is not a triangle list"));
                }
                let reader = primitive.reader(|b| Some(&buffers[b.index()].0));
                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .ok_or_else(|| fail(&"a primitive has no positions"))?
                    .collect();
                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .ok_or_else(|| fail(&"a primitive has no normals"))?
                    .collect();
                let count = positions.len();
                let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
                    Some(uvs) => uvs.into_f32().collect(),
                    None => vec![[0.0; 2]; count],
                };
                let colors: Vec<[u8; 4]> = match reader.read_colors(0) {
                    Some(colors) => colors.into_rgba_u8().collect(),
                    None => vec![[255; 4]; count],
                };
                if normals.len() != count || uvs.len() != count || colors.len() != count {
                    return Err(fail(&"a primitive's attributes have different lengths"));
                }
                let mut indices: Vec<u32> = match reader.read_indices() {
                    Some(indices) => indices.into_u32().collect(),
                    None => (0..count as u32).collect(),
                };
                if mirrored {
                    for triangle in indices.chunks_exact_mut(3) {
                        triangle.swap(1, 2);
                    }
                }
                let material =
                    self.import_material(path, &buffers, primitive.material(), &mut materials)?;
                let vertices = (0..count)
                    .map(|i| TexturedVertex {
                        pos: transform
                            .transform_point3(Vec3::from(positions[i]))
                            .to_array(),
                        normal: (normals_to_world * Vec3::from(normals[i]))
                            .normalize_or(Vec3::Y)
                            .to_array(),
                        uv: uvs[i],
                        color: colors[i],
                        light: UNBAKED,
                    })
                    .collect();
                mesh.primitives.push(Primitive {
                    vertices,
                    indices,
                    material,
                });
            }
        }
        mesh.validate(self.materials.len()).map_err(|e| fail(&e))?;
        Ok(self.add_mesh(mesh))
    }

    fn import_material(
        &mut self,
        path: &Path,
        buffers: &[gltf::buffer::Data],
        material: gltf::Material<'_>,
        cache: &mut BTreeMap<Option<usize>, usize>,
    ) -> Result<usize, String> {
        if let Some(&index) = cache.get(&material.index()) {
            return Ok(index);
        }
        let pbr = material.pbr_metallic_roughness();
        let alpha = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask {
                cutoff: material.alpha_cutoff().unwrap_or(0.5),
            },
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        };
        let role = match alpha {
            AlphaMode::Mask { cutoff } => {
                verse_engine::mips::Role::masked(cutoff, pbr.base_color_factor()[3])?
            }
            _ => verse_engine::mips::Role::Color,
        };
        let image = match pbr.base_color_texture() {
            Some(info) if info.tex_coord() != 0 => {
                return Err(format!(
                    "{}: a base-color image uses a second texture coordinate set",
                    path.display()
                ));
            }
            Some(info) => Some(self.import_image(path, buffers, info.texture().source(), role)?),
            None => None,
        };
        let metallic = if pbr.metallic_roughness_texture().is_some() {
            0.0
        } else {
            pbr.metallic_factor()
        };
        let imported = TexturedMaterial {
            image,
            base_color: pbr.base_color_factor(),
            metallic,
            roughness: pbr.roughness_factor(),
            alpha,
            double_sided: material.double_sided(),
            emissive: emissive(&material),
        };
        imported
            .validate(self.images.len())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let index = self.add_material(imported);
        cache.insert(material.index(), index);
        Ok(index)
    }

    fn import_image(
        &mut self,
        path: &Path,
        buffers: &[gltf::buffer::Data],
        image: gltf::image::Image<'_>,
        role: verse_engine::mips::Role,
    ) -> Result<usize, String> {
        let name = match image.source() {
            gltf::image::Source::Uri { uri, .. } => {
                if uri.starts_with("data:") {
                    return Err(format!(
                        "{}: embedded data URIs are not supported",
                        path.display()
                    ));
                }
                path.parent()
                    .unwrap_or(Path::new("."))
                    .join(uri)
                    .display()
                    .to_string()
            }
            gltf::image::Source::View { .. } => {
                format!("{}#image{}", path.display(), image.index())
            }
        };
        let recipe = match role {
            verse_engine::mips::Role::Mask { cutoff } => format!(" [mask cutoff {cutoff:08x}]"),
            _ => String::new(),
        };
        let recipe_name = format!("{name}{recipe}");
        if let Some(index) = self.images.iter().position(|i| i.name == recipe_name) {
            return Ok(index);
        }
        let bytes = match image.source() {
            gltf::image::Source::Uri { .. } => {
                std::fs::read(&name).map_err(|e| format!("{name}: {e}"))?
            }
            gltf::image::Source::View { view, .. } => buffers[view.buffer().index()]
                .0
                .get(view.offset()..view.offset() + view.length())
                .ok_or_else(|| format!("{name}: the image's buffer view is out of range"))?
                .to_vec(),
        };
        let (width, height, rgba) = decode_png(&bytes).map_err(|e| format!("{name}: {e}"))?;
        let image = fit(
            BaseColorImage {
                name: recipe_name,
                width,
                height,
                rgba,
            },
            MAX_IMAGE_SIZE,
            role,
        );
        // Files built by one script often pack the same image; share it.
        if let Some(index) = self.images.iter().position(|i| {
            i.width == image.width
                && i.height == image.height
                && if recipe.is_empty() {
                    !i.name.contains(" [mask cutoff ")
                } else {
                    i.name.ends_with(&recipe)
                }
                && i.rgba == image.rgba
        }) {
            return Ok(index);
        }
        Ok(self.add_image(image))
    }
}

/// One animated textured model in a frame's dynamic mesh, such as a skinned
/// character posed on the CPU.
///
/// The renderer uploads `scene`'s images, materials, and the indices of its
/// one mesh once for each distinct `scene` (by [`Arc`] identity), then
/// rewrites one vertex buffer from `vertices` each frame. That keeps skinning
/// off the GPU, so OpenGL ES 3.0 and WebGL2, which have no storage buffers,
/// draw it as desktops do. It draws in the textured passes after the world's
/// cells, without culling, and casts shadows.
#[derive(Clone)]
pub struct Figure {
    /// Images, materials, and exactly one mesh without placements.
    pub scene: std::sync::Arc<TexturedScene>,
    /// This frame's world-space vertices: every primitive's in turn, as many
    /// as the mesh has.
    pub vertices: std::sync::Arc<Vec<TexturedVertex>>,
}

impl std::fmt::Debug for Figure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Figure({:?}, {} vertices)",
            self.scene,
            self.vertices.len()
        )
    }
}

impl Figure {
    /// Checks the scene, that the frame's vertices match its mesh, and that
    /// they are finite.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first part that is out of bounds.
    pub fn validate(&self) -> Result<(), String> {
        self.scene.validate()?;
        let [mesh] = self.scene.meshes.as_slice() else {
            return Err("a figure has exactly one mesh".into());
        };
        let count: usize = mesh.primitives.iter().map(|p| p.vertices.len()).sum();
        if !self.scene.placements.is_empty()
            || count != self.vertices.len()
            || count > u32::MAX as usize
            || !self.vertices.iter().all(TexturedVertex::finite)
        {
            return Err("a figure's vertices do not match its mesh".into());
        }
        Ok(())
    }

    /// The figure's mesh as uploaded: its bind-pose vertices, its indices
    /// offset into one buffer, and one batch per primitive.
    pub fn merged(&self) -> Merged {
        let mut merged = Merged::default();
        for mesh in &self.scene.meshes {
            for p in &mesh.primitives {
                let base = merged.vertices.len() as u32;
                let first = merged.indices.len() as u32;
                merged.vertices.extend_from_slice(&p.vertices);
                merged.indices.extend(p.indices.iter().map(|i| i + base));
                if p.indices.is_empty() {
                    continue;
                }
                merged.batches.push(Batch {
                    material: p.material,
                    first,
                    count: p.indices.len() as u32,
                    min: Vec3::splat(f32::NEG_INFINITY),
                    max: Vec3::splat(f32::INFINITY),
                    level: Level::Always,
                    run: None,
                });
            }
        }
        merged
    }
}

/// Copies of a set of meshes drawn as GPU instances in a frame's dynamic
/// mesh, such as the chunks of what a meteor broke: each record places one
/// mesh with its own transform, and each record's vertices take their own
/// light. The renderer uploads `scene`'s meshes once for each distinct
/// `scene` (by [`Arc`] identity) and writes only the records and the light
/// each frame, so nothing is posed vertex by vertex on the CPU. Records of
/// one mesh draw together, one indexed draw a mesh.
///
/// [`Arc`]: std::sync::Arc
#[derive(Clone)]
pub struct Instances {
    /// Images, materials, and the meshes, each one primitive in mesh space,
    /// without placements.
    pub scene: std::sync::Arc<TexturedScene>,
    /// This frame's copies.
    pub records: std::sync::Arc<Vec<InstanceOf>>,
    /// Each record's vertices' light channel ([`TexturedVertex::light`]),
    /// record after record, as many as each record's mesh has vertices.
    pub lights: std::sync::Arc<Vec<[u8; 4]>>,
}

/// Most sets of [`Instances`] a frame draws; the renderer keeps each set's
/// meshes uploaded in its own slot.
pub const INSTANCE_SETS: usize = 2;

/// One copy of an [`Instances`] mesh: which mesh, and its mesh-to-world
/// transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceOf {
    pub mesh: u32,
    pub transform: Mat4,
}

impl std::fmt::Debug for Instances {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Instances({:?}, {} records)",
            self.scene,
            self.records.len()
        )
    }
}

impl Instances {
    /// The vertices each of the scene's meshes holds, in all its
    /// primitives.
    #[must_use]
    pub fn mesh_vertices(&self) -> Vec<usize> {
        self.scene
            .meshes
            .iter()
            .map(|m| m.primitives.iter().map(|p| p.vertices.len()).sum())
            .collect()
    }

    /// Checks the scene, that every record names a mesh and has a finite
    /// transform, and that the light covers every record's vertices.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first part that is out of bounds.
    pub fn validate(&self) -> Result<(), String> {
        if !self.scene.placements.is_empty() {
            return Err("instances' scene has no placements".into());
        }
        let counts = self.mesh_vertices();
        let mut texels = 0usize;
        for record in self.records.iter() {
            let Some(count) = counts.get(record.mesh as usize) else {
                return Err("an instance names a mesh its scene lacks".into());
            };
            if !record.transform.is_finite() {
                return Err("an instance's transform is not finite".into());
            }
            texels += count;
        }
        if texels != self.lights.len() || texels > u32::MAX as usize {
            return Err("instances' light does not match their meshes".into());
        }
        Ok(())
    }

    /// Bytes a frame writes for them: the records and the light.
    #[must_use]
    pub fn frame_bytes(&self) -> u64 {
        (self.records.len() * std::mem::size_of::<super::instanced::Instance>()
            + self.lights.len() * 4) as u64
    }
}

/// A merged scene: world-space vertices and indices, and the cells that
/// draw ranges of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Merged {
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<Batch>,
}

/// The cell a placement merges into: its translation's, on the ground.
pub(crate) fn cell_of(transform: Mat4) -> (i32, i32) {
    (
        (transform.w_axis.x / CELL).floor() as i32,
        (transform.w_axis.z / CELL).floor() as i32,
    )
}

/// The box around `vertices`.
pub(crate) fn bounds(vertices: &[TexturedVertex]) -> (Vec3, Vec3) {
    vertices.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(min, max), v| (min.min(v.pos.into()), max.max(v.pos.into())),
    )
}

/// What one view of a scene draws ([`TexturedScene::frame_cost`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCost {
    /// Cells of one material in view at their level.
    pub cells: u64,
    /// Indexed draw calls.
    pub draws: u64,
    pub triangles: u64,
}

/// One cell of one material: a range of the merged indices and its bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Batch {
    pub material: usize,
    pub first: u32,
    pub count: u32,
    pub min: Vec3,
    pub max: Vec3,
    /// The distances it draws at.
    pub level: Level,
    /// For a run of instances, the instance records that draw this range of
    /// a shared mesh's indices; `None` for a merged cell, which draws once
    /// in world space.
    pub run: Option<super::instanced::Run>,
}

/// The order cells draw in: opaque, then masked, each in merge order so
/// cells of one material draw together, then blended cells from the
/// farthest center to the nearest, so nearer glass composites over farther
/// glass. `visible`, given each cell's index, drops culled cells.
pub fn draw_order(
    batches: &[Batch],
    materials: &[TexturedMaterial],
    eye: Vec3,
    visible: impl Fn(usize, &Batch) -> bool,
) -> Vec<usize> {
    let pass = |i: usize| materials[batches[i].material].alpha.pass();
    let distance = |i: usize| ((batches[i].min + batches[i].max) * 0.5).distance_squared(eye);
    let mut order: Vec<usize> = (0..batches.len())
        .filter(|&i| visible(i, &batches[i]))
        .collect();
    order.sort_by(|&a, &b| {
        pass(a)
            .cmp(&pass(b))
            .then_with(|| {
                if pass(a) == Pass::Blended {
                    distance(b).total_cmp(&distance(a))
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then(a.cmp(&b))
    });
    order
}

/// Whether the box from `min` to `max` may intersect the view of
/// `view_proj`, a projection with depth from 0 to 1 (Gribb and Hartmann,
/// "Fast Extraction of Viewing Frustum Planes", 2001).
pub fn in_frustum(min: Vec3, max: Vec3, view_proj: Mat4) -> bool {
    let rows = view_proj.transpose();
    let planes = [
        rows.w_axis + rows.x_axis,
        rows.w_axis - rows.x_axis,
        rows.w_axis + rows.y_axis,
        rows.w_axis - rows.y_axis,
        rows.z_axis,
        rows.w_axis - rows.z_axis,
    ];
    let center = (min + max) * 0.5;
    let half = (max - min) * 0.5;
    planes
        .iter()
        .all(|p| p.truncate().dot(center) + p.w + p.truncate().abs().dot(half) >= 0.0)
}

/// Whether the box from `min` to `max` lies within the side planes of
/// `view_proj`, ignoring its near and far planes: a shadow map's caster
/// test, since a caster beyond the light's near plane still shades.
pub fn in_slab(min: Vec3, max: Vec3, view_proj: Mat4) -> bool {
    let rows = view_proj.transpose();
    let planes = [
        rows.w_axis + rows.x_axis,
        rows.w_axis - rows.x_axis,
        rows.w_axis + rows.y_axis,
        rows.w_axis - rows.y_axis,
    ];
    let center = (min + max) * 0.5;
    let half = (max - min) * 0.5;
    planes
        .iter()
        .all(|p| p.truncate().dot(center) + p.w + p.truncate().abs().dot(half) >= 0.0)
}

/// The smallest size a cell may have at a distance and still draw, as a
/// fraction of that distance: about two pixels on a 1080-pixel-tall view.
/// A cell of small ground cover drops out a few tens of meters off, where
/// it would cover a pixel or two, while a cell of buildings or trees draws
/// until the fog.
pub const DETAIL: f32 = 1.0 / 90.0;

/// Whether a cell's box from `min` to `max` draws from `eye` under
/// `view_proj` when fog is total at `far` meters: it is in view, some of it
/// is nearer than `far`, and it is not too small to see at its distance
/// ([`DETAIL`]). An infinite `far` keeps every cell in view.
pub fn drawn(min: Vec3, max: Vec3, view_proj: Mat4, eye: Vec3, far: f32) -> bool {
    if !in_frustum(min, max, view_proj) {
        return false;
    }
    if !far.is_finite() {
        return true;
    }
    let distance = eye.clamp(min, max).distance(eye);
    distance <= far && (max - min).length() >= distance * DETAIL
}

/// How a pass's cells rasterize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raster {
    /// The fragment entry in `photo.wgsl`.
    pub entry: &'static str,
    /// Faces culled; `None` draws both sides.
    pub cull: Option<wgpu::Face>,
    pub depth_write: bool,
    /// Premultiplied blending over the scene.
    pub blend: bool,
    /// Whether the cells draw into the shadow map, and whether that draw
    /// tests alpha.
    pub shadow: Option<bool>,
}

/// The raster state of a pass for single- or double-sided materials.
pub fn raster(pass: Pass, double_sided: bool) -> Raster {
    let cull = (!double_sided).then_some(wgpu::Face::Back);
    match pass {
        Pass::Opaque => Raster {
            entry: "fs_textured",
            cull,
            depth_write: true,
            blend: false,
            shadow: Some(false),
        },
        Pass::Masked => Raster {
            entry: "fs_textured_masked",
            cull,
            depth_write: true,
            blend: false,
            shadow: Some(true),
        },
        Pass::Blended => Raster {
            entry: "fs_textured_blend",
            cull,
            depth_write: false,
            blend: true,
            shadow: None,
        },
    }
}

/// The material uniform `photo.wgsl` declares as `TexturedMaterial`.
pub fn uniform(material: &TexturedMaterial) -> [[f32; 4]; 2] {
    [
        material.base_color,
        [
            material.metallic,
            material.roughness,
            material.alpha.cutoff(),
            material.emissive,
        ],
    ]
}

/// The brightest emission a material may carry, cd/m²: about a candle
/// flame's luminance.
pub const MAX_EMISSIVE: f32 = 20_000.0;

/// A glTF material's emission as luminance per unit of base color: the
/// brightest channel of its emissive factor times its
/// `KHR_materials_emissive_strength`, read as cd/m².
fn emissive(material: &gltf::Material<'_>) -> f32 {
    let factor = material.emissive_factor().into_iter().fold(0.0, f32::max);
    let strength = material.emissive_strength().unwrap_or(1.0);
    let value = factor * strength;
    if value.is_finite() {
        value.clamp(0.0, MAX_EMISSIVE)
    } else {
        0.0
    }
}

pub fn srgb_to_linear() -> &'static [f32; 256] {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| verse_engine::mips::srgb_to_linear()[i] as f32))
}
pub fn material_role(material: &TexturedMaterial) -> verse_engine::mips::Role {
    match material.alpha {
        AlphaMode::Mask { cutoff } => {
            verse_engine::mips::Role::masked(cutoff, material.base_color[3])
                .expect("validated material factors")
        }
        _ => verse_engine::mips::Role::Color,
    }
}
/// Fit admitted image data with the same area/color recipe used for uploads.
fn fit(image: BaseColorImage, max: u32, role: verse_engine::mips::Role) -> BaseColorImage {
    if image.width <= max && image.height <= max {
        return image;
    }
    let levels = verse_engine::mips::cook(image.width, image.height, &image.rgba, role, max)
        .expect("decoded image extent");
    let (width, height, rgba) = levels.into_iter().next().unwrap();
    BaseColorImage {
        width,
        height,
        rgba,
        ..image
    }
}
#[cfg(test)]
fn coverage(rgba: &[u8], cutoff: f32, scale: f32) -> f32 {
    rgba.chunks_exact(4)
        .filter(|p| f32::from(p[3]) / 255. * scale >= cutoff)
        .count() as f32
        / (rgba.len() / 4) as f32
}
/// Cook sRGB color with the material's effective mask comparison.
#[cfg(test)]
pub fn mip_chain(
    image: &BaseColorImage,
    cutoff: Option<f32>,
    max: u32,
) -> Vec<verse_engine::mips::Level> {
    let role = cutoff.map_or(verse_engine::mips::Role::Color, |cutoff| {
        verse_engine::mips::Role::masked(cutoff, 1.).expect("validated cutoff")
    });
    verse_engine::mips::cook(image.width, image.height, &image.rgba, role, max)
        .expect("admitted image extent")
}

/// Decodes a PNG into RGBA8, expanding gray, palette, and 16-bit images.
fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    // An 8192-texel square RGBA image, with room for the decoder's rows.
    let limits = png::Limits { bytes: 300 << 20 };
    let mut decoder = png::Decoder::new_with_limits(std::io::Cursor::new(bytes), limits);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("the image is empty or larger than 8192 texels".into());
    }
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("the image is too large")?
    ];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    let channels = info.color_type.samples();
    let count = info.width as usize * info.height as usize;
    let pixels = &buffer[..info.buffer_size()];
    if pixels.len() < count * channels {
        return Err("the image's data is shorter than its size".into());
    }
    let mut rgba = Vec::with_capacity(count * 4);
    for p in pixels.chunks_exact(channels).take(count) {
        rgba.extend_from_slice(&match channels {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        });
    }
    Ok((info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_groups_select_one_level_across_both_switches() {
        let levels: Vec<_> = (0..3)
            .map(|level| Level::Group {
                group: 0,
                anchor: [0.0; 2],
                switches: [40.0, 80.0],
                level,
                fallback: 0,
            })
            .collect();
        let group = DetailGroup {
            anchor: [0.0; 2],
            switches: [40.0, 80.0],
            fallback: 0,
        };
        let mut was = None;
        for distance in (0..1000)
            .map(|n| n as f32 / 10.0)
            .chain((0..1000).rev().map(|n| n as f32 / 10.0))
        {
            let eye = Vec3::new(0.0, 2.0, distance);
            let selected = group.selected(eye, was);
            assert_eq!(
                levels
                    .iter()
                    .enumerate()
                    .filter(|(i, level)| level.drawn(*i == usize::from(selected)))
                    .count(),
                1,
                "distance {distance}"
            );
            was = Some(selected);
        }
        let mut was = None;
        for (distance, expected) in [
            (10.0, 0),
            (80.1, 2),
            (39.9, 0),
            (120.0, 2),
            (40.1, 1),
            (10.0, 0),
            (79.9, 1),
            (120.0, 2),
            (39.9, 0),
            (0.0, 0),
            (80.0, 2),
            (79.9, 2),
            (37.4, 0),
            (42.6, 1),
            (82.6, 2),
            (77.4, 1),
            (37.4, 0),
        ] {
            let selected = group.selected(Vec3::new(0.0, 2.0, distance), was);
            assert_eq!(selected, expected, "jump to {distance} from {was:?}");
            assert_eq!(
                levels
                    .iter()
                    .enumerate()
                    .filter(|(i, level)| level.drawn(*i == usize::from(selected)))
                    .count(),
                1
            );
            was = Some(selected);
        }
        for (distance, selected) in [(10.0, 0), (50.0, 1), (120.0, 2)] {
            assert!(levels[selected].drawn_from(Vec3::new(0.0, 2.0, distance)));
        }
    }

    #[test]
    fn detail_groups_merge_across_cells_keep_ranges_and_restore_after_far_damage() {
        let mut scene = scene(&[AlphaMode::Opaque]);
        scene.detail_groups.push(DetailGroup {
            anchor: [0.0; 2],
            switches: [40.0, 80.0],
            fallback: 0,
        });
        scene.placements.clear();
        for x in [-12.0, 12.0] {
            scene.place_detail(
                0,
                Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
                Detail::Group { group: 0, level: 0 },
            );
        }
        for level in [1, 2] {
            scene.place_detail(0, Mat4::IDENTITY, Detail::Group { group: 0, level });
        }
        let merged = scene.merge().unwrap();
        let near = merged
            .batches
            .iter()
            .find(|b| matches!(b.level, Level::Group { level: 0, .. }))
            .unwrap();
        assert_eq!(near.count, 12);
        assert!(near.min.x <= -12.0 && near.max.x >= 13.0);
        let ranges = scene.index_ranges();
        assert_eq!(ranges.len(), 4);
        for (i, ranges) in ranges.iter().enumerate() {
            for range in ranges {
                assert_eq!(
                    scene.range_indices(i, range),
                    merged.indices[range.first as usize..(range.first + range.count) as usize]
                );
            }
        }
        let eye = Vec3::new(0.0, 2.0, 120.0);
        let shown = |groups: &std::collections::BTreeSet<u16>| {
            merged
                .batches
                .iter()
                .filter(|b| b.level.drawn_with_fallback(b.level.near(eye, None), groups))
                .count()
        };
        assert_eq!(shown(&scene.edits.group_fallbacks()), 1);
        scene.edits.set_group_fallbacks([0].into_iter().collect());
        assert_eq!(shown(&scene.edits.group_fallbacks()), 1);
        assert!(
            near.level
                .drawn_with_fallback(false, &scene.edits.group_fallbacks())
        );
        let first = ranges[0][0];
        scene
            .edits
            .write(first.first, vec![first.base; first.count as usize]);
        let seen = scene.edits.revision();
        scene.edits.set_group_fallbacks([0].into_iter().collect());
        assert_eq!(scene.edits.revision(), seen);
        let second = ranges[1][0];
        scene
            .edits
            .write(second.first, vec![second.base; second.count as usize]);
        let (later, revision) = scene.edits.since(seen);
        assert!(revision > seen);
        assert_eq!(
            later,
            vec![(second.first, vec![second.base; second.count as usize])]
        );
        for (i, range) in [(0, first), (1, second)] {
            scene
                .edits
                .write(range.first, scene.range_indices(i, &range));
        }
        scene.edits.set_group_fallbacks(Default::default());
        assert_eq!(shown(&scene.edits.group_fallbacks()), 1);
        assert!(
            !near
                .level
                .drawn_with_fallback(false, &scene.edits.group_fallbacks())
        );
    }

    fn quad(material: usize) -> TexturedMesh {
        let v = |x: f32, y: f32| TexturedVertex::new(Vec3::new(x, y, 0.0), Vec3::Z, [x, y]);
        TexturedMesh {
            primitives: vec![Primitive {
                vertices: vec![v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0), v(0.0, 1.0)],
                indices: vec![0, 1, 2, 0, 2, 3],
                material,
            }],
        }
    }

    fn scene(alphas: &[AlphaMode]) -> TexturedScene {
        let mut scene = TexturedScene::default();
        for &alpha in alphas {
            let material = scene.add_material(TexturedMaterial {
                alpha,
                ..TexturedMaterial::default()
            });
            scene.add_mesh(quad(material));
        }
        scene
    }

    #[test]
    fn a_cells_near_and_far_levels_draw_in_turn_with_hysteresis() {
        let mut scene = scene(&[AlphaMode::Opaque, AlphaMode::Opaque]);
        scene.switches = vec![40.0];
        let at = Mat4::from_translation(Vec3::new(1.0, 0.0, 1.0));
        scene.place_detail(0, at, Detail::Near(0));
        scene.place_detail(1, at, Detail::Far(0));
        scene.place(0, Mat4::from_translation(Vec3::new(9.0, 0.0, 1.0)));
        let merged = scene.merge().unwrap();
        assert_eq!(merged.batches.len(), 3);
        let anchor = [CELL * 0.5, CELL * 0.5];
        assert!(merged.batches.iter().any(|b| b.level
            == Level::Near {
                anchor,
                switch: 40.0
            }));
        let drawn = |eye: Vec3, near: Option<bool>| -> Vec<usize> {
            merged
                .batches
                .iter()
                .filter(|b| b.level.drawn(b.level.near(eye, near)))
                .map(|b| b.material)
                .collect()
        };
        // One level of the cell draws at a time; the other cell always.
        let close = Vec3::new(anchor[0], 2.0, anchor[1] + 30.0);
        let far = Vec3::new(anchor[0], 2.0, anchor[1] + 50.0);
        assert_eq!(drawn(close, None), [0, 0]);
        assert_eq!(drawn(far, None), [0, 1]);
        // Just past the switch, a cell keeps the level it had.
        let past = Vec3::new(anchor[0], 2.0, anchor[1] + 40.0 + HYSTERESIS * 0.5);
        let short = Vec3::new(anchor[0], 2.0, anchor[1] + 40.0 - HYSTERESIS * 0.5);
        assert_eq!(drawn(past, Some(true)), [0, 0]);
        assert_eq!(drawn(short, Some(false)), [0, 1]);
        assert_eq!(drawn(past, None), [0, 1]);
        // Height doesn't count: distance is across the ground.
        let above = Vec3::new(anchor[0], 500.0, anchor[1] + 30.0);
        assert_eq!(drawn(above, None), [0, 0]);
        // A detail must name one of the scene's switches.
        scene.place_detail(0, at, Detail::Far(1));
        assert!(scene.validate().is_err());
    }

    fn quaternius(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/verse/props/quaternius")
            .join(format!("{name}.gltf"))
    }

    #[test]
    fn oversized_mask_fitting_preserves_coverage_before_runtime_cooking() {
        let source = BaseColorImage {
            name: "thin stems".into(),
            width: 16,
            height: 16,
            rgba: (0..256)
                .flat_map(|i| [40, 120, 30, if i % 4 == 0 { 255 } else { 0 }])
                .collect(),
        };
        let color = fit(source.clone(), 4, verse_engine::mips::Role::Color);
        let masked = fit(
            source,
            4,
            verse_engine::mips::Role::masked(0.6, 1.).unwrap(),
        );
        assert_eq!((masked.width, masked.height), (4, 4));
        assert_eq!(coverage(&color.rgba, 0.6, 1.), 0.);
        assert_eq!(coverage(&masked.rgba, 0.6, 1.), 0.25);
        let levels = mip_chain(&masked, Some(0.6), 4);
        assert_eq!(coverage(&levels[1].2, 0.6, 1.), 0.25);
    }
    #[test]
    fn the_cutoff_keeps_alpha_at_or_above_it() {
        let mask = AlphaMode::Mask { cutoff: 0.5 };
        assert!(mask.keeps(0.5));
        assert!(mask.keeps(1.0));
        assert!(!mask.keeps(0.499));
        assert!(AlphaMode::Opaque.keeps(0.0));
        assert!(!AlphaMode::Blend.keeps(0.0));
        assert!(AlphaMode::Blend.keeps(0.1));
        // The shader reads the cutoff from the uniform; opaque and blended
        // materials never discard.
        assert_eq!(
            uniform(&TexturedMaterial {
                alpha: mask,
                ..TexturedMaterial::default()
            })[1][2],
            0.5
        );
        assert_eq!(uniform(&TexturedMaterial::default())[1][2], 0.0);
    }

    #[test]
    fn only_masked_cells_discard_and_only_blended_cells_skip_depth_and_shadows() {
        for double_sided in [false, true] {
            let opaque = raster(Pass::Opaque, double_sided);
            let masked = raster(Pass::Masked, double_sided);
            let blended = raster(Pass::Blended, double_sided);
            assert_eq!(opaque.entry, "fs_textured");
            assert_eq!(masked.entry, "fs_textured_masked");
            assert_eq!(blended.entry, "fs_textured_blend");
            assert!(opaque.depth_write && masked.depth_write && !blended.depth_write);
            assert!(!opaque.blend && !masked.blend && blended.blend);
            assert_eq!(opaque.shadow, Some(false));
            assert_eq!(masked.shadow, Some(true));
            assert_eq!(blended.shadow, None);
        }
    }

    #[test]
    fn double_sided_materials_draw_both_faces() {
        for pass in Pass::ALL {
            assert_eq!(raster(pass, true).cull, None);
            assert_eq!(raster(pass, false).cull, Some(wgpu::Face::Back));
        }
    }

    #[test]
    fn placements_merge_into_cells_per_material() {
        let mut scene = scene(&[AlphaMode::Opaque, AlphaMode::Opaque]);
        // Two copies of mesh 0 in one cell, one in the next, one of mesh 1.
        scene.place(0, Mat4::from_translation(Vec3::new(1.0, 0.0, 1.0)));
        scene.place(0, Mat4::from_translation(Vec3::new(3.0, 0.0, 2.0)));
        scene.place(0, Mat4::from_translation(Vec3::new(9.0, 0.0, 1.0)));
        scene.place(1, Mat4::from_translation(Vec3::new(2.0, 0.0, 2.0)));
        let merged = scene.merge().unwrap();
        assert_eq!(merged.batches.len(), 3);
        assert_eq!(merged.vertices.len(), 16);
        assert_eq!(merged.indices.len(), 24);
        let first = merged.batches[0];
        assert_eq!((first.material, first.first, first.count), (0, 0, 12));
        assert_eq!(first.min, Vec3::new(1.0, 0.0, 1.0));
        assert_eq!(first.max, Vec3::new(4.0, 1.0, 2.0));
        // Indices address the merged vertices in world space.
        for batch in &merged.batches {
            let range = batch.first as usize..(batch.first + batch.count) as usize;
            for &i in &merged.indices[range] {
                let p = Vec3::from(merged.vertices[i as usize].pos);
                assert!(p.cmpge(batch.min).all() && p.cmple(batch.max).all());
            }
        }
    }

    #[test]
    fn index_ranges_find_each_placements_triangles_in_the_merge() {
        let mut scene = scene(&[AlphaMode::Opaque, AlphaMode::Blend]);
        scene.place(0, Mat4::from_translation(Vec3::new(1.0, 0.0, 1.0)));
        scene.place(1, Mat4::from_translation(Vec3::new(2.0, 0.0, 2.0)));
        scene.place(0, Mat4::from_translation(Vec3::new(9.0, 0.0, 1.0)));
        scene.place(0, Mat4::from_translation(Vec3::new(3.0, 0.0, 2.0)));
        let merged = scene.merge().unwrap();
        let ranges = scene.index_ranges();
        assert_eq!(ranges.len(), 4);
        for (index, (placement, ranges)) in scene.placements.iter().zip(&ranges).enumerate() {
            assert_eq!(ranges.len(), 1);
            let IndexRange { first, count, .. } = ranges[0];
            assert_eq!(
                scene.range_indices(index, &ranges[0]),
                merged.indices[first as usize..(first + count) as usize]
            );
            let at = placement.transform.w_axis.truncate();
            // Every vertex the range draws is this placement's quad.
            for &i in &merged.indices[first as usize..(first + count) as usize] {
                let p = Vec3::from(merged.vertices[i as usize].pos);
                assert!(p.x >= at.x - 1e-4 && p.x <= at.x + 1.0 + 1e-4, "{p} {at}");
                assert!((p.z - at.z).abs() < 1e-4, "{p} {at}");
            }
        }
        // The ranges cover the merge once.
        let total: u32 = ranges.iter().flatten().map(|r| r.count).sum();
        assert_eq!(total as usize, merged.indices.len());
        // Edits replay from any revision seen.
        let edits = IndexEdits::default();
        edits.write(6, vec![0; 6]);
        edits.write(0, vec![1; 6]);
        assert_eq!(edits.revision(), 2);
        assert_eq!(edits.since(0).0.len(), 2);
        assert_eq!(edits.since(1).0, vec![(0, vec![1; 6])]);
        edits.write(6, vec![2; 6]);
        assert_eq!(edits.since(2).0, vec![(6, vec![2; 6])]);
        assert_eq!(edits.since(0).0.len(), 2);
    }

    #[test]
    fn a_mirroring_placement_keeps_front_faces_counterclockwise() {
        let mut scene = scene(&[AlphaMode::Opaque]);
        scene.place(0, Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)));
        let merged = scene.merge().unwrap();
        let p = |i: u32| Vec3::from(merged.vertices[i as usize].pos);
        for triangle in merged.indices.chunks_exact(3) {
            let normal = (p(triangle[1]) - p(triangle[0])).cross(p(triangle[2]) - p(triangle[0]));
            // The mirrored quad still faces +Z, and so does its winding.
            assert!(normal.z > 0.0);
            assert!(merged.vertices[triangle[0] as usize].normal[2] > 0.0);
        }
    }

    #[test]
    fn blended_cells_draw_last_from_far_to_near() {
        let mut scene = scene(&[
            AlphaMode::Blend,
            AlphaMode::Opaque,
            AlphaMode::Mask { cutoff: 0.5 },
        ]);
        // Merge order runs from -20 to -4 by cell; seen from z = -40, that
        // puts the nearest glass first, under the farther glass.
        scene.place(0, Mat4::from_translation(Vec3::new(0.0, 0.0, -4.0)));
        scene.place(0, Mat4::from_translation(Vec3::new(0.0, 0.0, -20.0)));
        scene.place(0, Mat4::from_translation(Vec3::new(0.0, 0.0, -12.0)));
        scene.place(1, Mat4::from_translation(Vec3::new(0.0, 0.0, -30.0)));
        scene.place(2, Mat4::from_translation(Vec3::new(0.0, 0.0, -3.0)));
        let merged = scene.merge().unwrap();
        let eye = Vec3::new(0.0, 0.0, -40.0);
        let order = draw_order(&merged.batches, &scene.materials, eye, |_, _| true);
        let passes: Vec<Pass> = order
            .iter()
            .map(|&i| scene.materials[merged.batches[i].material].alpha.pass())
            .collect();
        assert_eq!(
            passes,
            [
                Pass::Opaque,
                Pass::Masked,
                Pass::Blended,
                Pass::Blended,
                Pass::Blended
            ]
        );
        let depths: Vec<f32> = order[2..]
            .iter()
            .map(|&i| merged.batches[i].min.z)
            .collect();
        assert_eq!(depths, [-4.0, -12.0, -20.0]);
        // Culled cells are left out.
        let near = draw_order(&merged.batches, &scene.materials, eye, |_, b| {
            b.min.z > -10.0
        });
        assert_eq!(near.len(), 2);
    }

    #[test]
    fn frustum_culling_keeps_boxes_in_view_and_drops_boxes_behind() {
        let view_proj = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0)
            * Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        assert!(in_frustum(
            Vec3::new(-1.0, -1.0, -11.0),
            Vec3::new(1.0, 1.0, -9.0),
            view_proj
        ));
        assert!(!in_frustum(
            Vec3::new(-1.0, -1.0, 9.0),
            Vec3::new(1.0, 1.0, 11.0),
            view_proj
        ));
        assert!(!in_frustum(
            Vec3::new(50.0, -1.0, -11.0),
            Vec3::new(52.0, 1.0, -9.0),
            view_proj
        ));
    }

    #[test]
    fn invalid_scenes_are_refused() {
        let mut scene = scene(&[AlphaMode::Mask { cutoff: 1.5 }]);
        assert!(scene.validate().is_err());
        scene.materials[0].alpha = AlphaMode::Mask { cutoff: 0.5 };
        assert!(scene.validate().is_ok());
        scene.meshes[0].primitives[0].indices.push(7);
        assert!(scene.validate().is_err());
        scene.meshes[0].primitives[0].indices.truncate(6);
        scene.place(3, Mat4::IDENTITY);
        assert!(scene.validate().is_err());
        scene.placements[0].mesh = 0;
        scene.placements[0].transform = Mat4::ZERO;
        assert!(scene.validate().is_err());
        scene.placements.clear();
        scene.add_image(BaseColorImage {
            name: "short".into(),
            width: 2,
            height: 2,
            rgba: vec![0; 4],
        });
        assert!(scene.validate().is_err());
    }

    #[test]
    fn mips_average_color_in_linear_light() {
        // A black and white checker averages to linear mid-gray, which sRGB
        // encodes near 188, not 128.
        let image = BaseColorImage {
            name: "checker".into(),
            width: 2,
            height: 2,
            rgba: [0, 255, 255, 0]
                .iter()
                .flat_map(|&c| [c, c, c, 255])
                .collect(),
        };
        let chain = mip_chain(&image, None, MAX_IMAGE_SIZE);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[1].0, 1);
        assert!((186..=190).contains(&chain[1].2[0]), "{}", chain[1].2[0]);
        assert_eq!(chain[1].2[3], 255);
    }

    #[test]
    fn masked_mips_keep_their_alpha_coverage_at_the_cutoff() {
        // Thin opaque stems one texel wide every four texels: a quarter of
        // the image passes the cutoff.
        let (w, h) = (16u32, 16u32);
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| [40, 120, 30, if i % w % 4 == 0 { 255 } else { 0 }])
            .collect();
        let image = BaseColorImage {
            name: "stems".into(),
            width: w,
            height: h,
            rgba,
        };
        let cutoff = 0.6;
        assert_eq!(coverage(&image.rgba, cutoff, 1.0), 0.25);
        let plain = mip_chain(&image, None, MAX_IMAGE_SIZE);
        let masked = mip_chain(&image, Some(cutoff), MAX_IMAGE_SIZE);
        assert_eq!(plain.len(), 5);
        assert_eq!(masked.len(), 5);
        // Averaging alone halves the stems' alpha below the cutoff, so the
        // foliage would vanish at the first mip.
        assert_eq!(coverage(&plain[1].2, cutoff, 1.0), 0.0);
        for (level, (_, _, rgba)) in masked.iter().enumerate() {
            assert!(
                coverage(rgba, cutoff, 1.0) >= 0.25,
                "level {level} lost its coverage"
            );
        }
        // Color is untouched by the alpha scale.
        assert_eq!(&masked[1].2[..3], &plain[1].2[..3]);
    }

    #[test]
    fn mip_chains_skip_levels_above_the_device_limit() {
        let image = BaseColorImage {
            name: "big".into(),
            width: 8,
            height: 4,
            rgba: vec![255; 8 * 4 * 4],
        };
        let chain = mip_chain(&image, None, 2);
        let sizes: Vec<(u32, u32)> = chain.iter().map(|l| (l.0, l.1)).collect();
        assert_eq!(sizes, [(2, 1), (1, 1)]);
        let fitted = fit(image, 4, verse_engine::mips::Role::Color);
        assert_eq!((fitted.width, fitted.height), (4, 2));
    }

    #[test]
    fn the_quaternius_props_import_with_shared_trim_sheets() {
        let mut scene = TexturedScene::default();
        let barrel = scene.import_gltf(&quaternius("Barrel")).unwrap();
        let table = scene.import_gltf(&quaternius("Table_Large")).unwrap();
        assert_eq!((barrel, table), (0, 1));
        // Both use the furniture and metal trim sheets, imported once.
        assert_eq!(scene.images.len(), 2);
        assert!(
            scene
                .images
                .iter()
                .all(|i| i.width == 2048 && i.height == 2048)
        );
        assert_eq!(scene.materials.len(), 4);
        for material in &scene.materials {
            assert!(material.image.is_some());
            assert!(material.double_sided);
            assert_eq!(material.alpha, AlphaMode::Opaque);
            // The unread metallic-roughness image leaves a dielectric.
            assert_eq!(material.metallic, 0.0);
        }
        let mesh = &scene.meshes[barrel];
        assert!(!mesh.primitives.is_empty());
        let (min, max) = mesh.primitives.iter().flat_map(|p| &p.vertices).fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), v| (lo.min(v.pos.into()), hi.max(v.pos.into())),
        );
        // A barrel about a meter tall, in meters.
        assert!((0.3..3.0).contains(&(max.y - min.y)), "{min} {max}");
        scene.place(barrel, Mat4::IDENTITY);
        scene.place(table, Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)));
        let merged = scene.merge().unwrap();
        assert_eq!(merged.batches.len(), 4);
    }

    fn chamber(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/verse/generated/chamber")
            .join(format!("{name}.glb"))
    }

    #[test]
    fn files_that_pack_the_same_image_share_it_and_keep_their_emission() {
        let mut scene = TexturedScene::default();
        scene.import_gltf(&chamber("cauldron_green")).unwrap();
        let images = scene.images.len();
        scene.import_gltf(&chamber("cauldron_red")).unwrap();
        // The two cauldrons pack the same iron, stone, and wood images.
        assert_eq!(scene.images.len(), images);
        let glowing: Vec<f32> = scene
            .materials
            .iter()
            .map(|m| m.emissive)
            .filter(|&e| e > 0.0)
            .collect();
        // Each liquid, its froth, and the embers glow; nothing else does.
        assert_eq!(glowing.len(), 6, "{glowing:?}");
        assert!(glowing.iter().all(|&e| (50.0..=MAX_EMISSIVE).contains(&e)));
        let flames = TexturedScene::default().import_gltf(&chamber("floor_candles"));
        assert!(flames.is_ok());
    }

    #[test]
    fn emission_rides_in_the_material_uniform_and_is_bounded() {
        let glowing = TexturedMaterial {
            emissive: 120.0,
            ..TexturedMaterial::default()
        };
        assert_eq!(uniform(&glowing)[1][3], 120.0);
        assert!(glowing.validate(0).is_ok());
        for bad in [-1.0, f32::NAN, MAX_EMISSIVE * 2.0] {
            let material = TexturedMaterial {
                emissive: bad,
                ..TexturedMaterial::default()
            };
            assert!(material.validate(0).is_err(), "{bad}");
        }
    }
}
