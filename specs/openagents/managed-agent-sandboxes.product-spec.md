---
spec_format_version: "0.1"
title: "OpenAgents Managed Agent Sandboxes"
artifact_type: "prd"
spec_revision: 2
author: "OpenAgents"
created_at: "2026-07-19T00:00:00Z"
updated_at: "2026-07-19T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "9023"
  openagents_lane: "SBX-00 through SBX-10 (#9024-#9034)"
  openagents_source: "docs/teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md"
  openagents_ide_roadmap: "docs/ide/ROADMAP.md (managed capability dependency of IDE-13 and IDE-17)"
  openagents_assurance_level: "managed execution, tenant isolation, lifecycle, and cross-surface authority"
  openagents_revision_2_note: "Rev 2 records the SBX-08 controller implementation. Mobile and authenticated web decode one shared bounded supervision schema, preserve exact actor attribution, and persist exact generation-fenced command bytes before send. The implementation remains default-off. SBX-09 still owns live GCP acceptance and rollout, and physical-device evidence remains a separate release gate. No acceptance criterion changed."
---

## Problem

OpenAgents already has GCE placement, Agent Computers, Firecracker workrooms,
capability brokers, runtime events, Khala Sync, artifacts, and receipts, but
those pieces do not yet present one coherent managed sandbox that the IDE,
Sarah, mobile, or an external harness can create and supervise. Callers must
understand several internal contracts, lifecycle truth is fragmented across
lease, guest, process, filesystem, and ingress state, and the existing
`OpenAgentsSandboxV1Client` seam is not a production server. This prevents the
IDE from starting durable remote agent work and prevents Sarah from creating
bounded GCP capacity through a narrow receipted tool.

Ascii Box demonstrates a compact external contract for the missing product
shape: one durable machine resource, explicit create/stop/resume lifecycle,
first-class prompt status and cursor events, deterministic file/command/
artifact I/O, and a TypeScript SDK whose base URL is configurable. Adopting
Ascii as the production control plane would discard OpenAgents' stronger GCP,
authority, isolation, capability, event, and receipt substrate. Copying names
without matching lifecycle and isolation semantics would be worse than either
choice.

## Hypothesis

If OpenAgents defines one owner-scoped, generation-fenced `SandboxResource`
over its existing Google Cloud workroom substrate. Exposes its native typed
service to Desktop, Sarah, mobile, and web. And serves an admitted subset of
the Box v1 API as a compatibility projection, then users can start and
supervise long-running agents on OpenAgents-managed capacity with a familiar
SDK while OpenAgents remains authoritative for where work runs, what it can
access, what happened, how much it consumed, and whether cleanup completed.

## Scope

```productspec-scope
in:
  - one canonical owner/tenant/work-unit/sandbox/generation/target/lease identity and durable lifecycle authority
  - explicit provisioning, ready, idle, running, stopping, stopped, resuming, deleting, deleted, failed, and recovery-required outcomes that keep guest, filesystem, ingress, and runtime-turn truth distinct
  - OpenAgents-managed isolated execution on Google Cloud through the existing oa-codex-control, oa-workroomd, Agent Computer, GCE, and Firecracker seams
  - a provider-neutral Effect service graph with Google Cloud layers selected below route and product logic
  - Box v1 Phase 1 compatibility for account/limits, sandbox lifecycle, prompt/status/events/interrupt, bounded files, commands, and artifacts from an OpenAgents-owned base URL
  - the unmodified MIT @asciidev/box-sdk at exact version 0.0.24 as an isolated development-only black-box conformance client with pinned package integrity and OpenAPI digest
  - native lossless OpenAgents events, authority decisions, private evidence, usage, and receipts beside a versioned bounded Box-compatible projection
  - long-running Codex and Claude work whose completion derives from structural runtime settlement, explicit stop, guardrail, or typed failure rather than silence
  - durable lifecycle and turn-order serialization, byte-idempotent retry, conflicting-byte refusal, disconnect recovery, and generation fencing
  - scoped provider, SCM, tool, network, ingress, file, command, and artifact capabilities with quotas, expiry, revocation, and cleanup
  - IDE-13 managed project-capability placement and IDE-17 background-agent execution without a second project, session, or agent graph
  - principal.sarah create, list, inspect, dispatch, interrupt, stop, resume, and delete through one exact owner-scoped capability broker after authority admission
  - bounded mobile and authenticated-web supervision through existing any-host, outbox, attention, portable-session, and IDE-14 projections
  - exact lease, runtime, usage, incremental-cost, artifact, stop, resume, delete, and zero-residue receipts
  - Phase 2 checkpoint, fork, and private desktop or preview ingress only after their exact semantics and security proofs pass
  - Effect Schema as the canonical contract source and Effect services/layers/scopes as the application and control-plane authority; Rust remains inside the existing process-opaque Cloud daemon and containment boundary
out:
  - replacing the OpenAgents Google Cloud substrate or making Ascii a production dependency or authority
  - making Box SDK models canonical inside OpenAgents product or infrastructure code
  - requiring an OCI container when the admitted isolation unit is honestly a GCE VM or Firecracker microVM
  - copying Optibox source, whose inspected snapshot has no declared license
  - raw account-wide secret replication, raw environment secret values, provider auth-home copying, broad service-account credentials, or credentialed repository URLs
  - public or ungated VNC, ambient SSH, generic shell, raw topology, guest IP, host path, or cloud-admin capability in Desktop, mobile, web, Sarah, or compatibility responses
  - treating a bearer token, model output, SDK response, UI silence, job id, or runtime state label as sufficient authority or completion evidence
  - silently substituting local, fake, foreign-user, or weaker-isolation execution when a managed sandbox is requested
cut:
  - payment, wallet, payout, settlement, public labor-market, or resale authority
  - stable release or public parity claims without their existing gates
  - remote Full Auto start or cross-machine FullAutoRun admission by inference; composing Full Auto with managed sandboxes requires an exact later Full Auto ProductSpec and AssuranceSpec revision
  - snapshot, fork, desktop, SSH, repository-discovery, API-key-metadata, or account-secret parity in the first operational milestone
```

