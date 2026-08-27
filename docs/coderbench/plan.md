# CoderBench plan

Date: 2026-08-27. Owner: Christopher David. Companion documents:
[`terminal-bench-lessons.md`](terminal-bench-lessons.md),
[`process.md`](process.md). This is a proposal until its first milestone lands;
nothing here edits a suite, a threshold, or `bench-results/` by existing.

## 1. The claim

The best evidence of what a coding agent must do is the record of coding work
actually done. Every long session on this machine — Codex rollouts, Claude
sessions, and every `openagents coder` run's ATIF export — documents real
engineering: porting the TypeScript CLI to Rust, closing forge issues with
closing-reference commits, migrating a package graph, removing a
working-directory jail, fixing a PTY test suite, and beyond this repository,
entire projects carried in other clones. Those sessions ended in checkable
facts: commits on a default branch, closed issues with references, green
gates. A benchmark built from them grades the real distribution of coding-agent
work — **general coding capability, measured where it actually happens** — and
it grows for free as work continues.

**Building coding agents is the first domain, not the boundary.** It is
simply the deepest slice of trace history here. The corpus and the task
generator are domain-tagged from day one so that work on any repository with
checkable outcomes — a protocol implementation, a firmware audit, a data
pipeline, a game engine — enters the same funnel under its own domain label.
The definition of CoderBench is the method, not the subject matter:

> a benchmark whose tasks are distilled from real sessions with real
> outcomes, covering whatever coding work its corpus honestly contains.

CoderBench exists to turn that record into three durable assets:

1. **A trace corpus.** Every qualifying local session, redacted to public-safe
   ATIF v1.7, deduplicated by content digest, domain-tagged, uploaded once,
   pinned forever. Public and sellable through the traces product; usable as
   optimizer fuel.
2. **A task suite.** Pinned, verifiable tasks distilled from trace outcomes:
   "repo at commit X, issue #Y open, make the gate green the way commit Z
   did" — with the historical diff held out of the context window and used
   only as the oracle. Suites are selected per domain or across domains.
3. **A graded lane** on the existing Gym rails: same Harbor adapter, same
   suite manifests, same thresholds, same append-only receipted store as
   tb2-quick and tb2-cross-section, so CoderBench numbers are comparable
   numbers.

## 2. Why this and not another public benchmark

- **Distribution match.** SWE-Bench samples GitHub issues; Terminal-Bench
  samples contrived terminal puzzles. Neither matches "port this module to
  Rust without breaking the PTY tests," "audit this firmware image and find
  the entropy flaw," or "close this issue so the tracker records the closing
  commit." Real sessions on real repositories do.
- **Evidence match.** This repository already decided what done looks like:
  forge issues carry closing references, gates are named (`pnpm run check`,
  the interactive PTY suite), and outcomes were shipped under review. The
  benchmark reuses those oracles instead of inventing new ones.
- **Feedback loop closure.** The autoimprovement loop
  (`docs/coder/autoimprove.md`) needs measurable deltas on a stable suite.
  CoderBench makes the suite's tasks come from the same activity whose quality
  the loop is trying to raise, closing the loop on itself — and per-domain
  rows keep that closure from collapsing into self-measurement only.
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
- **Not agent-building-only.** The first corpus slice happens to be building
  coding agents; nothing in the method depends on it. What makes a task
  CoderBench is a real coding session with a checkable outcome — the repo it
  touched is irrelevant. The only hard requirement is that the oracle belongs
  to the task's own repository and runs without OpenAgents-specific
  infrastructure.

## 4. Architecture on existing rails

Everything hangs off machinery that already exists:

| Asset | Existing piece | CoderBench use |
| --- | --- | --- |
| Trace discovery | `openagents trace list/show` over `~/.codex/sessions`, `~/.claude/projects`, `~/.openagents/exports` | inventory all local sessions |
| Redaction | `packages/atif` redactor + tripwire | scrub every candidate trace |
| Upload + storage | `openagents trace upload --visibility <rung>` → `POST /api/v1/traces`, idempotent by digest | publish the corpus once |
| Task provenance | forge issues with closing-reference commits (`bench/tasks/owned/README.md` pattern) | pin each task to its outcome |
| Suites | `bench/suites/*.suite.json` via `openagents.effectiveness_suite.v1`, content-digest pins | domain-tagged `coderbench-*.suite.json` |
| Runs | `bench/run-suite.sh` + `bench/adapters/openagents_coder.py` | grade the coder on the suite |
| Scoring | `packages/coder-effectiveness` thresholds + compare/report | floors, deltas, smoke rejection |
| Receipts | `bench-results/coderbench-<domain>-v1.jsonl` hash chains | append-only trend lines |

New code is limited to: a curation CLI that walks the inventory → redact →
upload → register pipeline and writes the ledger; a distiller that turns a
labeled trace cluster into a draft task JSON plus its verifier script; and a
small set of parameterized task-environment image families (one per
environment shape, not per task — see §5 M4).

## 5. Domains

A **domain** is a repository (or repository family) plus its gate vocabulary.
Domains are declared, not discovered implicitly, so suite composition and
cross-domain reporting stay honest:

```
{ domain: "agent-building",
  repos: ["OpenAgentsInc/openagents"],
  oracles: ["pnpm run check", "cargo test -p ..."],
  first_trace: "2026-06-…" }

{ domain: "firmware-security",
  repos: ["coldcard-firmware", "coldcard-rng-postmortem"],
  oracles: ["make repro", "pytest tests/"],
  first_trace: "2026-07-…" }
```

