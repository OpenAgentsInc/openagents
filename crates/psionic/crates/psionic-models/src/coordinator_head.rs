//! Learned-coordinator head (Khala M6 / TRINITY substrate, P2).
//!
//! A tiny linear head that reads a frozen-backbone penultimate-token hidden
//! state `h` (see `Cs336A1TransformerLm::forward_with_hidden`, P1) and emits
//! routing logits over `(worker, role)`:
//!
//! ```text
//!   h: [batch, hidden_dim]
//!     -> W: [hidden_dim, num_workers + num_roles]
//!     -> split into worker logits [batch, num_workers]
//!                and role   logits [batch, num_roles]
//!     -> softmax each block independently
//! ```
//!
//! For `hidden_dim ≈ 1024`, `num_workers ≈ 7`, `num_roles = 3` the head is
//! `1024 * 10 ≈ 10K` parameters, matching TRINITY's "tiny head" regime. The
//! backbone stays frozen; only this head is learned.
//!
//! ## What this module is
//!
//! This is the live, differentiable head plus its forward path. It is
//! deliberately backbone-agnostic: it takes a hidden-state `NnTensor` of shape
//! `[batch, hidden_dim]` and is agnostic to which backbone produced it.
//!
//! ## What is intentionally stubbed (documented TODOs, not faked)
//!
//! The full coordinator training loop is NOT implemented here. The remaining
//! primitives from `docs/sakana/psionic-coordinator-roadmap.md` are stubs with
//! explicit interfaces so the optimizer and reward work can land against a real
//! shape without faking an ML result:
//!
//! - **P3 — separable CMA-ES optimizer.** A gradient-free trainer that samples
//!   a population of perturbed `CoordinatorHead` parameter vectors, evaluates
//!   each via the P4 fitness hook, and updates a diagonal covariance. See
//!   [`CoordinatorHead::flatten_parameters`] /
//!   [`CoordinatorHead::with_flat_parameters`] for the exact parameter vector
//!   the optimizer perturbs. NOT implemented — needs GPU/eval budget.
//! - **P4 — scalar terminal-reward adapter + atomic evaluation.** A function
//!   `evaluate_coordinator(params) -> f32` that loads the head, runs one
//!   end-to-end select->role->dispatch->verify trajectory, and returns the
//!   verified-work reward (`verified ? 1.0 : 0.0`). NOT implemented.
//! - **P5 — worker-pool binding.** A typed, capability-filtered ordered list of
//!   the `L` eligible workers the head's worker logits index. NOT implemented.
//!
//! Those are not stubbed with fake numbers; they are absent, with the
//! interfaces this head exposes ([`flatten_parameters`], [`with_flat_parameters`],
//! [`CoordinatorDecision`]) being the contract a future P3/P4/P5 lane binds to.
//!
//! [`flatten_parameters`]: CoordinatorHead::flatten_parameters
//! [`with_flat_parameters`]: CoordinatorHead::with_flat_parameters

use psionic_core::Shape;
use psionic_nn::{softmax_last_dim, Linear, NnTensor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Static configuration for a coordinator head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorHeadConfig {
    /// Width of the backbone hidden state `h` the head reads.
    pub hidden_dim: usize,
    /// Number of interchangeable workers the worker logits index (`L`).
    pub num_workers: usize,
    /// Number of coordinator roles the role logits index (TRINITY uses 3:
    /// Worker / Thinker / Verifier).
    pub num_roles: usize,
}

impl CoordinatorHeadConfig {
    /// Total output width = worker logits followed by role logits.
    #[must_use]
    pub const fn output_width(&self) -> usize {
        self.num_workers + self.num_roles
    }

    /// Number of learnable parameters in the linear head (weight only; the
    /// head is bias-free by default to stay in the tiny-head regime).
    #[must_use]
    pub const fn parameter_count(&self) -> usize {
        self.hidden_dim * self.output_width()
    }

    fn validate(self) -> Result<Self, CoordinatorHeadError> {
        if self.hidden_dim == 0 {
            return Err(CoordinatorHeadError::InvalidConfiguration {
                detail: String::from("hidden_dim must be non-zero"),
            });
        }
        if self.num_workers == 0 {
            return Err(CoordinatorHeadError::InvalidConfiguration {
                detail: String::from("num_workers must be non-zero"),
            });
        }
        if self.num_roles == 0 {
            return Err(CoordinatorHeadError::InvalidConfiguration {
                detail: String::from("num_roles must be non-zero"),
            });
        }
        Ok(self)
    }
}

/// Errors raised by the coordinator head.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CoordinatorHeadError {
    /// Configuration values were invalid.
    #[error("invalid coordinator head configuration: {detail}")]
    InvalidConfiguration {
        /// Human-readable detail.
        detail: String,
    },
    /// A supplied flat parameter vector did not match the head's parameter
    /// count.
    #[error("expected {expected} parameters, received {received}")]
    ParameterCountMismatch {
        /// Expected parameter count.
        expected: usize,
        /// Received parameter count.
        received: usize,
    },
    /// An input hidden-state tensor had an unexpected shape.
    #[error("invalid hidden-state shape: {detail}")]
    InvalidHiddenShape {
        /// Human-readable detail.
        detail: String,
    },
    /// An underlying neural-network layer error.
    #[error("layer error: {0}")]
    Layer(String),
}

