//! HillClimber Parallel Sampler Module
//!
//! Implements test-time compute scaling through parallel candidate generation.
//! Generates multiple candidate solutions and selects the best one.
//!
//! Part of the MAP (Modular Agentic Planner) architecture.

use crate::error::{HillClimberError, Result};
use crate::evaluator::parse_pytest_output;
use crate::types::{CandidateResult, EvaluatorResult, SamplingResult, VerificationConfig};
use futures::future::join_all;
use std::path::{Path, PathBuf};
use tokio::fs;

// ============================================================================
// Configuration
// ============================================================================

/// Default number of parallel candidates to generate.
pub const DEFAULT_CANDIDATE_COUNT: usize = 3;

/// Default temperatures for parallel sampling.
pub const DEFAULT_TEMPERATURES: [f32; 3] = [0.3, 0.5, 0.7];

/// Default variation hints for each candidate.
const DEFAULT_HINTS: [&str; 3] = [
    "Focus on precision - avoid false positives",
    "Balance precision and recall",
    "Focus on recall - avoid false negatives",
];

// ============================================================================
// Parallel Sampler
// ============================================================================

/// Parallel sampler for generating and evaluating multiple candidates.
pub struct ParallelSampler {
    /// Base workspace directory
    pub base_workspace: PathBuf,
    /// Verification configuration
    pub verification: VerificationConfig,
    /// Number of candidates to generate
    pub candidate_count: usize,
    /// Temperatures for each candidate
    pub temperatures: Vec<f32>,
}

impl ParallelSampler {
    /// Create a new parallel sampler with default settings.
    pub fn new(base_workspace: PathBuf, verification: VerificationConfig) -> Self {
        Self {
            base_workspace,
            verification,
            candidate_count: DEFAULT_CANDIDATE_COUNT,
            temperatures: DEFAULT_TEMPERATURES.to_vec(),
        }
    }

    /// Create a new parallel sampler with custom settings.
    pub fn with_config(
        base_workspace: PathBuf,
        verification: VerificationConfig,
        candidate_count: usize,
        temperatures: Vec<f32>,
    ) -> Self {
        Self {
            base_workspace,
            verification,
            candidate_count,
            temperatures,
        }
    }

    /// Create temporary workspaces for each candidate.
    pub async fn create_workspaces(&self) -> Result<Vec<PathBuf>> {
        let mut workspaces = Vec::new();

        for i in 0..self.candidate_count {
            let workspace_path = self
                .base_workspace
                .parent()
                .unwrap_or(&self.base_workspace)
                .join(format!("candidate_{}", i));

            // Create the workspace directory
            fs::create_dir_all(&workspace_path)
                .await
                .map_err(|e| HillClimberError::Workspace(e.to_string()))?;

            // Copy files from base workspace
            copy_dir_recursive(&self.base_workspace, &workspace_path).await?;

            workspaces.push(workspace_path);
        }

        Ok(workspaces)
    }

