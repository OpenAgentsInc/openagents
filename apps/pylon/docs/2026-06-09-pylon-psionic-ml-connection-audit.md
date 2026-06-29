# Pylon v0.3 Psionic ML Connection Audit

Date: 2026-06-09

Status: audit for connecting standalone `OpenAgentsInc/pylon` v0.3 to the
Psionic ML substrate.

## Source Material Read

Pylon:

- `docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`
- `src/gepa-capability.ts`
- `src/launch-gates.ts`
- `packages/runtime/tests/benchmark-candidate-execution.test.ts`
- `docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md`

Psionic:

- `README.md`
- `AGENTS.md`
- `V0.2_PYLON_RELEASE_AUDIT.md`
- `docs/NON_GPT_OSS_QWEN35_PILOT.md`
- `docs/HERMES_QWEN35_COMPATIBILITY.md`
- `docs/HERMES_BACKEND_BENCHMARK.md`
- `docs/QWEN35_RESPONSES_TOOL_LOOP_PILOT.md`
- `docs/PROBE_GEPA_CANDIDATE_MANIFESTS.md`
- `docs/PROBE_GEPA_ROLLOUT_COORDINATOR.md`
- `docs/MESH_LANE_SERVICE_MODE.md`

## Verdict

Pylon v0.3 cannot honestly claim ML or model-training work until it connects to
Psionic through a typed connector, signed sidecar, or externally managed
Psionic service. The current Pylon guard is correct: GEPA is admitted only as a
benchmark/candidate rollout path, `supportsTraining` is `false`, and the Qwen
track is postponed.

Psionic already owns the ML substrate that Pylon needs:

- model serving and inference lanes;
- training and optimizer jobs;
- eval and benchmark execution contracts;
- GEPA candidate manifests and rollout coordinator;
- Qwen legal Pylon worker/scheduler boundary from the older v0.2 audit;
- artifact identity, retained receipts, and execution evidence.

The missing v0.3 launch work is not "teach Pylon ML from scratch." The missing
work is a clean integration boundary where Pylon can prove a local machine is
available, receive an assignment, invoke Psionic safely when the assignment
requires ML, and return public-safe evidence without claiming more authority
than Psionic or OpenAgents product surface granted.

## Current Gap

Standalone Pylon now owns the contributor node shell, TUI, wallet posture,
presence, local runtime package, and GEPA candidate execution fixtures. It does
not yet own:

- Psionic binary discovery;
- Psionic service discovery;
- Psionic capability negotiation;
- signed Psionic release manifest verification;
- local Psionic sidecar lifecycle;
- content-addressed model artifact download;
- training assignment execution through Psionic;
- Psionic worker receipts imported back into Pylon closeouts;
- launch gates that distinguish GEPA text optimization from neural training.

Without that boundary, Pylon should not advertise that it can train Qwen, run
adapter jobs, sell local inference capacity, or execute model-weight updates.

The separate Qwen inference roadmap is
`docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md`. It narrows the first
inference pass to `qwen3.5:0.8b` and `qwen3.5:2b` through an attach-only
Psionic OpenAI-compatible backend. That roadmap is intentionally inference-only
and does not reopen the postponed Qwen training claim.

## Ownership Split

Pylon should own:

- contributor identity and local config;
- host inventory projection;
- wallet and payout readiness projection;
- OpenAgents registration, heartbeat, assignment lease, progress, and closeout;
- local sandbox policy for work it launches;
- safe artifact and proof references;
- no-spend GEPA benchmark rollout execution;
- Psionic connector state and refusal reasons.

Psionic should own:

- model runtime and backend truth;
- MLX, Metal, CUDA, CPU, and cluster backend semantics;
- GEPA optimizer frontier and candidate manifest production;
- Qwen, Gemma, Apple FM, adapter, SFT, eval, and benchmark ML logic;
- training invocation manifests;
- model artifact manifests;
- worker receipts and retained ML execution evidence;
- promotion/eval rules for model artifacts.

OpenAgents product surface/OpenAgents should own:

- public assignment dispatch;
- lease lifecycle and cancellation;
- public claim projection;
- operator approval;
- payment authorization and settlement receipts;
- AGENTS/OpenAPI/manifest discovery surfaces.

MDK should own wallet mechanics only. MDK does not provide ML execution,
runtime binary distribution, model artifact validation, or training authority.

## GEPA Is The First Valid Bridge

The near-term connection should stay GEPA-first.

Psionic already defines `psionic.probe_gepa_candidate_manifest.v1`. Pylon's
runtime tests already consume that manifest shape and run retained benchmark
candidate execution. Psionic's rollout coordinator can import live OpenAgents product surface/Pylon
closeouts after OpenAgents product surface has produced normalized refs.

That means the first integration can be:

1. Psionic selects or generates a GEPA candidate manifest.
2. OpenAgents product surface assigns a bounded rollout to one or more Pylons.
3. Pylon validates the assignment requirements against
   `openagents.pylon.gepa_capability_envelope.v0.3`.
