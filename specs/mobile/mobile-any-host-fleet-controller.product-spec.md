---
spec_format_version: "0.1"
title: "OpenAgents Mobile: Any-Host Agent Fleet Controller"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-17T22:03:50.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-mobile/"
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
  openagents_sibling_specs: "specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md"
---

## Problem

An owner running parallel coding agents is away from the desk exactly when
the fleet needs them: an approval blocks a turn, a question stalls a thread,
a finished change needs review and push. Today's mobile options are either
chat companions that cannot control real work, or single-vendor controllers
with structural trust defects: T3 Code's mobile app proves full controller
breadth (multi-environment pairing, diff review, Git controls, native
terminal, offline outbox) but binds threads to single environments, routes
through hosted third-party relay infrastructure, and executes with no
containment or receipts; Cursor's iOS app supervises only Cursor's cloud.
Nobody ships a phone surface that can supervise and steer agents on any of
the owner's hosts — with the phone never executing anything, never holding
raw credentials, and every consequential action leaving a durable receipt.

## Hypothesis

If OpenAgents mobile is a complete controller client of the same typed
engine protocol as Desktop — an any-host environment directory with owned
relay reachability, a durable per-environment offline outbox, an attention
inbox that pins what needs the owner, and full workbench modes (thread,
files, changes, terminal, preview, artifacts) — and adds what no competitor
has (portable session identity across hosts, scoped revocable capability
grants, receipts on every consequential action), then owners will keep
materially more unattended work running because supervision stops being
desk-bound: approvals get answered in minutes from anywhere, and delegating
overnight or away-from-desk work becomes rational.

## Scope

```productspec-scope
in:
  - Operate strictly as a controller: the phone issues the same typed commands as Desktop and web against server-owned sessions, and never executes agent work, holds raw provider credentials, or receives raw filesystem paths.
  - Ship an any-host environment directory as the first product layer: discovery, QR and manual pairing that exchanges a bootstrap credential for a scoped session credential, cached offline environment truth, and reachability presented as classed hints rather than proof.
  - Route remote reachability through owned relay infrastructure that is end-to-end encrypted and grants reachability without authorization; the client still presents its normal credential to the environment.
  - Bind every environment-facing grant to DPoP-style scope-limited, revocable capability tokens stored in the device vault; every consequential remote action records a durable outcome and receipt.
  - Treat the portable session as the stable object: a session moves owner-local to managed cloud and back through quiesce, checkpoint, detach, attach, resume, and failback verbs, with exclusive attachment generations so exactly one host executes, secret-free checkpoints, and source-cleanup receipts.
  - Ship the workbench mode graph: Attention, Recent, Repositories, and Hosts entry points; per-session Thread, Files, Changes, Terminal, Preview, and Artifacts modes; routes and sheets on phone, list-plus-detail-plus-inspector on tablet, from one adaptive app.
  - Provide a durable per-environment offline outbox built on durable admission: commands queued with client-chosen idempotent IDs, admission acknowledged before the UI shows accepted, explicit steer-versus-queue choice surfaced, worker epochs and ordered replay on reconnect.
  - Ship attention as a product: an inbox where actionable items (approvals, questions, blockers) are pinned and never collapse; privacy-generic push payloads that revalidate at open and deep-link to the exact session; lock-screen presence for running work; share targets and quick actions as controller citizens.
  - Render the complete agent-graph projection with the same typed density rules as Desktop: full roster, live child lifecycle, drill-down into child transcripts, explicit gap accounting.
  - Make Changes writeback safe by construction: no force push, exact post-image receipts on every mutation.
  - Add voice as a session-neutral control channel over a sequenced, acknowledged dictation transport, layered after the controller core is complete.
  - Verify against disposable real servers with seeded deterministic state across device geometries, plus fault injection (network loss, token revocation, host restart), keeping fixture, deployed, and physical-device evidence as separate claims.
out:
  - No on-device agent execution or local model serving in this spec.
  - No desktop tokens, raw provider credentials, raw filesystem paths, or hidden danger modes on the phone.
  - No third-party hosted relay, tunnel, identity, or build/update dependencies; infrastructure is owned.
  - No web-wrapper shell and no second UI tree; one typed component contract renders both phone and tablet.
  - Notification state is never completion authority; only durable outcomes and receipts complete an action in the UI.
cut:
  - CUT-MOB-01: Pixel-streaming remote desktop is cut; the phone renders typed projections, not screen mirrors.
  - CUT-MOB-02: General on-phone code editing is cut to bounded review comments and small staged edits; full editing remains a desktop concern.
  - CUT-MOB-03: A separate tablet app is cut; one adaptive application serves both geometries.
  - CUT-MOB-04: Voice-first ambient assistant framing is cut from this revision; voice ships as a control channel for existing sessions.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user pairs a clean install to a new host by QR, the bootstrap credential is exchanged for a scoped session credential, the vault stores only scoped revocable tokens, and no raw long-lived credential ever persists on the device.
- id: AC-2
  criterion: When a user approves a tool call, steers a turn, or pushes a change from the phone, the action records a durable outcome with a receipt, and the UI completes only from that outcome, never from a notification alone.
- id: AC-3
  criterion: When the device is offline, commands queue in a visible per-environment outbox and replay exactly once on reconnect via idempotent IDs, with admitted and pending states rendered distinctly.
- id: AC-4
  criterion: When a session is moved from an owner-local host to a managed host and back, exactly one attachment generation is executing at every moment, the checkpoint contains no secrets, and the source host's cleanup is receipted.
- id: AC-5
  criterion: When actionable items exist (approvals, questions, blockers), the attention inbox pins them uncollapsed, and opening a push notification deep-links to the exact session state revalidated from the server.
- id: AC-6
  criterion: When a user reviews changes on the phone, the writeback path refuses force-push and displays the exact post-image receipt after any accepted mutation.
- id: AC-7
  criterion: When a session has child agents, the phone renders the complete roster with live lifecycle, supports drill-down into any child transcript, and renders missing history as explicit gap nodes.
- id: AC-8
  criterion: When the fault-injection suite severs the network, revokes a token, or restarts a host mid-action, the app shows honest degraded states (transient gap, unreachable, revoked) and never fabricates success.
- id: AC-9
  criterion: When the release verification harness runs, screenshot matrices against disposable real servers pass across phone and tablet geometries, and physical-device evidence is recorded separately from simulator evidence.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: mobile_weekly_active_supervisors_as_share_of_desktop_actives
  target: ">= 50%"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-2
  metric: median_push_to_decision_latency_for_approvals
  target: "<= 2 minutes"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days after push attention ships
- id: SM-3
  metric: share_of_fleet_approvals_handled_on_mobile
  target: ">= 40%"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-4
  metric: outbox_replay_exactly_once_success_rate
  target: ">= 99.9%"
  target_status: committed
  window: rolling 30 days from outbox availability
- id: SM-5
  metric: portable_session_moves_initiated_from_mobile_per_week
  target: "baseline established, then >= 1 per weekly active supervisor"
  target_status: provisional
  target_owner: "owner"
  window: within 120 days of portable movement shipping
```

