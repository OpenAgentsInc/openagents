//! Persistent asset identities and attributable provenance declarations.
//! Admission validates declared dependencies and rights; it is not legal attestation.
use crate::assets::Pack;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct AssetId(String);
impl AssetId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 160
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".:/-_".contains(&b))
        {
            return Err("Invalid persistent asset identity".into());
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum License {
    Apache2,
    Cc0,
    Ofl11,
    OwnerSuppliedLocal,
    Research,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub creator: String,
    pub license: License,
    pub revision: String,
    pub format: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    Model { key: String },
    Texture { slot: usize },
    Source { origin: Origin },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub binding: Binding,
    pub sha256: String,
    pub bytes: u64,
    pub dependencies: Vec<AssetId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub version: u32,
    pub compiler: AssetId,
    pub compiler_revision: String,
    pub assets: Vec<Asset>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    OriginalLocal,
    Capture,
    Redistribute,
}
#[derive(Clone, Debug, Serialize)]
pub struct Admission {
    pub schema: &'static str,
    pub purpose: Purpose,
    pub compiler: AssetId,
    pub compiler_revision: String,
    pub assets: Vec<AssetId>,
    pub origins: Vec<(AssetId, Origin)>,
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
impl Inventory {
    pub fn validate(&self, pack: &Pack) -> Result<(), String> {
        if self.version != 1
            || !digest_valid(&self.compiler_revision)
            || self.assets.len() > 1024
            || self.assets.is_empty()
        {
            return Err("Invalid asset inventory header or budget".into());
        }
        let mut ids = BTreeMap::new();
        let mut models = BTreeSet::new();
        let mut textures = BTreeSet::new();
        for asset in &self.assets {
            if ids.insert(&asset.id, asset).is_some()
                || !digest_valid(&asset.sha256)
                || asset.bytes == 0
                || asset.dependencies.len() > 32
            {
                return Err("Invalid or duplicate inventory asset".into());
            }
            if asset.dependencies.iter().collect::<BTreeSet<_>>().len() != asset.dependencies.len()
            {
                return Err("Duplicate asset dependency".into());
            }
            match &asset.binding {
                Binding::Model { key } => {
                    if !pack.models.contains_key(key)
                        || !models.insert(key)
                        || asset.dependencies.is_empty()
                    {
                        return Err("Invalid inventory model binding".into());
                    }
                }
                Binding::Texture { slot } => {
                    if *slot >= pack.textures.len()
                        || !textures.insert(slot)
                        || asset.dependencies.is_empty()
                    {
                        return Err("Invalid inventory texture binding".into());
                    }
                }
                Binding::Source { origin } => {
                    if !text_valid(&origin.creator)
                        || !text_valid(&origin.revision)
                        || !text_valid(&origin.format)
                    {
                        return Err("Invalid source provenance declaration".into());
                    }
                }
            }
        }
        if models.len() != pack.models.len() || textures.len() != pack.textures.len() {
            return Err("Inventory does not cover the complete pack".into());
        }
        for asset in &self.assets {
            for dependency in &asset.dependencies {
                if !ids.contains_key(dependency) {
                    return Err("Missing asset dependency".into());
                }
            }
            if let Binding::Model { key } = &asset.binding {
                for slot in pack.models[key]
                    .surfaces
                    .iter()
                    .flat_map(|surface| surface.texture_slots())
                {
                    let texture = self
                        .assets
                        .iter()
                        .find(|a| a.binding == Binding::Texture { slot })
                        .ok_or("Missing material texture identity")?;
                    if !asset.dependencies.contains(&texture.id) {
                        return Err("Inventory omits a material texture dependency".into());
                    }
                }
            }
        }
        // Inspect every declaration, including unreachable records, for cycles.
        fn visit<'a>(
            id: &'a AssetId,
            ids: &BTreeMap<&'a AssetId, &'a Asset>,
            active: &mut BTreeSet<&'a AssetId>,
            done: &mut BTreeSet<&'a AssetId>,
            depth: usize,
        ) -> Result<(), String> {
            if done.contains(id) {
                return Ok(());
            }
            if depth > 128 || !active.insert(id) {
                return Err("Cyclic or excessively deep asset dependency graph".into());
            }
            for child in &ids[id].dependencies {
                visit(child, ids, active, done, depth + 1)?;
            }
            active.remove(id);
            done.insert(id);
            Ok(())
        }
        let mut done = BTreeSet::new();
        for asset in &self.assets {
            visit(&asset.id, &ids, &mut BTreeSet::new(), &mut done, 0)?;
        }
        Ok(())
    }
    pub fn roots(&self) -> Vec<AssetId> {
        self.assets.iter().map(|a| a.id.clone()).collect()
    }
    pub fn admit(
        &self,
        pack: &Pack,
        purpose: Purpose,
        roots: &[AssetId],
    ) -> Result<Admission, String> {
        self.validate(pack)?;
        if roots.is_empty() || roots.len() > 1024 {
            return Err("Invalid asset admission roots".into());
        }
        let ids: BTreeMap<_, _> = self.assets.iter().map(|a| (&a.id, a)).collect();
        let mut used = BTreeSet::new();
        let mut pending: Vec<_> = roots.iter().collect();
        while let Some(id) = pending.pop() {
            let asset = ids.get(id).ok_or("Unknown asset admission root")?;
            if used.insert(id.clone()) {
                pending.extend(asset.dependencies.iter());
            }
        }
        let mut origins = vec![];
        for id in &used {
            if let Binding::Source { origin } = &ids[id].binding {
                let format = origin.format.to_ascii_lowercase();
                if origin.license == License::Research
                    || ["mpq", "m2", "wmo", "blp", "dbc", "adt", "wdt", "vmangos"]
                        .contains(&format.as_str())
                {
                    return Err(format!(
                        "Research provenance is not admitted: {}",
                        id.as_str()
                    ));
                }
                if purpose == Purpose::Redistribute && origin.license == License::OwnerSuppliedLocal
                {
                    return Err(format!(
                        "Local-only provenance cannot be redistributed: {}",
                        id.as_str()
                    ));
                }
                origins.push((id.clone(), origin.clone()));
            }
        }
        if origins.is_empty() {
            return Err("Asset closure has no provenance".into());
        }
        Ok(Admission {
            schema: "openagents.verse.asset-admission.v1",
            purpose,
            compiler: self.compiler.clone(),
            compiler_revision: self.compiler_revision.clone(),
            assets: used.into_iter().collect(),
            origins,
        })
    }
    #[cfg(feature = "asset-io")]
    pub fn verify(&self, pack: &Pack) -> Result<(), String> {
        self.validate(pack)?;
        for asset in &self.assets {
            match &asset.binding {
                Binding::Model { key } => {
                    let (sha256, bytes) = fingerprint(&pack.models[key])?;
                    if sha256 != asset.sha256 || bytes != asset.bytes {
                        return Err("Inventory model content digest or length mismatch".into());
                    }
                }
                Binding::Texture { slot } => {
                    if pack.textures[*slot].sha256 != asset.sha256 {
                        return Err("Inventory texture digest mismatch".into());
                    }
                }
                Binding::Source { .. } => {}
            }
        }
        Ok(())
    }
}
#[cfg(feature = "asset-io")]
pub fn fingerprint(value: &impl Serialize) -> Result<(String, u64), String> {
    use sha2::{Digest, Sha256};
    struct Writer {
        hash: Sha256,
        bytes: u64,
    }
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= 128 * 1024 * 1024)
                .ok_or_else(|| std::io::Error::other("Inventory artifact exceeds 128 MiB"))?;
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer {
        hash: Sha256::new(),
        bytes: 0,
    };
    serde_json::to_writer(&mut writer, value).map_err(|e| e.to_string())?;
    Ok((format!("{:x}", writer.hash.finalize()), writer.bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{Model, Surface, Texture, Vertex};
    fn aid(name: &str) -> AssetId {
        AssetId::new(name).unwrap()
    }
    fn fixture() -> (Pack, Inventory) {
        let model = Model {
            graph: None,
            markers: Vec::new(),
            states: Default::default(),
            skin: None,
            source: "original.mesh".into(),
            source_sha256: String::new(),
            height: 1.,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                material: Default::default(),
                vertices: vec![
                    Vertex {
                        position: [0.; 3],
                        normal: [0., 1., 0.],
                        uv: [0.; 2],
                        joints: [0; 4],
                        weights: [1., 0., 0., 0.]
                    };
                    3
                ],
                indices: vec![0, 1, 2],
                texture: 0,
                blend: 0,
                emissive: false,
                tint: [1.; 3],
            }],
        };
        let pack = Pack {
            inventory: None,
            version: 1,
            source_revision: "fixture".into(),
            models: BTreeMap::from([("fixture".into(), model)]),
            textures: vec![Texture {
                file: "old.png".into(),
                sha256: "a".repeat(64),
                width: 1,
                height: 1,
            }],
            placements: vec![],
        };
        let inventory = Inventory {
            version: 1,
            compiler: aid("compiler"),
            compiler_revision: "b".repeat(64),
            assets: vec![
                Asset {
                    id: aid("source"),
                    binding: Binding::Source {
                        origin: Origin {
                            creator: "Fixture author".into(),
                            license: License::Cc0,
                            revision: "1".into(),
                            format: "gltf".into(),
                        },
                    },
                    sha256: "c".repeat(64),
                    bytes: 1,
                    dependencies: vec![],
                },
                Asset {
                    id: aid("texture"),
                    binding: Binding::Texture { slot: 0 },
                    sha256: "a".repeat(64),
                    bytes: 1,
                    dependencies: vec![aid("source")],
                },
                Asset {
                    id: aid("model"),
                    binding: Binding::Model {
                        key: "fixture".into(),
                    },
                    sha256: "d".repeat(64),
                    bytes: 1,
                    dependencies: vec![aid("texture")],
                },
            ],
        };
        (pack, inventory)
    }
    #[test]
    fn every_authored_material_map_requires_a_provenance_dependency() {
        for channel in 0..4 {
            let (mut pack, mut inventory) = fixture();
            let mut map = pack.textures[0].clone();
            map.file = "material-map.png".into();
            pack.textures.push(map);
            let material = &mut pack.models.get_mut("fixture").unwrap().surfaces[0].material;
            match channel {
                0 => material.normal_texture = Some(1),
                1 => material.metallic_roughness_texture = Some(1),
                2 => material.occlusion_texture = Some(1),
                _ => material.emissive_texture = Some(1),
            }
            let mut map_asset = inventory.assets[1].clone();
            map_asset.id = aid("material-map");
            map_asset.binding = Binding::Texture { slot: 1 };
            inventory.assets.push(map_asset);
            assert!(
                inventory
                    .admit(&pack, Purpose::Capture, &inventory.roots())
                    .unwrap_err()
                    .contains("material texture dependency")
            );
            inventory.assets[2].dependencies.push(aid("material-map"));
            assert!(
                inventory
                    .admit(&pack, Purpose::Capture, &inventory.roots())
                    .is_ok()
            );
            pack.models.get_mut("fixture").unwrap().surfaces[0]
                .material
                .normal_texture = Some(2);
            assert!(pack.validate().is_err());
        }
    }
    #[test]
    fn renamed_research_and_local_only_origins_cannot_cross_rights_gates() {
        let (pack, mut inventory) = fixture();
        inventory
            .admit(&pack, Purpose::Redistribute, &inventory.roots())
            .unwrap();
        if let Binding::Source { origin } = &mut inventory.assets[0].binding {
            origin.license = License::OwnerSuppliedLocal;
        }
        inventory
            .admit(&pack, Purpose::Capture, &inventory.roots())
            .unwrap();
        assert!(
            inventory
                .admit(&pack, Purpose::Redistribute, &inventory.roots())
                .unwrap_err()
                .contains("Local-only")
        );
        inventory.assets[0].id = aid("renamed-original-source");
        inventory.assets[1].dependencies[0] = inventory.assets[0].id.clone();
        if let Binding::Source { origin } = &mut inventory.assets[0].binding {
            origin.license = License::Research;
        }
        assert!(
            inventory
                .admit(&pack, Purpose::Capture, &inventory.roots())
                .unwrap_err()
                .contains("Research")
        );
        if let Binding::Source { origin } = &mut inventory.assets[0].binding {
            origin.license = License::Cc0;
            origin.format = "BLP".into();
        }
        assert!(
            inventory
                .admit(&pack, Purpose::OriginalLocal, &inventory.roots())
                .is_err()
        );
    }
    #[test]
    fn graph_refuses_cycles_missing_edges_duplicate_ids_and_omitted_textures() {
        let (pack, inventory) = fixture();
        let mut bad = inventory.clone();
        bad.assets[0].dependencies.push(aid("model"));
        assert!(bad.validate(&pack).unwrap_err().contains("Cyclic"));
        let mut bad = inventory.clone();
        bad.assets[2].dependencies = vec![aid("source")];
        assert!(
            bad.validate(&pack)
                .unwrap_err()
                .contains("material texture")
        );
        let mut bad = inventory.clone();
        bad.assets[0].dependencies.push(aid("missing"));
        assert!(bad.validate(&pack).is_err());
        let mut bad = inventory.clone();
        bad.assets.push(bad.assets[0].clone());
        assert!(bad.validate(&pack).is_err());
        let mut bad = inventory.clone();
        bad.assets[2].dependencies.push(aid("texture"));
        assert!(bad.validate(&pack).is_err());
        assert!(AssetId::new("has space").is_err());
        assert!(serde_json::from_str::<AssetId>("\"unicode-☃\"").is_err());
    }
    #[test]
    fn persistent_resolution_survives_container_and_file_relocation() {
        let (mut pack, mut inventory) = fixture();
        pack.inventory = Some(inventory.clone());
        let old = crate::residency::Catalog::new(&pack).unwrap();
        let old_handle = old.model_asset(&aid("model")).unwrap();
        let model = pack.models.remove("fixture").unwrap();
        pack.models.insert("moved".into(), model);
        pack.textures[0].file = "moved.png".into();
        inventory.assets[2].binding = Binding::Model {
            key: "moved".into(),
        };
        pack.inventory = Some(inventory);
        let next = crate::residency::Catalog::new(&pack).unwrap();
        assert_eq!(
            next.model_name(next.model_asset(&aid("model")).unwrap())
                .unwrap(),
            "moved"
        );
        assert_eq!(
            next.texture_slot(next.texture_asset(&aid("texture")).unwrap())
                .unwrap(),
            0
        );
        assert!(next.model_name(old_handle).is_err());
    }
    #[test]
    fn texture_identity_survives_reordered_storage() {
        let (mut pack, mut inventory) = fixture();
        pack.textures.push(crate::assets::Texture {
            file: "second.png".into(),
            sha256: "e".repeat(64),
            width: 1,
            height: 1,
        });
        inventory.assets.push(Asset {
            id: aid("second-texture"),
            binding: Binding::Texture { slot: 1 },
            sha256: "e".repeat(64),
            bytes: 1,
            dependencies: vec![aid("source")],
        });
        pack.textures.swap(0, 1);
        pack.models.get_mut("fixture").unwrap().surfaces[0].texture = 1;
        inventory.assets[1].binding = Binding::Texture { slot: 1 };
        inventory.assets[3].binding = Binding::Texture { slot: 0 };
        pack.inventory = Some(inventory);
        let catalog = crate::residency::Catalog::new(&pack).unwrap();
        assert_eq!(
            catalog
                .texture_slot(catalog.texture_asset(&aid("texture")).unwrap())
                .unwrap(),
            1
        );
    }
    #[cfg(feature = "asset-io")]
    #[test]
    fn compiled_model_mutation_requires_a_new_content_fingerprint() {
        let (mut pack, mut inventory) = fixture();
        let (sha, bytes) = fingerprint(&pack.models["fixture"]).unwrap();
        inventory.assets[2].sha256 = sha;
        inventory.assets[2].bytes = bytes;
        inventory.verify(&pack).unwrap();
        pack.models.get_mut("fixture").unwrap().surfaces[0].tint[0] = 0.2;
        assert!(
            inventory
                .verify(&pack)
                .unwrap_err()
                .contains("model content")
        );
    }
}