4. Pylon runs the retained benchmark/candidate path locally.
5. Pylon emits progress, artifact refs, proof refs, and closeout.
6. Psionic imports the closeout into the rollout coordinator.

This is benchmark-driven optimization over text bundles. It is not neural
network training. Launch copy must preserve that distinction.

## Required Psionic Connector Modes

### 1. External Psionic Service

Pylon should support a user- or operator-provided Psionic service URL for
development and private fleets.

Minimum contract:

- `PYLON_PSIONIC_URL` or config equivalent;
- `GET /v1/health`;
- `GET /v1/capabilities`;
- request timeout and retry policy;
- connector status in `pylon status --json`;
- blocker refs when unavailable, unsupported, stale, or incompatible.

This is the fastest path for a controlled v0.3.x integration because it avoids
shipping binaries through npm before the release and signing pipeline is
settled.

### 2. User-Provided Psionic Binary

Pylon should support a local binary override:

- `PYLON_PSIONIC_BIN=/path/to/psionic-sidecar`;
- `pylon psionic doctor`;
- version check;
- capability check;
- optional checksum pin when configured;
- no automatic trust in arbitrary binaries for public paid-work claims.

This keeps power users unblocked while preserving launch honesty. A local
binary can run retained or private work, but public paid/training claims need
attested release identity or an operator-approved exception.

### 3. Signed Psionic Sidecar Download

This should be the recommended public v0.3.x architecture after GEPA smoke.

Pylon should download a small Psionic sidecar only from a signed release
manifest. The npm package should not embed the binary. The sidecar installer
should:

- fetch `psionic.release_manifest.v1`;
- select only macOS and Linux platform triples;
- verify SHA-256;
- verify release signature;
- verify expected package name and channel;
- cache by content digest;
- record release identity in Pylon local state;
- refuse execution if verification fails.

Recommended platform triples for the first public path:

- `darwin-arm64`;
- `linux-x64`;
- `linux-arm64`.

The first public package should not document or claim Windows support.

### 4. Bundled Psionic Binary

Bundling Psionic inside `@openagentsinc/pylon` is not recommended for launch.

Reasons:

- npm package size and platform matrix become unstable;
- Psionic has Rust, backend, MLX, CUDA, Metal, and model-artifact concerns that
  should not be collapsed into the TypeScript TUI package;
- binary security and provenance are harder to audit if npm tarball contents
  become the only release boundary;
- sidecar upgrades should not require republishing the Pylon UI package;
- model weights must never be bundled into Pylon.

Pylon may bundle TypeScript schema validators and connector clients. It should
not bundle Psionic model weights or heavy backend binaries for v0.3.

Current Pylon implementation:

- default package installation does not include Psionic binaries or model
  weights;
- startup does not download or launch Psionic;
- `pylon status --json` projects
  `openagents.pylon.psionic_connector.v0.3` connector state with typed
  `absent`, `configured`, `negotiated`, and `refused` phases; see
  `docs/psionic-connector.md`;
- `pylon psionic install --channel rc --manifest-url <url> --yes` verifies a
  Psionic release manifest and SHA-256 before placing a binary in the
  digest-addressed Pylon cache;
- `pylon psionic models install <model-key> --manifest-url <url> --yes`
  verifies a model artifact manifest and SHA-256 before placing the artifact
  in the digest-addressed Pylon cache;
- unsupported machine, missing consent, missing manifest, digest mismatch,
  memory, disk, and competing-workload failures return blocker refs before
  placement.

The remaining gap is Psionic publication of signed Pylon-consumable release and
model manifests. Until that exists, Pylon requires explicit manifest URLs or env
manifest overrides and does not claim one-command Psionic provisioning.

## Model Artifact Policy

Pylon should never download raw model files just because the app starts.
Artifact download must be assignment- or operator-driven.

Required model artifact gates:

- content-addressed artifact manifest;
- expected model family and backend;
- license and use boundary;
- byte size and disk budget;
- checksum;
- optional signature;
- local cache path under Pylon/Psionic cache, not public projection;
- redacted public artifact refs;
- explicit refusal if the artifact is missing, too large, unsupported, or
  unverified.

Public projections should carry refs and digests, not raw local paths, secrets,
or private topology.

Implemented Pylon installer projections follow
`openagents.pylon.psionic_install.v0.3` and expose only platform refs, backend
refs, artifact refs, digest refs, cache refs, blocker refs, and redaction state.

## Sidecar Security Boundary

The Psionic sidecar must not receive wallet secrets.

Pylon should pass:

- assignment id;
- work class;
- capability token scoped to localhost sidecar calls;
- artifact refs;
- local scratch root;
- budget limits;
- selected model/artifact refs;
- cancellation deadline.

Pylon should not pass:

- MDK mnemonic;
- bearer tokens unrelated to the assignment;
- private OpenAgents operator tokens;
- raw wallet preimages;
- provider account secrets unless a specific provider task explicitly requires
  them and the secret-ref policy admits it.

