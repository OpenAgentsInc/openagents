# CND-049 SWE-Bench And Custom Repo Adapter

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: adapter scaffold
Last updated: 2026-06-01

Benchmark Cloud is not Terminal-Bench-specific. The repository adapter maps
SWE-bench, SWT-Bench, and custom repo tasks into the same normalized runner,
artifact, result, and proof bundle contract.

## Supported Dataset Slugs

The initial repo adapter accepts:

- `custom-repo`
- `swe-bench`
- `swt-bench`

The adapter records the dataset slug and version in `result.json` and
`proof_bundle.json`; Worker/Khala Sync should not need dataset-specific state tables for
the first tracking path.

## Fixtures

```text
runners/py-bench-runner/fixtures/tasks/custom-repo-dry-run.json
runners/py-bench-runner/fixtures/tasks/swe-bench-dry-run.json
```

Both fixtures use the same artifact contract:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `commands.jsonl`
- `transcript.md`
- `workspace.diff`
- `patch.diff`
- `repo_result.json`
- verifier stdout/stderr

## Local Dry-Run

```bash
cd runners/py-bench-runner
OPENAGENTS_BENCH_REPO_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_custom_repo_dry \
  --task-run-id taskrun_custom_repo \
  --task-spec fixtures/tasks/custom-repo-dry-run.json \
  --artifact-dir /tmp/oa-bench-custom-repo-dry \
  --agent openagents-codex \
  --model codex-account
```

SWE-style dry-run:

```bash
OPENAGENTS_BENCH_REPO_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_swe_dry \
  --task-run-id taskrun_swe \
  --task-spec fixtures/tasks/swe-bench-dry-run.json \
  --artifact-dir /tmp/oa-bench-swe-dry \
  --agent openagents-codex \
  --model codex-account
```

## Real Repo Flow

When dry-run is disabled, the adapter:

1. clones `repo_url`;
2. checks out `base_commit` if provided;
3. executes declared verifier shell commands;
4. captures stdout/stderr;
5. captures `git diff --binary` as `workspace.diff` and `patch.diff`;
6. writes normalized result and proof-bundle artifacts.

This issue intentionally starts with a narrow adapter. A later implementation
can plug the full OpenAgents/Codex patch loop into the workspace before verifier
execution.

## Guardrails

- No broad SWE-bench leaderboard claim is made by this adapter.
- The verifier commands are declared in the task envelope.
- The adapter writes artifacts through the same redaction boundary as the
  Terminal-Bench and Codex adapters.
- Public claim projection stays blocked until Worker/Khala Sync verifies proof bundle,
  redaction, dataset version, task subset, harness version, retry policy, and
  artifact retention.
