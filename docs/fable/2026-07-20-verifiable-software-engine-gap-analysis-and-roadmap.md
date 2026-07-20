# The Engine of Verifiable Software — Gap Analysis and Roadmap

**Date:** 2026-07-20
**Lane:** Fable strategy analysis
**Status:** Analysis and evidence survey only. This document flips no promise
state, changes no runtime authority, mints no issue, and dispatches no work.
Factual status authorities remain current code, `docs/sol/MASTER_ROADMAP.md`
(revision 125), live issue state, contracts, and receipts. Proposal packets
named below require Sol admission and owner acceptance before any dispatch.
**Sources:** `docs/sol/MASTER_ROADMAP.md`, the full `specs/` corpus and
`docs/mvp/` ProductSpecs, all eight AssuranceSpecs, open and recently closed
GitHub issues (snapshot 2026-07-20), `docs/assure-repo/` artifacts,
`packages/assurance-spec`, `packages/behavior-contracts`, `packages/nip90`,
`docs/ide/ROADMAP.md`, `docs/promises/`, and `docs/sol/receipts/` plus
`docs/sol/evidence/`.
**Companion:** [`2026-07-19-verifiable-software.md`](./2026-07-19-verifiable-software.md)
— the thesis this analysis measures the repository against.

---

## I. The target

The essay defines the goal. Verifiable software is software that can show its
work, name its gaps, and prove what it did. Its deliverable is the accepted
outcome: work scoped in advance, executed, verified against a rubric, recorded
in a receipt, and settled. The "engine" of verifiable software is the system
that runs that loop end to end, for its own codebase first and for customers
after.

The engine decomposes into eight stages. Each stage must hold at product
scale, not just in one precedent:

1. **Intent.** Falsifiable definitions of done exist before work runs.
2. **Proof design.** Each intent carries oracles, falsifiers, and evidence
   policy, digest-bound to the exact intent bytes.
3. **Admission.** An independent reviewer, never the producer, admits the
   proof design.
4. **Execution.** Work runs inside declared containment, with the authority
   manifest and the execution receipt recorded as a pair.
5. **Evidence.** The host observes results through its own services. Absent
   evidence renders unknown, never green.
6. **Verification.** Oracles run, mutations test the oracles, drift checks
   test the documents, and independent falsifiers attack the claims.
7. **Acceptance.** A typed verdict issues from an oracle or an owner, at the
   narrowest true rung, and no rung implies the next.
8. **Delivery and settlement.** Verified work reaches users through signed
   releases, and verification itself becomes a priced, receipted product.

This analysis grades each stage against what the repository actually holds
today, then orders the remaining work.

## II. Scorecard — the engine on 2026-07-20

| Stage | Grade | Narrowest current truth |
| --- | --- | --- |
| 1. Intent | Strong | 12 canonical ProductSpecs plus 3 MVP specs. About 90 behavior contracts, 57 enforced. About 155 promise records. |
| 2. Proof design | Strong on paper | 8 AssuranceSpecs. Full Auto holds complete proof designs for all 76 criteria. |
| 3. Admission | Bottleneck | One admitted AssuranceSpec (the MVP). Seven remain `proposed`. No standing reviewer population. |
| 4. Execution | Partial | Sandboxes SBX-00..08 landed. SBX-09 live matrix green but `INCONCLUSIVE`. Manifest/receipt pairing absent from the run loop. |
| 5. Evidence | Strong, fresh | IDE-10/11/12 delivered host-observed tests, debug, and SCM delivery. Dated receipt trail active. |
| 6. Verification | Partial | Mutation engine landed. Inventory holds 161 surfaces, zero silent. Standing sweep runs degraded and red. |
| 7. Acceptance | One precedent | MVP: 18 obligations CONFIRMED with independent review. Full Auto "Completed" stays self-reported by explicit cut. |
| 8. Delivery, settlement | Widest gap | No shipped tag contains Full Auto. Confidence tiers exist only as prose. Settlement rails deliberately off. |

The pattern across all eight stages is consistent. The formats, laws, and
static machinery are real and deterministic. The dynamic layer — observed
obligations, independent admission at scale, verified artifacts in users'
hands, priced proof — is where the engine is not yet running.

### Stage detail, with evidence

**Intent is the strongest stage.** The spec corpus covers every major surface
with acceptance criteria: 52 for the Desktop trust workbench, 76 for Full
Auto, 31 for the web trust surface, 28 for mobile, 24 for Sarah, 18 for
sandboxes, 27 for authority delegation. The behavior-contract registry holds
about 90 contracts across 6 registry modules, with a closed state vocabulary
and enforcement tiers. The promise registry holds about 155 promise records
under a seven-state model with an eleven-gate transition stack.

