use rand::Rng;
use serde::{Deserialize, Serialize};
/// Pareto frontier management for GEPA optimizer
///
/// Implements per-example dominance tracking and coverage-weighted sampling
/// as described in the GEPA paper.
use std::collections::{HashMap, HashSet};

use crate::optimizer::gepa::GEPACandidate;

/// Pareto frontier maintaining candidates that excel on different examples
///
/// A candidate is on the Pareto frontier if it achieves the highest score
/// on at least one evaluation example. This ensures diversity and prevents
/// premature convergence to local optima.
#[derive(Debug, Clone)]
pub struct ParetoFrontier {
    /// All candidates currently on the frontier
    candidates: Vec<GEPACandidate>,

    /// Maps example index to the candidate IDs that achieve max score on it
    /// example_id -> [candidate_ids]
    example_to_best: HashMap<usize, Vec<usize>>,

    /// Maps candidate ID to the examples it wins on
    /// candidate_id -> [example_ids]
    candidate_to_examples: HashMap<usize, HashSet<usize>>,

    /// Next candidate ID to assign
    next_id: usize,
}

impl ParetoFrontier {
    /// Create a new empty Pareto frontier
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            example_to_best: HashMap::new(),
            candidate_to_examples: HashMap::new(),
            next_id: 0,
        }
    }

    /// Get the number of candidates on the frontier
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Check if frontier is empty
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Get all candidates on the frontier
    pub fn candidates(&self) -> &[GEPACandidate] {
        &self.candidates
    }

    /// Add or update a candidate based on its scores
    ///
    /// # Arguments
    /// * `candidate` - The candidate to add
    /// * `scores` - Score for each example in the evaluation set
    ///
    /// # Returns
    /// `true` if the candidate made it onto the frontier
    pub fn add_candidate(&mut self, mut candidate: GEPACandidate, scores: &[f32]) -> bool {
        // Assign ID to new candidate
        candidate.id = self.next_id;
        self.next_id += 1;

        // Find examples where this candidate achieves max score
        let mut wins_on = HashSet::new();

        for (example_idx, &score) in scores.iter().enumerate() {
            let current_best = self.example_to_best.get(&example_idx).and_then(|best_ids| {
                best_ids
                    .iter()
                    .filter_map(|&id| self.candidates.iter().find(|c| c.id == id))
                    .filter_map(|c| c.example_scores.get(example_idx))
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied()
            });

            match current_best {
                Some(best_score) if score > best_score => {
                    // New best for this example
                    wins_on.insert(example_idx);
                }
                Some(best_score) if (score - best_score).abs() < 1e-6 => {
                    // Tied for best
                    wins_on.insert(example_idx);
                }
                None => {
                    // First candidate for this example
                    wins_on.insert(example_idx);
                }
                _ => {}
            }
        }

        // Only add if candidate wins on at least one example
        if wins_on.is_empty() {
            return false;
        }

        // Store scores with candidate
        candidate.example_scores = scores.to_vec();

        // Update mappings
        for &example_idx in &wins_on {
            // Find current max score for this example
            let max_score = scores[example_idx];

            // Remove candidates that are now dominated on this example
            if let Some(best_ids) = self.example_to_best.get_mut(&example_idx) {
                // Keep only candidates with equal or better scores
                best_ids.retain(|&id| {
                    if let Some(existing) = self.candidates.iter().find(|c| c.id == id) {
                        if let Some(&existing_score) = existing.example_scores.get(example_idx) {
                            (existing_score - max_score).abs() < 1e-6 || existing_score > max_score
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });

                if (max_score - scores[example_idx]).abs() < 1e-6 {
                    best_ids.push(candidate.id);
                }
            } else {
                self.example_to_best.insert(example_idx, vec![candidate.id]);
            }
        }

        self.candidate_to_examples.insert(candidate.id, wins_on);

        // Remove dominated candidates from frontier
        self.prune_dominated();

        // Add new candidate
        self.candidates.push(candidate);

        true
    }

    /// Remove candidates that don't win on any example
    fn prune_dominated(&mut self) {
        let mut still_winning: HashSet<usize> = HashSet::new();

        for candidate_ids in self.example_to_best.values() {
            still_winning.extend(candidate_ids.iter());
        }

        self.candidates.retain(|c| still_winning.contains(&c.id));
        self.candidate_to_examples
            .retain(|id, _| still_winning.contains(id));
    }

    /// Sample a candidate from the frontier with probability proportional to coverage
    ///
    /// Candidates that win on more examples have higher probability of being selected.
    /// This balances exploration (sampling diverse candidates) with exploitation
    /// (sampling successful candidates).
    pub fn sample_proportional_to_coverage(&self) -> Option<&GEPACandidate> {
        if self.candidates.is_empty() {
            return None;
        }

        // Calculate coverage for each candidate
        let coverages: Vec<usize> = self
            .candidates
            .iter()
            .map(|c| {
                self.candidate_to_examples
                    .get(&c.id)
                    .map(|examples| examples.len())
                    .unwrap_or(0)
            })
            .collect();

        let total_coverage: usize = coverages.iter().sum();

        if total_coverage == 0 {
            // Fallback to uniform sampling
            return self.candidates.first();
        }

        // Sample proportional to coverage
        let mut rng = rand::thread_rng();
        let mut target = rng.gen_range(0..total_coverage);

        for (candidate, &coverage) in self.candidates.iter().zip(coverages.iter()) {
            if target < coverage {
                return Some(candidate);
            }
            target -= coverage;
        }

        // Fallback (shouldn't happen)
        self.candidates.last()
    }

    /// Get the best candidate by average score
    pub fn best_by_average(&self) -> Option<&GEPACandidate> {
        self.candidates.iter().max_by(|a, b| {
            let avg_a = a.average_score();
            let avg_b = b.average_score();
            avg_a.partial_cmp(&avg_b).unwrap()
        })
    }

    /// Get statistics about the frontier
    pub fn statistics(&self) -> ParetoStatistics {
        let num_candidates = self.candidates.len();
        let num_examples_covered = self.example_to_best.len();

        let coverage_per_candidate: Vec<usize> = self
            .candidates
            .iter()
            .map(|c| {
                self.candidate_to_examples
                    .get(&c.id)
                    .map(|examples| examples.len())
                    .unwrap_or(0)
            })
            .collect();

        let avg_coverage = if !coverage_per_candidate.is_empty() {
            coverage_per_candidate.iter().sum::<usize>() as f32
                / coverage_per_candidate.len() as f32
        } else {
            0.0
        };

        let max_coverage = coverage_per_candidate.iter().copied().max().unwrap_or(0);
        let min_coverage = coverage_per_candidate.iter().copied().min().unwrap_or(0);

        ParetoStatistics {
            num_candidates,
            num_examples_covered,
            avg_coverage,
            max_coverage,
            min_coverage,
        }
    }
}

impl Default for ParetoFrontier {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the Pareto frontier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoStatistics {
    /// Number of candidates on the frontier
    pub num_candidates: usize,

    /// Number of examples covered by at least one candidate
    pub num_examples_covered: usize,

    /// Average number of examples won by each candidate
    pub avg_coverage: f32,

    /// Maximum coverage (most examples won by any candidate)
    pub max_coverage: usize,

    /// Minimum coverage (fewest examples won by any candidate)
    pub min_coverage: usize,
}
