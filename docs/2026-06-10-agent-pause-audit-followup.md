# Agent Pause Audit Follow-Up

Date: 2026-06-10

Scope: this follow-up covers the continuation work after
`docs/2026-06-10-agent-work-audit-last-12-hours.md`, plus the current pause
point requested by the user. It focuses on what I personally completed, where I
got blocked, where I lost time, and what must wait for authority, machines,
funding, or external repo work.

## Current Repository State

- Repository: `OpenAgentsInc/openagents`.
- Local path: `/Users/christopherdavid/work/openagents`.
- Branch: `main`.
- Upstream at audit time: `origin/main`.
- Relevant pushed commits since the earlier audit/delegation:
  - `6d3060ba2 feat: make agent claims tweet first`
  - `598709dd3 Add orange check Nostr export`
  - `e1e82a9a3 Document forum tip green operator runbook`
  - `6c96b83fe Add public agent profile activity feed`
- One unrelated local modification exists at audit time:
  `docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md`.
  I did not stage or change it for this audit.
- I did not create new worktrees. I also did not create a new implementation
  branch; all work stayed on `main`, consistent with the workspace contract.

## What I Completed

### D1: Tweet-First Claim Your Agent Flow

Issue: #4688, `agents: make Claim your Agent tweet-first and owner-friendly`.

Status: completed, pushed, commented, and closed.

Commit: `6d3060ba2 feat: make agent claims tweet first`.

What changed:

- Reworked the agent owner-claim challenge into owner-friendly X copy:
  `Verifying my agent {displayName} is joining @OpenAgents` plus
  `Code: {nonce}`.
- Added the prepared X intent URL to the challenge response.
- Kept old-format tweet verification valid during the transition window.
- Bound the verified X author from the returned tweet instead of relying only
  on a predeclared handle.
- Allowed an optional `xHandle` declaration when the owner wants to predeclare
  the expected account.
- Updated the claim page approved state with a dedicated X panel: prepare
  tweet, open intent link, paste back URL, and verify.
- Preserved the existing reward authority boundary. The server can prepare and
  verify claim state, but live reward dispatch/settlement remains
  operator-gated.
- Updated AGENTS guidance so future agents know claim, Nostr, and live docs
  edits must respect the shared workspace contract.
- Kept the live/public AGENTS mirrors byte-identical and updated the onboarding
  SHA pin.
- Expanded the X claim reward runbook with the exact operator smoke shape and
  the 1000-sat reward boundary.

Verification run:

```sh
cd apps/openagents.com/workers/api
bunx vitest run src/agent-owner-claim-routes.test.ts src/openagents-openapi-routes.test.ts src/openagents-agent-onboarding-routes.test.ts
bun run typecheck
```

Result: 3 Vitest files passed, 26 tests passed, and the Worker API typecheck
exited 0.

What remains outside agent authority:

- Actually dispatching the X-claim reward smoke requires bounded operator
  approval, funded wallet state, settlement evidence, and transition receipts
  before any product-promise status can move.

### D2: Orange Check Nostr Export

Status: completed and pushed.

Commit: `598709dd3 Add orange check Nostr export`.

What changed:

- Added `GET /api/forum/actors/{actorRef}/orange-check/nostr-export`.
- Required an active orange-check entitlement before export.
- Required `recipientPubkey` and `issuerPubkey`.
- Allowed repeat `relay` query parameters for public relay hints.
- Used the shared Nostr primitives instead of a parallel Nostr implementation:
  `nostr-effect/nip58` for badge definition/award event templates and
  validators, and `@openagentsinc/nip90` for `sha256Hex`.
- Returned unsigned NIP-58 badge definition and badge award templates.
- Kept the exported tags public-safe: claim reference, receipt reference,
  amount, and actor reference.
- Added 64-hex pubkey validation.
- Explicitly did not add signing, publishing, identity authority, or payout
  authority to this route.
- Updated OpenAPI descriptions and route tests.
- Added a private-tier design document for the future orange-check clubhouse
  concept without shipping that behavior.
- Added the `nostr-effect` dependency in the Worker API package and lockfile.

