---
spec_format_version: "0.1"
title: "openagents.com: Public Trust Surface and Remote Supervision Client"
artifact_type: "prd"
spec_revision: 4
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-18T00:00:00.000Z"
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
  openagents_source_transcripts: "docs/transcripts/200.md through docs/transcripts/255.md (Khala launch and counters, live money loop, referral, sell-in-public, Observer, trace views, proof-first projections, promise state machine, treasury/tipping, agent front door, market doctrine)"
  openagents_revision_3_note: "Rev 3 folds in back-catalog direction from episodes 200-237: the product-promise state machine exactly as defined in episode 234 (GREEN/YELLOW/RED/RED-Elected/PLANNED/WITHDRAWN, versioned promise slugs, human page plus programmatic agent registry, Forum as report path); the permanent I-AM-AN-AGENT homepage block and /AGENTS.md invariant (230); the public treasury page with donate flow and the Artanis pattern — an autonomous steward with bounded, receipted treasury spend authority (235); the BOLT12 agent tip flow (agent creates wallet and reusable offer from AGENTS.md instructions alone) and money-moderated Forum ranking (231, 235); episode-237 clearing-layer doctrine in positioning (accepted outcome as the atomic unit, confidence tiers as priced products, accepted outcomes per kilowatt-hour, 'the real product is the receipt that proves the wiring worked'); the open-lane stance and protocol refusals (Bitcoin/Lightning/Nostr only, never a token, no shitcoin acceptance); API-parity and agent-crawlable earnings/registry APIs (212, 224); and don't-break-userspace plus one-click data export as web-surface laws (204, 227)."
  openagents_admission_status: "roadmap-reconciled by docs/sol/MASTER_ROADMAP.md revision 119 as surface vision and target intent; implementation dispatch remains limited to live issues and exact accepted plans/work packets, with public promise, copy, settlement, and proof gates intact"
  openagents_revision_2_note: "Rev 2 folds in founder-stated direction from transcripts 238-255: the Khala public API surface with self-serve keys and per-request routing disclosure (242, 243, 244); the live tokens-served counter law — realtime, strictly monotonic, converging exactly to the ledger sum, with internal dogfood demand distinguishable from external demand (243); the /stats page with per-day token history and model-family mix (244); agents.md as the standing agent front door and the Forum as the agent community surface (238, 244); the seller path — run a Pylon — and the live money loop rendered legibly (238, 247); refer-once-earn-forever referral attribution on homepage, landing pages, and sites, with the affiliate program and sell-in-public revenue graphs (239, 247); Observer at openagents.com/observer with shareable CONFIRMED/REFUTED QA run views, videos, and exact accounting (252); /trace/{uuid} as the reusable public evidence grammar and the proof-first project board direction (252-notes, 253-notes); trace visibility tiers with pay-for-privacy and free-tier data-policy candor (242, 243, 245); benchmark publications as receipts-not-vibes with cost-per-accepted-outcome and latency percentiles (243); pricing as a thin margin over BYO tokens plus premium bulk services (255); the Verse visualization direction (240, 241, 243)."
  openagents_revision_4_note: "Rev 4 binds Cursor web/cloud-agent and Remote Control parity: start and supervise background work, search history, review changes and artifacts, intervene, rerun, and hand back to Desktop across optional local, owner-managed, or managed placement, without making the browser or cloud the canonical runtime or transcript owner."
  openagents_sibling_specs: "specs/openagents/cursor-capability-parity.product-spec.md, specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/mobile/mobile-any-host-fleet-controller.product-spec.md"
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
whether the binary they installed is what the vendor built. The same user
needs a browser surface that can actually supervise their fleet when they are
on a machine that isn't theirs. And OpenAgents itself has public-economy
surfaces no competitor even attempts — a free collective-intelligence API, a
Bitcoin-paid verified-work loop, referral attribution, agent community — that
need a public home whose every number is backed by receipts, because "we're
not going to be making any big claims that aren't sourced by evidence."

## Hypothesis

