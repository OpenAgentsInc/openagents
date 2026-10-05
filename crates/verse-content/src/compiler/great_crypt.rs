//! The great crypt's models as static pack models, placed by
//! `verse_world::great_crypt::LAYOUT`, for the cultist fight
//! (`verse --crypt-fight`).
//!
//! Each model is the original glTF that `scripts/blender/great_crypt.py` or
//! `scripts/blender/chamber_lab.py` wrote, imported as a rigid model. A
//! surface that glows (flames, embers, glowing liquid, the summoning
//! circle, the night beyond the window) keeps its glTF emission scaled by
//! its `KHR_materials_emissive_strength`, so a candle flame outshines a
//! cauldron's liquid as it does in the crypt lab.
use glam::{Mat4, Quat, Vec3};
use std::path::Path;
use verse_engine::assets::{Bone, Pack, Placement};
use verse_world::great_crypt::{CHAMBER_MODELS, CRYPT_MODELS, LAYOUT};

/// The pack name of a crypt model.
#[must_use]
pub fn model_name(name: &str) -> String {
    format!("crypt/{name}")
}

/// How bright an emissive surface draws for a glTF emission strength in
/// cd/m²: a candle flame (1800) draws about 7 times its color.
fn glow(strength: f32) -> f32 {
    (strength / 250.0).clamp(0.3, 10.0)
}

/// Each primitive's emission strength, in the order the importer reads
/// them: the document's nodes, then each mesh's primitives.
fn strengths(name: &str, glb: &[u8]) -> Result<Vec<f32>, String> {
    let gltf = gltf::Gltf::from_slice(glb).map_err(|e| format!("{name}: {e}"))?;
    let mut out = Vec::new();
    for node in gltf.document.nodes() {
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                out.push(primitive.material().emissive_strength().unwrap_or(1.0));
            }
        }
    }
    Ok(out)
}

/// Imports every crypt model into `pack` as a rigid model, with textures
/// written to `dir`, and replaces the pack's placements with the great
/// crypt's layout. `glb` gives each model's binary glTF file by name: the
/// files built into the binary (`verse_zone_crypt::great_crypt_glb`), or
/// files read from `assets/verse/generated`.
///
/// # Errors
///
/// Returns a message when a model is missing or cannot be imported.
pub fn install<'a>(
    pack: &mut Pack,
    dir: &Path,
    glb: impl Fn(&str) -> Option<&'a [u8]>,
) -> Result<(), String> {
    for name in CRYPT_MODELS.iter().chain(CHAMBER_MODELS) {
        let bytes = glb(name).ok_or_else(|| format!("The great crypt has no model {name}"))?;
        let strengths = strengths(name, bytes)?;
        let mut model =
            super::characters::import_bytes(pack, dir, &format!("{name}.glb"), bytes, None)?;
        if strengths.len() != model.surfaces.len() {
            return Err(format!("{name}: primitives and surfaces disagree"));
        }
        for (surface, strength) in model.surfaces.iter_mut().zip(strengths) {
            let factor = Vec3::from(surface.material.emissive_factor);
            if factor.max_element() > 0.01 {
                surface.emissive = true;
                surface.tint = (factor * glow(strength)).to_array();
            }
            for vertex in &mut surface.vertices {
                vertex.joints = [0; 4];
                vertex.weights = [1.0, 0.0, 0.0, 0.0];
            }
        }
        model.skin = None;
        model.bones = vec![Bone {
            parent: -1,
            pivot: [0.0; 3],
        }];
        model.clips.clear();
        pack.models.insert(model_name(name), model);
    }
    let basis = crate::basis();
    let inverse = basis.inverse();
    pack.placements = LAYOUT
        .iter()
        .map(|&(name, x, y, z, yaw)| {
            // The importer turns a model half a turn about Y; turn it back.
            let turn = inverse * Mat4::from_rotation_y(yaw - std::f32::consts::PI) * basis;
            Placement {
                model: model_name(name),
                position: inverse.transform_point3(Vec3::new(x, y, z)).to_array(),
                rotation: Quat::from_mat4(&turn).normalize().to_array(),
                scale: 1.0,
            }
        })
        .collect();
    pack.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crypt_models_import_and_place() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = crate::compiler::original::generate(dir.path()).unwrap();
        let generated = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated");
        let files: std::collections::BTreeMap<&str, Vec<u8>> = CRYPT_MODELS
            .iter()
            .chain(CHAMBER_MODELS)
            .map(|&name| {
                let path = generated
                    .join(verse_world::great_crypt::model_folder(name))
                    .join(format!("{name}.glb"));
                (name, std::fs::read(path).unwrap())
            })
            .collect();
        install(&mut pack, dir.path(), |name| {
            files.get(name).map(Vec::as_slice)
        })
        .unwrap();
        assert_eq!(pack.placements.len(), LAYOUT.len());
        let hall = &pack.models[&model_name("great_crypt_hall")];
        assert!(hall.surfaces.iter().all(|s| !s.indices.is_empty()));
        // Flames glow brighter than the summoning circle's runes' base.
        let candles = &pack.models[&model_name("candelabrum_tall")];
        assert!(
            candles
                .surfaces
                .iter()
                .any(|s| s.emissive && s.tint[0] > 3.0)
        );
        let circle = &pack.models[&model_name("summoning_circle")];
        assert!(circle.surfaces.iter().any(|s| s.emissive));
        // A placement maps back to its world position through the basis.
        let world = crate::basis().transform_point3(Vec3::from(pack.placements[1].position));
        assert!(world.distance(Vec3::new(LAYOUT[1].1, LAYOUT[1].2, LAYOUT[1].3)) < 1e-3);
    }

    #[test]
    fn a_missing_model_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = crate::compiler::original::generate(dir.path()).unwrap();
        let error = install(&mut pack, dir.path(), |_| None).unwrap_err();
        assert_eq!(error, "The great crypt has no model great_crypt_hall");
    }
}
