# CND-048 OpenAgents/Codex Benchmark Adapter

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: adapter scaffold
Last updated: 2026-06-01

This adapter lets the same normalized Terminal-Bench task shape run through an
OpenAgents/Codex-compatible agent mode. It is not a public score claim. It is an
internal comparison path that produces artifacts Worker/Khala Sync can attach to a
Benchmark Workroom and proof bundle.

## Smoke Fixture

```text
runners/py-bench-runner/fixtures/tasks/terminal-bench-codex-dry-run.json
```

The fixture uses the same Terminal-Bench smoke selector as the oracle lane:

```text
dataset: terminal-bench
version: 2.0
terminalBenchTaskId: tb2-smoke-oracle
agent: openagents-codex
model: codex-account
sandbox: danger-full-access
```

`danger-full-access` is only acceptable here as an explicitly isolated
benchmark/container or VM profile with no wallet authority and no broad cloud
credentials.

## Local Dry-Run

```bash
cd runners/py-bench-runner
OPENAGENTS_BENCH_CODEX_DRY_RUN=1 \
python3 -m openagents_bench.run_task \
  --run-id run_tb2_codex_dry \
  --task-run-id taskrun_tb2_codex \
  --task-spec fixtures/tasks/terminal-bench-codex-dry-run.json \
  --artifact-dir /tmp/oa-bench-tb2-codex-dry \
  --agent openagents-codex \
  --model codex-account
```

The dry-run writes:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `commands.jsonl`
- `transcript.md`
- `codex_stdout.jsonl`
- `codex_stderr.log`
- `agent_stdout.log`
- `agent_stderr.log`
- `workspace.diff`
- verifier stdout/stderr placeholders

## Real Codex Command

When dry-run is disabled and `codex` is installed in the runner image, the
adapter executes:

```bash
codex exec \
  --skip-git-repo-check \
  --json \
  --sandbox "$SANDBOX_PROFILE" \
  "$PROMPT"
```

The prompt is derived from the normalized task envelope and redacted before it
is passed to Codex. Raw `auth.json`, access tokens, refresh tokens, device auth
IDs, code verifiers, API keys, wallet material, and broad GCP credentials must
not appear in artifacts or proof bundles.

## Proof Bundle Fields

`proof_bundle.json` includes:

- dataset and version;
- task id and task selector;
- harness version;
- agent and model;
- provider;
- retry policy;
- timeout seconds;
- artifact count and digests;
- redaction status;
- internal claim state.

Worker/Khala Sync can compare the oracle proof bundle and OpenAgents/Codex proof bundle
without treating either as a public benchmark claim.
