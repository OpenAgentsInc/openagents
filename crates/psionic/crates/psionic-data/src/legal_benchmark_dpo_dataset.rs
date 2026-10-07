use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const LEGAL_DPO_DATASET_SCHEMA_VERSION: &str = "legal_dpo_v1";
pub const LEGAL_DPO_DATASET_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.dpo_dataset_manifest.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalDpoMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalDpoPreferencePair {
    pub schema_version: String,
    pub pair_id: String,
    pub prompt: Vec<LegalDpoMessage>,
    pub chosen: String,
    pub rejected: String,
    pub reason: String,
    pub source_run_ids: Vec<String>,
    pub visibility: String,
    pub exclusion_flags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalDpoExcludedInput {
    pub source_path: String,
    pub source_run_id: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalDpoDatasetManifest {
    pub schema_version: String,
    pub dataset_id: String,
    pub output_jsonl: String,
    pub included_count: usize,
    pub excluded_count: usize,
    pub pair_counts_by_failure_class: BTreeMap<String, usize>,
    pub pair_counts_by_family: BTreeMap<String, usize>,
    pub source_receipts: Vec<String>,
    pub excluded_inputs: Vec<LegalDpoExcludedInput>,
    pub dataset_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalDpoDatasetBuilderConfig {
    pub runs_root: PathBuf,
    pub out_jsonl: PathBuf,
    pub manifest_json: PathBuf,
    pub dataset_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalDpoDatasetBuildResult {
    pub pairs: Vec<LegalDpoPreferencePair>,
    pub manifest: LegalDpoDatasetManifest,
}

#[derive(Debug, Error)]
pub enum LegalDpoDatasetBuilderError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("builder argument error: {0}")]
    InvalidArgument(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GoodLegalRun {
    run_id: String,
    task_id: String,
    visibility: String,
    required_path: String,
    answer_text: String,
    answer_file_count: usize,
    tool_trace_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BadLegalRun {
    run_id: String,
    full_prompt: String,
    failure_class: String,
    required_path: String,
    bad_response: String,
    correction: String,
}

pub fn build_legal_benchmark_dpo_dataset(
    config: &LegalDpoDatasetBuilderConfig,
) -> Result<LegalDpoDatasetBuildResult, LegalDpoDatasetBuilderError> {
    if !config.runs_root.exists() {
        return Err(LegalDpoDatasetBuilderError::InvalidArgument(format!(
            "runs root does not exist: {}",
            config.runs_root.display()
        )));
    }
    let mut json_paths = Vec::new();
    collect_json_files(config.runs_root.as_path(), &mut json_paths)?;
    json_paths.sort();

    let mut good_runs = Vec::new();
    let mut bad_runs = Vec::new();
    let mut excluded_inputs = Vec::new();
    let mut source_receipts = BTreeSet::new();

    for path in json_paths {
        let value = read_json(path.as_path())?;
        if looks_like_run_receipt(&value) {
            match parse_good_run(path.as_path(), &value) {
                Ok(good) => {
                    source_receipts.insert(receipt_ref(config.runs_root.as_path(), path.as_path()));
                    good_runs.push(good);
                }
                Err(reason) => excluded_inputs.push(LegalDpoExcludedInput {
                    source_path: path.display().to_string(),
                    source_run_id: run_id_from_receipt(&value).map(str::to_owned),
                    reason,
                }),
            }
        } else if looks_like_bad_run(&value) {
            match parse_bad_run(path.as_path(), &value) {
                Ok(bad) => {
                    source_receipts.insert(path.display().to_string());
                    bad_runs.push(bad);
                }
                Err(reason) => excluded_inputs.push(LegalDpoExcludedInput {
                    source_path: path.display().to_string(),
                    source_run_id: bad_run_id(&value).map(str::to_owned),
                    reason,
                }),
            }
        }
    }

    let mut pairs = Vec::new();
    for good in &good_runs {
        for bad in &bad_runs {
            append_preference_pairs(&mut pairs, good, bad);
        }
    }
    pairs.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    write_pairs(config.out_jsonl.as_path(), &pairs)?;
    let manifest = LegalDpoDatasetManifest {
        schema_version: String::from(LEGAL_DPO_DATASET_MANIFEST_SCHEMA_VERSION),
        dataset_id: config.dataset_id.clone(),
        output_jsonl: config.out_jsonl.display().to_string(),
        included_count: pairs.len(),
        excluded_count: excluded_inputs.len(),
        pair_counts_by_failure_class: pair_counts_by_failure_class(&pairs),
        pair_counts_by_family: pair_counts_by_family(&pairs),
        source_receipts: source_receipts.into_iter().collect(),
        excluded_inputs,
        dataset_hash: dataset_hash(&pairs)?,
    };
    write_json(config.manifest_json.as_path(), &manifest)?;
    Ok(LegalDpoDatasetBuildResult { pairs, manifest })
}

pub fn load_legal_dpo_dataset(
    path: impl AsRef<Path>,
) -> Result<Vec<LegalDpoPreferencePair>, LegalDpoDatasetBuilderError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| LegalDpoDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut pairs = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        pairs.push(serde_json::from_str(line).map_err(|source| {
            LegalDpoDatasetBuilderError::Json {
                path: path.to_path_buf(),
                source,
            }
        })?);
    }
    Ok(pairs)
}

fn parse_good_run(path: &Path, value: &Value) -> Result<GoodLegalRun, String> {
    let run_spec = value.get("run_spec").unwrap_or(&Value::Null);
    let run_id = string_at(run_spec, "run_id").unwrap_or("unknown_run");
    let task_id = string_at(run_spec, "task_id").unwrap_or("unknown_task");
    let visibility = string_at(run_spec, "benchmark_visibility").unwrap_or("unknown");
    if let Some(reason) = visibility_exclusion_reason(visibility) {
        return Err(reason);
    }
    if let Some(reason) = integrity_exclusion_reason(value) {
        return Err(reason);
    }
    if contains_forbidden_training_marker(value) {
        return Err(String::from("hidden/private/scorer-only marker found"));
    }
    let answer_files = read_answer_files_for_receipt(path, value)?;
    if answer_files.is_empty() {
        return Err(String::from("missing answer file content"));
    }
    let required_path = answer_files
        .first()
        .map(|answer| answer.0.clone())
        .unwrap_or_else(|| String::from("memo.md"));
    let answer_text = answer_files
        .iter()
        .map(|(relative_path, content)| format!("{relative_path}:\n{content}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let tool_trace_summary = value
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            format!(
                "{} tool call(s), including model-authored file write",
                calls.len()
            )
        })
        .unwrap_or_else(|| String::from("model-authored file write"));
    Ok(GoodLegalRun {
        run_id: run_id.to_string(),
        task_id: task_id.to_string(),
        visibility: training_visibility(visibility),
        required_path,
        answer_text,
        answer_file_count: answer_files.len(),
        tool_trace_summary,
    })
}

fn parse_bad_run(_path: &Path, value: &Value) -> Result<BadLegalRun, String> {
    if !value
        .get("training_eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(value
            .get("training_eligibility_reasons")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| String::from("bad run is not training eligible")));
    }
    if let Some(visibility) = string_at(value, "benchmark_visibility")
        .or_else(|| string_at(value, "visibility"))
        .or_else(|| string_at(value, "training_visibility"))
    {
        if let Some(reason) = visibility_exclusion_reason(visibility) {
            return Err(reason);
        }
    }
    if let Some(reason) = bad_run_integrity_exclusion_reason(value) {
        return Err(reason);
    }
    if contains_forbidden_training_marker(value) {
        return Err(String::from("hidden/private/scorer-only marker found"));
    }
    let run_id = bad_run_id(value).unwrap_or("bad_run.unknown");
    let required_path = value
        .get("required_file_paths")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .unwrap_or("memo.md");
    let failure_class =
        canonical_failure_class(string_at(value, "failure_class").unwrap_or("other"));
    Ok(BadLegalRun {
        run_id: run_id.trim_start_matches("bad_run.").to_string(),
        full_prompt: string_at(value, "full_prompt")
            .unwrap_or("Write the required legal benchmark answer file.")
            .to_string(),
        failure_class,
        required_path: required_path.to_string(),
        bad_response: string_at(value, "full_model_response")
            .unwrap_or("The prior run did not produce the required answer.")
            .to_string(),
        correction: string_at(value, "suggested_correction")
            .unwrap_or("Write the required answer file, validate it, then submit.")
            .to_string(),
    })
}

fn append_preference_pairs(
    pairs: &mut Vec<LegalDpoPreferencePair>,
    good: &GoodLegalRun,
    bad: &BadLegalRun,
) {
    let variants = pair_variants();
    for (index, variant) in variants.iter().enumerate() {
        let pair_id = format!(
            "{}.{}.{}.{}",
            good.run_id,
            bad.run_id,
            variant.family,
            index + 1
        );
        pairs.push(LegalDpoPreferencePair {
            schema_version: String::from(LEGAL_DPO_DATASET_SCHEMA_VERSION),
            pair_id,
            prompt: vec![
                system_message(),
                LegalDpoMessage {
                    role: String::from("user"),
                    content: variant.prompt(good, bad),
                },
            ],
            chosen: variant.chosen(good, bad),
            rejected: variant.rejected(good, bad),
            reason: bad.failure_class.clone(),
            source_run_ids: vec![good.run_id.clone(), bad.run_id.clone()],
            visibility: good.visibility.clone(),
            exclusion_flags: Vec::new(),
        });
    }
}

#[derive(Clone, Copy, Debug)]
struct PairVariant {
    family: &'static str,
    prompt_template: &'static str,
    chosen_template: &'static str,
    rejected_template: &'static str,
}

impl PairVariant {
    fn prompt(self, good: &GoodLegalRun, bad: &BadLegalRun) -> String {
        let focus = self
            .prompt_template
            .replace("{task_id}", good.task_id.as_str())
            .replace("{required_path}", good.required_path.as_str())
            .replace("{bad_required_path}", bad.required_path.as_str());
        format!("{}\n\nTraining focus: {focus}", bad.full_prompt)
    }

    fn chosen(self, good: &GoodLegalRun, bad: &BadLegalRun) -> String {
        self.chosen_template
            .replace("{required_path}", good.required_path.as_str())
            .replace("{answer_text}", good.answer_text.as_str())
            .replace("{tool_trace_summary}", good.tool_trace_summary.as_str())
            .replace("{answer_file_count}", &good.answer_file_count.to_string())
            .replace("{correction}", bad.correction.as_str())
    }

    fn rejected(self, good: &GoodLegalRun, bad: &BadLegalRun) -> String {
        self.rejected_template
            .replace("{required_path}", good.required_path.as_str())
            .replace("{bad_required_path}", bad.required_path.as_str())
            .replace("{bad_response}", bad.bad_response.as_str())
            .replace("{failure_class}", bad.failure_class.as_str())
    }
}

fn pair_variants() -> Vec<PairVariant> {
    vec![
        variant(
            "file_discipline",
            "For task {task_id}, the completion should write the required legal answer file at `{required_path}`.",
            "Write `{required_path}` through the output file tool, then validate that the file exists.\n\n{answer_text}",
            "{bad_response}\n\nNo model-written `{required_path}` exists.",
        ),
        variant(
            "file_discipline",
            "The completion should prove that the required file exists before submission.",
            "The correct trajectory writes `{required_path}`, checks the file, and submits only after validation. {tool_trace_summary}.",
            "{bad_response}\n\nThe failed trajectory stops before a valid answer file is written.",
        ),
        variant(
            "file_discipline",
            "The completion should follow legal benchmark file discipline for `{required_path}`.",
            "Chosen behavior: model-authored write to `{required_path}` with {answer_file_count} answer file(s) retained.\n\n{answer_text}",
            "Rejected behavior: {failure_class}; {bad_response}",
        ),
        variant(
            "file_discipline",
            "The completion should leave a scorable answer artifact.",
            "The run leaves a scorable, model-written artifact at `{required_path}`.\n\n{answer_text}",
            "The run does not leave a scorable answer artifact at `{required_path}`.\n\n{bad_response}",
        ),
        variant(
            "correct_path",
            "The completion should write the required path exactly.",
            "Use root `output` and relative_path `{required_path}`. Do not invent another path.\n\n{answer_text}",
            "The rejected response fails path discipline for `{bad_required_path}`.\n\n{bad_response}",
        ),
        variant(
            "correct_path",
            "For `{required_path}`, the completion should comply with the exact output path.",
            "The answer is placed at the required path `{required_path}` and can be scored there.",
            "The bad run does not place a valid answer at the required path. {bad_response}",
        ),
        variant(
            "correct_path",
            "The completion should avoid wrong-path output.",
            "Correct path trajectory: write `{required_path}`, validate `{required_path}`, then submit.",
            "Wrong or missing path trajectory: {bad_response}",
        ),
        variant(
            "correct_path",
            "The completion should create an answer the scorer can find at the requested file path.",
            "The scorer can find the answer at `{required_path}`.\n\n{answer_text}",
            "The scorer cannot find a valid answer at `{required_path}`.\n\n{bad_response}",
        ),
        variant(
            "source_grounding",
            "The completion should ground the legal answer in the source documents.",
            "Use only source-supported facts and write the concise work product.\n\n{answer_text}",
            "The rejected answer is not source-grounded enough for training.\n\n{bad_response}",
        ),
        variant(
            "source_grounding",
            "The completion should produce source-grounded legal work instead of unsupported output.",
            "The answer states the legal conclusion using the provided source material.\n\n{answer_text}",
            "The failed run does not produce a reliable source-grounded answer. {bad_response}",
        ),
        variant(
            "source_grounding",
            "The completion should teach a legal benchmark agent to rely on source documents.",
            "The chosen answer is tied to source material and required output discipline.\n\n{answer_text}",
            "The rejected answer should not be imitated: {bad_response}",
        ),
        variant(
            "source_grounding",
            "The completion should avoid hallucinated or missing source use.",
            "Source-grounded response:\n\n{answer_text}",
            "Rejected response with missing source use or no usable answer:\n\n{bad_response}",
        ),
        variant(
            "conciseness",
            "The completion should be a concise legal work product.",
            "Concise answer:\n\n{answer_text}",
            "Rejected answer is not a usable concise legal work product:\n\n{bad_response}",
        ),
        variant(
            "conciseness",
            "The completion should be short while still writing the required file.",
            "Write the necessary answer directly to `{required_path}` without rambling.\n\n{answer_text}",
            "The failed response is not a concise scorable file answer. {bad_response}",
        ),
        variant(
            "conciseness",
            "The completion should suit direct-answer legal benchmark mode.",
            "Direct answer with file write:\n\n{answer_text}",
            "Rejected non-answer or rambling trajectory:\n\n{bad_response}",
        ),
        variant(
            "conciseness",
            "The completion should be brief, complete, and written into the file.",
            "Brief complete file answer at `{required_path}`:\n\n{answer_text}",
            "Not a brief complete file answer:\n\n{bad_response}",
        ),
        variant(
            "submission",
            "The completion should write, check, and submit.",
            "Correct trajectory: write `{required_path}`, validate it, then submit.\n\n{answer_text}",
            "Rejected trajectory does not complete write-check-submit. {bad_response}",
        ),
        variant(
            "submission",
            "The completion should submit the final answer only after file validation.",
            "The model writes and checks the output before submission. {tool_trace_summary}.",
            "The rejected model answers in chat or stops early instead of submitting a valid file. {bad_response}",
        ),
        variant(
            "submission",
            "The completion should show the end-of-task behavior the agent should imitate.",
            "Imitate: write the required file, verify, then submit.",
            "Do not imitate: {bad_response}",
        ),
        variant(
            "submission",
            "The completion should create the final work product before finishing.",
            "Final work product exists at `{required_path}` before finish.\n\n{answer_text}",
            "Final work product is missing or not validated before finish.\n\n{bad_response}",
        ),
        variant(
            "integrity_safe",
            "The completion should be safe for preference training.",
            "Safe chosen output: model-written file, valid integrity, no harness-added answer text.\n\n{answer_text}",
            "Rejected behavior is only used as a negative trajectory, never as a chosen answer: {bad_response}",
        ),
        variant(
            "integrity_safe",
            "The completion should be a model-written answer file, not an invalid or missing answer.",
            "Integrity-safe answer file at `{required_path}`:\n\n{answer_text}",
            "Rejected trajectory is not an integrity-valid answer file:\n\n{bad_response}",
        ),
    ]
}

fn variant(
    family: &'static str,
    prompt_template: &'static str,
    chosen_template: &'static str,
    rejected_template: &'static str,
) -> PairVariant {
    PairVariant {
        family,
        prompt_template,
        chosen_template,
        rejected_template,
    }
}

fn visibility_exclusion_reason(visibility: &str) -> Option<String> {
    match visibility {
        "public" | "public_training" | "synthetic" | "synthetic_training" | "internal"
        | "internal_training" => None,
        "private" | "private_training" => Some(String::from(
            "private benchmark labels are audit-only by default",
        )),
        "hidden" | "hidden_training" => {
            Some(String::from("hidden benchmark labels are not trainable"))
        }
        _ => Some(String::from("unknown benchmark visibility")),
    }
}

fn integrity_exclusion_reason(value: &Value) -> Option<String> {
    let integrity_valid = value
        .get("integrity")
        .and_then(|integrity| integrity.get("valid"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !integrity_valid {
        return Some(String::from("answer integrity failure"));
    }
    let answer_files = value
        .get("answer_files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if answer_files.is_empty() {
        return Some(String::from("missing answer file metadata"));
    }
    for answer in answer_files {
        if !answer
            .get("integrity_valid")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Some(String::from("answer file integrity failure"));
        }
        if answer.get("last_modifying_actor").and_then(Value::as_str) != Some("model") {
            return Some(String::from("answer was not last modified by the model"));
        }
        if answer.get("creation_actor").and_then(Value::as_str) != Some("model") {
            return Some(String::from("answer was not created by the model"));
        }
    }
    None
}

fn bad_run_integrity_exclusion_reason(value: &Value) -> Option<String> {
    if value
        .get("integrity")
        .and_then(|integrity| integrity.get("valid"))
        .and_then(Value::as_bool)
        == Some(false)
    {
        return Some(String::from("integrity-invalid bad run excluded from DPO"));
    }
    for answer in value
        .get("answer_files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        if answer
            .get("creation_actor")
            .and_then(Value::as_str)
            .is_some_and(|actor| actor != "model")
            || answer
                .get("last_modifying_actor")
                .and_then(Value::as_str)
                .is_some_and(|actor| actor != "model")
        {
            return Some(String::from("harness-assisted bad run excluded from DPO"));
        }
    }
    None
}

fn contains_forbidden_training_marker(value: &Value) -> bool {
    let serialized = value.to_string().to_lowercase();
    [
        "hidden benchmark answer",
        "hidden scoring label",
        "scorer-only target",
        "harness-injected",
        "83 / 83",
        "private benchmark answer",
    ]
    .iter()
    .any(|marker| serialized.contains(marker))
}

fn read_answer_files_for_receipt(
    receipt_path: &Path,
    value: &Value,
) -> Result<Vec<(String, String)>, String> {
    let run_dir = receipt_path.parent().unwrap_or_else(|| Path::new("."));
    let mut answer_files = Vec::new();
    for answer in value
        .get("answer_files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let Some(relative_path) = answer.get("relative_path").and_then(Value::as_str) else {
            continue;
        };
        let path = run_dir.join("output").join(relative_path);
        if !path.exists() {
            return Err(format!("missing answer file content at {}", path.display()));
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        answer_files.push((relative_path.to_string(), content));
    }
    Ok(answer_files)
}

fn looks_like_run_receipt(value: &Value) -> bool {
    value.get("run_spec").is_some()
        && value.get("answer_files").is_some()
        && value.get("integrity").is_some()
}

fn looks_like_bad_run(value: &Value) -> bool {
    value.get("failure_class").is_some()
        && value.get("full_prompt").is_some()
        && value.get("training_eligible").is_some()
}

fn run_id_from_receipt(value: &Value) -> Option<&str> {
    value
        .get("run_spec")
        .and_then(|run_spec| string_at(run_spec, "run_id"))
}

fn bad_run_id(value: &Value) -> Option<&str> {
    value.get("example_id").and_then(Value::as_str)
}

fn string_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn training_visibility(visibility: &str) -> String {
    match visibility {
        "public" | "public_training" => String::from("public_training"),
        "synthetic" | "synthetic_training" => String::from("synthetic_training"),
        "internal" | "internal_training" => String::from("internal_training"),
        _ => String::from("excluded"),
    }
}

fn canonical_failure_class(value: &str) -> String {
    value
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn system_message() -> LegalDpoMessage {
    LegalDpoMessage {
        role: String::from("system"),
        content: String::from(
            "You are a legal benchmark agent. Prefer source-grounded, concise answers that write required output files through tools, validate them, and submit only after the files exist.",
        ),
    }
}

fn pair_counts_by_failure_class(pairs: &[LegalDpoPreferencePair]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for pair in pairs {
        *counts.entry(pair.reason.clone()).or_insert(0) += 1;
    }
    counts
}

fn pair_counts_by_family(pairs: &[LegalDpoPreferencePair]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for pair in pairs {
        let family = pair
            .pair_id
            .rsplit_once('.')
            .and_then(|(prefix, _)| prefix.rsplit_once('.').map(|(_, family)| family))
            .unwrap_or("unknown");
        *counts.entry(family.to_string()).or_insert(0) += 1;
    }
    counts
}

fn dataset_hash(pairs: &[LegalDpoPreferencePair]) -> Result<String, LegalDpoDatasetBuilderError> {
    serde_json::to_vec(pairs)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|source| LegalDpoDatasetBuilderError::Json {
            path: PathBuf::from("<dpo_pairs>"),
            source,
        })
}

fn write_pairs(
    path: &Path,
    pairs: &[LegalDpoPreferencePair],
) -> Result<(), LegalDpoDatasetBuilderError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalDpoDatasetBuilderError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = fs::File::create(path).map_err(|source| LegalDpoDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    for pair in pairs {
        serde_json::to_writer(&mut file, pair).map_err(|source| {
            LegalDpoDatasetBuilderError::Json {
                path: path.to_path_buf(),
                source,
            }
        })?;
        file.write_all(b"\n")
            .map_err(|source| LegalDpoDatasetBuilderError::Io {
                path: path.to_path_buf(),
                source,
            })?;
    }
    Ok(())
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), LegalDpoDatasetBuilderError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalDpoDatasetBuilderError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|source| LegalDpoDatasetBuilderError::Json {
            path: path.to_path_buf(),
            source,
        })?,
    )
    .map_err(|source| LegalDpoDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json(path: &Path) -> Result<Value, LegalDpoDatasetBuilderError> {
    let bytes = fs::read(path).map_err(|source| LegalDpoDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| LegalDpoDatasetBuilderError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn collect_json_files(
    root: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), LegalDpoDatasetBuilderError> {
    for entry in fs::read_dir(root).map_err(|source| LegalDpoDatasetBuilderError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| LegalDpoDatasetBuilderError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(path.as_path(), output)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            output.push(path);
        }
    }
    Ok(())
}

fn receipt_ref(runs_root: &Path, path: &Path) -> String {
    path.strip_prefix(runs_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_good_run(root: &Path, run_id: &str, visibility: &str, integrity_valid: bool) {
        let run_dir = root.join("good").join(run_id);
        fs::create_dir_all(run_dir.join("output")).expect("output dir");
        fs::write(
            run_dir.join("output/memo.md"),
            "memo.md:\nThe answer uses the source and states the legal conclusion.",
        )
        .expect("answer");
        fs::write(
            run_dir.join("run_receipt.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "receipt_id": format!("legal.run_receipt.{run_id}"),
                "run_spec": {
                    "schema_version": 1,
                    "run_id": run_id,
                    "task_id": "task.dpo",
                    "task_version": "v1",
                    "benchmark_id": "harvey",
                    "benchmark_visibility": visibility
                },
                "answer_files": [{
                    "relative_path": "memo.md",
                    "creation_actor": "model",
                    "last_modifying_actor": "model",
                    "integrity_valid": integrity_valid
                }],
                "integrity": {
                    "valid": integrity_valid
                },
                "tool_calls": [{
                    "tool_call_id": "call.write.memo",
                    "tool_name": "write"
                }]
            }))
            .expect("json"),
        )
        .expect("receipt");
    }

    fn write_bad_run(root: &Path, run_id: &str) {
        let run_dir = root.join("bad").join(run_id);
        fs::create_dir_all(&run_dir).expect("bad dir");
        fs::write(
            run_dir.join("bad_run.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "example_id": format!("bad_run.{run_id}"),
                "full_prompt": "Write memo.md.",
                "full_model_response": "I answered in chat and did not write the file.",
                "tool_call_transcript": [],
                "required_file_paths": ["memo.md"],
                "failure_class": "did_not_write_required_file",
                "suggested_correction": "Write memo.md, validate it, then submit.",
                "training_eligible": true,
                "sft_eligible": false,
                "training_eligibility_reasons": []
            }))
            .expect("json"),
        )
        .expect("bad run");
    }

    #[test]
    fn legal_dpo_builder_generates_twenty_plus_pairs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let runs = temp.path().join("runs");
        write_good_run(&runs, "run.good", "public", true);
        write_bad_run(&runs, "run.bad");
        let out = temp.path().join("datasets/legal-dpo-v1.jsonl");
        let manifest = temp.path().join("datasets/legal-dpo-v1.manifest.json");
        let result = build_legal_benchmark_dpo_dataset(&LegalDpoDatasetBuilderConfig {
            runs_root: runs,
            out_jsonl: out.clone(),
            manifest_json: manifest,
            dataset_id: String::from("legal-dpo-v1"),
        })
        .expect("dpo dataset");

        assert!(result.pairs.len() >= 20);
        assert_eq!(
            result
                .manifest
                .pair_counts_by_failure_class
                .get("DidNotWriteRequiredFile"),
            Some(&result.pairs.len())
        );
        let loaded = load_legal_dpo_dataset(out).expect("load dpo");
        assert_eq!(loaded.len(), result.pairs.len());
    }

    #[test]
    fn legal_dpo_builder_rejects_hidden_and_integrity_invalid_runs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let runs = temp.path().join("runs");
        write_good_run(&runs, "run.hidden", "hidden", true);
        write_good_run(&runs, "run.invalid", "public", false);
        write_bad_run(&runs, "run.bad");
        let result = build_legal_benchmark_dpo_dataset(&LegalDpoDatasetBuilderConfig {
            runs_root: runs,
            out_jsonl: temp.path().join("datasets/legal-dpo-v1.jsonl"),
            manifest_json: temp.path().join("datasets/legal-dpo-v1.manifest.json"),
            dataset_id: String::from("legal-dpo-v1"),
        })
        .expect("dpo dataset");

        assert!(result.pairs.is_empty());
        assert!(
            result
                .manifest
                .excluded_inputs
                .iter()
                .any(|input| input.reason.contains("hidden"))
        );
        assert!(
            result
                .manifest
                .excluded_inputs
                .iter()
                .any(|input| input.reason.contains("integrity"))
        );
    }
}
