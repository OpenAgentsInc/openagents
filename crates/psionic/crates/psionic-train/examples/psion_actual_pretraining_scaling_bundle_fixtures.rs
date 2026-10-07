use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_RECIPE_ID,
    PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_ID,
    PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_SCHEMA_VERSION, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingScalingBundle, PsionActualPretrainingScalingCandidate,
    PsionActualPretrainingScalingSelectionRule,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const TOKENS_PER_PARAMETER: u64 = 8;
const TOKENS_PER_STEP: u64 = 65_536;
const MAXIMUM_STAGE_LENGTH_MS: u64 = 4_200_000;
const MAXIMUM_TOTAL_COST_MICROUSD: u64 = 600_000_000;
const MAXIMUM_VALIDATION_LOSS_MILLI: u64 = 1_220;
const MINIMUM_REASONING_FLOOR_BPS: u32 = 8_000;
const VALIDATION_LOSS_DELTA_PER_DOUBLING: i64 = 22;
const AVERAGE_PASS_DELTA_PER_DOUBLING: i64 = 80;
const FLOOR_PASS_DELTA_PER_DOUBLING: i64 = 40;

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let lane_spec_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_lane_spec_v1.json");
    let recipe_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json");
    let data_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json");
    let systems_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json");
    let anchor_run_bundle_path =
        root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json");
    let anchor_stage_receipt_path =
        root.join("fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json");
    let anchor_observability_receipt_path = root.join(
        "fixtures/psion/observability/psion_broader_pretrain_run_observability_receipt_v1.json",
    );
    let benchmark_receipt_set_path =
        root.join("fixtures/psion/benchmarks/psion_benchmark_receipt_set_v1.json");
    let model_descriptor_path =
        root.join("fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json");

    let recipe_bundle = load_json(&recipe_bundle_path)?;
    let anchor_stage_receipt = load_json(&anchor_stage_receipt_path)?;
    let anchor_observability_receipt = load_json(&anchor_observability_receipt_path)?;
    let benchmark_receipt_set = load_json(&benchmark_receipt_set_path)?;
    let model_descriptor = load_json(&model_descriptor_path)?;

    let anchor_parameter_count = canonical_parameter_count(&model_descriptor)?;
    let anchor_train_token_budget =
        u64_at(&recipe_bundle, &["stage_schedule", "train_token_budget"])?;
    let anchor_validation_token_budget = u64_at(
        &recipe_bundle,
        &["stage_schedule", "validation_token_budget"],
    )?;
    let anchor_held_out_token_budget =
        u64_at(&recipe_bundle, &["stage_schedule", "held_out_token_budget"])?;
    let anchor_optimizer_steps = u64_at(&recipe_bundle, &["stage_schedule", "optimizer_steps"])?;
    let anchor_mean_tokens_per_second = u64_at(
        &anchor_observability_receipt,
        &["throughput", "mean_tokens_per_second"],
    )?;
    let anchor_wall_clock_ms = u64_at(
        &anchor_observability_receipt,
        &["throughput", "wall_clock_ms"],
    )?;
    let anchor_total_cost_microusd = u64_at(
        &anchor_observability_receipt,
        &["cost", "total_cost_microusd"],
    )?;
    let anchor_validation_loss_milli = weighted_validation_loss_milli(&anchor_stage_receipt)?;
    let (anchor_average_reasoning_pass_rate_bps, anchor_reasoning_floor_bps) =
        reasoning_pass_metrics(&benchmark_receipt_set)?;

    let measured_anchor = candidate_row(
        String::from("psion_actual_pretraining_internal128m_anchor"),
        String::from("measured_anchor"),
        String::from("internal128m"),
        anchor_parameter_count,
        anchor_train_token_budget,
        anchor_validation_token_budget,
        anchor_held_out_token_budget,
        anchor_optimizer_steps,
        anchor_mean_tokens_per_second,
        anchor_wall_clock_ms,
        anchor_total_cost_microusd,
        anchor_validation_loss_milli,
        anchor_average_reasoning_pass_rate_bps,
        anchor_reasoning_floor_bps,
        String::from("measured_anchor"),
        String::from(
            "Measured anchor comes directly from the retained broader-pretraining observability receipt, pretrain stage receipt, and benchmark receipt set.",
        ),
        true,
        true,
        None,
        String::from(
            "The canonical recipe stays on the measured 128M anchor because it is the largest candidate that clears the frozen stage-length, cost, and quality thresholds on the admitted four-node H100 lane.",
        ),
    );
    let smaller_projection = projected_candidate(
        String::from("psion_actual_pretraining_internal64m_projection"),
        String::from("smaller_projection"),
        String::from("internal64m"),
        67_108_864,
        anchor_parameter_count,
        anchor_mean_tokens_per_second,
        anchor_wall_clock_ms,
        anchor_total_cost_microusd,
        anchor_validation_loss_milli,
        anchor_average_reasoning_pass_rate_bps,
        anchor_reasoning_floor_bps,
        false,
        Some(String::from("smaller_than_selected_candidate")),
        String::from(
            "Smaller projection remains eligible but is not selected because the frozen rule chooses the largest eligible candidate rather than leaving unused budget on the table.",
        ),
    )?;
    let larger_projection = projected_candidate(
        String::from("psion_actual_pretraining_internal256m_projection"),
        String::from("larger_projection"),
        String::from("internal256m"),
        268_435_456,
        anchor_parameter_count,
        anchor_mean_tokens_per_second,
        anchor_wall_clock_ms,
        anchor_total_cost_microusd,
        anchor_validation_loss_milli,
        anchor_average_reasoning_pass_rate_bps,
        anchor_reasoning_floor_bps,
        false,
        Some(String::from("exceeds_stage_and_cost_budget")),
        String::from(
            "Larger projection improves the projected quality surfaces but exceeds the frozen wall-clock and total-cost ceilings on the admitted topology, so it cannot replace the canonical recipe yet.",
        ),
    )?;

    let selection_rule = PsionActualPretrainingScalingSelectionRule {
        rule_id: String::from("psion_actual_pretraining_scaling_rule_v1"),
        selection_policy: String::from("largest_eligible_candidate"),
        admitted_recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
        chosen_candidate_id: measured_anchor.candidate_id.clone(),
        tokens_per_parameter: TOKENS_PER_PARAMETER,
        tokens_per_step: TOKENS_PER_STEP,
        maximum_stage_length_ms: MAXIMUM_STAGE_LENGTH_MS,
        maximum_total_cost_microusd: MAXIMUM_TOTAL_COST_MICROUSD,
        maximum_validation_loss_milli: MAXIMUM_VALIDATION_LOSS_MILLI,
        minimum_reasoning_floor_bps: MINIMUM_REASONING_FLOOR_BPS,
        required_benchmark_package_families: vec![
            String::from("architecture_reasoning"),
            String::from("normative_spec_reading"),
            String::from("engineering_spec_interpretation"),
            String::from("memorization_versus_reasoning"),
        ],
        detail: String::from(
            "The scaling rule keeps recipe authority honest by choosing the largest candidate that still clears the admitted stage-length, cost, validation-loss, and reasoning-floor thresholds on the real lane.",
        ),
    };

    let mut bundle = PsionActualPretrainingScalingBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_SCHEMA_VERSION),
        scaling_bundle_id: String::from(PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        lane_spec: artifact_ref(&root, &lane_spec_path)?,
        recipe_bundle: artifact_ref(&root, &recipe_bundle_path)?,
        data_bundle: artifact_ref(&root, &data_bundle_path)?,
        systems_bundle: artifact_ref(&root, &systems_bundle_path)?,
        anchor_run_bundle: artifact_ref(&root, &anchor_run_bundle_path)?,
        anchor_stage_receipt: artifact_ref(&root, &anchor_stage_receipt_path)?,
        anchor_observability_receipt: artifact_ref(&root, &anchor_observability_receipt_path)?,
        benchmark_receipt_set: artifact_ref(&root, &benchmark_receipt_set_path)?,
        ablation_family_id: String::from("psion_actual_pretraining_scaling_family_v1"),
        candidates: vec![smaller_projection, measured_anchor, larger_projection],
        selection_rule,
        support_refs: vec![
            String::from("docs/PSION_ACTUAL_PRETRAINING_SCALING_BUNDLE.md"),
            String::from("docs/PSION_ACTUAL_PRETRAINING_RECIPE.md"),
            String::from("docs/TRAIN_SYSTEM.md"),
            String::from(
                "fixtures/psion/observability/psion_broader_pretrain_run_observability_receipt_v1.json",
            ),
            String::from("fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"),
            String::from("fixtures/psion/benchmarks/psion_benchmark_receipt_set_v1.json"),
        ],
        claim_boundary: String::from(
            "The actual-lane scaling bundle freezes one bounded 64M -> 128M -> 256M recipe-family comparison anchored to the retained broader-pretraining receipts. It does not claim an open-ended scaling-law service, unbounded sweep infrastructure, or detached research automation outside the frozen actual lane.",
        ),
        summary: String::from(
            "The canonical actual-pretraining scaling bundle binds one measured 128M anchor, one smaller projection, one larger projection, and one largest-eligible selection rule into actual recipe authority.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_scaling_bundle_digest(&bundle)?;
    bundle.validate()?;

    write_json(
        &fixtures_dir.join("psion_actual_pretraining_scaling_bundle_v1.json"),
        &bundle,
    )?;
    Ok(())
}

fn projected_candidate(
    candidate_id: String,
    candidate_kind: String,
    model_size_anchor: String,
    estimated_parameter_count: u64,
    anchor_parameter_count: u64,
    anchor_mean_tokens_per_second: u64,
    anchor_wall_clock_ms: u64,
    anchor_total_cost_microusd: u64,
    anchor_validation_loss_milli: u64,
    anchor_average_reasoning_pass_rate_bps: u32,
    anchor_reasoning_floor_bps: u32,
    selected_for_recipe: bool,
    rejection_reason: Option<String>,
    detail: String,
) -> Result<PsionActualPretrainingScalingCandidate, Box<dyn Error>> {
    let log2_steps = if estimated_parameter_count > anchor_parameter_count {
        1
    } else if estimated_parameter_count < anchor_parameter_count {
        -1
    } else {
        0
    };
    let projected_train_token_budget = estimated_parameter_count * TOKENS_PER_PARAMETER;
    let projected_mean_tokens_per_second = projected_tokens_per_second(
        estimated_parameter_count,
        anchor_parameter_count,
        anchor_mean_tokens_per_second,
    );
    let projected_wall_clock_ms = projected_wallclock_ms(
        projected_train_token_budget,
        projected_mean_tokens_per_second,
        anchor_parameter_count,
        anchor_mean_tokens_per_second,
        anchor_wall_clock_ms,
    )?;
    let projected_total_cost_microusd = round_scaled(
        anchor_total_cost_microusd,
        projected_wall_clock_ms,
        anchor_wall_clock_ms,
    );
    let projected_validation_loss_milli = signed_adjust(
        anchor_validation_loss_milli,
        -log2_steps * VALIDATION_LOSS_DELTA_PER_DOUBLING,
    )?;
    let projected_average_reasoning_pass_rate_bps = signed_adjust_u32(
        anchor_average_reasoning_pass_rate_bps,
        log2_steps * AVERAGE_PASS_DELTA_PER_DOUBLING,
    )?;
    let projected_reasoning_floor_bps = signed_adjust_u32(
        anchor_reasoning_floor_bps,
        log2_steps * FLOOR_PASS_DELTA_PER_DOUBLING,
    )?;

    let candidate = candidate_row(
        candidate_id,
        candidate_kind,
        model_size_anchor,
        estimated_parameter_count,
        projected_train_token_budget,
        projected_train_token_budget / 32,
        projected_train_token_budget / 128,
        projected_train_token_budget / TOKENS_PER_STEP,
        projected_mean_tokens_per_second,
        projected_wall_clock_ms,
        projected_total_cost_microusd,
        projected_validation_loss_milli,
        projected_average_reasoning_pass_rate_bps,
        projected_reasoning_floor_bps,
        String::from("projected_from_anchor"),
        String::from(
            "Projection uses the retained 128M broader-pretraining anchor, a fixed eight-tokens-per-parameter rule, and inverse-sqrt throughput scaling on the admitted four-node H100 topology.",
        ),
        projected_wall_clock_ms <= MAXIMUM_STAGE_LENGTH_MS
            && projected_total_cost_microusd <= MAXIMUM_TOTAL_COST_MICROUSD
            && projected_validation_loss_milli <= MAXIMUM_VALIDATION_LOSS_MILLI
            && projected_reasoning_floor_bps >= MINIMUM_REASONING_FLOOR_BPS,
        selected_for_recipe,
        rejection_reason,
        detail,
    );
    Ok(candidate)
}

#[allow(clippy::too_many_arguments)]
fn candidate_row(
    candidate_id: String,
    candidate_kind: String,
    model_size_anchor: String,
    estimated_parameter_count: u64,
    train_token_budget: u64,
    validation_token_budget: u64,
    held_out_token_budget: u64,
    optimizer_steps: u64,
    projected_mean_tokens_per_second: u64,
    projected_wall_clock_ms: u64,
    projected_total_cost_microusd: u64,
    projected_validation_loss_milli: u64,
    projected_average_reasoning_pass_rate_bps: u32,
    projected_reasoning_floor_bps: u32,
    evidence_kind: String,
    projection_basis: String,
    eligible_under_rule: bool,
    selected_for_recipe: bool,
    rejection_reason: Option<String>,
    detail: String,
) -> PsionActualPretrainingScalingCandidate {
    PsionActualPretrainingScalingCandidate {
        candidate_id,
        candidate_kind,
        model_size_anchor,
        estimated_parameter_count,
        train_token_budget,
        validation_token_budget,
        held_out_token_budget,
        optimizer_steps,
        projected_mean_tokens_per_second,
        projected_wall_clock_ms,
        projected_total_cost_microusd,
        projected_validation_loss_milli,
        projected_average_reasoning_pass_rate_bps,
        projected_reasoning_floor_bps,
        evidence_kind,
        projection_basis,
        eligible_under_rule,
        selected_for_recipe,
        rejection_reason,
        detail,
    }
}

fn canonical_parameter_count(model_descriptor: &Value) -> Result<u64, Box<dyn Error>> {
    let size_anchor = string_at(model_descriptor, &["size_anchor"])?;
    if size_anchor != "internal128m" {
        return Err(format!("unexpected model size anchor `{size_anchor}`").into());
    }
    Ok(134_217_728)
}

fn projected_tokens_per_second(
    estimated_parameter_count: u64,
    anchor_parameter_count: u64,
    anchor_mean_tokens_per_second: u64,
) -> u64 {
    let ratio = (anchor_parameter_count as f64 / estimated_parameter_count as f64).sqrt();
    (anchor_mean_tokens_per_second as f64 * ratio).round() as u64
}

fn projected_wallclock_ms(
    train_token_budget: u64,
    projected_mean_tokens_per_second: u64,
    anchor_parameter_count: u64,
    anchor_mean_tokens_per_second: u64,
    anchor_wall_clock_ms: u64,
) -> Result<u64, Box<dyn Error>> {
    let computed_anchor_ms = ((anchor_parameter_count * TOKENS_PER_PARAMETER) * 1000)
        .div_ceil(anchor_mean_tokens_per_second);
    let overhead_ratio = anchor_wall_clock_ms as f64 / computed_anchor_ms as f64;
    let raw_projected_ms = (train_token_budget * 1000).div_ceil(projected_mean_tokens_per_second);
    Ok((raw_projected_ms as f64 * overhead_ratio).round() as u64)
}

fn weighted_validation_loss_milli(stage_receipt: &Value) -> Result<u64, Box<dyn Error>> {
    let mut weighted_sum = 0u64;
    let mut total_share = 0u64;
    for report in array_at(stage_receipt, &["source_family_reports"])? {
        if string_field(report, "split_kind")? != "validation" {
            continue;
        }
        let share = u64_field(report, "token_share_bps_within_split")?;
        let loss = u64_field(report, "mean_next_token_loss_milli")?;
        weighted_sum += share * loss;
        total_share += share;
    }
    if total_share == 0 {
        return Err("validation reports missing from pretrain stage receipt".into());
    }
    Ok(weighted_sum / total_share)
}

fn reasoning_pass_metrics(benchmark_receipt_set: &Value) -> Result<(u32, u32), Box<dyn Error>> {
    let required = [
        "architecture_reasoning",
        "normative_spec_reading",
        "engineering_spec_interpretation",
        "memorization_versus_reasoning",
    ];
    let mut observed = Vec::new();
    for receipt in array_at(benchmark_receipt_set, &["receipts"])? {
        let package_family = string_field(receipt, "package_family")?;
        if !required.contains(&package_family.as_str()) {
            continue;
        }
        let metrics = array_field(receipt, "observed_metrics")?;
        let first_metric = metrics
            .first()
            .ok_or("benchmark receipt missing observed metrics")?;
        observed.push(u32_field(first_metric, "observed_bps")?);
    }
    if observed.len() != required.len() {
        return Err("missing required benchmark families for scaling bundle".into());
    }
    let average = observed.iter().copied().map(u64::from).sum::<u64>() / observed.len() as u64;
    let floor = observed
        .iter()
        .copied()
        .min()
        .ok_or("missing benchmark observations")?;
    Ok((average as u32, floor))
}

fn round_scaled(numerator_base: u64, numerator: u64, denominator: u64) -> u64 {
    ((numerator_base as f64) * (numerator as f64 / denominator as f64)).round() as u64
}

fn signed_adjust(base: u64, delta: i64) -> Result<u64, Box<dyn Error>> {
    let adjusted = base as i64 + delta;
    if adjusted <= 0 {
        return Err(format!("adjusted value became non-positive: {adjusted}").into());
    }
    Ok(adjusted as u64)
}

fn signed_adjust_u32(base: u32, delta: i64) -> Result<u32, Box<dyn Error>> {
    let adjusted = base as i64 + delta;
    if adjusted <= 0 {
        return Err(format!("adjusted value became non-positive: {adjusted}").into());
    }
    Ok(adjusted as u32)
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    let encoded = serde_json::to_vec_pretty(value)?;
    fs::write(path, encoded)?;
    Ok(())
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: sha256_hex(&bytes),
    })
}

