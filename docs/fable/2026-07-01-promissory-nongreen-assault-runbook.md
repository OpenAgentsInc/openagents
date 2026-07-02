# PROMISSORY — The Non-Green Promise Assault Runbook

Date: 2026-07-01
Status: standing operating procedure. This is the singular, repeatable
formula for a **PROMISSORY agent**: any agent (one, or ten at a time)
pointed at this doc claims the next non-green product promise, races
nobody, and thoroughly implements it until it is honestly done — blockers
cleared with evidence, or the owner-gated residue written down and routed.
It composes with [`EXECUTION.md`](./EXECUTION.md) (issues/PRs/worktrees/
review/counters) and the launch-alignment analysis
(`2026-07-01-product-promises-khala-code-launch-alignment.md`). It flips no
promise state by itself; state movement follows §6 exactly.

## 0. The One-Paragraph Formula

Pull the live registry → filter to eligible non-green promises → score each
with the selection formula (§3, throughline-weighted toward the current
campaign: **Khala Code imminent launch**) → claim the top unclaimed target
atomically via the claim protocol (§4) → run the assault ladder (§5): read
the record, decompose every blocker into engineering tasks, implement in a
clean worktree, verify, produce dereferenceable evidence, update the
registry in the same PR → move state only per the flip rules (§6) → merge
to `main` with the full landing bar green → write the owner-gated residue
to `NEEDS_OWNER.md` → release the claim → **immediately select the next
target**. Repeat until the pool is dry or the owner stops the run.

## 1. Ground Truth (as of registry `2026-07-01.2`)

- Canonical registry:
  `apps/openagents.com/workers/api/src/product-promises.ts`, served live at
  `GET https://openagents.com/api/public/product-promises`. Narrative
  mirror: `docs/promises/registry.md`. Gate stack:
  `docs/promises/checks-and-gates.md`. Record contract:
  `docs/promises/templates/promise-record.md`.
- Pool: **131 records — 34 green, 38 yellow, 41 planned, 15 red, 3
  withdrawn → 94 non-green targets.**
- Transition machinery: proposals via
  `POST /api/operator/product-promises/transitions`
  (`promise-transition-receipt-routes.ts`; mechanical checks:
  `promise_exists`, `from_state_differs`, `evidence_refs_present`,
  `verification_named`, and `blockers_clear_for_green`); public audit at
  `GET /api/public/product-promises/audit`.
- Every record already tells you what to build: `blockerRefs` are the task
  list, `verification` is the acceptance criteria, `authorityBoundary` is
  the fence, `unsafeCopy` is what you must never make true-by-copy.

## 2. Eligibility Filter (before scoring)

A promise is **assaultable** when ALL of:

1. `state ∈ {yellow, red, planned, degraded}` (withdrawn is history; green
   gets destale passes only, not assaults).
2. At least one blocker is **code-clearable**: an agent can clear it by
   building/test-proving something in this repo — surfaces, seams, tests,
   projections, receipts machinery, parity checklists, smokes,
   documentation-of-record. Blockers that are purely owner-gated (prod
   secrets, spend arming, sign-off, on-camera work, real-customer
   evidence, third-party deals) do NOT disqualify the promise, but a
   promise whose blockers are ONLY owner-gated is not assaultable — write
   its `NEEDS_OWNER.md` entry (if missing) and skip it.
3. Not currently claimed (§4).
4. **Not already mapped by other work.** Search GitHub for open issues,
   epics, and PRs referencing the promiseId, its blockerRefs, or its
   obvious surface (`gh issue list --state open --search "<promiseId>"`,
   plus the blocker slugs), and check whether a ROADMAP task or active
   fleet lane already covers it (the roadmap pairings in the
   launch-alignment doc §5 name several). If the work is mapped, it is
   OWNED — steer clear; do not PROMISSORY-claim it, do not duplicate it,
   and do not "help" inside someone else's lane. PROMISSORY exists to hunt
   the **hidden and overlooked** promises: the neglected tail nobody has
   filed anything against. (Exception: the mapping issue is stale — open
   \>14 days with no linked commits/PR and no active claim — in which case
   comment on it first, wait 6h, then claim per §4 with the takeover
   noted.) This whole filter is defeasible by explicit owner direction:
   if the owner points PROMISSORY at a specific promise or set, that
   direction wins over the already-mapped exclusion — note the override
   in the claim issue.
