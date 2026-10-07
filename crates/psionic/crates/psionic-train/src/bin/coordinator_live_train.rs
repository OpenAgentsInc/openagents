//! Live coordinator-evolution training driver (Khala M6, issue #6014 / EPIC #6017).
//!
//! This is the bounded **live training run** wiring on top of the merged P1-P5
//! substrate. It does two things, in order:
//!
//! 1. **No-spend simulated validation pass** (`--validate`, the default). Proves
//!    the full loop end-to-end on CPU with a REAL frozen cs336 backbone
//!    (`forward_with_hidden`) feeding a real `CoordinatorHead`, sep-CMA-ES
//!    optimizing it, the P5 capability-filtered worker pool, and the
//!    `DailySpendCap` wired in. The verdict source is the deterministic,
//!    zero-spend `SimulatedVerdictSource`, so this moves ZERO sats while
//!    exercising the cap-debit path. It is labeled a SMOKE, not an ML result.
//!
//! 2. **Bounded real run** (`--real`). Held by default: a real run needs a live
//!    Pylon verdict source (dispatch each trajectory as a buy-mode eval job)
//!    and the Tassadar `training.verification_classes.v1` verdict. Those move
//!    sats and MUST debit the shared autonomous daily cap in the `openagents`
//!    Worker (see `coordinator_live_training.rs` module docs). Without a
//!    reachable Pylon endpoint + spend-enabled buy-mode campaign this binary
//!    refuses to fabricate an ML result and prints exactly what is needed,
//!    while still proving the cap would fail closed.
//!
//! Run:
//!   cargo run -q -p psionic-train --bin coordinator_live_train            # validate (no spend)
//!   cargo run -q -p psionic-train --bin coordinator_live_train -- --real  # env-armed paid shadow run

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_core::Shape;
use psionic_core::TensorData;
use psionic_eval::CompiledAgentEvidenceClass;
use psionic_models::{
    CoordinatorHead, CoordinatorHeadConfig, Cs336A1ReferenceConfig, Cs336A1TransformerLm,
};
use psionic_nn::ModuleStateLoadMode;
use psionic_train::{
    CapDebitOwner, CompiledAgentArtifactValidatorLineage, CoordinatorArmState,
    CoordinatorCandidateEmission, CoordinatorLiveTrainingError, DailySpendCap,
    DispatchBackedVerdictSource, EvalSample, EvalVerdictSource, HttpBuyModeDispatch,
    LiveCoordinatorFitness, LiveRunLane, LiveRunReport, SepCmaEs, SepCmaEsConfig, ShadowComparison,
    SimulatedVerdictSource, TerminalRewardAdapter, TrajectoryOutcome, TrajectoryRequest,
    VerificationVerdict, WorkerKind, WorkerPoolBinding, WorkerPoolMember, OWNER_DAILY_CAP_MSATS,
};
use serde::Serialize;
use sha2::Digest;

const LIVE_WORKER_IDS_ENV: &str = "PSIONIC_M6_WORKER_IDS";
const LIVE_OUTPUT_ENV: &str = "PSIONIC_M6_REAL_OUTPUT";
const LIVE_PER_EVAL_MSATS_ENV: &str = "PSIONIC_M6_PER_EVAL_MSATS";
const LIVE_HEURISTIC_PER_EVAL_MSATS_ENV: &str = "PSIONIC_M6_HEURISTIC_PER_EVAL_MSATS";
const LIVE_DAILY_CAP_MSATS_ENV: &str = "PSIONIC_M6_DAILY_CAP_MSATS";
const LIVE_RUN_REF_ENV: &str = "PSIONIC_M6_RUN_REF";
const LIVE_HEURISTIC_ROLLBACK_ID_ENV: &str = "PSIONIC_M6_HEURISTIC_ROLLBACK_ID";

