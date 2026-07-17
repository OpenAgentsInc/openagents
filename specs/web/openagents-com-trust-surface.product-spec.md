---
spec_format_version: "0.1"
title: "openagents.com: Public Trust Surface and Remote Supervision Client"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-17T22:03:50.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents.com/"
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
  openagents_sibling_specs: "specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/mobile/mobile-any-host-fleet-controller.product-spec.md"
---

## Problem

The web surfaces of every audited competitor are marketing plus cloud-canonical
transcript custody, and their trust failures are public record: Cursor had two
pricing crises from opaque metering and a concealed base-model substitution;
Amp hides model identity behind mode names and holds transcripts cloud-only
with "unlisted" links that are internet-readable; Factory publishes mutually
contradictory data-flow pages; Command Code calls a hosted inference loop
"local" while posting an undisclosed device fingerprint; almost everyone's
release chain is unsigned or checksum-only from the same origin. A developer
or team deciding whether to trust an agent vendor has nowhere to verify
anything: not what model ran, not what a run cost, not where data went, not
whether the binary they installed is what the vendor built. Meanwhile the
same user needs a browser surface that can actually supervise their fleet
when they are on a machine that isn't theirs.

## Hypothesis

If openagents.com becomes the public trust surface — durable addressable
thread objects with owner-controlled receipted visibility, exact per-call
usage and model truth, a dereferenceable public ledger of release manifests
and receipt verification, a published per-work-unit data-flow matrix — and
simultaneously a full remote-supervision client with the same typed command
vocabulary as Desktop and mobile, then trust-sensitive developers and teams
will convert at materially higher rates and existing users will activate
across surfaces, because openagents.com is the only place in the market where
an agent vendor's claims can be checked instead of believed.

## Scope

```productspec-scope
in:
  - Present the thread as a durable, addressable, cross-surface work object: stable IDs and URLs, search across text, file, repository, author, and date, cross-references between threads, and remote control — while local-first custody holds and the web renders synced typed facts, never becoming the canonical transcript authority.
  - Make every visibility transition explicit and receipted: changing a thread from private to shared shows the exact before and after audiences, requires confirmation, and records a receipt; no silent visibility expansion on workspace join, and no ambiguous unlisted state.
  - Ship remote supervision parity: an attention inbox, fleet and agent-graph views, approvals, questions, steer-and-queue controls, and continuation links that hand a session to Desktop or mobile without forking identity, history, or authority.
  - Publish usage and model truth as product: every billable call resolves to provider, model, and cost in a routing receipt; budgets are visible before spend and reconciled against exact usage rows after; no silent model substitution, ever.
  - Publish a dereferenceable public trust ledger: release-set manifests and signing keys, the component compatibility ledger, receipt verification endpoints, and the product-promise registry, so third parties can verify artifacts and claims mechanically.
  - Publish a per-work-unit data-flow matrix stating local reads, uploaded context, provider destinations, storage, visibility, retention, and training as separate facts, kept consistent with observed behavior.
  - Ship an onboarding gradient measured in seconds: a zero-install command front door that stands up a paired supervising session with the pairing token confined to the URL fragment, import lanes that meet users inside their existing tool histories, and UI-first pairing, device-linking, and fleet-account connection flows.
  - Keep honesty conventions product-visible: inert or unsupported configuration is labeled, degraded enforcement renders as degraded, and public counters reconcile to exact receipted rows.
out:
  - The web surface never grants desktop privilege, never holds canonical transcript custody, and never executes agent work in the browser.
  - No growth of legacy pages; the retained public product routes stay minimal, and copy changes remain behind the existing promise-registry copy gates.
  - No third-party analytics, tracking, or SaaS dependencies on the public surface.
  - No unlisted-link visibility state; every visibility state has a named audience.
cut:
  - CUT-WEB-01: Public thread discovery feeds and leaderboards are cut; threads are shared deliberately or not at all.
  - CUT-WEB-02: An in-browser IDE or editor surface is cut; the web workbench is supervision and review, not editing.
  - CUT-WEB-03: A separate marketing microsite stack is cut; the trust ledger and the product are the marketing.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user opens a thread URL they are authorized for, the page renders the typed projection consistent with device truth, including a replay-to-live marker, and renders unreconstructable history as explicit transient-gap markers rather than fabricated continuity.
- id: AC-2
  criterion: When a user changes a thread's visibility, the flow displays the exact before and after audiences, requires explicit confirmation, and records a visibility receipt retrievable from the thread.
- id: AC-3
  criterion: When a user inspects usage, every billable call resolves to its provider, model, and cost, and the public counters reconcile to the exact receipted rows backing them.
- id: AC-4
  criterion: When a third party fetches the trust ledger, release artifacts verify against the signed release-set manifest with the published pinned key, and the receipt verification endpoint returns a mechanical pass or fail for a presented receipt.
- id: AC-5
  criterion: When a new user runs the zero-install front door command, they reach a paired supervising session in one command plus one browser confirmation, and the pairing token never leaves the URL fragment or appears in server logs.
- id: AC-6
  criterion: When a user approves, answers, or steers from the web, the action produces the same typed durable outcome records as the equivalent Desktop or mobile action, and a continuation link opens the same session on another surface without forking identity.
- id: AC-7
  criterion: When a user reads the data-flow matrix for a work-unit type, the stated local reads, uploads, provider destinations, storage, visibility, retention, and training facts match audited behavior for that work-unit type.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: front_door_to_paired_activation_rate
  target: ">= 25% of front-door starts reach a paired supervising session"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of the onboarding gradient shipping
- id: SM-2
  metric: weekly_trust_ledger_verifications
  target: "baseline established, then growing month over month"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days from trust-ledger availability
- id: SM-3
  metric: cross_surface_continuations_per_weekly_active_user
  target: ">= 2 per week"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of supervision parity shipping
- id: SM-4
  metric: usage_reconciliation_disputes
  target: "0 unresolved discrepancies between public counters and exact rows"
  target_status: committed
  window: rolling 30 days, continuously
- id: SM-5
  metric: share_of_new_signups_citing_verifiability
  target: "baseline established via onboarding survey"
  target_status: provisional
  target_owner: "owner"
  window: within 120 days of trust-ledger availability
```