5. Not explicitly postponed or descoped by an owner decision recorded in
   the registry notes/caveats (e.g. "out of scope by owner decision" —
   search the record and notes for postpone language before claiming).

## 3. The Selection Formula

Score every assaultable promise; claim the highest score you can win.

```text
score = 3·throughline + proximity + clearability + leverage + freshness + neglect − scope
```

- **throughline (0–3, weight ×3).** Alignment with the current campaign.
  The owner sets this; TODAY it is **Khala Code imminent launch**:
  3 = `khala_code.*` records and anything on the gateway funnel spine
  (launch-alignment doc §6: free API → fleet connect → wallet → forum
  identity → go-online), e.g. `khala_code.forum_hotbar.v1`,
  `data.*`/`privacy.*` capture spine, `qa.agentic_qa_runner.v1`;
  2 = records the funnel routes to (Sites, legal lane, labor, training
  visibility, mobile companion);
  1 = platform credibility records (metrics, proof, promises
  infrastructure);
  0 = unrelated tails. When the owner declares a new campaign, only this
  table changes — the rest of the runbook is campaign-agnostic.
- **proximity (1–3).** Distance to the next honest state upgrade:
  `yellow`/`degraded` = 3 (often one blocker from green-candidacy),
  `red` = 2, `planned` = 1. Exception: a `planned` record whose yellow
  criteria are already written in its `verification` field and are purely
  code-shaped scores 3 (e.g. a surface that just needs building).
- **clearability (1–3).** 3 = every remaining blocker is code-clearable;
  2 = mixed (you can clear some, the rest becomes a crisp owner ask);
  1 = mostly gated (you can still tighten evidence and write the
  transition proposal).
- **leverage (0–2).** 2 if clearing this unblocks other records (count
  `promise:` cross-references pointing at it, or it is a named dependency
  in ROADMAP pairings); 0 if a leaf.
- **freshness (0–1).** 1 if the record's evidence/copy is visibly stale
  against `main` (cheap wins that also protect greens).
- **neglect (0–1).** 1 if nothing anywhere references this promise — no
  issues ever filed, no roadmap task, no recent registry note, no active
  lane. The overlooked record that everyone scrolled past is exactly the
  PROMISSORY specialty; mapped-and-active work is excluded upstream by
  the §2.4 filter, so this bonus rewards the truly forgotten.
- **scope (0–3, subtract).** Honest size estimate: 0 = a day of one agent;
  3 = multi-week/multi-seam. Prefer many completed medium assaults over
  one heroic stall. If your top pick scores high only because of
  throughline but is scope-3, take the next target and file the big one as
  an epic instead.

Ten agents starting simultaneously: each computes the same ranking, then
claims rank 1; whoever loses the claim race takes rank 2, and so on down
the list. No coordination needed beyond the claim protocol.

## 4. The Claim Protocol (no conflicts, no races)

The claim unit is **one promiseId**. One live claim per promise, ever.

1. **Search first.** `gh issue list --state open --search "PROMISSORY:
   <promiseId>"` (and closed within 7 days, to avoid re-grabbing a
   just-failed target without reading its after-action).
2. **Claim = open the issue.** Title exactly `PROMISSORY: <promiseId>`.
   Body: current state, the blocker decomposition (§5.2), the pinned
   verify command(s), the score you computed, and the campaign throughline.
   The issue must satisfy the repo's strict issue conventions.
3. **Race resolution is mechanical.** If two agents create the issue near
   simultaneously, the LOWER issue number owns the claim; the other agent
   closes theirs with a comment linking the winner and takes the next
   target. Check for a duplicate immediately after creating yours.
4. **Fleet-native runs** additionally register a work claim in the Pylon
   orchestration store (`apps/pylon/src/orchestration/store.ts`,
   `workUnitRef = promise:<promiseId>`) so the FleetRun planner
   (`work-planner.ts` typed skip reasons) structurally refuses to dispatch
   a second worker at a claimed promise. The GitHub issue remains the
   public claim of record either way.
5. **Claims expire.** No commit referencing the issue within 48h, or no
   PR within 5 days → any agent may post a takeover comment, wait 6h, then
   re-claim by linking a fresh issue. Never silently work a promise you
   have not claimed.