## User Experience

In Desktop, the owner chooses `OpenAgents managed` as an explicit placement
for a project or background agent. Before admission the UI shows the effective
image/profile, custody, capabilities, network posture, TTL, budget, and
retained data. The same project and agent refs remain visible while the
sandbox provisions, runs, stops, resumes, or fails. Text, tool, file,
artifact, usage, and lifecycle events stream into the ordinary project and
agent graphs. A quiet process is not shown as idle unless its runtime has
structurally settled.

In the stable owner Sarah thread, the owner can ask Sarah to create a sandbox
for an exact repository/work unit, inspect it, dispatch bounded work, follow
its status, interrupt it, stop or resume it, and delete it. Sarah shows each
tool action and its actual target outcome. She has no generic `gcloud`, shell,
database, topology, or container-admin tool, and never says an action
succeeded before the target receipt exists.

Mobile and authenticated web render the same bounded lifecycle, attention,
changes, artifacts, budget, and cleanup facts and send the same exactly-once
commands. An external TypeScript harness may point the unmodified Box SDK at
the OpenAgents base URL and use only the documented compatibility subset.

## Acceptance Criteria

- **MSB-AC-01:** Every sandbox is bound before mutation to an authenticated
  owner, tenant, program/work unit, sandbox ref, attachment generation,
  target, image/profile, lease, budget, and capability set. A bearer or SDK
  box ID alone grants no access, and cross-owner or stale-generation access
  fails before a runtime effect.
- **MSB-AC-02:** Create, inspect, update, stop, resume, and delete serialize
  through durable lifecycle authority. Exact retries reconcile, conflicting
  request bytes refuse, and crash, disconnect, duplicate, and lost-ACK faults
  never create two work-accepting generations or repeat a settled effect.
- **MSB-AC-03:** A requested live managed target reports ready only from an
  observed healthy admitted GCP provisioner and guest. Fake mode, missing KVM
  or images, unhealthy boot, quota/budget/capacity exhaustion, or unavailable
  capacity returns a typed refusal. No local or fake execution can satisfy
  live acceptance. Region/class/image, concurrency, TTL, least-privilege
  keyless service identities, external-IP posture, and network policy bind
  before provisioning.
- **MSB-AC-04:** The unmodified `@asciidev/box-sdk@0.0.24` can select the
  OpenAgents `basePath` and pass the exact admitted method, status, envelope,
  query, cursor, retry, and error corpus. The SDK and vendor types remain an
  isolated development dependency and cannot become canonical runtime types.
  The receipt binds OpenAPI bytes, npm integrity, package digest, exact
  lockfile, SPDX/license notice, and translator version.
