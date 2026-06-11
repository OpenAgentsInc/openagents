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

One blunt gating fact before anything else: **the live `/autopilot`
product is core-team-gated.** `loggedInWorkroomAllowed` requires
`authHasCoreTeamAccess` plus completed onboarding
(`apps/web/src/main.ts`). Whatever else this roadmap says,
"operationalize" begins with letting customers in.

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

| Lane | Runner | Inference paid by | Settlement | Status |
| --- | --- | --- | --- | --- |
| **A — Hosted SHC** | platform container | user's leased account (or platform-metered) | Stripe credits | live (Stack A's only lane) |
| **B — Owner's Pylon** | the customer's own machine | the customer's own BYOK key/subscription | none (their device, their key) | built in Stack B (Claude Agent bridge, capability declaration, worker loop); never reachable from the product |
| **C — Labor market** | someone else's idle agent | the provider's own account | sats escrow → ladder settlement | plumbed end-to-end, zero inventory |

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
4. **Settlement symmetry.** Lane A meters credits; Lane C escrows
   sats. The accepted-work → payout-eligibility bridge (P0.6 of the
   gap audit, still unbuilt) is the one money seam that must land
   before Lane C jobs pay providers from product demand.
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

The productization pass over shipped engines:

1. **Open the door**: public signup path replacing the core-team gate;
   onboarding = connect ChatGPT/Codex (and GitHub when wanted), get
   launch-grant credits, first mission. *(Gate: the existing
   redaction/copy law; no capability claimed beyond smokes.)*
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

Promises fed: `autopilot.free_coding_task_beta.v1`,
`autopilot.issue_to_pr_loop.v1` (with Phase 1.3),
`payments.accepted_outcome_economics.v1` (anchor outcomes),
`autopilot.agentic_labor_products.v1` (self-serve flows).

### Phase 1 — Unify the stacks (one primitive, every front door)

1. **Mission↔work-order unification** (§3.2.1) — the spine becomes the
   product's dispatch path; the chat is one front door among four.
2. **Lane B live**: Pylon connect in settings; owner-Pylon placement
   in product; the Claude Agent bridge live-leg run (clears
   `pylon.local_claude_agent_bridge.v1` and
   `autopilot.codex_probe_pylon_successor.v1`'s last blocker); the
   `pylon work submit|status|review` entry command.
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

Phases overlap deliberately: 0.1–0.4 (door, dashboard, scheduling,
decisions) are the commercial wedge and block nothing; 1.2 (Lane B
live leg) is a one-run receipt available *today* on a credentialed
machine; 2.1–2.2 (first job + faucet) need no product work at all and
should not wait for Phase 1. The only hard orderings are
writeback-before-issue-to-PR-claims and settlement-bridge-before-paid-
Lane-C.

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
- **Copy law.** Nothing in this roadmap is claimable before its smoke
  or receipt; "Autopilot" is the product name, "Claude Agent"/"your
  local Claude" the bridge lane's only branding; internal names
  (Adjutant, SHC) stay internal.
- **Projection law.** Every new surface (account pool, decisions,
  team budgets, work-request inventory) rebuilds on state transitions
  and carries `generatedAt` — the #4751 epic applies to the wedge
  from day one, because a stale approval queue or a stale balance is
  the staleness defect class at its most expensive.

## One-sentence truth

The live `/autopilot` product already contains the hard half of all
six wedge problems — background container execution, multi-account
lease routing, regulated-tier isolation, metering, and a typed
decision/artifact record layer — but it is core-team-gated,
operator-faced, SHC-only, and unaware of the Pylon work-order spine
and sats-settled labor market built beside it; the roadmap is
therefore productization (faces, proofs, signup) before unification
(one payload, three lanes) before market (faucet, fanout, first
negotiated job), with Pylons anchoring lanes B and C and the existing
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
