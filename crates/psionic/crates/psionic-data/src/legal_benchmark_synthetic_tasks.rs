use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_legal_benchmark_dpo_dataset, build_legal_benchmark_sft_dataset,
    LegalDpoDatasetBuilderConfig, LegalDpoDatasetBuilderError, LegalSftDatasetBuilderConfig,
    LegalSftDatasetBuilderError,
};

pub const LEGAL_SYNTHETIC_WORKFLOW_GENERATOR_ID: &str =
    "psionic.legal_benchmark.synthetic_workflow_generator.v1";
pub const LEGAL_SYNTHETIC_WORKFLOW_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.synthetic_workflow_manifest.v1";
pub const LEGAL_SYNTHETIC_WORKFLOW_RUBRIC_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.synthetic_workflow_rubric.v1";
const DEFAULT_DPO_SAMPLE_TASK_COUNT: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalSyntheticWorkflowTaskGeneratorConfig {
    pub count: usize,
    pub out_dir: PathBuf,
    pub suite_id: String,
    pub seed: u64,
    pub base_model: String,
}

impl LegalSyntheticWorkflowTaskGeneratorConfig {
    #[must_use]
    pub fn new(count: usize, out_dir: impl Into<PathBuf>) -> Self {
        Self {
            count,
            out_dir: out_dir.into(),
            suite_id: String::from("synthetic_legal_workflow_v1"),
            seed: 20260520,
            base_model: String::from("synthetic-base-workflow-policy-v1"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalSyntheticWorkflowTaskManifestRow {
    pub task_id: String,
    pub task_type: String,
    pub practice_area: String,
    pub work_type: String,
    pub title: String,
    pub task_json_path: String,
    pub rubric_path: String,
    pub source_document_paths: Vec<String>,
    pub expected_answer_hash: String,
    pub base_run_id: String,
    pub base_outcome: String,
    pub training_tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalSyntheticWorkflowManifest {
    pub schema_version: String,
    pub suite_id: String,
    pub generator_id: String,
    pub seed: u64,
    pub synthetic: bool,
    pub source_policy: String,
    pub claim_boundary: String,
    pub task_count: usize,
    pub task_type_counts: BTreeMap<String, usize>,
    pub successful_base_run_count: usize,
    pub failed_base_run_count: usize,
    pub sft_example_count: usize,
    pub dpo_pair_count: usize,
    pub tasks_root: String,
    pub rubrics_root: String,
    pub runs_root: String,
    pub dpo_sample_runs_root: String,
    pub training_root: String,
    pub base_model: String,
    pub tasks: Vec<LegalSyntheticWorkflowTaskManifestRow>,
    pub sft_dataset_manifest: String,
    pub dpo_dataset_manifest: String,
    pub manifest_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalSyntheticWorkflowTaskGenerationResult {
    pub manifest_path: PathBuf,
    pub manifest: LegalSyntheticWorkflowManifest,
    pub sft_manifest_path: PathBuf,
    pub dpo_manifest_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum LegalSyntheticWorkflowTaskGeneratorError {
    #[error("generator argument error: {0}")]
    InvalidArgument(String),
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
    #[error("SFT dataset build failed: {0}")]
    Sft(#[from] LegalSftDatasetBuilderError),
    #[error("DPO dataset build failed: {0}")]
    Dpo(#[from] LegalDpoDatasetBuilderError),
}

#[derive(Clone, Debug)]
struct SyntheticTaskMaterial {
    task_id: String,
    task_type: &'static str,
    practice_area: &'static str,
    work_type: &'static str,
    title: String,
    instructions: String,
    source_name: String,
    source_text: String,
    expected_answer: String,
    expected_points: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct HarveySyntheticTaskJson {
    title: String,
    work_type: String,
    tags: Vec<String>,
    instructions: String,
    deliverables: BTreeMap<String, String>,
    criteria: Vec<HarveySyntheticCriterionJson>,
}

#[derive(Clone, Debug, Serialize)]
struct HarveySyntheticCriterionJson {
    id: String,
    title: String,
    deliverables: Vec<String>,
    match_criteria: String,
}

#[derive(Clone, Debug, Serialize)]
struct SyntheticRubricJson {
    schema_version: String,
    task_id: String,
    synthetic: bool,
    task_type: String,
    expected_answer_path: String,
    expected_answer: String,
    expected_points: Vec<String>,
    scoring_visibility: String,
    claim_boundary: String,
}

pub fn generate_legal_synthetic_workflow_tasks(
    config: &LegalSyntheticWorkflowTaskGeneratorConfig,
) -> Result<LegalSyntheticWorkflowTaskGenerationResult, LegalSyntheticWorkflowTaskGeneratorError> {
    if config.count == 0 {
        return Err(LegalSyntheticWorkflowTaskGeneratorError::InvalidArgument(
            "count must be greater than zero".to_string(),
        ));
    }
    if config.count > 10_000 {
        return Err(LegalSyntheticWorkflowTaskGeneratorError::InvalidArgument(
            "count must be 10,000 or lower for one deterministic generation pass".to_string(),
        ));
    }

    let tasks_root = config.out_dir.join("tasks");
    let rubrics_root = config.out_dir.join("rubrics");
    let runs_root = config.out_dir.join("runs");
    let training_root = config.out_dir.join("training");
    let dpo_sample_runs_root = training_root.join("dpo_run_sample");
    fs::create_dir_all(&tasks_root).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: tasks_root.clone(),
            source,
        }
    })?;
    fs::create_dir_all(&rubrics_root).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: rubrics_root.clone(),
            source,
        }
    })?;
    fs::create_dir_all(&runs_root).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: runs_root.clone(),
            source,
        }
    })?;
    fs::create_dir_all(&training_root).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: training_root.clone(),
            source,
        }
    })?;
    fs::create_dir_all(&dpo_sample_runs_root).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: dpo_sample_runs_root.clone(),
            source,
        }
    })?;

    let mut rows = Vec::with_capacity(config.count);
    let mut type_counts = BTreeMap::new();
    let mut successful_base_run_count = 0usize;
    let mut failed_base_run_count = 0usize;

    for index in 0..config.count {
        let material = materialize_synthetic_task(index, config.seed);
        let family_dir = tasks_root.join(material.practice_area);
        let task_dir = family_dir.join(task_slug(&material.task_id));
        let documents_dir = task_dir.join("documents");
        fs::create_dir_all(&documents_dir).map_err(|source| {
            LegalSyntheticWorkflowTaskGeneratorError::Io {
                path: documents_dir.clone(),
                source,
            }
        })?;
        let source_path = documents_dir.join(&material.source_name);
        write_text(source_path.as_path(), &material.source_text)?;

        let task_json_path = task_dir.join("task.json");
        write_json(
            task_json_path.as_path(),
            &harvey_task_json(&material, config.suite_id.as_str()),
        )?;

        let rubric_dir = rubrics_root.join(material.practice_area);
        fs::create_dir_all(&rubric_dir).map_err(|source| {
            LegalSyntheticWorkflowTaskGeneratorError::Io {
                path: rubric_dir.clone(),
                source,
            }
        })?;
        let rubric_path = rubric_dir.join(format!("{}.rubric.json", task_slug(&material.task_id)));
        write_json(rubric_path.as_path(), &rubric_json(&material))?;

        let base_run_id = format!("run.{}.base", material.task_id.replace('.', "_"));
        let base_outcome = base_outcome(index);
        match base_outcome {
            BaseOutcome::Success => {
                successful_base_run_count += 1;
                write_success_run(
                    runs_root.as_path(),
                    &material,
                    &base_run_id,
                    config.base_model.as_str(),
                )?;
                if index < DEFAULT_DPO_SAMPLE_TASK_COUNT {
                    write_success_run(
                        dpo_sample_runs_root.as_path(),
                        &material,
                        &base_run_id,
                        config.base_model.as_str(),
                    )?;
                }
            }
            BaseOutcome::Failure(failure_class) => {
                failed_base_run_count += 1;
                write_bad_run(
                    runs_root.as_path(),
                    &material,
                    &base_run_id,
                    config.base_model.as_str(),
                    failure_class,
                )?;
                if index < DEFAULT_DPO_SAMPLE_TASK_COUNT {
                    write_bad_run(
                        dpo_sample_runs_root.as_path(),
                        &material,
                        &base_run_id,
                        config.base_model.as_str(),
                        failure_class,
                    )?;
                }
            }
        }

        *type_counts
            .entry(material.task_type.to_string())
            .or_insert(0) += 1;
        rows.push(LegalSyntheticWorkflowTaskManifestRow {
            task_id: material.task_id.clone(),
            task_type: material.task_type.to_string(),
            practice_area: material.practice_area.to_string(),
            work_type: material.work_type.to_string(),
            title: material.title.clone(),
            task_json_path: display_path(task_json_path.as_path()),
            rubric_path: display_path(rubric_path.as_path()),
            source_document_paths: vec![display_path(source_path.as_path())],
            expected_answer_hash: sha256_hex(material.expected_answer.as_bytes()),
            base_run_id,
            base_outcome: base_outcome.label().to_string(),
            training_tags: vec![
                String::from("synthetic"),
                String::from("legal_workflow"),
                material.task_type.to_string(),
            ],
        });
    }

    let sft_manifest_path = training_root.join("sft_manifest.json");
    let dpo_manifest_path = training_root.join("dpo_manifest.json");
    let sft_result = build_legal_benchmark_sft_dataset(&LegalSftDatasetBuilderConfig {
        runs_root: runs_root.clone(),
        out_jsonl: training_root.join("sft_dataset.jsonl"),
        manifest_json: sft_manifest_path.clone(),
        dataset_id: format!("{}.sft", config.suite_id),
    })?;
    let dpo_result = build_legal_benchmark_dpo_dataset(&LegalDpoDatasetBuilderConfig {
        runs_root: dpo_sample_runs_root.clone(),
        out_jsonl: training_root.join("dpo_dataset.jsonl"),
        manifest_json: dpo_manifest_path.clone(),
        dataset_id: format!("{}.dpo", config.suite_id),
    })?;

    let mut manifest = LegalSyntheticWorkflowManifest {
        schema_version: String::from(LEGAL_SYNTHETIC_WORKFLOW_MANIFEST_SCHEMA_VERSION),
        suite_id: config.suite_id.clone(),
        generator_id: String::from(LEGAL_SYNTHETIC_WORKFLOW_GENERATOR_ID),
        seed: config.seed,
        synthetic: true,
        source_policy: String::from(
            "all task facts and documents are generated synthetic text; no hidden or private benchmark data is used",
        ),
        claim_boundary: String::from(
            "synthetic workflow tasks are for SFT, DPO, and GRPO reward shaping only; their scores must not be reported as Harvey benchmark scores",
        ),
        task_count: rows.len(),
        task_type_counts: type_counts,
        successful_base_run_count,
        failed_base_run_count,
        sft_example_count: sft_result.manifest.included_count,
        dpo_pair_count: dpo_result.manifest.included_count,
        tasks_root: display_path(tasks_root.as_path()),
        rubrics_root: display_path(rubrics_root.as_path()),
        runs_root: display_path(runs_root.as_path()),
        dpo_sample_runs_root: display_path(dpo_sample_runs_root.as_path()),
        training_root: display_path(training_root.as_path()),
        base_model: config.base_model.clone(),
        tasks: rows,
        sft_dataset_manifest: display_path(sft_manifest_path.as_path()),
        dpo_dataset_manifest: display_path(dpo_manifest_path.as_path()),
        manifest_hash: String::new(),
    };
    manifest.manifest_hash = synthetic_manifest_hash(&manifest)?;
    let manifest_path = config.out_dir.join("manifest.json");
    write_json(manifest_path.as_path(), &manifest)?;

    Ok(LegalSyntheticWorkflowTaskGenerationResult {
        manifest_path,
        manifest,
        sft_manifest_path,
        dpo_manifest_path,
    })
}

