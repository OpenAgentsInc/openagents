# P6/P7 (#4782 / #4783) — Remaining-Work Map

Date: 2026-06-14

#4777 (P1) and #4781 (P5) are **closed with live settlements**. The reusable
proven machinery from those runs:
- `apps/pylon/scripts/drive-labor-chain.ts` — settles a full market labor job
  (quote → offer → accept → escrow → codex execute [network-denied sandbox] →
  result → release → settled). This IS the "stranger's paid job" leg for #4782
  and the market-provider leg for #4783.
- `apps/openagents.com/scripts/backlog-faucet-list.ts` — lists real issues to the
  open market.
- Provider Pylon: home `/tmp/oa-provider-home`, pubkey `3fd9b3f1…`, lifecycle
  online, first-run labor approved, declares `capability.pylon.local_claude_agent`
  / `local_codex`.

## #4782 spare-capacity provider — what's left

Acceptance: one deployed Pylon completes its **owner's job** AND a **market
job** the same day, with own-work preemption, settlement to the owner's wallet,
public receipts.

- **Market-job leg: DONE-equivalent.** `drive-labor-chain.ts` already settles a
  market job by the provider (proven twice: #4777, #4781). Run it once more for a
  same-day receipt.
- **Preemption logic: EXISTS + is pure/testable.**
  `apps/pylon/src/coordinator/spare-capacity-provider.ts`
  `evaluateProviderAvailability` refuses market work when
  `ownedQueueDepth !== 0` (own work preempts). The worker gate
  `apps/openagents.com/workers/api/src/market-provider-policy.ts`
  `evaluateMarketProviderMode` is default-off; turn it on with the documented
  owner defaults (consent ref, pricing policy ref, earnings-visibility ref,
  `ownWorkPreemption=true`, `maxJobSats=25000`, `minQuoteSats=1000`,
  `settlementBridgeReady=true` — P4 #4780 is CLOSED). A focused test asserting
  `ready` with defaults and `preempted` when owned work is queued closes the
  policy half.
- **Owner-job leg: THE REAL GAP.** `pylon assignment run-no-spend`
  (`apps/pylon/src/assignment.ts` `runNoSpendAssignment`) executes an owner
  assignment and emits a receipt — **but it requires a dispatched no-spend
  assignment lease** from `pollAssignments`, which requires the provider Pylon to
  be **registered/linked** for owner-assignment dispatch (M4 #4762). The current
  provider home is `registered:false / linked:false` (presence-state). So the
  owner-job leg needs: (1) register/link the provider Pylon as a node, (2) submit
  an owner work order (`pylon work submit`) that dispatches a no-spend lease to
  it, (3) `pylon assignment run-no-spend` → owner-job receipt.
- **Earnings visibility:** surface per-job settled sats (the market job's 1 sat)
  in the provider projection (P9 law).

Closeable once the owner-job leg runs (registration + dispatch) on the same day
as a market job, both with public receipts. Do **not** fake the owner-job
receipt — register the node and run a real assignment.

## #4783 Lane C fanout — what's left

Acceptance: one real product order (Autopilot work order) with owned capacity
**dark** bursts to the market, completes via a provider, opt-in honored,
public-tier floor enforced server-side, paid in USD credits → P4 USD→sats bridge
→ sats settled, receipts public.

- **Foundation:** `apps/openagents.com/workers/api/src/lane-c-fanout-policy.ts`
  (the opt-in / public-tier gate). P4 bridge (#4780) is CLOSED/built
  (`market-provider-policy.ts`, `pylon-bitcoin-accounting-receipts.ts`).
- **What to wire:** Lane C as a placement fallback tier in the work-order
  placement policy (own Pylon → SHC → market), with the **public-tier floor
  enforced server-side** (only `public`-tier repos may leave first-party lanes);
  customer opt-in + per-order budget ceiling; funding via the P4 USD→sats bridge;
  validator-pass-before-release; delivery through the same artifact layer so a
  market-fulfilled order looks identical to a first-party one.
- **Live proof:** submit a public-tier Autopilot work order with owned capacity
  forced dark + customer opt-in → it fans out to the market → the provider Pylon
  quotes/executes/settles (reuse `drive-labor-chain.ts` for the market leg) →
  USD credit debit funds the sats escrow via P4 → settle → public receipts.

## NEEDS-OWNER (the real wall, found 2026-06-14)

Progress this iteration: the provider Pylon is now **registered** as the owner's
node (`pylon presence register`, `registrationRef registration.pylon.5e0fbea1…`).

But the **owner-job / product-order submission is auth-gated**, and this is the
genuine blocker for both #4782 and #4783's live proofs:

- `pylon work submit` and `POST /api/autopilot/work` use
  `authenticateCustomerOrderAgentRequest`
  (`customer-order-agent-auth.ts`), which requires the bearer agent to carry an
  **active customer-order grant with the required scope**. The labor requester
  token (Raynor) authenticates as an agent but has **no customer-order grant**,
  so every submit returns **401 unauthorized** (verified by direct probe).
- #4782's owner-job leg needs a *real* owner Autopilot work order dispatched to
  the provider node; #4783's leg needs a *real* product order. Both require that
  customer-order-granted credential. It cannot be faked (and the run mandate
  says not to fake the owner-job receipt).

**To unblock (owner action — one of):**
1. Provision a **customer-order grant** for an existing agent (e.g. Raynor) so
   `pylon work submit` is authorized, or
2. Provide an **agent token that already has a customer-order grant**, or
3. Submit the owner work order / product order via an **authenticated browser
   session** (the route also accepts `requireBrowserSession`).

Once any of those is available, the remaining steps are fully mapped above and
the market leg + drivers are proven — both issues can then close with live
receipts. Until then they are honestly owner-blocked on this credential, not on
missing engineering.

## Update 2 (owner unblocked the grant): structural blockers REMOVED

The owner authorized provisioning the credential. Done + verified:

1. **Customer-order grant provisioned** for Raynor (active `customer_orders.*`
   scopes in `agent_profiles.metadata_json`). `POST /api/autopilot/work` now
   authorizes (was 401). Owner work orders create successfully.
2. **Real placement bug fixed + deployed.** The work-order placement selector's
   `pylonRegistrations` dependency was never wired in production, so it always
   saw an empty registry and every order fell to the SHC lane —
   `requester_pylon` placement was dead. Wired it from the D1 pylon-api store
   (`index.ts`, deployed). Now an owner's online, eligible Pylon is selected for
   `requester_pylon`.
3. **Provider node made assignment-eligible** (owner-authorized, owner's own
   node): registered + `capability.pylon.assignment_ready` + wallet-ready +
   fresh heartbeat. Owner work orders now place on `pylon.5e0fbea1`
   (`placement: requester_pylon`).
4. **Owner-job dispatch → accept → execute → deliver PROVEN.** `pylon assignment
   run-no-spend` polls the dispatched no-spend lease, passes local admission,
   executes the provider's own agent, and the work order reaches state
   `delivered` with **progress + closeout receipts** (e.g. work order
   `28dd20a7…`, progress `assignment.progress.55e664e8…`, closeout
   `assignment.closeout.20e08e76…`).

**Remaining (smaller, known class):** the assignment's verification runs
`bun test` against the **full repo checkout**, so the closeout comes back
`rejected: claude_agent_test_failed` (the whole-repo suite, not the agent's
slice). This is the same whole-repo-verification problem already fixed for codex
labor (network-denied, self-contained slice). The clean path is the
self-contained runtime gate (`executeRuntimeGate` sum-fixture) or a scoped
verification command; routing the assignment there yields a clean `accepted`
closeout. Once an owner job closes clean, #4782 = that owner job + a same-day
market job (`drive-labor-chain.ts`, already proven via #4966) + preemption
(`evaluateProviderAvailability`) + earnings, all on `pylon.5e0fbea1`.

## Honesty posture (unchanged)

The provider is a genuinely independent node identity (separate home + pubkey),
a real second market participant at the protocol level — not a different human,
and never a faked receipt. Both remaining proofs must RUN (real assignment, real
fanout, real settlement) before their issues close.