## Solution

The phone is the fleet's remote control, not its runtime. One Effect Native
application model renders phone and tablet; all state arrives as typed
projections of the same engine protocol Desktop consumes, over Sync and the
owned relay. The environment directory and pairing come first; portable
session movement is the substrate that makes "which host" a detail rather
than an identity; the workbench modes, outbox, and attention inbox make
supervision complete; receipts make it trustworthy. T3 Code's mobile app is
the breadth bar; the trust layer is the difference.

## Strategic Positioning

Competitors prove demand for mobile supervision (T3's full controller,
Cursor's remote control, Amp's mobile thread control) but every one couples
it to a single vendor cloud or an environment-local thread model. Any-host
control plus portable session identity plus receipts is the position no one
else can copy without rebuilding their custody model.

## Risks

- Controller breadth without the desktop-grade engine work lands as a hollow
  shell; sequencing must keep mobile behind the protocol and portability
  substrates it projects.
- Owned relay operations (E2EE courier, device revocation) are new
  infrastructure surface; the reachability-not-authorization boundary must
  hold under audit.
- Push-attention latency targets depend on platform notification behavior
  the app does not control; measure honestly before committing targets.
- Voice adds transcription privacy surface; it stays behind the controller
  core and its own custody review.

## Open Questions

- What is the minimum credential-recovery story when a phone is lost —
  revoke-all from any other authenticated surface, plus what re-pairing
  proof?
- Which two workbench modes ship first after Thread (Changes and Files, or
  Changes and Terminal)?
- Do Live Activities ship at initial GA or after push attention proves
  reliable?

## Related Artifacts

- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Controller-parity evidence: `docs/teardowns/2026-07-17-t3-code-mobile-app-teardown.md`,
  `docs/teardowns/2026-07-17-t3-code-openagents-mobile-controller-gap-analysis.md`
- Sibling surface specs: `specs/desktop/desktop-trust-complete-workbench.product-spec.md`,
  `specs/web/openagents-com-trust-surface.product-spec.md`
- Portable-session intent: `specs/openagents/portable-coding-sessions.product-spec.md`

## Owner Gates

- Approval of the owned relay design and its operational cost before any
  hosted reachability ships.
- App Store distribution decisions (TestFlight cohorts, store listing,
  release cadence) remain owner actions.
- Push-notification entitlement and APNs key management.
- Sign-off on the lost-device revocation flow before scoped tokens ship
  broadly.

## Receipts

Planned receipt kinds this surface renders or triggers: remote-action
outcome receipts, writeback post-image receipts, session-movement and
source-cleanup receipts, grant issuance and revocation receipts, outbox
replay records. This section plans kinds; evidence lives in the receipt
systems, not in this spec.

## Promise Links

None yet. Public claims derived from this spec (any-host control, phone
never executes, exactly-once replay) must land in the promise registry with
verification gates before they appear in copy.