#[derive(Clone, Copy, Debug)]
enum BaseOutcome {
    Success,
    Failure(&'static str),
}

impl BaseOutcome {
    const fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure(failure_class) => failure_class,
        }
    }
}

fn base_outcome(index: usize) -> BaseOutcome {
    match index % 4 {
        0 | 3 => BaseOutcome::Success,
        1 => BaseOutcome::Failure("missing_answer"),
        _ => BaseOutcome::Failure("wrong_path"),
    }
}

fn harvey_task_json(material: &SyntheticTaskMaterial, suite_id: &str) -> HarveySyntheticTaskJson {
    let mut deliverables = BTreeMap::new();
    deliverables.insert(String::from("answer.md"), String::from("answer.md"));
    HarveySyntheticTaskJson {
        title: material.title.clone(),
        work_type: material.work_type.to_string(),
        tags: vec![
            String::from("synthetic"),
            String::from("legal-workflow-v1"),
            suite_id.to_string(),
            material.task_type.to_string(),
        ],
        instructions: material.instructions.clone(),
        deliverables,
        criteria: vec![
            HarveySyntheticCriterionJson {
                id: String::from("SYN-001"),
                title: String::from("Writes the required answer file"),
                deliverables: vec![String::from("answer.md")],
                match_criteria: String::from(
                    "PASS if the agent creates answer.md itself and submits only after the file exists.",
                ),
            },
            HarveySyntheticCriterionJson {
                id: String::from("SYN-002"),
                title: String::from("Uses the provided source text"),
                deliverables: vec![String::from("answer.md")],
                match_criteria: String::from(
                    "PASS if the answer relies only on the generated source document attached to this synthetic task.",
                ),
            },
            HarveySyntheticCriterionJson {
                id: String::from("SYN-003"),
                title: String::from("Matches the separate synthetic rubric"),
                deliverables: vec![String::from("answer.md")],
                match_criteria: String::from(
                    "PASS if the answer covers the expected points in the separate synthetic rubric JSON. Do not show that rubric to the model.",
                ),
            },
        ],
    }
}

