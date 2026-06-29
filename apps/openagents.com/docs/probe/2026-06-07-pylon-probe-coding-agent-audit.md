# Pylon Probe Coding Agent Audit

Date: 2026-06-07
Status: audit and implementation plan
Scope: `openagents`, `probe`, `openagents/apps/pylon`, and historical `deprecated/openagents.com` Pylon workload source material

## Executive Summary

Probe is already the runtime that should own coding-agent execution: session
lifecycle, backend/model selection, tool calls, approvals, child sessions,
runtime events, local artifacts, and replayable transcripts. Pylon already has
local code to advertise a `probe_agent` capability and invoke Probe through a
signed bridge. OpenAgents product surface already has product-side contracts for Probe run
projection, Pylon registration/status intake, managed-machine projection,
workroom sidecars, and provider settlement evidence.

The missing piece is the active broker between those worlds. OpenAgents product surface does not yet
expose the machine-facing Pylon workload API that a Pylon can poll to claim a
coding assignment, and it does not yet persist Probe managed-runtime events as
the authoritative product view of a coding run. Today, Pylon's runnable Probe
path is wired against the older Laravel-style `/api/pylon/workloads/*` broker
shape, while OpenAgents product surface's current Pylon API is a registration and progress surface.

The intended architecture should make OpenAgents product surface the product authority and Probe the
runtime authority:

- OpenAgents product surface decides which coding work exists, which account/team/workroom it belongs
  to, what policy applies, which Pylon is eligible, and when work is accepted.
- Pylon advertises local capability and environment facts, claims assigned work,
  launches the local Probe runtime, and forwards safe runtime events.
- Probe executes the coding task through its managed runtime, emits website-safe
  events, enforces tool policy and approval boundaries, and keeps raw runtime
  material local unless exported as a safe artifact reference.
- Pylon provider settlement remains evidence-only until accepted work and payout
  authority are explicitly recorded by the settlement layer.

Short term, OpenAgents product surface can support the existing Pylon `probe_agent` bridge so Pylons
can launch Probe for coding assignments. Medium term, the bridge should move
from the admin-chat signed command into Probe's scheduled-agent or managed
runtime contracts, because those contracts express durable sessions, approvals,
replay, child sessions, and environment selection directly.

## Current Ownership Boundaries

### Probe

Probe owns coding-agent runtime behavior. Its repo defines the CLI/server,
session lifecycle, protocol/event model, tool execution, permission and approval
policy, persistence/recovery, detached sessions, managed runtime API, scheduled
agent bridge, website-safe event export, managed environment matching, child
sessions, and backend adapters.

Probe is not the openagents.com product authority. It should not decide account
entitlements, team policy, workroom acceptance, provider payout, or settlement.
It should accept a bounded runtime request, execute within the provided policy,
emit safe events, and preserve raw runtime traces in Probe-local storage.

Important Probe contracts:

- `probe.admin_chat_bridge.signed.v1`: signed internal bridge used by the
  current Pylon implementation to submit a turn into Probe.
- `probe.website_event.v1`: redacted website-safe runtime event batch.
- `probe.scheduled_agent_bridge.v1`: recurring or autonomous work request
  contract that already names Pylon devices as possible execution targets.
- `probe.managed_runtime.v1`: durable start/resume/interrupt/cancel/approval
  and replay contract for managed coding-agent sessions.
- `probe.managed_environment.v1`: provider-neutral worker capability and
  constraint contract, including `pylon` provider and `pylon_device` host class.

### Pylon

Pylon owns the local worker/node boundary. It knows what is installed on the
machine, which local workspaces are allowed, whether the Probe CLI and signed
bridge are available, which backend profile is configured, and whether a bridge
secret is present without exposing the secret.

Pylon should not become the product scheduler. It should claim work assigned to
its registered identity, run the local runtime, and report events and terminal
evidence back to OpenAgents product surface.

The current Pylon implementation has two coding capabilities:

- `codex_agent`: legacy/current Codex CLI runner capability.
- `probe_agent`: Probe CLI runner capability, using the `pylon_probe` web mode
  and the `pylon_probe_signed_bridge` transport.

For Probe, Pylon checks:

