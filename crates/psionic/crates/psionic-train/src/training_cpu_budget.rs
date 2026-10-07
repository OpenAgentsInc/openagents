//! Bounded CPU-budget contract for psionic training entrypoints.
//!
//! Training runs default to at most one core (~100% CPU) and one parallel
//! worker. Wider allocations require an explicit owner opt-in through
//! `PSIONIC_TRAIN_CPU_BUDGET`; agents launching training must treat the
//! bounded default as binding (psionic#1123).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Environment variable carrying the explicit owner CPU-budget opt-in.
pub const PSIONIC_TRAIN_CPU_BUDGET_ENV: &str = "PSIONIC_TRAIN_CPU_BUDGET";

/// Bounded default core count when no explicit opt-in is present.
pub const PSIONIC_TRAIN_CPU_BUDGET_DEFAULT_CORES: u32 = 1;

/// Thread-pool environment variables capped to the effective core budget.
///
/// These cover the rayon global pool plus the common BLAS/OpenMP pools, so
/// the cap holds for library-spawned worker pools that size themselves from
/// the environment at first use.
pub const PSIONIC_TRAIN_CPU_BUDGET_THREAD_CAP_ENV_VARS: [&str; 5] = [
    "RAYON_NUM_THREADS",
    "OMP_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "MKL_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS",
];

/// Typed CPU budget for one training process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PsionicTrainCpuBudget {
    /// Bounded default: at most one core (~100% CPU) and one parallel worker.
    BoundedSingleCore,
    /// Explicit owner allocation. Valid only with `opted_in: true`.
    ExplicitCores {
        /// Admitted core count.
        cores: u32,
        /// Explicit owner opt-in marker. A wider allocation without this
        /// marker is refused; agents must never default it to `true`.
        opted_in: bool,
    },
}

impl Default for PsionicTrainCpuBudget {
    fn default() -> Self {
        PsionicTrainCpuBudget::BoundedSingleCore
    }
}

/// How the effective CPU budget was selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainCpuBudgetSource {
    /// No opt-in was present; the bounded default applies.
    BoundedDefault,
    /// The owner set `PSIONIC_TRAIN_CPU_BUDGET` explicitly.
    OwnerEnvOptIn,
}

/// Resolved CPU budget plus the provenance of the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCpuBudgetResolution {
    /// Validated budget.
    pub budget: PsionicTrainCpuBudget,
    /// Where the budget came from.
    pub source: PsionicTrainCpuBudgetSource,
}

impl PsionicTrainCpuBudget {
    /// Validates the budget. An explicit allocation without the opt-in
    /// marker is refused regardless of size.
    pub fn validate(&self) -> Result<(), PsionicTrainCpuBudgetError> {
        match self {
            PsionicTrainCpuBudget::BoundedSingleCore => Ok(()),
            PsionicTrainCpuBudget::ExplicitCores { cores, opted_in } => {
                if *cores == 0 {
                    return Err(PsionicTrainCpuBudgetError::ZeroCores);
                }
                if !opted_in {
                    return Err(PsionicTrainCpuBudgetError::MissingOptIn { cores: *cores });
                }
                Ok(())
            }
        }
    }

    /// Effective core cap for thread-pool sizing.
    #[must_use]
    pub fn effective_cores(&self) -> u32 {
        match self {
            PsionicTrainCpuBudget::BoundedSingleCore => PSIONIC_TRAIN_CPU_BUDGET_DEFAULT_CORES,
            PsionicTrainCpuBudget::ExplicitCores { cores, .. } => *cores,
        }
    }

    /// Maximum parallel training workers. Defaults to one; multi-worker
    /// sweeps require the explicit opt-in.
    #[must_use]
    pub fn max_parallel_workers(&self) -> u32 {
        self.effective_cores()
    }

    /// Thread-cap environment assignments enforcing the effective budget.
    #[must_use]
    pub fn thread_cap_env_assignments(&self) -> Vec<(String, String)> {
        let cores = self.effective_cores().to_string();
        PSIONIC_TRAIN_CPU_BUDGET_THREAD_CAP_ENV_VARS
            .iter()
            .map(|name| (String::from(*name), cores.clone()))
            .collect()
    }
}