fn rubric_json(material: &SyntheticTaskMaterial) -> SyntheticRubricJson {
    SyntheticRubricJson {
        schema_version: String::from(LEGAL_SYNTHETIC_WORKFLOW_RUBRIC_SCHEMA_VERSION),
        task_id: material.task_id.clone(),
        synthetic: true,
        task_type: material.task_type.to_string(),
        expected_answer_path: String::from("answer.md"),
        expected_answer: material.expected_answer.clone(),
        expected_points: material.expected_points.clone(),
        scoring_visibility: String::from("judge_only_not_model_visible"),
        claim_boundary: String::from(
            "this rubric is for synthetic workflow reward only and is not Harvey benchmark evidence",
        ),
    }
}

fn write_success_run(
    runs_root: &Path,
    material: &SyntheticTaskMaterial,
    run_id: &str,
    base_model: &str,
) -> Result<(), LegalSyntheticWorkflowTaskGeneratorError> {
    let run_dir = runs_root
        .join("base_success")
        .join(task_slug(&material.task_id));
    let output_dir = run_dir.join("output");
    fs::create_dir_all(&output_dir).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: output_dir.clone(),
            source,
        }
    })?;
    let answer_path = output_dir.join("answer.md");
    write_text(answer_path.as_path(), &material.expected_answer)?;
    let receipt = json!({
        "schema_version": "psionic.legal_benchmark.synthetic_base_run_receipt.v1",
        "run_spec": {
            "run_id": run_id,
            "task_id": material.task_id,
            "task_version": "synthetic-v1",
            "benchmark_visibility": "synthetic",
            "base_model": base_model,
            "required_output_paths": ["answer.md"]
        },
        "synthetic": true,
        "answer_files": [{
            "relative_path": "answer.md",
            "content_hash": sha256_hex(material.expected_answer.as_bytes()),
            "integrity_valid": true,
            "creation_actor": "model",
            "last_modifying_actor": "model"
        }],
        "integrity": {
            "valid": true
        },
        "tool_calls": [
            {
                "tool_name": "read_source",
                "path": material.source_name,
                "actor": "model"
            },
            {
                "tool_name": "write_file",
                "path": "answer.md",
                "actor": "model"
            },
            {
                "tool_name": "validate_file",
                "path": "answer.md",
                "actor": "model"
            },
            {
                "tool_name": "submit",
                "actor": "model"
            }
        ],
        "workflow_result": {
            "success": true,
            "reported_harvey_score": false
        }
    });
    write_json(run_dir.join("run_receipt.json").as_path(), &receipt)
}

