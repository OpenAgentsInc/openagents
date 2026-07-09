# `openagents.training_run_assignment.v1`

Status: implementation scaffold for `TRAIN-005`

This contract is the Cloud-side input for retained SHC benchmark/training
runs. It lets Vortex start one bounded Terminal-Bench task through the
account-backed Codex VM runner without giving Cloud arbitrary shell-command
authority or product authority over training success.

## Assignment Fields

| Field | Purpose |
| --- | --- |
| `training_run_id` | Vortex-owned parent training run id. |
| `benchmark_run_id` | Vortex-owned benchmark run id. |
| `task_run_id` | Stable child run id; used as the Cloud Codex run id. |
| `target_node_id` | First target is `oa-shc-katy-01`. |
| `dataset` | `dataset_slug`, `dataset_version`, `task_ref`, and optional task checksum. The first runner only accepts `terminal-bench` task refs such as `terminal-bench/db-wal-recovery`. |
| `variants` | Agent/model variant list. The first Cloud endpoint accepts exactly one variant per request. |
| `provider_account_ref` | Sanitized ChatGPT/Codex provider-account ref. |
| `auth_grant_ref` | Short-lived Vortex grant ref for the provider account. |
| `repository_ref` | Non-secret repo context, if any. |
| `signature_context` | Optional Blueprint/Probe signature ids, package digest, and selector-trace requirement. |
| `codex_adapter` | Codex/Harbor package adapter id, package name, version, and optional digest. |
| `budget` | Timeout, attempt count, and optional max cost. |
| `artifacts` | Retention mode, optional sink ref, and required artifact filenames. |
| `callback` | Non-secret callback ref and sequence policy. The actual callback URL/token still come from `oa-codex-control` env. |

## Runner Behavior

`oa-codex-control` accepts this contract at:

```text
POST /v1/training-runs
POST /v1/training-runs/start
```

The daemon validates the contract, writes `training-assignment.json` into the
local job directory, emits structured initial events, then converts the
assignment into the existing `openagents.codex_workroom_assignment.v1` runner
path.

Initial events:

- `training.assignment.validated`
- `benchmark.package.validated`
- `training.artifact_policy.attached`
- `signature.context.loaded` when signature context is present

The generated Codex prompt contains a fixed Terminal-Bench/Harbor command
shape with the selected dataset, task, variant, model, package version,
attempt limit, and required artifact filenames. Vortex cannot submit arbitrary
shell commands through this path.

## Required Artifacts

The first retained Terminal-Bench assignment should require:

```text
result.md
benchmark-result.json
artifact-manifest.json
proof-bundle.json
```

`result.md` is the human-readable summary. `benchmark-result.json`,
`artifact-manifest.json`, and `proof-bundle.json` are the normalized payloads
Vortex/Probe use to inspect the result, compare variants, and decide whether
to retain or publish evidence.

## Retention Modes

| Mode | Meaning |
| --- | --- |
| `durable_artifacts` | Runner should produce uploadable/retained artifacts and refs. |
| `redacted_only` | Runner should retain redacted summaries and digest refs only. |
| `local_only` | Runner should keep details local to SHC except modeled minimal status/heartbeat events. |

Vortex remains the authority for whether retained artifacts are shown to users,
used as training data, projected publicly, or kept local.

## Validation Rules

- Only `terminal-bench` is accepted for the first implementation.
- `task_ref` must be a registry ref such as `terminal-bench/<task>`.
- Assignment ids, signature ids, package ids, and refs must be bounded
  non-secret strings.
- Task and package digests must be `sha256:` refs or 64 hex chars.
- Artifact names are filenames, not paths.
- Timeout must be positive and no longer than one hour.
- Attempts must be between 1 and 3.
- The runner uses account-backed Codex only. API-key fallback is not an
  accepted path.

Fixture:

- `fixtures/training_run_assignment_v1/terminal-bench-retained.json`
