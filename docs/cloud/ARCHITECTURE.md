# OpenAgents Cloud Architecture

Status: **active** (rewritten 2026-07-09 for #8591, managed-sandbox plan added 2026-07-19)

OpenAgents Cloud is the managed-node and workroom execution layer for Agent
Computers, placement, capacity, and redacted receipts. Implementation lives
**in this monorepo** under `crates/*` (see `docs/cloud/MIGRATION.md`).

Contributor Pylon stays open source in `apps/pylon`. Cloud keeps private
topology and live secrets out of git (Secret Manager / runtime env only).

## Components

```text
openagents.com Worker
  admission, billing, credit ledgers, public projection, product promises

Khala Sync
  durable event/sync authority for thread-scoped runtime events and receipts

Aiur / operator consoles
  owner-facing operator UX over Worker + Sync surfaces

Forge
  work intake, template selection, assignment, verification, delivery receipts

Pylon (apps/pylon)
  open-source contributor app + local runtime; software inside Agent Computer images

oa-codex-control (crates/oa-codex-control)
  HTTP control plane: placement, Codex runs, fake/live GCE capacity, Cloud-VM

oa-node (crates/oa-node)
  managed node daemon for owned/org-owned machines

oa-workroomd (crates/oa-workroomd)
  per-workroom sidecar for gateways, ingress, Codex auth grants, artifacts, receipts

ManagedSandboxService / OpenAgentsSandboxV1 (planned under #9023)
  owner-scoped lifecycle and runtime authority over existing GCP/workroom seams

Box v1 compatibility facade (planned under #9023)
  bounded HTTP projection for a pinned development-only SDK conformance client

Psionic
  inference, training, sandbox execution, cluster runtime, execution evidence

Probe
  durable coding-agent runtime contract and evidence helpers

MDK / Nexus payout bridge
  currently active outbound payout / custody boundary only — not registry,
  product UX, or durable product-state authority

Benchmark Cloud
  bounded benchmark/workload execution lane (Terminal-Bench, SWE-style, etc.)
```

### Historical names (do not treat as current authority)

| Deprecated name | Current authority |
| --- | --- |
| Vortex | `openagents.com` Worker + Khala Sync (+ product UX on those surfaces) |
| Autopilot (as Cloud control authority) | Worker / Khala Code product surfaces. Autopilot product apps are separate |
| Nexus (as node registry / product state) | Worker / Khala Sync for state. **MDK/Nexus bridge only for active payout** |
| Treasury (as settlement product) | Private metering/receipts in Cloud. Customer credits on Worker. Payout on MDK/Nexus bridge |
| Convex (as product durable store for Cloud events) | Khala Sync / Worker ingest paths |

## Managed Node Responsibilities

`oa-node` owns:

- node registration and identity binding.
- capability inventory and backend readiness.
- lifecycle state, health events, update channel, rollback, and quarantine.
- typed Forge assignment intake and local acceptance/refusal receipts.
- Psionic/Probe worker attachment, scoped workspace policy, and receipt
  projection.
- heartbeat and receipt submission.
- internal-accounting or no-wallet settlement **mode labels** (metadata refs only).

`oa-node` does not own:

- contributor wallet UX.
- public Pylon install/TUI flow.
- user-facing work acceptance.
- hot inference/training/sandbox execution internals.
- customer credit ledgers or public claim promotion (Worker).

## Workroom Responsibilities

`oa-workroomd` owns:

- workroom-local metadata.
- link-local gateways for model, artifacts, receipts, memory, email, and
  settlement **metadata** (never wallet seeds or raw keys).
- session-scoped Codex auth materialization from **grant refs**
  (`openagents.codex_auth_grant.v1`) into a per-workroom `CODEX_HOME`.
- Codex VM runner: structured assignment intake, private no-wallet workspace,
  `codex exec`, normalized events, artifact capture, closeout, and cleanup.
- managed preview ingress registration.
- scoped capability attachment.
- artifact closeout.
- workroom receipt submission.

`oa-workroomd` does not own:

- public provider persona.
- contributor wallet authority.
- product authority over ChatGPT/Codex account connection (Worker / account
  surfaces issue grants. Cloud consumes refs only).
- durable storage of raw Codex `auth.json` material.
- open-ended task planning.
- final acceptance authority.

## Managed Sandbox Boundary

Epic [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023) and the
[`managed-sandbox ProductSpec`](../../specs/openagents/managed-agent-sandboxes.product-spec.md)
admit a planned `SandboxResource` over this existing control plane. The
resource binds owner, tenant, work unit, sandbox, generation, target, image,
lease, budget, capabilities, runtime turns, and receipts before mutation. Its
lifecycle keeps lease, guest, filesystem checkpoint, ingress, and runtime-turn
facts separate.

The admitted runtime may be a GCE VM or a Firecracker microVM and must report
that isolation unit honestly. It is not necessarily an OCI container. A live
managed request cannot fall back to fake/local execution, and readiness or
cleanup requires observed target evidence rather than a control-plane label.

OpenAgents serves a default-off bounded Box v1 facade at `/v1` from the Worker
so the unmodified pinned TypeScript SDK can exercise the admitted lifecycle,
prompt/events, files, commands, and artifacts surface. The adapter
authenticates an OpenAgents programmatic bearer. It resolves the linked owner
and derived tenant. It delegates each lifecycle and cursor operation to the
native Postgres authority.

SBX-04 connects runtime turns through the native Postgres turn/event authority
and the private, default-off `oa-codex-control` SDK-driver adapter. Dispatch
admission, reconnect, ordered text/tool/usage events, exact-turn interrupt,
and structural terminal receipts are implemented. Silence does not cause
completion.

SBX-05 connects guest file, command, and artifact calls through a private,
default-off control adapter. Root confinement, symlink proof, secret scans,
quotas, process-tree closure, deny-all egress, and content-addressed artifact
receipts remain below the facade.

The facade is lossy compatibility only:
native Effect Schema contracts, authorization, event cursors, private
evidence, usage, cost, artifact, and cleanup receipts remain authoritative.
Unsupported methods fail explicitly. No Box type enters the canonical Cloud
contract, and no client receives GCP credentials or vendor control-plane
authority. Local fake, loopback HTTP SDK, and turn-driver component tests are
implementation evidence. Only SBX-09 may arm or claim owner-gated live GCP and
provider acceptance.

Desktop and Sarah consume the same managed-sandbox broker. Mobile and web
receive bounded projections only. This boundary does not admit portable host
movement or cross-machine `FullAutoRun`. Both retain their separate product
and assurance gates.

## Codex Auth Boundary

Provider-account connection and short-lived grant issuance are product-side
concerns (Worker / account brokers). Cloud consumes grant refs through
`openagents.codex_auth_grant.v1`, resolves secret material only through an
approved broker/secret path, writes VM-local auth into a session `CODEX_HOME`,
runs `codex login status`, and scrubs the session auth directory after closeout
or failure.

The Cloud receipt layer records provider account refs, grant refs, decisions,
failure reasons, and digests only. It must not record raw access tokens,
refresh tokens, API keys, device auth IDs, code verifiers, `auth.json` content,
wallet material, or broad GCP credentials.

## Codex VM Runner Boundary

`openagents.codex_workroom_assignment.v1` is the Cloud/Probe compatibility
contract for the Codex VM runner. Product surfaces supply a structured
assignment with a matching Codex auth grant ref. `oa-workroomd codex run`
verifies the session account, creates a private workspace, writes a no-wallet
`AGENTS.md`, runs
`codex exec --skip-git-repo-check --json --sandbox <assignment sandbox>`,
captures declared artifacts, emits `openagents.codex_workroom_event.v1` events,
removes the workspace, and scrubs session Codex auth material.

`danger_full_access` is a declared assignment profile for externally isolated
VM workrooms. Keep the no-wallet boundary and session auth scrub requirements
active.

Cloud owns this VM-side lifecycle and redaction. Product control, review, and
acceptance stay on Worker / operator surfaces. Probe owns the durable
coding-agent runtime contract. Cloud keeps event and receipt shape compatible
with Probe.

Event transport for control-plane runs is the `oa-codex-control` HTTP path with
optional Worker / Khala Sync ingest of public-safe events and receipt refs.
A direct runner→database bridge is not required for the MVP.

## Benchmark Cloud Boundary

Benchmark Cloud is a Cloud execution lane, not a separate product authority.
Product UX, launch authorization, approvals, receipts, claim state, and
public/private projection are owned by the Worker / Khala Sync / operator
surfaces. Cloud owns bounded execution: runner images, task attempts, artifact
upload, execution events, and closeout evidence.

The first dataset adapter is Terminal-Bench 2 through a Harbor wrapper. The
control-plane contract stays dataset-neutral so SWE-bench, SWT-Bench, custom
repo tasks, Model Lab evals, and Pylon admission tests can reuse the same
runner and artifact model.

Default GCP shape (project ids and tokens from env/flags only):

- Artifact Registry for benchmark runner images.
- Cloud Storage for task specs, transcripts, logs, diffs, results, and proof
  bundles.
- Pub/Sub for task and run events.
- Cloud Run Jobs for importer/controller/aggregation jobs.
- Cloud Batch for isolated task attempts.
- Secret Manager for tightly scoped benchmark credentials.

See `docs/cloud/BENCHMARK_CLOUD.md` for the issue map, artifact contract, and
public claim guardrails.