/// One coordinator decision for a single batch row: the selected worker index,
/// the selected role index, and the softmax probability distributions both
/// were drawn from (argmax). Halt/accept is intentionally NOT part of this
/// decision — per `docs/sakana/coordinator-as-verified-work.md`, accept is the
/// replay-validator verdict, not a learned head output.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordinatorDecision {
    /// Argmax worker index in `[0, num_workers)`.
    pub worker_index: usize,
    /// Argmax role index in `[0, num_roles)`.
    pub role_index: usize,
    /// Worker probability distribution (length `num_workers`).
    pub worker_probabilities: Vec<f32>,
    /// Role probability distribution (length `num_roles`).
    pub role_probabilities: Vec<f32>,
}

/// A tiny frozen-backbone coordinator head: `h -> (worker logits, role logits)`.
#[derive(Clone, Debug)]
pub struct CoordinatorHead {
    config: CoordinatorHeadConfig,
    linear: Linear,
}

impl CoordinatorHead {
    /// Builds a coordinator head from a flat weight vector laid out row-major
    /// as `[output_width, hidden_dim]` (matching `psionic_nn::Linear`'s weight
    /// layout). The head is bias-free.
    pub fn from_flat_weights(
        config: CoordinatorHeadConfig,
        weights: Vec<f32>,
    ) -> Result<Self, CoordinatorHeadError> {
        let config = config.validate()?;
        if weights.len() != config.parameter_count() {
            return Err(CoordinatorHeadError::ParameterCountMismatch {
                expected: config.parameter_count(),
                received: weights.len(),
            });
        }
        let linear = Linear::from_f32_parts(
            "coordinator_head",
            config.hidden_dim,
            config.output_width(),
            weights,
            None,
        )
        .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;
        Ok(Self { config, linear })
    }

    /// Builds a zero-initialized head (useful as a P3 optimizer seed).
    pub fn zeros(config: CoordinatorHeadConfig) -> Result<Self, CoordinatorHeadError> {
        let config = config.validate()?;
        Self::from_flat_weights(config, vec![0.0; config.parameter_count()])
    }

    /// The head's static configuration.
    #[must_use]
    pub const fn config(&self) -> CoordinatorHeadConfig {
        self.config
    }

