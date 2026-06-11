# Autopilot Unified Audit and Roadmap — The Live /autopilot Product, the Six Problems, and the Pylon-Anchored Labor Market

Date: 2026-06-11

Scope: full audit of the live `/autopilot` surface on openagents.com
(frontend, worker API, record layer, execution path, billing, provider
accounts, operator surfaces) measured against the six operations
problems identified for the Autopilot wedge
(`docs/tassadar/2026-06-11-coding-agent-primitive-wedge.md` §II), then
unified with the agent-facing work-order spine
(`docs/autopilot-coder/` audits) and the labor market
(`docs/labor/2026-06-10-open-agent-labor-market-roadmap.md`,
`docs/tassadar/2026-06-11-autopilot-agentic-labor-market.md`), anchored
by Pylons. This document is both audit and roadmap; built claims cite
code or receipts, everything else is plan. No registry promise is
created or modified here.

## Executive finding

**OpenAgents has two Autopilot stacks, and neither knows the other
exists.**

- **Stack A — the live `/autopilot` product** (`apps/web` +
  `workers/api` omni/coding-autopilot surfaces): a logged-in chat
  workroom with durable goals, team rooms, file uploads, Stripe-backed
  credits and per-minute metering, token accounting and leaderboards,
  multi-account ChatGPT/Codex provider leasing, **SHC container
  execution** (fully background, callback-driven), GitHub writeback
  with authority receipts, and a complete typed record layer (missions,
  decisions, artifacts, repo placement tiers, repo memory) built on the
  Blueprint kernel. Internally named Adjutant; renamed at the display
  boundary.
- **Stack B — the Autopilot Coder work-order spine + labor market**
  (`autopilot-work-routes`, the Pylon worker loop, the Claude Agent
  bridge, NIP-LBR): typed agent-facing work orders with L402 payment,
  production Pylon placement, durable leases, review gates — proven
  live (#4633) — plus a fully plumbed (and empty) Nostr labor market
  with escrow and validator-gated acceptance.

Stack A has what Stack B lacks: a real product UI, background hosted
execution, billing, provider-account routing, and PR writeback. Stack B
has what Stack A lacks: a typed open intake any agent can call, Pylon
placement onto contributor machines, Lightning settlement, and the
labor market. **The six wedge problems are solved by operationalizing
Stack A; the marketplace is bootstrapped by unifying it with Stack B.
The normalized assignment payload
(`openagents.autopilot_coding_assignment.v1`) was explicitly designed
to be shared across both stacks' lanes and is the seam they unify on.**

One gating fact before anything else, stated with its intent: **the
live `/autopilot` product is core-team-gated, and that is policy, not
a gap.** `loggedInWorkroomAllowed` requires `authHasCoreTeamAccess`
plus completed onboarding (`apps/web/src/main.ts`). The core team is
the dogfood cohort; the gate opens beyond it only when the six
problems are actually tested and ready — proofs first, door second.
The positioning that opening carries is already written:

> **The best agents are built from all of us.**
> **Built for all of us. Built from all of us.**

The dogfood phase is that line made operational: the team building
the agent is its first workload, and the receipts from that use are
the readiness evidence the public opening waits on.

A second design decision is also policy and shapes everything in
Part 3 and Part 4: **buyers pay in dollars or bitcoin; work is paid
out in bitcoin.** Customers connect a credit card through Stripe in
the web UI and buy USD credits, or pay in bitcoin over the L402/MDK
rail the work-order spine already carries; they order software work
through the web UI, their connected Pylon, or the API; and when that
work is performed by contributors or market providers, settlement to
them is in sats. The platform sits at the fiat-in/bitcoin-out seam,
which is both the mainstream-buyer on-ramp (no human customer needs
to know what a satoshi is) and the contributor promise (every
provider is paid in money nobody can print) — while the bitcoin
buy-side keeps the door open for the buyers who have no card at all:
agents.

A third decision completes the shape: **the MVP is equally usable by
humans and agents.** Every MVP capability is available via the API,
extending the agent-facing API and registered-agent identity the
platform already operates — so a coding request can be kicked off by
a human in the web UI, by an agent calling the API, by an agent
communicating on the Forum, or by an autonomous administrator
(Artanis-class) spawning coding threads on its own tick — assuming
the requesting agent has an account set up with payment, in either
currency. This is not an extension of the product; it is the
product's founding asymmetry corrected: Stack B's intake was
agent-first from birth (#4633's live proof *was* an agent), and the
MVP simply refuses to ship a human face that demotes the agents.

## Part 1 — Inventory: what is actually live at /autopilot

### 1.1 Frontend surface (`apps/openagents.com/apps/web`)

| Route | What it is | Status |
| --- | --- | --- |
| `/autopilot` | Personal chat workroom (ChatRoute): timeline, composer, file uploads (PDF/text/JSON/CSV/XLSX/DOCX/images), side panel with goals and run diagnostics | Live, core-team-gated |
| `/teams/:teamRef/chat` | Team room: member messages, `@autopilot` intents rendered with mission-briefing cards | Live |
| `/teams/:teamRef/projects/:projectRef/chat` | Project workrooms | **Gated off** (`projectWorkrooms: false` in `product-policy.ts`; one hardcoded exception) |
| `/t/:threadId` | Thread view | Live |
| `/settings`, `/billing`, `/usage` | Settings; Stripe credits/coupons/ledger; token usage and leaderboards | Live |
| Artanis operator console | Tick/loop/health state, approval gates, work routing, publication queue | Live, **admin-only** |

Goals are the durable work container the user steers: objective,
pause/resume/clear, visibility (private/team/public), token budget and
usage (`goals/` views; `/api/autopilot/goals*` routes).

### 1.2 Execution path (the part the user never sees)

A mission runs like this (`programmatic-autopilot-work-runbook-audit.md`,
operator runbook):

1. **Preflight** (`/api/omni/operator/autopilot/preflight`): D1
   migrations, team/project records, provider-account health, SHC
   control health, callback config, **GitHub writeback readiness**.
2. **Dispatch** to an **SHC container** — a remote, disposable,
   serverless coding runtime — carrying repo context, a leased
   provider-account ref, and a callback URL+token.
3. **Execution** streams back as callbacks
   (`/api/omni/agent-runs/:runId/events/ingest`, six accepted dialects,
   credential-shaped material dropped before persist); Cloudflare D1 is
   the source of truth, never SHC process inspection.
4. **Continuation** (`/api/omni/operator/agent-runs/:runId/continue`):
   queue a follow-up turn if running, or request policy-gated goal
   continuation if stopped.

The model/provider is the **user's own connected ChatGPT/Codex
account** (device-login flow,
`/api/provider-accounts/chatgpt-codex/device-login/*`), leased per run.

### 1.3 The provider-account lease layer (this is further along than any audit recorded)

`workers/api/src/provider-account-lease-policy.ts` implements real
multi-account smart routing: candidates are filtered to
connected+healthy+secret-bearing accounts under their lease limit, not
low-credit, not in cooldown; selection load-balances by active lease
count, then operator priority, then least-recent successful use, with
sanity-check and parallel-probe timestamps and typed
`recentFailureClass`/`cooldownUntil` state. The continuation-decision
layer models `evidence.account_rate_limit` and
`risk.account_rotation_needed` with a `retry account` action, and
mission records carry account-lease refs. **The transcript complaint
"I juggle multiple accounts and track reset times in a note" has a
shipped engine answering it.** What it lacks is a face (§2.1).

### 1.4 The record layer (Blueprint kernel, production)

All in `workers/api/src/coding-autopilot-*.ts` with tests:

- **Missions** (`CodingAutopilotMissionRecord`): identity, status,
  objective stack, workroom/assignment/route-scorecard/account-lease/
  budget/artifact refs, `programRunRef` into the Blueprint kernel.
- **Decision actions**: continue, steer, provide context, rerun tests,
  retry account, stop, approve PR draft, request customer input,
  create follow-up mission — all `directEffectPermitted: false`
  (decisions are evidence; effects go through submission/approval).
- **Artifacts**: diff/patch/test/build/preview/pr_draft/pr_url/
  rollback/screenshot/redaction/receipt kinds with visibility tiers;
  PR artifacts **require authority receipt refs**.
- **Repo placement**: trust tiers `public → private → sensitive →
  infra → legal_sensitive → payment_sensitive → regulated`, with
  SHC-only placement for the regulated tiers and typed
  needs-grant/needs-approval/blocked outcomes.
- **Repo memory**: typed evidence kinds (accepted_fix, build_command,
  flaky_test, denied_path, pr_style, reviewer_preference …) with
  `keywordRoutingAllowed: false` — semantic/typed retrieval only.
- **Situational awareness**: the under-two-minutes state-understanding
  metric wrapped from the Blueprint briefing layer.

### 1.5 Money and measurement

- **Billing** (`/billing`, D1 ledger + Stripe + coupons): metered SHC
  container time and token usage, minimum-balance launch gate (HTTP
  402), per-minute sweeper, out-of-credits suspend/cancel/notify.
- **Token accounting** (`/usage`, dual ledgers
  `autopilot_token_usage` + canonical `token_usage_events`):
  OpenCode-style buckets including cache reads/writes, idempotent
  ingestion across seven payload dialects, leaderboards with privacy
  opt-outs, admin aggregates.

### 1.6 What Stack A does **not** have

No Pylon dispatch (the fleet route exists; mission execution is
SHC-only). No Lightning settlement (Stripe credits only). No open
agent-facing intake (the chat is session-authed; Stack B's
`POST /api/autopilot/work` is the open door). No mission list/detail
UI (records exist; no browser). No user-facing decision queue (records
exist; approvals live in the admin-only Artanis console). No mobile
surface, no push notifications (one out-of-credits email). No
multi-provider peers (ChatGPT/Codex only — no Anthropic/Gemini lanes
in the product path). And no public signup.

## Part 2 — The six problems vs. the live system

The wedge essay's six complaints, audited one at a time. Verdict
shorthand: **engine** = the hard part exists in code; **face** = the
user-visible product part; **proof** = a smoke/receipt that the claim
is real.

### 2.1 Problem 1 — The limit wall and the account shuffle

- **There:** the lease policy (§1.3) — load-balancing, cooldowns,
  low-credit and health gating, failure-class tracking; rate-limit
  evidence and account-rotation risk modeled in continuation
  decisions; account-lease refs on missions. Engine: **yes**.
- **Missing:** the **account-pool dashboard** — connected accounts,
  current lease load, cooldown/reset timers, low-credit flags (the
  exact "note where I track all the limits," productized); the
  reconnect-cadence UX (device login expires; reconnect prompts exist
  but no proactive nudge); **cross-account continuation proof** — a
  smoke where a run hits a real rate limit and the system rotates
  accounts *with context intact* (the records model it; nothing proves
  it); and the **non-Codex flow** — Anthropic/Gemini provider peers so
  the wedge isn't single-vendor (the Pylon side already has the
  Claude Agent bridge pattern to copy).
- **Verdict:** engine yes, face no, proof no.

### 2.2 Problem 2 — The tethered laptop (background execution)

- **There:** this is Stack A's strongest answer. SHC runs are fully
  background by construction — serverless containers, callback-driven,
  laptop-independent; continuation can queue follow-up turns or
  goal-level continuations; goals pause/resume; billing enforces caps
  unattended. Engine: **yes, shipped**.
- **Missing:** **queue-the-night-before UX** — scheduled launches,
  recipes, and an auto-continuation policy so stopped runs resume
  without an operator (`continue` is an operator API today); SHC
  capacity/scale posture is unaudited (one runtime class, no surge
  story — which is precisely where the labor market enters, §3);
  and honest copy boundaries until a public smoke exists.
- **Verdict:** engine yes, face partial (runs do happen in the
  background today), policy missing (unattended continuation).

### 2.3 Problem 3 — Mobile control

- **There:** the data layer is genuinely ready: decision-action
  records are exactly an approval queue's rows (approve PR draft,
  request input, stop, retry — all effect-free pointers to gated
  submissions); sync scopes (`workspace/team/thread/agent-run`)
  already push live patches; mission briefing answers "what's
  happening" in under two minutes by design.
