---
spec_format_version: "0.1"
title: "Remote-first Portable Coding Sessions"
artifact_type: "prd"
spec_revision: 4
author: "OpenAgents"
created_at: "2026-07-12T00:00:00Z"
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
  openagents_epic: "8566"
  openagents_lane: "PORT-00 through PORT-08 (#8745-#8753)"
  openagents_assurance_level: "cross-host-authority"
  openagents_revision_2_note: "Rev 2 binds Cursor-class cloud/background-agent and Remote Control parity while preserving the stronger any-host session identity: managed infrastructure is optional, placement is explicit, and moving a session never creates a cloud-canonical duplicate."
  openagents_revision_3_note: "Rev 3 binds portable sessions to the Zed-quality IDE project/evidence graph. Host-independent project/root/file/document/proposal/evidence refs and view hints can survive movement and drive exact Desktop/mobile/web continuation, while raw paths, PTYs, helper state, and implicit dirty-buffer migration remain excluded. Effect/TypeScript owns checkpoint, attachment, capability, projection, and recovery authority; Rust helpers are disposable host-local primitives."
  openagents_revision_4_note: "Rev 4 binds the portability contract explicitly to IDE-13 and its IDE-14 projection dependency in docs/ide/ROADMAP.md. Every portable project/capability/checkpoint/projection boundary is an identified Effect Schema with derived types and constrained refs; Context.Service/Layer.effect services own generation-fenced lifecycle and scoped helper teardown. Vim modal state and theme code are not portable authority: only bounded view hints and an effective Tokyo Night semantic review projection may cross surfaces."
  openagents_sibling_spec: "specs/openagents/cursor-capability-parity.product-spec.md"
  openagents_ide_architecture: "docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md"
  openagents_ide_roadmap: "docs/ide/ROADMAP.md (portable capability ownership: IDE-13; projection consumer: IDE-14)"
  openagents_ide_spec_crosswalk: "specs/IDE_ROADMAP_CROSSWALK.md"
---

## Problem

Coding sessions are still materially identified with the process and machine
that currently executes them. Cross-device Sync can continue a conversation,
but it does not yet prove that a nested coding-agent graph can quiesce, move to
a different authorized host, redeem new target-scoped capabilities, and resume
under the same durable identity without two executors or leaked secrets.

## Hypothesis

If one owner-minted coding-session identity, canonical agent graph, durable
event log, generation-fenced attachment, content-addressed secret-free
checkpoint, provider-neutral target contract, and capability broker are shared
by Desktop, mobile, Pylon, and Cloud, then owners can move real coding work
between local, owner-managed, and managed hosts while preserving supervision
and exact repository truth without adopting remote-desktop pixels or portable
process memory.

## Scope

```productspec-scope
in:
  - Cursor-class background-agent, cloud-agent, web, mobile Remote Control, CLI resume, and workstation handback outcomes over one portable identity
  - `docs/ide/ROADMAP.md` IDE-13 local/owner-managed/OpenAgents-managed project-capability symmetry as the exact portability boundary, with IDE-14 as a bounded projection consumer rather than a second project or editor authority
  - owner-minted host-independent coding-session and WorkContext identity
  - canonical nested agent graph with independent transcript and activity cursors
  - host-independent `IdeProjectRef`, `ProjectRootRef`, `ProjectFileRef`, `DocumentSnapshotRef`, proposal, diagnostic, test, artifact, and receipt identities sufficient to reopen the same safe code/evidence object after movement without making a path or process identity portable
  - generation-fenced project capability and view-continuation state: selected file/range, proposal/diff, Problems result, evidence item, and editor layout hints may move as non-authoritative refs; every destination reauthorizes and resolves them against its current attachment and document generations
  - one generation-fenced attachment covering the root and every descendant
  - content-addressed secret-free checkpoint and exact repository post-image
  - owner-local, owner-managed, OpenAgents-managed, and audited-provider targets
  - per-session placement choice and disclosure without a mandatory OpenAgents-managed server when the selected target and reachability mode can operate owner-local or owner-managed
  - target-scoped provider, SCM, tool, and API capability leases
  - typed stop, checkpoint, detach, attach, move, abort, resume, and failback commands
  - Effect Native mobile any-host control and persona-neutral voice modality
  - signed cross-host fault, update, rollback, failback, and reclaim dogfood
  - one Effect/TypeScript portability plane for attachments, checkpoints, capability negotiation, dirty-document policy, persistence, projections, and receipts; Rust PTY/containment helpers, any separately benchmark-admitted authority-free native kernel, and external LSP/harness processes remain disposable host-local state and are recreated only through destination admission
  - one identified Effect Schema source for every attachment, checkpoint, capability, move command/outcome, safe project ref, projection, and helper contract, with derived TypeScript types, constrained branded refs, stable schema identifiers where code generation applies, and decode-before-use at every host/surface boundary
  - Context.Service capabilities composed with Layer.effect, named Effect.fn operations, Schema.TaggedErrorClass failures, and scoped interruption of transports, streams, watchers, language/debug processes, terminals, and helper children when an attachment generation quiesces, moves, revokes, or dies
out:
  - transparent migration of live process memory, PTYs, sockets, or provider hidden state
  - remote-desktop pixel streaming
  - direct vendor or SSH APIs in renderer and mobile code
  - public pooled access to an owner homelab
  - implicit migration of unsaved buffers, undo stacks, editor models, terminal screen/process state, language-server caches, debug processes, or native-helper memory; a dirty document moves only through a separately admitted encrypted content-addressed document checkpoint with explicit conflict and disclosure policy
  - migration or remote authority for Desktop Vim mappings/modal state/key handlers or executable theme contributions; editor settings remain destination-owned, while a bounded non-authoritative view hint or the safe Tokyo Night semantic review tokens may be projected without changing destination policy
cut:
  - copying provider auth homes, environment files, or credential caches between targets
  - silently lowering isolation, custody, provider, account, region, or data posture
  - Sarah, avatar, video, persona, or ambient microphone behavior
```

