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
//! call; nothing depends on GPU instancing. Materials follow glTF 2.0's
//! metallic-roughness model at its simplest: a base-color image times a
//! linear factor and the vertex color, uniform metallic and roughness, an
//! alpha mode (opaque, masked at a cutoff, or blended), and a double-sided
//! flag. Normal, occlusion, metallic-roughness, and emissive images are not
//! read.
//!
//! Textured meshes draw only in physical frames: a frame with a
//! [`super::Sky`], or a [`super::Neon`] stage with a studio [`super::Key`].
//! Opaque cells draw first, then masked cells, then blended cells from the
//! farthest to the nearest. Triangles inside one blended cell are not sorted.
//! Opaque and masked cells cast sun or key shadows; blended cells do not.
//!
//! The shaders are the textured entries of `photo.wgsl`. They use only the
//! OpenGL ES 3.0 features every backend requests (see `crate::gles`), so
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
/// Most base-color images in one scene.
pub const MAX_IMAGES: usize = 64;
/// Most materials in one scene.
pub const MAX_MATERIALS: usize = 1024;
/// Most placements in one scene.
pub const MAX_PLACEMENTS: usize = 1 << 16;
/// Most bytes of merged vertices and indices, the zone geometry bound.
pub const MAX_BYTES: usize = 96 * 1024 * 1024;

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
}

impl TexturedVertex {
    /// A white vertex.
    #[must_use]
    pub fn new(pos: Vec3, normal: Vec3, uv: [f32; 2]) -> Self {
        Self {
            pos: pos.to_array(),
            normal: normal.to_array(),
            uv,
            color: [255; 4],
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
        match self {
            Self::Opaque => true,
            Self::Mask { cutoff } => alpha >= cutoff,
            Self::Blend => alpha > 0.0,
        }
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
    pub(crate) const ALL: [Self; 3] = [Self::Opaque, Self::Masked, Self::Blended];
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
}

/// Everything a zone's textured static geometry needs, in one value.
#[derive(Clone, Default, PartialEq)]
pub struct TexturedScene {
    pub images: Vec<BaseColorImage>,
    pub materials: Vec<TexturedMaterial>,
    pub meshes: Vec<TexturedMesh>,
    pub placements: Vec<Placement>,
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
        self.placements.push(Placement { mesh, transform });
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
        {
            return Err("textured scene exceeds its image, material, or placement bound".into());
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
            {
                return Err("textured placement has an invalid mesh or transform".into());
            }
        }
        let bytes = self.placements.iter().try_fold(0usize, |sum, placement| {
            self.meshes[placement.mesh]
                .primitives
                .iter()
                .try_fold(sum, |sum, p| {
                    let size = p
                        .vertices
                        .len()
                        .checked_mul(std::mem::size_of::<TexturedVertex>())?
                        .checked_add(p.indices.len().checked_mul(4)?)?;
                    sum.checked_add(size)
                })
        });
        if bytes.is_none_or(|bytes| bytes > MAX_BYTES) {
            return Err("textured scene exceeds its GPU bounds".into());
        }
        Ok(())
    }

    /// The smallest cutoff among masked materials that use image `image`,
    /// which its mip chain preserves coverage at.
    pub(crate) fn mask_cutoff(&self, image: usize) -> Option<f32> {
        self.materials
            .iter()
            .filter(|m| m.image == Some(image))
            .filter_map(|m| match m.alpha {
                AlphaMode::Mask { cutoff } if cutoff > 0.0 => Some(cutoff),
                _ => None,
            })
            .reduce(f32::min)
    }

    /// Merges every placement into world-space cells, one per pass,
    /// material, and [`CELL`]. A mirroring transform reverses its triangles'
    /// winding so front faces stay counterclockwise.
    ///
    /// # Errors
    ///
    /// Returns the validation error when the scene is out of bounds.
    pub(crate) fn merge(&self) -> Result<Merged, String> {
        self.validate()?;
        type Cell = (Vec<TexturedVertex>, Vec<u32>);
        let mut cells: BTreeMap<(Pass, usize, i32, i32), Cell> = BTreeMap::new();
        for placement in &self.placements {
            let t = placement.transform;
            let normals = Mat3::from_mat4(t).inverse().transpose();
            let mirrored = t.determinant() < 0.0;
            let cell = (
                (t.w_axis.x / CELL).floor() as i32,
                (t.w_axis.z / CELL).floor() as i32,
            );
            for p in &self.meshes[placement.mesh].primitives {
                let pass = self.materials[p.material].alpha.pass();
                let (vertices, indices) =
                    cells.entry((pass, p.material, cell.0, cell.1)).or_default();
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
        let mut merged = Merged::default();
        for ((_, material, _, _), (vertices, indices)) in cells {
            if indices.is_empty() {
                continue;
            }
            let base = merged.vertices.len() as u32;
            let first = merged.indices.len() as u32;
            let (min, max) = vertices.iter().fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(min, max), v| (min.min(v.pos.into()), max.max(v.pos.into())),
            );
            merged.indices.extend(indices.iter().map(|i| i + base));
            merged.vertices.extend(vertices);
            merged.batches.push(Batch {
                material,
                first,
                count: merged.indices.len() as u32 - first,
                min,
                max,
            });
        }
        Ok(merged)
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
        let fail = |message: &dyn std::fmt::Display| format!("{}: {message}", path.display());
        let gltf = gltf::Gltf::open(path).map_err(|e| fail(&e))?;
        let buffers =
            gltf::import_buffers(&gltf.document, path.parent(), gltf.blob).map_err(|e| fail(&e))?;
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
        let image = match pbr.base_color_texture() {
            Some(info) if info.tex_coord() != 0 => {
                return Err(format!(
                    "{}: a base-color image uses a second texture coordinate set",
                    path.display()
                ));
            }
            Some(info) => Some(self.import_image(path, buffers, info.texture().source())?),
            None => None,
        };
        let alpha = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask {
                cutoff: material.alpha_cutoff().unwrap_or(0.5),
            },
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
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
        if let Some(index) = self.images.iter().position(|i| i.name == name) {
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
                name,
                width,
                height,
                rgba,
            },
            MAX_IMAGE_SIZE,
        );
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
    pub(crate) fn merged(&self) -> Merged {
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
                });
            }
        }
        merged
    }
}