/// Resolves the CPU budget from the raw `PSIONIC_TRAIN_CPU_BUDGET` value.
///
/// An unset or empty value selects the bounded single-core default. A set
/// value is the explicit owner opt-in and accepts either a core count
/// (`"4"`) or a CPU percentage (`"400%"`, rounded up to whole cores).
/// Unparseable or zero values are refused instead of falling back.
pub fn resolve_psionic_train_cpu_budget(
    env_value: Option<&str>,
) -> Result<PsionicTrainCpuBudgetResolution, PsionicTrainCpuBudgetError> {
    let trimmed = env_value.map(str::trim).unwrap_or_default();
    if trimmed.is_empty() {
        return Ok(PsionicTrainCpuBudgetResolution {
            budget: PsionicTrainCpuBudget::BoundedSingleCore,
            source: PsionicTrainCpuBudgetSource::BoundedDefault,
        });
    }
    let cores = if let Some(percent_text) = trimmed.strip_suffix('%') {
        let percent: u32 = percent_text.trim().parse().map_err(|_| {
            PsionicTrainCpuBudgetError::UnparseableValue {
                value: String::from(trimmed),
            }
        })?;
        percent.div_ceil(100)
    } else {
        trimmed
            .parse()
            .map_err(|_| PsionicTrainCpuBudgetError::UnparseableValue {
                value: String::from(trimmed),
            })?
    };
    let budget = PsionicTrainCpuBudget::ExplicitCores {
        cores,
        opted_in: true,
    };
    budget.validate()?;
    Ok(PsionicTrainCpuBudgetResolution {
        budget,
        source: PsionicTrainCpuBudgetSource::OwnerEnvOptIn,
    })
}

/// Launch banner line stating the effective budget and how it was set, so
/// transcripts show whether a run was authorized to go wide.
#[must_use]
pub fn psionic_train_cpu_budget_banner(resolution: &PsionicTrainCpuBudgetResolution) -> String {
    let cores = resolution.budget.effective_cores();
    let workers = resolution.budget.max_parallel_workers();
    match resolution.source {
        PsionicTrainCpuBudgetSource::BoundedDefault => format!(
            "psionic-train cpu budget: {cores} core(s), {workers} worker(s) max [bounded default; set {PSIONIC_TRAIN_CPU_BUDGET_ENV}=<cores|percent%> for an explicit owner opt-in]"
        ),
        PsionicTrainCpuBudgetSource::OwnerEnvOptIn => format!(
            "psionic-train cpu budget: {cores} core(s), {workers} worker(s) max [explicit owner opt-in via {PSIONIC_TRAIN_CPU_BUDGET_ENV}]"
        ),
    }
}

/// CPU-budget refusal reasons.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionicTrainCpuBudgetError {
    #[error("psionic-train cpu budget must admit at least one core")]
    ZeroCores,
    #[error(
        "psionic-train cpu budget of {cores} core(s) requires the explicit owner opt-in marker; the bounded single-core default stays binding without it"
    )]
    MissingOptIn { cores: u32 },
    #[error(
        "psionic-train cpu budget value `{value}` is unparseable; expected a core count such as `4` or a percentage such as `400%`"
    )]
    UnparseableValue { value: String },
}

#[cfg(test)]
mod tests {
    use super::{
        PSIONIC_TRAIN_CPU_BUDGET_THREAD_CAP_ENV_VARS, PsionicTrainCpuBudget,
        PsionicTrainCpuBudgetError, PsionicTrainCpuBudgetSource, psionic_train_cpu_budget_banner,
        resolve_psionic_train_cpu_budget,
    };

    #[test]
    fn default_budget_is_bounded_single_core_single_worker() {
        let resolution = resolve_psionic_train_cpu_budget(None)
            .expect("unset env should resolve to the bounded default");
        assert_eq!(resolution.budget, PsionicTrainCpuBudget::BoundedSingleCore);
        assert_eq!(
            resolution.source,
            PsionicTrainCpuBudgetSource::BoundedDefault
        );
        assert_eq!(resolution.budget.effective_cores(), 1);
        assert_eq!(resolution.budget.max_parallel_workers(), 1);
    }

