use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const LEGAL_SFT_DATASET_SCHEMA_VERSION: &str = "legal_sft_v1";
pub const LEGAL_SFT_DATASET_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.sft_dataset_manifest.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalSftMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalSftAnswerFile {
    pub relative_path: String,
    pub content: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalSftDatasetExample {
    pub schema_version: String,
    pub example_id: String,
    pub source_run_ids: Vec<String>,
    pub visibility: String,
    pub reasoning_mode: String,
    pub base_task_id: String,
    pub messages: Vec<LegalSftMessage>,
    pub tool_trace: Vec<Value>,
    pub answer_files: Vec<LegalSftAnswerFile>,
    pub training_tags: Vec<String>,
    pub exclusion_flags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalSftExcludedInput {
    pub source_path: String,
    pub source_run_id: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalSftDatasetManifest {
    pub schema_version: String,
    pub dataset_id: String,
    pub output_jsonl: String,
    pub included_count: usize,
    pub excluded_count: usize,
    pub family_counts: BTreeMap<String, usize>,
    pub source_receipts: Vec<String>,
    pub excluded_inputs: Vec<LegalSftExcludedInput>,
    pub dataset_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalSftDatasetBuilderConfig {
    pub runs_root: PathBuf,
    pub out_jsonl: PathBuf,
    pub manifest_json: PathBuf,
    pub dataset_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegalSftDatasetBuildResult {
    pub examples: Vec<LegalSftDatasetExample>,
    pub manifest: LegalSftDatasetManifest,
}

#[derive(Debug, Error)]
pub enum LegalSftDatasetBuilderError {
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

pub fn build_legal_benchmark_sft_dataset(
    config: &LegalSftDatasetBuilderConfig,
) -> Result<LegalSftDatasetBuildResult, LegalSftDatasetBuilderError> {
    if !config.runs_root.exists() {
        return Err(LegalSftDatasetBuilderError::InvalidArgument(format!(
            "runs root does not exist: {}",
            config.runs_root.display()
        )));
    }
    let mut json_paths = Vec::new();
    collect_json_files(config.runs_root.as_path(), &mut json_paths)?;
    json_paths.sort();

    let mut examples = Vec::new();
    let mut excluded_inputs = Vec::new();
    let mut source_receipts = BTreeSet::new();

    for path in json_paths {
        let value = read_json(path.as_path())?;
        if looks_like_run_receipt(&value) {
            process_good_run_receipt(
                config.runs_root.as_path(),
                path.as_path(),
                &value,
                &mut examples,
                &mut excluded_inputs,
                &mut source_receipts,
            );
        } else if looks_like_bad_run(&value) {
            process_bad_run(
                path.as_path(),
                &value,
                &mut examples,
                &mut excluded_inputs,
                &mut source_receipts,
            );
        }
    }

    examples.sort_by(|left, right| left.example_id.cmp(&right.example_id));
    write_examples(config.out_jsonl.as_path(), &examples)?;
    let family_counts = family_counts(&examples);
    let dataset_hash = dataset_hash(&examples)?;
    let manifest = LegalSftDatasetManifest {
        schema_version: String::from(LEGAL_SFT_DATASET_MANIFEST_SCHEMA_VERSION),
        dataset_id: config.dataset_id.clone(),
        output_jsonl: config.out_jsonl.display().to_string(),
        included_count: examples.len(),
        excluded_count: excluded_inputs.len(),
        family_counts,
        source_receipts: source_receipts.into_iter().collect(),
        excluded_inputs,
        dataset_hash,
    };
    write_json(config.manifest_json.as_path(), &manifest)?;
    Ok(LegalSftDatasetBuildResult { examples, manifest })
}

fn process_good_run_receipt(
    runs_root: &Path,
    path: &Path,
    value: &Value,
    examples: &mut Vec<LegalSftDatasetExample>,
    excluded_inputs: &mut Vec<LegalSftExcludedInput>,
    source_receipts: &mut BTreeSet<String>,
) {
    let run_spec = value.get("run_spec").unwrap_or(&Value::Null);
    let run_id = string_at(run_spec, "run_id").unwrap_or("unknown_run");
    let task_id = string_at(run_spec, "task_id").unwrap_or("unknown_task");
    let visibility = string_at(run_spec, "benchmark_visibility").unwrap_or("unknown");
    let source_path = path.display().to_string();
    let exclude = good_run_exclusion_reason(value, visibility);
    if let Some(reason) = exclude {
        excluded_inputs.push(LegalSftExcludedInput {
            source_path,
            source_run_id: Some(run_id.to_string()),
            reason,
        });
        return;
    }

    let answer_files = match read_answer_files_for_receipt(path, value) {
        Ok(files) if !files.is_empty() => files,
        Ok(_) => {
            excluded_inputs.push(LegalSftExcludedInput {
                source_path,
                source_run_id: Some(run_id.to_string()),
                reason: String::from("missing answer file content for SFT"),
            });
            return;
        }
        Err(reason) => {
            excluded_inputs.push(LegalSftExcludedInput {
                source_path,
                source_run_id: Some(run_id.to_string()),
                reason,
            });
            return;
        }
    };

    source_receipts.insert(receipt_ref(runs_root, path));
    let required_path = answer_files
        .first()
        .map(|answer| answer.relative_path.as_str())
        .unwrap_or("memo.md");
    let answer_text = answer_files
        .iter()
        .map(|answer| format!("{}:\n{}", answer.relative_path, answer.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let base_user = format!(
        "Task {task_id}: inspect the source materials, write the required legal answer file at `{required_path}`, verify it, and submit."
    );
    let tool_trace = value
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    push_example(
        examples,
        LegalSftDatasetExample {
            schema_version: String::from(LEGAL_SFT_DATASET_SCHEMA_VERSION),
            example_id: format!("{run_id}.golden_workflow"),
            source_run_ids: vec![run_id.to_string()],
            visibility: training_visibility(visibility),
            reasoning_mode: String::from("direct_answer"),
            base_task_id: task_id.to_string(),
            messages: vec![
                system_message(),
                LegalSftMessage {
                    role: String::from("user"),
                    content: base_user.clone(),
                },
                LegalSftMessage {
                    role: String::from("assistant"),
                    content: format!(
                        "I will read the task, use the available sources, write `{required_path}`, validate the file, and submit it.\n\n{answer_text}"
                    ),
                },
            ],
            tool_trace: tool_trace.clone(),
            answer_files: answer_files.clone(),
            training_tags: vec![String::from("golden_workflow")],
            exclusion_flags: Vec::new(),
        },
    );
    push_example(
        examples,
        LegalSftDatasetExample {
            schema_version: String::from(LEGAL_SFT_DATASET_SCHEMA_VERSION),
            example_id: format!("{run_id}.source_grounded_answer"),
            source_run_ids: vec![run_id.to_string()],
            visibility: training_visibility(visibility),
            reasoning_mode: String::from("direct_answer"),
            base_task_id: task_id.to_string(),
            messages: vec![
                system_message(),
                LegalSftMessage {
                    role: String::from("user"),
                    content: format!(
                        "Draft the legal work product for task {task_id}. Use only the source material and write `{required_path}`."
                    ),
                },
                LegalSftMessage {
                    role: String::from("assistant"),
                    content: answer_text.clone(),
                },
            ],
            tool_trace: tool_trace.clone(),
            answer_files: answer_files.clone(),
            training_tags: vec![String::from("source_grounded_answer")],
            exclusion_flags: Vec::new(),
        },
    );
    push_example(
        examples,
        LegalSftDatasetExample {
            schema_version: String::from(LEGAL_SFT_DATASET_SCHEMA_VERSION),
            example_id: format!("{run_id}.tool_discipline"),
            source_run_ids: vec![run_id.to_string()],
            visibility: training_visibility(visibility),
            reasoning_mode: String::from("direct_answer"),
            base_task_id: task_id.to_string(),
            messages: vec![
                system_message(),
                LegalSftMessage {
                    role: String::from("user"),
                    content: format!(
                        "Show the correct tool discipline for writing and checking `{required_path}`."
                    ),
                },
                LegalSftMessage {
                    role: String::from("assistant"),
                    content: format!(
                        "Use the write tool with root `output` and relative_path `{required_path}`. After writing, validate that `{required_path}` exists before submitting."
                    ),
                },
            ],
            tool_trace: tool_trace.clone(),
            answer_files: answer_files.clone(),
            training_tags: vec![String::from("tool_discipline")],
            exclusion_flags: Vec::new(),
        },
    );
    if answer_text.len() <= 4_000 {
        push_example(
            examples,
            LegalSftDatasetExample {
                schema_version: String::from(LEGAL_SFT_DATASET_SCHEMA_VERSION),
                example_id: format!("{run_id}.minimal_answer"),
                source_run_ids: vec![run_id.to_string()],
                visibility: training_visibility(visibility),
                reasoning_mode: String::from("direct_answer"),
                base_task_id: task_id.to_string(),
                messages: vec![
                    system_message(),
                    LegalSftMessage {
                        role: String::from("user"),
                        content: format!(
                            "Give a concise legal answer and write it to `{required_path}`."
                        ),
                    },
                    LegalSftMessage {
                        role: String::from("assistant"),
                        content: answer_text,
                    },
                ],
                tool_trace,
                answer_files,
                training_tags: vec![String::from("minimal_answer")],
                exclusion_flags: Vec::new(),
            },
        );
    }
}

fn process_bad_run(
    path: &Path,
    value: &Value,
    examples: &mut Vec<LegalSftDatasetExample>,
    excluded_inputs: &mut Vec<LegalSftExcludedInput>,
    source_receipts: &mut BTreeSet<String>,
) {
    let run_id = value
        .get("example_id")
        .and_then(Value::as_str)
        .unwrap_or("bad_run.unknown")
        .trim_start_matches("bad_run.");
    let training_eligible = value
        .get("training_eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let source_path = path.display().to_string();
    if !training_eligible {
        excluded_inputs.push(LegalSftExcludedInput {
            source_path,
            source_run_id: Some(run_id.to_string()),
            reason: value
                .get("training_eligibility_reasons")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| String::from("bad run is not training eligible")),
        });
        return;
    }
    source_receipts.insert(path.display().to_string());
    let required_path = value
        .get("required_file_paths")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .unwrap_or("memo.md");
    let failure_class = string_at(value, "failure_class").unwrap_or("other");
    let bad_response = string_at(value, "full_model_response").unwrap_or("");
    let correction = string_at(value, "suggested_correction")
        .unwrap_or("Write the required answer file, verify it exists, then submit.");
    push_example(
        examples,
        LegalSftDatasetExample {
            schema_version: String::from(LEGAL_SFT_DATASET_SCHEMA_VERSION),
            example_id: format!("{run_id}.failure_correction"),
            source_run_ids: vec![run_id.to_string()],
            visibility: String::from("public_training"),
            reasoning_mode: String::from("direct_answer"),
            base_task_id: string_at(value, "base_task_id")
                .unwrap_or("unknown_task")
                .to_string(),
            messages: vec![
                system_message(),
                LegalSftMessage {
                    role: String::from("user"),
                    content: format!(
                        "The prior trajectory failed as `{failure_class}`. Bad response summary:\n{bad_response}\n\nCorrect the behavior for required path `{required_path}`."
                    ),
                },
                LegalSftMessage {
                    role: String::from("assistant"),
                    content: format!(
                        "{correction}\n\nCorrect trajectory: write a complete answer to `{required_path}`, validate that file, then submit only after validation passes."
                    ),
                },
            ],
            tool_trace: value
                .get("tool_call_transcript")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            answer_files: Vec::new(),
            training_tags: vec![String::from("failure_correction")],
            exclusion_flags: Vec::new(),
        },
    );
}

fn good_run_exclusion_reason(value: &Value, visibility: &str) -> Option<String> {
    match visibility {
        "public" | "synthetic" | "internal" => {}
        "private" => {
            return Some(String::from(
                "private benchmark receipt is audit-only by default",
            ));
        }
        "hidden" => return Some(String::from("hidden benchmark receipt is not trainable")),
        _ => return Some(String::from("unknown benchmark visibility")),
    }
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
    }
    let serialized = value.to_string().to_lowercase();
    for forbidden in [
        "hidden benchmark answer",
        "hidden scoring label",
        "scorer-only target",
        "harness-injected",
        "83 / 83",
    ] {
        if serialized.contains(forbidden) {
            return Some(format!("forbidden training marker: {forbidden}"));
        }
    }
    None
}

fn read_answer_files_for_receipt(
    receipt_path: &Path,
    value: &Value,
) -> Result<Vec<LegalSftAnswerFile>, String> {
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
        answer_files.push(LegalSftAnswerFile {
            relative_path: relative_path.to_string(),
            content_hash: sha256_hex(content.as_bytes()),
            content,
        });
    }
    Ok(answer_files)
}

fn push_example(examples: &mut Vec<LegalSftDatasetExample>, example: LegalSftDatasetExample) {
    examples.push(example);
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

fn system_message() -> LegalSftMessage {
    LegalSftMessage {
        role: String::from("system"),
        content: String::from(
            "You are a legal benchmark agent. Use the provided source material, write required output files through tools, verify the files, and submit only after the required work product exists.",
        ),
    }
}

fn training_visibility(visibility: &str) -> String {
    match visibility {
        "public" => String::from("public_training"),
        "synthetic" => String::from("synthetic_training"),
        "internal" => String::from("internal_training"),
        _ => String::from("excluded"),
    }
}

fn string_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn family_counts(examples: &[LegalSftDatasetExample]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for example in examples {
        for tag in &example.training_tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn dataset_hash(
    examples: &[LegalSftDatasetExample],
) -> Result<String, LegalSftDatasetBuilderError> {
    serde_json::to_vec(examples)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|source| LegalSftDatasetBuilderError::Json {
            path: PathBuf::from("<dataset_examples>"),
            source,
        })
}

fn receipt_ref(runs_root: &Path, path: &Path) -> String {
    path.strip_prefix(runs_root)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn collect_json_files(
    root: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), LegalSftDatasetBuilderError> {
    for entry in fs::read_dir(root).map_err(|source| LegalSftDatasetBuilderError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| LegalSftDatasetBuilderError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| LegalSftDatasetBuilderError::Io {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_json_files(path.as_path(), paths)?;
        } else if file_type.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, LegalSftDatasetBuilderError> {
    let bytes = fs::read(path).map_err(|source| LegalSftDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| LegalSftDatasetBuilderError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn write_examples(
    path: &Path,
    examples: &[LegalSftDatasetExample],
) -> Result<(), LegalSftDatasetBuilderError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalSftDatasetBuilderError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = fs::File::create(path).map_err(|source| LegalSftDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    for example in examples {
        serde_json::to_writer(&mut file, example).map_err(|source| {
            LegalSftDatasetBuilderError::Json {
                path: path.to_path_buf(),
                source,
            }
        })?;
        file.write_all(b"\n")
            .map_err(|source| LegalSftDatasetBuilderError::Io {
                path: path.to_path_buf(),
                source,
            })?;
    }
    Ok(())
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), LegalSftDatasetBuilderError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalSftDatasetBuilderError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|source| LegalSftDatasetBuilderError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    fs::write(path, bytes).map_err(|source| LegalSftDatasetBuilderError::Io {
        path: path.to_path_buf(),
        source,
    })
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
    use tempfile::tempdir;

    fn write_good_run(root: &Path, run_id: &str) {
        let run_dir = root.join(run_id);
        fs::create_dir_all(run_dir.join("output")).expect("dirs");
        fs::write(
            run_dir.join("output/memo.md"),
            "# Memo\n\nUse the source.\n",
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
                    "task_id": "task.good",
                    "task_version": "v1",
                    "benchmark_id": "harvey",
                    "benchmark_visibility": "public",
                    "base_model_id": "qwen",
                    "tokenizer_id": "qwen-tokenizer",
                    "tokenizer_hash": {"algorithm": "sha256", "value": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    "prompt_template_id": "legal.autopilot.v1",
                    "prompt_template_hash": {"algorithm": "sha256", "value": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    "thinking_mode": "disabled",
                    "tool_list": ["write", "validate_deliverables"],
                    "source_document_hashes": [{"algorithm": "sha256", "value": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
                    "git_commit": "abcdef1",
                    "git_dirty": false,
                    "deterministic_replay_command": ["cargo", "run"]
                },
                "answer_files": [{
                    "schema_version": 1,
                    "relative_path": "memo.md",
                    "byte_size": 24,
                    "content_hash": {"algorithm": "sha256", "value": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
                    "creation_actor": "model",
                    "last_modifying_actor": "model",
                    "integrity_valid": true
                }],
                "integrity": {
                    "schema_version": 1,
                    "valid": true,
                    "integrity_report_hash": {"algorithm": "sha256", "value": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    "invalid_reasons": []
                },
                "tool_calls": [{"tool_call_id": "call.write.memo", "tool_name": "write"}]
            }))
            .expect("json"),
        )
        .expect("receipt");
    }

    fn write_bad_run(root: &Path, run_id: &str) {
        let run_dir = root.join(run_id);
        fs::create_dir_all(&run_dir).expect("dirs");
        fs::write(
            run_dir.join("bad_run.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "example_id": format!("bad_run.{run_id}"),
                "full_prompt": "Write memo.md.",
                "full_model_response": "I stopped before writing the file.",
                "tool_call_transcript": [],
                "required_file_paths": ["memo.md"],
                "failure_class": "did_not_write_required_file",
                "suggested_correction": "Write memo.md with a concise legal answer, validate it, then submit.",
                "training_eligible": true,
                "sft_eligible": false,
                "training_eligibility_reasons": []
            }))
            .expect("json"),
        )
        .expect("bad");
    }

    fn write_invalid_run(root: &Path, run_id: &str) {
        let run_dir = root.join(run_id);
        fs::create_dir_all(run_dir.join("output")).expect("dirs");
        fs::write(run_dir.join("output/memo.md"), "# Memo\n\nC-001 C-002\n").expect("answer");
        fs::write(
            run_dir.join("run_receipt.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "receipt_id": format!("legal.run_receipt.{run_id}"),
                "run_spec": {
                    "run_id": run_id,
                    "task_id": "task.invalid",
                    "benchmark_visibility": "public"
                },
                "answer_files": [{
                    "relative_path": "memo.md",
                    "last_modifying_actor": "harness",
                    "integrity_valid": false
                }],
                "integrity": {
                    "valid": false,
                    "invalid_reasons": ["harness-injected text produced invalid 83 / 83 style run"]
                }
            }))
            .expect("json"),
        )
        .expect("invalid");
    }

    #[test]
    fn legal_sft_dataset_builder_produces_good_and_correction_examples() {
        let temp = tempdir().expect("tempdir");
        let runs = temp.path().join("runs");
        write_good_run(&runs, "run.good");
        write_bad_run(&runs, "run.bad");
        let out = temp.path().join("datasets/legal-sft-v1.jsonl");
        let manifest = temp.path().join("datasets/legal-sft-v1.manifest.json");
        let result = build_legal_benchmark_sft_dataset(&LegalSftDatasetBuilderConfig {
            runs_root: runs,
            out_jsonl: out.clone(),
            manifest_json: manifest.clone(),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("build");

        assert!(out.exists());
        assert!(manifest.exists());
        assert!(
            result
                .examples
                .iter()
                .any(|example| example.training_tags == ["golden_workflow"])
        );
        assert!(
            result
                .examples
                .iter()
                .any(|example| example.training_tags == ["failure_correction"])
        );
        assert_eq!(result.manifest.included_count, result.examples.len());
    }

    #[test]
    fn legal_sft_dataset_builder_refuses_invalid_83_of_83_style_run() {
        let temp = tempdir().expect("tempdir");
        let runs = temp.path().join("runs");
        write_invalid_run(&runs, "run.invalid");
        let result = build_legal_benchmark_sft_dataset(&LegalSftDatasetBuilderConfig {
            runs_root: runs,
            out_jsonl: temp.path().join("out.jsonl"),
            manifest_json: temp.path().join("manifest.json"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("build");

        assert!(result.examples.is_empty());
        assert_eq!(result.manifest.excluded_count, 1);
        assert!(
            result.manifest.excluded_inputs[0]
                .reason
                .contains("integrity")
        );
    }
}
