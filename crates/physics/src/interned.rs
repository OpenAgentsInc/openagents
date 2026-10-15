//! Immutable shared data (hull and distance-grid geometry) behind small
//! `Copy` handles, so shapes and colliders stay plain values.
//!
//! A registry is append-only for the life of the process and deduplicates
//! by content: the same data always gets the same handle within a process,
//! so a world serialized (which writes the data itself) and read back
//! compares equal. A handle's number never reaches the simulation, which
//! reads only the data, so results do not depend on the order things were
//! registered in. Entries are never freed; they are assets.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use sha2::{Digest, Sha256};

pub(crate) struct Registry<T> {
    items: RwLock<Vec<Arc<T>>>,
    index: RwLock<BTreeMap<[u8; 32], u32>>,
}

impl<T: serde::Serialize> Registry<T> {
    pub(crate) const fn new() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
            index: RwLock::new(BTreeMap::new()),
        }
    }

    /// The handle for `value`, registering it if it is new.
    pub(crate) fn intern(&self, value: T) -> u32 {
        let bytes = serde_json::to_vec(&value).unwrap_or_default();
        let key: [u8; 32] = Sha256::digest(&bytes).into();
        let mut index = self
            .index
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(&i) = index.get(&key) {
            return i;
        }
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let i = u32::try_from(items.len()).expect("fewer than 2^32 interned shapes");
        items.push(Arc::new(value));
        index.insert(key, i);
        i
    }

    pub(crate) fn get(&self, i: u32) -> Arc<T> {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(&items[i as usize])
    }
}