fn write_bad_run(
    runs_root: &Path,
    material: &SyntheticTaskMaterial,
    run_id: &str,
    base_model: &str,
    failure_class: &str,
) -> Result<(), LegalSyntheticWorkflowTaskGeneratorError> {
    let bad_dir = runs_root.join("base_failure");
    fs::create_dir_all(&bad_dir).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Io {
            path: bad_dir.clone(),
            source,
        }
    })?;
    let prompt = format!(
        "{}\n\nRequired output path: answer.md\nSource document: {}",
        material.instructions, material.source_name
    );
    let response = match failure_class {
        "missing_answer" => {
            "I reviewed the issue and would answer in prose, but I did not create answer.md."
        }
        "wrong_path" => "I wrote a draft in notes.txt instead of the required answer.md path.",
        _ => "The run did not complete the required legal workflow.",
    };
    let correction = format!(
        "Read the generated source text, write answer.md with the required legal work product, validate that answer.md exists, and submit. A correct answer should cover: {}.",
        material.expected_points.join("; ")
    );
    let bad_run = json!({
        "schema_version": "psionic.legal_benchmark.synthetic_bad_run.v1",
        "example_id": format!("bad_run.{run_id}"),
        "base_task_id": material.task_id,
        "benchmark_visibility": "synthetic",
        "base_model": base_model,
        "synthetic": true,
        "failure_class": failure_class,
        "full_prompt": prompt,
        "full_model_response": response,
        "suggested_correction": correction,
        "required_file_paths": ["answer.md"],
        "training_eligible": true,
        "training_eligibility_reasons": [
            "synthetic workflow failure with generated source text only",
            "no Harvey score is claimed"
        ],
        "integrity": {
            "valid": true
        },
        "tool_call_transcript": []
    });
    write_json(
        bad_dir
            .join(format!("{}.bad_run.json", task_slug(&material.task_id)))
            .as_path(),
        &bad_run,
    )
}