## User Experience

An authorized owner can open the same session and nested agent graph on mobile
or Desktop, see shell and recent top-level metadata before detail hydration,
inspect one child transcript, request a move, and follow the same refs while
the source quiesces and the destination rehydrates. Tap, click, menu, palette,
and supported key paths invoke the same typed action and outcome. Voice is an
editable input modality over those actions. Text always remains available.

## Acceptance Criteria

- Session identity is independent of host, path, process, provider-native
  session, Pylon home, adapter, and current placement.
- At most one attachment generation accepts work, and its fence covers every
  active descendant.
- The canonical parent edges, agent refs, transcript refs, and per-thread
  cursors survive restart, stream gaps, movement, failback, and projection
  repair without child leakage or flattening.
- Every checkpoint verifies its content digest and exact repository post-image
  and contains no credentials, host handles, live process state, sockets,
  memory, PTYs, raw provider history, or unbounded private content.
- Every destination reauthorizes and redeems new owner/session/attachment/
  target/capability/TTL-scoped grants. Source grants revoke and cleanup is
  receipted.
- Local to managed to owner-managed movement and failback preserve exact refs
  with no duplicate accepted parent or child work.
- Physical iOS, Android emulator, and Desktop expose the same directory,
  graph, typed controls, files/diff/terminal/preview/artifact facts, and durable
  outcomes without host paths, tokens, or vendor APIs.
- Offline, lost ACK, replay/order, restart, update, revocation, expiry,
  migration, rollback, and reclaim faults converge without fork, leak, silent
  substitution, orphan, false authority, or repository data loss.
- Cursor-class background and remote-control journeys can start owner-local,
  owner-managed, or OpenAgents-managed, survive client closure, accept web or
  mobile intervention, and return to Desktop under the same session refs. No
  journey creates a second cloud-canonical transcript or silently changes
  placement, custody, model, harness, or authority.
- Project/file/document/proposal/evidence deep links survive a host move as
  opaque safe refs, but the destination resolves them only after attachment,
  project, document, and audience checks. Stale or unavailable generations
  open an explicit snapshot/diff/unavailable state rather than a current line.
- PTYs, terminal screen/process state, LSP/tsserver/DAP processes and caches,
  and Rust helper state never enter a portable checkpoint. A destination
  negotiates compatible capabilities and starts fresh helpers under Effect
  admission. Cached evidence is labeled until new live generations arrive.
- Every portable and cross-surface boundary value is decoded from one
  identified Effect Schema and its TypeScript type is derived from that source.
  raw per-client interfaces or handwritten unions cannot become a second
  checkpoint, transport, helper, mobile, web, or public-share contract.
- Quiesce, detach, attach, move, failback, revocation, and restart close the
  owning scoped Effect layer and interrupt all generation-bound transports,
  streams, watchers, language/debug processes, terminal children, and helper
  children before a later generation can accept work. Late output is rejected.
- Portable continuation carries no authoritative Vim or theme state. Desktop
  resolves its current built-in Vim setting and Tokyo Night projection after
  reauthorization. Mobile/web may render only the allowlisted semantic review
  tokens and cannot change editor policy.

## Success Metrics