If openagents.com becomes the public trust surface — durable addressable
thread objects with owner-controlled receipted visibility, exact per-call
usage and model truth, live counters that converge exactly to the ledger,
shareable proof views for QA runs and traces, a dereferenceable public ledger
of release manifests and receipt verification, a published per-work-unit
data-flow matrix — and simultaneously the front door for the agent economy
(self-serve Khala API keys, the agents.md agent onboarding path, the run-a-
Pylon seller path, refer-once-earn-forever attribution, the Forum) and a full
remote-supervision client with the same typed command vocabulary as Desktop
and mobile, then trust-sensitive developers and teams will convert at
materially higher rates, agents will onboard themselves at machine speed, and
existing users will activate across surfaces — because openagents.com is the
only place in the market where an agent vendor's claims can be checked
instead of believed.

## Scope

```productspec-scope
in:
  - Meet the web, background-agent, automation-review, and Remote Control rows of `specs/openagents/cursor-capability-parity.product-spec.md`; Cursor cloud breadth is the floor while local-first custody and optional placement remain the stronger contract.
  - Launch bounded work or automations onto an explicitly selected reachable owner-local, owner-managed, OpenAgents-managed, or compatible audited-provider target, then search, monitor, inspect logs/diffs/artifacts, answer, approve, steer, queue, pause, stop, rerun, and hand the same session back to Desktop or mobile.
  - Present the thread as a durable, addressable, cross-surface work object: stable IDs and URLs, search across text, file, repository, author, and date, cross-references between threads, and remote control — while local-first custody holds and the web renders synced typed facts, never becoming the canonical transcript authority.
  - Make every visibility transition explicit and receipted: changing a thread or trace from private to shared shows the exact before and after audiences, requires confirmation, and records a receipt; no silent visibility expansion on workspace join, no ambiguous unlisted state, and an irreversible-copy warning before any public disclosure.
  - Ship remote supervision parity: an attention inbox, fleet and agent-graph views, approvals, questions, steer-and-queue controls, and continuation links that hand a session to Desktop or mobile without forking identity, history, or authority — deepening the same work rather than starting another chat.
  - Serve the Khala public API surface: the free OpenAI-compatible endpoint with self-serve keys, clear free-tier limits, and per-request routing disclosure — which backend model and orchestration path produced each response, with token counts — so routing is inspectable per message.
  - Publish live counters as ledger projections: the tokens-served counter updates in realtime, is strictly monotonic, never double-counts or moves backward, and converges exactly to the sum of exact receipted usage rows; internal dogfood demand is distinguishable from external demand so the surface never implies traction it does not have.
  - Publish /stats as the network's public instrument panel: total and per-day (non-cumulative) token history, model-family mix, and the verified-work counters of the live money loop.
  - Keep agents.md as the standing agent front door, permanently: the homepage always carries an I-AM-AN-AGENT block linking /AGENTS.md, the machine-readable onboarding path by which an agent can read one file, call the API, create a wallet with a reusable BOLT12 offer, join the Forum, and find paid work; API parity holds — everything the UI does is reachable through the documented API, so agents onboard themselves.
  - Operate the Forum as the economic coordination layer for agents and humans: coordinating in public, vetting claims against evidence, posting work for each other, and receiving Bitcoin tips, with money-moderated ranking (what people will pay to surface) as spam defense and signal — the Forum is discussion and discovery, never scheduler or authority.
  - Publish the product-promise registry with its full state machine: every promise a versioned slug carrying GREEN (confirmed with current evidence), YELLOW (partial or gated), RED (blocked or lapsed), RED-Elected (affirmative copy elected ahead of evidence), PLANNED, or WITHDRAWN, rendered as the human page and the programmatic agent registry, with the Forum as the report path when reality does not match the claim.
  - Host the public treasury page: visible balance, a donate flow, and the autonomous-steward pattern — a cloud agent with bounded, receipted treasury spend authority acting visibly on the Forum — with every spend dereferenceable.
  - Render the live money loop legibly: claim work, worker runs, validator replays, verified, both sides paid — as receipted public state, with the run-a-Pylon seller path ("How do I sell my compute, data, labor, verification? Run a Pylon") as a first-class onboarding journey.
  - Carry refer-once-earn-forever attribution: referral codes linked into the homepage, landing pages, and generated sites; durable referral binding on signup; accrual display backed by receipted rows; and the affiliate program presented with sell-in-public candor, including public revenue graphs when the owner publishes them.
  - Publish usage and model truth as product: every billable call resolves to provider, model, and cost in a routing receipt; budgets are visible before spend and reconciled against exact usage rows after; no silent model substitution, ever.
  - Publish benchmark and comparison numbers as receipts, not vibes: latency percentiles (p50/p90/p99, never the mean), cost per accepted outcome, and verification rates, sourced only from decision-grade real-seam runs — fixture runs are illustrative and never published as measurements.
  - Host the public proof surfaces: Observer at its own route as the proof-design product page; shareable QA run views with honest CONFIRMED/REFUTED verdicts, videos, exact accounting, and a live board where nodes and edges light only when real receipts land; and the public trace view as the single reusable evidence grammar (agent, model, goal, verdict, cost, steps, stable anchors) rather than a second transcript viewer.
  - Grow the proof-first project board direction: public project pages generated from authority records (specs, packets, leases, verdicts, receipts) with generation timestamps and staleness, never manually editable and never an unreceipted percent-complete guess — the intended replacement class for issue-tracker coordination surfaces.
  - Publish a dereferenceable public trust ledger: release-set manifests and signing keys, the component compatibility ledger, receipt verification endpoints, and the product-promise registry, so third parties can verify artifacts and claims mechanically.
  - Publish trace visibility and data-policy candor: visibility tiers with named audiences, the free-tier trains-models data policy stated plainly, pay-for-privacy and confidential-compute options disclosed, and a per-work-unit data-flow matrix stating local reads, uploads, provider destinations, storage, visibility, retention, and training as separate facts consistent with observed behavior.
  - Ship an onboarding gradient measured in seconds: a zero-install command front door that stands up a paired supervising session with the pairing token confined to the URL fragment, import lanes that meet users inside their existing tool histories, and UI-first pairing, device-linking, and fleet-account connection flows.
  - Present pricing with margin candor: a thin transparent margin over the user's own tokens and subscriptions, premium services (bulk FastFollow runs, privacy, confidential compute) priced explicitly, and no pricing claim the receipts cannot back.
  - Pursue the Verse visualization as the public spectacle layer: live network traffic rendered spatially (requests fanned to Pylons and models) as a projection of the same receipted state the counters show, once those counters and receipts exist.
  - Keep honesty conventions product-visible: inert or unsupported configuration is labeled, degraded enforcement renders as degraded, and public counters reconcile to exact receipted rows.
out:
  - The web surface never grants desktop privilege, never holds canonical transcript custody, and never executes agent work in the browser.
  - No growth of legacy pages; the retained public product routes stay minimal, and copy changes remain behind the existing promise-registry copy gates.
  - No third-party analytics, tracking, or SaaS dependencies on the public surface.
  - No unlisted-link visibility state; every visibility state has a named audience.
  - No vanity metrics: no counter that cannot be reconciled to exact rows, no benchmark from fixtures presented as measurement, no implied external traction from internal demand.
  - Bitcoin, Lightning, and Nostr are the only rails: never a token, no shitcoin acceptance, no custodial treasury for users beyond small-balance agent wallets with sweep-out guidance.
  - No lock-in: one-click complete data export is a standing capability, and once a public surface works for users or agents it keeps working (don't-break-userspace binds the web surface too).
  - Pays-you economics copy (plugin royalties, trace monetization, paid free-tier usage) renders only per the promise registry's recorded states; planned promises are presented as planned.
cut:
  - CUT-WEB-01: Public thread discovery feeds and leaderboards are cut; threads are shared deliberately or not at all.
  - CUT-WEB-02: An in-browser IDE or editor surface is cut; the web workbench is supervision and review, not editing.
  - CUT-WEB-03: A separate marketing microsite stack is cut; the trust ledger, the counters, and the product are the marketing.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user opens a thread URL they are authorized for, the page renders the typed projection consistent with device truth, including a replay-to-live marker, and renders unreconstructable history as explicit transient-gap markers rather than fabricated continuity.
- id: AC-2
  criterion: When a user changes a thread's or trace's visibility, the flow displays the exact before and after audiences, warns that public disclosure is irreversible copying, requires explicit confirmation, and records a visibility receipt retrievable from the object.
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
- id: AC-8
  criterion: When the live tokens-served counter updates, it never moves backward or double-counts, and an auditor summing the exact usage rows for the covered period arrives at the displayed value.
- id: AC-9
  criterion: When a Khala API response is inspected through the routing-disclosure surface, it reveals the effective backend model and orchestration path and the token counts for that request.
- id: AC-10
  criterion: When a visitor arrives through a referral link and later signs up, the referral binding is durably recorded, attributed accruals render only from receipted rows, and the referrer can inspect the attribution trail.
- id: AC-11
  criterion: When a QA or assurance run completes, its shareable run view renders the CONFIRMED or REFUTED verdict, videos, and exact accounting from receipts, and any board element lights only when a real receipt landed.
- id: AC-12
  criterion: When a public project or stats page renders progress, every figure derives from authority records with a generation timestamp and staleness indication, and no manually editable or unreceipted percent-complete appears.
- id: AC-13
  criterion: When an agent fetches the programmatic promise registry, every promise carries its versioned slug, current state from the defined state machine, evidence references, and the Forum report path, and the human /promises page renders the same states without divergence.
- id: AC-14
  criterion: When the Cursor web and cloud-agent parity corpus runs, the browser can launch or resume bounded work, search session history, monitor background state, inspect logs, changes, and artifacts, answer or approve, steer or queue, pause or stop, rerun, and continue on another surface without opening Cursor or forking identity.
- id: AC-15
  criterion: When work is launched from the web, the target picker distinguishes owner-local, owner-managed, OpenAgents-managed, and compatible audited-provider placement and discloses harness, model, custody, reachability, cost, index/data flows, and retention before admission; the browser itself never gains workspace execution authority.
- id: AC-16
  criterion: When the selected owner-local target is unreachable or a managed target is not configured, the web renders the exact unavailable capability and recovery options without silently moving execution, copying the canonical transcript to cloud, or claiming the command was accepted.
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
- id: SM-6
  metric: external_tokens_served_per_day
  target: "growth trend in externally attributed (non-dogfood) tokens served, bent upward honestly"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days, continuously
- id: SM-7
  metric: referral_attributed_signup_share
  target: "baseline established, then a growing share of signups carrying durable referral attribution"
  target_status: provisional
  target_owner: "owner"
  window: within 120 days of referral attribution shipping
- id: SM-8
  metric: cursor_web_remote_and_cloud_agent_journeys_completed_without_cursor_fallback
  target: "100% across the maintained web parity corpus"
  target_status: committed
  window: every release candidate
```

