//! Harvey Labs compatibility loader for legal benchmark tasks.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    task_spec_digest, ArtifactKind, BenchmarkTaskSpec, CriterionKind, CriterionSpec,
    DataClassification, DeliverableKind, DeliverableSpec, JudgeMode, JudgePolicy, SourceArtifact,
    SourceCompatibility, ToolPolicy, LEGAL_BENCHMARK_SCHEMA_VERSION,
};

const HARVEY_UPSTREAM_SUITE: &str = "harvey_labs";

/// Harvey compatibility scan result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarveyCorpusScan {
    /// Corpus summary.
    pub summary: HarveyCorpusSummary,
    /// Normalized task specs.
    pub tasks: Vec<BenchmarkTaskSpec>,
}

/// Machine-readable summary for a Harvey compatibility scan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarveyCorpusSummary {
    /// Schema version for this report.
    pub schema_version: u16,
    /// Upstream suite name.
    pub upstream_suite: String,
    /// Upstream commit used as the task version.
    pub upstream_commit: String,
    /// Original tasks root path supplied by the caller.
    pub tasks_root: String,
    /// Number of discovered tasks.
    pub task_count: usize,
    /// Number of practice areas.
    pub practice_area_count: usize,
    /// Number of criteria.
    pub criterion_count: usize,
    /// Number of source documents.
    pub source_document_count: usize,
    /// Number of deliverables.
    pub deliverable_count: usize,
    /// Counts by practice area slug.
    pub practice_area_counts: BTreeMap<String, usize>,
    /// Counts by Harvey work type.
    pub work_type_counts: BTreeMap<String, usize>,
    /// Counts by source document extension.
    pub source_extension_counts: BTreeMap<String, usize>,
    /// Counts by deliverable extension.
    pub deliverable_extension_counts: BTreeMap<String, usize>,
    /// Per-task summaries.
    pub task_summaries: Vec<HarveyTaskSummary>,
}

/// Per-task summary emitted by the scanner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarveyTaskSummary {
    /// Owned task id.
    pub task_id: String,
    /// Upstream task path relative to the tasks root.
    pub upstream_task_path: String,
    /// Practice area slug.
    pub practice_area: String,
    /// Harvey work type.
    pub work_type: String,
    /// Number of criteria in the task.
    pub criterion_count: usize,
    /// Number of source documents in the task.
    pub source_document_count: usize,
    /// Number of deliverables in the task.
    pub deliverable_count: usize,
    /// Stable digest over the normalized task spec.
    pub task_spec_hash: String,
}