```productspec-success-metrics
- id: portable_move_integrity
  metric: accepted_cross_host_moves_with_identical_session_graph_and_repository_post_image
  target: "100% in the signed R7 acceptance corpus"
  window: PORT-08 owner dogfood window
  segment: owner-local, OpenAgents-managed, and owner-managed target journeys
  source: portable_session_move_and_reclaim_receipts
- id: exclusive_attachment_safety
  metric: observed_states_with_more_than_one_generation_accepting_work
  target: "0"
  window: all deterministic fault suites and PORT-08 dogfood
  segment: every adopted portable coding session
  source: attachment_authority_event_log_and_invariant_audit
- id: portable_secret_exposure
  metric: raw_secret_or_credential_occurrences_in_clients_sync_checkpoints_logs_artifacts_or_receipts
  target: "0"
  window: every move and teardown scan
  segment: provider, SCM, tool, and API capability paths
  source: broker_receipts_and_forbidden_material_scans
- id: portable_schema_and_scope_integrity
  metric: portable_boundary_contracts_with_one_identified_effect_schema_derived_types_and_generation_scoped_resource_teardown
  target: "100%; zero parallel client contracts and zero accepted late-generation events"
  window: every deterministic movement/fault suite and release candidate
  segment: attachment, checkpoint, capability, helper, mobile, web, and public-share boundaries
  source: schema_inventory_scope_teardown_and_late_event_receipts
```

## Risks

- A broad "resume" implementation can accidentally become silent rerun or a
  second executor. Generation and idempotency checks must precede mutation.
- Provider hidden state is not portable. The product must say checkpoint and
  rehydrate, not imply live process-memory migration.
- Secret-bearing logs or snapshots can invalidate the whole architecture even
  when movement works. Broker and scan failures fail closed.
- Target-provider breadth can race ahead of the session fence. PORT-01 and
  PORT-02 remain the critical path before adapters or UI claim portability.
- A superficial Cursor Remote Control clone can preserve vendor-cloud lock-in.
  Placement disclosure and the local/owner-managed path are acceptance
  requirements, not later enterprise options.
- Treating editor models, unsaved buffers, terminal screens, LSP caches, or
  Rust-helper memory as portable state would silently turn checkpointing into
  process migration and leak host-private data. Only explicitly admitted,
  content-addressed project/document/evidence records cross hosts.

## Solution

The portable layer extends the canonical Effect project graph rather than
serializing an IDE process. Effect Schema owns host-independent project/file/
document/proposal/evidence refs, attachment generations, capability
negotiation, checkpoints, destination admission, recovery, and safe mobile/web
projections. A move transfers exact admitted content and causal identities,
then the destination recreates its Monaco models, language/Git/task services,
external harnesses, and any process-opaque Rust PTY/containment helpers under
fresh local policy. UI selection and layout are hints. Authority, dirty-buffer
content, and native process state are never inferred from them.

The portability service graph uses identified `Schema.Struct`,
`Schema.TaggedStruct`, and `Schema.TaggedUnion` contracts with derived types and
constrained refs. `Context.Service` capabilities compose through `Layer.effect`.
named `Effect.fn` operations expose `Schema.TaggedErrorClass` failures. The
attachment scope owns transports, streams, watchers, external processes, and
helper interruption. Destination-owned editor configuration is resolved only
after reauthorization: Vim state is never migrated as authority, and Tokyo
Night reaches lower-trust surfaces only as an allowlisted semantic evidence
projection.

## Related Artifacts

- Canonical IDE roadmap and IDE-13/IDE-14 boundary: `docs/ide/ROADMAP.md`
- Roadmap-to-ProductSpec/AssuranceSpec traceability:
  `specs/IDE_ROADMAP_CROSSWALK.md`
- Zed-quality IDE and Effect/Rust architecture:
  `docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md`
- Cursor parity contract:
  `specs/openagents/cursor-capability-parity.product-spec.md`
- Desktop, mobile, and web owning surface contracts:
  `specs/desktop/desktop-trust-complete-workbench.product-spec.md`,
  `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`, and
  `specs/web/openagents-com-trust-surface.product-spec.md`

## Owner Gates

- Physical-phone acceptance of PORT-06/PORT-07 and final signed PORT-08 journey.
- Any public claim that portable sessions or a named managed provider have
  shipped. Code and fixture proof alone cannot authorize that statement.

## Receipts

- `openagents_apps.portable_session_contract_freeze.v1` for PORT-00.
- Schema/model/fault receipts for PORT-01 and broker security receipts for
  PORT-02.
- Exact local↔managed and owner-managed move/failback/reclaim receipts.
- Physical mobile any-host/voice receipts and the signed PORT-08 acceptance
  ledger with a zero-exception invariant summary.

## Promise Links

- The pending remote-first behavior contracts in
  `packages/behavior-contracts/src/openagents-apps.ts` remain non-green until
  their numbered PORT leaves produce the named runtime and owner receipts.