/// Builds a deterministic frozen cs336 backbone with `d_model == hidden_dim`.
/// On the real lane this is replaced by a frozen Qwen3-0.6B; the loop is
/// identical because the head only sees `forward_with_hidden`'s `[1, d_model]`
/// hidden state.
fn frozen_backbone(d_model: usize, vocab: usize) -> Cs336A1TransformerLm {
    let config = Cs336A1ReferenceConfig {
        vocab_size: vocab,
        context_length: 16,
        d_model,
        num_layers: 2,
        num_heads: 2,
        d_ff: d_model * 2,
    };
    let mut model =
        Cs336A1TransformerLm::new("frozen_backbone", config, 10_000.0, 1e-5).expect("backbone");
    let mut weights = model.state_dict();
    // Deterministic but well-spread frozen weights. The token embedding must be
    // strongly token-dependent so different prompts produce well-separated
    // last-token hidden states (otherwise no linear head can route them apart).
    for (index, (path, entry)) in weights.entries.iter_mut().enumerate() {
        let len = entry.spec.shape().element_count();
        let values: Vec<f32> = if path.contains("ln") || path.ends_with("norm.weight") {
            vec![1.0; len]
        } else if path == "token_embeddings.weight" {
            // Distinct, spread embeddings per (token, dim): a 2D-ish pattern so
            // different token ids land in clearly different directions.
            (0..len)
                .map(|i| {
                    let row = (i / d_model) as f32;
                    let col = (i % d_model) as f32;
                    (0.7 * (row * 1.3 + col * 0.9).sin()) + 0.3 * ((row - col) * 0.5).cos()
                })
                .collect()
        } else {
            let base = ((index % 7) as f32) * 0.03 + 0.02;
            (0..len)
                .map(|i| base + ((i % 5) as f32 * 0.05 - 0.1) + 0.04 * ((i as f32) * 0.7).sin())
                .collect()
        };
        entry.data = TensorData::F32(values);
    }
    model
        .load_state_dict(&weights, ModuleStateLoadMode::Strict)
        .expect("frozen weights");
    model
}

fn validation_pool() -> Result<WorkerPoolBinding, CoordinatorLiveTrainingError> {
    let candidates = vec![
        WorkerPoolMember {
            worker_id: "frontier-claude".to_string(),
            kind: WorkerKind::Frontier,
            receipted_capabilities: ["rust_build".to_string(), "python".to_string()]
                .into_iter()
                .collect(),
        },
        WorkerPoolMember {
            worker_id: "open-pylon-a".to_string(),
            kind: WorkerKind::Open,
            receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
        },
        WorkerPoolMember {
            worker_id: "open-pylon-b".to_string(),
            kind: WorkerKind::Open,
            receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
        },
        // Filtered out: lacks rust_build.
        WorkerPoolMember {
            worker_id: "open-python-only".to_string(),
            kind: WorkerKind::Open,
            receipted_capabilities: ["python".to_string()].into_iter().collect(),
        },
    ];
    WorkerPoolBinding::from_candidates(candidates, "rust_build")
        .map_err(CoordinatorLiveTrainingError::from)
}

fn validation_samples() -> Vec<EvalSample> {
    vec![
        EvalSample {
            sample_id: "task-0".to_string(),
            token_ids: vec![1, 5, 9, 2],
        },
        EvalSample {
            sample_id: "task-1".to_string(),
            token_ids: vec![3, 7, 1, 8],
        },
        EvalSample {
            sample_id: "task-2".to_string(),
            token_ids: vec![2, 2, 6, 4],
        },
    ]
}

fn hidden_for_tokens(
    backbone: &Cs336A1TransformerLm,
    tokens: &[usize],
) -> Result<Vec<f32>, CoordinatorLiveTrainingError> {
    let (_, h) = backbone
        .forward_with_hidden(Shape::new(vec![1, tokens.len()]), tokens)
        .map_err(|e| CoordinatorLiveTrainingError::VerdictSource {
            detail: e.to_string(),
        })?;
    h.as_f32_slice()
        .map(<[f32]>::to_vec)
        .map_err(|e| CoordinatorLiveTrainingError::VerdictSource {
            detail: e.to_string(),
        })
}