- Probe is enabled in Pylon config.
- The `probe` binary is installed.
- `probe admin-chat-bridge signed --help` is available.
- `PROBE_ADMIN_CHAT_BRIDGE_SECRET` or configured secret env is present and at
  least 32 bytes.
- The configured Probe backend profile is supported.
- At least one workspace mapping is configured or inherited from Codex
  workspaces.

If all checks pass, Pylon advertises a ready `probe_agent` capability with safe
metadata such as bridge schema, backend profile, bridge availability, and
workspace count.

### OpenAgents product surface

OpenAgents product surface is the active `openagents.com` product surface. It owns the user-facing
product, admin/operator API boundaries, Pylon registration/status API,
projection contracts, workroom and managed-machine product views, and
settlement evidence gates.

Today OpenAgents product surface has several important contracts but no active Probe workload broker:

- Probe coding runtime projection contract:
  `workers/api/src/probe-coding-runtime-contract.ts`
- Pylon provider settlement evidence contract:
  `workers/api/src/pylon-settlement-bridge.ts`
- Pylon registration/status/progress API:
  `workers/api/src/pylon-api.ts` and `workers/api/src/pylon-api-routes.ts`
- `oa-node` managed-machine projection:
  `workers/api/src/oa-node-managed-machine.ts`
- Workroom sidecar projection:
  `workers/api/src/oa-workroomd-sidecar-contract.ts`
- Runner gateway contracts for `cloudflare_container`, `gcloud_vm`, and
  `shc_vm`, but not yet `pylon` or `probe` as an executable backend.

OpenAgents product surface's current Pylon API records registered Pylons, heartbeat/status refs,
wallet readiness refs, payout target admission refs, assignment progress refs,
artifact proof refs, payment receipt refs, and settlement status refs. It
intentionally does not dispatch paid work, approve payout targets, spend
bitcoin, settle providers, or run coding agents.

### Historical Laravel Source

The deprecated Laravel `openagents.com` clone contains the closest historical
implementation of the missing broker. It exposes machine-facing routes:

- `GET /api/pylon/workloads/next`
- `GET /api/pylon/workloads/{assignment}`
- `POST /api/pylon/workloads/{assignment}/events`
- `POST /api/pylon/workloads/{assignment}/complete`

It also has a `ProbeBridgeAdapter` that creates coding-agent runs with
`runner_kind=probe`, selects a ready Pylon with `capability_key=probe_agent`,
creates a Pylon assignment using `mode=pylon_probe`, and leaves actual runtime
execution to the Pylon worker.

That source should be treated as reference material only. New implementation
belongs in OpenAgents product surface.

## How Probe Works Today

Probe provides multiple surfaces for coding-agent execution:

- `probe exec` for non-interactive command execution.
- `probe chat` and `probe tui` for interactive local operation.
- `probe-server` and `probe-daemon` for long-running service modes.
- Detached sessions and website/admin bridge support.
- Managed runtime/environment contracts for hosted or delegated sessions.
- Scheduled bridge support for recurring autonomous work.
- Backend profiles including OpenAI Codex subscription, Psionic inference mesh,
  local Apple FM bridge, and Qwen registry/oracle profiles.

The website-facing contracts are intentionally redacted. Probe's
`probe.website_event.v1` event vocabulary includes run lifecycle events, text
deltas, tool call summaries, approval requests/resolutions, child-session
updates, artifact refs, runtime progress, and terminal events. The contract
forbids model keys, bearer tokens, refresh/access tokens, raw local paths, raw
tool args/output, assignment nonces, and other unsafe material.

The managed runtime API is a better long-term fit for OpenAgents product surface because it supports:

- Start, resume, interrupt, cancel, approval resolution, replay, heartbeat, and
  child-session recording operations.
- Durable runtime event logs under Probe home.
- Explicit separation between product truth and runtime truth.
- Event replay after website disconnects or Pylon restarts.
- Approval-paused and child-session states.

The managed environment contract is also a good fit for Pylon selection. Probe
already models a `pylon` provider with a `pylon_device` host class, restricted
network egress, existing checkout working directories, persistent package cache,
persistent volume, on-demand checkpointing, labels, backend profiles, language
and tool capabilities, and structured compatibility reasons.

## How Pylon Uses Probe Today