**Proof design is nearly complete on paper.** Every AssuranceSpec obligation
in the corpus sits at `designed`, `mapped`, `needs_design`, or
`INCONCLUSIVE`. The Full Auto AssuranceSpec states its own boundary exactly:
"All 76 criteria now have complete proof designs, but a designed oracle is
not a passing observation." The grader in `assure-repo` never emits
`observed` or `accepted` — those require a passing sweep receipt or owner
acceptance, which do not yet exist.

**Admission is the live bottleneck.** The admission machinery is typed and
strict: `compileAssuranceManifest` refuses any spec that is not `admitted`,
digests must match exactly, and `producer_may_verify: true` is a hard error.
The independent-verifier mechanism landed and closed as QA-5 #8910 — no agent
accepts its own work. But exactly one AssuranceSpec has ever been admitted,
and no open issue tracks scaling the reviewer function itself. The
independence law exists as AD-AC-10 and as a gate inside #8978. The standing
capacity to satisfy it does not.

**Execution containment is close but unclosed.** SBX-00 froze the sandbox
authority and assurance contracts, SBX-01..08 landed, and the SBX-09
producer-run live matrix passed in full. Its aggregate deliberately records
`independentAssurance=INCONCLUSIVE` and blocks production, public claims, and
Phase 2 until an independent reviewer and the owner record dispositions. The
run-level authority-manifest / execution-receipt pair — the heart of stage 4 —
exists as typed machinery inside `packages/assurance-spec` and as 52-AC spec
intent, but it is absent from the Desktop run loop. The desktop app's own
README says execution receipts "remain experimental evidence, not general
support claims."

**Evidence moved fastest this week.** IDE-10 (terminal, tasks, tests, Output
with actor receipts), IDE-11 (supervised debug), and IDE-12 (safe SCM
mutation, worktrees, and delivery with exact-version receipts) are all
delivered per `docs/ide/ROADMAP.md`, IDE-12 on 2026-07-20 itself. That closes
the "tests and delivery are honest absences" gap the essay's Addendum I named
at the code-landed rung. The Sol receipt trail is active and dated, with RC21
through RC25 publications, the Full Auto owner-real acceptance receipt, and
the SBX evidence series.