fn run_validation() -> Result<(LiveRunReport, CoordinatorHead), CoordinatorLiveTrainingError> {
    let d_model = 8;
    let backbone = frozen_backbone(d_model, 32);

    // P5: capability-filtered eligible pool (3 workers for `rust_build`).
    let pool = validation_pool()?;
    println!(
        "P5 worker pool: {} eligible for `{}` -> {:?}",
        pool.len(),
        pool.required_capability(),
        pool.workers()
            .iter()
            .map(|w| w.worker_id.as_str())
            .collect::<Vec<_>>()
    );

    let head_config = CoordinatorHeadConfig {
        hidden_dim: d_model,
        num_workers: pool.len(),
        num_roles: 3,
    };
    let seed_head = CoordinatorHead::zeros(head_config)?;
    println!(
        "P2 head: hidden_dim={} num_workers={} num_roles={} -> {} params",
        head_config.hidden_dim,
        head_config.num_workers,
        head_config.num_roles,
        head_config.parameter_count()
    );

    // Three eval samples, each must route to a specific eligible worker to
    // Verify. The "correct" worker varies per sample so the head must learn a
    // real h -> worker mapping (not a constant).
    let samples = validation_samples();
    let source = SimulatedVerdictSource::new(vec![
        ("task-0".to_string(), 0),
        ("task-1".to_string(), 2),
        ("task-2".to_string(), 1),
    ]);

    let hidden = move |tokens: &[usize]| -> Result<Vec<f32>, CoordinatorLiveTrainingError> {
        let (_, h) = backbone
            .forward_with_hidden(Shape::new(vec![1, tokens.len()]), tokens)
            .map_err(|e| CoordinatorLiveTrainingError::VerdictSource {
                detail: e.to_string(),
            })?;
        h.as_f32_slice().map(<[f32]>::to_vec).map_err(|e| {
            CoordinatorLiveTrainingError::VerdictSource {
                detail: e.to_string(),
            }
        })
    };

    // Owner cap wired in (no spend on this lane, but the path is exercised).
    let cap = DailySpendCap::owner_default("2026-06-22");
    let fitness = LiveCoordinatorFitness::new(
        seed_head.clone(),
        pool,
        TerminalRewardAdapter::offline(),
        samples,
        hidden,
        source,
        cap,
    )?;

    let dimension = head_config.parameter_count();
    let optimizer = SepCmaEs::new(SepCmaEsConfig {
        dimension,
        population_size: 40,
        generations: 200,
        initial_sigma: 0.8,
        seed: 0x6014_6017,
    })?;
    let initial = seed_head.flatten_parameters()?;
    let outcome = optimizer.optimize(&fitness, &initial)?;
    let trained_head = seed_head.with_flat_parameters(outcome.best_parameters.clone())?;

    let cap_after = fitness.cap_snapshot();
    Ok((
        LiveRunReport {
            lane: LiveRunLane::SimulatedNoSpend,
            initial_fitness: outcome.initial_fitness,
            best_fitness: outcome.best_fitness,
            improved: outcome.improved(),
            evaluations: outcome.evaluations,
            spent_msats: cap_after.spent_today_msats(),
            cap_msats: cap_after.cap_msats(),
            halted_on_cap: fitness.halted_on_cap(),
            day_key: cap_after.day_key().to_string(),
        },
        trained_head,
    ))
}

#[derive(Debug, Serialize)]
struct RealShadowReceipt {
    schema: &'static str,
    issue_ref: &'static str,
    generated_at_day_key_utc: String,
    run: LiveRunReport,
    shadow: ShadowComparison,
    candidate: CoordinatorCandidateEmission,
    learned_outcomes: Vec<TrajectoryOutcome>,
    heuristic_outcomes: Vec<TrajectoryOutcome>,
    live_refs: RealShadowRefs,
}

