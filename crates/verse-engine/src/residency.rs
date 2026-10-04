//! Typed references to one admitted asset catalog. Rebuilding invalidates every handle.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CATALOG: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct CatalogId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ModelHandle {
    catalog: u64,
    slot: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TextureHandle {
    catalog: u64,
    slot: usize,
}

/// Model keys are logical identities; source paths and content digests are separate.
/// Inventories provide persistent IDs; legacy packs retain name and slot lookup.
#[derive(Debug)]
pub struct Catalog {
    id: u64,
    models: BTreeMap<String, usize>,
    names: Vec<String>,
    animation_states: BTreeMap<String, BTreeSet<crate::motion::State>>,
    sockets: BTreeMap<String, BTreeSet<u16>>,
    textures: usize,
    persistent_models: BTreeMap<crate::inventory::AssetId, usize>,
    model_ids: BTreeMap<String, crate::inventory::AssetId>,
    persistent_textures: BTreeMap<crate::inventory::AssetId, usize>,
}
impl Catalog {
    pub fn new(pack: &crate::assets::Pack) -> Result<Self, String> {
        pack.validate()?;
        let mut catalog =
            Self::allocate(pack.models.keys().cloned().collect(), pack.textures.len())?;
        catalog.animation_states = pack
            .models
            .iter()
            .map(|(name, model)| (name.clone(), model.states.keys().copied().collect()))
            .collect();
        catalog.sockets = pack
            .models
            .iter()
            .map(|(name, model)| {
                (
                    name.clone(),
                    model.attachments.iter().map(|a| a.id).collect(),
                )
            })
            .collect();
        if let Some(inventory) = &pack.inventory {
            for asset in &inventory.assets {
                match &asset.binding {
                    crate::inventory::Binding::Model { key } => {
                        catalog.model_ids.insert(key.clone(), asset.id.clone());
                        catalog
                            .persistent_models
                            .insert(asset.id.clone(), catalog.models[key]);
                    }
                    crate::inventory::Binding::Texture { slot } => {
                        catalog.persistent_textures.insert(asset.id.clone(), *slot);
                    }
                    crate::inventory::Binding::Source { .. } => {}
                }
            }
        }
        Ok(catalog)
    }
    fn allocate(names: Vec<String>, textures: usize) -> Result<Self, String> {
        let id = NEXT_CATALOG
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "Asset catalog identities exhausted".to_string())?;
        let models = names
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), slot))
            .collect();
        Ok(Self {
            id,
            models,
            names,
            animation_states: BTreeMap::new(),
            sockets: BTreeMap::new(),
            textures,
            persistent_models: BTreeMap::new(),
            model_ids: BTreeMap::new(),
            persistent_textures: BTreeMap::new(),
        })
    }
    /// Named states must be declared by the admitted model. Numeric selections
    /// retain the research adapter's idle/rest fallback.
    pub fn check_animation(
        &self,
        model: ModelHandle,
        selection: crate::motion::Selection,
    ) -> Result<(), String> {
        let name = self.model_name(model)?;
        if let crate::motion::Selection::Named(state) = selection
            && !self
                .animation_states
                .get(name)
                .is_some_and(|states| states.contains(&state))
        {
            return Err(format!("Missing animation state for {name}: {state:?}"));
        }
        Ok(())
    }
    pub fn check_socket(&self, model: ModelHandle, socket: u16) -> Result<(), String> {
        let name = self.model_name(model)?;
        if !self.sockets.get(name).is_some_and(|s| s.contains(&socket)) {
            return Err("Attachment socket is missing from the admitted model".into());
        }
        Ok(())
    }
    pub fn model_asset(&self, id: &crate::inventory::AssetId) -> Result<ModelHandle, String> {
        let slot = *self
            .persistent_models
            .get(id)
            .ok_or("Missing persistent model identity")?;
        Ok(ModelHandle {
            catalog: self.id,
            slot,
        })
    }
    pub fn texture_asset(&self, id: &crate::inventory::AssetId) -> Result<TextureHandle, String> {
        self.texture(
            *self
                .persistent_textures
                .get(id)
                .ok_or("Missing persistent texture identity")?,
        )
    }
    pub fn id(&self) -> CatalogId {
        CatalogId(self.id)
    }
    pub fn check(&self, id: CatalogId) -> Result<(), String> {
        if id != self.id() {
            return Err("Stale or foreign asset catalog".into());
        }
        Ok(())
    }
    pub fn model(&self, name: &str) -> Result<ModelHandle, String> {
        if let Some(id) = self.model_ids.get(name) {
            return self.model_asset(id);
        }
        let slot = *self
            .models
            .get(name)
            .ok_or_else(|| format!("Missing logical model: {name}"))?;
        Ok(ModelHandle {
            catalog: self.id,
            slot,
        })
    }
    pub fn texture(&self, slot: usize) -> Result<TextureHandle, String> {
        if slot >= self.textures {
            return Err("Texture slot is outside the catalog".into());
        }
        Ok(TextureHandle {
            catalog: self.id,
            slot,
        })
    }
    pub fn model_name(&self, handle: ModelHandle) -> Result<&str, String> {
        if handle.catalog != self.id {
            return Err("Stale or foreign model handle".into());
        }
        self.names
            .get(handle.slot)
            .map(String::as_str)
            .ok_or_else(|| "Invalid model handle".into())
    }
    pub fn texture_slot(&self, handle: TextureHandle) -> Result<usize, String> {
        if handle.catalog != self.id || handle.slot >= self.textures {
            return Err("Stale or foreign texture handle".into());
        }
        Ok(handle.slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebuilding_rejects_old_handles_even_when_names_match() {
        let old = Catalog::allocate(vec!["adventurer".into(), "claude".into()], 2).unwrap();
        let new = Catalog::allocate(vec!["adventurer".into(), "claude".into()], 2).unwrap();
        assert!(new.check(old.id()).is_err());
        assert!(new.check(new.id()).is_ok());
        assert!(new.model_name(old.model("claude").unwrap()).is_err());
        assert!(new.texture_slot(old.texture(0).unwrap()).is_err());
        assert_eq!(
            new.model_name(new.model("claude").unwrap()).unwrap(),
            "claude"
        );
    }
    #[test]
    fn reordered_slots_never_relabel_old_references() {
        let old = Catalog::allocate(vec!["claude".into()], 1).unwrap();
        let new = Catalog::allocate(vec!["adventurer".into(), "claude".into()], 1).unwrap();
        assert!(new.check(old.id()).is_err());
        assert!(new.check(new.id()).is_ok());
        assert!(new.model_name(old.model("claude").unwrap()).is_err());
        assert_eq!(
            new.model_name(new.model("claude").unwrap()).unwrap(),
            "claude"
        );
        assert!(new.model("missing").is_err());
        assert!(new.texture(1).is_err());
    }
}
