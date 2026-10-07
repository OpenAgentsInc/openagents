use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use psionic_serve::{
    CpuGgufQwen35TextGenerationService, Qwen35CpuRecurrentTrace, Qwen35CpuRecurrentTracePhase,
    TokenId, TokenSequence,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const SCHEMA_VERSION: &str = "psionic_qwen38_cpu_recurrent_intermediate_parity_v1";
const LLAMA_TRACE_SCHEMA_VERSION: &str = "qwen38_llama_cpp_recurrent_trace_v1";
const PINNED_LLAMA_CPP_REVISION: &str = "9b05354ec6fb58b4e665e9a39ebc40285c015638";
const PREFILL_TOKENS: [u32; 2] = [9419, 11];
const DECODE_TOKENS: [u32; 1] = [353];
const STAGES: [&str; 14] = [
    "attn_norm",
    "linear_attn_qkv_mixed",
    "conv_output_raw",
    "conv_output_silu",
    "a_softplus",
    "gate",
    "beta_sigmoid",
    "q_conv_predelta",
    "k_conv_predelta",
    "v_conv_predelta",
    "new_state",
    "attn_output",
    "final_output",
    "linear_attn_out",
];

#[derive(Clone, Debug)]
struct ManifestEntry {
    phase: String,
    stage: String,
    shape: [usize; 4],
    element_count: usize,
    file: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Tolerance {
    max_abs_diff: f64,
    normalized_rmse: f64,
    minimum_cosine_similarity: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ErrorMetrics {
    max_abs_diff: f64,
    mean_abs_diff: f64,
    rmse: f64,
    reference_rms: f64,
    normalized_rmse: f64,
    cosine_similarity: f64,
}

#[derive(Clone, Debug, Serialize)]
struct TensorComparison {
    phase: String,
    stage: String,
    shape: [usize; 4],
    element_count: usize,
    state_layout: String,
    psionic_f32le_sha256: String,
    llama_cpp_f32le_sha256: String,
    metrics: ErrorMetrics,
    tolerance: Tolerance,
    passed: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ArtifactIdentity {
    filename: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct ComparatorIdentity {
    implementation: String,
    revision: String,
    trace_schema_version: String,
    backend: String,
    layer_index: usize,
}

#[derive(Clone, Debug, Serialize)]
struct PsionicIdentity {
    implementation: String,
    model_id: String,
    plan_digest: String,
    backend: String,
    layer_index: usize,
}

#[derive(Clone, Debug, Serialize)]
struct TokenContract {
    prompt_text: String,
    prefill_token_ids: Vec<u32>,
    retained_decode_token_ids: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
struct ParityReport {
    schema_version: String,
    artifact: ArtifactIdentity,
    comparator: ComparatorIdentity,
    psionic: PsionicIdentity,
    tokens: TokenContract,
    comparisons: Vec<TensorComparison>,
    all_passed: bool,
    claim_boundary: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let model_path = required_path(&mut args, "MODEL_GGUF")?;
    let llama_trace_dir = required_path(&mut args, "LLAMA_TRACE_DIR")?;
    let report_path = required_path(&mut args, "REPORT_JSON")?;
    if args.next().is_some() {
        return Err(usage_error().into());
    }

    let metadata = read_metadata(&llama_trace_dir.join("metadata.tsv"))?;
    require_metadata(&metadata, "schema_version", LLAMA_TRACE_SCHEMA_VERSION)?;
    require_metadata(&metadata, "llama_cpp_revision", PINNED_LLAMA_CPP_REVISION)?;
    require_metadata(&metadata, "prefill_tokens", "9419,11")?;
    require_metadata(&metadata, "decode_tokens", "353")?;
    require_metadata(&metadata, "layer_index", "0")?;
    require_metadata(&metadata, "byte_order", "little_endian")?;
    let manifest = read_manifest(&llama_trace_dir.join("manifest.tsv"))?;
    let model_metadata = fs::metadata(&model_path)?;
    let model_byte_length = model_metadata.len();
    let model_byte_length_text = model_byte_length.to_string();
    let model_sha256 = sha256_file(&model_path)?;
    require_metadata(
        &metadata,
        "artifact_byte_length",
        model_byte_length_text.as_str(),
    )?;
    require_metadata(&metadata, "artifact_sha256", model_sha256.as_str())?;

    let service = CpuGgufQwen35TextGenerationService::from_gguf_path(&model_path)?;
    let trace = service.trace_first_recurrent_layer(
        &TokenSequence::new(PREFILL_TOKENS.into_iter().map(TokenId).collect::<Vec<_>>()),
        &TokenSequence::new(DECODE_TOKENS.into_iter().map(TokenId).collect::<Vec<_>>()),
    )?;
    let comparisons = compare_all(&trace, &manifest, &llama_trace_dir)?;
    let all_passed = comparisons.iter().all(|comparison| comparison.passed);
    let report = ParityReport {
        schema_version: SCHEMA_VERSION.to_string(),
        artifact: ArtifactIdentity {
            filename: model_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid model filename")
                })?
                .to_string(),
            byte_length: model_byte_length,
            sha256: model_sha256,
        },
        comparator: ComparatorIdentity {
            implementation: String::from("ggml-org/llama.cpp"),
            revision: PINNED_LLAMA_CPP_REVISION.to_string(),
            trace_schema_version: LLAMA_TRACE_SCHEMA_VERSION.to_string(),
            backend: String::from("cpu"),
            layer_index: 0,
        },
        psionic: PsionicIdentity {
            implementation: String::from("native_psionic_cpu"),
            model_id: trace.model_id,
            plan_digest: trace.plan_digest,
            backend: String::from("cpu"),
            layer_index: trace.layer_index,
        },
        tokens: TokenContract {
            prompt_text: String::from("Hello,"),
            prefill_token_ids: PREFILL_TOKENS.to_vec(),
            retained_decode_token_ids: DECODE_TOKENS.to_vec(),
        },
        comparisons,
        all_passed,
        claim_boundary: String::from(
            "Layer-zero recurrent intermediates only. Prefill and retained-state decode use the pinned UD-Q3_K_XL artifact on CPU; this report does not claim full-logit, CUDA, Metal, MTP, or multimodal parity.",
        ),
    };
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "comparisons={} passed={} report={}",
        report.comparisons.len(),
        report.all_passed,
        report_path.display()
    );
    for comparison in &report.comparisons {
        println!(
            "{}/{} pass={} max_abs={:.8} nrmse={:.8} cosine={:.8}",
            comparison.phase,
            comparison.stage,
            comparison.passed,
            comparison.metrics.max_abs_diff,
            comparison.metrics.normalized_rmse,
            comparison.metrics.cosine_similarity
        );
    }
    Ok(all_passed)
}

fn required_path(
    args: &mut impl Iterator<Item = String>,
    _label: &str,
) -> Result<PathBuf, io::Error> {
    args.next().map(PathBuf::from).ok_or_else(usage_error)
}

fn usage_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: qwen38_cpu_intermediate_compare MODEL_GGUF LLAMA_TRACE_DIR REPORT_JSON",
    )
}

fn read_metadata(path: &Path) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    let mut lines = contents.lines();
    if lines.next() != Some("key\tvalue") {
        return Err(format!("invalid metadata header in {}", path.display()).into());
    }
    let mut metadata = BTreeMap::new();
    for (line_index, line) in lines.enumerate() {
        let (key, value) = line.split_once('\t').ok_or_else(|| {
            format!(
                "invalid metadata row {} in {}",
                line_index + 2,
                path.display()
            )
        })?;
        if metadata
            .insert(key.to_string(), value.to_string())
            .is_some()
        {
            return Err(format!("duplicate metadata key `{key}` in {}", path.display()).into());
        }
    }
    Ok(metadata)
}