#[derive(Debug, Serialize)]
struct RealShadowRefs {
    worker_ids: Vec<String>,
    dispatch_endpoint_ref: &'static str,
    verdict_class_ref: &'static str,
    spend_authority_ref: &'static str,
}

fn parse_u64_env(name: &str, fallback: u64) -> Result<u64, CoordinatorLiveTrainingError> {
    match std::env::var(name) {
        Ok(value) => {
            value
                .trim()
                .parse::<u64>()
                .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
                    detail: format!("{name} must be a positive integer"),
                })
        }
        Err(std::env::VarError::NotPresent) => Ok(fallback),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(CoordinatorLiveTrainingError::VerdictSource {
                detail: format!("{name} must be valid UTF-8"),
            })
        }
    }
}

fn env_output_path() -> Option<PathBuf> {
    std::env::var(LIVE_OUTPUT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn utc_day_key_now() -> Result<String, CoordinatorLiveTrainingError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
            detail: String::from("system clock is before unix epoch"),
        })?
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

// Howard Hinnant's civil-from-days conversion, with days counted from
// 1970-01-01. This avoids adding a date dependency to a tiny launch binary.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    ((y + if m <= 2 { 1 } else { 0 }) as i32, m as u32, d as u32)
}

fn live_pool_from_env() -> Result<WorkerPoolBinding, CoordinatorLiveTrainingError> {
    let value = std::env::var(LIVE_WORKER_IDS_ENV).map_err(|_| {
        CoordinatorLiveTrainingError::VerdictSource {
            detail: format!("{LIVE_WORKER_IDS_ENV} is required for --real (comma-separated live Pylon worker ids)"),
        }
    })?;
    let candidates: Vec<WorkerPoolMember> = value
        .split(',')
        .map(str::trim)
        .filter(|worker_id| !worker_id.is_empty())
        .map(|worker_id| WorkerPoolMember {
            worker_id: worker_id.to_string(),
            kind: WorkerKind::Open,
            receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
        })
        .collect();
    WorkerPoolBinding::from_candidates(candidates, "rust_build")
        .map_err(CoordinatorLiveTrainingError::from)
}

fn live_run_ref_from_env() -> Result<Option<String>, CoordinatorLiveTrainingError> {
    let value = match std::env::var(LIVE_RUN_REF_ENV) {
        Ok(value) => value.trim().to_string(),
        Err(_) => return Ok(None),
    };
    if value.is_empty() {
        return Ok(None);
    }
    let valid = value.len() <= 80
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
    if !valid {
        return Err(CoordinatorLiveTrainingError::VerdictSource {
            detail: format!(
                "{LIVE_RUN_REF_ENV} must be <=80 chars of ASCII letters, digits, dot, underscore, or hyphen"
            ),
        });
    }
    Ok(Some(value))
}

