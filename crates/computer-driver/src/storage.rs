//! Provider storage facts. A partial inventory never proves that an object
//! disappeared, and byte counts with different meanings are not interchangeable.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_STORAGE_OBJECTS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Disk,
    Snapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageObjectState {
    Present,
    Deleting,
    Deleted,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageByteKind {
    ProvisionedCapacity,
    ProviderArchive,
    ProviderReported,
}

/// The API field that supplied the fact, not a claim about billed storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageSource {
    GceDisk,
    GceImage,
    SpaceSandbox,
    SpaceSnapshot,
    Test,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageCoverage {
    Partial,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageCleanup {
    Unknown,
    Pending,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageObject {
    /// Immutable provider object ID. Names and lifecycle checkpoint IDs do not qualify.
    pub id: String,
    pub kind: StorageKind,
    pub state: StorageObjectState,
    /// `None` means unknown; `Some(0)` is an observed zero.
    pub bytes: Option<u64>,
    pub byte_kind: StorageByteKind,
    pub source: StorageSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageInventory {
    pub kind: StorageKind,
    /// Complete means all private objects of this kind, including objects
    /// being deleted. Finishing a filtered or paginated list is insufficient.
    pub coverage: StorageCoverage,
    pub source: StorageSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageObservation {
    pub computer: String,
    pub objects: Vec<StorageObject>,
    pub inventories: Vec<StorageInventory>,
    /// A deleted VM alone does not establish storage cleanup.
    pub cleanup: StorageCleanup,
}

pub(crate) fn valid_storage_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

impl StorageObservation {
    /// Validate before persistence. A caller also compares `computer` with
    /// the requested resource and obtains its owner from its own records.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_storage_id(&self.computer) || self.objects.len() > MAX_STORAGE_OBJECTS {
            return Err("invalid storage observation bounds");
        }
        if self.inventories.len() > 2 {
            return Err("invalid storage inventories");
        }
        let mut inventories = BTreeSet::new();
        for inventory in &self.inventories {
            if !inventories.insert(inventory.kind)
                || !valid_source(inventory.kind, inventory.source)
            {
                return Err("invalid storage inventory");
            }
        }
        let mut objects = BTreeSet::new();
        for object in &self.objects {
            if !valid_storage_id(&object.id)
                || !objects.insert((object.kind, &object.id))
                || object.bytes.is_some_and(|bytes| bytes > i64::MAX as u64)
                || !valid_source(object.kind, object.source)
                || !self.inventories.iter().any(|inventory| {
                    inventory.kind == object.kind && inventory.source == object.source
                })
            {
                return Err("invalid storage object");
            }
            let expected = match object.source {
                StorageSource::GceDisk | StorageSource::SpaceSandbox => {
                    Some(StorageByteKind::ProvisionedCapacity)
                }
                StorageSource::GceImage => Some(StorageByteKind::ProviderArchive),
                StorageSource::SpaceSnapshot => Some(StorageByteKind::ProviderReported),
                StorageSource::Test => None,
            };
            if expected.is_some_and(|kind| kind != object.byte_kind) {
                return Err("invalid storage byte semantics");
            }
        }
        if self.cleanup == StorageCleanup::Complete
            && (self.inventories.len() != 2
                || self
                    .inventories
                    .iter()
                    .any(|inventory| inventory.coverage != StorageCoverage::Complete)
                || self
                    .objects
                    .iter()
                    .any(|object| object.state != StorageObjectState::Deleted))
        {
            return Err("storage cleanup is not established");
        }
        Ok(())
    }
}

fn valid_source(kind: StorageKind, source: StorageSource) -> bool {
    matches!(
        (kind, source),
        (
            StorageKind::Disk,
            StorageSource::GceDisk | StorageSource::SpaceSandbox
        ) | (
            StorageKind::Snapshot,
            StorageSource::GceImage | StorageSource::SpaceSnapshot
        ) | (_, StorageSource::Test)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> StorageObservation {
        StorageObservation {
            computer: "sbx_example".into(),
            objects: vec![StorageObject {
                id: "snap_example".into(),
                kind: StorageKind::Snapshot,
                state: StorageObjectState::Present,
                bytes: None,
                byte_kind: StorageByteKind::ProviderReported,
                source: StorageSource::SpaceSnapshot,
            }],
            inventories: vec![StorageInventory {
                kind: StorageKind::Snapshot,
                coverage: StorageCoverage::Partial,
                source: StorageSource::SpaceSnapshot,
            }],
            cleanup: StorageCleanup::Unknown,
        }
    }

    #[test]
    fn unknown_bytes_and_observed_zero_remain_distinct() {
        let mut observation = observation();
        let unknown = serde_json::to_value(&observation).unwrap();
        observation.objects[0].bytes = Some(0);
        assert_eq!(unknown["objects"][0]["bytes"], serde_json::Value::Null);
        assert_eq!(
            serde_json::to_value(&observation).unwrap()["objects"][0]["bytes"],
            0
        );
        assert_eq!(observation.validate(), Ok(()));
        observation.objects[0].bytes = Some(i64::MAX as u64 + 1);
        assert!(observation.validate().is_err());
    }

    #[test]
    fn duplicate_ids_wrong_sources_and_capacity_confusion_are_rejected() {
        let mut observation = observation();
        observation.objects.push(observation.objects[0].clone());
        assert!(observation.validate().is_err());
        observation.objects.pop();
        observation.objects[0].byte_kind = StorageByteKind::ProvisionedCapacity;
        assert!(observation.validate().is_err());
        observation.objects[0].byte_kind = StorageByteKind::ProviderReported;
        observation.objects[0].id = "https://provider/private?token=secret".into();
        assert!(observation.validate().is_err());
    }

    #[test]
    fn cleanup_requires_complete_private_inventory_and_no_unresolved_objects() {
        let mut observation = observation();
        observation.cleanup = StorageCleanup::Complete;
        assert!(observation.validate().is_err());
        observation.objects.clear();
        observation.inventories[0].coverage = StorageCoverage::Complete;
        observation.inventories.push(StorageInventory {
            kind: StorageKind::Disk,
            coverage: StorageCoverage::Complete,
            source: StorageSource::SpaceSandbox,
        });
        assert_eq!(observation.validate(), Ok(()));
    }

    #[test]
    fn oversized_observations_are_rejected_before_persistence() {
        let mut observation = observation();
        observation.objects = (0..=MAX_STORAGE_OBJECTS)
            .map(|index| StorageObject {
                id: format!("snap_{index}"),
                ..observation.objects[0].clone()
            })
            .collect();
        assert!(observation.validate().is_err());
    }

    #[tokio::test]
    async fn unsupported_storage_stays_unknown_through_arc_forwarding() {
        use crate::{Driver, Outcome, fake::FakeDriver};
        let directory = tempfile::tempdir().unwrap();
        let driver = std::sync::Arc::new(FakeDriver::new(directory.path()));
        assert!(matches!(
            driver.storage("computer").await,
            Outcome::Unknown { .. }
        ));
    }
}