fn require_metadata(
    metadata: &BTreeMap<String, String>,
    key: &str,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    let actual = metadata
        .get(key)
        .ok_or_else(|| format!("missing llama trace metadata `{key}`"))?;
    if actual != expected {
        return Err(format!(
            "llama trace metadata `{key}` mismatch: expected `{expected}`, actual `{actual}`"
        )
        .into());
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Vec<ManifestEntry>, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    let mut lines = contents.lines();
    if lines.next() != Some("phase\tstage\tne0\tne1\tne2\tne3\telement_count\tfile") {
        return Err(format!("invalid trace manifest header in {}", path.display()).into());
    }
    let mut entries = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 8 {
            return Err(format!(
                "invalid trace manifest row {} in {}",
                line_index + 2,
                path.display()
            )
            .into());
        }
        entries.push(ManifestEntry {
            phase: fields[0].to_string(),
            stage: fields[1].to_string(),
            shape: [
                fields[2].parse()?,
                fields[3].parse()?,
                fields[4].parse()?,
                fields[5].parse()?,
            ],
            element_count: fields[6].parse()?,
            file: fields[7].to_string(),
        });
    }
    if entries.len() != STAGES.len() * 2 {
        return Err(format!(
            "trace manifest entry count mismatch: expected {}, actual {}",
            STAGES.len() * 2,
            entries.len()
        )
        .into());
    }
    Ok(entries)
}