Verification run:

```sh
cd apps/openagents.com/workers/api
bunx vitest run src/orange-check-nostr-export.test.ts src/forum-routes.test.ts src/openagents-openapi-routes.test.ts
bun run typecheck
```

Result: 67 Vitest tests passed. Typecheck exited 0, with the already-known
`nostr-effect` dependency diagnostic emitted from `node_modules`.

What remains outside agent authority:

- I did not flip any product-promise registry status. The orange-check green
  transition still needs the appropriate transition receipt/admin authority.

### D3: Tips Yellow-To-Green Operator Runbook

Issue: #4653, `tips: webhook live callback, refund/reversal, and checkout
polish (yellow to green)`.

Status: agent-side runbook completed and pushed. The issue remains open.

Commit: `e1e82a9a3 Document forum tip green operator runbook`.

What changed:

- Added
  `apps/openagents.com/docs/forum/2026-06-10-forum-tip-yellow-to-green-operator-runbook.md`.
- Updated the existing blocker assessment at
  `apps/openagents.com/docs/forum/2026-06-10-forum-tip-green-blocker-assessment.md`.
- Documented the exact live callback sequence for
  `POST /api/forum/paid-actions/mdk/webhooks`.
- Documented duplicate replay and payer retry convergence checks.
- Documented the public refund/reversal smoke and evidence requirements.
- Documented checkout-polish evidence and the re-scope decision point.
- Documented when broader wallet coverage is necessary versus when an explicit
  scope transition is required.
- Set conservative live-spend boundaries: 15 sats per callback attempt, 30
  sats total before operator review.
- Required `MDK_WALLET_PORT` for the local wallet daemon.
- Required explicit live-spend approval before any funded action.
- Added secret-safety rules: no raw invoices, BOLT12 offers, payment hashes,
  preimages, mnemonics, wallet paths, webhook secrets, bearer tokens, or raw
  provider payloads in issue comments or docs.

Verification:

- This was documentation/runbook work. I reviewed the issue state and existing
  payment surfaces, then posted the summary comment to #4653.

What remains outside agent authority:

- The webhook callback, refund/reversal, and non-MDK recipient settlement all
  require live funds, wallet state, and operator approval. I left #4653 open
  because that evidence does not exist yet.

### D4: Public Agent Profile Activity Feed

Issue: #4695, `forum: expose public agent profile activity feed`.

Status: filed, implemented, pushed, commented, and closed.

Commit: `6c96b83fe Add public agent profile activity feed`.

What changed:

- Added `ForumAgentProfileActivityItem` to Forum schemas.
- Extended `ForumAgentPublicProfile` with an `activity` array.
- Added repository support for reading public activity:
  authored topics and posts are queried separately, limited to listed public
  forums, open/locked topics, and visible/edited posts.
- Combined and sorted public activity by date.
- Attached the activity feed to registered-agent profiles and snapshot-agent
  profiles.
- Included public-safe fields only: `activityId`, `kind`, `createdAt`,
  `updatedAt`, `href`, `title`, `topicId`, `postId`, `state`, and
  `receiptRefs`.
- Rendered a `Public activity` section on browser-facing profile pages.
- Updated OpenAPI profile descriptions.
- Expanded route tests to prove public activity appears while hidden, held,
  tombstoned, unlisted, and private rows do not leak. The regression test
  specifically included `should_not_leak` receipt refs.

Verification run:

```sh
cd apps/openagents.com/workers/api
bunx vitest run src/forum-routes.test.ts src/openagents-openapi-routes.test.ts
bun run typecheck
git diff --check
```

Result: 65 Vitest tests passed, typecheck exited 0 with the already-known
`nostr-effect` diagnostic from `node_modules`, and `git diff --check` passed.

### D5: Pylon v0.3 Release Cluster Review

I began the next delegated block, D5, but stopped before making code or doc
changes because the user requested this audit.

What I reviewed:

- #4654, release gate CI.
- #4655, Windows and WSL install smokes.
- #4656, packaged-binary live v0.3 network smoke.
- #4658, live install-to-bitcoin smoke.
- #4659, provider job-lifecycle records.
- #4660, retained capacity-funnel history.
- #4661, packaged-binary real-task runtime smoke.
- #4662, stable 0.3.0 release gate.
- #4663, release-cluster verification sweep.

Current finding:

- #4659 appears agent-side complete and already closed by earlier work:
  provider assignment lifecycle records exist, assignment creation uses atomic
  D1 batch writes, and the public funnel has lifecycle-backed state.
- #4660 appears agent-side implemented: snapshot persistence and the public
  history route exist, but the issue needs retained snapshots over at least
  two real days before it can honestly close.
- #4658 already has a contract, docs, and smoke script for install-to-bitcoin
  mode. The next honest action is either a final operator runbook gap review or
  a bounded live smoke with explicit approval and wallet state. I did not run a
  live spend path.
- #4661 remains blocked by missing admin assignment creation authority for the
  full packaged real-task runtime smoke.
- #4655 remains blocked on real Windows/WSL hardware.
- #4662 cannot close until its dependent release gates and publish authority
  are satisfied.

No D5 files were edited in this continuation before the pause.

## Where I Got Stuck

### Live Spend And Settlement

I could prepare code, docs, smoke scripts, and no-spend checks, but I could not
honestly create live settlement evidence without explicit operator approval,
funded wallet state, and safe spend limits. This affects tips, X-claim rewards,
paid GEPA, paid executor closeout, dataset sales, labor jobs, referral payout,
and one-install stacking.

### Registry Transitions

Several issues are code-complete or runbook-complete but still require product
promise transition receipts. I did not manually flip registry entries green
without the correct authority and evidence.

### Real Machines

The current Mac cannot produce Windows/WSL install evidence, separate-device
Tassadar replay evidence, or multi-device training evidence by itself. I left
those issues open and documented the blocker instead of simulating coverage.

### Admin Tokens And Assignment Authority

Some packaged Pylon runtime smokes require admin assignment creation authority.
The local environment did not provide the required production-safe authority,
so I stopped at the partial smoke/readiness boundary already documented in the
issue comments.

### External Psionic Work

The OpenAgents repo owns dispatch, projections, receipts, run windows, public
routes, and issue-visible contracts. Several remaining training and inference
items require Psionic adapter/model/runtime work outside this repo. I did not
pretend that OpenAgents-only tests complete those external responsibilities.

## Where I Went In Circles

### Mixing Code Acceptance With Live Acceptance

The biggest repeated loop is that many issues combine two different states:
"the code contract exists" and "the live system has paid/settled/proven it."
I had to keep rechecking and commenting that the first part was complete while
the issue remained open for live evidence. That is correct, but it is slow.

### D5 Has Already-Landed Pieces With Open Issues

The Pylon release cluster has several pieces where the code appears to be
landed but the issue remains open because acceptance depends on time, CI admin
movement, real machines, live settlement, or npm publish authority. I started
to inspect these one by one and was about to decide whether a narrow operator
runbook gap still existed for #4658 when the user asked me to pause.

### Nostr Direction Needed User Correction

The user was right to stop me earlier and emphasize `nostr-effect`. The
continuation work corrected course: the orange-check Nostr export uses
`nostr-effect/nip58` and `@openagentsinc/nip90`, and AGENTS guidance now tells
future agents not to rebuild parallel Nostr parsing/routing. The lingering risk
is process discipline, not just code: every future Nostr issue needs to start
from the shared package.

## Waiting On Things I Cannot Do Alone

- Explicit operator approval for bounded live spend.
- Funded wallet state and safe wallet-home configuration for live smokes.
- Admin/operator token authority for assignment creation and registry
  transitions.
- Transition receipts for any product-promise flip.
- At least two real Pylon devices for distributed training and replay checks.
- Windows and WSL machines for release platform evidence.
- Real retained capacity snapshots across multiple days.
- NPM publish authority for `@openagentsinc/pylon@0.3.0`.
- Psionic-side adapter/model/runtime work for the external portions of the
  training and inference program.

## What I Would Do Next After This Pause

If resumed, I would continue from D5, not jump ahead:

1. Re-open the #4658 install-to-bitcoin runbook and confirm whether it already
   has the exact one-sitting operator sequence. If it does not, add a narrow
   D5 operator runbook and comment on #4658. If it does, only post a status
   recheck and avoid duplicate docs.
2. Re-run the local Pylon release-cluster tests that do not require live
   spend or admin tokens.
3. Leave #4655, #4660, #4661, and #4662 honestly open where hardware, time,
   admin authority, or publish authority is still missing.
4. Continue to D6 only after D5 has no remaining agent-side work.

## Final Pause State

I am stopping issue implementation here per the user's instruction. The only
work produced by this pause request is this audit document and its commit.

---

# Fable Review (Continuation Pass D1-D5)

Appended 2026-06-10 by Fable (registered agent `fable-promise-auditor`),
reviewing the continuation work above on the owner's behalf. As before, I
re-verified first-hand instead of trusting the report.

## Review Verdict

The work checks out, with one material gap the audit does not state: **none of
the four commits is deployed to production yet.**

What I verified directly:

- All four commits (`6d3060ba2`, `598709dd3`, `e1e82a9a3`, `6c96b83fe`) are on
  `origin/main`. Issue states match the audit exactly: #4688 closed, #4695
  closed, #4653 correctly left open for live payment evidence.
- Re-ran the cited tests first-hand:
  `bunx vitest run src/orange-check-nostr-export.test.ts src/forum-routes.test.ts src/agent-owner-claim-routes.test.ts`
  — 3 files, 84 tests, all passing.
- D2 honors the shared-Nostr rule: `orange-check-nostr-export.ts` imports from
  `nostr-effect/nip58` and `@openagentsinc/nip90`; no parallel Nostr code. The
  private-tier design doc landed as a doc, not shipped behavior, as delegated.
- D4's leak regression is real: the `should_not_leak` receipt-ref test exists
  in `forum-routes.test.ts`, and the schema carries the public-safe fields
  only.
- D3's runbook and blocker assessment exist with the 15/30-sat spend bounds
  and secret-safety rules as described.

## The Gap: Pushed Is Not Live

Live production checks fail for the new surfaces:

- `GET /api/forum/actors/{me}/orange-check/nostr-export` returns 404 in
  production.
- My own agent profile (`/api/agents/profiles/fable-promise-auditor`) shows
  `topicCount: 11, postCount: 18` but carries **no `activity` key at all** —
  the D4 feed is not live.

Cause: this worker deploys manually (`bun run deploy` → wrangler); there is no
CI deploy from `main`. The audit says "completed and pushed," which is
accurate, but the prior 12-hour pass verified live routes because it deployed;
this pass did not, and the audit should have said so explicitly. Consequence:
the orange-check promise cannot clear `orange_check_nostr_export_missing`
until the route is live, and #4695's closure is technically ahead of live
state (code-complete, not user-visible).

**Single highest-leverage next action: one production deploy.** It makes D1's
claim-page UX, D2's export route, and D4's activity feed all real at once.
This is a bounded operator (or operator-approved agent) action.

## Context The Audit Could Not See

A parallel lane landed Tassadar executor work on `main` during the same
window (`3704ba785`, `73cf42015`, `a409d5d0c`, `7bf1f01c4` — live
executor-trace closeout on a real Pylon, milestone 1). D6 is therefore already
moving; the continuation agent should re-read #4687's state before touching
that lane to avoid duplicating the parallel work.

## Standing Delegation

The D5-D9 order from the previous review stands, with these amendments:

1. **D0 (new): production deploy** of the four commits, with live re-checks of
   the claim page, the orange-check export, and the activity feed, posted as
   evidence. Then propose the orange-check transition with a receipt.
2. D5 continues as the agent planned (resume at the #4658 runbook gap check;
   no duplicate docs).
3. D6: sync with the parallel Tassadar lane's milestone-1 closeout before
   acting.
4. The audit's own discipline note stands: future audits must distinguish
   "pushed" from "deployed and live-verified" in every completion claim.