    #[test]
    fn empty_env_value_resolves_to_bounded_default() {
        let resolution = resolve_psionic_train_cpu_budget(Some("  "))
            .expect("blank env should resolve to the bounded default");
        assert_eq!(resolution.budget, PsionicTrainCpuBudget::BoundedSingleCore);
        assert_eq!(
            resolution.source,
            PsionicTrainCpuBudgetSource::BoundedDefault
        );
    }

    #[test]
    fn explicit_core_count_resolves_as_owner_opt_in() {
        let resolution = resolve_psionic_train_cpu_budget(Some("6"))
            .expect("explicit core count should resolve");
        assert_eq!(
            resolution.budget,
            PsionicTrainCpuBudget::ExplicitCores {
                cores: 6,
                opted_in: true,
            }
        );
        assert_eq!(
            resolution.source,
            PsionicTrainCpuBudgetSource::OwnerEnvOptIn
        );
        assert_eq!(resolution.budget.effective_cores(), 6);
        assert_eq!(resolution.budget.max_parallel_workers(), 6);
    }

    #[test]
    fn percentage_value_rounds_up_to_whole_cores() {
        let resolution = resolve_psionic_train_cpu_budget(Some("450%"))
            .expect("percentage value should resolve");
        assert_eq!(resolution.budget.effective_cores(), 5);
        assert_eq!(
            resolution.source,
            PsionicTrainCpuBudgetSource::OwnerEnvOptIn
        );
    }

    #[test]
    fn zero_budget_is_refused() {
        assert_eq!(
            resolve_psionic_train_cpu_budget(Some("0")),
            Err(PsionicTrainCpuBudgetError::ZeroCores)
        );
        assert_eq!(
            resolve_psionic_train_cpu_budget(Some("0%")),
            Err(PsionicTrainCpuBudgetError::ZeroCores)
        );
    }

    #[test]
    fn unparseable_budget_is_refused_instead_of_falling_back() {
        assert_eq!(
            resolve_psionic_train_cpu_budget(Some("unbounded")),
            Err(PsionicTrainCpuBudgetError::UnparseableValue {
                value: String::from("unbounded"),
            })
        );
    }

    #[test]
    fn explicit_cores_without_opt_in_marker_are_refused() {
        let budget = PsionicTrainCpuBudget::ExplicitCores {
            cores: 8,
            opted_in: false,
        };
        assert_eq!(
            budget.validate(),
            Err(PsionicTrainCpuBudgetError::MissingOptIn { cores: 8 })
        );
    }

    #[test]
    fn thread_cap_assignments_pin_every_known_pool_to_the_budget() {
        let budget = PsionicTrainCpuBudget::BoundedSingleCore;
        let assignments = budget.thread_cap_env_assignments();
        assert_eq!(
            assignments.len(),
            PSIONIC_TRAIN_CPU_BUDGET_THREAD_CAP_ENV_VARS.len()
        );
        for (name, value) in &assignments {
            assert!(PSIONIC_TRAIN_CPU_BUDGET_THREAD_CAP_ENV_VARS.contains(&name.as_str()));
            assert_eq!(value, "1");
        }
    }

    #[test]
    fn banner_states_the_budget_and_its_source() {
        let bounded = resolve_psionic_train_cpu_budget(None)
            .expect("unset env should resolve to the bounded default");
        let bounded_banner = psionic_train_cpu_budget_banner(&bounded);
        assert!(bounded_banner.contains("1 core(s)"));
        assert!(bounded_banner.contains("bounded default"));

        let opted_in = resolve_psionic_train_cpu_budget(Some("4"))
            .expect("explicit core count should resolve");
        let opted_in_banner = psionic_train_cpu_budget_banner(&opted_in);
        assert!(opted_in_banner.contains("4 core(s)"));
        assert!(opted_in_banner.contains("explicit owner opt-in"));
    }
}
