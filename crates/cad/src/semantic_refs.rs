use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadSemanticRefStatus {
    #[serde(rename = "valid")]
    Valid,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "rebound")]
    Rebound,
}

impl CadSemanticRefStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Expired => "expired",
            Self::Rebound => "rebound",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadSemanticRefEntry {
    pub semantic_ref: String,
    pub entity_id: String,
    pub source_feature_id: String,
    pub status: CadSemanticRefStatus,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadSemanticRefRegistry {
    pub entries: BTreeMap<String, CadSemanticRefEntry>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CadSemanticRefReconcileReceipt {
    pub inserted: usize,
    pub retained: usize,
    pub rebound: usize,
    pub expired: usize,
}

impl CadSemanticRefRegistry {
    pub fn register(
        &mut self,
        semantic_ref: impl Into<String>,
        entity_id: impl Into<String>,
        source_feature_id: impl Into<String>,
    ) -> CadResult<()> {
        let semantic_ref = semantic_ref.into();
        validate_semantic_ref_name(&semantic_ref)?;
        let entity_id = sanitize_entity_id(entity_id.into())?;
        let source_feature_id = sanitize_feature_id(source_feature_id.into())?;

        let next = CadSemanticRefEntry {
            semantic_ref: semantic_ref.clone(),
            entity_id: entity_id.clone(),
            source_feature_id,
            status: CadSemanticRefStatus::Valid,
        };

        match self.entries.get_mut(&semantic_ref) {
            Some(current) => {
                if current.entity_id != entity_id {
                    current.entity_id = entity_id;
                    current.status = CadSemanticRefStatus::Rebound;
                } else {
                    current.status = CadSemanticRefStatus::Valid;
                }
            }
            None => {
                self.entries.insert(semantic_ref, next);
            }
        }

        Ok(())
    }

    pub fn mark_expired(&mut self, semantic_ref: &str) -> bool {
        let Some(entry) = self.entries.get_mut(semantic_ref) else {
            return false;
        };
        entry.status = CadSemanticRefStatus::Expired;
        true
    }

    pub fn resolve(&self, semantic_ref: &str) -> Option<&CadSemanticRefEntry> {
        let entry = self.entries.get(semantic_ref)?;
        if entry.status == CadSemanticRefStatus::Expired {
            return None;
        }
        Some(entry)
    }

    pub fn to_stable_ids(&self) -> BTreeMap<String, String> {
        self.entries
            .iter()
            .filter_map(|(semantic_ref, entry)| {
                (entry.status != CadSemanticRefStatus::Expired)
                    .then_some((semantic_ref.clone(), entry.entity_id.clone()))
            })
            .collect()
    }

    pub fn from_stable_ids(stable_ids: BTreeMap<String, String>) -> CadResult<Self> {
        let mut registry = CadSemanticRefRegistry::default();
        for (semantic_ref, entity_id) in stable_ids {
            registry.register(semantic_ref, entity_id, "feature.unknown")?;
        }
        Ok(registry)
    }

    pub fn reconcile_with_stable_ids(
        &mut self,
        stable_ids: &BTreeMap<String, String>,
    ) -> CadResult<CadSemanticRefReconcileReceipt> {
        for semantic_ref in stable_ids.keys() {
            validate_semantic_ref_name(semantic_ref)?;
        }

        let mut receipt = CadSemanticRefReconcileReceipt::default();

        for (semantic_ref, entry) in &mut self.entries {
            match stable_ids.get(semantic_ref) {
                Some(entity_id) => {
                    if entry.entity_id == *entity_id {
                        entry.status = CadSemanticRefStatus::Valid;
                        receipt.retained = receipt.retained.saturating_add(1);
                    } else {
                        entry.entity_id.clone_from(entity_id);
                        entry.status = CadSemanticRefStatus::Rebound;
                        receipt.rebound = receipt.rebound.saturating_add(1);
                    }
                }
                None => {
                    entry.status = CadSemanticRefStatus::Expired;
                    receipt.expired = receipt.expired.saturating_add(1);
                }
            }
        }

        for (semantic_ref, entity_id) in stable_ids {
            if self.entries.contains_key(semantic_ref) {
                continue;
            }
            self.entries.insert(
                semantic_ref.clone(),
                CadSemanticRefEntry {
                    semantic_ref: semantic_ref.clone(),
                    entity_id: entity_id.clone(),
                    source_feature_id: "feature.unknown".to_string(),
                    status: CadSemanticRefStatus::Valid,
                },
            );
            receipt.inserted = receipt.inserted.saturating_add(1);
        }

        Ok(receipt)
    }
}

fn sanitize_entity_id(value: String) -> CadResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "semantic ref entity id must not be empty".to_string(),
        });
    }
    Ok(trimmed.to_string())
}

