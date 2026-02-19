//! Entity storage and reference counting.

use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use slotmap::{KeyData, SecondaryMap, SlotMap};
use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    cmp::Ordering,
    collections::HashSet,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    num::NonZeroU64,
    sync::{
        Arc, Weak,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    thread::panicking,
};

use derive_more::{Deref, DerefMut};

slotmap::new_key_type! {
    pub struct EntityId;
}

impl From<u64> for EntityId {
    fn from(value: u64) -> Self {
        Self(KeyData::from_ffi(value))
    }
}

impl EntityId {
    pub fn as_non_zero_u64(self) -> NonZeroU64 {
        NonZeroU64::new(self.0.as_ffi()).unwrap()
    }

    pub fn as_u64(self) -> u64 {
        self.0.as_ffi()
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u64())
    }
}

struct EntityRefCounts {
    counts: SlotMap<EntityId, AtomicUsize>,
    dropped_entity_ids: Vec<EntityId>,
}

pub(crate) struct EntityMap {
    entities: SecondaryMap<EntityId, Box<dyn Any>>,
    pub accessed_entities: RefCell<HashSet<EntityId>>,
    ref_counts: Arc<RwLock<EntityRefCounts>>,
}

impl EntityMap {
    pub fn new() -> Self {
        Self {
            entities: SecondaryMap::new(),
            accessed_entities: RefCell::new(HashSet::default()),
            ref_counts: Arc::new(RwLock::new(EntityRefCounts {
                counts: SlotMap::with_key(),
                dropped_entity_ids: Vec::new(),
            })),
        }
    }

    pub fn reserve<T: 'static>(&self) -> Slot<T> {
        let id = self.ref_counts.write().counts.insert(1.into());
        Slot(Entity::new(id, Arc::downgrade(&self.ref_counts)))
    }

    pub fn insert<T>(&mut self, slot: Slot<T>, entity: T) -> Entity<T>
    where
        T: 'static,
    {
        let mut accessed_entities = self.accessed_entities.borrow_mut();
        accessed_entities.insert(slot.entity_id);

        let handle = slot.0;
        self.entities.insert(handle.entity_id, Box::new(entity));
        handle
    }

    #[track_caller]
    pub fn lease<T>(&mut self, pointer: &Entity<T>) -> Lease<T> {
        self.assert_valid_context(pointer);
        let mut accessed_entities = self.accessed_entities.borrow_mut();
        accessed_entities.insert(pointer.entity_id);

        let entity = Some(
            self.entities
                .remove(pointer.entity_id)
                .unwrap_or_else(|| double_lease_panic::<T>("update")),
        );
        Lease {
            entity,
            id: pointer.entity_id,
            entity_type: PhantomData,
        }
    }

    pub fn end_lease<T>(&mut self, mut lease: Lease<T>) {
        self.entities.insert(lease.id, lease.entity.take().unwrap());
    }

    pub fn read<T: 'static>(&self, entity: &Entity<T>) -> &T {
        self.assert_valid_context(entity);
        let mut accessed_entities = self.accessed_entities.borrow_mut();
        accessed_entities.insert(entity.entity_id);

        self.entities
            .get(entity.entity_id)
            .and_then(|entity| entity.downcast_ref())
            .unwrap_or_else(|| double_lease_panic::<T>("read"))
    }

    fn assert_valid_context(&self, entity: &AnyEntity) {
        debug_assert!(
            Weak::ptr_eq(&entity.entity_map, &Arc::downgrade(&self.ref_counts)),
            "used an entity with the wrong context"
        );
    }

    pub fn take_dropped(&mut self) -> Vec<(EntityId, Box<dyn Any>)> {
        let mut ref_counts = self.ref_counts.write();
        let dropped_entity_ids = mem::take(&mut ref_counts.dropped_entity_ids);
        let mut accessed_entities = self.accessed_entities.borrow_mut();

        dropped_entity_ids
            .into_iter()
            .filter_map(|entity_id| {
                let count = ref_counts.counts.remove(entity_id).unwrap();
                debug_assert_eq!(
                    count.load(SeqCst),
                    0,
                    "dropped an entity that was referenced"
                );
                accessed_entities.remove(&entity_id);
                Some((entity_id, self.entities.remove(entity_id)?))
            })
            .collect()
    }
}

impl Default for EntityMap {
    fn default() -> Self {
        Self::new()
    }
}

#[track_caller]
fn double_lease_panic<T>(operation: &str) -> ! {
    panic!(
        "cannot {operation} {} while it is already being updated",
        type_name::<T>()
    )
}

pub(crate) struct Lease<T> {
    entity: Option<Box<dyn Any>>,
    pub id: EntityId,
    entity_type: PhantomData<T>,
}

impl<T: 'static> std::ops::Deref for Lease<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.entity.as_ref().unwrap().downcast_ref().unwrap()
    }
}

impl<T: 'static> std::ops::DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.entity.as_mut().unwrap().downcast_mut().unwrap()
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        if self.entity.is_some() && !panicking() {
            panic!("Leases must be ended with EntityMap::end_lease")
        }
    }
}