## Solution

The web app is two products on one typed substrate. First, a projection
client: the same generated protocol and command vocabulary as Desktop and
mobile, rendered for the browser, with custody staying local-first and sync
carrying typed facts. Second, a trust ledger: the public, machine-checkable
face of the receipts, manifests, and promises the rest of the system already
produces — release verification, usage truth, data-flow candor. The
onboarding gradient (zero-install command, fragment-token pairing, import
lanes) connects the two: verification first, supervision seconds later.

## Strategic Positioning

Every competitor asks to be believed; none can be checked. Cursor, Amp,
Factory, and Command Code each failed publicly on exactly the dimensions this
surface makes verifiable. A public trust ledger is cheap to render once the
underlying receipts exist and is structurally hard for cloud-custody vendors
to copy, because their business models depend on the opacity it removes.

## Risks

- A trust ledger with gaps reads worse than none; ship each ledger section
  only when its underlying receipts are real and continuously produced.
- Supervision parity on the web must not quietly turn the browser into a
  privilege escalation path; the projection-only boundary needs the same
  IPC-grade discipline as Desktop's renderer.
- Usage-truth commitments (SM-4) create a standing operational obligation to
  reconcile counters; that cost is the product working as intended, but it
  must be staffed.
- Fragment-token onboarding depends on relay and pairing infrastructure from
  the mobile/desktop programs; web sequencing cannot outrun them.

## Open Questions

- Which trust-ledger section ships first: release verification (smallest
  dependency surface) or usage truth (highest user demand)?
- Do team workspaces get visibility-policy templates at launch, or is every
  share an explicit per-thread decision initially?
- How much of the supervision client is served to signed-out users viewing a
  shared thread (read-only projection versus none)?

## Related Artifacts

- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Competitor trust-failure evidence: `docs/teardowns/2026-07-11-cursor-product-teardown.md`,
  `docs/teardowns/2026-07-16-amp-code-teardown.md`,
  `docs/teardowns/2026-07-16-command-code-teardown.md`,
  `docs/teardowns/2026-07-16-factory-desktop-cli-teardown.md`
- Sibling surface specs: `specs/desktop/desktop-trust-complete-workbench.product-spec.md`,
  `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`
- Public-claim authority remains the promise registry
  (`docs/promises/`, `/api/public/product-promises`).

## Owner Gates

- All public copy changes remain behind the promise-registry copy gates with
  owner sign-off.
- Publication of signing keys and the receipt verification endpoint is an
  owner release decision.
- The data-flow matrix wording requires owner review before publication,
  since it is a standing public claim.
- Any team-workspace visibility defaults require owner sign-off.

## Receipts

Planned receipt kinds this surface renders or verifies: visibility-transition
receipts, model/usage routing receipts, release-manifest verification
results, receipt-verification endpoint results, continuation-handoff records.
This section plans kinds; evidence lives in the receipt systems, not in this
spec.

## Promise Links

None yet. Every public claim this surface makes (usage truth, verifiable
releases, data-flow candor) must be registered in the promise registry with
verification gates before it appears in copy; SM-4 is written to be
consistent with the exact-rows law already governing public counters.
