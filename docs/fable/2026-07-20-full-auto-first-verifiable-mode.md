# Full Auto, Reimagined: The First Verifiable Mode

**Date:** 2026-07-20
**Lane:** Fable design synthesis (owner-directed)
**Status:** Design connecting Full Auto to the four-provider readiness flow,
per owner direction 2026-07-20. This document reimagines the mode and argues
its bootstrap role. Dispatch authority lives in the minted issues (#9110
FAV-00 epic, #9111–#9114) and the existing Full Auto chain (#8967, #8978,
#8979). Factual status authorities remain current code,
`docs/sol/MASTER_ROADMAP.md`, live issue state, and receipts.
**Companions:**
[`2026-07-19-verifiable-software.md`](./2026-07-19-verifiable-software.md)
(the thesis) and
[`2026-07-20-verifiable-software-engine-gap-analysis-and-roadmap.md`](./2026-07-20-verifiable-software-engine-gap-analysis-and-roadmap.md)
(the gap analysis this design executes against).

---

## I. The owner direction

Verbatim intent, 2026-07-20: full auto mode working ASAP, extending the
newly set up process that first checks for Codex, Claude, Grok, and Apple
FM. Those first agents at capacity in full auto mode. Full Auto delivered
as the first verifiable mode, the one that bootstraps the rest of the
roadmap and all envisioned functionality.

This document turns that direction into a coherent design over what the
repository already holds, and names exactly what is new.

## II. What already connects

The two halves of this design already read the same truth. The Desktop
BOOT SEQUENCE probes the four agents in order — Codex, Claude Code, Grok,
Apple FM — and renders `checking`, `available`, or `unavailable` per row,
plus sovereign identity and wallet rows. Its law is already the essay's
law: "an agent is 'available' only when its lane reports it can actually
run a turn." The probes are real: Codex runs a bounded session probe per
account, Apple FM probes the native bridge and runs one bounded test
inference.

Full Auto's routing gate reads the same substrate — the harness lanes and
lane capability reports. Its ordered routing policy admits up to eight
candidates over the four action lanes that exist today: `codex-local`,
`claude-local`, `acp:grok-cli`, and `acp:cursor-agent`. Admission is
fail-closed: "the FIRST refusal wins and the whole policy is refused —
candidates are never silently filtered." Rotation happens only on typed
reasons (`account_exhausted`, `rate_limited`, `provider_error`), and the
non-overridable guardrail core enforces own-capacity-only dispatch.

So the reimagining is not a rewrite. It is a binding: the readiness scan
and the run loop become one visible system, and the run inherits the
scan's honesty.

## III. The design, in five moves

**1. Readiness gates the run (FAV-01, #9111).** The routing-policy
composer shows live per-lane readiness from the same projection the boot
scan renders. A run binds a readiness snapshot at start — which lanes
were ready, which were not, and why — and the run report carries it. A
not-ready candidate is a visible typed refusal or skip, never a silent
drop. The scan set and the lane set reconcile: every Full Auto-eligible
lane gets a scan row, and Apple FM's row carries an advisory-only marker.

**2. Four action lanes at rotation parity (FAV-02, #9112).** The proven
Codex↔Claude owner-real rows extend to Grok and Cursor: same ordered
policy, same typed failover, same handoff envelope with disclosed
truncation, same bounded public-safe rotation history. Four lanes, one
contract.

**3. Apple FM at capacity in its admitted role (FAV-03, #9113).** The
standing authority is explicit: "A local model can recommend a route or
produce an advisory result. Deterministic policy must make the route
decision. Existing host services must perform all actions." Inside Full
Auto, Apple FM therefore runs at capacity as the advisory layer: on-device
route recommendations that inform (never decide) the deterministic gate,
and bounded read-only analysis of run progress, recorded as clearly
labeled advisory evidence. Recommendation and decision are distinct
recorded facts. Zero marginal cloud cost, zero action authority.

**4. Capacity becomes typed truth (FAV-04, #9114).** "At capacity" gets a
ledger: per-lane capacity states (ready, busy, exhausted, cooling)
derived from probes, quota signals, and rotation history. On top of it,
an explicit ProductSpec revision defines owner-granted bounded
concurrency — multiple runs across distinct ready lanes and accounts,
non-overlapping claims, guardrail core unchanged. The shipped
single-active-run contract stays true until that revision is admitted.
This is the owner-directed re-gating path for what the roadmap deferred
as Wave-3 portfolios, taken as its own spec revision rather than a silent
unlock.

**5. The run verifies, and is verified (the VSE bridge).** Full Auto is
the first verifiable mode only if its own chain closes: #8978 independent
AssuranceSpec admission, #8979 signed packaged release with owner
observation. The VSE groundwork (#9104) then makes the mode
self-strengthening: the standing repo sweep runs as a Full Auto lane
(#9105), drift repair becomes standing work (#9106), the
independent-review function gets specified so admissions have capacity
(#9108), and done-condition oracles give autonomous runs verified
terminal states where objectives admit one (#9109).

## IV. Why Full Auto bootstraps everything else

The gap analysis found the engine's pattern: static machinery strong,
dynamic layer missing. Every dynamic gap needs the same thing — something
that runs continuously, produces receipts, and can be trusted while
nobody watches. That is Full Auto's exact definition.

Concretely, the bootstrap loop: Full Auto runs the AR-3 sweep, which
produces the first `observed` obligations. Those observations feed #8978's
admission evidence. Admission plus the DIST chain produces the first
signed release containing Full Auto — the first verified artifact in a
user's hands. That release runs more Full Auto, at four-lane capacity,
which executes the remaining roadmap packets as verifiable runs. Each run
leaves readiness snapshots, routing records, rotation history, receipts,
and (per #9109) oracle-issued done verdicts. The mode that ships first is
the mode that verifies the rest into existence.

This is also why the four-provider readiness flow matters beyond UX. The
scan is the supply-side truth of the machine-work thesis at desktop
scale: what capacity is actually available right now, checked, not
assumed. Binding it to the run loop means every unit of autonomous work
starts from verified capacity and ends in a receipt — accepted outcomes
per unit of available capacity, measured honestly, on one machine first.

## V. Boundaries that hold

- Apple FM gains no action authority. Its capacity is advisory capacity.
- The owner-ordered grant stays the routing law. Readiness gates and
  informs the order. It never lets the loop choose providers on its own.
- Concurrency arrives only through the FAV-04 spec revision with the
  guardrail core intact. Until then, one active run remains the truth.
- No promise flips, public claims, or release assertions come from this
  design. #8978 and #8979 remain the only path to those.

## VI. Status, honestly

As of this writing: the boot scan and the four action lanes exist and
read one substrate. The Codex↔Claude rotation is owner-real proven. Grok
and Cursor are eligible but unproven in the matrix. Apple FM probes and
test-infers but has no Full Auto integration. Capacity is sequential.
The readiness-to-run binding, the ledger, the advisory layer, and the
concurrency revision are the new work, minted as #9111–#9114 under epic
#9110. The verifiable-mode claim itself remains gated on #8978 and #8979,
exactly as before.