#[derive(Deref, DerefMut)]
pub(crate) struct Slot<T>(Entity<T>);

pub struct AnyEntity {
    pub(crate) entity_id: EntityId,
    pub(crate) entity_type: TypeId,
    entity_map: Weak<RwLock<EntityRefCounts>>,
}

impl AnyEntity {
    fn new(id: EntityId, entity_type: TypeId, entity_map: Weak<RwLock<EntityRefCounts>>) -> Self {
        Self {
            entity_id: id,
            entity_type,
            entity_map,
        }
    }

    #[inline]
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    #[inline]
    pub fn entity_type(&self) -> TypeId {
        self.entity_type
    }

    pub fn downgrade(&self) -> AnyWeakEntity {
        AnyWeakEntity {
            entity_id: self.entity_id,
            entity_type: self.entity_type,
            entity_ref_counts: self.entity_map.clone(),
        }
    }

    pub fn downcast<T: 'static>(self) -> Result<Entity<T>, AnyEntity> {
        if TypeId::of::<T>() == self.entity_type {
            Ok(Entity {
                any_entity: self,
                entity_type: PhantomData,
            })
        } else {
            Err(self)
        }
    }
}

impl Clone for AnyEntity {
    fn clone(&self) -> Self {
        if let Some(entity_map) = self.entity_map.upgrade() {
            let entity_map = entity_map.read();
            let count = entity_map
                .counts
                .get(self.entity_id)
                .expect("detected over-release of an entity");
            let prev_count = count.fetch_add(1, SeqCst);
            assert_ne!(prev_count, 0, "Detected over-release of an entity.");
        }

        Self {
            entity_id: self.entity_id,
            entity_type: self.entity_type,
            entity_map: self.entity_map.clone(),
        }
    }
}

impl Drop for AnyEntity {
    fn drop(&mut self) {
        if let Some(entity_map) = self.entity_map.upgrade() {
            let entity_map = entity_map.upgradable_read();
            let count = entity_map
                .counts
                .get(self.entity_id)
                .expect("detected over-release of a handle.");
            let prev_count = count.fetch_sub(1, SeqCst);
            assert_ne!(prev_count, 0, "Detected over-release of an entity.");
            if prev_count == 1 {
                let mut entity_map = RwLockUpgradableReadGuard::upgrade(entity_map);
                entity_map.dropped_entity_ids.push(self.entity_id);
            }
        }
    }
}

impl<T> From<Entity<T>> for AnyEntity {
    #[inline]
    fn from(entity: Entity<T>) -> Self {
        entity.any_entity
    }
}

impl Hash for AnyEntity {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entity_id.hash(state);
    }
}

impl PartialEq for AnyEntity {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entity_id == other.entity_id
    }
}

impl Eq for AnyEntity {}

impl Ord for AnyEntity {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.entity_id.cmp(&other.entity_id)
    }
}

impl PartialOrd for AnyEntity {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for AnyEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyEntity")
            .field("entity_id", &self.entity_id.as_u64())
            .finish()
    }
}

#[derive(Deref, DerefMut)]
pub struct Entity<T> {
    #[deref]
    #[deref_mut]
    pub(crate) any_entity: AnyEntity,
    pub(crate) entity_type: PhantomData<fn(T) -> T>,
}

impl<T: 'static> Entity<T> {
    #[inline]
    fn new(id: EntityId, entity_map: Weak<RwLock<EntityRefCounts>>) -> Self {
        Self {
            any_entity: AnyEntity::new(id, TypeId::of::<T>(), entity_map),
            entity_type: PhantomData,
        }
    }

    #[inline]
    pub fn entity_id(&self) -> EntityId {
        self.any_entity.entity_id
    }

    #[inline]
    pub fn downgrade(&self) -> WeakEntity<T> {
        WeakEntity {
            any_entity: self.any_entity.downgrade(),
            entity_type: self.entity_type,
        }
    }

    #[inline]
    pub fn into_any(self) -> AnyEntity {
        self.any_entity
    }
}

impl<T> Clone for Entity<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            any_entity: self.any_entity.clone(),
            entity_type: self.entity_type,
        }
    }
}

impl<T> std::fmt::Debug for Entity<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entity")
            .field("entity_id", &self.any_entity.entity_id)
            .field("entity_type", &type_name::<T>())
            .finish()
    }
}

impl<T> Hash for Entity<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.any_entity.hash(state);
    }
}

impl<T> PartialEq for Entity<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.any_entity == other.any_entity
    }
}

impl<T> Eq for Entity<T> {}

impl<T> PartialEq<WeakEntity<T>> for Entity<T> {
    #[inline]
    fn eq(&self, other: &WeakEntity<T>) -> bool {
        self.any_entity.entity_id() == other.entity_id()
    }
}

impl<T: 'static> Ord for Entity<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.entity_id().cmp(&other.entity_id())
    }
}