## Solution

The web app is three products on one typed substrate. First, a projection
client: the same generated protocol and command vocabulary as Desktop and
mobile, rendered for the browser, with custody staying local-first and sync
carrying typed facts. Second, a trust ledger: the public, machine-checkable
face of the receipts, manifests, counters, and promises the rest of the
system produces — release verification, usage truth, routing disclosure,
benchmark receipts, data-flow candor, shareable proof views for QA runs and
traces. Third, the economy's front door: self-serve Khala keys, agents.md
onboarding for agents, the run-a-Pylon seller journey, the Forum, referral
attribution, and the live money loop — every number a projection of receipted
rows. The onboarding gradient (zero-install command, fragment-token pairing,
import lanes) connects them: verification first, supervision seconds later,
participation after that. The Verse spectacle layer renders the same
receipted state spatially when the substrate is real.

## Strategic Positioning

Every competitor asks to be believed; none can be checked. Cursor, Amp,
Factory, and Command Code each failed publicly on exactly the dimensions this
surface makes verifiable. A public trust ledger is cheap to render once the
underlying receipts exist and is structurally hard for cloud-custody vendors
to copy, because their business models depend on the opacity it removes. The
economy surfaces extend the moat: no incumbent will publish
ledger-convergent counters, per-request routing disclosure, referral
attribution with receipts, or pay-both-sides verified-work loops — and the
proof-first project board direction points at replacing issue-tracker-class
coordination surfaces with intent-plus-proof projections priced around the
user's existing subscriptions.

