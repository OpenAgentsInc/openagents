---
spec_format_version: "0.1"
title: "OpenAgents Desktop: Trust-Complete Coding Workbench"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-17T22:03:50.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-desktop/"
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
  openagents_source_synthesis: "docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md"
  openagents_admission_status: "authored from the full teardown-catalog synthesis; surface-vision PRD pending owner admission and MASTER_ROADMAP reconciliation; MASTER_ROADMAP retains sequencing authority"
  openagents_sibling_specs: "specs/mobile/mobile-any-host-fleet-controller.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md"
---

## Problem

Developers who supervise coding agents now get supervision breadth from every
competitor — parallel worktree agents, diff review, terminals, remote control
— but no product gives them trust. The full teardown catalog (ChatGPT/Codex
desktop, Claude desktop, Claude Code, Cursor, Factory, Amp, Grok Build,
OpenCode, T3 Code, OpenChamber, Command Code) shows the same holes everywhere:
permission dialogs presented as containment while sandboxes are off by
default; "completed" claimed without delivery states; no record of what
authority a run was granted versus what enforcement actually ran; unsigned or
fail-open release chains; agent topology that is fully retained on disk yet
almost invisible in the UI. A developer who wants to hand real repositories to
parallel unattended agents has no surface where deep supervision and
verifiable trust coexist. That developer either accepts a default-open
competitor or does not delegate the work at all.

## Hypothesis

If OpenAgents Desktop combines the market-converged supervision shape — one
typed engine seam, a workbench that deepens beyond chat, worktree-parallel
agents, a fully rendered live agent tree — with a legible trust layer that no
competitor ships (named execution profiles compiled to OS enforcement,
per-run authority manifests paired with execution receipts, a delivery
lifecycle distinct from completion, a signed end-to-end release chain), then
developers will (a) choose it over broader-but-untrusted competitors and
(b) delegate materially more unattended work through it, because the surface
makes delegation inspectable instead of faith-based.

## Scope