fn materialize_synthetic_task(index: usize, seed: u64) -> SyntheticTaskMaterial {
    let serial = index + 1;
    let variant = ((seed as usize) + index) % 97;
    match index % 8 {
        0 => contract_clause_extraction(serial, variant),
        1 => employment_summary(serial, variant),
        2 => nda_risk_list(serial, variant),
        3 => lease_obligation_extraction(serial, variant),
        4 => litigation_source_summary(serial, variant),
        5 => statute_to_facts_application(serial, variant),
        6 => privilege_log_classification(serial, variant),
        _ => answer_file_workflow(serial, variant),
    }
}

fn contract_clause_extraction(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let days = 10 + variant % 20;
    let source = format!(
        "Generated service agreement excerpt.\nEither party may terminate for uncured material breach after {days} days written notice. Fees are due within {} days after invoice receipt. Notices may be delivered by email with confirmation.",
        days + 5
    );
    let answer = format!(
        "# Contract clause extraction\n\nTermination requires {days} days written notice for an uncured material breach. Fees are due within {} days after invoice receipt. Email notice is allowed if receipt is confirmed.",
        days + 5
    );
    task_material(
        serial,
        "contract_clause_extraction",
        "corporate",
        "extract",
        "Contract clause extraction",
        "Read the generated service agreement excerpt and write answer.md extracting the termination notice period, payment deadline, and notice method.",
        "service-agreement.txt",
        source,
        answer,
        vec![
            format!("termination notice period is {days} days"),
            format!("payment deadline is {} days", days + 5),
            String::from("email notice requires confirmation"),
        ],
    )
}