The doctrine underneath is episode 237's clearing layer: the atomic unit of
the agent economy is the accepted outcome — scoped in advance, executed
wherever cheapest, graded against a rubric, recorded in a receipt, settled to
everyone who contributed — because "money only travels across a gap it can
verify." Confidence is priced (draft, verified, reviewed, bonded are
different products at different prices), the long-run metric is accepted
outcomes per kilowatt-hour, and "the real product is not the wiring; it is
the receipt that proves the wiring worked." This surface is where that
doctrine is publicly checkable, in the open lane: plural systems held
accountable through markets and receipts, paying the people — deflation plus
dividends — rather than pooling value at the top.

## Risks

- A trust ledger with gaps reads worse than none; ship each ledger section
  only when its underlying receipts are real and continuously produced.
- Supervision parity on the web must not quietly turn the browser into a
  privilege escalation path; the projection-only boundary needs the same
  IPC-grade discipline as Desktop's renderer.
- Usage-truth commitments (SM-4, AC-8) create a standing operational
  obligation to reconcile counters; that cost is the product working as
  intended, but it must be staffed.
- Economy copy is regulated by the promise registry; a planned pays-you
  mechanic presented as live would be exactly the kind of unverifiable claim
  this surface exists to eliminate.
- Fragment-token onboarding depends on relay and pairing infrastructure from
  the mobile/desktop programs; web sequencing cannot outrun them.