/// A merged scene: world-space vertices and indices, and the cells that
/// draw ranges of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Merged {
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<Batch>,
}

/// One cell of one material: a range of the merged indices and its bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Batch {
    pub material: usize,
    pub first: u32,
    pub count: u32,
    pub min: Vec3,
    pub max: Vec3,
}

/// The order cells draw in: opaque, then masked, each in merge order so
/// cells of one material draw together, then blended cells from the
/// farthest center to the nearest, so nearer glass composites over farther
/// glass. `visible` drops culled cells.
pub(crate) fn draw_order(
    batches: &[Batch],
    materials: &[TexturedMaterial],
    eye: Vec3,
    visible: impl Fn(&Batch) -> bool,
) -> Vec<usize> {
    let pass = |i: usize| materials[batches[i].material].alpha.pass();
    let distance = |i: usize| ((batches[i].min + batches[i].max) * 0.5).distance_squared(eye);
    let mut order: Vec<usize> = (0..batches.len())
        .filter(|&i| visible(&batches[i]))
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
pub(crate) fn in_frustum(min: Vec3, max: Vec3, view_proj: Mat4) -> bool {
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

/// How a pass's cells rasterize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Raster {
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
pub(crate) fn raster(pass: Pass, double_sided: bool) -> Raster {
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
pub(crate) fn uniform(material: &TexturedMaterial) -> [[f32; 4]; 2] {
    [
        material.base_color,
        [
            material.metallic,
            material.roughness,
            material.alpha.cutoff(),
            0.0,
        ],
    ]
}

/// The sRGB transfer function's decoding of each 8-bit value.
fn srgb_to_linear() -> &'static [f32; 256] {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}

fn linear_to_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// Halves an RGBA8 image with a 2×2 box filter: color averaged in linear
/// light, alpha averaged as is. Odd edges repeat their last texel.
fn halve(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let table = srgb_to_linear();
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let texels = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| {
                let sx = (2 * x + dx).min(width - 1) as usize;
                let sy = (2 * y + dy).min(height - 1) as usize;
                (sy * width as usize + sx) * 4
            });
            for c in 0..3 {
                let sum: f32 = texels.iter().map(|&t| table[rgba[t + c] as usize]).sum();
                out.push(linear_to_srgb(sum * 0.25));
            }
            let alpha: u32 = texels.iter().map(|&t| u32::from(rgba[t + 3])).sum();
            out.push(((alpha + 2) / 4) as u8);
        }
    }
    (w, h, out)
}

