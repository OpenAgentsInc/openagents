# Agent Work Audit: Last 12 Hours

Date: 2026-06-10

Scope: this audit covers the intensive issue, implementation, documentation, and
verification pass on `OpenAgentsInc/openagents` from roughly 2026-06-09 21:15
through 2026-06-10 09:15 Central time. The concrete commit range visible at
pause time is `702ff30ec` through `109e804b3` on `main`.

## Executive Summary

The pass moved a large amount of OpenAgents product infrastructure from promise
or audit text into integrated Worker, Pylon, Forum, Nostr, and training surfaces.
The work landed on `main` and was pushed continuously. The most important
outcome is that the repo now has much clearer public and internal contracts for:

- Forum paid actions, orange-check purchases, and atomic redemption writes.
- Pylon v0.3 release gating, package smokes, runtime-task gating, capacity
  funnel accounting, MDK send readiness, and legacy Spark/Breez recovery.
- NIP-90/NIP-DS market primitives, including a scoped relay, provider loop,
  operator-gated buy mode, and public market receipts.
- Conversation-bundle data listings, redaction, compliant labor job kinds, and
  referral payout accounting.
- The CS336 distributed-homework program, including A1/A2/A3/A4/A5 public
  status feeds, verification challenge queues, receipt-backed leaderboards, and
  Tassadar executor-trace homework wiring.
- The Tassadar/Percepta research thread, including the relationship between
  compiled computation and trained computation, and why OpenAgents should treat
  Rust as the implementation language for this lane.

The work did not make every open promise green. Many AtlantisPleb issues remain
open because their acceptance criteria require live spend, production operator
authority, real multi-device Pylons, Windows/WSL machines, settled Lightning
receipts, or external Psionic adapter/model work. In those cases I posted
evidence comments where I had completed the no-spend or code-contract portion
and explicitly left the issue open.

## Repository State At Pause

- Branch: `main`.
- Upstream: `origin/main`.
- Worktree: clean before writing this audit.
- Worktrees: only `/Users/christopherdavid/work/openagents` existed; no stale
  worktrees made by me were present.
- Stop point: I had just rechecked issue #4680 and run the A4 smoke locally.
  I had not posted the #4680 evidence comment before the user requested this
  audit and pause.

## Major Work Completed

### Forum, Payments, And Orange Check

I implemented and pushed the Forum paid-action path needed to sell orange-check
membership through the main OpenAgents surface. This included checkout page
serving, QR rendering, Forum entitlement storage, badge projections, homepage
Forum stats, and D1 constraint/migration support for the `orange_check` action
kind.

The redemption path was corrected after a live incident: consistency-critical
Forum paid-action redemption writes now use an atomic D1 batch instead of
sequential `.run()` calls. I reran and documented the orange-check smooth
purchase path and recorded the first live orange-check purchase in the product
registry.

I also worked through BOLT12 direct-tip smoke evidence. The important result was
that the strict funded smoke passed after fixing false self-pay classification
around LSP pubkeys. Remaining tip work is not just code: webhook callback,
refund/reversal behavior, and checkout polish still need live callback evidence.

### Product Promise And Five-Streams Audit

I audited the five Bitcoin revenue streams promise and corrected a bad product
premise: the fifth stream is agent labor, not capacity resale. I removed the
capacity-resale promise direction and replaced it with a no-resale-compliant
plan centered on paying for accepted work output.

I added a binding delegation contract to the plan, recorded filed issue numbers,
and refreshed registry/product-promise context. This work created the issue
backlog now being processed in sequence.

### Nostr, NIP-90, And Market Work

After the user called out that the workspace already has substantial Effect
Nostr code, I corrected course toward shared Nostr primitives instead of
rebuilding behavior ad hoc.

Concrete outcomes:

- Exposed a shared NIP-90 protocol package in the Bun/Effect workspace.
- Restored draft NIP specs for DS, SKL, SA, AC, and TRN as living docs.
- Promoted the relay POC into a scoped market relay.
- Added the Pylon NIP-90 provider loop behind the explicit `GO ONLINE`
  boundary.
- Added an operator-gated buy-mode dispatcher with spend caps.
- Exposed public NIP-90 market receipts and stats projections.
- Added NIP-DS listing and offer flow work for dataset listings.

The repo guidance was also updated so future agents know to use the shared
`nostr-effect` / `@openagents/nip90` primitives and not hand-roll Nostr logic in
new surfaces.

One important caveat: the broader `nostr-effect` Effect v4 migration was handled
as a prerequisite direction, but the live product issues still need continued
discipline here. Any new Nostr work should start by reading the shared package
and the repo instructions, not by writing another parallel parser/router.

### Data, Labor, And Referral Streams

I implemented the conversation-bundle sale digest alignment, redaction tooling,
and dataset-listing pieces needed for the data stream's code contract. I also
defined compliant labor job kinds and added Pylon labor intake so work runs on
the contributor's own agent rather than brokering access to accounts or seats.