```productspec-scope
in:
  - Consume one generated, versioned engine protocol with the hierarchy Thread -> Turn -> Item, extended with Work Unit (task identity plus delivery lifecycle), Authority Manifest, Execution Receipt, and Delivery Receipt; the renderer holds projections only and never conversation, tool, Git, or PTY authority.
  - Record every input durably before scheduling (client-chosen idempotent IDs, causal parent, typed delivery intent), and expose steer, queue, and interrupt as three explicit verbs with typed composer admission states (idle, active-steerable, active-non-steerable, interrupting, repairing, queued, offline, blocked); queue never silently becomes steer.
  - Provide three read surfaces per session as contract: bounded current projections, a durable replayable per-aggregate log with a replay-to-live marker, and a volatile live stream documented as lossy; reconnect is repair with honest transient-gap markers, never fabricated completion.
  - Ship a hardened stock Electron shell: locked fuses, sandboxed renderers, schema-decoded and sender-validated IPC on every channel, partitioned sessions for artifact, preview, terminal, and browser surfaces, deny-by-default permission and navigation handlers, and a locally versioned renderer (never a live site).
  - Ship a right-panel workbench surface manager hosting review-summary, diff, files, file, terminal, plan, and preview tabs, each a projection over engine state with no renderer authority.
  - Virtualize the transcript with a turn navigator and a three-level message hierarchy (prose primary, compact work rows, exact evidence on disclosure); checked-in performance baselines gate merges on transcript surfaces.
  - Render the complete agent tree: a canonical persisted agent graph with live per-child lifecycle, causal inline child cards at the exact spawn point in the parent timeline, one-gesture navigation into each child's full independent transcript, and explicit orphan or gap nodes where history is incomplete; the same typed projection renders at every surface density with no capability tier.
  - Provide file checkpoints independent of Git with conflict-aware, three-mode (conversation, code, both), staged two-phase rewind that discloses reversible versus irreversible effects before commit.
  - Treat worktrees as durable engine resources with outcome-sensitive lifecycle (auto-remove unchanged, retain changed, refuse dirty or unpushed) and cleanup receipts; render delivery states (changes_produced, reviewed, committed, pushed, merged, accepted) distinct from turn completion.
  - Compile named execution profiles (projection-only, workspace-bounded, networked-build, isolated-guest, owner-local danger mode, managed cloud) to OS enforcement; owner-local danger mode is explicit and visually persistent, never a default; unavailable containment fails closed.
  - Provide a hermetic execution profile that excludes every ambient input (hooks, plugins, memory, learned preferences, workspace instructions) unless explicitly admitted, and emits a complete admitted-input manifest.
  - Drive palette, keyboard, menus, slash commands, and model-proposed actions from one central command registry of stable command IDs with typed schemas, capability requirements, approval flags, idempotency, and redaction class.
  - Ship best-of-N and plan-first execution as typed fan-out with per-child receipts and an explicit comparison record, and review fan-out compiled into assurance manifests whose acceptance decision sits outside the reviewed party.
  - Provide a terminal renderer family (full-screen, native-scrollback, headless) over one typed transcript projection, gated by emulator-backed PTY test matrices and frame-time baselines.
  - Package for the full six-target matrix (macOS, Windows, Linux, x64 and arm64) under a signed release-set manifest, a component compatibility ledger across shell, engine, renderer, and extensions, retained-slot rollback, no downgrade flags, and coordinated drain-before-update of live engine work.
out:
  - The desktop app does not load a remotely deployed website as its renderer, and web deployment authority never becomes desktop code authority.
  - No browser-runtime fork or proprietary Chromium; capability ships through narrow typed native modules on stock Electron.
  - No ambient screen-recording or inferred-memory capability in this revision; any future ambient memory is a separate spec bound to private-by-construction custody.
  - No computer-use or OS-automation capability in this revision.
  - No additional provider integrations before one provider is complete under the eleven-predicate closure bar (known, decoded, owned, retained, projected, presented, authorized, recovered, fast, receipted, shipped).
  - No third-party plugin execution inside the trusted engine or shell process; extensions run under declared isolation profiles with signatures and receipts or they do not run.
cut:
  - CUT-DSK-01: Light theme and theme switching are cut; the single dark Khala theme remains the only visual identity.
  - CUT-DSK-02: Simultaneous token-by-token mirroring of every child agent is cut; the roster shows live typed lifecycle and any child transcript opens on demand.
  - CUT-DSK-03: Editor-first IDE ambitions are cut; the workbench deepens supervision and review, not general text editing.
  - CUT-DSK-04: A public plugin marketplace is cut from this revision; signed ingestion of existing open formats (MCP, MCPB, skills) with provenance comes first.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user sends a follow-up while a turn is running, the composer requires an explicit queue-or-steer choice, and the transcript later shows the input's admission, promotion, execution, and terminal states as distinct facts.
- id: AC-2
  criterion: When a user opens any run's detail view, they see both the authority manifest (what policy admitted) and the execution receipt (what containment actually enforced); requested and effective enforcement are never merged into one indicator.
- id: AC-3
  criterion: When a run is started under a profile whose OS enforcement cannot be represented on the current platform, the run refuses to start with a typed reason instead of degrading silently.
- id: AC-4
  criterion: When a session spawns child agents, the agent tree shows every retained child with live lifecycle state, an in-flight spawn is visible before it resolves, any child's full transcript opens within two interactions, and unlinked history renders as an explicit gap node.
- id: AC-5
  criterion: When a user rewinds to a prior turn, the app stages the restore, discloses reversible versus irreversible effects and externally modified files, and only applies on explicit commit, emitting a rewind receipt.
- id: AC-6
  criterion: When agent work in a worktree finishes, unchanged worktrees are auto-removed, changed worktrees are retained, dirty or unpushed worktrees are refused for cleanup, and each outcome emits a cleanup receipt; the Work Unit's delivery state is visible and distinct from turn completion.
- id: AC-7
  criterion: When the packaged app is inspected by release tests, Electron fuse oracles pass (RunAsNode, NODE_OPTIONS, and inspect disabled; ASAR integrity on), and every IPC channel rejects messages that fail schema decoding or sender validation.
- id: AC-8
  criterion: When a transcript holds ten thousand or more items, scrolling and turn navigation stay within the checked-in p95 frame-time baselines, and those baselines gate release.
- id: AC-9
  criterion: When an update is offered, the client verifies the signed release-set manifest against the pinned key before install, refuses version downgrades, retains a rollback slot proven by test, and drains live engine work before relaunch.
- id: AC-10
  criterion: When a run executes under the hermetic profile, the emitted manifest lists every admitted input source, and no ambient instruction, hook, plugin, or memory outside that manifest influenced the run.
- id: AC-11
  criterion: When the same command ID is invoked from the palette, a keyboard shortcut, a menu, and a model-proposed action, all four paths produce identical typed outcome records.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: unattended_turn_share
  target: ">= 40% of desktop turns run in background or Full Auto modes without manual per-turn approval"
  target_status: provisional
  target_owner: "owner"
  window: within 60 days of general availability
- id: SM-2
  metric: median_concurrent_sessions_per_active_user
  target: ">= 3"
  target_status: provisional
  target_owner: "owner"
  window: within 60 days of general availability
- id: SM-3
  metric: receipt_or_manifest_inspections_per_weekly_active_user
  target: ">= 5 per week"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-4
  metric: forced_restart_session_recovery_rate
  target: ">= 99.5% of sessions resume intact with honest gap accounting"
  target_status: committed
  window: rolling 30 days from first packaged release
- id: SM-5
  metric: verified_auto_update_success_rate
  target: ">= 99% of update attempts verify, apply, and relaunch without user intervention"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days per release channel
```