    /// Flat parameter vector the P3 optimizer perturbs, row-major
    /// `[output_width, hidden_dim]`.
    pub fn flatten_parameters(&self) -> Result<Vec<f32>, CoordinatorHeadError> {
        self.linear
            .weight_f32()
            .map(<[f32]>::to_vec)
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))
    }

    /// Returns a new head with the supplied flat parameter vector, leaving the
    /// configuration unchanged. This is the seam sep-CMA-ES (P3) uses to
    /// instantiate each population member before evaluation (P4).
    pub fn with_flat_parameters(
        &self,
        weights: Vec<f32>,
    ) -> Result<Self, CoordinatorHeadError> {
        Self::from_flat_weights(self.config, weights)
    }

    /// Forward pass: hidden state `h: [batch, hidden_dim]` -> raw combined
    /// logits `[batch, num_workers + num_roles]` (worker block first).
    pub fn forward_logits(&self, hidden: &NnTensor) -> Result<NnTensor, CoordinatorHeadError> {
        let dims = hidden.dims();
        if dims.len() != 2 || dims[1] != self.config.hidden_dim {
            return Err(CoordinatorHeadError::InvalidHiddenShape {
                detail: format!(
                    "expected [batch, {}], found {:?}",
                    self.config.hidden_dim, dims
                ),
            });
        }
        self.linear
            .forward(hidden)
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))
    }

    /// Forward pass returning the separately-normalized worker and role
    /// probability distributions. Worker and role blocks are softmaxed
    /// independently (TRINITY treats them as two heads sharing one weight).
    pub fn forward_distributions(
        &self,
        hidden: &NnTensor,
    ) -> Result<(NnTensor, NnTensor), CoordinatorHeadError> {
        let combined = self.forward_logits(hidden)?;
        let values = combined
            .as_f32_slice()
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;
        let batch = combined.dims()[0];
        let width = self.config.output_width();
        let num_workers = self.config.num_workers;
        let num_roles = self.config.num_roles;

        let mut worker_logits = vec![0.0_f32; batch * num_workers];
        let mut role_logits = vec![0.0_f32; batch * num_roles];
        for row in 0..batch {
            let base = row * width;
            worker_logits[row * num_workers..(row + 1) * num_workers]
                .copy_from_slice(&values[base..base + num_workers]);
            role_logits[row * num_roles..(row + 1) * num_roles]
                .copy_from_slice(&values[base + num_workers..base + width]);
        }

        let worker_tensor = NnTensor::f32(Shape::new(vec![batch, num_workers]), worker_logits)
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;
        let role_tensor = NnTensor::f32(Shape::new(vec![batch, num_roles]), role_logits)
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;

        let worker_probabilities = softmax_last_dim(&worker_tensor)
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;
        let role_probabilities = softmax_last_dim(&role_tensor)
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;
        Ok((worker_probabilities, role_probabilities))
    }

    /// Argmax decision per batch row. The worker index maps onto the P5
    /// worker-pool ordering (stubbed); the role index maps onto the TRINITY
    /// role ordering.
    pub fn decide(
        &self,
        hidden: &NnTensor,
    ) -> Result<Vec<CoordinatorDecision>, CoordinatorHeadError> {
        let (worker_probabilities, role_probabilities) = self.forward_distributions(hidden)?;
        let batch = worker_probabilities.dims()[0];
        let num_workers = self.config.num_workers;
        let num_roles = self.config.num_roles;
        let worker_values = worker_probabilities
            .as_f32_slice()
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;
        let role_values = role_probabilities
            .as_f32_slice()
            .map_err(|error| CoordinatorHeadError::Layer(error.to_string()))?;

        let mut decisions = Vec::with_capacity(batch);
        for row in 0..batch {
            let worker_slice = &worker_values[row * num_workers..(row + 1) * num_workers];
            let role_slice = &role_values[row * num_roles..(row + 1) * num_roles];
            decisions.push(CoordinatorDecision {
                worker_index: argmax(worker_slice),
                role_index: argmax(role_slice),
                worker_probabilities: worker_slice.to_vec(),
                role_probabilities: role_slice.to_vec(),
            });
        }
        Ok(decisions)
    }
}

fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .fold((0_usize, f32::NEG_INFINITY), |(best_index, best), (index, &value)| {
            if value > best {
                (index, value)
            } else {
                (best_index, best)
            }
        })
        .0
}

#[cfg(test)]
mod tests {
    use super::{CoordinatorHead, CoordinatorHeadConfig, CoordinatorHeadError};
    use psionic_core::Shape;
    use psionic_nn::NnTensor;

    fn smoke_config() -> CoordinatorHeadConfig {
        CoordinatorHeadConfig {
            hidden_dim: 2,
            num_workers: 2,
            num_roles: 3,
        }
    }

    #[test]
    fn parameter_count_matches_weight_layout() {
        let config = smoke_config();
        // 2 hidden * (2 workers + 3 roles) = 10 params.
        assert_eq!(config.output_width(), 5);
        assert_eq!(config.parameter_count(), 10);
        let head = CoordinatorHead::zeros(config).expect("head");
        assert_eq!(head.flatten_parameters().expect("flat").len(), 10);
    }

    #[test]
    fn forward_logits_selects_worker_and_role_blocks() {
        let config = smoke_config();
        // Weight is [output_width=5, hidden_dim=2], row-major. Make output i
        // read hidden[0] for even rows and hidden[1] for odd rows so we can
        // predict the projection.
        // rows: w0,w1, r0,r1,r2  -> pick distinct columns.
        let weights = vec![
            1.0, 0.0, // worker 0 <- hidden[0]
            0.0, 1.0, // worker 1 <- hidden[1]
            1.0, 0.0, // role 0   <- hidden[0]
            0.0, 1.0, // role 1   <- hidden[1]
            0.0, 0.0, // role 2   <- 0
        ];
        let head = CoordinatorHead::from_flat_weights(config, weights).expect("head");
        let hidden = NnTensor::f32(Shape::new(vec![1, 2]), vec![3.0, 5.0]).expect("hidden");
        let logits = head.forward_logits(&hidden).expect("logits");
        assert_eq!(logits.dims(), &[1, 5]);
        assert_eq!(
            logits.as_f32_slice().expect("slice"),
            &[3.0, 5.0, 3.0, 5.0, 0.0]
        );
    }

