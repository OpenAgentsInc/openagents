# OpenAgents Cloud Architecture

## Purpose

OpenAgents Cloud is the private managed-node and workroom layer beneath
Autopilot and Forge.

The public contributor path remains Pylon in the open-source OpenAgents tree.
This private repo owns managed nodes and workroom sidecars that implement the
same public contract where useful, while keeping private topology and policy
out of public Pylon.

## Components

```text
Autopilot
  user/org UX, approvals, workroom creation, review, acceptance

Forge
  work intake, template selection, assignment, verification, delivery receipts

Nexus
  node registry, heartbeats, provider state, payout/accounting rails

Pylon
  open-source contributor app implementing public node contract

oa-node
  private managed node daemon for owned/org-owned machines

oa-workroomd
  private per-workroom sidecar for local gateways, ingress, artifacts, receipts

Psionic
  inference, training, sandbox execution, cluster runtime, execution evidence

Treasury
  payer-side settlement, reconciliation, internal accounting

Benchmark Cloud
  bounded benchmark/workload execution lane for Terminal-Bench, SWE-bench,
  custom repo tasks, Model Lab evals, and provider admission tests
```

## Managed Node Responsibilities

`oa-node` owns:

- node registration and identity binding;
- capability inventory and backend readiness;
- lifecycle state, health events, update channel, rollback, and quarantine;
- typed Forge assignment intake and local acceptance/refusal receipts;
- Psionic/Probe worker attachment, scoped workspace policy, and receipt
  projection;
- heartbeat and receipt submission;
- internal settlement or no-wallet mode.

`oa-node` does not own:

- contributor wallet UX;
- public Pylon install/TUI flow;
- user-facing work acceptance;
- hot inference/training/sandbox execution internals.

## Workroom Responsibilities

`oa-workroomd` owns:

- workroom-local metadata;
- link-local gateways for model, artifacts, receipts, memory, email, and
  settlement metadata;
- session-scoped Codex auth materialization from Vortex provider-account grant
  refs into a per-workroom `CODEX_HOME`;
- the first Codex VM runner scaffold: structured assignment intake, private
  no-wallet workspace creation, `codex exec`, normalized events, artifact
  capture, closeout, and cleanup;
- managed preview ingress registration;
- scoped capability attachment;
- artifact closeout;
- workroom receipt submission.

`oa-workroomd` does not own:

- public provider persona;
- contributor wallet authority;
- product authority over ChatGPT/Codex account connection;
- durable storage of raw Codex `auth.json` material;
- open-ended task planning;
- final acceptance authority.

## Codex VM Auth Boundary

For the Codex VM MVP, Vortex owns provider-account connection and issues
short-lived grant refs. Cloud consumes those refs through
`openagents.codex_auth_grant.v1`, resolves secret material only through an
approved broker/secret path, writes VM-local auth material into a session
`CODEX_HOME`, runs `codex login status`, and scrubs the session auth directory
after closeout or failure.

The Cloud receipt layer records provider account refs, grant refs, decisions,
failure reasons, and digests only. It must not record raw access tokens,
refresh tokens, API keys, device auth IDs, code verifiers, `auth.json` content,
wallet material, or broad GCP credentials.

## Codex VM Runner Boundary

`openagents.codex_workroom_assignment.v1` is the temporary Cloud/Probe
compatibility contract for the Codex VM runner. Vortex/Autopilot supplies a
structured assignment with a matching Codex auth grant ref. `oa-workroomd codex
run` verifies the session account with `codex login status`, creates a private
workspace, writes a no-wallet `AGENTS.md`, runs
`codex exec --skip-git-repo-check --json --sandbox <assignment sandbox>`,
captures declared artifacts through the normal artifact
closeout path, emits `openagents.codex_workroom_event.v1` events, removes the
workspace, and scrubs session Codex auth material.

`danger_full_access` is a declared assignment profile for externally isolated
VM workrooms. It is the current SHC real-account Codex profile because Codex
`workspace-write` fails on the nested Katy VPS at the bubblewrap/loopback
layer. Keep the no-wallet boundary and session auth scrub requirements active.

Cloud owns this VM-side lifecycle and redaction. Vortex owns product control,
review, and acceptance. Probe owns the durable coding-agent runtime contract;
the Cloud runner must therefore keep event and receipt shape compatible with
Probe rather than inventing an SSH/operator-only path.

The Artanis/Pylon bootstrap path is a specialized structured assignment over
the same runner. It imports Artanis source refs, Pylon capability labels, and
Blueprint signatures into a private SHC Codex workroom that must produce launch
plans, continual-learning plans, work-order drafts, and proof artifacts. It
does not give Cloud product authority over Artanis or Pylon. Vortex and the
public projection layer remain responsible for operator approval, public-safe
projection, and user-visible mission state.

The MVP event transport is the `oa-codex-control` HTTP callback path into
Vortex. A direct Rust Convex bridge from SHC is deferred until Vortex exposes
runner-scoped service identity and narrow Convex functions for append-only
events, heartbeat, artifact refs, status, and pending commands. See
`docs/control/CODEX_CONVEX_BRIDGE_EVALUATION.md`.

## Benchmark Cloud Boundary

Benchmark Cloud is a Cloud execution lane, not a separate product authority.
Vortex owns benchmark workroom UX, launch authorization, approvals, receipts,
claim state, and public/private projection. Cloud owns bounded execution:
runner images, task attempts, artifact upload, execution events, and closeout
evidence.

The first dataset adapter is Terminal-Bench 2 through a Harbor wrapper. The
control-plane contract stays dataset-neutral so SWE-bench, SWT-Bench, custom
repo tasks, Model Lab evals, and Pylon admission tests can reuse the same
runner and artifact model.

The default GCP shape is:

- Artifact Registry for benchmark runner images;
- Cloud Storage for task specs, transcripts, logs, diffs, results, and proof
  bundles;
- Pub/Sub for task and run events;
- Cloud Run Jobs for importer/controller/aggregation jobs;
- Cloud Batch for isolated task attempts;
- Secret Manager for tightly scoped benchmark credentials.

GKE is deferred until measured compatibility needs, such as nested containers,
long-lived warm pools, GPUs, or sidecars, make Batch and Cloud Run Jobs
insufficient.

See `docs/BENCHMARK_CLOUD.md` for the issue map, artifact contract, and public
claim guardrails.