- **MSB-AC-05:** Phase 1 serves only the admitted account/limits, lifecycle,
  prompt/status/events/interrupt, file, command, and artifact operations.
  Every unsupported Box operation returns stable typed `501
capability_not_implemented`. No empty result or fake success implies parity.
- **MSB-AC-06:** A prompt creates one exact runtime turn with effective
  provider, model, harness, work unit, and generation truth. Prompt status is
  first class. Reconnectable ascending cursor pages preserve order. Interrupt
  targets exactly one turn and is idempotent.
- **MSB-AC-07:** A long-running turn has no silence-based completion and no
  arbitrary production wall timeout. Structural runtime completion, explicit
  stop, a declared lease/budget guardrail, or a typed failure settles it. Idle
  stop can arm only after that settlement and cannot race active hidden work.
- **MSB-AC-08:** Provider, SCM, tool, network, ingress, and API capabilities
  are scoped, expiring, revocable broker leases redeemed only inside the exact
  sandbox generation. Raw credentials, auth homes, service-account material,
  private paths, and topology never enter payloads, checkpoints, prompts,
  logs, public events, issues, or receipts.
- **MSB-AC-09:** File, command, process, network, and artifact operations
  enforce root-relative path, symlink, secret, binary, byte, duration, output,
  concurrency, egress, and quota policy below every client adapter. Artifacts
  bind exact content digest, size, source generation, retention, and evidence
  refs.
- **MSB-AC-10:** Native OpenAgents events and receipts remain the lossless
  record. Every Box-compatible event identifies its translator version and
  cursor. Conformance proves projection ordering and omission, but a Box event
  or SDK terminal state can never replace native authority, private evidence,
  usage truth, or cleanup evidence.
- **MSB-AC-11:** Desktop creates and attaches a managed sandbox through the
  IDE-13 project-capability interface, and IDE-17 agents consume the same
  project, file, document, proposal, work-unit, agent, and evidence refs as a
  local placement. Effective target, image/profile, custody, latency,
  generation, lease, budget, and capability state remain visible, with no
  renderer credential, raw root, generic control-plane client, or silent
  managed fallback.
- **MSB-AC-12:** After an exact authority-profile and broker admission,
  `principal.sarah` can create, list, inspect, dispatch into, interrupt, stop,
  resume, and delete only the authenticated owner's sandboxes. Every action
  requires exact program/work-unit/target/profile/TTL/budget/capability refs,
  emits ordered activity and authority plus target receipts, and exposes no
  generic cloud, shell, database, topology, or credential tool.
- **MSB-AC-13:** Mobile and authenticated web decode bounded lifecycle,
  effective runtime, last structural event, attention, file/change/artifact,
  lease/budget, and cleanup projections from shared schemas and send only
  typed commands through the durable outbox. They host no SDK, runtime, GCP
  client, provider credential, raw filesystem, PTY, or generic shell.
- **MSB-AC-14:** Lease expiry, budget or quota exhaustion, revoke, guest crash,
  broker outage, control-plane restart, cursor loss, stop/resume failure, and
  teardown failure remain distinct typed outcomes. Cleanup is complete only
  when receipts prove zero residual compute, firewall/ingress, scratch,
  process, and capability grants. Otherwise the sandbox is recovery-required.
- **MSB-AC-15:** Snapshot and fork remain unavailable until an exact completed
  checkpoint binds source sandbox/generation, image/toolchain, repository
  post-image, content digest, and retention. Fork creates a fresh sandbox and
  fresh capabilities and never clones credentials, memory, processes, sockets,
  ports, network identity, or provider hidden state.
- **MSB-AC-16:** Private desktop or preview ingress remains unavailable until
  its short-lived owner/audience-scoped capability, revocation, redaction,
  audit, and cleanup tests pass. Public or ungated VNC is not admitted.
- **MSB-AC-17:** The deterministic fault and isolation corpus covers
  cross-owner access, concurrent lifecycle calls, stale generations,
  replay/conflict, partial provision, guest crash, event gaps, capability
  revoke, secret markers, quota, cost cap, and partial teardown without false
  readiness, duplicate execution, leaked private material, or residue.
- **MSB-AC-18:** Live acceptance independently proves the pinned SDK against
  staging and owner-gated GCP plus one Desktop and one Sarah create-to-agent-
  turn-to-stop/resume/delete journey. Receipts bind source/deployed revisions,
  image/provisioner/translator identities, measured incremental cost, zero
  residue, rollback, limitations, and owner observation. Fixture or fake proof
  cannot satisfy this criterion.

## Success Metrics

