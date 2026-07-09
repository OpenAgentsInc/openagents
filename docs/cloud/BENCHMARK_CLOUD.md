# Benchmark Cloud

Status: canonical Cloud execution plan
Last updated: 2026-07-09 (#8591 authority rewrite)

Benchmark Cloud is the reusable benchmark and workload execution lane for
OpenAgents Cloud. Terminal-Bench 2 is the first dataset adapter, but the
product boundary is a general benchmark harness that can later run SWE-bench,
SWT-Bench, custom repo tasks, Model Lab evals, and Pylon provider admission
tests.

## Authority Split

| Area | Owner | Notes |
| --- | --- | --- |
| Product UX, approvals, receipts, public/private projection | openagents.com Worker + Khala Sync | Worker/operator surfaces decide who can launch, inspect, publish, or invalidate claims. |
| Managed execution | Cloud (`crates/*`) | Cloud schedules bounded jobs and returns artifacts, events, and closeout evidence. |
| Coding-agent runtime behavior | Probe / Codex-compatible runner | Cloud may host the runner, but it must not invent product authority. |
| Hot inference/training/sandbox internals | Psionic | Benchmark adapters may call Psionic capabilities through scoped contracts. |
| Public provider admission | Pylon + Worker | Benchmark results can feed capability and trust state after receipts exist. |
| Outbound payout / custody | MDK/Nexus payout bridge | Active money-movement boundary only — not benchmark claim authority. |

Cloud does not become a leaderboard, a public claim authority, or a hidden
labor path. Every benchmark job must have declared inputs, resource limits,
policies, expected artifacts, and receipt semantics before execution.

## Target Flow

```text
Worker / operator Benchmark Workroom / Model Lab
  -> Khala Sync + Worker durable state and Effect control services
  -> Cloud benchmark execution contract
  -> GCP execution backend
     - Artifact Registry for runner images
     - Cloud Storage for task specs, logs, transcripts, diffs, results, proof bundles
     - Pub/Sub for task and run events
     - Cloud Run Jobs for importer/controller/aggregation jobs
     - Cloud Batch for isolated benchmark task attempts
     - Secret Manager for tightly scoped benchmark credentials
```

Cloud receives a normalized task envelope and emits normalized task events,
artifact manifests, and result JSON. The Worker / Khala Sync remain the durable
state and claim-projection authority.

## Normalized Contracts

The first implementation uses a runner-local JSON contract:

- `BenchmarkTask`
- `BenchmarkResult`
- `BenchmarkEvent`
- `BenchmarkArtifactManifest`
- `BenchmarkProofBundle`

Dataset adapters translate native inputs into those contracts:

```text
Terminal-Bench task -> BenchmarkTask -> runner -> verifier -> BenchmarkResult
SWE-bench instance  -> BenchmarkTask -> runner -> tests    -> BenchmarkResult
custom repo issue   -> BenchmarkTask -> runner -> tests    -> BenchmarkResult
```

The normalized layer prevents Terminal-Bench-specific fields from becoming the
Cloud/Worker control-plane model.

Every benchmark or workroom run should also carry
`openagents.resource_usage_receipt.v1` when Cloud can observe the facts. That
receipt records host/device facts, run resource usage, artifact/log sizes, and
model token usage or an explicit unavailable-token reason. Subscription-backed
Codex currently records `count_source=unavailable` with
`subscription_backed_codex_no_token_counts`; product surfaces must not treat missing token
counts as silently complete proof.

The first local implementation lives in `runners/py-bench-runner`. It can run
fake pass, timeout, and exception tasks while always writing `result.json`,
`events.jsonl`, `metadata.json`, `artifact_manifest.json`, and
`proof_bundle.json`. It now also writes `resource_usage_receipt.json` and
`cloud_execution_closeout.json`, then threads both digests into the proof
bundle. That runner is the contract proof that the Cloud Batch and
Terminal-Bench adapters build on.

`cloud_execution_closeout.json` is the Cloud-side closeout gate for the Pylon
launch path. It carries only public-safe refs and digests, sets
`walletAuthority=false`, `payoutAuthority=false`, and
`publicClaimAuthority=false`, and marks `authorityOwner=worker` (historical label `omega` means product authority, not a separate Cloud owner). Cloud can prove
that a bounded SHC/Benchmark task ran and produced artifacts; it does not
approve public claims, settle payouts, or act as a wallet provider.

## Execution Backends

| Backend | Role |
| --- | --- |
| Local runner | Contract smoke tests, fake tasks, timeout/error behavior. |
| Cloud Run Jobs | Dataset import, result aggregation, score recomputation, report generation. |
| Cloud Batch | Default isolated benchmark task executor for Terminal-Bench and SWE-style task attempts. |
| GKE | Deferred until nested containers, long-lived pools, GPUs, or sidecars require it. |

Start with Cloud Batch for task attempts. Do not add GKE until a measured
compatibility gap proves Batch and Cloud Run Jobs are insufficient.

The development GCP substrate is scripted in:

- `scripts/gcp-benchmark-bootstrap.sh`
- `scripts/gcp-benchmark-smoke.sh`
- `scripts/gcp-benchmark-cleanup.sh`
- `scripts/gcp-benchmark-submit-batch.sh`

Runbooks:

- `docs/bootstrap/CND-045-gcp-benchmark-cloud-substrate.md`
- `docs/bootstrap/CND-046-cloud-batch-benchmark-backend.md`
- `docs/bootstrap/CND-047-terminal-bench-harbor-wrapper.md`
- `docs/bootstrap/CND-048-openagents-codex-benchmark-adapter.md`
- `docs/bootstrap/CND-049-swe-custom-repo-benchmark-adapter.md`
- `docs/bootstrap/CND-050-shc-codex-terminal-bench-smoke.md`
- `docs/bootstrap/CND-051-shc-codex-terminal-bench-8task.md`
- `docs/bootstrap/CND-052-shc-codex-terminal-bench-16task-preserved.md`
- `docs/bootstrap/CND-053-terminal-bench-signature-routing-fixtures.md`

Measured SHC smoke:

- `oa-shc-katy-01` ran one official Terminal-Bench 2.0 task through Harbor's
  built-in Codex agent on 2026-06-01. The internal smoke used
  `terminal-bench/openssl-selfsigned-cert`, Codex `0.135.0`, model `gpt-5.5`,
  and temporary ChatGPT Codex auth injected by `CODEX_AUTH_JSON_PATH`. The
  verifier passed 6/6 checks with reward `1.0` and mean `1.000`. Treat this as
  a one-task substrate proof, not a leaderboard or public product claim. See
  `docs/bootstrap/CND-050-shc-codex-terminal-bench-smoke.md`.
- A larger selected 8-task SHC Codex smoke ran afterward against
  `terminal-bench@2.0`. Harbor reported the dataset contains 89 tasks. The
  selected internal batch passed 6/8 tasks with mean reward `0.75` and reported
  model cost `$3.697649`. Treat this as a broader substrate proof, still not a
  full Terminal-Bench score. See
  `docs/bootstrap/CND-051-shc-codex-terminal-bench-8task.md`.
- A preserved selected 16-task SHC Codex run then passed 11/16 tasks with mean
  reward `0.6875` and reported model cost `$13.300340`. Raw Harbor traces,
  verifier outputs, aggregate JSON, per-file checksums, and tarballs remain on
  `oa-shc-katy-01`; the committed report records artifact paths and digests
  without copying raw benchmark secrets or auth material into Git. See
  `docs/bootstrap/CND-052-shc-codex-terminal-bench-16task-preserved.md`.
- The failed retained tasks now have public-safe signature-routing fixtures
  that map failure families to Probe seed signatures. These fixtures preserve
  task checksums, expected evidence, forbidden unrelated signatures, and raw
  Codex baseline summaries without copying hidden verifier details. See
  `docs/bootstrap/CND-053-terminal-bench-signature-routing-fixtures.md`.

## Required Artifacts

Every runner attempt must upload these files even on failure once the runner
starts:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `resource_usage_receipt.json`
- `cloud_execution_closeout.json`

Optional artifacts include:

- `commands.jsonl`
- `transcript.md`
- `agent_stdout.log`
- `agent_stderr.log`
- `verifier_stdout.log`
- `verifier_stderr.log`
- `workspace.diff`
- `patch.diff`
- raw dataset-runner output bundles

Credential material, raw provider tokens, broad environment dumps, customer
secrets, and production topology must not appear in artifacts, logs, events,
docs, or issue comments.

Runner artifact writers redact secret-like text in logs and JSON artifacts
before the manifest and closeout are generated. The redaction gate covers common
provider token, authorization, password, API key, access-token, refresh-token,
private-key, and wallet-seed markers. Secret refs may appear only as
server-side references from the relevant contract layer; raw auth material must
not appear in Benchmark Cloud evidence.

## Terminal-Bench MVP

The first credible demo is:

1. Operator / Worker launches a Terminal-Bench 2 oracle smoke run.
2. Cloud runs one selected task on Google Cloud Batch through the Python runner.
3. The runner wraps Harbor for Terminal-Bench compatibility.
4. Artifacts land in Cloud Storage.
5. Events update Worker / Khala Sync task state.
6. Operator surfaces generate an internal proof bundle.
7. The same task later runs through the OpenAgents/Codex agent adapter for
   oracle-vs-agent comparison.

The first Terminal-Bench lane is a Harbor wrapper implemented by the
`terminal-bench` dataset adapter in `runners/py-bench-runner`. A native
Terminal-Bench adapter can follow after oracle parity is understood.

The first OpenAgents/Codex comparison lane is also implemented in the runner.
It captures command, transcript, Codex stdout/stderr, workspace diff, verifier
placeholder output, and proof-bundle fields while keeping raw Codex auth
material out of artifacts.

The first repository-task lane supports `custom-repo`, `swe-bench`, and
`swt-bench` dataset slugs through the same normalized task/result contract. It
captures transcript, commands, verifier logs, `workspace.diff`, `patch.diff`,
and proof-bundle artifacts without adding dataset-specific product-state coupling.

The first Probe+Codex signature lane uses retained Terminal-Bench fixtures under
`runners/py-bench-runner/fixtures/signature-routing/`. Raw Codex baselines
record `selectionEnabled=false`; `probe-codex` runs record
`signature_selector_trace.json` with the selected Probe signature, required
evidence, closeout artifacts, and forbidden signature ids for comparison.

## Public Claim Guardrails

No benchmark result should become public unless the projected claim includes:

- pinned dataset slug and version;
- disclosed task selector/subset;
- agent, model, and harness version;
- retry, timeout, and budget policy;
- artifact retention state;
- proof bundle digest;
- redaction status;
- verifier/scorer result;
- resource/device receipt and token ledger or explicit unavailable-token
  receipt;
- superseded or invalidated state when applicable.

Modeled cost, online nodes, and runner self-reported summaries are not accepted
benchmark claims by themselves.

Cloud proof bundles remain internal by default. A valid Cloud closeout is
necessary evidence for product claim promotion, but it is not sufficient by
itself: promotion still requires the product authority to verify launch
authorization, claim policy, public/private projection, settlement state, and
any Probe/GEPA campaign gates.

## Issue Map

Cloud issues:

- CND-043: Benchmark Cloud execution lane and Terminal-Bench tracking.
- CND-044: normalized benchmark contracts and local Python runner skeleton.
- CND-045: GCP Benchmark Cloud substrate provisioning.
- CND-046: Cloud Batch benchmark task backend.
- CND-047: Terminal-Bench 2 Harbor wrapper on Cloud Batch.
- CND-048: OpenAgents/Codex benchmark agent adapter and proof bundles.
- CND-049: SWE-bench and custom repo adapter path.
- CND-053: retained Terminal-Bench signature routing fixtures.
- TRAIN-005: SHC retained Terminal-Bench training assignment endpoint through
  account-backed Codex.
- CND-050: resource/device and model token usage receipts for every run.

Historical product-surface companion issues (filed in the deprecated Vortex
repo; the current product surface is the `openagents.com` Worker + Khala Sync
/ Khala Code — route any new companion work to `openagents`):

- OpenAgentsInc/vortex#91: Benchmark Cloud Convex state and Effect control
  service.
- OpenAgentsInc/vortex#92: Terminal-Bench Benchmark Workroom UI and proof
  projection.
