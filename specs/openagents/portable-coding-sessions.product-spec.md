---
spec_format_version: "0.1"
title: "Remote-first Portable Coding Sessions"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-12T00:00:00Z"
updated_at: "2026-07-12T00:00:00Z"
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
  - owner-minted host-independent coding-session and WorkContext identity
  - canonical nested agent graph with independent transcript and activity cursors
  - one generation-fenced attachment covering the root and every descendant
  - content-addressed secret-free checkpoint and exact repository post-image
  - owner-local, owner-managed, OpenAgents-managed, and audited-provider targets
  - target-scoped provider, SCM, tool, and API capability leases
  - typed stop, checkpoint, detach, attach, move, abort, resume, and failback commands
  - Effect Native mobile any-host control and persona-neutral voice modality
  - signed cross-host fault, update, rollback, failback, and reclaim dogfood
out:
  - transparent migration of live process memory, PTYs, sockets, or provider hidden state
  - remote-desktop pixel streaming
  - direct vendor or SSH APIs in renderer and mobile code
  - public pooled access to an owner homelab
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
editable input modality over those actions; text always remains available.

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
  target/capability/TTL-scoped grants; source grants revoke and cleanup is
  receipted.
- Local to managed to owner-managed movement and failback preserve exact refs
  with no duplicate accepted parent or child work.
- Physical iOS, Android emulator, and Desktop expose the same directory,
  graph, typed controls, files/diff/terminal/preview/artifact facts, and durable
  outcomes without host paths, tokens, or vendor APIs.
- Offline, lost ACK, replay/order, restart, update, revocation, expiry,
  migration, rollback, and reclaim faults converge without fork, leak, silent
  substitution, orphan, false authority, or repository data loss.

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
```

## Risks

- A broad "resume" implementation can accidentally become silent rerun or a
  second executor; generation and idempotency checks must precede mutation.
- Provider hidden state is not portable. The product must say checkpoint and
  rehydrate, not imply live process-memory migration.
- Secret-bearing logs or snapshots can invalidate the whole architecture even
  when movement works; broker and scan failures fail closed.
- Target-provider breadth can race ahead of the session fence. PORT-01 and
  PORT-02 remain the critical path before adapters or UI claim portability.

## Owner Gates

- Physical-phone acceptance of PORT-06/PORT-07 and final signed PORT-08 journey.
- Any public claim that portable sessions or a named managed provider have
  shipped; code and fixture proof alone cannot authorize that statement.

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