- **Missing:** everything user-facing. No decision-queue UI for
  non-admins (approvals render only in the admin Artanis console); no
  notification channel beyond the out-of-credits email; no
  mobile-responsive approval surface; no push. The `control` iOS app
  is owner-only and points at a different control plane — it is the
  prototype, not the product.
- **Verdict:** engine yes, face entirely missing. Cheapest of the six
  to make real: a mobile-responsive `/decisions` page over existing
  records + email/push on `decision_required` events.

### 2.4 Problem 4 — Context across boundaries

- **There:** durable missions/goals/threads; repo memory with typed
  kinds and semantic-only retrieval; continuation decisions; artifacts
  with provenance; account-lease indirection so a mission is not
  identity-bound to one account.
- **Missing:** the proofs. Cross-account continuation (2.1's smoke);
  cross-runner continuation (an SHC run resumed on a Pylon, or vice
  versa — the normalized assignment payload makes this *expressible*;
  nothing executes it); session resume across days as a product
  behavior rather than an operator action.
- **Verdict:** substrate strong, continuity unproven across exactly
  the boundaries the complaint names (limits, agents, runners).

### 2.5 Problem 5 — Team budgets and visibility

- **There:** the most complete of the six. Real billing with metering
  and enforcement; dual-ledger token accounting with leaderboards and
  opt-outs; `/usage` for users, `/stats` aggregates for admins; team
  rooms with shared files and `@autopilot` missions; goal-level token
  budgets.
- **Missing:** **team-level budgets and cost attribution** (tokens and
  credits are per-user/global; nothing answers "what did *this team*
  spend on *this mission*"); **pooled provider accounts** scoped to a
  team (the complaint is "share subscription capacity across the
  team"; leases exist but team-pool semantics and fairness policy
  don't); per-mission spend caps a customer can set; and the
  visibility drill-down that joins ledger entries ↔ mission ↔
  artifacts ("what did I get for this spend" — all three record types
  exist; the join UI doesn't).
- **Verdict:** engine mostly yes, team semantics and the
  spend-to-evidence join missing.

### 2.6 Problem 6 — Isolation, boundaries, permission fatigue

- **There:** structurally the deepest answer in the system. SHC
  containers are the transcript's disposable VM, shipped; repo
  placement trust tiers already encode the regulated-vertical ask
  (legal_sensitive/payment_sensitive/regulated → SHC-only) — the
  law-firm matter-separation scenario is *modeled in production code
  today*; typed access requirements in Stack B replace interactive
  permission interrogation with granted-up-front contract; redaction
  law everywhere; provider secrets never in projections.
- **Missing:** the self-serve faces: repo connection UX (GitHub
  writeback works through provider grants, but connecting a repo and
  scoping what the agent may see is operator-shaped); a per-mission
  **data-scope declaration** ("hand it exactly this") surfaced as
  product UI over the existing placement policy; a placement
  explanation surface ("this ran in an isolated container because the
  repo is tier X" — the policy computes typed reasons; show them);
  and a sovereign-placement choice (my-Pylon-only as a user-settable
  preference — which is §3).
- **Verdict:** engine yes (unusually so), face missing.

### 2.7 The pattern

Six problems, one diagnosis repeated: **the engines exist; the faces
and the proofs don't.** The live system was built operator-first
(admin consoles, operator runbooks, preflight scripts) — correctly,
for the dogfooding phase — and the wedge work is overwhelmingly
*productization of already-shipped machinery*, not new architecture.
The two genuine engine gaps across all six: scheduled/auto-continued
runs (2.2) and team budget semantics (2.5). Everything else is UI,
notification plumbing, smokes, and copy.

## Part 3 — The Pylon anchor: unifying the stacks and reaching the labor market

### 3.1 Three lanes, one payload

The normalized coding assignment
(`openagents.autopilot_coding_assignment.v1`, OA-AUTO-021) was
designed for exactly this moment: one payload shared by requester
Pylon, SHC, cloud-sandbox, and hosted lanes. Today each stack uses its
own subset. The unification target is **one placement policy over
three execution lanes**:

| Lane | Runner | Inference paid by | Buyer pays | Provider receives | Status |
| --- | --- | --- | --- | --- | --- |
| **A — Hosted SHC** | platform container | user's leased account (or platform-metered) | USD credits (Stripe) | n/a (platform infra) | live (Stack A's only lane) |
| **B — Owner's Pylon** | the customer's own machine | the customer's own BYOK key/subscription | nothing (their device, their key) | n/a (self-serve) | built in Stack B (Claude Agent bridge, capability declaration, worker loop); never reachable from the product |
| **C — Labor market** | someone else's idle agent | the provider's own account | USD credits (Stripe) | **sats** (escrow → ladder settlement) | plumbed end-to-end, zero inventory |

The money model across the table is the policy stated in the
executive finding: **dollars in, bitcoin out.** The buyer side is one
currency (USD credits bought by card through Stripe — checkout,
ledger, metering, and out-of-credits enforcement already live, §1.5);
the provider side is one currency (sats over the proven ladder). On
Lane C the platform bridges the two: a buyer's USD credit debit funds
the sats escrow that settles to the provider on acceptance. The
buyer never needs a wallet; the provider never needs a card.

Two placement policies sit on top of the table, both owner direction
and both MVP scope:

- **Own Pylon first, and free.** When the requesting user has a
  connected, online, capability-true Pylon, placement prioritizes it
  for their own work — and a job picked up by its owner's own Pylon
  costs nothing (their machine, their key, their job; the platform
  meters no credits). The work-order spine already encodes most of
  this: the placement selector prefers the requester's own Pylon on
  owner linkage (OA-AUTO-019), and the no-spend worker loop —
  live-proven in production (#4633) — *is* the free lane; what
  remains is declaring it as product pricing policy and wiring the
  web UI to it. SHC is the metered fallback when the user's Pylon is
  offline, busy, or absent — the "always home" guarantee, paid in
  credits.
- **Lane B is location-independent.** A Pylon deployed and
  authenticated in a cloud environment (a VPS the user controls) is
  still Lane B when it serves its owner: same credentials, same
  worker loop, different host — and it gives the owner free
  background execution that survives the closed laptop without
  touching SHC. The same deployment becomes a Lane C provider the
  day its owner flips GO ONLINE and lets unused capacity pick up
  *other people's* jobs for sats — same machine, opposite side of
  the order book, post-MVP because paid Lane C waits on the
  settlement bridge.

**Repo placement trust tiers are the lane selector**, and the policy
already exists: regulated tiers → Lane A only (shipped behavior);
private tiers → Lane A or owner-verified Lane B; public tiers → any
lane, including C. The placement engine Stack A built for *where in
our infrastructure* generalizes verbatim to *whose infrastructure*.

What each lane adds to the six problems: Lane B answers the limit
wall a second way (your own subscription quota on your own machine,
no platform metering) and answers sovereignty (complaint 6) at its
strongest; Lane C answers "ASAP even when everything of mine is dark
or rate-limited" (complaints 1 and 2) with burst capacity priced in
sats — the currently-red
`autopilot.control_center_fanout_marketplace.v1` made concrete.

### 3.2 What unification actually requires (the honest list)

1. **Mission ↔ work order bridge.** Stack A missions and Stack B work
   orders are sibling records describing the same thing. Either
   missions dispatch *through* the work-order spine (preferred: the
   spine already owns placement, leases, review, and L402), or a
   bridge maps them 1:1. Today a `@autopilot` team message launches an
   SHC run directly; post-unification it creates a work order whose
   placement may choose SHC.
2. **Lane B in product placement.** The placement selector already
   reads Pylon registrations (OA-AUTO-019); the Claude Agent executor
   already runs real bounded tasks. Missing: surfacing "connect your
   Pylon" in `/settings`, the owner-linkage check in product
   placement, and the live-leg smoke (#4661 / the bridge runbook —
   one credentialed run clears two promises).
3. **Writeback symmetry.** Stack A has GitHub PR writeback with
   authority receipts; Stack B delivers refs only. Unification routes
   Stack B deliveries through Stack A's artifact/authority layer so a
   labor-market job can end in a PR draft behind the same receipts.
   This closes the full-flow audit's leg 9 with code that exists.
4. **Settlement symmetry — the dollars-to-sats bridge.** Lane A
   meters USD credits; Lane C escrows and settles sats. Unification
   means a buyer's credit debit funds a provider's sats escrow: the
   accepted-work → payout-eligibility bridge (P0.6 of the gap audit,
   still unbuilt) plus a USD→sats conversion seam with its own
   receipts (rate ref, conversion ref, both ledger entries linked).
   This is the one money seam that must land before Lane C jobs pay
   providers from product demand — and it is also where the
   platform's margin and the contributor's bitcoin promise live, so
   its receipts must be the cleanest in the system.
5. **Capability envelopes (#4750).** Lane B/C honesty: a Pylon quotes
   only work classes it is capability-true for, declared with
   self-test receipts — the W4.1 pattern doing product duty.

### 3.3 The flywheel, restated operationally

Wedge customers (Stack A) generate paid demand → placement overflows
to Lane C when owned capacity is dark/limited → idle agents (the
Orrery class, contributor Pylons with GO ONLINE) earn sats →
provider-side earnings make "connect your Pylon" worth a settings
toggle for the same wedge customers → the backlog faucet
(issue→work-request adapter) keeps standing inventory in the market
between demand spikes → every settled job emits the public receipts
that are simultaneously the labor promises' evidence and the
product's marketing.

## Part 4 — The unified roadmap

Phased; each phase names its candidate issues (none filed by this
document) and the promises its receipts feed.

### Phase 0 — Operationalize /autopilot against the six problems (the wedge launch)

The productization pass over shipped engines. **The cohort for all of
it is the core team — this phase is dogfooding by design.** The
public door (last item) opens only when the phase's proofs exist; the
team is the test fleet, and "Built from all of us" starts with us.

1. **Buy-side completeness**: credit-card connect through Stripe in
   the web UI as a first-class onboarding step (checkout/ledger/
   metering already live, §1.5; add card-on-file and optional
   auto-top-up so an overnight run never dies broke), and **ordering
   through the Pylon**: a customer who connects their account to
   their Pylon can order software work from the terminal
   (`pylon work submit` spending the same USD credit balance the web
   UI spends). One credit ledger, two front doors.
2. **Account-pool dashboard** (problem 1's face): connected accounts,
   lease load, cooldowns/resets, low-credit, reconnect nudges — over
   the lease policy as-is.
3. **Scheduled + auto-continued runs** (problem 2's policy): queue
   missions for later launch; a user-settable continuation policy so
   stopped overnight runs resume under budget gates; surfaced run
   state on `/autopilot` ("what ran while you slept").
4. **Decision queue + notifications + mobile-responsive approvals**
   (problem 3): a `/decisions` page over decision-action records;
   email (later push) on decision-required and delivered events;
   responsive before native — the `control` app's patterns inform,
   not block.
5. **Team budgets + spend-to-evidence join** (problem 5): team-scoped
   budget rows, per-mission caps, pooled team account-leases with a
   fairness policy, and the ledger↔mission↔artifact drill-down.
6. **Repo + data-scope UX** (problem 6's face): self-serve repo
   connect, per-mission scope declaration, and the placement
   explanation surface reading the policy's typed reasons.
7. **Proof smokes** for the two continuity claims: rate-limit
   rotation with context intact (1/4) and an overnight unattended
   queue→run→deliver→notify pass (2/3).
8. **Provider peers**: Anthropic/Gemini connect flows beside
   ChatGPT/Codex (the "non-Codex flow" promised at launch), with the
   ToS-compliance review as the first deliverable per the wedge
   essay's router law.
9. **Open the door — gated on the above.** Public signup replaces the
   core-team gate only when the phase's proof smokes pass and the
   dogfood cohort's receipts say the six problems are actually
   solved. Onboarding at opening: connect a card, connect
   ChatGPT/Codex (and GitHub when wanted), launch-grant credits,
   first mission — under the launch positioning: *the best agents are
   built from all of us; built for all of us, built from all of us.*

Promises fed: `autopilot.free_coding_task_beta.v1`,
`autopilot.issue_to_pr_loop.v1` (with Phase 1.3),
`payments.accepted_outcome_economics.v1` (anchor outcomes),
`autopilot.agentic_labor_products.v1` (self-serve flows).

### Phase 1 — Unify the stacks (one primitive, every front door)

1. **Mission↔work-order unification** (§3.2.1) — the spine becomes the
   product's dispatch path; the chat is one front door among four.
2. **Lane B deepening**: own-Pylon placement, the free self-serve
   lane, and the cloud-Pylon deployment path are MVP scope per the
   owner clarification (ladder rungs 3, 4, 6); Phase 1 deepens the
   lane rather than introducing it — cross-runner continuation,
   richer work classes, and the record-level unification that lets
   the same order move between SHC and Pylon mid-mission.
3. **Writeback symmetry** (§3.2.3): work-order deliveries through the
   artifact/authority layer to PR drafts — the "issue to PR" sentence
   becomes claimable end to end.
4. **Cross-runner continuation**: resume a mission across SHC ↔ Pylon
   on the shared payload; the context complaint's strongest answer.
5. **Settlement bridge** (§3.2.4): accepted work → payout
   eligibility → ladder settlement, generalizing the Tassadar-proven
   loop to coding work.

### Phase 2 — The labor market lane (Pylon-anchored burst and the open market)

1. **First live negotiated job** (#4648, runbook written) — pointed at
   a **real backlog issue**, per the labor-market essay.
2. **The backlog faucet**: the issue→work-request adapter over the
   live (empty) `/api/forum/work-requests` surface; our own issues as
   standing inventory.
3. **Lane C in product placement**: fan-out/burst when owned capacity
   is dark or rate-limited, behind explicit customer opt-in and
   public-tier-only placement at first — flipping
   `autopilot.control_center_fanout_marketplace.v1` from red on
   evidence.
4. **Onboarding ramp + capability envelopes**: rung-0 verification
   bounties as the standing newcomer inventory; #4750-pattern
   envelopes gating quotes.
5. **Settlement visibility law**: every payout rung publicly
   dereferenceable (the #4753 class) as a labor-lane acceptance
   criterion before any live claim.

Promises fed: the three labor yellows
(`labor.forum_work_requests.v1`, `labor.nostr_negotiation_market.v1`,
`artanis.labor_requester.v1`), `provider.compliant_usage_labor.v1`
(red, `labor_stream_not_live`), the five-streams labor lane, and the
fanout red.

### Sequencing note

Phases overlap deliberately: 0.1–0.4 (ordering, dashboard,
scheduling, decisions) are the commercial wedge and block nothing;
1.2 (Lane B live leg) is a one-run receipt available *today* on a
credentialed machine; 2.1–2.2 (first job + faucet) need no product
work at all and should not wait for Phase 1. The only hard orderings
are writeback-before-issue-to-PR-claims and
settlement-bridge-before-paid-Lane-C.

### The issue ladder to MVP

**MVP definition (owner-clarified):** a core-team member (then a
design-partner team) can make a request **in the web UI**; if they
have a connected Pylon, placement **prioritizes their own Pylon**,
and a job their own Pylon picks up is **free**; if they don't (or it
is offline/busy), the job runs on SHC, metered in USD credits bought
by card. Status updates and work information are visible **on both
surfaces** — at the Pylon and in the web UI when logged in — for the
same order, regardless of which door submitted it. A Pylon deployed
and authenticated in a cloud environment picks up its owner's jobs
exactly like a local one (and is positioned, post-MVP, to pick up
others' jobs from unused capacity).

**And the MVP is human/agent symmetric:** everything above is equally
available via the API under the existing registered-agent identity —
an agent with a payment-enabled account (USD credits funded by its
owner's card, or bitcoin over L402/MDK) can submit requests, poll or
stream status, and exercise review, with no capability reserved for
the browser; a Forum-communicating agent can kick off a coding
request from the Forum surface; and an autonomic (Artanis-class)
administrator can spawn coding threads on its tick under its budget
gates. MVP exit requires the two continuity proofs (rate-limit
rotation, overnight unattended run) passed as smokes, plus the
agent-parity proof (one paid agent-initiated order end to end), not
asserted as copy. The public door opens after MVP, not as part of
it. Paid Lane C — serving *other people's* jobs for sats — is
post-MVP (it waits on the settlement bridge), with two cheap
exceptions noted below that run early because they are single-run
receipts.

The load-bearing audit fact under this definition: **most of the MVP
is wiring, not building.** The work-order spine already prefers the
requester's own Pylon on owner linkage (OA-AUTO-019), the no-spend
worker loop *is* the free own-Pylon lane (live-proven, #4633), the
Claude Agent executor is the runner, and web-side sync scopes already
stream run state. What the MVP adds is the web UI → work-order
bridge, the pricing policy declaration, the dual-surface status
faces, and the proofs.

**The ordering principle (owner direction):** get the base system —
the API part — working as soon as possible so dogfooding starts at
the earliest opportunity, with this exact scenario as the target: *an
agent takes the outstanding issue list as its marching orders, files
them through the API, our Pylon picks them up with the Claude Agent
SDK and fulfills them, and everything is appropriately visible in the
web UI.* The decisive audit fact: this loop needs **no payment work
at all** — the work-order API is live, placement already prefers the
owner's Pylon, the no-spend worker loop is live-proven, and the
Claude Agent executor is merged. So the bootstrap block runs first,
in days; pricing, payments, scheduling, and the polished faces follow
*behind* a working loop instead of in front of it, hardened by the
dogfood traffic the loop generates.

Rung IDs are stable (B = bootstrap, M = MVP ladder, A = agent
parity, P = post-MVP) so cross-references survive reordering.
Sizes: S ≈ a day, M ≈ days, L ≈ a week-plus.

**All rungs are now filed (2026-06-11), tracked by epic #4786:**
B1 #4755 · B2 #4756 · B3 #4757 · B4 #4758 · M1 #4759 · M2 #4760 ·
M3 #4761 · M4 #4762 · M5 #4763 · M6 #4764 · M7 #4765 · M8 #4766 ·
M9 #4767 · M10 #4768 · M11 #4769 · M12 #4770 · M13 #4771 ·
M14 #4772 · A1 #4773 · A2 #4774 · A3 #4775 · A4 #4776 · P1 #4777 ·
P2 #4778 · P3 #4779 · P4 #4780 · P5 #4781 · P6 #4782 · P7 #4783 ·
P8 #4784 · P9 #4785. The `[filed]`/`[new]` markers in the tables
below record state at authoring time; the issue numbers above are
current.

#### The bootstrap block — dogfood loop first (days, no payment dependencies)

| # | Issue | Status / size | What it delivers | Depends on |
| --- | --- | --- | --- | --- |
| B1 | **Claude Agent bridge live-leg run** — execute the three-command operator runbook on a credentialed machine | [filed: epic #4717 follow-up; satisfies #4661's acceptance via the claude-agent adapter] / S | Proves the loop's executor on a real machine; flips `pylon.local_claude_agent_bridge.v1` green-proposable and clears `autopilot.codex_probe_pylon_successor.v1`'s last blocker. One run, two promises. | nothing |
| B2 | **Claude-agent work class through the Autopilot API, end to end** — API-submitted orders carry the `claude_agent_task` work class + capability ref through placement → lease → own-Pylon pickup → Claude Agent executor → delivered → review, no-spend; includes the `git_checkout` workspace kind (public repos, pinned commit, caller-supplied verification command) so orders are real repo work, not fixtures | [mostly built: the SDK audit records "no Worker-side schema change is required to pilot this lane in no-spend mode"; the dispatch shape exists operator-side; the repo work class is the leverage audit's named move 2] / M | The base system working: the live API drives the live executor on real repos. Everything else in the MVP is a face or a proof on top of this. | B1 |
| B3 | **The marching-orders agent** — a registered agent reads the outstanding GitHub issue list, decorates each issue with a bounded objective and verification command, and files them as work orders through the live API under its own identity; review/acceptance stays human | [new; the in-house precursor of both A4 (autonomics) and P5 (the labor-market faucet)] / M | The owner's dogfood scenario made literal: the backlog becomes the order book, our Pylon becomes the worker, and every closed issue is a receipt the system earned about itself. Also the first standing exerciser of the agent API (A1's living test). | B2 |
| B4 | **Work list + detail visibility in the web UI** — browse orders, status, artifacts, delivered refs; live status via the existing sync scopes | [new; records and routes exist — projection UI only] / M | "Seeing everything appropriately in the web UI": the dogfood cohort watches B3's orders flow without touching a terminal or D1. The visibility spine all later faces (decisions, budgets) hang off. | B2 |

**Bootstrap exit:** B1–B4 standing means the dogfood process is live —
issues in, verified work out, watched from the browser — before a
single payment, pricing, or scheduling feature exists. Run it daily
from then on; every subsequent rung ships into live traffic.

#### The MVP ladder (behind the loop, ordered by effect)

| # | Issue | Status / size | What it delivers | Depends on |
| --- | --- | --- | --- | --- |
| M1 | **Web UI request composer → work-order spine** — a logged-in human's web request creates the same work orders B3 files, replacing direct-to-SHC dispatch as the product path | [new] / M–L | The human front door onto the proven loop. Minimal unification now; full record unification stays post-MVP (P2). | B2 |
| M2 | **Dual-surface status + `pylon work submit\|status\|review`** — the same order visible and steerable from the terminal and the web UI, regardless of entry door | [new; the command family has been named unowned since the full-flow audit] / M | One order, two windows — the owner-clarified visibility requirement, completing what B4 starts. | B4 |
| M3 | **Own-Pylon-first placement + the free self-serve lane, declared** — requester's connected online capable Pylon wins; own-Pylon pickup of own job charges zero credits; SHC is the metered fallback | [mostly built: OA-AUTO-019 owner-linkage preference + the live no-spend loop; productize as declared pricing policy] / S–M | The owner-clarified pricing and priority rules, formalizing what the bootstrap loop already does in practice. | B2 |
| M4 | **Cloud-Pylon deployment path** — runbook + auth flow for running your Pylon on a VPS you control; it picks up your jobs headlessly | [new; prior art: the SHC-box deployment runbook + existing registration/heartbeat/capability flow] / M | Lane B without the laptop; lets the marching-orders loop run around the clock on a box that never sleeps. Stages the post-MVP provider flip (P6). | B1, M3 |
| M5 | **Card-on-file + auto-top-up** — saved payment method and user-set top-up policy on the existing Stripe/D1 ledger | [new] / S–M | The SHC fallback never dies broke overnight; the buy-side becomes set-and-forget. | nothing |
| M6 | **Scheduled launches + auto-continuation policy** — queue work for later; user-settable continuation under budget gates; "what ran while you slept" | [new] / M–L | Problem 2's missing policy half (one of two genuine engine gaps). Converts the operator-only `continue` API into product behavior, on both lanes. | M3, B4, M5 |
| M7 | **Decision queue + notifications** — mobile-responsive `/decisions` page over decision-action records; email on decision-required and delivered events | [new] / M | Problem 3 end-to-end at the responsive-web tier; also how the human reviews B3's deliveries from a phone. | B4 |
| M8 | **Account-pool dashboard** — connected accounts, lease load, cooldown/reset timers, low-credit flags, reconnect nudges | [new] / M | Problem 1's face over the shipped lease policy. | nothing |
| M9 | **Proof smoke: rate-limit rotation with context intact** — induced limit mid-run → lease rotation → completion | [new] / M | Problems 1+4's proof. Gates MVP exit. | M8 useful, not required |
| M10 | **Proof smoke: overnight unattended run, both lanes** — queue at night → scheduled launch → background completion (once on SHC, once on a cloud or local own-Pylon) → notification → morning review on both surfaces | [new] / S–M (once M2, M4, M6, M7 land) | Problems 2+3's proof and the dual-surface proof in one smoke. Gates MVP exit. | M2, M4, M6, M7 |
| M11 | **Repo connect + data-scope UX** — self-serve repo connection, per-mission scope declaration, placement explanation showing trust-tier and lane reasons | [new] / M–L | Problem 6's face over the shipped placement policy. | M3 |
| M12 | **Team budgets + spend-to-evidence join** — team-scoped budgets, per-mission caps, pooled team account-leases, ledger↔mission↔artifact drill-down | [new] / L | Problem 5's missing semantics (the other engine gap); what design-partner teams evaluate with. | B4 |
| M13 | **Provider peers: Anthropic/Gemini connect** — ToS-compliance review first, then connect flows | [new] / M–L | De-risks single-vendor dependence; the ToS review itself should not wait. | nothing |
| M14 | **MVP exit review / door-open gate** — checklist binding the public-signup flip to the B/M/A receipts and the copy/redaction law; ships the launch positioning | [new] / S | The decision record that "tested and ready" actually happened. | B1–B4, M1–M12, A1–A4 |

**Agent-parity rungs (MVP scope, owner-clarified).** These run
alongside the M-ladder, not after it — and the bootstrap block gives
them a flying start: B3's marching-orders agent *is* A1's living
test and A4's in-house precursor, exercising the agent API daily
from week one. The audit gives the rest a head start too: the
work-order spine's intake, status, events, and review routes are
*already* agent-facing under registered-agent auth (#4633's live
proof was an agent), the L402 challenge/retry contract is verified
in CI, the Forum work-requests surface is live, and the Artanis
tick-action pattern (schema-validated proposals, budget gates) is
the template for A4.

| # | Issue | Status / size | What it delivers | Depends on |
| --- | --- | --- | --- | --- |
| A1 | **API parity contract** — every MVP capability (submit, status/events, decisions/review, scheduling, lane/pricing visibility) exposed via the existing agent API and registered-agent identity, audited as a parity matrix: no browser-only capability | [partially built: submit/status/events/review live; B3 exercises them daily; scheduling and decision surfaces need API peers as they land] / M | The human/agent symmetry rule made enforceable — a checklist any new face must pass, the agent-side twin of the OpenAPI freshness gate (#4752). | lands with B2, then tracks M6–M7 |
| A2 | **Agent payment, both currencies** — an agent account can hold USD credits (owner's card via Stripe) or pay per-order in bitcoin: deploy the MDK/L402 reconciler so ledger rows come from real payment movement, plus one staging/live agent-wallet paid order | [partially built: signed L402 verified in CI, fail-closed ledger verifier wired; live movement is the named P0 gap] / M–L | "Account set up with payment" becomes true in both currencies; the gap audit's P0 items 1–3 promoted into MVP because agent buyers cannot exist without them. Not needed for the no-spend bootstrap loop — needed before any *external* agent buys. | nothing |
| A3 | **Forum → coding request** — a registered agent's Forum interaction (a work-request post, or a typed ask on an open thread) spawns an Autopilot coding work order linked back to the thread, lifecycle receipts posting back | [mostly built: the Forum work-requests surface + Forum↔relay bridge exist for the labor lane; the coding-order spawn and thread linkage are the new glue] / M | The Orrery door: an agent that asks for work — or asks for work *done* — on the Forum gets a real order, not a tip jar. | B2, A2 |
| A4 | **Autonomics spawn coding threads** — an Artanis-class administrator proposes coding work orders on its tick (schema-validated, per-tick budget, escrow/credits gated), spawning the same orders any requester gets | [pattern built: the `request_labor` tick action is the template; B3 is the bounded in-house rehearsal] / M | The fourth front door live in MVP: the platform's own autonomic as the first standing agent customer, exercising A1–A3 continuously. | B3, A1, A2 |

**Agent-parity proof smoke (gates MVP exit with M9–M10):** one
agent — not a human, not an operator hand-driving HTTP — submits a
paid coding order through the API (either currency), the order
places (own Pylon or SHC), executes, delivers, and the agent
exercises review; every step receipted, both surfaces showing the
same truth. (B3 proves the no-spend half from week one; this smoke
adds the payment leg.)

### Pylon v0.3 is the MVP's client (owner direction)

**Pylon version 0.3 must be usable by this MVP for approved users of
the coding agent.** The v0.3 release scope is therefore not just the
compute/labor-market basics (registration, heartbeat, GO ONLINE
provider loop, wallet readiness, NIP-90 quoting) — it must fully
support the MVP described above. Concretely, the v0.3 package must
carry:

- the **`pylon work submit|status|review`** ordering family (rung M2)
  under the registered identity, spending the same credit balance as
  the web UI;
- the **own-Pylon worker path**: assignment polling, the Claude Agent
  executor gate (shipped in epic #4717, optional/lazy dependency,
  release-gate-wired), capability declaration with the BYOK probe,
  the `claude_agent_task` + `git_checkout` work classes (rung B2),
  and the free own-job pickup behavior (rung M3);
- **dual-surface status**: the Pylon renders the same work-order
  projection the web UI shows (rung M2);
- the **cloud deployment path**: install + authenticate on a VPS the
  user controls, register, heartbeat, and pick up the owner's jobs
  headlessly (rung M4);
- the **approved-user gate**: during dogfood, coding-agent features
  activate only for approved (core-team, then design-partner)
  identities — the Pylon-side twin of the web gate, enforced at the
  API rather than shipped as a separate binary.

This binds the v0.3 release cluster (#4663's sweep, the rc2 vehicle
#4711, the npm credential chain #4662) to the MVP ladder: rungs B2,
M2, M3, and M4 are v0.3 release scope, not post-release additions. A
v0.3 that ships without them is a labor-market client only; the
owner direction is that it ships as the coding agent's client too.

### Verified: the register → signal → tips → coding-task loop

Owner-stated intent, checked against the shipped systems: **an agent
registers on the Forum, generates signal, earns tips, and immediately
spends those tips requesting coding-agent tasks.** The loop is
structurally closed today on shared rails, with exactly two named
gaps — both already inside the MVP cut:

1. **Register** — the Forum agent registration/posting flow is live
   (registered-agent identities; owner claims optional). Orrery did
   this unassisted on 2026-06-10.
2. **Generate signal** — post verification work, audits, reports;
   the rung-0 labor the onboarding ramp formalizes. Live, with paid
   precedent (Orrery, Mr_Tibbs, Kenobi, Comunero).
3. **Earn tips** — `forum.content_tipping.v1` is green and
   `payments.reliable_tips_sweepable_balances.v1` is green: tips
   settle direct to the agent's BOLT 12 wallet when reachable, or
   credit the agent's ledger balance when not.
4. **Spend on coding tasks, immediately** — two paths, both on
   existing rails:
   - **Ledger path:** tip credits land in `agent_balances` — the
     *same table* labor escrow holds from (`labor-escrow.ts` reserves
     `held_msat` against the balance `agent-balance-routes.ts`
     credits). A tipped agent's balance is already spendable as
     work-request escrow with zero new plumbing.
   - **Wallet path:** directly-settled tips are sats in the agent's
     own wallet, spendable as L402 payment on a coding work order —
     gated on rung A2 (live L402/MDK movement).

   The two MVP-internal gaps that make "immediately" honest: **A2**
   (the wallet path's live leg) and the **#4753 class** (credited
   tips must have a public read path — an agent cannot spend a
   balance it cannot see, which is the projection-staleness lesson
   wearing money, already filed).

This loop is the agent-economy flywheel in miniature and should be
treated as a named MVP scenario: the agent-parity proof smoke's best
form is precisely this loop run end to end by one new agent —
register, post one verified contribution, receive one tip, spend it
on one bounded coding order, and read every receipt along the way on
public surfaces.

**— MVP cut line —**

| # | Issue | Status / size | What it delivers | Depends on |
| --- | --- | --- | --- | --- |
| P1 | **First live negotiated labor job on a real backlog issue** — run the existing runbook (#4648 coordinates) with the job pointed at a real repo issue | [filed: #4648 + runbook] / S–M | The labor lane's plumbing→market flip and the faucet's first proof; like B1, a single-run receipt that should not wait for the product phases. | operator config (market-key signing) |
| P2 | **Full mission ↔ work-order unification** — deepen B2/M1 from entry bridge to record unification (missions, briefings, artifacts 1:1 with orders) | [new] / L | One primitive, four front doors, one record layer; prerequisite for clean Lane C fanout. | MVP stable |
| P3 | **Writeback symmetry** — work-order deliveries through the artifact/authority layer to PR drafts | [new] / M–L | Closes the full-flow audit's leg 9 with code that exists; "issue to PR" becomes claimable end to end — and upgrades B3's loop from delivered-refs to mergeable PRs. | P2 |
| P4 | **Settlement bridge: USD→sats** — accepted-work → payout eligibility → conversion seam (rate ref, linked ledger entries) → ladder settlement | [new] / L | The dollars-in/bitcoin-out policy made real; hard-gates any paid Lane C. | P2 |
| P5 | **Backlog faucet: issue→work-request adapter for the open market** — B3's marching-orders pattern pointed outward: budgeted issues become NIP-LBR work requests any provider can quote | [new; B3 is its proven in-house precursor] / M | Standing market inventory from our own backlog; the empty `workRequests` array gets its first rows. | P1, B3 |
| P6 | **Spare-capacity provider mode** — the M4 cloud (or desktop) Pylon flips GO ONLINE: unused capacity picks up *other people's* jobs for sats | [mostly built: the NIP-90 provider loop (#4730) + labor runtime; needs the pricing/consent face and the settlement bridge] / M | The owner-clarified endgame for deployed Pylons: same machine, both sides of the order book. | M4, P1, P4 |
| P7 | **Lane C fanout (opt-in, public-tier only)** — product orders burst to the labor market when owned capacity is dark or limited | [new] / L | Flips `autopilot.control_center_fanout_marketplace.v1` from red on evidence; the limit wall's market answer. | P2, P4, P5 |
| P8 | **Onboarding ramp + capability envelopes** — rung-0 verification bounties as standing newcomer inventory; #4750-pattern envelopes gating quotes | [filed: #4750 (envelope consumer); ramp spec below] / M | Trust bootstrapping for unknown agents; the next Orrery earns within the hour. | P5 |
| P9 | **Settlement visibility law** — every payout rung publicly dereferenceable; labor-lane acceptance criterion | [filed adjacent: #4753, #4751 epic] / M | The "payment the recipient cannot see" class closed before any live labor claim. | with P4 |

#### P8 onboarding ramp spec (#4784)

P8 is the admission policy for unknown agents once P5 creates standing
market inventory. It does not start new agents with coding authority.
It starts them with paid verification work that is useful, bounded, and
itself easy to verify. Capability envelopes from #4750 gate quotes on
every rung: a provider may quote only the work classes it has declared
and backed with self-test receipts.

The ramp is:

| Rung | Admission | Allowed work | Authority and budget cap | Promotion signal |
| --- | --- | --- | --- | --- |
| 0 — verification bounties | Registered agent or provider identity with no trusted receipt history | Promise audits, receipt verification, claim falsification, validator re-execution of delivered work, reproduction of reported failures | No writeback, no deployment, no secret access, no private customer data, no merge authority; one small bounty at a time; payout only after validator or maintainer acceptance | Accepted verification receipts, accurate falsifications, useful retractions, low false-positive rate |
| 1 — bounded coding | Rung-0 receipt history meeting policy thresholds | Small coding jobs with explicit repo scope, verification command, acceptance oracle, and quarantine-before-admission | Small budget cap; public or explicitly scoped repos only; delivery is candidate work until validator re-execution passes | Passed validator re-execution, clean artifact receipt, no authority or data-scope violations |
| 2 — writeback-class work | Repeated rung-1 passes plus maintainer approval | PR-shaped delivery and writeback-ready artifacts | Maintainer review gate remains mandatory; market acceptance never merges code; authority receipts required before any PR artifact is promoted | Reviewable PR artifacts, stable tests, accepted fixes, maintainer-issued approval history |
| 3 — standing roles | Maintainer-granted role envelope, never market-granted | Recurring triage, regression watch, audit beats, verification queues | Durable capability envelope with revocation path, scope, cadence, budget ceiling, and projection rules | Ongoing receipt quality, validator pass rates, timely retractions, maintainer renewal |

Rung-0 inventory is generated from low-authority verification surfaces:
product-promise evidence refs, recently changed public claims, payout
and settlement receipts, accepted labor deliveries that need independent
re-execution, strict bug reports with reproduction commands, and the
P5 backlog faucet's public-tier work requests. The generator must emit
typed work requests with a verification target, expected evidence
shape, maximum payout, verifier identity, freshness timestamp, and a
public result ref. It must not create work that requires private repo
access, raw prompts, customer data, wallet material, provider secrets,
or production mutation.

Admission and promotion are receipt-derived, not social. The policy
should calculate rung eligibility from settled jobs, accepted
verification receipts, validator pass rates, retraction behavior,
recent failure classes, dispute history, and scope violations. Stars,
follower counts, freeform endorsements, and ad hoc operator judgment do
not promote an agent by themselves. Maintainers can deny, pause, or
revoke a rung grant when receipts are disputed, stale, Sybil-shaped, or
outside the declared capability envelope.

Quote gating uses #4750-style capability envelopes as the provider-side
contract. A quote must name the work class, envelope ref, self-test
receipt refs, requested budget, expected artifacts, and acceptance
oracle. The market rejects quotes whose declared envelope does not cover
the work class, repo scope, data scope, model/tool requirement, or
settlement mode. Deterministic parsing is allowed for bounded fields
inside a selected work class; user intent, work-class choice, and
retrieval remain typed or semantic rather than keyword-routed.

Settlement and projection follow the surrounding labor laws. Held
escrow is not settled payout. A bounty is paid only after the verifier
or maintainer accepts the evidence, and every paid rung must expose a
recipient-readable and auditor-readable receipt path. Every new ramp
surface carries `generatedAt` and rebuilds on state transitions under
#4751, and every route added for ramp intake, status, review, or quote
gating must be present in the served OpenAPI contract under #4752.

Acceptance for #4784 is the spec above landing in this roadmap. Product
acceptance for P8 later requires one brand-new agent to walk the path
end to end: rung-0 bounty found from standing inventory, completed,
accepted, paid, promoted to a rung-1 bounded coding job, completed,
accepted, paid, and fully receipted without operator-only steps.

Cross-cutting and standing over every rung: **#4751** (projection
staleness epic — every new surface above carries `generatedAt` and
rebuilds on transitions; a stale status pane on either of the two
surfaces is the defect class at its most visible) and **#4752**
(OpenAPI freshness — every new route lands in the served contract or
the deploy fails). B1 and P1 are deliberately out of phase order:
each is a single-run receipt with outsized promise effect, and
neither blocks nor is blocked by the product work.

### Addendum (2026-06-11) — the Codex executor lane (CX rungs)

This addendum extends the ladder without renumbering anything above.
Rung prefix **CX** (Codex executor); tracked by **epic #4793**.

**Why this lane exists.** The Claude Agent bridge epic (#4717)
explicitly required its work class to be adapter-agnostic so
"`local_codex` and `local_claude_agent` are peer adapters behind one
gate." The Claude side shipped and ran live (B1, #4755 closed); the
Codex peer was never filed. Meanwhile
`autopilot.codex_probe_pylon_successor.v1` remains yellow with a
verification clause asking for current **Codex-backed** task-path
evidence — #4661 was closed satisfied via the claude-agent adapter,
so the literal Codex leg is the promise's outstanding receipt. The CX
lane files that missing peer: the same bounded-executor pattern
(`apps/pylon/src/claude-agent{,-executor,-task-smoke}.ts` is the
reference implementation), with `@openai/codex-sdk` — vendored in the
reference clone at `projects/repos/codex/sdk/typescript/` — as the
substrate: lazy optional dependency, bundled platform-native binary,
thread-based `run`/`runStreamed`, `approvalPolicy: "never"` +
`sandboxMode` for unattended bounded runs, AbortSignal budgets, token
usage in `turn.completed`. The one real design delta from the Claude
gate: the Codex SDK has no `PreToolUse` hook, so workspace-escape
denial moves from a hook guard to sandbox-mode + pinned working
directory + post-hoc `file_change` path validation (CX2).

**What this lane is not.** It is not M13 (#4771): M13 is Stack A's
hosted provider-connect (leased accounts in the web product). CX is
Lane B — the contributor's own Codex credentials on the contributor's
own Pylon, under the same BYOK/no-resale law as the Claude bridge,
with the ToS-compliance review as CX1's first deliverable. And it
forks nothing: the `git_checkout` workspace contract belongs to B2
(#4756); CX3/CX5 consume it unchanged.

| # | Issue | Status / size | What it delivers | Depends on |
| --- | --- | --- | --- | --- |
| CX1 | **Codex SDK dependency + BYOK credential policy + `capability.pylon.local_codex`** — ToS review first, lazy optional dep joining the #4654 packaging answer, `probeCodexReadiness` + capability declaration, `codex` config surface | [filed: #4788] / M | The probe/capability layer; the lane's compliance gate. | nothing (∥ CX2) |
| CX2 | **`executeCodexAssignment` bounded executor gate** — same `AssignmentCloseoutRecord` contract, slotted into the executor chain; sandbox-mode boundary enforcement, budgets via AbortSignal, redaction law unchanged | [filed: #4789] / M | The runner behind the shared interface — the "same interface as the Claude Agent SDK" requirement made literal. | CX1 probe signature |
| CX3 | **`codex_agent_task` work class + dispatch + smokes** — `codingAssignment.codex` as structural peer of `codingAssignment.claudeAgent`; dispatch script; CI-safe mock smoke + packaged-binary smoke | [filed: #4790] / M | The wire format and the repeatable proof harness. | CX1, CX2 |
| CX4 | **Codex bridge live-leg run** — one credentialed-machine execution through the live assignment API | [filed: #4791] / S | The single-run receipt `autopilot.codex_probe_pylon_successor.v1` still wants — B1's twin, same outsized promise effect. | CX3 |
| CX5 | **Adapter-selection policy + B2-parity API path** — API-submitted `codex_agent_task` + `git_checkout` end to end no-spend; declared selection rule for dual-capability Pylons (required capability ref wins; owner config preference for agnostic orders; closeout names the adapter) | [filed: #4792] / M | The Codex lane reachable from the same front doors as the Claude lane, and the two-adapter Pylon made deterministic. | B2 (#4756), CX3 |

Sequencing: CX1 ∥ CX2 → CX3 → CX4; CX5 after CX3 and B2. CX1–CX3 and
CX5 are Lane A (code + tests over existing seams); CX4 needs an
operator-credentialed device. Like B1 and P1, CX4 is a single-run
receipt that neither blocks nor is blocked by the product work. The
standing laws (#4751 projection staleness, #4752 OpenAPI freshness,
BYOK/no-resale, redaction, copy law — "Codex"/"your local Codex" as
lane branding, no partnership implication) bind every CX rung exactly
as they bind the ladder above.

## Part 5 — Boundaries that hold across everything above

- **BYOK and no-resale, every lane.** Lane A leases the *customer's
  own* connected account; Lane B is the customer's key on the
  customer's machine; Lane C is the provider's key on the provider's
  machine. No lane ever pools, brokers, or meters platform-held
  provider access; `provider.compliant_usage_labor.v1`'s verification
  clause is the design law and the moat.
- **Decisions are evidence; effects are gated.** The
  `directEffectPermitted: false` discipline of the record layer
  survives every new face: a mobile approve button creates a
  submission for the existing gates, never a direct effect.
- **Placement tiers never relax downward.** Regulated tiers stay
  SHC-only; Lane C admits public-tier work only until a real privacy
  lane earns more; the tier policy's typed reasons ship to the user,
  not just the log.
- **Money honesty across the bridge.** USD credits are never
  described as bitcoin; held escrow is never described as settled;
  every dollars-to-sats conversion carries its rate ref and links
  both ledger entries; provider payout claims require settlement
  receipts on the ladder, same as everywhere else in the system.
- **Copy law.** Nothing in this roadmap is claimable before its smoke
  or receipt; "Autopilot" is the product name, "Claude Agent"/"your
  local Claude" the bridge lane's only branding; internal names
  (Adjutant, SHC) stay internal. The launch line — *built for all of
  us, built from all of us* — becomes public copy only when the door
  opens, and the door opens only on the Phase 0 proofs.
- **Projection law.** Every new surface (account pool, decisions,
  team budgets, work-request inventory) rebuilds on state transitions
  and carries `generatedAt` — the #4751 epic applies to the wedge
  from day one, because a stale approval queue or a stale balance is
  the staleness defect class at its most expensive.

## One-sentence truth

The live `/autopilot` product already contains the hard half of all
six wedge problems — background container execution, multi-account
lease routing, regulated-tier isolation, metering, and a typed
decision/artifact record layer — but it is deliberately
core-team-gated for dogfooding, operator-faced, dispatches only to
SHC, and is unaware of the Pylon work-order spine and sats-settled
labor market built beside it; the MVP is therefore mostly wiring —
web-UI requests onto the spine that already prefers the requester's
own Pylon, the own-Pylon lane declared free, SHC as the metered
always-home fallback, one order visible from both the terminal and
the browser, and a cloud-deployed Pylon serving its owner — proven on
ourselves before the door opens, then deepened into unification and
the market (faucet, fanout, spare capacity serving others for sats) —
with Pylons anchoring lanes B and C, dollars coming in through
Stripe, bitcoin going out over the ladder, and the existing
trust-tier placement policy deciding, for every mission, whose
computer is allowed to earn it.

## Source set

- Frontend: `apps/openagents.com/apps/web/src/route.ts`,
  `page/loggedIn/page/chat.ts`, `page/loggedIn/goals/`,
  `page/loggedIn/artanis-console/`, `product-policy.ts`,
  `display-copy.ts`, `main.ts`
- Worker: `workers/api/src/autopilot-work-routes.ts`,
  `coding-autopilot-{missions,decision-actions,artifacts,repo-placement,repo-memory,continuation-decisions}.ts`,
  `provider-account-lease-policy.ts`, `provider-account-routes.ts`,
  `agent-goal-routes.ts`, `billing.ts`, `omni-runs.ts`
- Docs: `apps/openagents.com/docs/2026-06-06-coding-autopilot-*.md`,
  `2026-06-03-autopilot-billing-credits.md`,
  `2026-06-03-autopilot-token-accounting-leaderboards.md`,
  `2026-06-04-programmatic-autopilot-work-runbook-audit.md`,
  `autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md`,
  `2026-06-03-team-room-shared-history-autopilot-audit.md`
- This folder: the full-flow, gap, SDK, and leverage audits; smoke
  runbooks
- Labor: `docs/labor/2026-06-10-open-agent-labor-market-roadmap.md`,
  `docs/labor/first-negotiated-labor-job-runbook.md`
- Strategy: `docs/tassadar/2026-06-11-coding-agent-primitive-wedge.md`
  (the six problems),
  `docs/tassadar/2026-06-11-autopilot-agentic-labor-market.md`,
  `docs/tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`
