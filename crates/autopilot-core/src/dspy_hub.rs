//! DSPy Hub for storing and loading optimized modules.
//!
//! Provides a storage layer for compiled DSPy modules, enabling:
//! - Saving optimized modules with their demos
//! - Loading modules by compiled_id or signature name
//! - Querying for promoted modules
//! - A/B routing support
//!
//! # Directory Structure
//!
//! ```text
//! ~/.openagents/dspy/
//! ├── optimized/
//! │   └── {signature_name}/
//! │       └── {compiled_id}.json    # Manifest + demos
//! └── training/
//!     └── {signature_name}/
//!         └── {date}.jsonl          # Training examples
//! ```

use anyhow::{Context, Result};
use dsrs::Example;
use dsrs::evaluate::promotion::PromotionState;
use dsrs::manifest::CompiledModuleManifest;
use serde::{Deserialize, Serialize};
use std::fs::{self, create_dir_all};
use std::path::PathBuf;

/// Routing strategy for A/B testing and shadow mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    /// Always use the promoted module.
    Promoted,

    /// Run candidate in shadow mode alongside promoted.
    Shadow {
        /// The candidate module to compare.
        candidate_id: String,
    },

    /// Route a percentage of requests to the candidate.
    ABTest {
        /// Percentage of requests to route to candidate (0.0 to 1.0).
        candidate_pct: f32,
        /// The candidate module ID.
        candidate_id: String,
    },
}

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self::Promoted
    }
}

/// Stored module containing manifest and demos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredModule {
    /// The compiled module manifest.
    pub manifest: CompiledModuleManifest,

    /// Demonstrations for the module (few-shot examples).
    pub demos: Vec<Example>,
}

/// Hub for storing and loading optimized DSPy modules.
pub struct DspyHub {
    /// Base path for storage (~/.openagents/dspy/).
    base_path: PathBuf,
}