In the current Pylon code, a Probe-capable Pylon has `PylonProbeConfig` fields
for:

- enabled flag
- `probe_bin`
- optional `probe_home`
- bridge secret env name
- backend profile
- provider mode
- workspace mappings
- workload timeout
- cancel-poll interval

Pylon health detection runs the Probe binary, confirms the signed bridge command
exists, inspects secret readiness, checks backend-profile allowlists, and
confirms workspace mappings. If ready, Pylon advertises `probe_agent`.

When Pylon receives a Probe assignment, it:

1. Verifies the assignment capability is `probe_agent`.
2. Verifies the mode is empty or `pylon_probe`.
3. Checks current Probe health.
4. Checks the prompt is non-empty.
5. Maps the requested workspace to a configured local workspace.
6. Builds a Probe admin-chat bridge request with provider mode `pylon_probe`,
   backend profile metadata, workroom/conversation/run IDs, account references,
   and an approval-required tool policy.
7. Signs the request using HMAC-SHA256 with purpose
   `probe-admin-chat-bridge-v1`, key id `openagents-pylon`, short expiry, and
   nonce.
8. Executes:

   ```sh
   probe admin-chat-bridge signed --request <path> --secret-env <env> --cwd <workspace> --format json
   ```

9. Optionally passes `--probe-home`.
10. Streams projected events back to the workload broker.
11. Deletes the temporary signed request file.

Pylon maps Probe output into workload events such as:

- `run.status` running/succeeded/failed
- `probe.session.accepted`
- `assistant.delta`
- `pylon.error`
- generic `probe.event`

Current limitation: the Probe admin-chat bridge accepts the signed request into
a real Probe session and appends the first turn, but the current bridge result
is primarily an acceptance result. It is not yet the full managed-runtime
execution stream. Pylon currently compensates by marking success when no
terminal event is present. That is acceptable for early plumbing, but it is not
strong enough for product acceptance, user-visible completion, or provider
settlement.

## How OpenAgents product surface Works Today

OpenAgents product surface has the shape of the product contract but not the active assignment loop.

The Probe coding runtime contract defines safe run requests, turn events, tool
summaries, run records, and projections. It also encodes important policy:

- Runs have explicit statuses such as queued, running, succeeded, failed,
  cancelled, timed out, needs context, needs review, and retained failure.
- Terminal runs require evidence.
- Retained-failure states require retained failure refs.
- Unsafe material is rejected or redacted, including raw logs, provider payloads,
  credentials, local paths, private repo refs, raw source, wallet/payment/payout
  material, and raw timestamps.

The Pylon API currently supports registration and status/progress ingestion. It
is suitable for knowing that a Pylon exists and has reported readiness or
assignment progress refs, but it does not yet provide:

- a durable coding work-order table,
- a Pylon assignment table,
- a signed claim endpoint,
- a workload nonce ledger,
- an event ingestion path equivalent to `/api/pylon/workloads/{assignment}/events`,
- a completion endpoint,
- a Probe runtime event replay endpoint,
- approval/cancel/control endpoints for active Probe sessions.

The settlement bridge is deliberately separate. It records evidence such as
assignment refs, wallet readiness refs, buyer payment evidence, accepted-work
refs, reward intent, payout eligibility, payout dispatch, confirmation,
verification, and settlement refs. It does not grant spend authority or mutate
settlement by itself. This boundary should be preserved when Probe work starts
flowing through Pylons.

## Target Architecture

The target state is a Probe-first Pylon coding-work pipeline:

1. A Pylon registers with OpenAgents product surface.

   The registration includes stable Pylon identity, safe capability refs, and
   eventually structured capability snapshots for `probe_agent`, including
   backend profile, bridge/runtime schema support, workspace scopes, language
   and tool support, trust level, and managed environment advertisement refs.

2. OpenAgents product surface creates a coding work order.

   The work order belongs to a team/account/workroom/program run. It includes
   objective refs, source authority refs, policy refs, workspace scope, trust
   level, backend profile constraints, approval policy, idempotency refs, and
   correlation refs. It should not include raw secrets, raw repo credentials,
   raw local paths, or wallet material.