## Solution

One long-lived local engine supervisor owns every conversation; Desktop is
its first and deepest client. The engine seam is a generated Effect Schema
protocol; all clients — Desktop renderer, terminal, mobile, web — consume the
same projections. Authority is compiled, not narrated: profiles become OS
enforcement, and every run leaves the manifest/receipt pair. Recovery is
local-first: append-before-side-effect logs, checkpoints, staged rewind,
conservative worktree retention. Distribution is a signed transaction: a
release-set manifest covering every target and component, verified fail-closed
on the client. The synthesis essay in `docs/teardowns/` (2026-07-17
full-catalog synthesis) records the per-competitor evidence for each element.

## Strategic Positioning

Supervision breadth (parallel agents, worktrees, workbench panels, remote
control) is converging table stakes across Cursor, T3 Code, Factory, Amp, and
OpenCode. The unclaimed lane, named in every teardown, is the trust half:
explicit authority, effective-containment truth, countersigned receipts,
portable sessions, signed provenance, honest usage/model/data truth.
Desktop's differentiation is that lane, rendered as product.

## Risks

- Trust surfaces could read as friction if receipts and manifests are not
  quiet by default; the design must keep them one gesture away, not in the
  path of every action.
- Fail-closed containment on platforms with weak sandbox primitives may block
  workflows competitors allow; the owner-local danger mode must remain an
  explicit, visible escape hatch without becoming a default.
- The six-target matrix and owned-runner release chain are operationally
  expensive; sequencing must not let breadth starve depth.
- Rendering the full agent tree live at fleet scale has real performance
  risk; the perf-baseline gates exist to keep it honest.

## Open Questions

- Which two or three receipt kinds are surfaced most prominently in the
  default UI before users opt into the rest?
- Does the hermetic profile ship as a developer feature first or as the
  substrate for reproducible support bundles?
- What is the minimum viable component compatibility ledger UI — a page, a
  dialog, or a receipt feed?

## Related Artifacts

- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Full teardown catalog and evidence conventions: `docs/teardowns/README.md`
- Sibling surface specs: `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`,
  `specs/web/openagents-com-trust-surface.product-spec.md`
- Desktop implementation-state authorities remain
  `apps/openagents-desktop/GUARANTEES.md` and `docs/sol/MASTER_ROADMAP.md`.

## Owner Gates

- Sign-off on the named execution-profile set and the visual treatment of
  owner-local danger mode.
- Platform signing identities and the owned six-target runner fleet for the
  release matrix.
- Release channel policy (stable/RC identity split, rollback windows).
- Any future ambient-memory capability requires a separate owner-admitted
  spec.

## Receipts

Planned receipt kinds this surface emits or renders: execution receipts,
authority manifests, delivery receipts, worktree cleanup receipts, rewind
receipts, update/rollback receipts, hermetic admitted-input manifests,
fan-out comparison records. This section plans kinds; the evidence ledger
lives in the assurance and receipt systems, not in this spec.

## Promise Links

None yet. Public claims derived from this spec (trust layer, signed updates,
recovery guarantees) must land in the promise registry with verification
gates before they appear in copy.
