//! Typed references to one admitted asset catalog. Rebuilding invalidates every handle.
use std::{
    collections::BTreeMap,
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
/// Texture identities remain pack-local slots until the next asset schema.
#[derive(Debug)]
pub struct Catalog {
    id: u64,
    models: BTreeMap<String, usize>,
    names: Vec<String>,
    textures: usize,
}
impl Catalog {
    pub fn new(pack: &crate::assets::Pack) -> Result<Self, String> {
        pack.validate()?;
        Self::allocate(pack.models.keys().cloned().collect(), pack.textures.len())
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
            textures,
        })
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