fn employment_summary(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let period = 6 + variant % 6;
    let source = format!(
        "Generated employment agreement excerpt.\nThe employee will be paid a base salary of ${},000. The confidentiality obligation lasts indefinitely. The nonsolicit covenant lasts {period} months after termination and applies only to customers contacted during employment.",
        90 + variant % 30
    );
    let answer = format!(
        "# Employment agreement summary\n\nThe agreement provides a base salary, indefinite confidentiality, and a {period}-month customer nonsolicit limited to customers contacted during employment. The main follow-up is whether local law permits that nonsolicit scope."
    );
    task_material(
        serial,
        "employment_agreement_summary",
        "employment",
        "summarize",
        "Employment agreement summary",
        "Read the generated employment agreement excerpt and write answer.md summarizing compensation, confidentiality, nonsolicit scope, and one legal follow-up.",
        "employment-agreement.txt",
        source,
        answer,
        vec![
            String::from("mentions base salary"),
            String::from("confidentiality lasts indefinitely"),
            format!("nonsolicit lasts {period} months"),
            String::from("asks whether local law permits the covenant"),
        ],
    )
}

fn nda_risk_list(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let years = 2 + variant % 4;
    let source = format!(
        "Generated NDA excerpt.\nRecipient may retain one archival copy. Residual knowledge is excluded from confidentiality. Source code, customer lists, and pricing are confidential for {years} years. Injunctive relief is available for breach."
    );
    let answer = format!(
        "# NDA risk list\n\nKey risks are the residual-knowledge carveout, the archival-copy right, and whether {years} years is long enough for source code, customer lists, and pricing. The injunctive relief clause helps, but the disclosing party should tighten residuals and archival-copy controls."
    );
    task_material(
        serial,
        "nda_risk_list",
        "commercial",
        "risk_list",
        "NDA risk list",
        "Read the generated NDA excerpt and write answer.md listing the main drafting risks and practical fixes.",
        "nda.txt",
        source,
        answer,
        vec![
            String::from("flags residual-knowledge carveout"),
            String::from("flags archival-copy right"),
            format!("confidentiality period is {years} years"),
            String::from("suggests tightening residuals or archival-copy controls"),
        ],
    )
}

fn lease_obligation_extraction(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let hours = 24 + variant % 24;
    let source = format!(
        "Generated lease excerpt.\nTenant must maintain general liability insurance. Landlord must repair roof leaks within {hours} hours after written notice. Tenant must not assign the lease without prior written consent, not to be unreasonably withheld."
    );
    let answer = format!(
        "# Lease obligation extraction\n\nTenant must maintain general liability insurance and must not assign without prior written consent. Landlord must repair roof leaks within {hours} hours after written notice, and consent to assignment may not be unreasonably withheld."
    );
    task_material(
        serial,
        "lease_obligation_extraction",
        "real_estate",
        "extract",
        "Lease obligation extraction",
        "Read the generated lease excerpt and write answer.md identifying tenant obligations, landlord obligations, and consent standards.",
        "lease.txt",
        source,
        answer,
        vec![
            String::from("tenant insurance obligation"),
            format!("landlord repair deadline is {hours} hours"),
            String::from("assignment requires prior written consent"),
            String::from("consent cannot be unreasonably withheld"),
        ],
    )
}

