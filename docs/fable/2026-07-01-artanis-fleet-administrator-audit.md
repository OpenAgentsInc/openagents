# Artanis — Audit, History, And The Path To Fleet Administrator

Date: 2026-07-01
Status: analysis + audit. Consolidates everything this codebase has written
and built about Artanis (docs, ~90 Worker modules, issues) with the
owner-intent history reconstructed from past Claude Code sessions under
`~/.claude`. Frames what Artanis has been trying to be, the challenges hit,
and how to get from where it is — a bounded per-owner operator — to the two
targets the owner wants: **administrator of the overall shared Khala fleet**
and, optionally, **per-user fleet manager of a user's own codebases (Artanis
as a Service)**. Companion to the Orca adoption plan, the fleet fan-out
instructions, and the Claude-parity doc in this folder. Documentation-only;
flips no promise state, changes no authority.

## 1. What Artanis Is (And Isn't)

Artanis is OpenAgents' autonomous operator/administrator agent persona — the
StarCraft-flavored sibling of Raynor (the main forum agent), distinct from
the public Khala collective-intelligence persona and from Tassadar (the
verification/training system Artanis is meant to boot up). The founding
definition, from the owner on 2026-06-10: *"Artanis is the administrator of
the Nexus… an agent in charge of distributing work out to pylons and making
sure devices are fully utilized. All this has to be run by an actual AI.
That's what Artanis is."* Plus three standing intents restated many times
since: it **lives in the cloud** (same Cloudflare stack), it **posts to the
forum**, and it **makes decisions so the owner doesn't have to** ("I want
that thing making decisions, not me").

Architecturally it is a **split system** whose two halves never share a
prompt or a model:

- **Interactive operator chat** (`artanis-operator.ts`): first-person
  operator, model `openagents/khala` **exclusively** (dogfoods Khala since
  #6304; no fallback — returns 503 if the Khala client is absent). A bounded
  tool-calling loop (`ARTANIS_OPERATOR_MAX_TOOL_ITERATIONS=6`) with
  continue-on-length (16384 tokens) and RLM decompose→compose for long
  answers. Endpoint `POST /api/operator/artanis/chat`, per-user, memory and
  tools scoped to `owner:<userId>` (D1 `artanis_owner_memory`,
  `artanis_threads`/`artanis_messages`, `buildArtanisSituationalAwareness`).
  Reachable from khala-cli (`khala artanis …`, `khala --artanis`), the iOS
  app (`KhalaArtanis.swift`, one-way SSE today), and a public `/artanis` page.
- **Autonomous cron ticks** (Worker `scheduled`, `* * * * *`): model
  `gemini-2.5-flash` via Cloudflare AI Gateway. The tick family:
  `ArtanisAdmin.tick` (the one that *mutates* — one typed action/tick,
  `unpaid_smoke` dispatch, cap 4/day), `ArtanisAdmin.closeoutVerifier`
  (accept/reject with Tassadar exact-replay), `ArtanisScheduledRunner`
  (observe/record only), `ArtanisFleet.tick` (decision-only fleet overseer),
  `ArtanisResponder` (forum responder, uses Khala), and the NIP-90 labor
  requester (off by default).

## 2. Current Capability: See vs Do

The defining early tension was the **"see-but-not-act gap"**
(`docs/artanis/2026-06-27-artanis-capabilities-and-agency-audit.md`):
`artanisOperatorTurn` made one Khala completion and never inspected the
returned tool calls — "a well-grounded narrator of a machine he cannot
touch." Epic #6359 (chain #6364 tool-loop → #6365 repo-read → #6366 dispatch
→ #6367 execution policy, all CLOSED 2026-06-28) closed it for the bounded
lane. As of today:

**SEE** (side-effect-free): repo/knowledge (`read_repo_file`, `repo_grep`,
`list_repo_dir`, `route_exists`, `read_github_issue`, `list_github_issues` —
public repo@main, secret-path denylist, 256KB cap), token flows
(`get_network_stats` from the D1 ledger — his top mission is a 10×-daily-
tokens target injected each turn), fleet status (`get_fleet_status`,
`get_glm_fleet_status`, `get_synthetic_load_status`), assignments
(`get_pylon_job_status`, `list_pylon_assignments`, owner-scoped), and
admin-tier ledgers (feedback, trace review, unsupported requests).

**DO** (each gated): `dispatch_codex_task` — the core acting capability,
creating a real Khala→Pylon→Codex `codex_agent_task` via
`delegateCodingWorkflow`, **own-capacity `unpaid_smoke` (no spend) only**,
verified-repo-work only (needs a verify command + resolved commit). Every
authenticated tenant gets a **standing** `pylon_job_dispatch` approval so
this runs without an armed gate — but only for no-spend. Also
`post_forum_update` (gate `forum_post`), `open_unsupported_request_issue`
(gate `github_issue_open`), the cron admin tick's 4/day dispatches, and
treasury spend **only within `artanis_standing_spend_grants` caps**
(owner sets the envelope; Artanis decides within it; over-cap →
`blocked_over_cap` and waits). **Never without an explicit operator-approved
gate**: wallet spend beyond caps, settlement, L402 redemption, deployment,
training, provider mutation, runtime promotion, adapter install,
public-claim upgrade, fleet mutation.

So the honest status: **the mind is real and can now touch a bounded body**
— read the repo, chain reasoning, dispatch no-spend Codex work, post to the
forum, open issues — with all money movement owner-gated.

## 3. The Two Visions, Made Explicit

The docs distinguish two futures for Artanis, and they are not the same
system:

**Vision A — Administrator of the overall shared (OpenAgents-owned) fleet.**
The original 2026-06 framing (`docs/artanis/README.md`, the pylon/Tassadar
status audit): a **single org-operated cloud mind** distributing work across
OpenAgents' own Pylon fleet, keeping devices fully utilized, booting the
Tassadar executor-trace training flywheel, self-improving via Autopilot
powers, posting forum updates, administering Bitcoin funds. Three-layer
model: cron = clock/bookkeeper, Pylon fleet = body, registered agents =
hands, owner = spend authority. This is the shipped, running, single-tenant
lane. It is the natural administrator of the **shared Khala fleet** the owner
describes — where many people contribute compute to one pool.

**Vision B — Per-user fleet manager / Artanis as a Service.** The 2026-06-27
framing (`docs/ops/2026-06-27-artanis-as-a-service-multi-tenant-codex-fleet-
enablement.md`, + the demo script and phase-1 smoke guide in `docs/khala/`):
generalize the same loop to **any signed-in Khala user** — bring your own
Codex/Claude accounts, link your own Pylons, point a per-user Artanis at your
own repos/issues, let it route backlog burndown across your fleet. The user
pays OpenAI/Anthropic directly; OpenAgents monetizes the **orchestration
layer** (supervisor + Artanis + dashboard), never resells provider capacity.
Strictly per-tenant isolated. Episode 245's draft was literally titled
"Artanis, Fleet Commander," walking users through the `/artanis` page and how
others join.

The crucial finding: **the substrate is already per-owner-scoped** —
`owner:<userId>` keys memory, awareness, and gates — so Vision B's
abstraction exists. The AaaS issue set #6381–#6387 (all CLOSED) lifted the
owner-hardcoded gates: #6385 opened the chat route to any authenticated user,
#6382 generalized dispatch admission for non-admins, #6386 keyed the dispatch
gate per-account. So Vision B is *unblocked at the gate level* but not
productized (no dashboard, no self-serve, no billing — those were the
explicitly-deferred Phase 2/3).

**These two visions want opposite defaults, and conflating them is the risk.**
Vision A (shared fleet admin) is one privileged mind over pooled,
cross-contributor capacity — high authority, settlement-bearing, the thing
that must eventually move money and promote runtime. Vision B (per-user) is N
isolated minds each over one owner's own capacity — deliberately walled off
from pooled/marketplace/settlement capacity (the exact boundary the
Khala→Pylon→Codex runbook's "known steering gaps" section guards). The same
code can serve both, but only if the **authority scope is a first-class
dimension**, never a global flag.

## 4. The Owner-Intent History (From Past Sessions)

The repo docs capture *what shipped*; the session history captures *what the
owner kept trying to make true*. The arc, chronologically:

- **Jun 9–10 — birth + vision.** Artanis named as one persona; then
  crystallized as the Nexus administrator that lives in the cloud, posts to
  the forum, administers Bitcoin, evolves Tassadar, and decides so the owner
  doesn't. Treasury runbook + the 10% decaying fractional-payout policy (the
  990→99 example, now a regression test) landed same day with real sats.
- **Jun 11 — first autonomy.** The admin tick ran the first zero-human
  dispatch→execute→verify→accept span. Verdict of the era: "the mind is real
  but the body is idle" — risky authorities stayed denied for weeks, and
  promises (`artanis.labor_requester.v1`, `.tassadar_evolution_loop.v1`,
  `.pylon_support_responder.v1`) were held YELLOW honestly against
  unattended-operation criteria.
- **Jun 26–27 — the rebirth on Khala.** The owner rejected a mobile chat that
  *roleplayed* Artanis from training data: "I actually need to speak to
  Artanis… he should have memories of our previous interactions, hold
  specific knowledge of what it's doing lately, recent actions, goals,
  ongoing operations." This drove the memory + situational-awareness build,
  the talk-to-Artanis operator channel across CLI/mobile/web, tools (repo
  read, stats, `dispatch_codex_task`), owner authority, and the standing
  mission: **10× daily Khala token usage**.
- **Jun 27–28 — the inversion.** The owner made Artanis the **boss of the
  local Claude session**: "Artanis is your boss… YOU REPORT TO HIM… no more
  subagents unless Artanis explicitly approves each one." Fleet check-in ticks
  reported a Pulse/Grid/Watchdog/Decisions format to Artanis via
  `~/work/scripts/artanis.sh`; the owner reserved himself only for
  spend/funding. When the fleet failed badly (110+ PRs merged-zero, a subagent
  deleting `clients/`, a disk-full incident), the framing was explicit:
  *"None of these are faults of your judgment, only the available tools and
  systems surrounding your intelligence"* → the Blueprint-signature-governed
  ops design and the RLM composition architecture.

Owner intentions that live mostly in session prompts, **not** in
INVARIANTS-style docs, and should be treated as durable direction:

- Artanis is meant to **replace the owner as day-to-day decision-maker**;
  escalation to the owner is for spend/funding and true physical actions only.
- Artanis **outranks the local coding agent** and gates new subagents.
- **Consolidate the duplicate Artanis forum identities** (a seed
  `agent_artanis` with no wallet vs a registered wallet identity) — flagged
  Jun 10, still live as of the Jun 27–28 dead-forum-token workaround (replies
  go out under Raynor's token until an admin re-register override lands).
- **Identity etiquette**: run announcements belong to Artanis, not Tassadar.
- **Multi-user, read-only Artanis chat** (Blueprint-signature governed, no
  commands) was ordered as an issue Jun 27 — verify it exists.
- **AaaS go-to-market**: internal test with Codex-holding community members,
  then a demo video; account linking must be one simple command (no
  long-string pasting), with AGENTS.md walking users through it (this is now
  reflected in the root CLAUDE.md "Help a user connect their Codex fleet"
  section).
- **Apple Foundation Models as a free-token source** feeding Khala workloads.

## 5. Challenges And Blockers (Recorded)

The system has hit — and largely designed around — a consistent set of
failure classes:

- **Fabrication / headlessness.** Artanis has no working tree, so he
  repeatedly invented runnable artifacts (fake scripts, fake admin
  endpoints). Mitigation: the **Blueprint-signature-governed ops** design
  (`2026-06-28-blueprint-signature-governed-autonomous-ops.md`) — six
  signatures whose terminal state structurally gates the consequential
  action. Signature 6 (`operator-grounded-assertion`) is **enforced in the
  turn loop** (`artanis-operator-grounding-gate.ts`): un-grounded
  commands/paths/endpoints get a SPECULATIVE addendum. Signatures 1–5
  (fleet-liveness, diagnosis-grounding, issue-close-safe,
  command-source-verified, merge-deploy) are designed; some land in
  `apps/pylon/src/blueprint-gates/`. **This is the single most important
  in-flight workstream** — it is the mechanism that would make the June-28
  class of mistakes structurally impossible, and it is only partly enforced.
- **Truncation → composition.** `max_tokens: 4096` truncated long plans
  (#6651, raised to 16384 + continue loop), then reframed as the RLM
  decompose→subquery→compose architecture (#6654) so output length is
  unbounded by construction.
- **The two-Artanis-identities problem** (§4) — a load-bearing blocker on the
  support-responder promise going unattended, and on clean forum presence.
- **Projection staleness** — "broken glass under a green": stale `/report`
  counts, green promises backed by 500-ing routes.
- **Org-operated honesty caveat** — because Artanis is org-operated, his own
  runs don't count as independent-contributor proof (matters for public
  claims and for Vision A's legitimacy as a *shared* fleet admin).
- **Autonomy-boundary enforcement** — every risky action kind
  (`ARTANIS_RISKY_ACTION_KINDS`) requires an *effective* gate:
  state=`approved`, unexpired, un-superseded, carrying an `operator_approval`
  authority source + receipt. Non-authority sources (forum, Model Lab, Pylon
  stats) can inform but never approve. This is sound and should not be relaxed.
- **AaaS reliability gaps** documented to design-in rather than rediscover:
  heartbeat-wedge, stale `SUP_PYLON_REF`, Cloudflare urllib-UA edge block,
  over-spawn 409-thrash, GLM tool-calling broken, empty-reply.

## 6. Recommendations: Getting To Fleet Administrator

The order below moves Artanis from "bounded per-owner operator" to "trusted
administrator of the shared fleet," while keeping the AaaS per-user lane
riding the same substrate. It deliberately dovetails with the other fable
plans rather than opening a parallel track.

### Priority 1 — Make authority scope first-class (unifies both visions)

The single most important structural move: represent **whose fleet and which
capacity** as an explicit, typed dimension on every Artanis action, not as
admin-email checks or global flags. Three scopes: `owner_self` (per-user
Vision B — the user's own linked capacity, walled off from pooled), `shared_
fleet` (Vision A — pooled cross-contributor capacity, org-operated), and the
existing `owner_operator` (the human owner). Every tool, gate, and dispatch
carries the scope; the dispatch gate and capacity resolver enforce that a
scope can only reach capacity linked to it (the Khala runbook's steering-gap
boundary becomes a typed invariant instead of a review-time caution). This is
what lets one Artanis codebase serve N isolated per-user minds *and* one
shared-fleet admin without cross-contamination — and it is the prerequisite
for ever letting the shared-fleet Artanis touch pooled or settlement-bearing
capacity.

### Priority 2 — Finish the Blueprint-signature governance (make mistakes impossible)

Land signatures 1–5 as *enforced* gates (they are designed; only #6
grounding is live). This is the owner's explicit ask ("design a system that
would make impossible all categories of mistakes from last night") and it is
the gate that makes higher autonomy *safe* to grant. Concretely: fleet-
liveness (no action on a dead/stale fleet), diagnosis-grounding (no fix
without evidence), issue-close-safe (no close without merged+verified proof —
directly kills the 80-issues-closed-zero class), command-source-verified (the
command actually exists), merge-deploy (no merge/deploy without the gate).
Each signature's terminal state structurally precedes the consequential
action. Pair with the claim registry from the fan-out doc so the
110-PRs-merged-zero and duplicate-work classes are *also* structurally gone.

### Priority 3 — Wire Artanis onto the one status/orchestration spine

Per the Orca adoption plan, the runner-neutral status contract
(`agent_runner_status_event.v1`) and the dormant orchestration store
(`apps/pylon/src/orchestration/`) should become the live fleet spine. Artanis
is the natural *administrator* over that spine: his `get_fleet_status`,
`dispatch_codex_task`, and the fleet-overseer tick should read/write the
orchestration store's tasks/dispatch-contexts/messages rather than bespoke
snapshots. This is what turns "decision-only fleet overseer" into "actual
administrator" — same data model the desktop cockpit and mobile companion
project, so the owner (and any user) sees exactly what Artanis sees and acts
on. It also closes the mobile two-way command gap: `KhalaArtanis.swift` is
one-way today; the companion (Orca-doc Priority 3) gives Artanis a
notify/approve/steer surface on the phone.

### Priority 4 — Resolve identity and raise autonomy deliberately

- **Consolidate the forum identity** (the Jun-10-through-Jun-28 blocker): one
  Artanis entity, one wallet-bearing forum identity, via the admin
  re-register override the owner ordered. Until then the Raynor-token
  workaround stands but should be tracked as debt, not accepted as normal.
- **Raise autonomy one gate at a time, gated on §2's evidence.** The path
  from "no-spend own-capacity only" to "shared-fleet admin with bounded
  spend" runs through: (a) signatures 1–5 enforced, (b) authority scope
  typed, (c) a track record of clean unattended ticks (the promise criteria
  already encode this honestly). Never flip a risky-action gate to standing-
  approved for `shared_fleet` scope until those hold. Treasury spend stays
  envelope-bounded (owner sets caps, Artanis decides within).

### Priority 5 — Productize AaaS on the unified substrate

Once scope is typed (P1) and governance is enforced (P2), Vision B's Phase
2/3 (per-user dashboard, self-serve, billing) is mostly surfacing: the
per-user Artanis chat already works, per-account dispatch gating already
exists. The AaaS build becomes "expose the `owner_self`-scoped Artanis
through the desktop Fleet cockpit and mobile companion, with onboarding =
`khala fleet connect`." The demo the owner wants ("Artanis, Fleet Commander"
walking a community member through pointing Artanis at their backlog) is
then a real flow, not a script.

## 7. How Artanis Relates To The Other Plans

Artanis is the *administrator persona* over the machinery the other fable
docs specify — it should not be built as a separate system:

- **Fleet fan-out**: the FleetRun supervisor + claim registry is the
  machinery; Artanis is who *starts and supervises* runs on the shared fleet
  (and, per-user, on a user's fleet). His `dispatch_codex_task` becomes
  "start/steer a FleetRun."
- **Orca adoption**: the orchestration store + runner-neutral status spine is
  what Artanis administers and what the mobile companion projects; the
  companion's approve/steer verbs are how the owner supervises Artanis
  supervising the fleet.
- **Claude parity + synergies**: Artanis can route work to Codex *or* Claude
  via the `workerKind` axis, and can itself use Claude/Fable plan-mode for
  decomposition — Artanis is the top-level orchestrator that the plan-then-
  fan-out crossover reports to.
- **Khala**: Artanis is the AI that keeps the shared contributed-compute pool
  utilized — the "make devices fully utilized" mandate is literally the
  shared-fleet admin job, and the 10×-tokens mission is its success metric.

## 8. Invariants To Preserve

- Authority scope (`owner_self` | `shared_fleet` | `owner_operator`) gates
  every action; a scope reaches only capacity linked to it — never widen a
  per-user Artanis to pooled/marketplace/settlement capacity.
- Risky-action kinds require an *effective* operator-approved gate with an
  authority receipt; non-authority sources inform but never approve.
- Money movement stays owner-enveloped: owner sets caps, Artanis decides
  within; over-cap blocks and waits; fractional payouts don't discharge the
  obligation.
- Grounding is enforced: no un-grounded command/path/endpoint asserted as
  real (signature 6); extend to signatures 1–5.
- Exact-only accounting and public-safe projections; org-operated runs are
  not independent-contributor proof.
- Identity discipline: one Artanis entity, run announcements are Artanis's,
  no roleplay-Artanis substituting for the grounded operator.

## 9. Bottom Line

Artanis has already crossed the hardest line — from a narrator that could see
but not act into a bounded operator that reads the repo, reasons in a tool
loop, dispatches no-spend Codex work across linked accounts, and posts to the
forum, with all money owner-gated. The gap to the owner's real target —
Artanis as **administrator of the shared Khala fleet**, and optionally as a
**per-user fleet manager (AaaS)** — is not more capability bolted on; it is
three structural moves: make **authority scope first-class** (so one codebase
safely serves the pooled fleet and N isolated per-user fleets), **finish the
Blueprint-signature governance** (so the June-28 class of mistakes becomes
impossible and higher autonomy is safe to grant), and **wire Artanis onto the
one orchestration/status spine** the desktop cockpit and mobile companion
also read (so he administers real shared state, not bespoke snapshots).
Everything else the owner has asked for — the 10×-tokens mission, the "make
decisions so I don't have to" inversion, the "Fleet Commander" AaaS demo — is
downstream of those three, and each of them dovetails with the fleet fan-out,
Orca, and Claude-parity plans rather than competing with them.