impl<T: 'static> PartialOrd for Entity<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone)]
pub struct AnyWeakEntity {
    pub(crate) entity_id: EntityId,
    entity_type: TypeId,
    entity_ref_counts: Weak<RwLock<EntityRefCounts>>,
}

impl AnyWeakEntity {
    #[inline]
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    pub fn is_upgradable(&self) -> bool {
        let ref_count = self
            .entity_ref_counts
            .upgrade()
            .and_then(|ref_counts| Some(ref_counts.read().counts.get(self.entity_id)?.load(SeqCst)))
            .unwrap_or(0);
        ref_count > 0
    }

    pub fn upgrade(&self) -> Option<AnyEntity> {
        let ref_counts = &self.entity_ref_counts.upgrade()?;
        let ref_counts = ref_counts.read();
        let ref_count = ref_counts.counts.get(self.entity_id)?;

        let prev =
            ref_count.fetch_update(SeqCst, SeqCst, |v| if v == 0 { None } else { Some(v + 1) });

        if prev.is_err() {
            return None;
        }
        drop(ref_counts);

        Some(AnyEntity {
            entity_id: self.entity_id,
            entity_type: self.entity_type,
            entity_map: self.entity_ref_counts.clone(),
        })
    }
}

impl std::fmt::Debug for AnyWeakEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyWeakEntity")
            .field("entity_id", &self.entity_id)
            .field("entity_type", &self.entity_type)
            .finish()
    }
}

impl<T> From<WeakEntity<T>> for AnyWeakEntity {
    #[inline]
    fn from(entity: WeakEntity<T>) -> Self {
        entity.any_entity
    }
}

impl Hash for AnyWeakEntity {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entity_id.hash(state);
    }
}

impl PartialEq for AnyWeakEntity {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entity_id == other.entity_id
    }
}

impl Eq for AnyWeakEntity {}

impl Ord for AnyWeakEntity {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.entity_id.cmp(&other.entity_id)
    }
}

impl PartialOrd for AnyWeakEntity {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Deref, DerefMut)]
pub struct WeakEntity<T> {
    #[deref]
    #[deref_mut]
    any_entity: AnyWeakEntity,
    entity_type: PhantomData<fn(T) -> T>,
}

impl<T> std::fmt::Debug for WeakEntity<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakEntity")
            .field("entity_id", &self.any_entity.entity_id)
            .field("entity_type", &type_name::<T>())
            .finish()
    }
}

impl<T> Clone for WeakEntity<T> {
    fn clone(&self) -> Self {
        Self {
            any_entity: self.any_entity.clone(),
            entity_type: self.entity_type,
        }
    }
}

impl<T: 'static> WeakEntity<T> {
    pub fn upgrade(&self) -> Option<Entity<T>> {
        Some(Entity {
            any_entity: self.any_entity.upgrade()?,
            entity_type: self.entity_type,
        })
    }
}

impl<T> Hash for WeakEntity<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.any_entity.hash(state);
    }
}

impl<T> PartialEq for WeakEntity<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.any_entity == other.any_entity
    }
}

impl<T> Eq for WeakEntity<T> {}

impl<T> PartialEq<Entity<T>> for WeakEntity<T> {
    #[inline]
    fn eq(&self, other: &Entity<T>) -> bool {
        self.entity_id() == other.any_entity.entity_id()
    }
}

impl<T: 'static> Ord for WeakEntity<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.entity_id().cmp(&other.entity_id())
    }
}

impl<T: 'static> PartialOrd for WeakEntity<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEntity {
        pub value: i32,
    }

    #[test]
    fn test_entity_map_reserve_insert() {
        let mut map = EntityMap::new();

        let slot = map.reserve::<TestEntity>();
        let entity = map.insert(slot, TestEntity { value: 42 });

        let read = map.read(&entity);
        assert_eq!(read.value, 42);
    }

    #[test]
    fn test_entity_map_lease() {
        let mut map = EntityMap::new();

        let slot = map.reserve::<TestEntity>();
        let entity = map.insert(slot, TestEntity { value: 10 });

        {
            let mut lease = map.lease(&entity);
            lease.value = 20;
            map.end_lease(lease);
        }

        let read = map.read(&entity);
        assert_eq!(read.value, 20);
    }

    #[test]
    fn test_entity_clone_and_drop() {
        let mut map = EntityMap::new();

        let slot = map.reserve::<TestEntity>();
        let entity1 = map.insert(slot, TestEntity { value: 1 });
        let entity2 = entity1.clone();

        drop(entity1);

        let read = map.read(&entity2);
        assert_eq!(read.value, 1);

        drop(entity2);

        let dropped = map.take_dropped();
        assert_eq!(dropped.len(), 1);
    }

    #[test]
    fn test_weak_entity_upgrade() {
        let mut map = EntityMap::new();

        let slot = map.reserve::<TestEntity>();
        let entity = map.insert(slot, TestEntity { value: 99 });
        let weak = entity.downgrade();

        assert!(weak.upgrade().is_some());

        drop(entity);
        map.take_dropped();

        assert!(weak.upgrade().is_none());
    }
}