fn sanitize_feature_id(value: String) -> CadResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "semantic ref source feature id must not be empty".to_string(),
        });
    }
    Ok(trimmed.to_string())
}

fn validate_semantic_ref_name(semantic_ref: &str) -> CadResult<()> {
    let trimmed = semantic_ref.trim();
    if trimmed.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "semantic ref must not be empty".to_string(),
        });
    }
    let valid = trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '.' | '-'));
    if !valid {
        return Err(CadError::InvalidFeatureGraph {
            reason: format!(
                "semantic ref '{trimmed}' must use lowercase ascii letters, digits, '_', '.', or '-'"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CadSemanticRefRegistry, CadSemanticRefStatus};
    use std::collections::BTreeMap;

    #[test]
    fn register_and_resolve_is_deterministic() {
        let mut registry = CadSemanticRefRegistry::default();
        registry
            .register("rack_outer_face", "face.1", "feature.base")
            .expect("register should succeed");
        registry
            .register("mount_hole_pattern", "pattern.1", "feature.mount_holes")
            .expect("register should succeed");

        let outer = registry
            .resolve("rack_outer_face")
            .expect("semantic ref should resolve");
        assert_eq!(outer.entity_id, "face.1");
        assert_eq!(outer.status, CadSemanticRefStatus::Valid);

        let stable_ids = registry.to_stable_ids();
        assert_eq!(
            stable_ids.get("rack_outer_face"),
            Some(&"face.1".to_string())
        );
        assert_eq!(
            stable_ids.get("mount_hole_pattern"),
            Some(&"pattern.1".to_string())
        );
    }

    #[test]
    fn reconcile_marks_expired_and_rebound_refs() {
        let mut registry = CadSemanticRefRegistry::default();
        registry
            .register("rack_outer_face", "face.1", "feature.base")
            .expect("register should succeed");
        registry
            .register("mount_hole_pattern", "pattern.1", "feature.mount_holes")
            .expect("register should succeed");

        let receipt = registry
            .reconcile_with_stable_ids(&BTreeMap::from([
                ("rack_outer_face".to_string(), "face.9".to_string()),
                ("vent_face_set".to_string(), "faces.3".to_string()),
            ]))
            .expect("reconcile should succeed");

        assert_eq!(receipt.rebound, 1);
        assert_eq!(receipt.expired, 1);
        assert_eq!(receipt.inserted, 1);

        let outer = registry
            .entries
            .get("rack_outer_face")
            .expect("outer face should exist");
        assert_eq!(outer.entity_id, "face.9");
        assert_eq!(outer.status, CadSemanticRefStatus::Rebound);

        let mount = registry
            .entries
            .get("mount_hole_pattern")
            .expect("mount pattern should exist");
        assert_eq!(mount.status, CadSemanticRefStatus::Expired);

        let vent = registry
            .entries
            .get("vent_face_set")
            .expect("vent face set should be inserted");
        assert_eq!(vent.status, CadSemanticRefStatus::Valid);
    }

    #[test]
    fn registry_round_trips_through_stable_ids_for_apcad_persistence() {
        let stable_ids = BTreeMap::from([
            ("rack_outer_face".to_string(), "face.1".to_string()),
            ("mount_hole_pattern".to_string(), "pattern.1".to_string()),
        ]);
        let registry = CadSemanticRefRegistry::from_stable_ids(stable_ids.clone())
            .expect("registry should parse from stable ids");
        assert_eq!(registry.to_stable_ids(), stable_ids);
    }

    #[test]
    fn routine_rebuild_reconcile_keeps_refs_stable_when_ids_do_not_change() {
        let mut registry = CadSemanticRefRegistry::default();
        registry
            .register("rack_outer_face", "face.1", "feature.base")
            .expect("register should succeed");
        registry
            .register("mount_hole_pattern", "pattern.1", "feature.mount_holes")
            .expect("register should succeed");
        let stable_before = registry.to_stable_ids();

        let receipt = registry
            .reconcile_with_stable_ids(&stable_before)
            .expect("reconcile should succeed");
        assert_eq!(receipt.inserted, 0);
        assert_eq!(receipt.rebound, 0);
        assert_eq!(receipt.expired, 0);
        assert_eq!(receipt.retained, 2);
        assert_eq!(registry.to_stable_ids(), stable_before);
        assert_eq!(
            registry.entries["rack_outer_face"].status,
            CadSemanticRefStatus::Valid
        );
    }

    #[test]
    fn invalid_semantic_ref_names_are_rejected() {
        let mut registry = CadSemanticRefRegistry::default();
        let error = registry
            .register("Rack Outer Face", "face.1", "feature.base")
            .expect_err("invalid semantic ref should fail");
        assert!(error.to_string().contains("semantic ref 'Rack Outer Face'"));
    }
}
