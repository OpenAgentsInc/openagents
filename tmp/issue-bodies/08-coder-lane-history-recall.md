## Problem

The RLM integration program (epic per `docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md`) made chat/desktop/full-auto history addressable: deterministic corpus export (`@openagentsinc/history-corpus`), Tier D recall without model calls, Tier S behind the sandboxed leaf, host tools wired in `omega-effectd`. The one surface still excluded is the **coder lane** (`docs/coder/2026-08-26-dspy-gepa-coder-optimization.md`: "RLM stays out of scope for the coder lane entirely").

Trajectory `2026-08-27T12:29:02` shows what that exclusion costs. A single coding session burned ~5–7 of 22 minutes re-executing suites solely to re-read output it had already received but could not address again:

- Step 48 ran the full monorepo suite (152s) piped to `tail -8`; the failure names were truncated away.
- Step 49 re-ran the identical 150s suite just to recover those names.
- Step 55 executed `pnpm run test:rust` 3× in one invocation for three different greps.

Root cause stated in RLM terms: the coder treats its own session as a **bounded window** instead of an **environment object**. Once a tool result scrolls out of reliable reach, the model's only way back to the facts is to pay to regenerate them. This is the exact failure class the RLM paper's prompt-as-environment invariant eliminates, and the repo already owns every component needed to fix it deterministically.

## Recommendation

Extend the existing RLM architecture to the coder lane — **Tier D only** (deterministic, zero model calls, no Python leaf process, no recursion). This amends the 2026-08-26 coder-lane exclusion narrowly: what is being admitted is the deterministic reader over already-persisted session artifacts, not the REPL loop and not semantic fan-out. It follows the standing hybrid decision and the audit's "depth 0 is already valuable" finding.

Stage 0 — artifacts exist to be addressed (prereq, #152):
- Long-running commands persist full output to `$SESSION_DIR/cmd-N.log`; structured reports per #155 land beside them.

Stage 1 — run-scoped corpus:
- Reuse the `HistoryCorpus` builder contract over the coder run's durable event log plus the `$SESSION_DIR` artifacts. Cursor-addressed entries; manifest records counts/ranges/exclusions. Pure addition, same shape as RLM-01.

Stage 2 — deterministic recall service + host tool:
- `HistoryRecall`-shaped service (grep, cursor/time slicing, key-step extraction, structural summaries — zero model calls) exposed as a `history_recall`-style host tool on coder turns.
- Budget caps enforced even for deterministic work (corpus size, span count); responses carry cited cursor spans and an honesty field (`complete` / `partial_budget` / `refused`); tool call/result re-enter the neutral stream like every other coder action.
- Model-facing rule pairs with #154: recovering details of past output means calling recall against the corpus — never re-executing the command.

Deferred (explicitly NOT this issue): Tier S semantic escalation (needs the RLM-07 admission gate evidence path), the Python leaf executor in the coder lane, recursion above depth 0, any public long-context claim.

## Why this lands cheap

- Corpus builder, recall vocabulary, caps, citation discipline, honesty fields, host-tool registration, and neutral-stream re-entry all exist from RLM-01..03 and are consumption-stable published trains (0.2.1-rc.4).
- The waste telemetry motivating it is measurable (#156/#157 give exporter visibility), so delta can be proven: replay-shaped sessions should show mid-loop duplicate-head executions dropping to near zero once recall answers failure-name questions (#153 kills the rest mechanically).

## Acceptance criteria

- [ ] Coder run corpora build deterministically from the run event log + `$SESSION_DIR` artifacts, cursor-addressed, with manifest exclusions recorded.
- [ ] Deterministic recall host tool available on coder turns; answers structural/lexical questions over past tool output with cited spans and zero model calls; capped and honest on truncation.
- [ ] Trajectory replay of the step-48/49 shape resolves the second question via recall with no suite execution.
- [ ] `pnpm run check` green; no change to spend, network, or repository policy surfaces (recall reads owner-local stores only).
