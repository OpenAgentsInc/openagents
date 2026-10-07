#![allow(clippy::print_stdout)]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use psionic_eval::{
    ArtifactExtractionPolicy, ArtifactExtractorRegistry, ExtractionFailureKind, scan_harvey_corpus,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SliceExtractionSummary {
    tasks_root: String,
    upstream_commit: String,
    task_limit: usize,
    task_count_scanned: usize,
    source_artifact_count: usize,
    extracted_count: usize,
    structured_failure_count: usize,
    failures_by_kind: BTreeMap<String, usize>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let tasks_root = args.next().map(PathBuf::from).ok_or_else(|| {
        "usage: legal_benchmark_extract_slice <harvey_tasks_root> <upstream_commit> [task_limit]"
            .to_string()
    })?;
    let upstream_commit = args.next().ok_or_else(|| {
        "usage: legal_benchmark_extract_slice <harvey_tasks_root> <upstream_commit> [task_limit]"
            .to_string()
    })?;
    let task_limit = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(20);

    let scan = scan_harvey_corpus(tasks_root.as_path(), upstream_commit.clone())?;
    let registry = ArtifactExtractorRegistry::default();
    let policy = ArtifactExtractionPolicy::default();

    let mut source_artifact_count = 0usize;
    let mut extracted_count = 0usize;
    let mut structured_failure_count = 0usize;
    let mut failures_by_kind = BTreeMap::new();

    for task in scan.tasks.iter().take(task_limit) {
        for source_artifact in &task.source_artifacts {
            source_artifact_count += 1;
            let source_path = tasks_root.join(source_artifact.relative_path.as_str());
            let bytes = fs::read(source_path)?;
            let result = registry.extract(source_artifact, bytes.as_slice(), &policy);
            match result.receipt.failure_kind {
                Some(kind) => {
                    structured_failure_count += 1;
                    *failures_by_kind
                        .entry(failure_kind_label(kind))
                        .or_insert(0) += 1;
                }
                None => extracted_count += 1,
            }
        }
    }

    let summary = SliceExtractionSummary {
        tasks_root: tasks_root.display().to_string(),
        upstream_commit,
        task_limit,
        task_count_scanned: scan.tasks.len().min(task_limit),
        source_artifact_count,
        extracted_count,
        structured_failure_count,
        failures_by_kind,
    };
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn failure_kind_label(kind: ExtractionFailureKind) -> String {
    format!("{kind:?}")
}