    #[test]
    fn distributions_normalize_blocks_independently() {
        let config = smoke_config();
        let head = CoordinatorHead::zeros(config).expect("head");
        let hidden = NnTensor::f32(Shape::new(vec![1, 2]), vec![1.0, 1.0]).expect("hidden");
        let (workers, roles) = head.forward_distributions(&hidden).expect("dists");
        assert_eq!(workers.dims(), &[1, 2]);
        assert_eq!(roles.dims(), &[1, 3]);
        // Zero head => uniform within each block, each block sums to 1.
        let worker_sum: f32 = workers.as_f32_slice().expect("w").iter().sum();
        let role_sum: f32 = roles.as_f32_slice().expect("r").iter().sum();
        assert!((worker_sum - 1.0).abs() < 1e-6);
        assert!((role_sum - 1.0).abs() < 1e-6);
        for value in workers.as_f32_slice().expect("w") {
            assert!((value - 0.5).abs() < 1e-6);
        }
        for value in roles.as_f32_slice().expect("r") {
            assert!((value - 1.0 / 3.0).abs() < 1e-6);
        }
    }

    #[test]
    fn decide_returns_argmax_per_row() {
        let config = smoke_config();
        let weights = vec![
            1.0, 0.0, // worker 0 <- hidden[0]
            0.0, 1.0, // worker 1 <- hidden[1]
            0.0, 1.0, // role 0   <- hidden[1]
            1.0, 0.0, // role 1   <- hidden[0]
            0.0, 0.0, // role 2
        ];
        let head = CoordinatorHead::from_flat_weights(config, weights).expect("head");
        // Row 0: hidden[0] dominates -> worker 0, role 1.
        // Row 1: hidden[1] dominates -> worker 1, role 0.
        let hidden = NnTensor::f32(Shape::new(vec![2, 2]), vec![10.0, 0.0, 0.0, 10.0])
            .expect("hidden");
        let decisions = head.decide(&hidden).expect("decisions");
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].worker_index, 0);
        assert_eq!(decisions[0].role_index, 1);
        assert_eq!(decisions[1].worker_index, 1);
        assert_eq!(decisions[1].role_index, 0);
        assert_eq!(decisions[0].worker_probabilities.len(), 2);
        assert_eq!(decisions[0].role_probabilities.len(), 3);
    }

    #[test]
    fn forward_is_deterministic() {
        let config = smoke_config();
        let head = CoordinatorHead::from_flat_weights(
            config,
            vec![0.1, -0.2, 0.3, 0.4, -0.5, 0.6, 0.7, -0.8, 0.9, 1.0],
        )
        .expect("head");
        let hidden = NnTensor::f32(Shape::new(vec![1, 2]), vec![0.5, -0.5]).expect("hidden");
        let first = head.forward_logits(&hidden).expect("first");
        let second = head.forward_logits(&hidden).expect("second");
        assert_eq!(
            first.as_f32_slice().expect("a"),
            second.as_f32_slice().expect("b")
        );
    }

    #[test]
    fn with_flat_parameters_round_trips_for_optimizer_seam() {
        let config = smoke_config();
        let head = CoordinatorHead::zeros(config).expect("head");
        let new_weights: Vec<f32> = (0..config.parameter_count())
            .map(|index| index as f32 * 0.01)
            .collect();
        let updated = head
            .with_flat_parameters(new_weights.clone())
            .expect("updated");
        assert_eq!(updated.flatten_parameters().expect("flat"), new_weights);
    }

    #[test]
    fn rejects_wrong_parameter_count() {
        let config = smoke_config();
        let error = CoordinatorHead::from_flat_weights(config, vec![0.0; 3]).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorHeadError::ParameterCountMismatch {
                expected: 10,
                received: 3
            }
        ));
    }

    #[test]
    fn rejects_wrong_hidden_shape() {
        let config = smoke_config();
        let head = CoordinatorHead::zeros(config).expect("head");
        let hidden = NnTensor::f32(Shape::new(vec![1, 3]), vec![0.0, 0.0, 0.0]).expect("hidden");
        let error = head.forward_logits(&hidden).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorHeadError::InvalidHiddenShape { .. }
        ));
    }
}
