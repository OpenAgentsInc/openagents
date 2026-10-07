//! Regenerates the committed Psion instruct SFT lane fixtures (psionic#1117).
//!
//! Writes the owned chat template contract, the labeled example instruct
//! corpus with provenance manifest, the token-level generation-mask fixture,
//! and the bounded lane report produced by one deterministic smoke run with
//! checkpoint/resume drill.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PSION_CHAT_TEMPLATE_FIXTURE_PATH, PSION_INSTRUCT_CORPUS_MANIFEST_FIXTURE_PATH,
    PSION_INSTRUCT_EXAMPLE_CORPUS_FIXTURE_PATH, PSION_INSTRUCT_GENERATION_MASK_FIXTURE_PATH,
    PSION_INSTRUCT_SFT_LANE_REPORT_FIXTURE_PATH, PsionChatTemplate, PsionInstructArtifactRef,
    build_psion_instruct_corpus_manifest, default_psion_instruct_sft_lane_config,
    psion_instruct_example_corpus_records, run_psion_instruct_sft_lane,
};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    // Optional first argument: alternate output root (used by the check
    // script to regenerate into a temporary directory for comparison).
    let root = match std::env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => workspace_root()?,
    };
    let template = PsionChatTemplate::v1();
    template.validate()?;
    let records = psion_instruct_example_corpus_records();

    // Owned versioned chat template contract.
    write_json(&root.join(PSION_CHAT_TEMPLATE_FIXTURE_PATH), &template)?;

    // Example instruct corpus as JSONL, one record per line.
    let mut corpus_lines = Vec::new();
    for record in &records {
        corpus_lines.push(serde_json::to_string(record)?);
    }
    let corpus_payload = format!("{}\n", corpus_lines.join("\n"));
    let corpus_path = root.join(PSION_INSTRUCT_EXAMPLE_CORPUS_FIXTURE_PATH);
    if let Some(parent) = corpus_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&corpus_path, corpus_payload.as_bytes())?;
    let corpus_sha256 = format!("{:x}", Sha256::digest(corpus_payload.as_bytes()));

    // Provenance manifest under the same digest discipline as the
    // pretraining data bundles.
    let manifest = build_psion_instruct_corpus_manifest(
        &template,
        "psion_instruct_corpus_example",
        "v1",
        true,
        PsionInstructArtifactRef {
            path: String::from(PSION_INSTRUCT_EXAMPLE_CORPUS_FIXTURE_PATH),
            sha256: corpus_sha256,
        },
        &records,
    )?;
    write_json(
        &root.join(PSION_INSTRUCT_CORPUS_MANIFEST_FIXTURE_PATH),
        &manifest,
    )?;

    // Token-level generation-mask fixture: every record rendered through the
    // pinned template with its exact mask vector.
    let rendered: Vec<_> = records
        .iter()
        .map(|record| record.validate(&template))
        .collect::<Result<_, _>>()?;
    write_json(
        &root.join(PSION_INSTRUCT_GENERATION_MASK_FIXTURE_PATH),
        &rendered,
    )?;

    // Bounded lane report: deterministic masked-loss smoke run with applied
    // learning-rate schedule and checkpoint/resume drill.
    let config = default_psion_instruct_sft_lane_config(manifest.manifest_digest.as_str());
    let report = run_psion_instruct_sft_lane(&config, &records)?;
    if !report.resume_drill.resume_bit_exact {
        return Err("checkpoint/resume drill was not bit exact".into());
    }
    if !report.loss_improved {
        return Err("bounded smoke run did not improve loss".into());
    }
    write_json(
        &root.join(PSION_INSTRUCT_SFT_LANE_REPORT_FIXTURE_PATH),
        &report,
    )?;

    println!(
        "wrote instruct SFT lane fixtures: template={} manifest={} report={}",
        template.template_digest, manifest.manifest_digest, report.report_digest
    );
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut payload = serde_json::to_string_pretty(value)?;
    payload.push('\n');
    fs::write(path, payload.as_bytes())?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("fixtures").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err("failed to locate workspace root".into());
        }
    }
}
