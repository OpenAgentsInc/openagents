use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::semantic_refs::CadSemanticRefRegistry;
use crate::{CadError, CadResult};

/// Canonical `.apcad` format tag.
pub const APCAD_FORMAT_TAG: &str = "apcad";

/// Canonical `.apcad` schema version for Wave 1.
pub const APCAD_SCHEMA_VERSION: u32 = 1;

/// File header for `.apcad` documents.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApcadHeader {
    pub format: String,
    pub version: u32,
    pub canonical_unit: String,
}

/// Minimal `.apcad` envelope with deterministic map fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApcadDocumentEnvelope {
    pub header: ApcadHeader,
    pub document_id: String,
    pub stable_ids: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
    pub analysis_cache: Option<BTreeMap<String, String>>,
}

impl ApcadDocumentEnvelope {
    /// Create a new deterministic `.apcad` envelope.
    pub fn new(document_id: impl Into<String>) -> Self {
        Self {
            header: ApcadHeader {
                format: APCAD_FORMAT_TAG.to_string(),
                version: APCAD_SCHEMA_VERSION,
                canonical_unit: crate::policy::CANONICAL_UNIT.to_string(),
            },
            document_id: document_id.into(),
            stable_ids: BTreeMap::new(),
            metadata: BTreeMap::new(),
            analysis_cache: None,
        }
    }

    /// Serialize to compact JSON with deterministic key ordering.
    pub fn to_json(&self) -> CadResult<String> {
        serde_json::to_string(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize .apcad json: {error}"),
        })
    }

    /// Serialize to pretty JSON with deterministic key ordering.
    pub fn to_pretty_json(&self) -> CadResult<String> {
        serde_json::to_string_pretty(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize .apcad pretty json: {error}"),
        })
    }

    /// Deserialize from JSON.
    pub fn from_json(payload: &str) -> CadResult<Self> {
        serde_json::from_str(payload).map_err(|error| CadError::Serialization {
            reason: format!("failed to parse .apcad json: {error}"),
        })
    }

    /// Persist semantic reference registry entries into deterministic stable ids.
    pub fn set_semantic_ref_registry(&mut self, registry: &CadSemanticRefRegistry) {
        self.stable_ids = registry.to_stable_ids();
    }

    /// Recover semantic reference registry from persisted `.apcad` stable ids.
    pub fn semantic_ref_registry(&self) -> CadResult<CadSemanticRefRegistry> {
        CadSemanticRefRegistry::from_stable_ids(self.stable_ids.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{APCAD_FORMAT_TAG, APCAD_SCHEMA_VERSION, ApcadDocumentEnvelope};
    use crate::semantic_refs::CadSemanticRefRegistry;

    #[test]
    fn deterministic_serialization_is_stable_across_runs() {
        let mut left = ApcadDocumentEnvelope::new("doc-1");
        left.stable_ids
            .insert("feature.mount_holes".to_string(), "sid-002".to_string());
        left.stable_ids
            .insert("feature.base".to_string(), "sid-001".to_string());
        left.metadata
            .insert("material".to_string(), "6061-T6".to_string());
        left.metadata
            .insert("title".to_string(), "Rack".to_string());

        let mut right = ApcadDocumentEnvelope::new("doc-1");
        right
            .stable_ids
            .insert("feature.base".to_string(), "sid-001".to_string());
        right
            .stable_ids
            .insert("feature.mount_holes".to_string(), "sid-002".to_string());
        right
            .metadata
            .insert("title".to_string(), "Rack".to_string());
        right
            .metadata
            .insert("material".to_string(), "6061-T6".to_string());

        let left_json = left.to_json();
        let right_json = right.to_json();

        assert_eq!(
            left_json, right_json,
            "BTreeMap-backed serialization must be deterministic"
        );
    }

    #[test]
    fn header_defaults_match_apcad_contract() {
        let envelope = ApcadDocumentEnvelope::new("doc-2");
        assert_eq!(envelope.header.format, APCAD_FORMAT_TAG);
        assert_eq!(envelope.header.version, APCAD_SCHEMA_VERSION);
        assert_eq!(envelope.header.canonical_unit, "mm");
    }

    #[test]
    fn round_trip_deserialization_preserves_content() {
        let mut envelope = ApcadDocumentEnvelope::new("doc-3");
        envelope
            .metadata
            .insert("objective".to_string(), "airflow".to_string());
        envelope.analysis_cache = Some(std::collections::BTreeMap::from([(
            "weight_kg".to_string(),
            "2.71".to_string(),
        )]));

        let serialized_result = envelope.to_pretty_json();
        assert!(
            serialized_result.is_ok(),
            "serialization should succeed for valid envelope"
        );

        let serialized = match serialized_result {
            Ok(payload) => payload,
            Err(_) => return,
        };

        let parsed_result = ApcadDocumentEnvelope::from_json(&serialized);
        assert!(parsed_result.is_ok(), "deserialization should succeed");

        if let Ok(parsed) = parsed_result {
            assert_eq!(parsed, envelope);
        }
    }

    #[test]
    fn semantic_ref_registry_round_trips_through_apcad_stable_ids() {
        let mut registry = CadSemanticRefRegistry::default();
        registry
            .register("rack_outer_face", "face.1", "feature.base")
            .expect("register should succeed");
        registry
            .register("mount_hole_pattern", "pattern.1", "feature.mount_holes")
            .expect("register should succeed");

        let mut envelope = ApcadDocumentEnvelope::new("doc-semantic");
        envelope.set_semantic_ref_registry(&registry);
        let recovered = envelope
            .semantic_ref_registry()
            .expect("registry should recover from stable ids");
        assert_eq!(recovered.to_stable_ids(), registry.to_stable_ids());
    }
}