6. **Scope fence.** A claim covers ONE promiseId. If your work would
   materially advance another record, note it in your PR and let the next
   agent claim it — do not sprawl.

## 5. The Assault Ladder (per claimed promise)

### 5.1 Audit (read before building)

Read, in order: the promise record itself (claim, safeCopy, unsafeCopy,
blockerRefs, verification, authorityBoundary, evidenceRefs — follow every
evidence ref that still exists); the relevant `docs/promises/` gate docs;
any owning-app `AGENTS.md`/`INVARIANTS.md` for surfaces you will touch;
recent registry notes mentioning the record. Output: a short audit comment
on your claim issue stating what is REAL today vs what the record hopes.

### 5.2 Decompose blockers into tasks

Every `blockerRef` becomes exactly one of:

- **BUILD** — code-clearable: name the module/surface/test that clears it.
- **EVIDENCE** — already true but unproven: name the receipt, smoke,
  projection, or dereferenceable artifact that will prove it.
- **OWNER** — gated: write the crisp single-action ask (what to click,
  arm, sign, or spend) destined for `NEEDS_OWNER.md`.
- **EXTERNAL** — requires a real customer/third party: record what the
  moment it happens will need to capture, so the evidence isn't lost.

Post the decomposition to the issue. This is the contract your PR is
reviewed against.

### 5.3 Implement — "fully thoroughly, until done"

- Fresh worktree from clean `origin/main`; never disturb dirty checkouts.
- Build every BUILD item with tests; produce every EVIDENCE item as a
  dereferenceable artifact (route, receipt ref, committed smoke output,
  public projection) — narrative claims are not evidence.
- Honest-state discipline while building: `pending`/`not_measured` over
  fabricated values; skip-safe live tiers; public-safe projections;
  exact-only accounting; never weaken a gate, test, or policy to make a
  blocker "pass".
- Cleaner-than-found: fix cheap pre-existing breakage in touched areas;
  explicitly flag what you don't fix (the OpenAPI route-coverage red is
  the standing example of a flagged pre-existing failure).

### 5.4 Update the registry in the same PR