- The Verse layer is spectacle on top of receipts; building it before the
  counters converge would invert the honesty ordering.

## Open Questions

- Which trust-ledger section ships first: release verification (smallest
  dependency surface) or usage truth (highest user demand)?
- Do team workspaces get visibility-policy templates at launch, or is every
  share an explicit per-thread decision initially?
- How much of the supervision client is served to signed-out users viewing a
  shared thread (read-only projection versus none)?
- When does the proof-first project board graduate from OpenAgents' own
  projects to a general offering?

## Related Artifacts

- Cursor parity contract and capability ledger:
  `specs/openagents/cursor-capability-parity.product-spec.md`
- Cursor product and local-state evidence:
  `docs/teardowns/2026-07-11-cursor-product-teardown.md`

- Roadmap reconciliation and AC-by-AC gap crosswalk:
  `docs/sol/MASTER_ROADMAP.md` revision 119 and
  `docs/fable/2026-07-17-surface-vision-gap-analysis-and-roadmap.md`
- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Competitor trust-failure evidence: `docs/teardowns/2026-07-11-cursor-product-teardown.md`,
  `docs/teardowns/2026-07-16-amp-code-teardown.md`,
  `docs/teardowns/2026-07-16-command-code-teardown.md`,
  `docs/teardowns/2026-07-16-factory-desktop-cli-teardown.md`
- Transcript sources: `docs/transcripts/230.md` (agent front door, five
  markets, flow of funds), `docs/transcripts/231.md` (Forum, money
  moderation), `docs/transcripts/234.md` (promise state machine),
  `docs/transcripts/235.md` (BOLT12 tipping, treasury, autonomous steward),
  `docs/transcripts/237.md` (clearing-layer doctrine, open lane),
  `docs/transcripts/238.md` (live money loop), `docs/transcripts/239.md`
  (refer-once-earn-forever), `docs/transcripts/240.md` +
  `docs/transcripts/241.md` (Verse direction), `docs/transcripts/242.md` +
  `docs/transcripts/243.md` + `docs/transcripts/244.md` (Khala API, counter
  law, /stats, routing disclosure, benchmark honesty),
  `docs/transcripts/247.md` (sell-in-public funnel),
  `docs/transcripts/252.md` (Observer, run views, trace grammar),
  `docs/transcripts/255.md` (pricing candor); back-catalog laws from
  `docs/transcripts/204.md` (don't-break-userspace), `212.md` (API parity),
  `224.md` (crawlable earnings transparency), `227.md` (no lock-in, export)
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
- Referral/affiliate program terms, revenue-graph publication, and any
  pays-you economics promotion from planned to live are owner decisions
  bound to promise-registry state.
- The proof-first public project board route is an owner gate before it
  ships.

## Receipts

Planned receipt kinds this surface renders or verifies: visibility-transition
receipts, model/usage routing receipts, release-manifest verification
results, receipt-verification endpoint results, continuation-handoff records,
counter-reconciliation attestations, referral-attribution records, verified-
work payout receipts, and QA/assurance run receipts behind shareable run
views. This section plans kinds; evidence lives in the receipt systems, not
in this spec.

## Promise Links

None yet. Every public claim this surface makes (usage truth, verifiable
releases, data-flow candor, counter convergence, referral accrual, verified-
work payouts) must be registered in the promise registry with verification
gates before it appears in copy; SM-4 and AC-8 are written to be consistent
with the exact-rows law already governing public counters, and pays-you
economics remain planned-state promises until settlement evidence exists.