For referrals, I added signup/order attribution consumption and a referral
payout ledger. The stream still needs the first settled referral payout and live
evidence before the corresponding blocker can honestly clear.

### Pylon Release And Runtime Hardening

I added or refreshed a large part of the Pylon v0.3 release-support surface:

- Release-gate workflow staging.
- Platform support matrix docs.
- Packaged network smoke.
- Packaged runtime-task smoke.
- MDK send-readiness preflight.
- Provider lifecycle accounting.
- Retained capacity-funnel history.
- Runtime task gate.
- Psionic connector state.
- Psionic training boundary.
- Legacy Spark migration preflight.

I fixed follow-up gaps as they surfaced, including package/runtime smoke
packaging, runtime time helpers, and conversation-bundle sale digest alignment.

### Legacy Spark/Breez Migration

The user reported that `pylon wallet migrate-spark` on v0.2.5 detected an old
Spark balance but failed with `Missing Breez API key`, while the user still had
the 12-word mnemonic.

I investigated old Pylon/Spark migration behavior, opened a new GitHub issue for
the failure mode, implemented guided recovery support, documented the flow, and
closed the issue after pushing the fix.

Final issue handled for this specific report:

- #4686, `pylon: ship guided Spark/Breez mnemonic migration for v0.2.5 users`
- Commit: `372e54742 fix: guide legacy spark wallet recovery`
- Verification: `bun test tests/wallet.test.ts --max-concurrency=1` and
  `bun test --max-concurrency=1` in `apps/pylon`

This does not magically restore channel liquidity from a mnemonic. The honest
contract is a guided recovery path that explains when a Breez API key is needed,
what is locally recoverable, and when manual/operator-assisted recovery remains
necessary.

### CS336 Distributed Homework Program

I audited the deleted homework economy and reconstructed the modern route
through the current Worker authority instead of resurrecting deprecated
`nexus-control` code. The resulting audit lives at
`docs/2026-06-10-cs336-distributed-homework-continuation-audit.md`.

Implemented surfaces include:

- Training window authority.
- Training verification challenge queue.
- CS336 A1 no-spend homework rehearsal.
- Validator assignments for weak-device work.
- Public training run pages.
- A1 real-gradient public status.
- A3 IsoFLOP dashboard feed.
- A4 data-refinery contracts.
- A2 device-capability feed.
- A5 alignment eval feed.
- Receipt-backed training leaderboards.
- Tassadar executor-trace homework wiring.

The code contracts are present and tested, but live acceptance for most of these
issues requires real runs, real devices, verified receipt rows, and in some
cases operator funding.

### Tassadar And Research Documentation

I created and expanded the Tassadar documentation lane, audited Tassadar and
Percepta history across OpenAgents and Psionic, and wrote the explanation that
connects Tassadar to CS336: compiled computation and trained computation are
separate but mutually useful work classes.

The docs now include:

- A relocated Tassadar/Percepta audit.
- Ramifications essays.
- A transformer-VM reference review.
- A comprehensive Tassadar essay.
- README/AGENTS documentation that makes the research lane easier to find.
- A clarification that C is incidental to the referenced construction, while
  Rust is the implementation language for our work.

I also seeded Research Forum topics for Tassadar and Psionic and recorded
executor-compiler batch landings in the design doc.

## Issues Closed During The Pass

Notable recently closed AtlantisPleb issues include:

- #4635 through #4640 for NIP-90/NIP-DS/relay/provider/buy-mode/receipt
  surfaces.
- #4643, #4644, #4646, #4647, #4649, and #4650 for data, labor, and referral
  code contracts.
- #4657, #4659, #4664, #4672, #4673, #4674, #4685, and #4686 for Pylon,
  provider lifecycle, Psionic connector, Spark/Breez migration, training
  authority, and verification machinery.

Some issues were closed only where the code/docs/tests acceptance was actually
met. I left issues open where the remaining acceptance criteria require live
settlements, operator flips, real machines, or external Psionic work.

## Open Issues Rechecked But Not Closed

I rechecked and commented on many open AtlantisPleb issues with current evidence
instead of closing them prematurely. Current recurring blocker classes:

- No settled Bitcoin receipt yet.
- No verified public receipt row yet.
- No live real Pylon capacity beyond dark/unassigned accounting.
- No operator token or spend authority in the local environment.
- No Windows/WSL hardware available from this Mac.
- No remote multi-device Qwen training run completed.
- Psionic adapter/model work still external to this monorepo.
- Product-promise registry transitions require operator/maintainer authority.

Specific issues confirmed open at pause time included #4641, #4642, #4645,
#4648, #4651, #4652, #4653, #4654, #4655, #4656, #4658, #4660, #4661, #4662,
#4663, #4665, #4666, #4667, #4668, #4669, #4670, #4671, and #4675 through
#4684.