The first seeded domain is **agent-building** (this repository's history).
Others enter as their traces qualify: the ~270 other repositories on this
machine are candidate domains — the Coldcard firmware and RNG postmortem work,
the Effect ecosystem clones, the 3D/graphics projects, protocol
implementations — each admitted only when its sessions pass the same filters
and its gates run deterministically in a container. Nothing about the
pipeline is agent-building-specific; that domain is simply first in line
because its evidence is deepest.

Domain rules:

- Suites may be single-domain (`coderbench-agent-building-v1`) or
  cross-domain; a cross-domain suite must sample every included domain
  proportionally to its labeled pool, so no domain's quirks dominate.
- Scores never mix across suite keys (existing law). Per-domain rows are the
  default view; cross-domain aggregates are derived, never recorded.
- A domain whose oracle determinism cannot be proven stays at smoke tier
  forever or is dropped — no exceptions for interesting subjects.

## 6. Milestones

Milestones land one PR each, in order; each leaves the tree green and the docs
honest. M0–M5 cover the first domain (agent-building) end to end; later
domains re-enter at M1 with their own traces and skip nothing.

### M0 — Corpus inventory (no upload)

Walk all three local stores; emit `docs/coderbench/inventory.json`: one row
per session with source kind, path, size, step count estimate, timestamps,
model if stated, repo hint, domain tag, and a content digest. Estimate:
~4,000 candidates, 35+ GB raw, of which far fewer qualify (see §7 filters).
Deliverable also includes the qualification report: how many pass each filter
and why the rest drop.

Exit: inventory reproducible from a single command; two independent runs
diff-clean apart from mtime fields.

### M1 — First hundred traces uploaded

Batch pipeline: qualify → convert → redact (`trace redact`) → tripwire →
upload at `ledger` visibility (content + metadata) with `pulse` reserved for
sessions too sensitive even after redaction and `dark` never used (a trace
that cannot be published has no benchmark value). Respect the server rate
limit (120/hour/owner); idempotent re-runs make retries free. Emit
`corpus.jsonl` mapping digest → trace uuid → source session → domain →
qualified-task seeds found inside.

Exit: ≥100 traces fetchable at their `/trace/{uuid}` URLs, zero redaction
incidents reported post-upload, corpus ledger committed.

### M2 — Ten labeled outcomes

Hand-label ten completed sessions — all from the first domain — against the
forge: which issue(s) they closed, which commits landed, whether the outcome
is gradeable (env buildable, gate runnable, no secrets needed). Produce the
label schema — outcome type, oracle command, holdout policy — as
`labels.schema.json` with the ten instances filled. This milestone is
deliberately manual: labeling discipline decided now prevents a thousand
mislabeled rows later.

Exit: ten labels reviewed by the owner; schema committed.

### M3 — Distiller v0

From a labeled session, generate a draft task: start state pin (base commit),
instruction text (rebuilt from the user turns, not copied verbatim when the
original quotes the fix), oracle command (the real gate), and metadata linking
every source trace uuid. Human reviews every draft; the distiller never opens
a PR itself (optimizer law applies: output is a candidate, not a deployment).

Exit: ≥7 of 10 drafts survive human review into `bench/tasks/coderbench/`.

### M4 — First suite recorded

Promote the surviving tasks into
`bench/suites/coderbench-agent-building-v1.suite.json` (content-digest pins,
`environmentAvailable` set only where the environment builds), record the
runbook section for the lane, and put the first smoke-tier results row into
`bench-results/` through the normal tooling — clearly marked smoke until the
environments are verified on amd64, exactly as the owned-closed-issues suite
did. Environments amortize by image family: one parameterized build per
toolchain shape (pnpm+cargo, pytest, make), tasks pinning only commit and
oracle.

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

### M6 — Second domain admitted

Run a second repository's qualified traces through the same funnel — label,
distill, review, environment family, determinism burn-in — and record its own
suite beside the first. This milestone exists to prove the method is general
before scaling within any one domain: if distillation only works on
OpenAgents commits, CoderBench is a private eval wearing a general name.

Exit: a second domain's suite recorded; a cross-domain report comparing
per-domain rows generated and committed.

## 7. Qualification filters (what makes a trace corpus-worthy)

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

## 8. Metrics and success criteria

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

Generality is itself a metric once M6 lands: **domain transfer** — does a
lever that lifts the first domain lift the second, or only home soil? Per-
domain rows make this visible directly; report deltas per domain and treat a
one-domain-only improvement as suspect.

## 9. Risks and standing guards

- **Leakage.** The strongest risk. Guard: instruction text rebuilt, diffs
  held out, tasks cite but never embed their answers; spot-audit each batch.
- **Overfit-to-self.** Improving on tasks drawn from your own history may
  optimize quirks, not competence. Guard: always report the paired tb2 number;
  after M6, report per-domain deltas; retire any practice whose only evidence
  is home-soil delta (best practice V1 applied to levers).
- **Generality theater.** Declaring many domains before any but the first has
  cleared determinism burn-in makes the benchmark look broad while measuring
  one repo. Guard: a domain counts when it has a score-tier suite, not when it
  appears in a diagram; the M6 exit criterion is a second domain's recorded
  rows, nothing softer.
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

## 10. Open questions

1. Which repository becomes the second domain at M6? Candidates visible on
   this machine: a protocol implementation with a conformance suite, a
   firmware project with reproducible builds, or any clone whose gates run
   clean in a container. Decide at M5 with the funnel numbers in hand.
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
5. Cross-domain aggregation: pure per-domain reporting is the default, but a
   single headline number will eventually be asked for. Decide only when two
   or more score-tier domains exist, and weight by labeled-pool size, never
   equally by domain.
