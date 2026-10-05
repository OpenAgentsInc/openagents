//! Builds the textured scene and its navigation blockers from the loaded
//! Everglade pack and the layout's placements.
//!
//! Only the models the layout places, and the materials and images they
//! use, are copied into the scene. The pack carries base color only, so
//! every material is a dielectric: rough for wood, plaster, stone, and
//! leaves, and smooth for the blended window glass.
//!
//! A placement may be painted ([`Paint`]): its kit plaster and roof tiles
//! then sample the pack's neutral images (`village/T_Plaster_Luma` and
//! `village/T_RoundTiles_Luma`, admitted for the generated buildings)
//! tinted by the paint's colors, so kit-built houses vary in color without
//! another model or image.

use super::layout::Placement;
use crate::controller::Footprint;
use crate::pbr::textured::{
    AlphaMode, BaseColorImage, Primitive, TexturedMaterial, TexturedMesh, TexturedScene,
    TexturedVertex, UNBAKED,
};
use crate::zones::everglade_pack::{self, ZonePack};
use std::collections::BTreeMap;

/// Roughness of every opaque and masked material.
const ROUGH: f32 = 0.85;
/// Roughness of blended glass.
const GLASS: f32 = 0.1;

/// Linear colors that replace a kit model's plaster and roof tiles; `None`
/// keeps the kit's own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Paint {
    pub plaster: Option<[f32; 3]>,
    pub roof: Option<[f32; 3]>,
}

impl Paint {
    /// A key that orders and compares paints exactly.
    fn key(self) -> [u32; 6] {
        let bits = |c: Option<[f32; 3]>, i: usize| c.map_or(u32::MAX, |c| c[i].to_bits());
        [
            bits(self.plaster, 0),
            bits(self.plaster, 1),
            bits(self.plaster, 2),
            bits(self.roof, 0),
            bits(self.roof, 1),
            bits(self.roof, 2),
        ]
    }
}

/// The kit materials a paint replaces, in the village set or a generated
/// copy of a kit piece, and the neutral image each samples.
const PAINTED: [(&str, &str); 2] = [
    ("/MI_Plaster", "village/T_Plaster_Luma"),
    ("/MI_RoundTiles", "village/T_RoundTiles_Luma"),
];

/// Pack indices already copied into the scene.
#[derive(Default)]
pub(crate) struct Copied<'a> {
    images: BTreeMap<u16, usize>,
    materials: BTreeMap<(u16, [u32; 6]), usize>,
    /// White materials that a placed model's colors fold into, by image,
    /// alpha mode and cutoff, and face culling.
    folded: BTreeMap<(Option<usize>, u8, u32, bool), usize>,
    meshes: BTreeMap<(&'a str, [u32; 6]), (usize, ([f32; 3], [f32; 3]))>,
}

/// A material's look once its paint is applied: the scene image, the
/// linear base color, and the pack's alpha mode and face culling.
#[derive(Clone, Copy)]
struct Look {
    image: Option<usize>,
    base_color: [f32; 4],
    alpha: everglade_pack::AlphaMode,
    double_sided: bool,
}

/// The textured scene of `placements` and the blockers their collision
/// declares.
///
/// # Errors
///
/// Returns a message when a placement names a model the pack lacks, or the
/// scene exceeds the renderer's bounds.
pub(crate) fn build(
    pack: &ZonePack,
    placements: &[Placement],
) -> Result<(TexturedScene, Vec<Footprint>), String> {
    build_painted(pack, placements, |_| Paint::default())
}

/// [`build`], with each placement's kit plaster and roof tiles painted by
/// `paint`.
///
/// # Errors
///
/// Returns a message when a placement names a model the pack lacks, or the
/// scene exceeds the renderer's bounds.
pub(crate) fn build_painted(
    pack: &ZonePack,
    placements: &[Placement],
    paint: impl Fn(&Placement) -> Paint,
) -> Result<(TexturedScene, Vec<Footprint>), String> {
    let mut scene = TexturedScene::default();
    let mut copied = Copied::default();
    let mut blockers = Vec::new();
    for placement in placements {
        let colors = paint(placement);
        let (mesh, bounds) = match copied.meshes.get(&(placement.model, colors.key())) {
            Some(entry) => *entry,
            None => {
                let model = pack
                    .model(placement.model)
                    .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
                let mesh = copy_model(pack, model, &mut scene, &mut copied, colors)?;
                let entry = (mesh, model.bounds());
                copied.meshes.insert((placement.model, colors.key()), entry);
                entry
            }
        };
        scene.place(mesh, placement.transform());
        blockers.extend(placement.footprints(bounds));
    }
    scene.validate()?;
    Ok((scene, blockers))
}

