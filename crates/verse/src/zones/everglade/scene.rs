//! Builds the textured scene and its navigation blockers from the loaded
//! Everglade pack and the layout's placements.
//!
//! Only the models the layout places, and the materials and images they
//! use, are copied into the scene. The pack carries base color only, so
//! every material is a dielectric: rough for wood, plaster, stone, and
//! leaves, and smooth for the blended window glass.

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

/// Pack indices already copied into the scene.
#[derive(Default)]
pub(crate) struct Copied<'a> {
    images: BTreeMap<u16, usize>,
    materials: BTreeMap<u16, usize>,
    meshes: BTreeMap<&'a str, (usize, ([f32; 3], [f32; 3]))>,
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
    let mut scene = TexturedScene::default();
    let mut copied = Copied::default();
    let mut blockers = Vec::new();
    for placement in placements {
        let (mesh, bounds) = match copied.meshes.get(placement.model) {
            Some(entry) => *entry,
            None => {
                let model = pack
                    .model(placement.model)
                    .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
                let mesh = copy_model(pack, model, &mut scene, &mut copied)?;
                let entry = (mesh, model.bounds());
                copied.meshes.insert(placement.model, entry);
                entry
            }
        };
        scene.place(mesh, placement.transform());
        blockers.extend(placement.footprints(bounds));
    }
    scene.validate()?;
    Ok((scene, blockers))
}

fn copy_model(
    pack: &ZonePack,
    model: &everglade_pack::Model,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
) -> Result<usize, String> {
    let mut primitives = Vec::with_capacity(model.primitives.len());
    for primitive in &model.primitives {
        let material = copy_material(pack, primitive.material, scene, copied)?;
        primitives.push(Primitive {
            vertices: primitive
                .vertices
                .iter()
                .map(|v| TexturedVertex {
                    pos: v.position,
                    normal: v.normal,
                    uv: v.uv,
                    color: v.color,
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
    if let Some(&material) = copied.materials.get(&index) {
        return Ok(material);
    }
    let source = pack
        .materials
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing material")?;
    let image = match source.texture {
        Some(texture) => Some(copy_image(pack, texture, scene, copied)?),
        None => None,
    };
    let (alpha, roughness) = match source.alpha {
        everglade_pack::AlphaMode::Opaque => (AlphaMode::Opaque, ROUGH),
        everglade_pack::AlphaMode::Mask { cutoff } => (AlphaMode::Mask { cutoff }, ROUGH),
        everglade_pack::AlphaMode::Blend => (AlphaMode::Blend, GLASS),
    };
    let material = scene.add_material(TexturedMaterial {
        image,
        base_color: source.base_color,
        metallic: 0.0,
        roughness,
        alpha,
        double_sided: source.double_sided,
    });
    copied.materials.insert(index, material);
    Ok(material)
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