3. OpenAgents product surface selects an eligible Pylon.

   Selection should use typed capability/environment data, not ad hoc keyword
   matching. The selector should compare required capability, provider kind,
   host class, backend profile, workspace scope, trust level, network policy,
   tool/language requirements, resource requirements, and freshness of Pylon
   heartbeat. Probe's managed-environment compatibility model is the right
   conceptual template.

4. OpenAgents product surface creates a Pylon assignment.

   The assignment binds the work order to a Pylon identity and a capability key
   such as `probe_agent`. It carries a mode such as `pylon_probe`, sequence
   state, expiration, cancellation state, idempotency key, and opaque safe refs
   for grants or source authority. It should be claimable only by the assigned
   Pylon.

5. Pylon claims the assignment.

   Pylon should prove its identity using the existing NIP-98/account-link
   mechanism or a typed successor. The broker returns a bounded assignment
   payload and a short-lived clear nonce for event/completion calls. Replay
   protection must persist proof IDs and reject duplicate proof/nonce use.

6. Pylon launches Probe.

   In the compatibility phase, Pylon can continue using:

   ```sh
   probe admin-chat-bridge signed --request <path> --secret-env <env> --cwd <workspace> --format json
   ```

   The request must be signed with the local bridge secret, have a short TTL,
   and contain only safe product refs and runtime policy. Longer term, Pylon
   should call Probe through `probe.scheduled_agent_bridge.v1` or
   `probe.managed_runtime.v1` so full session lifecycle, approvals, replay, and
   child sessions are represented natively.

7. Probe executes and emits website-safe events.

   Probe should own model/backend calls, tool execution, local file operations,
   approvals, artifacts, child sessions, and transcript storage. It should emit
   `probe.website_event.v1` batches or equivalent managed-runtime event batches
   to Pylon for forwarding.

8. Pylon forwards events to OpenAgents product surface.

   OpenAgents product surface persists redacted Probe events, updates the `OpenAgentsProbeRunRecord`,
   exposes public/operator/team projections, and broadcasts UI updates. Raw
   stdout, raw model payloads, local paths, secrets, and wallet material should
   be rejected at the boundary.

9. OpenAgents product surface controls approvals and cancellation.

   The product surface needs endpoints for approval listing, approve/reject,
   cancel, replay, and child-session state. These operations should flow to
   Probe through Pylon and preserve idempotency.

10. OpenAgents product surface records acceptance separately from completion.

    A Probe run can finish successfully without being accepted as product work.
    Acceptance should require product-side review or policy. Settlement should
    consume accepted-work refs and payout eligibility refs, not raw Probe
    success events.

## Compatibility Bridge Versus Long-Term Runtime

The current Pylon Probe path is useful and should be supported first because it
already exists and can get local Pylons launching Probe quickly. However, it
should be treated as a compatibility bridge.

The compatibility bridge is good for:

- validating Pylon capability advertisement,
- proving signed local runtime invocation,
- testing workspace mapping,
- testing event ingestion,
- exercising safe projection/redaction,
- running early internal coding tasks with explicit review.

It is weak for:

- durable run replay,
- long-running coding sessions,
- approval-paused execution,
- user-visible terminal status,
- child-session orchestration,
- cancellation across restarts,
- accepted-work and settlement evidence.

The long-term runtime should use Probe managed-runtime or scheduled-agent
contracts because those are explicitly designed for durable delegated agent
work. OpenAgents product surface should avoid inventing a second Probe runtime protocol inside the
Pylon broker.

## Implementation Plan For OpenAgents product surface

### Phase 1: Workload Broker Skeleton

Add an OpenAgents product surface Worker service and D1 schema for Pylon coding work:

- coding work orders,
- Pylon assignments,
- assignment claim state,
- event sequence ledger,
- nonce/proof replay ledger,
- completion state,
- cancellation state,
- safe artifact refs,
- Probe run record refs.

Expose machine-facing routes equivalent to the historical broker shape:

- `GET /api/pylon/workloads/next`
- `GET /api/pylon/workloads/{assignmentRef}`
- `POST /api/pylon/workloads/{assignmentRef}/events`
- `POST /api/pylon/workloads/{assignmentRef}/complete`

The route names can change if OpenAgents product surface has a preferred API namespace, but Pylon
currently expects this shape.