/// Copies a placed model. Each primitive's base color, after its paint, is
/// folded into its vertex colors, and its material is the white one for its
/// image, alpha, and face culling, so models that differ only in color, such
/// as the generated buildings' tinted plaster and tiles and the painted
/// houses, share a material and merge into the same batch of each cell.
fn copy_model(
    pack: &ZonePack,
    model: &everglade_pack::Model,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
    paint: Paint,
) -> Result<usize, String> {
    let mut primitives = Vec::with_capacity(model.primitives.len());
    for primitive in &model.primitives {
        let look = look(pack, primitive.material, scene, copied, paint)?;
        let key = match look.alpha {
            everglade_pack::AlphaMode::Opaque => (0, 0),
            everglade_pack::AlphaMode::Mask { cutoff } => (1, cutoff.to_bits()),
            everglade_pack::AlphaMode::Blend => (2, 0),
        };
        let key = (look.image, key.0, key.1, look.double_sided);
        let material = match copied.folded.get(&key) {
            Some(&material) => material,
            None => {
                let material = add_material(
                    scene,
                    &Look {
                        base_color: [1.0; 4],
                        ..look
                    },
                );
                copied.folded.insert(key, material);
                material
            }
        };
        let tint = look.base_color;
        primitives.push(Primitive {
            vertices: primitive
                .vertices
                .iter()
                .map(|v| TexturedVertex {
                    pos: v.position,
                    normal: v.normal,
                    uv: v.uv,
                    color: std::array::from_fn(|c| {
                        (f32::from(v.color[c]) * tint[c]).round().clamp(0.0, 255.0) as u8
                    }),
                    light: UNBAKED,
                })
                .collect(),
            indices: primitive.indices.clone(),
            material,
        });
    }
    Ok(scene.add_mesh(TexturedMesh { primitives }))
}

pub(crate) fn copy_material(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
) -> Result<usize, String> {
    copy_painted(pack, index, scene, copied, Paint::default())
}

/// [`copy_material`], with a kit plaster or roof-tile material replaced by
/// its neutral image in `paint`'s color.
pub(crate) fn copy_painted(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
    paint: Paint,
) -> Result<usize, String> {
    let key = (index, paint_key(pack, index, paint)?);
    if let Some(&material) = copied.materials.get(&key) {
        return Ok(material);
    }
    let look = look(pack, index, scene, copied, paint)?;
    let material = add_material(scene, &look);
    copied.materials.insert(key, material);
    Ok(material)
}

/// The paint color that applies to pack material `index`, if any, and the
/// neutral image it samples.
fn painted(
    pack: &ZonePack,
    index: u16,
    paint: Paint,
) -> Result<Option<([f32; 3], &'static str)>, String> {
    let source = pack
        .materials
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing material")?;
    Ok(
        match PAINTED
            .iter()
            .position(|(name, _)| source.name.ends_with(name))
        {
            Some(0) => paint.plaster.map(|c| (c, PAINTED[0].1)),
            Some(_) => paint.roof.map(|c| (c, PAINTED[1].1)),
            None => None,
        },
    )
}

/// The part of a paint that changes pack material `index`.
fn paint_key(pack: &ZonePack, index: u16, paint: Paint) -> Result<[u32; 6], String> {
    Ok(
        painted(pack, index, paint)?.map_or([u32::MAX; 6], |(c, _)| {
            let mut key = [u32::MAX; 6];
            key[..3].copy_from_slice(&c.map(f32::to_bits));
            key
        }),
    )
}

/// Pack material `index` under `paint`, with its image copied into the
/// scene.
fn look(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
    paint: Paint,
) -> Result<Look, String> {
    let source = pack
        .materials
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing material")?;
    let (image, base_color) = match painted(pack, index, paint)? {
        Some((c, luma)) => {
            let texture = pack
                .textures
                .iter()
                .position(|t| t.name == luma)
                .ok_or_else(|| format!("The Everglade pack has no {luma}"))?;
            let texture = u16::try_from(texture).map_err(|_| "Too many textures")?;
            (
                Some(copy_image(pack, texture, scene, copied)?),
                [c[0], c[1], c[2], source.base_color[3]],
            )
        }
        None => (
            match source.texture {
                Some(texture) => Some(copy_image(pack, texture, scene, copied)?),
                None => None,
            },
            source.base_color,
        ),
    };
    Ok(Look {
        image,
        base_color,
        alpha: source.alpha,
        double_sided: source.double_sided,
    })
}

/// Adds the dielectric material `look` describes.
fn add_material(scene: &mut TexturedScene, look: &Look) -> usize {
    let (alpha, roughness) = match look.alpha {
        everglade_pack::AlphaMode::Opaque => (AlphaMode::Opaque, ROUGH),
        everglade_pack::AlphaMode::Mask { cutoff } => (AlphaMode::Mask { cutoff }, ROUGH),
        everglade_pack::AlphaMode::Blend => (AlphaMode::Blend, GLASS),
    };
    scene.add_material(TexturedMaterial {
        image: look.image,
        base_color: look.base_color,
        metallic: 0.0,
        roughness,
        alpha,
        double_sided: look.double_sided,
    })
}

fn copy_image(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
) -> Result<usize, String> {
    if let Some(&image) = copied.images.get(&index) {
        return Ok(image);
    }
    let texture = pack
        .textures
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing texture")?;
    let image = scene.add_image(BaseColorImage {
        name: texture.name.clone(),
        width: texture.width,
        height: texture.height,
        rgba: texture.rgba.clone(),
    });
    copied.images.insert(index, image);
    Ok(image)
}