fn collect_outcomes<S: EvalVerdictSource>(
    lane_label: &str,
    head: &CoordinatorHead,
    pool: &WorkerPoolBinding,
    samples: &[EvalSample],
    backbone: &Cs336A1TransformerLm,
    verdicts: &S,
    cap: &mut DailySpendCap,
    live_run_ref: Option<&str>,
) -> Result<Vec<TrajectoryOutcome>, CoordinatorLiveTrainingError> {
    use psionic_nn::NnTensor;

    let hidden_dim = head.config().hidden_dim;
    let mut outcomes = Vec::with_capacity(samples.len());

    for sample in samples {
        let hidden_values = hidden_for_tokens(backbone, &sample.token_ids)?;
        let hidden =
            NnTensor::f32(Shape::new(vec![1, hidden_dim]), hidden_values).map_err(|error| {
                CoordinatorLiveTrainingError::VerdictSource {
                    detail: error.to_string(),
                }
            })?;
        let decisions = head.decide(&hidden)?;
        let decision = &decisions[0];
        let worker = pool.resolve(decision.worker_index).ok_or_else(|| {
            CoordinatorLiveTrainingError::VerdictSource {
                detail: format!(
                    "head selected worker index {} outside live pool size {}",
                    decision.worker_index,
                    pool.len()
                ),
            }
        })?;
        let outcome = verdicts.verdict_for(&TrajectoryRequest {
            worker_index: decision.worker_index,
            worker_id: worker.worker_id.clone(),
            role_index: decision.role_index,
            sample_id: live_run_ref.map_or_else(
                || sample.sample_id.clone(),
                |run_ref| format!("{lane_label}.{}.{}", sample.sample_id, run_ref),
            ),
        })?;
        cap.try_debit(outcome.spend_msats)?;
        outcomes.push(TrajectoryOutcome {
            verdict: outcome.verdict,
            cost: outcome.spend_msats as f32 / 1_000.0,
        });
    }

    Ok(outcomes)
}

fn heuristic_head(
    config: CoordinatorHeadConfig,
) -> Result<CoordinatorHead, CoordinatorLiveTrainingError> {
    let mut params = vec![0.0; config.parameter_count()];
    if config.num_workers > 0 {
        // Bias the baseline toward the first eligible worker. The learned head
        // is the validation-trained sep-CMA-ES candidate; this baseline models a
        // fixed route policy with no sample-dependent hidden-state routing.
        params[0] = 1.0;
    }
    CoordinatorHead::from_flat_weights(config, params).map_err(CoordinatorLiveTrainingError::from)
}

fn all_verified(outcomes: &[TrajectoryOutcome]) -> bool {
    outcomes
        .iter()
        .all(|outcome| outcome.verdict == VerificationVerdict::Verified)
}

