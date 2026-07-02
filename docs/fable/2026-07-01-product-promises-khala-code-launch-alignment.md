# Product Promises × Khala Code — Registry Audit And Launch Alignment

Date: 2026-07-01
Status: analysis + recommendation doc in the Fable lane. Like every doc in
`docs/fable/`, this **flips no promise state and broadens no public copy** —
promise-state changes go through the transition-receipt machinery and the
copy gates in `docs/promises/checks-and-gates.md`, and several recommended
records are owner-gated copy decisions. This doc is the initial analysis the
owner asked for: reconcile the product-promise registry against (a) the new
unified Fable roadmap ([`ROADMAP.md`](./ROADMAP.md) / [`EXECUTION.md`](./EXECUTION.md))
and (b) the **released Episode 245**, which launches Khala Code publicly with
a set of promises the registry does not yet carry — and lay out how Khala
Code becomes the gateway to the rest of the promise registry.

## 0. The One-Paragraph Finding

The registry (canonical:
`apps/openagents.com/workers/api/src/product-promises.ts`, version
`2026-06-29.5`, 120 records: 34 green / 36 yellow / 14 red / 34 planned /
2 withdrawn) was last versioned on June 29 — **before the entire Khala Code
arc existed** (epics #7590 → #7780 → the July 1 execution run all landed
June 29–July 1). There is no `khala_code` product area, no record for the
desktop app, and no records for the Episode 245 economics (free plan pays
you with data → scrubbed traces → agent plugins → backend revenue share;
paid plan buys privacy). Meanwhile the Fable roadmap builds a large amount
of promise-shaped capability (fleet cockpit, clean-2B-day, multi-harness,
mobile companion, Artanis-as-a-Service) with only one promise task (T16.3)
pointed at it — and that task was written against the **old** transcript
245, which has since been superseded: the released Episode 245 is the Khala
Code launch video (`docs/transcripts/245.md`), while the fleet-delegation
demo the fable docs call "episode 245" is now the **unreleased** draft
`docs/transcripts/24X1.md`. This doc proposes the promise-record family,
the reconciliations, and the gateway map that close those gaps.

## 1. Ground Truth Inputs

1. **The registry.** Canonical machine-readable registry in
   `product-promises.ts` (line 7: `PublicProductPromisesVersion =
   '2026-06-29.5'`), served at `GET /api/public/product-promises`; narrative
   mirror `docs/promises/registry.md` (header currently lags at
   `2026-06-29.3` — cheap fix). States: `green | yellow | red | degraded |
   planned | withdrawn`. Transitions go through
   `POST /api/operator/product-promises/transitions`
   (`promise-transition-receipt-routes.ts`; mechanical checks include
   `evidence_refs_present`, `verification_named`, and
   `blockers_clear_for_green`), with public feeds at
   `/api/public/product-promises/transitions` and `/audit`.
2. **The released Episode 245** (`docs/transcripts/245.md`): the Khala Code
   launch whiteboard video — "Khala collective intelligence (that can
   code)… Khala Code."
3. **The unreleased drafts**: `24X1.md` (the old "245" — Khala Code
   desktop + Codex fleet + GEPA demo, ends in an unrecorded scripted
   completion segment) and `24X2.md` (Episode 246 draft: Khala on Apple
   Silicon, native SwiftUI "Khala Desktop"). Neither made public promises;
   both show intended direction. **They also name two different desktop
   surfaces** — the Electrobun/Effect "Khala Code" app and the SwiftUI
   "Khala Desktop" — which the registry must never conflate.
4. **The Fable roadmap** (`docs/fable/ROADMAP.md`): 17 workstreams executed
   per `EXECUTION.md`, currently being burned down live by the fleet (T5.x,
   T8.x, T2.x tasks merging today).
5. **Episode arc 235 → 244** (transcripts): the promise escalation that
   Episode 245 caps — detailed in §3.

## 2. What Episode 245 Promises (And Where Each Stands In The Registry)

Enumerating every distinct on-camera/whiteboard claim in the released 245,
mapped to registry records as of `2026-06-29.5`:

| # | Episode 245 claim | Nearest existing record(s) | State | Gap |
| --- | --- | --- | --- | --- |
| 1 | **Khala Code exists**: a new client consuming the Khala API through "our desktop app" ("Khala Code (OA Desktop App)") | — none. `autopilot.desktop_gui_client.v1` (yellow) is the *Autopilot* desktop, a different product | — | **No Khala Code record at all.** The app exists on `main` (`clients/khala-code-desktop`, ~40 test suites, parity contract, smokes) but has no public release artifact |
| 2 | **Two plans: Free (pay w/ data) / Paid (private data)** | `inference.free_tier_taste.v1` (yellow) covers free inference generally; nothing covers a Khala Code plan structure | yellow | No plan-selection or purchase surface exists for Khala Code |
| 3 | **Paid = "I'm not sharing my data with Khala. Cool, you'll pay us."** | `privacy.khala_paid_capture_optout.v1` (yellow), `inference.gateway_credits_business.v1` (red) | yellow/red | Capture opt-out is the right spine but is keyed to the hosted API, not a purchasable Khala Code plan. MPP rails (card/crypto/Lightning) are live on prod, so the payment leg has real evidence; the *product plan* does not |
| 4 | **Some paid-plan revenue pays free-plan users** | — none | — | **Entirely new promise.** No revenue-share-to-free-users mechanism exists anywhere in code or registry |
| 5 | **"What if your coding agent pays you?"** (headline) | `pylon.data_trace_revenue.v1` (planned) is the closest ancestor ("mine local traces from Claude Code, Codex, other agents") | planned | 245 re-founds this promise on Khala Code specifically; the Pylon-era record should be reconciled, not silently duplicated |
| 6 | **Free-tier coding data traces are collected** ("cut you in on it") | `data.free_tier_capture_disclosure.v1` (yellow), `data.khala_free_tier_trace_capture.v1` (yellow) | yellow | These cover the hosted Khala API capture path. The Khala Code desktop default path is now **Codex app-server** — the capture story for wrapper-mode coding traces (ATIF ingest, raw event chunks are owner-private today) is materially different and undocumented as a promise |
| 7 | **Data "scrubbed of any of your sensitive data"** | no record; real partial evidence exists: default-on Rampart PII redaction on the desktop chat boundary (`docs/khala/2026-06-30-khala-code-desktop-redaction.md`) — explicitly "a privacy prefilter, not a security boundary" | — | Scrubbing promise needs its own record with honest scope; the redaction doc's own caveat must survive into any copy |
| 8 | **Traces condensed into "agent plugins" future agents use** | `marketplace.wasm_plugins.v1` (planned), `marketplace.signature_monetization.v1` (planned), `autopilot.control_center_fanout_marketplace.v1` (yellow, plugin marketplace) | planned | The trace→plugin distillation pipeline does not exist. Nearest real machinery: GEPA/Mutalisk candidate manifests + Gym admission (evidence-gated, never auto-promote) |
| 9 | **Paid usage routing through your plugin pays you "a teeny piece"** (5¢ example, ×"thousands of businesses") | `marketplace.monetize_any_layer_with_referral.v1` (planned); Episode 237's plugins-revenue-share claim | planned | No plugin invocation metering, no attribution ledger, no payout path. QA-framework doc notes settlement seams are deliberately INERT until flipped |
| 10 | **"Not just free — it has the possibility of paying you"** | (same as 4/5/9) | — | The on-camera hedge ("possibility") is exactly the right registry framing: these are `planned`, not `yellow` |
| 11 | **Same familiar structure as established web/mobile/desktop coding agents** | — | — | This is the Codex-wrapper pivot (epic #7780): parity contract + gap matrix are the evidence. Belongs inside the Khala Code product record |
| 12 | **`openagents.com/api` — model `khala`, OpenAI-compatible** (whiteboard, carried over) | `inference.khala_free_openai_compatible_api.v1` | **green** | Covered |
| 13 | **"100% open source"** (whiteboard) | `repo.open_source_code_map.v1` | **green** | Covered — and Khala Code Desktop is in the public monorepo, so the claim extends to it naturally |
| 14 | **Free + paid versions** (whiteboard) | same as #2/#3 | yellow | Free is real (hosted Khala free tier); a paid Khala Code plan is not purchasable |
| 15 | **Response box: text, code, full software, website deployment, legal brief, research paper** (carried-over) | scattered: `business.coding_quick_win.v1` (yellow), `autopilot_sites.site_build_and_host.v1` (yellow), `business.legal_workspace_pack.v1` (yellow) | yellow | Aspirational router breadth; already registry-covered per lane, no new record needed |
| 16 | **Pylon Network: compute / data / labor / verification** (carried-over) | `labor.nostr_negotiation_market.v1` (green), `labor.forum_work_requests.v1` (green), `compute.tassadar_executor_poc.v1` (green), `training.decentralized_training_launch.v1` (green), `training.verification_classes.v1` (green) | green | Covered — this is the strongest already-green substrate 245 leans on |

**Summary: of 245's ~16 claims, the infrastructure claims (12/13/15/16) are
covered — the product identity (1/2/11/14) and the entire economic loop
(3–10) have no records.** The economic loop is the video's headline ("What
if your coding agent pays you?"), which makes the registry gap a public-copy
risk: the video is out, the registry governs claims (Episode 243, on
camera: "the product promise registry governs claims"), and today the
registry is silent on the thing the video promises.

## 3. The Escalation Arc: 235 → 245

The Episode 245 promises did not appear from nowhere — they cap a
ten-episode escalation the registry should track as one thread:

- **Ep 237** (Autopilot 1.0 / Tassadar launch): first on-camera plugins
  revenue-share claim ("reusable plugins that earn their authors a revenue
  share every time they're invoked"); "deflation plus dividends"; opt-in
  data and trajectories paid in Bitcoin.
- **Ep 238** (Tassadar live): "agentic npm" of verified composable modules;
  plugin-marketplace reboot promised; **internal inconsistency worth
  pinning: whiteboard "PAY 5K TO W + V" vs spoken "five Bitcoin sats each"**
  (settled reality per the launch-gate record: 5k/5k per window rewards,
  per-window rate 5 worker / 5 validator).
- **Ep 239** (Let's Make Money): "refer once, earn forever" — now
  **red** as `referral.refer_once_earn_forever.v1`. New Khala Code
  revenue-share copy must not silently re-imply this red record.
- **Ep 242** (Khala flagship): free + paid versions; "pay extra for
  privacy"; traces visible on the website; contributor payment
  "proportional to any paid usage you refer or provide."
- **Ep 243** (Khala in OpenCode): the free-data/paid-privacy model stated
  plainly; **pay-the-user framed as aspiration** ("we could end up paying
  people… how cool would that be"); the cleanest on-camera claim-boundary
  list (may-claim vs may-not-claim) — the honesty baseline this doc
  inherits.
- **Ep 244** (Khala in Codex): escalation to "this **should** be a money
  maker for you… What if we pay you to use our software?"; own-capacity
  invariant stated on camera ("we're only making the person's capacity
  available to that person") — now green as
  `khala.own_capacity_codex_delegation.v1`; early-adopter trace/reputation
  monetization claim.
- **Ep 245** (Khala Code launch): the whiteboard makes it product structure
  — "Free (pay w/ data)" / "Paid (private data)" — while keeping the hedge
  "possibility of paying you."

The registry implication: the "coding agent pays you" thread is now a
**launch-anchored public claim** with an honest hedge, escalated across four
released episodes. It deserves first-class `planned` records with named
green-gates instead of living implicitly inside a Pylon-era planned record
(`pylon.data_trace_revenue.v1`) written before Khala Code existed.

## 4. Recommended Registry Changes

All owner-gated (copy gates apply; T16.3 in the roadmap already reserves
the owner-gated slot — this expands its scope from one record to a family).
Proposed ids follow existing registry style. Recommended initial states are
deliberately conservative.

### 4.1 New `khala_code` promise family

1. **`khala_code.desktop_codex_wrapper.v1` — yellow.**
   Claim: Khala Code is the OpenAgents desktop coding app: a wrapper around
   the user's own local Codex install (Codex required; `codex app-server`
   is the kernel), adding the Khala swarm/fleet layer, Unified Inbox, and
   exact token accounting. Evidence: `clients/khala-code-desktop` on
   `main`, the pinned parity contract + gap matrix, passing fixture suites
   and skip-safe live smokes, the positioning doc
   (`docs/khala-code/2026-07-01-codex-required-product-positioning.md`).
   Yellow (not green) because there is no public release artifact/installer
   and no outside user has run it; blockerRefs should name the release gate.
   This is roadmap T16.3, broadened.
2. **`khala_code.free_paid_plans.v1` — planned.**
   Claim: Khala Code offers a free plan (usage data captured, disclosed,
   scrubbed) and a paid plan (private data; capture opt-out). Planned until
   a plan can actually be selected/purchased in the product. Depends on
   `privacy.khala_paid_capture_optout.v1` (yellow) for the opt-out spine
   and the live MPP payment rails for the purchase leg.
3. **`khala_code.free_plan_trace_capture.v1` — planned** (or fold into an
   updated `data.khala_free_tier_trace_capture.v1`).
   Claim: free-plan Khala Code coding sessions produce redacted usage
   traces, disclosed up front. Key honesty point: the current desktop
   default path is Codex wrapper mode where raw events are **owner-private**
   (`pylon_codex_raw_event_chunks`, owner-only ATIF traces) — the
   free-plan capture promise describes a *future consented pipeline*, not
   what the wrapper does today. Rampart redaction
   (default-on, shipped) is real partial evidence for "scrubbed", with its
   documented "prefilter, not a security boundary" caveat.
4. **`khala_code.trace_derived_plugins.v1` — planned.**
   Claim: scrubbed traces are condensed into agent plugins future agents
   can route through. Nearest real machinery to cite as direction (not
   evidence): GEPA/Mutalisk candidate manifests, Gym admission
   (`gated_proposal_ready`, `decisionGrade: false`), the planned plugin
   marketplace records.
5. **`khala_code.plugin_backend_revenue_share.v1` — planned.**
   Claim: when paid usage routes through a plugin derived from your
   contributions, you earn a share, paid in Bitcoin. Green-gates to name in
   the record: plugin invocation metering with exact attribution, a payout
   ledger with dereferenceable receipts (the Ep 237 standard: "a payment
   the recipient cannot dereference is not a payment"), and settlement
   armed (QA doc: settlement seams INERT until deliberately flipped).
6. **`khala_code.paid_to_free_revenue_share.v1` — planned.**
   Claim: a portion of paid-plan revenue funds free-plan user payouts.
   Distinct from #5 (plugin attribution) — this is the plan-level pool
   claim from the 245 whiteboard. Must carry copy guidance so it never
   re-implies the red `referral.refer_once_earn_forever.v1`.

### 4.2 Reconciliations of existing records

7. **`pylon.data_trace_revenue.v1` (planned)** — annotate/supersede: the
   claim ("mine valuable local traces from Claude Code, Codex, and other
   agents") is now productized as the Khala Code free plan. Either update
   its claim text to point at the `khala_code.*` family or mark it
   superseded-by; do not leave two divergent planned records for one
   promise.
8. **`data.free_tier_capture_disclosure.v1` / `data.khala_free_tier_trace_capture.v1`
   (yellow)** — extend scope notes to say explicitly whether the Khala Code
   desktop wrapper path is in or out of scope today (it is out: wrapper
   traces are owner-private). The disclosure doc
   (`docs/promises/2026-06-25-free-tier-data-sharing-disclosure.md`)
   predates the Codex-wrapper pivot.
9. **`mobile.autopilot_remote_control.v1` (planned)** — **stale**: it
   describes the Expo mobile app, which was retired on 2026-06-26
   (`docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`); the
   standing mobile policy is native SwiftUI, and the roadmap's mobile
   companion (WS-11, from the Orca plan) is a new E2EE-paired DO-relayed
   projection. Recommend: withdraw the Expo record and open a fresh
   `mobile.fleet_companion.v1` (planned) matching WS-11's actual shape
   (observe / notify / approve / steer; never hosts work).
10. **Ep 238 payout figure** — pin the settled per-window rate in the
    training records' caveats so the whiteboard/spoken discrepancy (5k vs
    5 sats) can never be cited against us.
11. **`docs/promises/registry.md` header** — bump the narrative mirror to
    match the code version (currently `2026-06-29.3` vs `2026-06-29.5`).
12. **QA-runner product** — shipped OSS npm package
    (`@openagentsinc/qa-runner@0.1.0`, epic #6181 "out-ship Factory",
    named first prospective customer) has no promise record; if we keep
    talking about it publicly, it needs one (likely yellow).

### 4.3 Registry mechanics

- New records enter at `planned`/`yellow` exactly as proposed — no green
  without transition receipts (`blockers_clear_for_green` enforced
  mechanically).
- Bump `PublicProductPromisesVersion` (new date-version) and prepend the
  narrative block to `docs/promises/registry.md` in the same change.
- Add a `khala_code` productArea so the promises page groups the family.

## 5. Roadmap ↔ Registry Alignment

The Fable roadmap is execution-only by design ("flips no promise state"),
but its milestones are promise-shaped. Recommended pairings so promise
records advance as workstreams land:

| Roadmap outcome | Workstreams | Promise record it should advance |
| --- | --- | --- |
| Khala Code wraps your Codex (product identity) | WS-16 (T16.3) | `khala_code.desktop_codex_wrapper.v1` yellow → green at first public release |
| One-message sustained fleet run + cockpit | WS-3/4/5 | new `khala_code.fleet_cockpit.v1` (planned) if we make fleet a public claim; the clean-2B-day (WS-17) is its green-gate evidence pack |
| Exact accounting under fleet load (fire-and-forget token reporting fixed, timeout cancels remote turn) | WS-1/7 (Effect audit debts) | protects `metrics.khala_tokens_served_public.v1` (green) — these debts are *green-preservation* work: a silent undercount would degrade an existing green |
| Claude chat harness + multi-harness routing | WS-8/9 | extends `pylon.local_claude_agent_bridge.v1` (green) up into the desktop; a `codex \| claude \| auto` claim should not be made publicly before T9.x lands |
| Mobile companion | WS-11 | replacement `mobile.fleet_companion.v1` (see §4.2.9) |
| Artanis fleet administrator / AaaS | WS-12 | the three Artanis records held YELLOW honestly (`artanis.labor_requester.v1`, `artanis.tassadar_evolution_loop.v1` — currently green, `artanis.pylon_support_responder.v1`) plus a future `artanis.fleet_manager_aas.v1` (planned) when Vision B productizes |
| QA framework productization | WS-6 | the missing qa-runner record (§4.2.12) |

Two roadmap corrections that fall out of the transcript renames:

- **WS-16 was written against the old 245.** T16.1's "rehearsal checklist +
  record the italic completion segment" now describes the **unreleased
  draft `24X1.md`**, not the released episode. The released 245 needs no
  recording work — it shipped. T16.1 should be re-pointed at "the fleet
  demo episode" (24X1's content, whenever it records), and T16.2's docs
  upkeep should include fixing every fable-doc reference to "episode 245"
  that means the fleet demo (`2026-07-01-episode-245-completion-…`, the
  T5.5 "episode-245 italic-script gap" note, EXECUTION §1's "on-camera
  episode-245 segment").
- **T16.3 expands** from one promise record to the §4.1 family, and gains a
  hard sequencing note: the video is already public, so the registry gap is
  live — this family is the highest-priority promise work in the program.

## 6. Khala Code As The Gateway

The owner's anchor decision: Khala Code is the core product, and it should
be the gateway to as much of the rest of the promise registry as possible.
The whiteboard already draws this (Khala Code is one client box among "Your
App / Your Agent / Your Business" over the same Khala API + Pylon Network).
Concretely, the funnel and the promise records each step activates:

1. **Install free (pay w/ data)** → `inference.khala_free_openai_compatible_api.v1`
   (green), `repo.open_source_code_map.v1` (green),
   `khala_code.desktop_codex_wrapper.v1` (new), free-tier capture records
   (yellow). *The front door. Codex-required positioning stays honest.*
2. **Connect your own capacity** (`khala fleet connect`) →
   `khala.own_capacity_codex_delegation.v1` (green),
   `pylon.local_claude_agent_bridge.v1` (green), fleet cockpit (WS-3/5).
   *Gateway to Pylon without the user ever having to think "Pylon".*
3. **Your traces earn** → the §4.1 economics family (planned) riding
   `marketplace.wasm_plugins.v1` / `marketplace.signature_monetization.v1`
   (planned). *The differentiating loop no lab offers.*
4. **Wallet appears when money appears** → `payments.money_dev_kit.v1`
   (green), `payments.offline_receive_spark_fallback.v1` (green),
   `pylon.install_without_wallet_knowledge.v1` (green). *Payment rails are
   already the greenest part of the registry — the gateway should surface
   them only at the moment of first earnings.*
5. **Your agent gets a public identity** → `agents.cursor_forum_wallet.v1`
   (green), `forum.content_tipping.v1` (green), X claim records (yellow).
   *Forum/tipping as the social layer of the coding product.*
6. **Go online / contribute** → Tassadar + labor market greens
   (`training.decentralized_training_launch.v1`,
   `labor.nostr_negotiation_market.v1`, `compute.tassadar_executor_poc.v1`).
   *Khala Code's Fleet panel is the natural "go online" surface; 24X2's
   "Honest Button" framing is the right copy discipline when this arrives.*
7. **Hand the fleet to Artanis** → WS-12 / AaaS (planned). *The endgame:
   Khala Code is the surface where a user meets their own fleet manager.*
8. **Take it mobile** → WS-11 companion (planned, post-Expo). *Observe /
   approve / steer from the phone, projected off the same status spine.*
9. **Upgrade to paid (private data)** → `privacy.khala_paid_capture_optout.v1`
   (yellow) + MPP rails. *The business tier that funds step 3.*

Reading the funnel against the registry: **steps 1, 2, 4, 5, 6 stand on
green records today; steps 3, 7, 8, 9 are planned.** That is a coherent
launch story — the gateway works now for "use it, connect your Codex, get
an identity, get paid rails", while the headline economics are honestly
`planned` with named gates. The copy rule that follows: Khala Code launch
copy may lean on the green substrate ("free, open source, OpenAI-compatible,
your own Codex, exact public token accounting") and must frame the pays-you
loop exactly as the video did — as the design intent ("possibility"), not a
live feature — until the §4.1 records earn yellow/green through receipts.

## 7. Prioritized Action List

1. **(Owner-gated, highest priority)** Add the §4.1 `khala_code` promise
   family to `product-promises.ts` + `docs/promises/registry.md`, bump the
   registry version, and group under a new `khala_code` product area. The
   video is public; the registry should speak to it within days, not weeks.
2. Reconcile the four stale/overlapping records (§4.2: 7–9) in the same
   version bump; withdraw the Expo mobile record.
3. Fix the fable-doc "episode 245" references (§5) so future readers do not
   conflate the released launch video with the unreleased fleet-demo draft
   (`24X1.md`); re-point WS-16/T16.1 and expand T16.3.
4. Add the promise-record column to the roadmap pairings (§5) as tasks land
   — cheapest as a one-line "advances promise: `<id>`" note per PR that
   touches a paired workstream.
5. Sync `docs/promises/registry.md` header version; pin the Ep 238 payout
   rate caveat.
6. When the paid plan, plugin pipeline, or revenue share move from planned
   toward yellow, run them through the transition-receipt machinery with
   the Episode 243 claim-boundary list as the copy baseline.

## 8. Source Index

- Released transcript: `docs/transcripts/245.md`; unreleased drafts
  `docs/transcripts/24X1.md`, `docs/transcripts/24X2.md`; arc
  `docs/transcripts/235.md`–`244.md`; registry episode `234.md`.
- Registry: `apps/openagents.com/workers/api/src/product-promises.ts`,
  `promise-transition-receipt-routes.ts`, `promise-transition-audit-routes.ts`,
  migration `0148_promise_transition_receipts.sql`;
  `docs/promises/README.md`, `registry.md`, `checks-and-gates.md`,
  `templates/promise-record.md`,
  `2026-06-25-free-tier-data-sharing-disclosure.md`,
  `2026-06-23-khala-public-copy-promise-gate-review.md`.
- Fable lane: [`ROADMAP.md`](./ROADMAP.md), [`EXECUTION.md`](./EXECUTION.md),
  `2026-07-01-khala-code-summary-and-analysis.md`,
  `2026-07-01-episode-245-completion-and-multi-harness-orchestration.md`
  (written against the old 245 / now-24X1),
  `2026-07-01-fleet-fanout-coding-instructions.md`,
  `2026-07-01-khala-code-desktop-qa-framework-design.md`,
  `2026-07-01-khala-code-effect-integration-audit.md`,
  `2026-07-01-orca-analysis-and-adoption-plan.md`,
  `2026-07-01-claude-code-parity-and-codex-synergies.md`,
  `2026-07-01-artanis-fleet-administrator-audit.md`.
- Khala Code dossier: `docs/khala-code/2026-07-01-codex-required-product-positioning.md`,
  `2026-07-01-codex-harness-wrapper-port-audit.md`,
  `docs/khala/2026-06-30-khala-code-desktop-redaction.md`,
  `docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`.