fn compare_all(
    trace: &Qwen35CpuRecurrentTrace,
    manifest: &[ManifestEntry],
    trace_dir: &Path,
) -> Result<Vec<TensorComparison>, Box<dyn Error>> {
    let mut comparisons = Vec::with_capacity(STAGES.len() * 2);
    for (phase_label, phase, token_count) in [
        (
            "prefill",
            Qwen35CpuRecurrentTracePhase::Prefill,
            PREFILL_TOKENS.len(),
        ),
        (
            "decode",
            Qwen35CpuRecurrentTracePhase::Decode,
            DECODE_TOKENS.len(),
        ),
    ] {
        for stage in STAGES {
            let entry = manifest
                .iter()
                .find(|entry| entry.phase == phase_label && entry.stage == stage)
                .ok_or_else(|| format!("missing manifest entry for {phase_label}/{stage}"))?;
            let observations = trace
                .tensors
                .iter()
                .filter(|tensor| tensor.phase == phase && tensor.stage == stage)
                .collect::<Vec<_>>();
            if observations.len() != token_count {
                return Err(format!(
                    "Psionic observation count mismatch for {phase_label}/{stage}: expected {token_count}, actual {}",
                    observations.len()
                )
                .into());
            }
            let actual = if stage == "new_state" {
                observations
                    .last()
                    .expect("observation count was checked")
                    .values
                    .clone()
            } else {
                observations
                    .iter()
                    .flat_map(|tensor| tensor.values.iter().copied())
                    .collect()
            };
            if actual.len() != entry.element_count {
                return Err(format!(
                    "Psionic element count mismatch for {phase_label}/{stage}: expected {}, actual {}",
                    entry.element_count,
                    actual.len()
                )
                .into());
            }
            if actual.iter().any(|value| !value.is_finite()) {
                return Err(
                    format!("non-finite Psionic trace value for {phase_label}/{stage}").into(),
                );
            }
            validate_observation_shapes(stage, observations.as_slice(), entry.shape, token_count)?;
            let reference = read_f32le(&trace_dir.join(&entry.file), entry.element_count)?;
            let metrics = error_metrics(actual.as_slice(), reference.as_slice());
            let tolerance = tolerance_for(stage);
            let passed = metrics.max_abs_diff <= tolerance.max_abs_diff
                && metrics.normalized_rmse <= tolerance.normalized_rmse
                && metrics.cosine_similarity >= tolerance.minimum_cosine_similarity;
            comparisons.push(TensorComparison {
                phase: phase_label.to_string(),
                stage: stage.to_string(),
                shape: entry.shape,
                element_count: entry.element_count,
                state_layout: if stage == "new_state" {
                    String::from("ggml_transposed_state_direct")
                } else {
                    String::from("not_applicable")
                },
                psionic_f32le_sha256: sha256_f32(actual.as_slice()),
                llama_cpp_f32le_sha256: sha256_f32(reference.as_slice()),
                metrics,
                tolerance,
                passed,
            });
        }
    }
    Ok(comparisons)
}

fn validate_observation_shapes(
    stage: &str,
    observations: &[&psionic_serve::Qwen35CpuRecurrentTraceTensor],
    expected: [usize; 4],
    token_count: usize,
) -> Result<(), Box<dyn Error>> {
    if stage == "new_state" {
        for observation in observations {
            if observation.shape != expected {
                return Err(format!(
                    "Psionic shape mismatch for new_state: expected {expected:?}, actual {:?}",
                    observation.shape
                )
                .into());
            }
        }
        return Ok(());
    }
    let token_dimension = match stage {
        "beta_sigmoid" | "q_conv_predelta" | "k_conv_predelta" | "v_conv_predelta"
        | "attn_output" => 2,
        _ => 1,
    };
    if expected[token_dimension] != token_count {
        return Err(format!(
            "llama.cpp token dimension mismatch for {stage}: expected {token_count}, shape {expected:?}"
        )
        .into());
    }
    let mut per_token_shape = expected;
    per_token_shape[token_dimension] = 1;
    for observation in observations {
        if observation.shape != per_token_shape {
            return Err(format!(
                "Psionic per-token shape mismatch for {stage}: expected {per_token_shape:?}, actual {:?}",
                observation.shape
            )
            .into());
        }
    }
    Ok(())
}