Apply the concurrent-safe edit protocol (§7): update YOUR record's
blockerRefs/evidenceRefs/safeCopy, add one registry note, bump the
version. The note must say exactly what cleared, what remains, and that
no state flipped (unless §6 authorizes the flip you're making).

### 5.5 Verify, deliver, close out

One PR per claim issue (`Closes #N`), body carrying the pinned verify
output. The landing bar: the record's own `verification` requirements you
touched, the full relevant suites, `check:deploy`, and the promise test
pins (the green-count assertion in `product-promises.test.ts` must be
updated ONLY by an authorized flip, never to make a failure go away).
Review per EXECUTION.md §5 (never self-reviewed by the authoring worker in
fleet mode). Merge → confirm issue closed → write `NEEDS_OWNER.md` items →
release any store claim → select the next target.

## 6. State-Flip Rules (what a PROMISSORY agent may and may not move)

- **→ green: NEVER by an agent.** Green is owner-signed, receipt-first,
  per `proof.claim_upgrade_receipts.v1`, with `blockers_clear_for_green`
  mechanically enforced. The agent's job is to make green a five-minute
  owner decision: every blocker cleared or crisply owner-shaped, the
  transition proposal drafted (promiseId, from/to, evidence refs,
  verification name), and the `NEEDS_OWNER.md` entry pointing at it.
- **planned → yellow: allowed** when the record's own `verification` text
  names yellow criteria and your PR satisfies them with tested, cited
  evidence. Say so in the registry note; flag it prominently for review.
- **red → yellow: allowed with caution** under the same standard — the
  blocking condition the red was protecting against must be genuinely
  resolved, not re-worded. When in doubt, clear blockers and leave state.
- **any → red/degraded: allowed and encouraged** when you discover a
  record overclaims reality ("broken glass under a green" — report it,
  downgrade honestly, file the fix).
- **→ withdrawn: owner direction only.**
- **Copy:** you may tighten/destale safeCopy for YOUR record; you may not
  broaden public copy anywhere, change marketing/homepage/onboarding
  wording, or make `unsafeCopy` sentences true by rewording them.

## 7. Concurrent Registry Edit Protocol (10 agents, one file)

`product-promises.ts` is a shared hot spot. Rules that keep N concurrent
PRs mergeable:

1. Touch ONLY: (a) your one promise record, (b) one appended note entry,
   (c) the `PublicProductPromisesVersion` constant. Nothing else.
2. Version numbering: take today's next free suffix (`YYYY-MM-DD.N`). On
   rebase conflict — someone landed first — re-number to the next free
   suffix and keep your note; the conflict resolution is always "both
   notes, highest N wins the constant."
3. Insert your note directly below the template line (newest first);
   never edit or reflow another pass's note or record.
4. Mirror the note into `docs/promises/registry.md` (prepend) in the same
   PR.
5. Run `bun run test -- src/product-promises.test.ts` (vitest, not `bun
   test`) plus typecheck before pushing; rebase-then-rerun on every
   conflict.

## 8. Mass Dispatch — Task the Fleet Right Now

The supervisor (Artanis-role session, per EXECUTION.md) runs:

1. **Snapshot + rank.** Pull the live registry, apply §2/§3, produce the
   ranked target list. (One bun script against
   `publicProductPromisesDocument()` suffices; publish the ranking in the
   run ledger.)
2. **Open the wave.** For the top K targets (K = ready fleet slots),
   confirm no existing claims, then dispatch one worker per target with
   the standard pinned prompt:

```sh
$PYLON khala request \
  --prompt "PROMISSORY assault on promise <promiseId> per docs/fable/2026-07-01-promissory-nongreen-assault-runbook.md. Claim via GitHub issue 'PROMISSORY: <promiseId>' (search first; lower issue number wins). Execute the assault ladder; registry edits per §7; state flips per §6 only. Public issue refs and public-safe evidence only." \
  --workflow codex_agent_task \
  --pylon-ref "<owner pylon ref>" \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "<current origin/main sha>" \
  --verify "bun run --cwd apps/openagents.com/workers/api test -- src/product-promises.test.ts" \
  --json
```

   (Equivalently: the `khala_fleet` MCP / Fleet-panel delegate with the
   same pins; once FleetRun T3.x is live, one `fleet_run_start` with the
   ranked list as the work source and `promise:<id>` as work units.)
3. **Refill on completion.** Every closeout (accepted or dead-end)
   releases its claim; the supervisor dispatches the next-ranked unclaimed
   target into the freed slot. Dead-ends get an after-action comment on
   the claim issue before the target returns to the pool.
4. **Ledger + counters.** Per EXECUTION.md §4/§6: dispatch ledger rows
   (promiseId ↔ issue ↔ assignmentRef ↔ worker ↔ state), exact
   token-row verification per closeout, periodic public-safe Forum
   updates, `NEEDS_OWNER.md` accumulating the owner-gated residue as a
   batch the owner can burn through in one sitting.
5. **Stop conditions.** Pool dry (no assaultable promise unclaimed), owner
   stop, or campaign switch (owner edits the §3 throughline table; ranking
   recomputes; running claims finish, new dispatches follow the new
   weights).

## 9. Definition of Done (per promise, per wave)

A PROMISSORY claim is DONE when: every code-clearable blocker is cleared
with tests and dereferenceable evidence; the registry record and mirror are
updated in the merged PR with an honest note; state moved only per §6; the
full landing bar was green at merge; owner-gated residue (if any) exists as
a crisp `NEEDS_OWNER.md` entry plus a drafted transition proposal; the
claim is released. A WAVE is done when every dispatched target is DONE or
after-actioned. The metric that matters is **owner-decisions-ready**: how
many promises are one owner action from green.

## 10. Anti-Patterns (instant review rejection)

- Claim-jumping (working an unclaimed or someone-else's promise), claim
  sprawl (one PR touching multiple records), or registry edits outside §7.
- Flipping green, weakening a gate/test/policy to clear a blocker, editing
  the green-count pin to silence a failure, or making `unsafeCopy` true by
  rewording.
- Broadened public copy, fabricated/narrative evidence, counter movement
  as proof, secrets or private data in issues/PRs/registry text.
- Sitting on an owner-gated blocker instead of writing the ask and moving
  to the next target — owner gates never stall the run.
- Heroic scope: if it's an epic, file it as one and take the next target.