fn litigation_source_summary(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let date = 3 + variant % 20;
    let source = format!(
        "Generated litigation file note.\nOn March {date}, the project manager wrote that the vendor missed milestone two because client data arrived late. On March {}, outside counsel asked the team to preserve schedule emails and draft a chronology for mediation.",
        date + 2
    );
    let answer = format!(
        "# Litigation source summary\n\nThe source says milestone two slipped because client data arrived late, according to the March {date} project-manager note. Outside counsel then asked on March {} for preservation of schedule emails and a mediation chronology.",
        date + 2
    );
    task_material(
        serial,
        "litigation_memo_source_summary",
        "litigation",
        "summarize",
        "Litigation memo source summary",
        "Read the generated litigation file note and write answer.md summarizing the key chronology and counsel request.",
        "litigation-note.txt",
        source,
        answer,
        vec![
            format!("mentions March {date} project-manager note"),
            String::from("identifies late client data as the cause"),
            String::from("mentions outside counsel preservation request"),
            String::from("mentions mediation chronology"),
        ],
    )
}

fn statute_to_facts_application(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let amount = 5000 + (variant % 20) * 250;
    let source = format!(
        "Generated statute and facts.\nStatute text: A vendor must give written cancellation notice within 5 business days when prepaid services over ${amount} cannot be performed within 30 days. Facts: Customer prepaid ${} for a workshop that the vendor cannot staff for 45 days.",
        amount + 750
    );
    let answer = format!(
        "# Statute-to-facts application\n\nThe notice rule likely applies because the customer prepaid more than ${amount}, the service cannot be performed within 30 days, and the vendor knows it cannot staff the workshop for 45 days. The vendor should send written cancellation notice within 5 business days."
    );
    task_material(
        serial,
        "statute_to_facts_application",
        "regulatory",
        "apply",
        "Statute-to-facts application",
        "Read the generated statute and facts and write answer.md applying the rule to the facts.",
        "statute-and-facts.txt",
        source,
        answer,
        vec![
            format!("threshold is prepaid services over ${amount}"),
            String::from("facts exceed the threshold"),
            String::from("performance delay is 45 days"),
            String::from("written notice due within 5 business days"),
        ],
    )
}

fn privilege_log_classification(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let entry = 20 + variant % 30;
    let source = format!(
        "Generated privilege log excerpt.\nEntry {entry}: Email from outside counsel to general counsel giving litigation strategy. Entry {}: Finance spreadsheet sent to counsel and business team showing ordinary revenue forecasts.",
        entry + 1
    );
    let answer = format!(
        "# Privilege log classification\n\nEntry {entry} is likely privileged because outside counsel sent litigation strategy to the general counsel. Entry {} is weaker because an ordinary finance spreadsheet copied to counsel is primarily business material on these facts.",
        entry + 1
    );
    task_material(
        serial,
        "privilege_log_classification",
        "litigation",
        "classify",
        "Privilege log classification",
        "Read the generated privilege log excerpt and write answer.md classifying both entries with short reasons.",
        "privilege-log.txt",
        source,
        answer,
        vec![
            format!("entry {entry} likely privileged"),
            String::from("outside counsel litigation strategy"),
            format!("entry {} weaker or likely not privileged", entry + 1),
            String::from("ordinary finance spreadsheet is business material"),
        ],
    )
}