### Phase 2: Capability Model And Selector

Extend OpenAgents product surface's Pylon registration/progress model from generic refs into a
structured safe capability snapshot for coding selection. The selector should
match:

- `capability_key=probe_agent`,
- mode `pylon_probe`,
- Probe bridge or managed-runtime schema support,
- backend profile,
- workspace scope,
- trust level,
- network policy,
- language/tool requirements,
- resource constraints,
- Pylon liveness and registration status.

This selector should be typed and testable. It should not route based on prompt
strings or free-form keyword matching.

### Phase 3: Probe Event Ingestion

Map Pylon-forwarded Probe events into OpenAgents product surface's
`OpenAgentsProbeRunRecord`/`OpenAgentsProbeRunProjection` contract. The mapping
should preserve:

- run lifecycle,
- assistant text deltas,
- tool call summaries,
- approval requests/resolutions,
- child-session refs,
- artifact refs,
- terminal evidence,
- retained failure refs,
- safe cost/model/resource refs when available.

Add redaction tests that reject unsafe payloads before persistence.

### Phase 4: Control Plane

Add product-side controls:

- cancel run,
- list approvals,
- approve/reject approval,
- replay runtime events,
- inspect child sessions,
- mark needs review,
- record accepted-work refs.

The control plane should target the assigned Pylon and Probe session. If Pylon
is offline, OpenAgents product surface should record the desired terminal/control state and surface
the inability to deliver it rather than pretending the runtime stopped.

### Phase 5: Managed Runtime Upgrade

Move from the admin-chat signed bridge to Probe managed-runtime or
scheduled-agent bridge:

- use `probe.managed_runtime.v1` for direct start/resume/control/replay,
- use `probe.scheduled_agent_bridge.v1` for scheduled recurring work,
- keep `probe.website_event.v1` as the product-safe event export shape,
- preserve compatibility with the current Pylon CLI until all deployed Pylons
  advertise managed-runtime support.

### Phase 6: Acceptance And Settlement Integration

After Probe run completion, OpenAgents product surface should create accepted-work refs only through
the product acceptance path. The Pylon settlement bridge can then consume those
refs with buyer payment evidence, reward intent, payout eligibility, dispatch,
confirmation, and verification refs.

No code path should treat a successful Probe exit, a Pylon heartbeat, or an
artifact upload as payout authority.

## Gaps And Risks

1. OpenAgents product surface has no Pylon workload broker yet.

   The active OpenAgents product surface API can register Pylons and record progress refs, but Pylon
   currently polls a broker shape that exists in the deprecated Laravel clone,
   not OpenAgents product surface.

2. Pylon's current Probe bridge can overstate completion.

   The current bridge accepts the Probe session/turn. Pylon may project success
   when no terminal event appears. OpenAgents product surface must not use that as accepted work or
   settlement evidence.

3. There are overlapping contracts.

   Probe defines website events, managed runtime, scheduled bridge, managed
   environment, and admin-chat bridge. OpenAgents product surface defines Probe run projection. The
   historical Laravel broker defines assignment/event/completion shape. The
   first OpenAgents product surface implementation should explicitly map these rather than creating
   another incompatible protocol.

4. Pylon capability data is not yet structured enough in OpenAgents product surface.

   OpenAgents product surface stores capability refs today. Scheduling Probe coding work needs a
   typed snapshot of `probe_agent` readiness, backend profile, workspace scope,
   runtime schema support, and policy constraints.

5. Workspace authorization is a hard boundary.

   Pylon maps assignment workspace scopes to configured local workspaces. OpenAgents product surface
   must not infer workspace or repository access from the prompt. It should pass
   bounded workspace refs and source authority refs.

6. Secrets must remain local or grant-scoped.

   `PROBE_ADMIN_CHAT_BRIDGE_SECRET`, Probe backend credentials, account tokens,
   repo credentials, and wallet material must not be written to D1 event rows,
   public projections, logs, or docs.

7. Approvals are not optional for coding writes.

   Probe and Pylon both model approval-required tool policy. OpenAgents product surface needs product
   surfaces for approval requests/resolutions before it can safely support
   autonomous file writes, shell commands, network actions, or pull requests.