impl DspyHub {
    /// Create a new hub with default path (~/.openagents/dspy/).
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let base_path = PathBuf::from(&home).join(".openagents/dspy");
        Self { base_path }
    }

    /// Create a hub with a custom base path.
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Get the path for optimized modules.
    pub fn optimized_path(&self) -> PathBuf {
        self.base_path.join("optimized")
    }

    /// Get the path for training data.
    pub fn training_path(&self) -> PathBuf {
        self.base_path.join("training")
    }

    /// Ensure directories exist.
    fn ensure_dirs(&self, signature_name: &str) -> Result<()> {
        let sig_path = self.optimized_path().join(signature_name);
        create_dir_all(&sig_path).context("Failed to create optimized directory")?;

        let training_path = self.training_path().join(signature_name);
        create_dir_all(&training_path).context("Failed to create training directory")?;

        Ok(())
    }

    /// Save a compiled module with its demos.
    ///
    /// Returns the compiled_id.
    pub fn save_module(
        &self,
        manifest: &CompiledModuleManifest,
        demos: &[Example],
    ) -> Result<String> {
        self.ensure_dirs(&manifest.signature_name)?;

        // Compute or use existing compiled_id
        let compiled_id = manifest.compiled_id.clone().unwrap_or_else(|| {
            manifest
                .compute_compiled_id()
                .unwrap_or_else(|_| "unknown".to_string())
        });

        let stored = StoredModule {
            manifest: manifest.clone(),
            demos: demos.to_vec(),
        };

        let path = self
            .optimized_path()
            .join(&manifest.signature_name)
            .join(format!("{}.json", compiled_id));

        let json = serde_json::to_string_pretty(&stored)?;
        fs::write(&path, json).context("Failed to write module")?;

        Ok(compiled_id)
    }

    /// Load a module by its compiled_id.
    pub fn load_module(&self, compiled_id: &str) -> Result<(CompiledModuleManifest, Vec<Example>)> {
        // Search all signature directories for the compiled_id
        let optimized = self.optimized_path();
        if !optimized.exists() {
            anyhow::bail!("No optimized modules directory found");
        }

        for entry in fs::read_dir(&optimized)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let module_path = entry.path().join(format!("{}.json", compiled_id));
                if module_path.exists() {
                    let contents = fs::read_to_string(&module_path)?;
                    let stored: StoredModule = serde_json::from_str(&contents)?;
                    return Ok((stored.manifest, stored.demos));
                }
            }
        }

        anyhow::bail!("Module not found: {}", compiled_id)
    }

    /// Load a module by signature name and compiled_id.
    pub fn load_module_by_signature(
        &self,
        signature_name: &str,
        compiled_id: &str,
    ) -> Result<(CompiledModuleManifest, Vec<Example>)> {
        let path = self
            .optimized_path()
            .join(signature_name)
            .join(format!("{}.json", compiled_id));

        if !path.exists() {
            anyhow::bail!("Module not found: {}/{}", signature_name, compiled_id);
        }

        let contents = fs::read_to_string(&path)?;
        let stored: StoredModule = serde_json::from_str(&contents)?;
        Ok((stored.manifest, stored.demos))
    }

    /// List all modules for a signature (or all signatures if None).
    pub fn list_modules(
        &self,
        signature_name: Option<&str>,
    ) -> Result<Vec<CompiledModuleManifest>> {
        let mut manifests = Vec::new();
        let optimized = self.optimized_path();

        if !optimized.exists() {
            return Ok(manifests);
        }

        let dirs: Vec<_> = if let Some(sig) = signature_name {
            vec![optimized.join(sig)]
        } else {
            fs::read_dir(&optimized)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .collect()
        };

        for dir in dirs {
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(contents) = fs::read_to_string(&path) {
                        if let Ok(stored) = serde_json::from_str::<StoredModule>(&contents) {
                            manifests.push(stored.manifest);
                        }
                    }
                }
            }
        }

        // Sort by creation time (newest first)
        manifests.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(manifests)
    }

    /// Get the promoted module for a signature.
    pub fn get_promoted(&self, signature_name: &str) -> Result<Option<CompiledModuleManifest>> {
        let manifests = self.list_modules(Some(signature_name))?;

        // Find the first promoted module
        Ok(manifests
            .into_iter()
            .find(|m| m.promotion_state == PromotionState::Promoted))
    }

    /// Get a module for routing based on strategy.
    pub fn get_module_for_routing(
        &self,
        signature_name: &str,
        routing: &RoutingStrategy,
    ) -> Result<CompiledModuleManifest> {
        match routing {
            RoutingStrategy::Promoted => self
                .get_promoted(signature_name)?
                .ok_or_else(|| anyhow::anyhow!("No promoted module for {}", signature_name)),

            RoutingStrategy::Shadow { candidate_id } => {
                // In shadow mode, return the candidate for evaluation
                let (manifest, _) = self.load_module_by_signature(signature_name, candidate_id)?;
                Ok(manifest)
            }

            RoutingStrategy::ABTest {
                candidate_pct,
                candidate_id,
            } => {
                // Simple random routing based on percentage
                let random: f32 = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| (d.subsec_nanos() as f32) / 1_000_000_000.0)
                    .unwrap_or(0.5);

                if random < *candidate_pct {
                    let (manifest, _) =
                        self.load_module_by_signature(signature_name, candidate_id)?;
                    Ok(manifest)
                } else {
                    self.get_promoted(signature_name)?
                        .ok_or_else(|| anyhow::anyhow!("No promoted module for {}", signature_name))
                }
            }
        }
    }

    /// Delete a module by its compiled_id.
    pub fn delete_module(&self, signature_name: &str, compiled_id: &str) -> Result<()> {
        let path = self
            .optimized_path()
            .join(signature_name)
            .join(format!("{}.json", compiled_id));

        if path.exists() {
            fs::remove_file(&path)?;
        }

        Ok(())
    }

    /// Get all candidate modules (not yet promoted) for a signature.
    pub fn get_candidates(&self, signature_name: &str) -> Result<Vec<CompiledModuleManifest>> {
        let manifests = self.list_modules(Some(signature_name))?;

        Ok(manifests
            .into_iter()
            .filter(|m| m.promotion_state == PromotionState::Candidate)
            .collect())
    }

    /// Get modules in shadow mode for a signature.
    pub fn get_shadow_modules(&self, signature_name: &str) -> Result<Vec<CompiledModuleManifest>> {
        let manifests = self.list_modules(Some(signature_name))?;

        Ok(manifests
            .into_iter()
            .filter(|m| m.promotion_state == PromotionState::Shadow)
            .collect())
    }

    /// Update the promotion state of a module.
    pub fn update_promotion_state(
        &self,
        signature_name: &str,
        compiled_id: &str,
        new_state: PromotionState,
    ) -> Result<()> {
        let (mut manifest, demos) = self.load_module_by_signature(signature_name, compiled_id)?;
        manifest.promotion_state = new_state;
        self.save_module(&manifest, &demos)?;
        Ok(())
    }
}

