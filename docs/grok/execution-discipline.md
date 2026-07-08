# Execution discipline

Date: 2026-07-08
Status: orientation
Sources: EXECUTION.md, ROADMAP.md, ROADMAP_QA.md, fleet fan-out, PROMISSORY
runbook, MASTER_ROADMAP testing notes

## Why this matters

OpenAgents sells agents that do real work with proof. The monorepo
delivery system is supposed to be a continuous dogfood of that claim:
multi-agent fleet, receipts, reviews, and public counters.

## Default delivery loop

From EXECUTION.md (compressed):

1. **One GitHub issue** per roadmap task.
2. **Clean worktree** from current `origin/main` when the main checkout
   is dirty or contested.
3. **One PR → merge to `main`** closes the issue — not branch theater.
4. **Final review** by supervisor or tightly controlled reviewer — not
   the authoring worker alone.
5. **Token/usage verification** through the exact chain into public
   counters when the work claims fleet/metered usage.
6. **Fleet bugs found in-flight** are first-class work, not background
   noise.

## Sequencing authorities

| Question | Authority |
| --- | --- |
| What ships next at company level? | `MASTER_ROADMAP.md` |
| Desktop/harness task graph? | `ROADMAP.md` (content) |
| QA engine tasks? | `ROADMAP_QA.md` |
| Business funnel tasks? | `ROADMAP_BIZ.md` |
| How to run a promise assault? | PROMISSORY runbook |

When documents disagree on order, **MASTER_ROADMAP wins**; file issues
against its phase lanes.

## Parallelization rules of thumb

- Land small foundations first (typed contracts, one orchestration state
  model) before wide fan-out.
- Workstreams are the parallel unit; tasks are the issue unit.
- Hard deps mean do not start; soft-after means preferred order.
- Delegability grades (HIGH/MED/LOW) exist so fleets don't eat judgment-
  heavy seams unsupervised.

## QA is continuous

- Nightly matrix, visual baselines, perf budgets, seam probes.
- Mobile gate (`qa:mobile:gate` class) blocks bad screens.
- Behavior contracts run in the normal sweep.
- Explorer/monkey modes need coverage ledgers so chaos is accounted.
- Customer #1 for QA Swarm productization is Khala Code itself.

## Claim discipline during execution

- Fable docs analyze; they do not green promises.
- PROMISSORY assaults prepare evidence; owner flips green.
- Public copy and App Store metadata stay behind promises/copy passes.
- Label modeled vs measured in any operator-facing metric.

## Anti-patterns (seen historically in the corpus)

| Anti-pattern | Replacement |
| --- | --- |
| Duplicate PRs / unclaimed parallelism | Claim registry + typed planner |
| Trust-the-summary completion | Typed lifecycle events + verify |
| PTY heuristics as truth | Runner-neutral status contracts |
| Big-bang UI rewrites mid-ship | No-new-legacy + critical-path conversion |
| Closing issues without main merge | PR-only close |
| Author self-merge of high-blast work | Independent review |

## Grok operating note

When implementing from fable:

1. Read MASTER_ROADMAP phase status (may be stale — check issues).
2. Prefer clean worktree if monorepo checkout is dirty.
3. Keep scope to one issue; do not "helpfully" expand epic surface.
4. Leave receipts (tests, docs, evidence paths) the next agent can
   dereference.
5. Append learnings to `docs/grok/` only when analysis — not as a
   substitute for the owning roadmap.
