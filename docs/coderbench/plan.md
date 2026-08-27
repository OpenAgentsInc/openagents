# CoderBench plan

Date: 2026-08-27. Owner: Christopher David. Companion documents:
[`terminal-bench-lessons.md`](terminal-bench-lessons.md),
[`process.md`](process.md). This is a proposal until its first milestone lands;
nothing here edits a suite, a threshold, or `bench-results/` by existing.

## 1. The claim

The best evidence of what a coding agent must do is the record of coding
agents being built. Every long session on this machine — Codex rollouts,
Claude sessions, and every `openagents coder` run's ATIF export — documents
real work in this repository: porting the TypeScript CLI to Rust, closing
forge issues with closing-reference commits, migrating a package graph,
removing a working-directory jail, fixing a PTY test suite. Those sessions
ended in checkable facts: commits on `main`, closed issues with references,
green gates. A benchmark built from them grades exactly the distribution this
project cares about — **agents building agents** — and it grows for free as
work continues.

CoderBench exists to turn that record into three durable assets:

1. **A trace corpus.** Every qualifying local session, redacted to public-safe
   ATIF v1.7, deduplicated by content digest, uploaded once, pinned forever.
   Public and sellable through the traces product; usable as optimizer fuel.
2. **A task suite.** Pinned, verifiable tasks distilled from trace outcomes:
   "repo at commit X, issue #Y open, make the gate green the way commit Z
   did" — with the historical diff held out of the context window and used
   only as the oracle.
3. **A graded lane** on the existing Gym rails: same Harbor adapter, same
   suite manifests, same thresholds, same append-only receipted store as
   tb2-quick and tb2-cross-section, so CoderBench numbers are comparable
   numbers.

## 2. Why this and not another public benchmark

- **Distribution match.** SWE-Bench samples GitHub issues; Terminal-Bench
  samples contrived terminal puzzles. Neither matches "port this module to
  Rust without breaking the PTY tests" or "close this issue so the tracker
  records the closing commit." The sessions that built Coder do.
- **Evidence match.** This repository already decided what done looks like:
  forge issues carry closing references, gates are named (`pnpm run check`,
  the interactive PTY suite), and outcomes were shipped under review. The
  benchmark reuses those oracles instead of inventing new ones.
- **Feedback loop closure.** The autoimprovement loop
  (`docs/coder/autoimprove.md`) needs measurable deltas on a stable suite.
  CoderBench makes the suite's tasks come from the same activity whose quality
  the loop is trying to raise, closing the loop on itself.
- **Traces are already the product.** Episode 275 names trace upload as a
  first-class surface ("visible and sellable"). The corpus is that surface,
  seeded rather than empty, and each benchmarked task cites the traces that
  generated it.

## 3. Non-goals

- **Not a training-data pipeline.** Traces inform task selection and act as
  demonstration material for the optimizer tier; nothing uploads model weights
  or fine-tunes from the corpus inside this project.
- **Not a leak-prone eval.** Tasks derived from an outcome must never ship the
  outcome's diff into the graded context. Where separation is impossible (the
  fix is fully described in the prompt), the task is disqualified.
- **Not a replacement for tb2.** Terminal-Bench stays the external yardstick
  and the cross-model comparison lane. CoderBench is the owned-distribution
  lane beside it; suite keys differ, rows never mix.
- **Not general-agent work.** Computer control, forum posts, wallet flows, and
  other Coder domains stay out of scope except where a session's coding
  outcome required them.

## 4. Architecture on existing rails

Everything hangs off machinery that already exists:

| Asset | Existing piece | CoderBench use |
| --- | --- | --- |
| Trace discovery | `openagents trace list/show` over `~/.codex/sessions`, `~/.claude/projects`, `~/.openagents/exports` | inventory all local sessions |
| Redaction | `packages/atif` redactor + tripwire | scrub every candidate trace |
| Upload + storage | `openagents trace upload --visibility <rung>` → `POST /api/v1/traces`, idempotent by digest | publish the corpus once |
| Task provenance | forge issues with closing-reference commits (`bench/tasks/owned/README.md` pattern) | pin each task to its outcome |
| Suites | `bench/suites/*.suite.json` via `openagents.effectiveness_suite.v1`, content-digest pins | `coderbench-v1.suite.json` |
| Runs | `bench/run-suite.sh` + `bench/adapters/openagents_coder.py` | grade the coder on the suite |
| Scoring | `packages/coder-effectiveness` thresholds + compare/report | floors, deltas, smoke rejection |
| Receipts | `bench-results/coderbench-v1.jsonl` hash chain | append-only trend line |

New code is limited to: a curation CLI that walks the inventory → redact →
upload → register pipeline and writes the ledger; a distiller that turns a
labeled trace cluster into a draft task JSON plus its verifier script; and a
task-environment image per task kind (start repo state + gate runner).

## 5. Milestones

Milestones land one PR each, in order; each leaves the tree green and the docs
honest.

### M0 — Corpus inventory (no upload)

Walk all three local stores; emit `docs/coderbench/inventory.json`: one row
per session with source kind, path, size, step count estimate, timestamps,
model if stated, and a content digest. Estimate: ~4,000 candidates, 35+ GB
raw, of which far fewer qualify (see §6 filters). Deliverable also includes
the qualification report: how many pass each filter and why the rest drop.

Exit: inventory reproducible from a single command; two independent runs
diff-clean apart from mtime fields.

### M1 — First hundred traces uploaded

Batch pipeline: qualify → convert → redact (`trace redact`) → tripwire →
upload at `ledger` visibility (content + metadata) with `pulse` reserved for
sessions too sensitive even after redaction and `dark` never used (a trace
that cannot be published has no benchmark value). Respect the server rate
limit (120/hour/owner); idempotent re-runs make retries free. Emit
`corpus.jsonl` mapping digest → trace uuid → source session → qualified-task
seeds found inside.

Exit: ≥100 traces fetchable at their `/trace/{uuid}` URLs, zero redaction
incidents reported post-upload, corpus ledger committed.

### M2 — Ten labeled outcomes

Hand-label ten completed sessions against the forge: which issue(s) they
closed, which commits landed, whether the outcome is gradeable (env buildable,
gate runnable, no secrets needed). Produce the label schema — outcome type,
oracle command, holdout policy — as `labels.schema.json` with the ten instances
filled. This milestone is deliberately manual: labeling discipline decided now
prevents a thousand mislabeled rows later.

Exit: ten labels reviewed by the owner; schema committed.

### M3 — Distiller v0

From a labeled session, generate a draft task: start state pin (base commit),
instruction text (rebuilt from the user turns, not copied verbatim when the
original quotes the fix), oracle command (the real gate), and metadata linking
every source trace uuid. Human reviews every draft; the distiller never opens
a PR itself (optimizer law applies: output is a candidate, not a deployment).

Exit: ≥7 of 10 drafts survive human review into `bench/tasks/coderbench/`.

### M4 — coderbench-v1 suite recorded

Promote the surviving tasks into `bench/suites/coderbench-v1.suite.json`
(content-digest pins, `environmentAvailable` set only where the environment
builds), record the runbook section for the lane, and put the first smoke-tier
results row into `bench-results/` through the normal tooling — clearly marked
smoke until the environments are verified on amd64, exactly as the
owned-closed-issues suite did.

Exit: suite manifest passes `--check`; one real Harbor run executed end to
end; store accepted the row.

### M5 — Score tier + baseline rows

Verify every task's environment and oracle determinism (same commit + same
binary → same verdict three runs running), promote the suite to score tier
with threshold floors set from the first honest baseline, then record
baselines for the Flash configuration the owner uses day to day, per issue
#143's native-baseline discipline. From here the autoimprovement cycle can
target CoderBench deltas like any other lane.

Exit: baseline rows chained in `bench-results/`; the improvement loop's
runbook mentions the lane.

## 6. Qualification filters (what makes a trace corpus-worthy)

A session enters the corpus only if all hold:

1. **Coding outcome.** It touched this or a companion repository's source with
   intent to land (not exploratory chats, not failed experiments unless the
   failure analysis itself is the artifact).
2. **Checkable ending.** It maps to a landing commit, a closed issue with a
   closing reference, or an explicit rejection worth keeping.
3. **Substance.** ≥10 steps or ≥15 minutes wall clock; trivial Q&A adds noise,
   not signal.
4. **Redactable.** The ATIF redactor + tripwire pass with margin; anything
   needing hand-holding beyond one reviewed exception waits.
5. **Derebatable length.** Under the ingest cap after conversion; huge
   rollouts are split at natural user-turn boundaries into linked segments
   (ATIF `continued_trajectory_ref` semantics).
6. **Ownership.** Work done on other people's private repos in this machine's
   caches is excluded unless the repo is public — the corpus is publishable
   content, and import does not grant publication rights.

The filters err toward exclusion. A small clean corpus beats a large ambiguous
one; every exclusion reason is counted in the inventory report so the
boundaries stay visible and arguable.

## 7. Metrics and success criteria

For the benchmark itself, track:

- **Corpus**: count of uploaded traces, total steps captured, GB raw vs.
  stored, dedup ratio, incident count (tripwire catches post-redaction = 0
  tolerance).
- **Task yield**: labeled outcomes per 100 uploaded traces; drafted tasks per
  100; tasks surviving review per 100. Expect steep attrition; the numbers say
  whether the funnel is viable.
- **Gradeability**: fraction of surviving tasks whose verifier reproduces
  (deterministic oracle) and whose environment builds clean on amd64.
- **Discriminative power** (after M5): does the suite separate configurations?
  A suite where Flash scores ≈ a degraded 0.6B local model scores is not
  measuring anything. tb2-quick already demonstrated separation (50% vs 0%
  with a failing gate); CoderBench must show the same before trusting trends.

For the improved coder (the actual point): CoderBench delta per autoimprove
cycle, cross-checked against tb2 deltas so in-distribution gains are not just
overfitting to home soil. A lever that lifts CoderBench while flat-lining tb2
gets a skeptical review, not a celebration.

## 8. Risks and standing guards

- **Leakage.** The strongest risk. Guard: instruction text rebuilt, diffs
  held out, tasks cite but never embed their answers; spot-audit each batch.
- **Overfit-to-self.** Improving on tasks drawn from your own history may
  optimize quirks, not competence. Guard: always report the paired tb2 number;
  retire any practice whose only evidence is home-soil delta (best practice
  V1 applied to levers).
- **Privacy of third parties.** Sessions mention people, emails, client repos.
  Guard: filter 6, redactor categories, tripwire, and the visibility ladder
  defaulting conservative; `glass` requires an explicit per-trace decision,
  never a batch flag.
- **Tracker circularity.** If tasks close issues, and closing issues trains
  the agent that closes issues, the ledger could drift into self-flattery.
  Guard: scores enter only through Harbor runs on pinned states; nobody grades
  their own working tree.
- **Scale illusions.** 33 GB of Codex rollouts sounds like a dataset; most of
  it is not eligible. Publish the funnel numbers so nobody (including future
  us) mistakes volume for corpus.

## 9. Open questions

1. Do non-openagents sessions (raw Claude/Codex work in other clones)
   contribute tasks, or only corpus? (Current lean: corpus yes, tasks only
   from openagents-repo work where the oracle is ours.)
2. Should upgraded traces backfill older suites' analyses (e.g. wasted-reexec
   study from issue-cluster #152–#158)? Probably yes, note-only, never
   rewriting recorded results.
3. Subagent trajectories: ATIF v1.7 supports embedded subagent arrays; do we
   flatten delegate/fleet children into first-class corpus entries or keep
   them nested? Decide at M1 with real data.
4. Selling access: does the corpus go `ledger` everywhere uniformly, or does
   the marketplace need finer-grained licensing fields? Deferred to the
   traces-product owners; the corpus stores provenance enough to re-license
   later.