impl Default for DspyHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::manifest::Scorecard;
    use tempfile::TempDir;

    fn test_hub() -> (DspyHub, TempDir) {
        let temp = TempDir::new().unwrap();
        let hub = DspyHub::with_base_path(temp.path().to_path_buf());
        (hub, temp)
    }

    #[test]
    fn test_hub_creation() {
        let (hub, _temp) = test_hub();
        assert!(hub.optimized_path().ends_with("optimized"));
        assert!(hub.training_path().ends_with("training"));
    }

    #[test]
    fn test_save_and_load_module() {
        let (hub, _temp) = test_hub();

        let manifest = CompiledModuleManifest::new("TestSignature", "GEPA")
            .with_instruction("Test instruction")
            .with_scorecard(Scorecard::new(0.85))
            .finalize()
            .unwrap();

        let demos = vec![];
        let compiled_id = hub.save_module(&manifest, &demos).unwrap();

        let (loaded_manifest, loaded_demos) = hub.load_module(&compiled_id).unwrap();
        assert_eq!(loaded_manifest.signature_name, "TestSignature");
        assert_eq!(loaded_manifest.optimizer, "GEPA");
        assert!(loaded_demos.is_empty());
    }

    #[test]
    fn test_list_modules() {
        let (hub, _temp) = test_hub();

        // Save two modules
        let manifest1 = CompiledModuleManifest::new("Sig1", "GEPA")
            .with_instruction("Instruction 1")
            .finalize()
            .unwrap();

        let manifest2 = CompiledModuleManifest::new("Sig1", "MIPROv2")
            .with_instruction("Instruction 2")
            .finalize()
            .unwrap();

        hub.save_module(&manifest1, &[]).unwrap();
        hub.save_module(&manifest2, &[]).unwrap();

        let all = hub.list_modules(None).unwrap();
        assert_eq!(all.len(), 2);

        let sig1 = hub.list_modules(Some("Sig1")).unwrap();
        assert_eq!(sig1.len(), 2);

        let sig2 = hub.list_modules(Some("Sig2")).unwrap();
        assert!(sig2.is_empty());
    }

    #[test]
    fn test_get_promoted() {
        let (hub, _temp) = test_hub();

        // Save a candidate
        let candidate = CompiledModuleManifest::new("TestSig", "GEPA")
            .with_instruction("Candidate")
            .finalize()
            .unwrap();

        hub.save_module(&candidate, &[]).unwrap();

        // No promoted yet
        assert!(hub.get_promoted("TestSig").unwrap().is_none());

        // Save a promoted module
        let promoted = CompiledModuleManifest::new("TestSig", "MIPROv2")
            .with_instruction("Promoted")
            .with_promotion_state(PromotionState::Promoted)
            .finalize()
            .unwrap();

        hub.save_module(&promoted, &[]).unwrap();

        let found = hub.get_promoted("TestSig").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().instruction.as_deref(), Some("Promoted"));
    }

    #[test]
    fn test_routing_strategy() {
        let (hub, _temp) = test_hub();

        let promoted = CompiledModuleManifest::new("RouterTest", "GEPA")
            .with_instruction("Promoted")
            .with_promotion_state(PromotionState::Promoted)
            .finalize()
            .unwrap();

        let promoted_id = hub.save_module(&promoted, &[]).unwrap();

        // Test promoted routing
        let result = hub
            .get_module_for_routing("RouterTest", &RoutingStrategy::Promoted)
            .unwrap();
        assert_eq!(result.compiled_id, Some(promoted_id.clone()));

        // Save a candidate
        let candidate = CompiledModuleManifest::new("RouterTest", "MIPROv2")
            .with_instruction("Candidate")
            .finalize()
            .unwrap();

        let candidate_id = hub.save_module(&candidate, &[]).unwrap();

        // Test shadow routing
        let shadow_strategy = RoutingStrategy::Shadow {
            candidate_id: candidate_id.clone(),
        };
        let result = hub
            .get_module_for_routing("RouterTest", &shadow_strategy)
            .unwrap();
        assert_eq!(result.compiled_id, Some(candidate_id));
    }

    #[test]
    fn test_delete_module() {
        let (hub, _temp) = test_hub();

        let manifest = CompiledModuleManifest::new("DeleteTest", "GEPA")
            .with_instruction("To delete")
            .finalize()
            .unwrap();

        let compiled_id = hub.save_module(&manifest, &[]).unwrap();
        assert!(hub.load_module(&compiled_id).is_ok());

        hub.delete_module("DeleteTest", &compiled_id).unwrap();
        assert!(hub.load_module(&compiled_id).is_err());
    }

    #[test]
    fn test_update_promotion_state() {
        let (hub, _temp) = test_hub();

        let manifest = CompiledModuleManifest::new("PromoteTest", "GEPA")
            .with_instruction("Test")
            .finalize()
            .unwrap();

        let compiled_id = hub.save_module(&manifest, &[]).unwrap();

        // Initially candidate
        let (loaded, _) = hub.load_module(&compiled_id).unwrap();
        assert_eq!(loaded.promotion_state, PromotionState::Candidate);

        // Update to promoted
        hub.update_promotion_state("PromoteTest", &compiled_id, PromotionState::Promoted)
            .unwrap();

        let (loaded, _) = hub.load_module(&compiled_id).unwrap();
        assert_eq!(loaded.promotion_state, PromotionState::Promoted);
    }
}
