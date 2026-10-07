use serde::{Deserialize, Serialize};

use psionic_ir::TassadarComputationalModuleManifest;

/// Provider-facing receipt for one computational module manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleManifestReceipt {
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// Stable module reference.
    pub module_ref: String,
    /// Stable ABI family.
    pub abi_family: String,
    /// Explicit claim class.
    pub claim_class: String,
    /// Stable trust posture label.
    pub trust_posture: psionic_ir::TassadarModuleTrustPosture,
    /// Number of imported symbols.
    pub import_count: u32,
    /// Number of exported symbols.
    pub export_count: u32,
    /// Number of state fields.
    pub state_field_count: u32,
    /// Number of benchmark lineage refs.
    pub benchmark_ref_count: u32,
    /// Stable compatibility digest.
    pub compatibility_digest: String,
    /// Stable manifest digest.
    pub manifest_digest: String,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarModuleManifestReceipt {
    /// Projects a provider-facing receipt from the shared computational module manifest.
    #[must_use]
    pub fn from_manifest(manifest: &TassadarComputationalModuleManifest) -> Self {
        Self {
            manifest_id: manifest.manifest_id.clone(),
            module_ref: manifest.module_ref.clone(),
            abi_family: manifest.abi_family.clone(),
            claim_class: manifest.claim_class.clone(),
            trust_posture: manifest.trust_posture,
            import_count: manifest.imports.len() as u32,
            export_count: manifest.exports.len() as u32,
            state_field_count: manifest.state_fields.len() as u32,
            benchmark_ref_count: manifest.benchmark_lineage_refs.len() as u32,
            compatibility_digest: manifest.compatibility_digest.clone(),
            manifest_digest: manifest.manifest_digest.clone(),
            detail: format!(
                "module manifest `{}` publishes {} imports, {} exports, {} state fields, and {} benchmark refs under trust posture {:?}",
                manifest.module_ref,
                manifest.imports.len(),
                manifest.exports.len(),
                manifest.state_fields.len(),
                manifest.benchmark_lineage_refs.len(),
                manifest.trust_posture,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModuleManifestReceipt;
    use psionic_ir::seeded_tassadar_computational_module_manifests;

    #[test]
    fn module_manifest_receipt_projects_shared_manifest() {
        let manifest = seeded_tassadar_computational_module_manifests()
            .into_iter()
            .find(|manifest| manifest.module_ref == "candidate_select_core@1.1.0")
            .expect("candidate manifest");
        let receipt = TassadarModuleManifestReceipt::from_manifest(&manifest);

        assert_eq!(receipt.import_count, 1);
        assert_eq!(receipt.export_count, 1);
        assert_eq!(receipt.state_field_count, 1);
        assert_eq!(receipt.benchmark_ref_count, 2);
        assert_eq!(receipt.module_ref, "candidate_select_core@1.1.0");
    }
}