fn stable_scaling_bundle_digest(
    bundle: &PsionActualPretrainingScalingBundle,
) -> Result<String, Box<dyn Error>> {
    let mut clone = bundle.clone();
    clone.bundle_digest.clear();
    let bytes = serde_json::to_vec(&clone)?;
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_scaling_bundle|");
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn load_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn array_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a [Value], Box<dyn Error>> {
    value_at(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array at {}", dotted(path)).into())
}

fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a [Value], Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array field `{field}`").into())
}

fn string_at(value: &Value, path: &[&str]) -> Result<String, Box<dyn Error>> {
    value_at(value, path)?
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("expected string at {}", dotted(path)).into())
}

fn u64_at(value: &Value, path: &[&str]) -> Result<u64, Box<dyn Error>> {
    value_at(value, path)?
        .as_u64()
        .ok_or_else(|| format!("expected u64 at {}", dotted(path)).into())
}

fn string_field(value: &Value, field: &str) -> Result<String, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| format!("expected string field `{field}`").into())
}

fn u64_field(value: &Value, field: &str) -> Result<u64, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("expected u64 field `{field}`").into())
}

fn u32_field(value: &Value, field: &str) -> Result<u32, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .map(|value| value as u32)
        .ok_or_else(|| format!("expected u32 field `{field}`").into())
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, Box<dyn Error>> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| format!("missing path {}", dotted(path)))?;
    }
    Ok(current)
}

fn dotted(path: &[&str]) -> String {
    path.join(".")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let mut current = std::env::current_dir()?;
    loop {
        if current.join("Cargo.toml").exists() && current.join("fixtures").exists() {
            return Ok(current);
        }
        if !current.pop() {
            return Err("could not locate workspace root".into());
        }
    }
}