/// Errors returned by the Harvey compatibility loader.
#[derive(Debug, Error)]
pub enum HarveyLoaderError {
    /// File-system operation failed.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// JSON parsing failed.
    #[error("JSON error at {path}: {source}")]
    Json {
        /// JSON path.
        path: PathBuf,
        /// Source error.
        #[source]
        source: serde_json::Error,
    },
    /// The supplied tasks root does not exist or is not a directory.
    #[error("Harvey tasks root is not a directory: {0}")]
    InvalidTasksRoot(PathBuf),
    /// A task is structurally invalid.
    #[error("invalid Harvey task at {path}: {reason}")]
    InvalidTask {
        /// Task path.
        path: PathBuf,
        /// Reason.
        reason: String,
    },
    /// Relative path conversion failed.
    #[error("failed to compute relative path for {path} from root {root}")]
    RelativePath {
        /// Root path.
        root: PathBuf,
        /// Child path.
        path: PathBuf,
    },
    /// Digest calculation failed.
    #[error("failed to digest normalized task {task_id}: {source}")]
    Digest {
        /// Task id.
        task_id: String,
        /// Source error.
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct HarveyRawTask {
    title: String,
    work_type: String,
    tags: Vec<String>,
    instructions: String,
    deliverables: BTreeMap<String, String>,
    criteria: Vec<HarveyRawCriterion>,
}

#[derive(Clone, Debug, Deserialize)]
struct HarveyRawCriterion {
    id: String,
    title: String,
    deliverables: Vec<String>,
    match_criteria: String,
}

/// Scans a Harvey `tasks` directory and returns normalized task specs plus a
/// corpus summary.
pub fn scan_harvey_corpus(
    tasks_root: impl AsRef<Path>,
    upstream_commit: impl Into<String>,
) -> Result<HarveyCorpusScan, HarveyLoaderError> {
    let tasks_root = tasks_root.as_ref();
    let upstream_commit = upstream_commit.into();
    if !tasks_root.is_dir() {
        return Err(HarveyLoaderError::InvalidTasksRoot(
            tasks_root.to_path_buf(),
        ));
    }

    let task_json_paths = discover_task_json_paths(tasks_root)?;
    let mut tasks = Vec::with_capacity(task_json_paths.len());
    let mut task_summaries = Vec::with_capacity(task_json_paths.len());
    let mut practice_area_counts = BTreeMap::new();
    let mut work_type_counts = BTreeMap::new();
    let mut source_extension_counts = BTreeMap::new();
    let mut deliverable_extension_counts = BTreeMap::new();
    let mut total_criteria = 0usize;
    let mut total_source_documents = 0usize;
    let mut total_deliverables = 0usize;

    for task_json_path in task_json_paths {
        let task_dir = task_json_path
            .parent()
            .ok_or_else(|| HarveyLoaderError::InvalidTask {
                path: task_json_path.clone(),
                reason: String::from("task.json has no parent directory"),
            })?;
        let relative_task_json = relative_path(tasks_root, &task_json_path)?;
        let relative_task_dir = relative_path(tasks_root, task_dir)?;
        let practice_area = first_path_component(&relative_task_dir).ok_or_else(|| {
            HarveyLoaderError::InvalidTask {
                path: task_json_path.clone(),
                reason: String::from("task path has no practice area component"),
            }
        })?;

        let raw_task = read_harvey_raw_task(&task_json_path)?;
        validate_raw_task(&task_json_path, &raw_task)?;
        let source_artifacts = read_source_artifacts(task_dir, &relative_task_dir)?;
        if source_artifacts.is_empty() {
            return Err(HarveyLoaderError::InvalidTask {
                path: task_json_path.clone(),
                reason: String::from("documents/ exists but contains no files"),
            });
        }

        for source_artifact in &source_artifacts {
            *source_extension_counts
                .entry(file_extension_key(&source_artifact.relative_path))
                .or_insert(0) += 1;
        }

        let deliverables = normalize_deliverables(&raw_task);
        for deliverable in &deliverables {
            *deliverable_extension_counts
                .entry(file_extension_key(&deliverable.required_path))
                .or_insert(0) += 1;
        }

        let deliverable_ids_by_harvey_name = deliverable_id_map(&raw_task);
        let criteria = normalize_criteria(
            &task_json_path,
            &raw_task,
            &deliverable_ids_by_harvey_name,
            &source_artifacts,
        )?;
        let task_slug = task_slug_from_relative_dir(&relative_task_dir);
        let task_id = format!("harvey.{practice_area}.{task_slug}");

        let mut upstream_fields = BTreeMap::new();
        upstream_fields.insert(String::from("harvey_work_type"), json!(raw_task.work_type));
        upstream_fields.insert(String::from("harvey_tags"), json!(raw_task.tags));

        let task_spec = BenchmarkTaskSpec {
            schema_version: LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: task_id.clone(),
            task_version: upstream_commit.clone(),
            domain: String::from("legal"),
            practice_area: practice_area.clone(),
            workflow: raw_task.work_type.clone(),
            title: raw_task.title.clone(),
            instructions: raw_task.instructions.clone(),
            work_type: raw_task.work_type.clone(),
            tags: raw_task.tags.clone(),
            source_artifacts,
            deliverables,
            criteria,
            judge_policy: default_harvey_judge_policy(),
            tool_policy: default_harvey_tool_policy(),
            source_compatibility: Some(SourceCompatibility {
                upstream_suite: String::from(HARVEY_UPSTREAM_SUITE),
                upstream_commit: upstream_commit.clone(),
                upstream_task_path: relative_task_json.to_string_lossy().to_string(),
                upstream_fields,
            }),
            metadata: BTreeMap::new(),
        };
        let task_spec_hash =
            task_spec_digest(&task_spec).map_err(|source| HarveyLoaderError::Digest {
                task_id: task_id.clone(),
                source,
            })?;

        *practice_area_counts
            .entry(practice_area.clone())
            .or_insert(0) += 1;
        *work_type_counts
            .entry(raw_task.work_type.clone())
            .or_insert(0) += 1;

        total_criteria += task_spec.criteria.len();
        total_source_documents += task_spec.source_artifacts.len();
        total_deliverables += task_spec.deliverables.len();
        task_summaries.push(HarveyTaskSummary {
            task_id,
            upstream_task_path: relative_task_json.to_string_lossy().to_string(),
            practice_area,
            work_type: raw_task.work_type,
            criterion_count: task_spec.criteria.len(),
            source_document_count: task_spec.source_artifacts.len(),
            deliverable_count: task_spec.deliverables.len(),
            task_spec_hash,
        });
        tasks.push(task_spec);
    }

    Ok(HarveyCorpusScan {
        summary: HarveyCorpusSummary {
            schema_version: LEGAL_BENCHMARK_SCHEMA_VERSION,
            upstream_suite: String::from(HARVEY_UPSTREAM_SUITE),
            upstream_commit,
            tasks_root: tasks_root.to_string_lossy().to_string(),
            task_count: tasks.len(),
            practice_area_count: practice_area_counts.len(),
            criterion_count: total_criteria,
            source_document_count: total_source_documents,
            deliverable_count: total_deliverables,
            practice_area_counts,
            work_type_counts,
            source_extension_counts,
            deliverable_extension_counts,
            task_summaries,
        },
        tasks,
    })
}

fn discover_task_json_paths(tasks_root: &Path) -> Result<Vec<PathBuf>, HarveyLoaderError> {
    let mut discovered = Vec::new();
    discover_task_json_paths_inner(tasks_root, &mut discovered)?;
    discovered.sort();
    Ok(discovered)
}

fn discover_task_json_paths_inner(
    path: &Path,
    discovered: &mut Vec<PathBuf>,
) -> Result<(), HarveyLoaderError> {
    for entry in read_dir_sorted(path)? {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            discover_task_json_paths_inner(&entry_path, discovered)?;
        } else if entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "task.json")
        {
            discovered.push(entry_path);
        }
    }
    Ok(())
}