fn tolerance_for(stage: &str) -> Tolerance {
    match stage {
        "attn_norm" => Tolerance {
            max_abs_diff: 0.000_1,
            normalized_rmse: 0.000_001,
            minimum_cosine_similarity: 0.999_999,
        },
        "q_conv_predelta" | "k_conv_predelta" => Tolerance {
            max_abs_diff: 0.005,
            normalized_rmse: 0.006,
            minimum_cosine_similarity: 0.999_98,
        },
        "a_softplus" => Tolerance {
            max_abs_diff: 0.06,
            normalized_rmse: 0.002,
            minimum_cosine_similarity: 0.999_99,
        },
        "gate" | "beta_sigmoid" => Tolerance {
            max_abs_diff: 0.01,
            normalized_rmse: 0.005,
            minimum_cosine_similarity: 0.999_9,
        },
        "linear_attn_qkv_mixed" => Tolerance {
            max_abs_diff: 0.3,
            normalized_rmse: 0.006,
            minimum_cosine_similarity: 0.999_98,
        },
        "conv_output_raw" | "conv_output_silu" | "v_conv_predelta" => Tolerance {
            max_abs_diff: 0.15,
            normalized_rmse: 0.005,
            minimum_cosine_similarity: 0.999_99,
        },
        "new_state" => Tolerance {
            max_abs_diff: 0.15,
            normalized_rmse: 0.01,
            minimum_cosine_similarity: 0.999_95,
        },
        "attn_output" => Tolerance {
            max_abs_diff: 0.01,
            normalized_rmse: 0.008,
            minimum_cosine_similarity: 0.999_99,
        },
        "final_output" => Tolerance {
            max_abs_diff: 0.01,
            normalized_rmse: 0.005,
            minimum_cosine_similarity: 0.999_99,
        },
        "linear_attn_out" => Tolerance {
            max_abs_diff: 0.1,
            normalized_rmse: 0.012,
            minimum_cosine_similarity: 0.999_9,
        },
        _ => unreachable!("all stages are statically enumerated"),
    }
}

fn read_f32le(path: &Path, expected_count: usize) -> Result<Vec<f32>, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let expected_bytes = expected_count.saturating_mul(std::mem::size_of::<f32>());
    if bytes.len() != expected_bytes {
        return Err(format!(
            "F32 trace byte length mismatch for {}: expected {expected_bytes}, actual {}",
            path.display(),
            bytes.len()
        )
        .into());
    }
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect::<Vec<_>>();
    if values.iter().any(|value| !value.is_finite()) {
        return Err(format!("non-finite llama.cpp trace value in {}", path.display()).into());
    }
    Ok(values)
}

fn error_metrics(actual: &[f32], reference: &[f32]) -> ErrorMetrics {
    let mut max_abs_diff = 0.0_f64;
    let mut sum_abs_diff = 0.0_f64;
    let mut sum_squared_diff = 0.0_f64;
    let mut sum_reference_squared = 0.0_f64;
    let mut sum_actual_squared = 0.0_f64;
    let mut dot_product = 0.0_f64;
    for (&actual, &reference) in actual.iter().zip(reference.iter()) {
        let actual = f64::from(actual);
        let reference = f64::from(reference);
        let difference = (actual - reference).abs();
        max_abs_diff = max_abs_diff.max(difference);
        sum_abs_diff += difference;
        sum_squared_diff += difference * difference;
        sum_reference_squared += reference * reference;
        sum_actual_squared += actual * actual;
        dot_product += actual * reference;
    }
    let count = actual.len().max(1) as f64;
    let rmse = (sum_squared_diff / count).sqrt();
    let reference_rms = (sum_reference_squared / count).sqrt();
    let denominator = (sum_actual_squared * sum_reference_squared).sqrt();
    let cosine_similarity = if denominator == 0.0 {
        if sum_actual_squared == 0.0 && sum_reference_squared == 0.0 {
            1.0
        } else {
            0.0
        }
    } else {
        dot_product / denominator
    };
    ErrorMetrics {
        max_abs_diff,
        mean_abs_diff: sum_abs_diff / count,
        rmse,
        reference_rms,
        normalized_rmse: rmse / reference_rms.max(1.0e-12),
        cosine_similarity,
    }
}

fn sha256_f32(values: &[f32]) -> String {
    let mut digest = Sha256::new();
    for value in values {
        digest.update(value.to_le_bytes());
    }
    hex::encode(digest.finalize())
}

fn sha256_file(path: &Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 8 * 1024 * 1024];
    loop {
        let read = file.read(buffer.as_mut_slice())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}
