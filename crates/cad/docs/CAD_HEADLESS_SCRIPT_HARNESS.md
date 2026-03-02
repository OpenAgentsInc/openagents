# CAD Headless Script Harness

This document defines deterministic headless workflow scripting for CLI and MCP CAD surfaces.

## Module

- `crates/cad/src/headless_script_harness.rs`

## Entry Points

- `run_headless_script(&CadHeadlessScript)`
- `canonical_headless_cli_workflow_script()`
- `canonical_headless_mcp_workflow_script()`
- `fail_fast_headless_workflow_script()`

## Script Contract

- `script_id` identifies the workflow run.
- `fail_fast` controls whether later steps are skipped after the first failure.
- `steps` is an ordered sequence with tagged kinds:
  - `cli_command`
  - `mcp_create_sample`
  - `mcp_inspect_document`
  - `mcp_export_document`

## Determinism Contract

- Workspace is recreated under `target/parity/headless-script-harness/<script_id>`.
- Input seeds (`seed_mesh.json`, `seed_import.step`, `seed_import.stl`) are deterministic.
- Step reports encode stable hashes for stdout/stderr/tool responses and output files.
- Report-level `deterministic_signature` is stable across replay for identical scripts.

## Fail-Fast Behavior

- On step failure with `fail_fast=true`, remaining steps are marked `skipped`.
- Harness exposes:
  - `halted` flag
  - executed/failed/skipped counts
  - per-step status with diagnostics