fn answer_file_workflow(serial: usize, variant: usize) -> SyntheticTaskMaterial {
    let deadline = 7 + variant % 10;
    let source = format!(
        "Generated workflow instruction.\nThe client asks for a one-page answer. The only required filing is answer.md. Include the deadline of {deadline} calendar days and say that no separate appendix is needed."
    );
    let answer = format!(
        "# Workflow answer\n\nThe answer.md file should state that the deadline is {deadline} calendar days. No separate appendix is needed. This response is intentionally limited to the required answer file."
    );
    task_material(
        serial,
        "answer_file_workflow_only",
        "operations",
        "workflow",
        "Answer-file workflow task",
        "Read the generated workflow instruction and write only answer.md. Do not create extra files.",
        "workflow-instruction.txt",
        source,
        answer,
        vec![
            format!("deadline is {deadline} calendar days"),
            String::from("no separate appendix is needed"),
            String::from("writes only answer.md"),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn task_material(
    serial: usize,
    task_type: &'static str,
    practice_area: &'static str,
    work_type: &'static str,
    title_prefix: &str,
    instructions: &str,
    source_name: &str,
    source_text: String,
    expected_answer: String,
    expected_points: Vec<String>,
) -> SyntheticTaskMaterial {
    SyntheticTaskMaterial {
        task_id: format!("synthetic.legal_workflow_v1.{task_type}.{serial:04}"),
        task_type,
        practice_area,
        work_type,
        title: format!("{title_prefix} {serial:04}"),
        instructions: format!(
            "{instructions} Use only the generated source document. This is a synthetic training task, not a Harvey benchmark task."
        ),
        source_name: source_name.to_string(),
        source_text,
        expected_answer,
        expected_points,
    }
}

fn task_slug(task_id: &str) -> String {
    task_id.replace('.', "-").replace('_', "-")
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), LegalSyntheticWorkflowTaskGeneratorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            LegalSyntheticWorkflowTaskGeneratorError::Io {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|source| LegalSyntheticWorkflowTaskGeneratorError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_text(path: &Path, content: &str) -> Result<(), LegalSyntheticWorkflowTaskGeneratorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            LegalSyntheticWorkflowTaskGeneratorError::Io {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }
    fs::write(path, content).map_err(|source| LegalSyntheticWorkflowTaskGeneratorError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn synthetic_manifest_hash(
    manifest: &LegalSyntheticWorkflowManifest,
) -> Result<String, LegalSyntheticWorkflowTaskGeneratorError> {
    let mut clone = manifest.clone();
    clone.manifest_hash.clear();
    let bytes = serde_json::to_vec(&clone).map_err(|source| {
        LegalSyntheticWorkflowTaskGeneratorError::Json {
            path: PathBuf::from("synthetic_manifest"),
            source,
        }
    })?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn generator_builds_synthetic_tasks_and_training_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result =
            generate_legal_synthetic_workflow_tasks(&LegalSyntheticWorkflowTaskGeneratorConfig {
                count: 16,
                out_dir: dir.path().join("synthetic"),
                suite_id: String::from("synthetic_legal_workflow_test"),
                seed: 20260520,
                base_model: String::from("synthetic-base-test"),
            })
            .expect("generate synthetic tasks");

        assert_eq!(result.manifest.task_count, 16);
        assert_eq!(result.manifest.successful_base_run_count, 8);
        assert_eq!(result.manifest.failed_base_run_count, 8);
        assert!(result.manifest.synthetic);
        assert!(!result.manifest.manifest_hash.is_empty());
        assert!(result.sft_manifest_path.exists());
        assert!(result.dpo_manifest_path.exists());
        assert!(result.manifest.sft_example_count > 0);
        assert!(result.manifest.dpo_pair_count > 0);

        let first = result.manifest.tasks.first().expect("first row");
        let task_json = fs::read_to_string(&first.task_json_path).expect("task json");
        assert!(task_json.contains("synthetic"));
        let rubric_json = fs::read_to_string(&first.rubric_path).expect("rubric json");
        assert!(rubric_json.contains("judge_only_not_model_visible"));
    }

    #[test]
    fn generator_refuses_empty_count() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err =
            generate_legal_synthetic_workflow_tasks(&LegalSyntheticWorkflowTaskGeneratorConfig {
                count: 0,
                out_dir: dir.path().join("synthetic"),
                suite_id: String::from("synthetic_legal_workflow_test"),
                seed: 20260520,
                base_model: String::from("synthetic-base-test"),
            })
            .expect_err("empty count should fail");
        assert!(matches!(
            err,
            LegalSyntheticWorkflowTaskGeneratorError::InvalidArgument(_)
        ));
    }
}