The sidecar should run with a narrow environment allowlist, local scratch
directory, timeout, memory/disk budget, and kill/cancel path. Pylon closeout
must include the Psionic release identity and receipt digest when a Psionic
task ran.

## Proposed Public Schemas

Pylon should add or consume these schemas before training claims are allowed:

- `openagents.pylon.psionic_connector_status.v0.3`
- `openagents.pylon.psionic_sidecar_release.v0.3`
- `openagents.pylon.psionic_capability_envelope.v0.3`
- `openagents.pylon.psionic_assignment_requirements.v0.3`
- `openagents.pylon.psionic_worker_receipt_ref.v0.3`
- `psionic.release_manifest.v1`
- `psionic.model_artifact_manifest.v1`
- `psionic.training_assignment.v1`
- `psionic.worker_receipt.v1`

Pylon already consumes `psionic.probe_gepa_candidate_manifest.v1` through the
runtime test fixture. That should remain the first concrete bridge.

## Blocker Refs Required

Pylon should expose specific blocker refs instead of vague "offline" or
"unsupported" states:

- `blocker.psionic.connector_unconfigured`
- `blocker.psionic.service_unreachable`
- `blocker.psionic.sidecar_missing`
- `blocker.psionic.sidecar_version_unsupported`
- `blocker.psionic.release_manifest_missing`
- `blocker.psionic.release_signature_unverified`
- `blocker.psionic.checksum_unverified`
- `blocker.psionic.platform_unsupported`
- `blocker.psionic.capability_missing`
- `blocker.psionic.model_artifact_missing`
- `blocker.psionic.model_artifact_unverified`
- `blocker.psionic.sandbox_profile_missing`
- `blocker.psionic.training_authority_missing`
- `blocker.psionic.training_claim_postponed`

These blockers should feed both `pylon status --json` and the TUI telemetry
panel.

## Launch Gate Changes

The existing `claim.pylon.qwen_training` gate is correctly blocked. It should
be widened into a Psionic-backed ML gate family:

| Claim | State until connector lands | Required evidence |
| --- | --- | --- |
| GEPA candidate rollout on Pylon | Allowed only for no-spend retained/canary work | Psionic candidate manifest, Pylon assignment, Pylon closeout, Psionic import receipt |
| Psionic service connected | Blocked | connector status, health response, capability envelope |
| Psionic sidecar installed | Blocked | signed release manifest, checksum, version/capability check |
| Optional Qwen3.5 local inference backend | Blocked until roadmap gates pass | Psionic `/health`, `/v1/models`, 0.8B/2B model refs, chat/tool-call smoke receipts |
| Local inference sellable | Blocked | model artifact manifest, backend readiness, pricing/lease/payment gate |
| Qwen training on devices | Blocked/postponed | Psionic training assignment, sidecar release identity, model artifact gate, sandbox gate, worker receipt, closeout import |
| Adapter/SFT worker | Blocked | training assignment schema, artifact lineage, eval gate, receipt import |

## Implementation Sequence

1. Add Psionic connector status schemas and status projection to Pylon.
2. Add `pylon psionic doctor` for unconfigured, service, and binary modes.
3. Add fake Psionic sidecar tests that stream status and emit a receipt.
4. Wire GEPA assignments to carry optional Psionic candidate manifest refs.
5. Add Psionic closeout receipt refs to Pylon artifact/proof bundles.
6. Publish Psionic signed release/model manifests for Pylon consumption.
7. Wire default manifest discovery to the existing Pylon installer.
8. Add sidecar process supervision once release identity exists.
9. Add launch gate blockers for every unsupported ML/training claim.
10. Add the attach-only Qwen3.5 0.8B/2B inference backend described in
    `docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md`.
11. Only after separate training gates pass, re-open Qwen/training assignment
    support.

## Copy Rules

Allowed now:

- "Pylon v0.3 can run GEPA-first retained benchmark candidate work."
- "Pylon consumes Psionic GEPA candidate manifests."
- "Pylon has a roadmap for optional Psionic Qwen3.5 0.8B and 2B local
  inference once attach, model, and tool-call gates pass."
- "Qwen and model-training work are postponed until the Psionic connector and
  sidecar gates pass."

Blocked now:

- "Pylon trains Qwen on people's devices."
- "Pylon v0.3 sells local ML capacity live."
- "Pylon downloads models automatically."
- "MDK handles ML runtime capacity."
- "The npm package includes the Psionic ML stack."
- "Qwen local inference is live on Pylons before the Psionic Qwen roadmap
  gates pass."

## Conclusion

Pylon should connect to Psionic through a staged connector, not by absorbing
Psionic into the TypeScript app. The public v0.3 release candidate should keep
GEPA as the first bridge and continue blocking all neural training claims. The
next real work is to add connector status, doctor checks, fake sidecar tests,
signed sidecar release manifests, and model artifact gates. Once those exist,
Pylon can safely advertise Psionic-backed ML work with machine-checkable
evidence instead of implied capability.