```productspec-success-metrics
- id: managed_sandbox_lifecycle_integrity
  metric: admitted_live_sandbox_lifecycle_operations_with_exact_idempotent_outcome_and_cleanup_receipts
  target: "100%; zero duplicate accepting generations and zero untracked residual resources"
  window: every release candidate and rolling 30-day owner dogfood
  segment: OpenAgents-managed GCP sandboxes
  source: sandbox_lifecycle_authority_and_cleanup_receipts
- id: managed_agent_observability
  metric: long_running_turns_with_reconnectable_structural_progress_and_terminal_truth
  target: ">= 99.9%; zero silence-derived completion"
  window: rolling 30 days after owner-gated rollout
  segment: Codex and Claude managed turns
  source: native_runtime_event_and_turn_receipts
- id: box_sdk_phase1_conformance
  metric: admitted_box_v1_sdk_corpus_passing_without_vendor_runtime_authority
  target: "100% for the pinned Phase 1 corpus; every unsupported method explicitly refused"
  window: every compatibility service candidate
  segment: local fake, staging, and owner-gated live GCP
  source: pinned_sdk_conformance_receipts
- id: sarah_and_ide_same_broker
  metric: desktop_and_sarah_managed_sandbox_journeys_using_the_same_lifecycle_authority_and_target_receipts
  target: "100%; zero client-specific lifecycle or authority models"
  window: every managed-sandbox release candidate
  segment: authenticated owner journeys
  source: authority_target_and_cross_surface_receipt_audit
```

## Risks

- A VM-shaped resource marketed as a container can obscure the actual
  isolation and cost boundary. Product and receipts name the effective GCE VM
  or Firecracker microVM and never claim OCI semantics unless implemented.
- Compatibility pressure can weaken secret, network, desktop, or lifecycle
  policy. Typed incompatibility is preferable to unsafe parity. Account-wide
  secret replication and public VNC remain unsupported.
- Prewarming every message can become a cost leak. Admission decides whether
  to prewarm, and idle stop waits for structural settlement under an exact
  lease and budget.
- A lossy compatibility event plane can be mistaken for evidence. Native
  events, authority decisions, usage, and cleanup receipts remain canonical.
- A convenient Sarah tool can become generic cloud-admin authority. Her broker
  accepts only closed lifecycle and work-unit operations with exact bounded
  inputs and independently enforced target policy.
- Snapshot or resume language can imply process-memory continuity. The product
  states filesystem checkpoint and service restart. Hidden provider state,
  processes, sockets, and memory do not move.

## Related Artifacts

- Source teardown and architecture recommendation:
  `docs/teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md`
- Accepted implementation plan and issue ledger:
  `docs/sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md` and epic
  [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023)
- Canonical IDE sequence and traceability:
  `docs/ide/ROADMAP.md` and `specs/IDE_ROADMAP_CROSSWALK.md`
- Sarah owner-orchestrator intent and authority:
  `specs/openagents/sarah-owner-orchestrator.product-spec.md`, `AUTHORITY.md`,
  and `docs/authority/SARAH_AUTHORITY.md`
- Existing Cloud and portable-session boundaries:
  `docs/cloud/README.md`, `docs/cloud/INVARIANTS.md`, and
  `specs/openagents/portable-coding-sessions.product-spec.md`

## Owner Gates

The current owner direction admits this ProductSpec, epic, roadmap integration,
and bounded implementation program. It does not make the capability live.
Before Sarah or any client may mutate managed sandboxes, SBX-00 must land the
exact authority-profile revisions and broker contract, and SBX-02 must prove
the effective GCP target. Before production rollout, SBX-09 requires
independent assurance, measured spend within the standing cap, rollback, and
owner observation. Any stable release or public compatibility/availability
claim retains its existing release and promise gates.

## Receipts

- `openagents.authority_decision_receipt.v1` for lifecycle and Sarah actions.
- Canonical sandbox lifecycle, generation, lease, capability, runtime-turn,
  event-cursor, usage, cost, artifact, checkpoint, ingress, and cleanup
  receipts designed under SBX-00.
- Pinned SDK/OpenAPI/translator conformance receipts for fake, staging, and
  owner-gated live GCP.
- Desktop, Sarah, mobile/web, fault/isolation, rollback, and zero-residue
  journey receipts at their exact proof rungs.

## Promise Links

None yet. No public Box-compatibility, managed-container, cloud-agent,
long-running-agent, IDE-placement, or Sarah-execution claim may ship until the
promise registry names the exact implemented subset and consumes SBX-09 live
evidence.