fn run_real(
    trained_head: CoordinatorHead,
) -> Result<RealShadowReceipt, CoordinatorLiveTrainingError> {
    let dispatch_config = psionic_train::http_buy_mode_dispatch_config_from_env()
        .map_err(|error| CoordinatorLiveTrainingError::VerdictSource {
            detail: error.to_string(),
        })?
        .ok_or_else(|| CoordinatorLiveTrainingError::VerdictSource {
            detail: String::from(
                "HTTP buy-mode dispatch is not armed; set PSIONIC_BUY_MODE_HTTP_ARM=armed plus endpoint/token",
            ),
        })?;
    let learned_dispatch = HttpBuyModeDispatch::new(dispatch_config.clone()).map_err(|error| {
        CoordinatorLiveTrainingError::VerdictSource {
            detail: error.to_string(),
        }
    })?;
    let heuristic_dispatch = HttpBuyModeDispatch::new(dispatch_config).map_err(|error| {
        CoordinatorLiveTrainingError::VerdictSource {
            detail: error.to_string(),
        }
    })?;

    let pool = live_pool_from_env()?;
    if pool.len() != trained_head.config().num_workers {
        return Err(CoordinatorLiveTrainingError::VerdictSource {
            detail: format!(
                "{LIVE_WORKER_IDS_ENV} must contain exactly {} eligible workers; got {}",
                trained_head.config().num_workers,
                pool.len(),
            ),
        });
    }

    let per_eval_msats = parse_u64_env(LIVE_PER_EVAL_MSATS_ENV, 1_000)?;
    let heuristic_per_eval_msats = parse_u64_env(
        LIVE_HEURISTIC_PER_EVAL_MSATS_ENV,
        per_eval_msats.saturating_mul(2),
    )?;
    let daily_cap_msats = parse_u64_env(LIVE_DAILY_CAP_MSATS_ENV, OWNER_DAILY_CAP_MSATS)?;
    let live_run_ref = live_run_ref_from_env()?;
    let day_key = utc_day_key_now()?;
    let cap = DailySpendCap::for_day(day_key.clone(), daily_cap_msats);
    let samples = validation_samples();
    let backbone = frozen_backbone(trained_head.config().hidden_dim, 32);

    let learned_source = DispatchBackedVerdictSource::new(
        learned_dispatch,
        CoordinatorArmState::Armed,
        per_eval_msats,
        CapDebitOwner::Fitness,
        cap.clone(),
    );
    let heuristic_source = DispatchBackedVerdictSource::new(
        heuristic_dispatch,
        CoordinatorArmState::Armed,
        heuristic_per_eval_msats,
        CapDebitOwner::Fitness,
        cap.clone(),
    );

    let mut shared_cap = cap;
    let learned_outcomes = collect_outcomes(
        "learned",
        &trained_head,
        &pool,
        &samples,
        &backbone,
        &learned_source,
        &mut shared_cap,
        live_run_ref.as_deref(),
    )?;
    let heuristic = heuristic_head(trained_head.config())?;
    let heuristic_outcomes = collect_outcomes(
        "heuristic",
        &heuristic,
        &pool,
        &samples,
        &backbone,
        &heuristic_source,
        &mut shared_cap,
        live_run_ref.as_deref(),
    )?;

    let shadow = ShadowComparison::compare(&learned_outcomes, &heuristic_outcomes);
    if !all_verified(&learned_outcomes) {
        return Err(CoordinatorLiveTrainingError::VerdictSource {
            detail: String::from("learned coordinator did not verify every live shadow trajectory"),
        });
    }
    if !shadow.recommends_promotion() {
        return Err(CoordinatorLiveTrainingError::VerdictSource {
            detail: format!(
                "paid shadow did not produce a promotion-eligible win: {}",
                shadow.summary
            ),
        });
    }

    let report = LiveRunReport {
        lane: LiveRunLane::BoundedReal,
        initial_fitness: 0.0,
        best_fitness: shadow.learned.verified_rate(),
        improved: shadow.learned_wins,
        evaluations: learned_outcomes.len() + heuristic_outcomes.len(),
        spent_msats: shared_cap.spent_today_msats(),
        cap_msats: shared_cap.cap_msats(),
        halted_on_cap: false,
        day_key: shared_cap.day_key().to_string(),
    };

    let validator_report_digest = {
        let json = serde_json::to_vec(&shadow).map_err(|error| {
            CoordinatorLiveTrainingError::VerdictSource {
                detail: error.to_string(),
            }
        })?;
        let digest = sha2::Sha256::digest(json);
        hex::encode(digest)
    };
    let validator_report_ref = format!(
        "psionic.khala_m6.paid_shadow.{}.{}",
        report.day_key.replace('-', ""),
        &validator_report_digest[..16],
    );
    let candidate = CoordinatorCandidateEmission::emit(
        &trained_head,
        std::env::var(LIVE_HEURISTIC_ROLLBACK_ID_ENV)
            .unwrap_or_else(|_| "compiled_agent.baseline.rule_v1.coordinator_route".to_string()),
        shadow.clone(),
        CompiledAgentArtifactValidatorLineage {
            validator_report_ref,
            validator_report_digest,
            xtrain_cycle_receipt_ref: None,
            xtrain_cycle_receipt_digest: None,
        },
        CompiledAgentEvidenceClass::LearnedLane,
    )
    .map_err(|error| CoordinatorLiveTrainingError::VerdictSource {
        detail: error.to_string(),
    })?;

    Ok(RealShadowReceipt {
        schema: "psionic.khala_m6.paid_shadow_run.v1",
        issue_ref: "OpenAgentsInc/openagents#6014",
        generated_at_day_key_utc: day_key,
        run: report,
        shadow,
        candidate,
        learned_outcomes,
        heuristic_outcomes,
        live_refs: RealShadowRefs {
            worker_ids: pool
                .workers()
                .iter()
                .map(|worker| worker.worker_id.clone())
                .collect(),
            dispatch_endpoint_ref: "openagents.worker.operator_buy_mode_eval",
            verdict_class_ref: "training.verification_classes.v1.exact_trace_replay",
            spend_authority_ref: "openagents.buy_mode_campaign.daily_cap_msats",
        },
    })
}

