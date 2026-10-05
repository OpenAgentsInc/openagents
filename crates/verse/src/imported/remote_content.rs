//! Portable content admission shared with the dedicated host.
#[cfg(test)]
use sha2::{Digest, Sha256};
pub use verse_content::remote_content::*;
#[cfg(test)]
use verse_engine::director::Scene;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equipment_preflight_requires_static_models_and_parent_sockets() {
        use verse_world::service::equipment::{Catalog, Gear, Slot};
        let dir = tempfile::tempdir().unwrap();
        let pack = verse_content::compiler::original::generate(dir.path()).unwrap();
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut catalog = Catalog {
            version: 1,
            gear: vec![Gear {
                id: 3,
                name: "Ritual hat".into(),
                slot: Slot::Head,
                model: "gear-hat".into(),
                offset: [0, 0, 230],
                health: 100,
                mana: 0,
            }],
        };
        equipment_models(&pack, &scene, &Default::default(), &catalog).unwrap();
        let mut missing = pack.clone();
        missing
            .models
            .get_mut("adventurer")
            .unwrap()
            .attachments
            .retain(|a| a.id != 5);
        assert!(equipment_models(&missing, &scene, &Default::default(), &catalog).is_err());
        catalog.gear[0].model = "missing-model".into();
        assert!(equipment_models(&pack, &scene, &Default::default(), &catalog).is_err());
        catalog.gear[0].model = "adventurer".into();
        assert!(equipment_models(&pack, &scene, &Default::default(), &catalog).is_err());
        catalog.gear[0].model = "gear-hat".into();
        let outfits = verse_world::service::outfits::Catalog {
            version: 1,
            outfits: vec![verse_world::service::outfits::Outfit {
                id: 2,
                name: "Broken outfit".into(),
                model: "missing-rig".into(),
            }],
        };
        assert!(equipment_models(&pack, &scene, &outfits, &catalog).is_err());
    }
    #[test]
    fn outfit_admission_refuses_missing_models_and_animation_states() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = verse_content::compiler::original::generate(dir.path()).unwrap();
        let catalog = verse_world::service::outfits::Catalog {
            version: 1,
            outfits: vec![verse_world::service::outfits::Outfit {
                id: 2,
                name: "Test outfit".into(),
                model: "adventurer".into(),
            }],
        };
        outfit_models(&pack, &catalog).unwrap();
        assert!(outfit_model(&pack, "missing-outfit").is_err());
        pack.models
            .get_mut("adventurer")
            .unwrap()
            .states
            .remove(&verse_engine::motion::State::Walk);
        assert!(outfit_models(&pack, &catalog).is_err());
    }
    #[test]
    fn identity_binds_scene_geometry_and_verified_texture_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = verse_content::compiler::original::generate(dir.path()).unwrap();
        let mut scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let original = identity(&pack, &scene, dir.path()).unwrap();
        assert_eq!(original, identity(&pack, &scene, dir.path()).unwrap());
        let relocated = tempfile::tempdir().unwrap();
        for texture in &pack.textures {
            std::fs::copy(
                dir.path().join(&texture.file),
                relocated.path().join(&texture.file),
            )
            .unwrap();
        }
        assert_eq!(original, identity(&pack, &scene, relocated.path()).unwrap());
        scene.actors[0].position.x += 1.;
        assert_ne!(original, identity(&pack, &scene, dir.path()).unwrap());
        scene.actors[0].position.x -= 1.;
        pack.source_revision.push_str("-changed");
        assert_ne!(original, identity(&pack, &scene, dir.path()).unwrap());
        pack.source_revision
            .truncate(pack.source_revision.len() - 8);
        assert!(!pack.textures.is_empty());
        let path = dir.path().join(&pack.textures[0].file);
        let before = std::fs::read(&path).unwrap();
        let mut modified = before.clone();
        modified[0] ^= 1;
        std::fs::write(&path, &modified).unwrap();
        assert!(identity(&pack, &scene, dir.path()).is_err());
        pack.textures[0].sha256 = format!("{:x}", Sha256::digest(&modified));
        assert_ne!(original, identity(&pack, &scene, dir.path()).unwrap());
        std::fs::remove_file(path).unwrap();
        assert!(identity(&pack, &scene, dir.path()).is_err());
    }
}
