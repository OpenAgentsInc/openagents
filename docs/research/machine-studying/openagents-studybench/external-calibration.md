# StudyBench External Calibration

Date: 2026-06-17
Status: MVP calibration manifest, not a benchmark claim

## Purpose

OpenAgents uses upstream StudyBench as an external public calibration suite for
loaders, schema validation, rubric scoring, and DSPy/GEPA behavior. It is not a
hidden benchmark and cannot be standalone evidence for a public OpenAgents repo
studying product claim.

## Dataset Refs

- `hf://jacobli/studybench/dspy`
  - Hugging Face loader shape:
    `load_dataset("jacobli/studybench", "dspy", split="train")`
  - Expected rows: 30.
  - Codebase attribution: `stanfordnlp/dspy` at
    `9cdb0aac28b2a04b064e40697ccd301872cf6a43`.
- `hf://jacobli/studybench/openclaw`
  - Hugging Face loader shape:
    `load_dataset("jacobli/studybench", "openclaw", split="train")`
  - Expected rows: 20.
  - Codebase attribution: `openclaw/openclaw` at
    `da228660306b55a9cce3b973946f3aacfc515848`.

## Licensing

- StudyBench questions, gold answers, and rubrics are attributed as
  CC-BY-4.0 material.
- Embedded DSPy and OpenClaw source excerpts remain attributed to the upstream
  MIT-licensed repositories.

## Runtime Boundary

Probe runtime validates manifests and loaded rows. It does not fetch Hugging
Face datasets during contract validation. Operator scripts or later benchmark
runner work may load rows, then pass them through the OpenAgents StudyBench
task contracts.

The public repo should not vendor upstream StudyBench rows for MVP. Keep the
runtime calibration artifact as refs, loader instructions, license refs, and
source attribution refs.
