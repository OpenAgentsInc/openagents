---
spec_format_version: "0.1"
title: "Omega Full Auto Host Contract"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-24T08:00:00.000Z"
updated_at: "2026-07-24T08:00:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "docs/omega/"
  - path: "specs/omega/"
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
  openagents_issue: "OpenAgentsInc/omega#19 (OMEGA-FA-00)"
  openagents_freeze: "docs/omega/2026-07-24-full-auto-contract-freeze.md"
  openagents_port_audit: "docs/omega/2026-07-24-full-auto-port-audit.md"
  openagents_desktop_product_spec: "specs/desktop/full-auto.product-spec.md @ spec_revision: 14"
  openagents_desktop_assurance_spec: "specs/desktop/full-auto.assurance-spec.md @ assurance_revision: 6"
  openagents_assurance_companion: "specs/omega/full-auto.assurance-spec.md (proposed; no admission or release authority)"
  openagents_revision_1_note: "Rev 1 admits the Omega host delta for Full Auto: supervised omega-effectd owns durable run mutation; GPUI is launcher and monitor only; Desktop ProductSpec rev 14 remains the lifecycle authority; MemoHarness and initiative stay deferred for OMEGA-FA-01 through OMEGA-FA-07."
---

## Problem

Omega must host Full Auto without inventing a second run lifecycle.
The Desktop ProductSpec already defines the durable run.
The Electron surface cannot move into GPUI.
If Omega copies run authority into GPUI state, Sync projections, or ACP
panels, the eight-run lease model and typed closeout laws break.

## Hypothesis

Omega can implement Full Auto without a second durable authority when these
conditions hold. Desktop Full Auto ProductSpec rev 14 remains the lifecycle
authority. The Effect run engine runs in supervised `omega-effectd`. GPUI
stays a read and control surface only. The freeze keeps the redaction map and
the three non-overridable guardrails. MemoHarness and initiative stay deferred
for the first port.

## Scope

```productspec-scope
in:
  - Omega host binding for the Desktop Full Auto run lifecycle (ten states, legal transitions)
  - active run limit of 8 and one active lease per thread
  - non-overridable guardrails workspace_binding, own_capacity_only, no_rate_limit_reset_triggering
  - first admitted action-lane set with default codex-local then claude-local
  - public-safe receipt schema openagents.desktop.full_auto_run_receipt.v1 redaction map for receipts, notifications, and Sync
  - durable mutation only through full-auto-run-actions (or released successor) inside omega-effectd
  - GPUI launcher and concurrent run monitor as control and projection surfaces only
  - freeze digests recorded in docs/omega/2026-07-24-full-auto-contract-freeze.md
out:
  - Electron IPC channel names or Electron-owned durable stores inside Omega
  - GPUI-owned durable Full Auto run store
  - composer-toggle or ambient chat Full Auto preference
  - Pylon or FleetRun coupling as Full Auto run authority
  - MemoHarness adaptation, experience bank, or optimizer release in the first Omega port
  - HANDS-6 initiative or self-claim autonomy expansion in the first Omega port
  - release or public-claim admission from this host delta alone
cut:
  - rewriting the ten-state lifecycle in Rust
  - a second active-run limit or lease model
  - weakening non-overridable guardrails through config or UI
  - treating provider text as run closeout authority
```

## User Experience

The owner starts Full Auto from a dedicated Omega launcher.
The owner sees concurrent runs in a monitor.
The owner can pause, resume, and stop by `runRef`.
Ordinary chat and ACP panels do not start Full Auto authority.
Mobile may send typed control intents.
Omega Desktop remains the sole local executor.

## Acceptance Criteria

- **OMEGA-FA-AC-01:** Omega Full Auto keeps the Desktop ten-state lifecycle and
  the exact legal transition graph from `full-auto-run-registry.ts` at the
  freeze digests. An illegal transition refuses with a typed error.
- **OMEGA-FA-AC-02:** The active run limit is exactly 8. Each thread holds at
  most one active lease.
- **OMEGA-FA-AC-03:** The non-overridable guardrail set is exactly
  `workspace_binding`, `own_capacity_only`, and
  `no_rate_limit_reset_triggering`. No config or UI control may weaken it.
- **OMEGA-FA-AC-04:** The first admitted action-lane set includes
  `codex-local` and `claude-local`. Default routing order is `codex-local`
  then `claude-local` when Advanced policy is absent.
- **OMEGA-FA-AC-05:** Receipts, notifications, and Sync projections carry only
  public-safe receipt fields for schema
  `openagents.desktop.full_auto_run_receipt.v1`. Raw objective text, workspace
  paths, credentials, and transcript text are forbidden.
- **OMEGA-FA-AC-06:** Durable run mutation occurs only through
  `full-auto-run-actions` (or its released successor) inside supervised
  `omega-effectd`. GPUI, ACP panels, and ordinary chat are not run authority.
- **OMEGA-FA-AC-07:** MemoHarness and initiative remain deferred for
  `OMEGA-FA-01` through `OMEGA-FA-07` unless a later freeze revision admits
  them.
- **OMEGA-FA-AC-08:** Full Auto remains a dedicated run. It is never a
  composer toggle or ambient chat preference.

## Success Metrics

- **OMEGA-FA-SM-01:** Every later Omega Full Auto implementation packet cites
  this ProductSpec revision and the freeze digests, or a superseding freeze
  revision.
- **OMEGA-FA-SM-02:** No landed Omega Full Auto packet introduces a
  GPUI-owned durable run store or a second lifecycle enumeration.

## Owner Gates

- Owner direction 2026-07-24 admits this freeze for Omega issues `#19`
  through `#30`.
- Shared runtime seam `OMEGA-OA-01` and identity/Sync packets remain soft
  prerequisites for FA-01 and later wiring, not for this freeze text.

## Receipts

- Freeze receipt: `docs/omega/2026-07-24-full-auto-contract-freeze.md`
- Omega consumer pointer:
  `OpenAgentsInc/omega` `docs/src/development/omega-full-auto-contract-freeze.md`

## Promise Links

- This host delta alone does not admit a public product-promise flip.
