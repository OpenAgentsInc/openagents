# Terminal-Bench issue review and experiment safeguards

Reviewed on September 24, 2026. The next selected issue was
[#9582](https://github.com/OpenAgentsInc/openagents/issues/9582), experiment
analysis and early stopping. Its first implementation landed in
`625b55f520` during this review. This follow-up keeps that implementation
and fixes the cost and restart behavior found while checking real records.

## Issue decisions

| Issue | Disposition | Remaining work or closure evidence |
| --- | --- | --- |
| [#9558](https://github.com/OpenAgentsInc/openagents/issues/9558), full TB4 suite | Keep open; full-suite work paused | The Luna pivot changes the next experiments. Targeted controller wins do not establish a general gain across the suite. |
| [#9569](https://github.com/OpenAgentsInc/openagents/issues/9569), effort routing | Keep open; further variants deferred | The 60% cost target is unproven, with one unpriced baseline call. Routed passes are 8/15 against xhigh's 10/14. |
| [#9570](https://github.com/OpenAgentsInc/openagents/issues/9570), persistence | Closed | Implementation, cheaper executor, spend and progress limits, and repeated v8 near-miss trials are complete. `production-planning` passed 2/3, satisfying the stated completion clause. This does not attribute those passes to persistence: the v10 trace audit attributes none to it. |
| [#9577](https://github.com/OpenAgentsInc/openagents/issues/9577), human marks | Keep open | CLI, TUI, and agreement measurements exist. Independent human labels are still missing; generated labels cannot replace them. |
| [#9582](https://github.com/OpenAgentsInc/openagents/issues/9582), experiment pulse | Completed, with this follow-up | Retain the CLI, TUI, Jev cache, and paired stopping rule. Seal restart policies and preserve unknown costs through display, scheduling, and replay. |
| [#9583](https://github.com/OpenAgentsInc/openagents/issues/9583), Luna baseline | Keep open | The 14-task, three-attempt comparison is running. It still needs its completed, graded report. |
| [#9584](https://github.com/OpenAgentsInc/openagents/issues/9584), truthful checks | Keep open | The held-out combined verdict improves failure recall to 22/60 from 6/60. Precision remains uncertain; new-task failures and Luna labels are still needed. |
| [#9585](https://github.com/OpenAgentsInc/openagents/issues/9585), Microluna | Keep open | The executor, Coder One integration, bounded session loop, and Codex-only arm have landed. The matched mini-task and TB4 comparisons are running. |
| [#9586](https://github.com/OpenAgentsInc/openagents/issues/9586), strategy fingerprints | Closed by the concurrent task | Phase classification, fingerprints, moves, replay integration, and the assessment have landed. The final report covers 505 subset trajectories and 419 Coder One runs. |
| [#9587](https://github.com/OpenAgentsInc/openagents/issues/9587), best-of-N Luna | Keep open | `control.best_of`, retained candidates, selection, and oracle reporting have landed. The matched single, best-of-3, and best-of-5 measurement is still pending. |

These are review-time states. The linked issues track later changes.

## What changed in the experiment system

**Cost coverage is part of the decision.** A trial with an unknown total
stays unknown even if Claude reports some spending. An arm enters the
cost-dominance test only when every graded trial has a whole-trial price.
This applies to the Python scheduler, the Rust pulse, and the historical
stopping replay. A partial sample mean could otherwise stop the wrong arm.

The pulse shows how many trials are unpriced. JSON returns `null` for an
incomplete arm's total and mean, and keeps its fully priced subtotal under
`priced_total_cost_usd`. Zero is a valid known price. Native Harbor totals
remain readable when there is no harness attempt record.

**Restarting preserves the experiment's design.** New experiments pin the
early-stopping flag, alpha, and acceptance bar alongside their arms and
tasks. Different settings need a new ID. Older experiments seal their last
recorded settings on restart. Experiments from before the feature, with no
settings recorded, keep stopping disabled. This also preserves experiments
that explicitly enabled the feature before policy pinning landed. Existing
ledger stops stay in force; raising the Claude quota is still allowed.

The statistical rule itself is unchanged. A decided result must survive
every remaining pair going the other way. A currently significant prefix
is insufficient. Multiple-arm comparisons still have no multiplicity
correction. Jev's live judgments remain advisory.

## What the real records exposed

The [validation record](2026-09-24-experiment-safeguards.json) covers four
retained status snapshots: 114 graded trials, including 31 `nop` controls
and 83 Coder One trials. It records input digests, per-arm prices and
coverage, stopping verdicts, pooled signals, and Jev cache measurements.
No benchmark trial was rerun or stopped for this review.

- **v10:** one `mvcc-lsm-compaction` call is unpriced. Three fully priced
  trials total $15.0410; the fourth has a $2.6603 lower bound. The arm
  therefore cost at least $17.7013, or $4.4253 per trial. Against v7's
  $5.7427, the saving is **at most 23%**. The previous unqualified saving
  and “no call was unpriced” statements were wrong.
- **v10 stopping:** four tied pairs leave eight open pairs. If all eight
  favor one arm, exact McNemar p is 0.0078125. Replay correctly keeps the
  experiment open. Its historical stop was an operator decision, not proof
  that significance was impossible.
- **Effort routing:** one xhigh Jev call is unpriced. Its total is a lower
  bound, so routed's 76% cost ratio is an upper bound. The recording does
  not establish the 60% target. The missing call would need roughly $15.4
  of extra charge to reach that target on the graded attempts, much more
  than the priced Jev calls; that is a sensitivity calculation, not a
  measured charge.
- **Checks:** of the 83 Coder One trials, “all passed” accompanies 19
  verifier passes and 19 failures. “Inconclusive” has 17 passes and 19
  failures, including six passing trials with no scenario. “A check
  failed” has four passes and five failures. These counts are observational;
  passing after a component ran does not establish a rescue by it.

The [v10 assessment](2026-09-24-persist-v10.md),
[effort report](2026-09-24-effort-routing.md), benchmark index, and root
README now qualify those claims.

## Validation

- All 248 benchmark Python tests pass; one is skipped. Regression cases
  cover new and legacy policy pinning, refused policy changes, unknown
  total costs, partial arm coverage, and known zero prices.
- Focused Rust pulse and Jev tests pass (nine), as do the stopping tests
  (five), including the new replay coverage regression.
- A live v10 pulse reused eight run-ranking answers and made five
  experiment requests: 6,687 input tokens, $0.000280854. Repeating it used
  cached answers and made no requests. These are judgments, not verifier
  outcomes or causal evidence.

The full manual Rust gate was run on `622a68bd32` with this work in
progress. Formatting, both strict Clippy configurations, dependency policy,
and PostgreSQL acceptance passed. Both workspace test phases failed at the
existing Coder One scratch-path assertion. The default phase also hit a
resume-timing assertion, which passed when run alone. The scoped Gym gate
found six default and eight TUI failures in the retained-run fixture,
recorded-answer, and learning-order tests: fixture task text differs and
three recorded Jev answers no longer match the loaded evidence. The pulse
TUI test passed. No affected assertion was removed or weakened.

The full gate is **not green**. Its local record is
`.coder/verification/20260924T074744Z-19911b/run.json`; the scoped test
record is `20260924T075332Z-bea6a4/run.json`. A subsequent formatting and
both-Clippy pass is recorded in `20260924T075605Z-b6a340/run.json`. These
checks cover their recorded trees and scopes, not subsequent unrelated
changes on main.

After integrating main through `678caf4198`, all 35 Terminal-Bench Rust
tests passed, including the pulse TUI, stopping replay, and Jev cache
tests. All 248 Python tests passed again, with one skipped. Formatting
and both strict Gym Clippy configurations passed; their scoped record is
`.coder/verification/20260924T080251Z-203592/run.json`. Documentation links
and the retained JSON also validate. This does not replace the failed
workspace-wide test result above.

For operation, use `gym experiment pulse ID`, add `--jev` for the cached
judgments, or press **w** in the Gym Terminal-Bench view. See the
[CLI guide](../gym/terminal-bench-cli.md#read-an-experiment-in-flight).