8. Cancellation and offline behavior need durable state.

   Pylons are local machines and can go offline. OpenAgents product surface must record cancellation
   intent and terminal state separately from delivery confirmation.

9. Settlement must stay decoupled.

   Pylon provider rewards need accepted-work refs and settlement evidence, not
   raw Probe terminal events.

## Concrete OpenAgents product surface Deliverables

The next implementation should add:

- D1 migrations for coding work orders, Pylon assignments, event sequences,
  proof/nonce replay, and completion state.
- Worker route handlers for Pylon workload claim, status, events, and
  completion.
- A typed `probe_agent` capability snapshot model.
- A typed Pylon selector based on capability/environment constraints.
- Mapping from Pylon-forwarded Probe events to `OpenAgentsProbeRunRecord`.
- Redaction tests for Probe/Pylon event ingestion.
- Compatibility fixtures based on current Pylon `probe_agent` output.
- A runbook for configuring a local Pylon with Probe and pointing it at OpenAgents product surface.
- Control endpoints for cancellation, approval resolution, event replay, and
  accepted-work recording.

After those are in place, OpenAgents product surface can enable early internal Pylons to deploy Probe
for coding with product-visible evidence while keeping runtime authority,
product authority, and settlement authority separate.

## Source References

Probe:

- `/Users/christopherdavid/work/probe/README.md`
- `/Users/christopherdavid/work/probe/docs/91-openagents-com-admin-chat-bridge.md`
- `/Users/christopherdavid/work/probe/docs/92-website-safe-runtime-events.md`
- `/Users/christopherdavid/work/probe/docs/93-scheduled-agent-bridge-contract.md`
- `/Users/christopherdavid/work/probe/docs/94-managed-runtime-api.md`
- `/Users/christopherdavid/work/probe/docs/95-managed-environment-contract.md`
- `/Users/christopherdavid/work/probe/docs/100-managed-child-sessions.md`
- `/Users/christopherdavid/work/probe/docs/101-openagents-coder-runtime-adapter.md`
- `/Users/christopherdavid/work/probe/crates/probe-protocol/src/managed_environment.rs`
- `/Users/christopherdavid/work/probe/crates/probe-core/src/managed_environment.rs`

Pylon:

- `/Users/christopherdavid/work/openagents/apps/pylon/src/lib.rs`
- `/Users/christopherdavid/work/openagents/apps/pylon-tui/src/lib.rs`
- `/Users/christopherdavid/work/openagents/docs/pylon/PYLON_ACCOUNT_LINKING_NIP98.md`
- `/Users/christopherdavid/work/openagents/docs/pylon/PYLON_VERIFICATION_MATRIX.md`

OpenAgents product surface:

- `/Users/christopherdavid/work/openagents/docs/pylon/2026-06-06-probe-coding-runtime-adapter-contract.md`
- `/Users/christopherdavid/work/openagents/workers/api/src/probe-coding-runtime-contract.ts`
- `/Users/christopherdavid/work/openagents/docs/pylon/2026-06-06-pylon-provider-settlement-bridge.md`
- `/Users/christopherdavid/work/openagents/workers/api/src/pylon-settlement-bridge.ts`
- `/Users/christopherdavid/work/openagents/docs/nexus/2026-06-07-pylon-agent-api-runbook.md`
- `/Users/christopherdavid/work/openagents/workers/api/src/pylon-api.ts`
- `/Users/christopherdavid/work/openagents/workers/api/src/pylon-api-routes.ts`
- `/Users/christopherdavid/work/openagents/docs/pylon/2026-06-06-oa-node-managed-machine-contract.md`
- `/Users/christopherdavid/work/openagents/workers/api/src/oa-node-managed-machine.ts`
- `/Users/christopherdavid/work/openagents/docs/pylon/2026-06-06-oa-workroomd-sidecar-contract.md`
- `/Users/christopherdavid/work/openagents/workers/api/src/oa-workroomd-sidecar-contract.ts`

Historical reference only:

- `/Users/christopherdavid/work/deprecated/openagents.com/routes/api.php`
- `/Users/christopherdavid/work/deprecated/openagents.com/app/Services/ProbeBridgeAdapter.php`
- `/Users/christopherdavid/work/deprecated/openagents.com/docs/pylon-account-linking.md`