fn read_dir_sorted(path: &Path) -> Result<Vec<fs::DirEntry>, HarveyLoaderError> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| HarveyLoaderError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| HarveyLoaderError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());
    Ok(entries)
}

fn read_harvey_raw_task(path: &Path) -> Result<HarveyRawTask, HarveyLoaderError> {
    let bytes = fs::read(path).map_err(|source| HarveyLoaderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| HarveyLoaderError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_raw_task(path: &Path, task: &HarveyRawTask) -> Result<(), HarveyLoaderError> {
    if task.title.trim().is_empty() {
        return invalid_task(path, "title is empty");
    }
    if task.instructions.trim().is_empty() {
        return invalid_task(path, "instructions are empty");
    }
    if task.work_type.trim().is_empty() {
        return invalid_task(path, "work_type is empty");
    }
    if task.deliverables.is_empty() {
        return invalid_task(path, "deliverables are empty");
    }
    if task.criteria.is_empty() {
        return invalid_task(path, "criteria are empty");
    }
    let deliverables = task.deliverables.keys().cloned().collect::<BTreeSet<_>>();
    for criterion in &task.criteria {
        if criterion.id.trim().is_empty() {
            return invalid_task(path, "criterion id is empty");
        }
        if criterion.match_criteria.trim().is_empty() {
            return invalid_task(path, "criterion match_criteria is empty");
        }
        if criterion.deliverables.is_empty() {
            return invalid_task(path, "criterion has no deliverable refs");
        }
        for deliverable_ref in &criterion.deliverables {
            if !deliverables.contains(deliverable_ref) {
                return invalid_task(
                    path,
                    format!(
                        "criterion {} references missing deliverable {deliverable_ref}",
                        criterion.id
                    ),
                );
            }
        }
    }
    Ok(())
}

fn invalid_task<T>(path: &Path, reason: impl Into<String>) -> Result<T, HarveyLoaderError> {
    Err(HarveyLoaderError::InvalidTask {
        path: path.to_path_buf(),
        reason: reason.into(),
    })
}

fn read_source_artifacts(
    task_dir: &Path,
    relative_task_dir: &Path,
) -> Result<Vec<SourceArtifact>, HarveyLoaderError> {
    let documents_dir = task_dir.join("documents");
    if !documents_dir.is_dir() {
        return Err(HarveyLoaderError::InvalidTask {
            path: task_dir.join("task.json"),
            reason: String::from("missing documents/ directory"),
        });
    }
    let mut document_paths = Vec::new();
    discover_files(&documents_dir, &mut document_paths)?;
    document_paths.sort();

    let mut artifacts = Vec::with_capacity(document_paths.len());
    for document_path in document_paths {
        let relative_document_path = relative_path(task_dir, &document_path)?;
        let relative_to_tasks_root = relative_task_dir.join(&relative_document_path);
        let bytes = fs::read(&document_path).map_err(|source| HarveyLoaderError::Io {
            path: document_path.clone(),
            source,
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha256 = hex::encode(hasher.finalize());
        let original_filename = document_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document")
            .to_owned();
        let relative_path_string = relative_to_tasks_root.to_string_lossy().to_string();
        artifacts.push(SourceArtifact {
            artifact_id: format!(
                "artifact.source.{}",
                artifact_id_fragment(&relative_path_string)
            ),
            artifact_kind: ArtifactKind::SourceDocument,
            relative_path: relative_path_string.clone(),
            original_filename,
            media_type: media_type_for_path(&relative_path_string).to_owned(),
            byte_size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha256,
            data_classification: DataClassification::PublicReference,
            provenance: Some(String::from(HARVEY_UPSTREAM_SUITE)),
        });
    }
    Ok(artifacts)
}

fn discover_files(path: &Path, discovered: &mut Vec<PathBuf>) -> Result<(), HarveyLoaderError> {
    for entry in read_dir_sorted(path)? {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            discover_files(&entry_path, discovered)?;
        } else if entry_path.is_file() {
            discovered.push(entry_path);
        }
    }
    Ok(())
}

fn normalize_deliverables(task: &HarveyRawTask) -> Vec<DeliverableSpec> {
    task.deliverables
        .iter()
        .map(|(deliverable_name, deliverable_path)| DeliverableSpec {
            deliverable_id: deliverable_id(deliverable_name),
            deliverable_kind: deliverable_kind_for_path(deliverable_path),
            required_path: format!("outputs/{deliverable_path}"),
            description: deliverable_name.clone(),
            required: true,
        })
        .collect()
}

fn normalize_criteria(
    task_json_path: &Path,
    task: &HarveyRawTask,
    deliverable_ids_by_harvey_name: &BTreeMap<String, String>,
    source_artifacts: &[SourceArtifact],
) -> Result<Vec<CriterionSpec>, HarveyLoaderError> {
    let source_artifact_ids = source_artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    task.criteria
        .iter()
        .map(|criterion| {
            let deliverable_ids = criterion
                .deliverables
                .iter()
                .map(|deliverable| {
                    deliverable_ids_by_harvey_name
                        .get(deliverable)
                        .cloned()
                        .ok_or_else(|| HarveyLoaderError::InvalidTask {
                            path: task_json_path.to_path_buf(),
                            reason: format!(
                                "criterion {} references missing deliverable {deliverable}",
                                criterion.id
                            ),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CriterionSpec {
                criterion_id: format!("criterion.{}", slugify(&criterion.id)),
                criterion_kind: infer_criterion_kind(&criterion.title, &criterion.match_criteria),
                description: format!("{}\n\n{}", criterion.title, criterion.match_criteria),
                weight_bps: None,
                deliverable_ids,
                source_artifact_ids: source_artifact_ids.clone(),
            })
        })
        .collect()
}

fn deliverable_id_map(task: &HarveyRawTask) -> BTreeMap<String, String> {
    task.deliverables
        .keys()
        .map(|deliverable_name| (deliverable_name.clone(), deliverable_id(deliverable_name)))
        .collect()
}

fn default_harvey_judge_policy() -> JudgePolicy {
    JudgePolicy {
        mode: JudgeMode::Llm,
        provider: String::from("provider_neutral"),
        model: String::from("judge_configured_at_run_time"),
        prompt_template_id: String::from("judge.harvey_criterion_compatibility.v1"),
        prompt_template_hash: String::from("pending_runner_configuration"),
        all_pass_required: true,
        sample_count: 1,
    }
}

fn default_harvey_tool_policy() -> ToolPolicy {
    ToolPolicy {
        allowed_tools: vec![
            String::from("shell"),
            String::from("read"),
            String::from("write"),
            String::from("edit"),
            String::from("glob"),
            String::from("grep"),
            String::from("inventory"),
            String::from("email_summary"),
            String::from("spreadsheet_summary"),
            String::from("pdf_search"),
            String::from("evidence_table"),
            String::from("validate_deliverables"),
        ],
        network_allowed: false,
        source_artifacts_read_only: true,
        max_turns: 128,
        max_wall_time_seconds: 3600,
    }
}

fn first_path_component(path: &Path) -> Option<String> {
    path.components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
}

fn relative_path(root: &Path, path: &Path) -> Result<PathBuf, HarveyLoaderError> {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| HarveyLoaderError::RelativePath {
            root: root.to_path_buf(),
            path: path.to_path_buf(),
        })
}

fn task_slug_from_relative_dir(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(slugify)
        .unwrap_or_else(|| String::from("task"))
}

fn deliverable_id(name: &str) -> String {
    format!("deliverable.{}", slugify(name))
}

fn infer_criterion_kind(title: &str, match_criteria: &str) -> CriterionKind {
    let haystack = format!("{title} {match_criteria}").to_ascii_lowercase();
    if haystack.contains("format") || haystack.contains("structure") {
        CriterionKind::Formatting
    } else if haystack.contains("cite")
        || haystack.contains("citation")
        || haystack.contains("evidence")
    {
        CriterionKind::CitationEvidence
    } else if haystack.contains("legal") || haystack.contains("clause") {
        CriterionKind::LegalReasoning
    } else if haystack.contains("accurate")
        || haystack.contains("identifies")
        || haystack.contains("notes")
    {
        CriterionKind::FactualAccuracy
    } else if haystack.contains("deliverable") || haystack.contains("output") {
        CriterionKind::DeliverableValidation
    } else {
        CriterionKind::Completeness
    }
}

fn deliverable_kind_for_path(path: &str) -> DeliverableKind {
    match file_extension_key(path).as_str() {
        "docx" => DeliverableKind::Docx,
        "xlsx" => DeliverableKind::Xlsx,
        "pdf" => DeliverableKind::Pdf,
        "json" => DeliverableKind::Json,
        "md" | "markdown" => DeliverableKind::Markdown,
        "txt" => DeliverableKind::Text,
        _ => DeliverableKind::Other,
    }
}

fn media_type_for_path(path: &str) -> &'static str {
    match file_extension_key(path).as_str() {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "eml" => "message/rfc822",
        "json" => "application/json",
        "md" | "markdown" => "text/markdown",
        "pdf" => "application/pdf",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" => "text/plain",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

fn file_extension_key(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
        .unwrap_or_else(|| String::from("none"))
}

fn artifact_id_fragment(value: &str) -> String {
    slugify(value).replace('_', ".")
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            slug.push('_');
            previous_was_separator = true;
        }
    }
    slug.trim_matches('_').to_owned()
}

/// Converts a scan into summary-only JSON value for CLI/reporting use.
pub fn harvey_corpus_summary_json(scan: &HarveyCorpusScan) -> Value {
    json!(&scan.summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_harvey_sample_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/legal_benchmark/harvey_compatibility_sample/tasks");
        let scan = scan_harvey_corpus(root, "fixturecommit").expect("scan sample fixture");

        assert_eq!(scan.summary.task_count, 1);
        assert_eq!(scan.summary.practice_area_count, 1);
        assert_eq!(scan.summary.criterion_count, 2);
        assert_eq!(scan.summary.source_document_count, 2);
        assert_eq!(scan.summary.deliverable_count, 1);
        assert_eq!(
            scan.summary.practice_area_counts.get("corporate"),
            Some(&1usize)
        );
        assert_eq!(scan.summary.work_type_counts.get("review"), Some(&1usize));
        assert_eq!(
            scan.summary.source_extension_counts.get("txt"),
            Some(&1usize)
        );
        assert_eq!(
            scan.summary.source_extension_counts.get("eml"),
            Some(&1usize)
        );
        assert_eq!(
            scan.summary.deliverable_extension_counts.get("docx"),
            Some(&1usize)
        );

        let task = scan.tasks.first().expect("one normalized task");
        assert_eq!(task.task_id, "harvey.corporate.sample_contract_review");
        assert_eq!(task.task_version, "fixturecommit");
        assert_eq!(task.criteria.len(), 2);
        assert_eq!(task.source_artifacts.len(), 2);
        assert!(task
            .tool_policy
            .allowed_tools
            .contains(&String::from("inventory")));
        assert!(task
            .tool_policy
            .allowed_tools
            .contains(&String::from("evidence_table")));
        assert!(task
            .tool_policy
            .allowed_tools
            .contains(&String::from("validate_deliverables")));
        assert_eq!(
            task.deliverables[0].deliverable_id,
            "deliverable.risk_memo_docx"
        );
        assert!(task.source_compatibility.is_some());
        assert_eq!(scan.summary.task_summaries[0].task_spec_hash.len(), 64);
    }

    #[test]
    fn fails_on_missing_documents_directory() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/legal_benchmark/harvey_compatibility_invalid/missing_documents/tasks",
        );
        let error = scan_harvey_corpus(root, "fixturecommit")
            .expect_err("missing documents directory fails");
        assert!(error.to_string().contains("missing documents"));
    }

    #[test]
    fn fails_on_missing_deliverable_reference() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/legal_benchmark/harvey_compatibility_invalid/missing_deliverable_ref/tasks",
        );
        let error =
            scan_harvey_corpus(root, "fixturecommit").expect_err("missing deliverable ref fails");
        assert!(error.to_string().contains("missing deliverable"));
    }

    #[test]
    fn fails_on_missing_criteria() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/legal_benchmark/harvey_compatibility_invalid/missing_criteria/tasks",
        );
        let error = scan_harvey_corpus(root, "fixturecommit").expect_err("missing criteria fails");
        assert!(error.to_string().contains("criteria"));
    }

    #[test]
    fn fails_on_malformed_json() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/legal_benchmark/harvey_compatibility_invalid/malformed_json/tasks",
        );
        let error = scan_harvey_corpus(root, "fixturecommit").expect_err("malformed JSON fails");
        assert!(error.to_string().contains("JSON error"));
    }
}