For #4680 specifically, before pausing I had verified:

- `bunx vitest run src/cs336-a4-data-refinery.test.ts src/training-run-window-routes.test.ts src/training-leaderboards.test.ts`
  passed.
- `bun run smoke:cs336-a4:data-refinery` passed with 2 files and 9 tests.
- Live `GET /api/training/leaderboards/a4_eval_delta` was present but had no
  rows and reported the blocker
  `blocker.training_leaderboard.a4_eval_delta.requires_verified_receipts`.

I did not post the #4680 evidence comment because the user interrupted the turn
and asked for this audit instead.

## Where I Got Stuck

### Live Spend And Settlement Authority

Several issues require spending sats, receiving provider-confirmed settlements,
or running paid closeouts. I did not have explicit operator approval for new
Lane B spend beyond bounded smoke paths already discussed, and I did not have a
current OpenAgents admin token for registry transitions or production mutation
routes. I therefore stopped at code/docs/tests or no-spend live reads and left
those issues open.

### Real Device Coverage

Pylon v0.3 acceptance repeatedly calls for real packaged-binary smokes on
specific platforms and multiple real devices. From this environment I could
verify macOS/local package behavior and public routes, but I could not honestly
produce Windows/WSL evidence or multi-device remote training evidence without
available machines and credentials.

### Psionic External Dependencies

Several CS336 issues intentionally split authority: this monorepo owns dispatch,
verification, receipts, run/window authority, leaderboards, and public
projection, while Psionic owns adapter ports, model code, kernels, and training
lanes. I documented exact asks and wired OpenAgents boundaries, but I did not
complete external Psionic conformance work inside this repo.

### Product-Promise Registry Flips

The code can expose evidence and proposed status, but flipping promises green is
operator/maintainer authority. I did not backfill registry transitions without a
proper transition receipt.

## Where I Went In Circles

### Spark/Breez Migration Issue Duplication

The legacy Spark/Breez migration problem appeared in multiple issue forms
(including #4672, #4685, and then the user-requested #4686). The useful work was
the final guided recovery implementation and docs, but the issue flow was more
duplicative than ideal. Future handling should consolidate follow-up Spark/Breez
reports under the final guided-migration issue family unless there is a truly
new failure mode.

### Open Versus Done On Acceptance Criteria

Many issue bodies mixed "land the code contract" with "prove it live with
settled receipts or real devices." I repeatedly had to distinguish those two
states in comments. That is correct, but it made the loop slower: a code
surface could be complete while the issue still had to remain open.

### Nostr Effect Direction

The user correctly stopped me to reinforce that OpenAgents already has Effect
Nostr code and that new work must use it. The correction was made by emphasizing
shared `nostr-effect` and `@openagents/nip90` usage and avoiding a parallel
Nostr rebuild. Future Nostr work should begin from that shared package and the
workspace instructions.

### Broad Repo Discovery

At pause time I accidentally started an overly broad `find .. -name AGENTS.md`
scan, which was unnecessary in a repo of this size. I killed that process and
continued with targeted status/docs/GitHub checks. Future orientation should use
`rg --files -g AGENTS.md` or scoped paths.

## Waiting On Work I Cannot Honestly Do Alone

- Operator funding or explicit spend approval for live paid smokes.
- A current admin/operator token for product-promise transition routes.
- Real multi-device Pylon availability for distributed training evidence.
- Windows and WSL machines for platform-matrix smoke evidence.
- Provider-confirmed settlement receipts for compute, data, labor, referrals,
  tips, and stacked one-install flows.
- Psionic adapter conformance for remaining A4 data-refinery stages and model
  training/eval loops for A3/A5 bonus-quality evidence.
- Production deployment decisions when issue acceptance requires deployed
  state beyond local code/tests.

## Verification Pattern Used

The dominant verification pattern was:

- Worker tests from `apps/openagents.com/workers/api` with
  `bunx vitest run <files>`.
- Web tests from `apps/openagents.com/apps/web` when UI route behavior was in
  scope.
- Pylon tests from `apps/pylon` with `bun test`.
- Live route reads against `https://openagents.com` with cache-busting query
  parameters where public deployment state mattered.
- Issue comments carrying commit SHAs, test commands, live route names, and
  blocker refs rather than claiming green status prematurely.

## Recommended Next Step After This Pause

Resume at #4680, because the A4 smoke has passed but the evidence comment has
not yet been posted. Then continue numerically through #4681, #4682, #4683, and
#4684. Do not close those unless their live receipt/run/device acceptance is
actually satisfied.

For the broader open queue, prioritize grouping by blocker class rather than
touching code blindly:

- Live-payment issues need operator spend and settlement receipts.
- Device issues need real machines.
- Psionic issues need external repo work or a documented dependency handoff.
- Registry issues need transition receipts before product-promise edits.