**Verification has its engine but not its cadence.** Mutation testing landed
as the oracle-sensitivity engine (#8766) with typed receipts. The AR-2
false-green audit scanned 2,418 test files and holds 20 candidate leads,
explicitly "leads, not findings" until a surviving mutation demonstrates
each. AR-4 drift oracles run over 6 governed documents with 0 broken claims.
But the AR-3 standing sweep currently returns `overall=red` against the
moving tree, runs `degraded_terminal_observed`, and is not wired into the
real Full Auto run loop. The `readiness` command answers honestly: "no sweep
receipt (no receipt means no light)."

**Acceptance has exactly one full-scale precedent.** The MVP's 18 obligations
reached CONFIRMED with mutation receipts and independent review. Nothing at
surface scale has followed it yet. And the flagship's terminal state is
honest by explicit cut: Full Auto's spec defers automatic done-condition
verification, so "Completed remains a self-reported, owner-reviewable
disposition backed by the run report, not a verified-truth claim." No
successor spec restores that cut. The replay-verification thesis in
`docs/tassadar/` — verification whose cost collapses to re-running an exact
trace — has zero issue backing.

**Delivery and settlement are the widest gaps.** The release chain is
partially real: ReleaseSet v2, Gatekeeper oracles that fail closed, verified
`/download` resolution, and dated RC publications. But #8979's own audit
found that no Desktop tag contains any Full Auto commit, and DIST-04/12/13
plus REL-FEED-01 remain open. Confidence tiers (draft, verified, reviewed,
bonded) exist in specs and essays only — no typed tier state exists in any
TypeScript file, and no `bonded` literal appears in code. The public
proof-replay endpoint is archived and returns 410. The `nip90` market grammar
(labor lifecycle, bonds, content-addressed closeouts) is present, tested, and
deliberately settlement-free.

## III. The seven gaps, ranked

Each gap below names its narrowest honest statement and its current owning
authority. Four have owners. Three have none — those are the true holes.

**G1 — Independent admission, then independent capacity.** #8978 must
independently admit the Full Auto AssuranceSpec, and #8979 must bind that
proof to a signed packaged release with owner observation. Those two issues
are the entire remaining Wave 0 chain. Behind them sits the deeper gap: no
spec, issue, or budget owns a standing independent-reviewer function. Every
admission so far consumed ad hoc reviewer capacity. The essay's Addendum II
argues a market fills this at scale. Nothing in the repository yet models
even the internal version.

**G2 — A verified artifact in users' hands.** The engine has never shipped
its flagship through its own gate. Closing requires the DIST chain (#8917
runners, #8925 proof campaign, #8926 one-command release), REL-FEED-01's live
update feed, and #8979's admission evidence. Exit condition, already written
into the issues: one immutable signed candidate, owner-observed packaged
restart-resume, and promise flips only through registry transition gates.

**G3 — Run-level manifest/receipt pairing and typed confidence tiers.** Both
are fully spec'd (Desktop trust workbench, 52 ACs) and named in roadmap Wave
2, execution-order item 7. Neither has a minted issue. Until the pair exists
for every run, the essay's claim that "every consequential run leaves a pair
of records" describes the contract, not the product. Tiers are the same: a
visible Work Unit state in the spec, prose everywhere else.

**G4 — Machine-checked done conditions.** Explicitly cut from Full Auto
(CUT-FA-04), honestly labeled, and unowned by any successor. This is the
engine's acceptance stage failing open at the flagship: the run that works
while nobody watches ends in a state nobody verified. The natural design
already exists in-house — AR-3-style oracles adjudicating an objective, and
for replayable work classes, the `docs/tassadar/` exact-replay route.

**G5 — Public verifiability.** The web trust-surface spec holds 31 ACs
including a mechanical receipt-verification endpoint: a third party presents
a receipt and receives a pass or fail. No AssuranceSpec covers that spec, the
Observatory program closed without a continuation issue, and the archived
proof-replay route returns 410. Today the public surface serves receipt
projections, not verification. "The only place where an agent vendor's
claims can be checked instead of believed" remains a design.

**G6 — Standing verification operations.** The sweep is red and degraded, and
its full-fidelity preconditions (IDE-10, SBX-06) landed this week — the
wiring packet is now unblocked but unminted. Mutation evidence has no
freshness gate. The promise registry is 185 KB of disciplined prose rather
than typed, queryable rows, so state distribution is not machine-checkable.
And drift is visible in the governance surfaces themselves: issue #8978's
body still says 37 rev-9 obligations `needs_design` while the rev-6
AssuranceSpec and the receipts ledger record 76/76 designed at rev-14
binding. The roadmap's open-issue projection likewise lists IDE packets that
closed after revision 125 was cut. These are exactly the
unverified-operational-directive class, inside the tracker.

**G7 — Priced verification.** No ProductSpec anywhere owns priced confidence
tiers, bonded outcomes, verification markets, or settlement. The rails exist
and are deliberately inert: real Nostr identity live in Pylon, the NIP-LBR
bond and closeout grammar implemented settlement-free, wallet actions
returning `money_capability_retired`. This is correct sequencing — proof
machinery before priced proof — and it is also the widest distance between
the essay's economics and the inventory. Turning it on is an owner-gated
authority decision, not a research problem.

## IV. Roadmap

The ordering principle: close the loop once at full scale before widening it.
One surface (Full Auto) through all eight stages — admitted, contained,
observed, verified, accepted, shipped — is worth more than every surface at
the design rung. Existing authority already orders most of this. The rest is
named here as proposal packets that would need Sol admission.

**Now — close Wave 0 (owned: #8967, #8978, #8979, #9033, DIST chain).**
Execute #8978's independent admission of the 76-obligation AssuranceSpec.
Record the SBX-09 independent and owner dispositions. Finish #8917/#8925/#8926
and REL-FEED-01, then run #8979: one signed tag containing Full Auto, owner
observing packaged restart-resume, promises flipped only by their gates. Exit
evidence: the first non-MVP AssuranceSpec reaches `admitted`, and the first
verified flagship artifact reaches a user machine.

**Next — wire the standing sweep (owned in part: AR-3 follow-through).**
IDE-10 and SBX-06 are landed, so AR-3's full-fidelity step — host-observed
test evidence inside a Full Auto lane, receipts consumed by readiness — has
its technical preconditions met. Its remaining ordering gate is #8978, per
the execution order. This converts the inventory's 147 oracle refs from index
entries toward observations, and it is the cheapest way to make `observed`
a normal obligation state instead of a structural zero. Fixing the G6 drift
items (issue bodies, roadmap projection, IDE-09 scope line) belongs here
too, and AR-4 should own those checks going forward.

**Then — mint Wave 2 (owned as roadmap intent, item 7 of the execution
order).** Authority manifest / execution receipt on every run, the named
containment profile, the release trust ledger, routing disclosure, and
counter attestation, as bounded packets against the Desktop 52-AC and web
31-AC specs. Typed confidence tiers land here as Work Unit states — unpriced
at first, priced later under G7 authority.

**Proposed packets with no current owner (require admission before any
work).** Named in the spirit of ASSURE-REPO, which went from essay argument
to owner-admitted program in one day:

- **VERIFY-DONE.** Restore the done-condition cut as a separately admitted
  contract: typed objective oracles for autonomous runs, starting with
  repository objectives AR-3 can already adjudicate, and exact-replay
  verification for the work classes that admit it.
- **VERIFY-CAPACITY.** Specify the standing independent-review function:
  who may admit, how reviewer identity stays separate from producer
  identity, what the review cadence and budget are, and how community
  falsification enters through the promise/audit loop.
- **VERIFY-PUBLIC.** Serve the receipt-verification endpoint and the
  release/component trust ledger from the web spec, so a stranger can
  mechanically verify a receipt instead of reading a projection.
- **VERIFY-FRESH.** Make mutation and drift evidence a standing gate with
  staleness windows, so a green that was demonstrated once cannot silently
  become a green that is merely remembered.
- **VERIFY-SERVER.** Extend AssuranceSpec coverage to the server side: the
  Cloud Run monolith, the public counters that must reconcile to exact
  rows, and the promise-registry surface itself — including converting the
  registry to typed rows a machine can grade.
- **VERIFY-PRICED.** Last, and owner-gated throughout: the ProductSpec for
  priced tiers and bonded outcomes on the existing settlement-free grammar,
  activating rails only under `AUTHORITY.md` and the payment invariants.

**Update (2026-07-20, same day):** the owner admitted the phase 0-1 packets,
and they now have owning issues under epic VSE-00
[#9104](https://github.com/OpenAgentsInc/openagents/issues/9104):
VERIFY-FRESH plus the AR-3 full-fidelity wiring is VSE-01
[#9105](https://github.com/OpenAgentsInc/openagents/issues/9105), the
governance-drift reconciliation (G6) is VSE-02
[#9106](https://github.com/OpenAgentsInc/openagents/issues/9106),
VERIFY-CAPACITY is VSE-03
[#9108](https://github.com/OpenAgentsInc/openagents/issues/9108), and
VERIFY-DONE is VSE-04
[#9109](https://github.com/OpenAgentsInc/openagents/issues/9109).
VERIFY-PUBLIC, VERIFY-SERVER, and VERIFY-PRICED remain unminted by design,
in later phases. The same direction made Full Auto the first verifiable
mode: epic FAV-00
[#9110](https://github.com/OpenAgentsInc/openagents/issues/9110) with
packets #9111–#9114 connects the four-provider readiness flow
(Codex/Claude/Grok/Apple FM) to readiness-gated, at-capacity Full Auto,
per
[`2026-07-20-full-auto-first-verifiable-mode.md`](./2026-07-20-full-auto-first-verifiable-mode.md)
and roadmap revision 126, owner decision 30. That reorders this
document's roadmap in one respect: Full Auto is not just the first loop
through the eight stages, it is the declared bootstrap instrument for all
of them.

## V. Falsifiers — how we will know the engine runs

The thesis fails, or holds, on observable events. In rough order:

1. A second AssuranceSpec reaches `admitted` through a reviewer the producer
   did not choose, and its highest-risk obligations reach `observed` via
   source-bound receipts. (G1, #8978.)
2. A signed release whose tag contains Full Auto completes an owner-observed
   packaged restart-resume, and the promise registry flips through its own
   gates, never by narrative. (G2, #8979.)
3. The AR-3 sweep goes green against current `main` at full fidelity, and
   at least one obligation state transitions to `observed` from a sweep
   receipt rather than a human memory. (G6.)
4. A run detail view shows the authority manifest beside the execution
   receipt, and requested versus effective enforcement never merge. (G3.)
5. An autonomous run terminates with a done-condition verdict issued by an
   oracle, not by provider prose. (G4.)
6. A third party fetches a receipt and receives a mechanical pass or fail
   from the public surface. (G5.)
7. Someone pays for verification — a falsification bounty, a reviewed tier,
   a bonded outcome — and the receipt settles. (G7.)

Each event is binary, dated, and receipt-bearing. Until an event occurs, the
honest state of its gap is the state recorded here.

## VI. Boundary

This analysis is a snapshot of a repository moving at roughly 400 closed
issues per ten days. Its numbers decay quickly — several IDE packets closed
while the research for this document was running. That is not a caveat
against the analysis. It is the argument for G6: a repository this fast needs
its self-description checked by machines, which is what the drift oracles
exist to do. This document should be among the documents they check.