    /// Cleanup temporary workspaces.
    pub async fn cleanup_workspaces(&self, workspaces: &[PathBuf]) -> Result<()> {
        for workspace in workspaces {
            if workspace.exists() {
                fs::remove_dir_all(workspace)
                    .await
                    .map_err(|e| HillClimberError::Workspace(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Evaluate all candidates in parallel.
    pub async fn evaluate_candidates(&self, workspaces: &[PathBuf]) -> Result<Vec<EvaluatorResult>> {
        let futures: Vec<_> = workspaces
            .iter()
            .map(|workspace| self.evaluate_single(workspace))
            .collect();

        let results = join_all(futures).await;

        // Convert Results to a single Result containing Vec
        results.into_iter().collect()
    }

    /// Evaluate a single candidate workspace.
    async fn evaluate_single(&self, workspace: &Path) -> Result<EvaluatorResult> {
        let cmd = self
            .verification
            .command
            .clone()
            .unwrap_or_else(|| "pytest -v".to_string());

        let start = std::time::Instant::now();

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(workspace)
            .output()
            .await
            .map_err(|e| HillClimberError::Io(e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        let parse_result = parse_pytest_output(&combined);
        let progress = if parse_result.total > 0 {
            parse_result.passed as f64 / parse_result.total as f64
        } else {
            0.0
        };

        Ok(EvaluatorResult {
            passed: output.status.success() && parse_result.failed == 0,
            progress,
            tests_total: parse_result.total,
            tests_passing: parse_result.passed,
            failures: parse_result.failures,
            suggestion: None,
            raw_output: combined,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Select the best candidate from evaluation results.
    pub fn select_best(
        &self,
        workspaces: &[PathBuf],
        results: &[EvaluatorResult],
    ) -> Option<SamplingResult> {
        if results.is_empty() {
            return None;
        }

        let mut candidates: Vec<CandidateResult> = Vec::new();
        let mut best_idx = 0;
        let mut best_progress = 0.0f64;

        for (i, result) in results.iter().enumerate() {
            let temperature = self.temperatures.get(i).copied().unwrap_or(0.5);
            let hint = DEFAULT_HINTS.get(i).unwrap_or(&"").to_string();

            let candidate = CandidateResult {
                index: i,
                temperature,
                variation_hint: hint,
                workspace: workspaces.get(i).cloned().unwrap_or_default(),
                passed: result.passed,
                progress: result.progress,
                tests_passing: result.tests_passing,
                tests_total: result.tests_total,
                solution: None,
            };

            if result.progress > best_progress {
                best_progress = result.progress;
                best_idx = i;
            }

            candidates.push(candidate);
        }

        let average_progress = if !candidates.is_empty() {
            candidates.iter().map(|c| c.progress).sum::<f64>() / candidates.len() as f64
        } else {
            0.0
        };

        Some(SamplingResult {
            best: candidates[best_idx].clone(),
            all: candidates,
            average_progress,
            improvement: best_progress - average_progress,
        })
    }

    /// Apply the best solution to the main workspace.
    pub async fn apply_best(
        &self,
        best_workspace: &Path,
        target_workspace: &Path,
        solution_file: &str,
    ) -> Result<()> {
        let src = best_workspace.join(solution_file);
        let dst = target_workspace.join(solution_file);

        if src.exists() {
            // Ensure parent directory exists
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| HillClimberError::Workspace(e.to_string()))?;
            }

            fs::copy(&src, &dst)
                .await
                .map_err(|e| HillClimberError::Workspace(e.to_string()))?;
        }

        Ok(())
    }

    /// Get variation hints for each candidate.
    pub fn get_variation_hints(&self) -> Vec<String> {
        (0..self.candidate_count)
            .map(|i| {
                DEFAULT_HINTS
                    .get(i)
                    .unwrap_or(&"Try a different approach")
                    .to_string()
            })
            .collect()
    }

    /// Get temperature for a candidate index.
    pub fn get_temperature(&self, index: usize) -> f32 {
        self.temperatures.get(index).copied().unwrap_or(0.5)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Recursively copy a directory.
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .await
        .map_err(|e| HillClimberError::Workspace(e.to_string()))?;

    let mut entries = fs::read_dir(src)
        .await
        .map_err(|e| HillClimberError::Workspace(e.to_string()))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| HillClimberError::Workspace(e.to_string()))?
    {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else {
            fs::copy(&src_path, &dst_path)
                .await
                .map_err(|e| HillClimberError::Workspace(e.to_string()))?;
        }
    }

    Ok(())
}

/// Quick sampling run: create candidates, evaluate, select best.
pub async fn quick_sample(
    base_workspace: &Path,
    verification: &VerificationConfig,
    solution_file: &str,
) -> Result<Option<SamplingResult>> {
    let sampler = ParallelSampler::new(base_workspace.to_path_buf(), verification.clone());

    // Create workspaces
    let workspaces = sampler.create_workspaces().await?;

    // Evaluate all candidates
    let results = sampler.evaluate_candidates(&workspaces).await?;

    // Select best
    let sampling_result = sampler.select_best(&workspaces, &results);

    // If we found a best candidate, apply it
    if let Some(ref result) = sampling_result {
        sampler
            .apply_best(&result.best.workspace, base_workspace, solution_file)
            .await?;
    }

    // Cleanup
    sampler.cleanup_workspaces(&workspaces).await?;

    Ok(sampling_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_cleanup_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir_all(&base).await.unwrap();

        // Create a test file
        fs::write(base.join("test.txt"), "hello").await.unwrap();

        let sampler = ParallelSampler::new(base.clone(), VerificationConfig::default());

        // Create workspaces
        let workspaces = sampler.create_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 3);

        // Verify each workspace has the test file
        for workspace in &workspaces {
            assert!(workspace.exists());
            assert!(workspace.join("test.txt").exists());
        }

        // Cleanup
        sampler.cleanup_workspaces(&workspaces).await.unwrap();

        // Verify cleanup
        for workspace in &workspaces {
            assert!(!workspace.exists());
        }
    }

    #[test]
    fn test_select_best() {
        let sampler = ParallelSampler::new(PathBuf::from("/tmp"), VerificationConfig::default());

        let workspaces = vec![
            PathBuf::from("/tmp/candidate_0"),
            PathBuf::from("/tmp/candidate_1"),
            PathBuf::from("/tmp/candidate_2"),
        ];

        let results = vec![
            EvaluatorResult {
                passed: false,
                progress: 0.5,
                tests_total: 10,
                tests_passing: 5,
                ..Default::default()
            },
            EvaluatorResult {
                passed: false,
                progress: 0.8,
                tests_total: 10,
                tests_passing: 8,
                ..Default::default()
            },
            EvaluatorResult {
                passed: false,
                progress: 0.6,
                tests_total: 10,
                tests_passing: 6,
                ..Default::default()
            },
        ];

        let sampling_result = sampler.select_best(&workspaces, &results).unwrap();

        assert_eq!(sampling_result.best.index, 1);
        assert_eq!(sampling_result.best.progress, 0.8);
        assert_eq!(sampling_result.all.len(), 3);
    }

    #[test]
    fn test_variation_hints() {
        let sampler = ParallelSampler::new(PathBuf::from("/tmp"), VerificationConfig::default());
        let hints = sampler.get_variation_hints();

        assert_eq!(hints.len(), 3);
        assert!(hints[0].contains("precision"));
        assert!(hints[1].contains("Balance"));
        assert!(hints[2].contains("recall"));
    }

    #[test]
    fn test_temperatures() {
        let sampler = ParallelSampler::new(PathBuf::from("/tmp"), VerificationConfig::default());

        assert_eq!(sampler.get_temperature(0), 0.3);
        assert_eq!(sampler.get_temperature(1), 0.5);
        assert_eq!(sampler.get_temperature(2), 0.7);
        assert_eq!(sampler.get_temperature(10), 0.5); // Default fallback
    }
}