fn main() -> ExitCode {
    let real = std::env::args().any(|a| a == "--real");

    println!("== Coordinator LIVE training driver (Khala M6, #6014 / EPIC #6017) ==");
    println!(
        "Owner HARD budget cap: {} sats/day ({} msats/day). Fails closed.\n",
        OWNER_DAILY_CAP_MSATS / 1_000,
        OWNER_DAILY_CAP_MSATS
    );

    // Always run the no-spend validation pass first.
    println!("-- Phase 1: no-spend simulated validation (proves loop + cap enforcement) --");
    let (report, trained_head) = match run_validation() {
        Ok(report) => report,
        Err(error) => {
            eprintln!("validation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("\nLiveRunReport (simulated, no-spend):");
    println!("  lane            : {:?}", report.lane);
    println!("  initial fitness : {:.4}", report.initial_fitness);
    println!("  best fitness    : {:.4}", report.best_fitness);
    println!("  improved        : {}", report.improved);
    println!("  evaluations     : {}", report.evaluations);
    println!(
        "  spent (msats)   : {}  (must be 0 on this lane)",
        report.spent_msats
    );
    println!("  cap   (msats)   : {}", report.cap_msats);
    println!("  halted on cap   : {}", report.halted_on_cap);
    println!("  within cap      : {}", report.within_cap());

    // Hard gates on the validation pass.
    if report.spent_msats != 0 {
        eprintln!("\nFAIL: simulated lane must spend zero sats.");
        return ExitCode::FAILURE;
    }
    if !report.within_cap() {
        eprintln!("\nFAIL: cap invariant violated.");
        return ExitCode::FAILURE;
    }
    if !report.improved || (report.best_fitness - 1.0).abs() > 1e-6 {
        eprintln!(
            "\nFAIL: ES did not drive the real-backbone head to a verified routing (got {}).",
            report.best_fitness
        );
        return ExitCode::FAILURE;
    }
    println!("\nVALIDATION PASS: real frozen backbone -> head -> sep-CMA-ES reached verified");
    println!("routing (fitness 1.0) over the capability-filtered pool, zero sats spent, cap");
    println!("enforcement path exercised. This is a SMOKE, not a frontier ML result.");

    if !real {
        println!("\n(Pass --real to attempt the env-armed bounded paid shadow run.)");
        return ExitCode::SUCCESS;
    }

    // -- Phase 2: bounded real run -----------------------------------------
    println!("\n-- Phase 2: bounded REAL run (--real) --");
    let receipt = match run_real(trained_head) {
        Ok(receipt) => receipt,
        Err(error) => {
            eprintln!("real run failed closed: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("Paid shadow summary: {}", receipt.shadow.summary);
    println!(
        "Paid shadow spent {} msats of {} msats cap across {} evals.",
        receipt.run.spent_msats, receipt.run.cap_msats, receipt.run.evaluations
    );
    if let Some(path) = env_output_path() {
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                eprintln!("failed to create receipt directory: {error}");
                return ExitCode::FAILURE;
            }
        }
        let json = match serde_json::to_string_pretty(&receipt) {
            Ok(json) => json,
            Err(error) => {
                eprintln!("failed to encode paid shadow receipt: {error}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(error) = fs::write(&path, format!("{json}\n")) {
            eprintln!("failed to write paid shadow receipt: {error}");
            return ExitCode::FAILURE;
        }
        println!("Paid shadow receipt: {}", path.display());
    }
    ExitCode::SUCCESS
}