/// Halves `image` until both sides are at most `max` texels.
fn fit(mut image: BaseColorImage, max: u32) -> BaseColorImage {
    while image.width > max || image.height > max {
        let (width, height, rgba) = halve(image.width, image.height, &image.rgba);
        image = BaseColorImage {
            width,
            height,
            rgba,
            ..image
        };
    }
    image
}

/// The fraction of texels whose alpha, scaled by `scale`, reaches `cutoff`.
fn coverage(rgba: &[u8], cutoff: f32, scale: f32) -> f32 {
    let texels = rgba.len() / 4;
    if texels == 0 {
        return 0.0;
    }
    let kept = rgba
        .chunks_exact(4)
        .filter(|t| f32::from(t[3]) / 255.0 * scale >= cutoff)
        .count();
    kept as f32 / texels as f32
}

/// Scales a mip level's alpha by the smallest factor that brings its
/// coverage at `cutoff` up to `wanted` (Castaño, "Computing Alpha Mipmaps",
/// 2010), so masked foliage keeps its density with distance instead of
/// thinning out as averaged alpha falls below the cutoff.
fn preserve_coverage(rgba: &mut [u8], cutoff: f32, wanted: f32) {
    if wanted <= 0.0 || coverage(rgba, cutoff, 1.0) >= wanted {
        return;
    }
    // Coverage only grows with the scale, and at 256 every nonzero alpha
    // passes any cutoff up to 1.
    let (mut low, mut high) = (1.0f32, 256.0f32);
    for _ in 0..24 {
        let middle = (low + high) * 0.5;
        if coverage(rgba, cutoff, middle) >= wanted {
            high = middle;
        } else {
            low = middle;
        }
    }
    for texel in rgba.chunks_exact_mut(4) {
        texel[3] = (f32::from(texel[3]) * high).ceil().min(255.0) as u8;
    }
}

/// An image's mip chain, largest first, down to one texel, without levels
/// wider or taller than `max`. With a mask `cutoff`, each level keeps the
/// full image's alpha coverage at that cutoff.
pub(crate) fn mip_chain(
    image: &BaseColorImage,
    cutoff: Option<f32>,
    max: u32,
) -> Vec<(u32, u32, Vec<u8>)> {
    let wanted = cutoff.map(|cutoff| (cutoff, coverage(&image.rgba, cutoff, 1.0)));
    let mut levels = Vec::new();
    let mut current = (image.width, image.height, image.rgba.clone());
    loop {
        let (width, height) = (current.0, current.1);
        let next = (width > 1 || height > 1).then(|| {
            let mut next = halve(width, height, &current.2);
            if let Some((cutoff, wanted)) = wanted {
                preserve_coverage(&mut next.2, cutoff, wanted);
            }
            next
        });
        if width <= max && height <= max {
            levels.push(current);
        }
        match next {
            Some(next) => current = next,
            None => break,
        }
    }
    levels
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

    fn quaternius(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/verse/props/quaternius")
            .join(format!("{name}.gltf"))
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
        let order = draw_order(&merged.batches, &scene.materials, eye, |_| true);
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
        let near = draw_order(&merged.batches, &scene.materials, eye, |b| b.min.z > -10.0);
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
        let fitted = fit(image, 4);
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
}
