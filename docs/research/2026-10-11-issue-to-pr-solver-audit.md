# The issue-to-PR solver, audited as a workflow: what code should own, what judgment should own, what the model should keep

Date: 2026-10-11. This audit reads the owner's longest Claude Code session
against the solver we built inside it, and asks which parts of the observed
issue-to-PR loop should move out of a model's context and into the harness.

Companion documents: the [AgentRun
assessment](2026-10-11-agentrun-assessment.md), [Coder as a tunable
system](../optimization/coder-components.md), [the solver
spec](../coder/solver.md), [today's solver
handoff](../coder/2026-10-11-solver-handoff.md), and the
[calibration audit](typesafe/2026-10-03-calibration.md).

## Sources

- **The trace.** `~/.claude/projects/-Users-christopherdavid-work-openagents/
  3ba359c8-…-e591e.jsonl`: 96 MB, 29,495 records, 2026-10-08 22:05 through
  2026-10-11 21:25, plus 827 subagent transcripts totalling about 1.9 GB. This
  is the multi-megabyte thread the owner asked us to mine. It ran four
  context compactions and at least two usage-limit pauses. Statistics below
  were computed from the raw JSONL, not sampled.
- **The sibling session** `e87ba65e-…-e4326.jsonl` (about 5 MB, still live as
  of writing, one 36 MB subagent) covers the same day's solver-swarm work; it
  is noted but not audited line by line.
- **The product.** `crates/coder-new/src/solver/{mod,hands,tests}.rs`,
  `crates/coder-new/src/issue_run/{setup,decide,agent,verdict,cards}.rs`, and
  the solver plugin at `crates/coder-new/src/solver_plugin/tests.rs`.
- **The outside reference.** Parcha's AgentRun (`~/work/agentrun`, assessed
  in the companion doc) and TypeSafe's public claims, including two tweets
  quoted below.
- **Issues #11375–#11380**, the AgentRun-harvest work items filed today.
- **Transcripts 286–288** (there is no 299; 289 is the newest) and the
  `docs/research/typesafe/` corpus.

## The claim being audited

Two tweets from Parcha/TypeSafe-adjacent accounts this week:

> "We built a new harness using @typesafeai's Jev that cuts the cost of
> repetitive work by 90%. The harness learns the job as it runs, moving steps
> from LLM calls to code. Running 100,000 compliance alerts costs >$290K on
> Opus 5. With agentrun() we got it down to <$26K."

> "been watching our api costs drop like crazy since switching to learned
> patterns instead of hitting the llm every single time, feels like finally
> finding the obvious solution that was hiding in plain sight"

The claim has three separable parts:

1. **Move steps from model calls to code.** Whatever a model does
   predictably, a program does cheaper and exactly.
2. **Learn the job as it runs.** Repeated structure observed in traces
   becomes pinned workflow, so each later run skips the discovery cost.
3. **Keep the model where judgment is needed.** The residual spend is on
   decisions and generation, not on orchestration.

Our own measurements already point the same direction. The Jev-probe work
(episode 287, `docs/terminal-bench/winning-runs-analysis.md`) cut cost 28–61%
at equal pass rates by front-loading evidence selection; the tuner
decomposition (`docs/optimization/coder-components.md`) prices a Jev request
at ~$0.0001 against ~$0.016 for one low-effort Opus turn — a judgment that
saves one turn pays for itself ~160 times over. AgentRun's 90% is a
repetitive-workload number (100,000 near-identical alerts); issue-to-PR is
not that workload, so our expected gain is structural, not proportional.
What transfers is the method: **instrument the loop, find the repeated
shape, pin it in code, and spend the model on the parts that vary.**

## What the trace shows: an orchestrator doing code's job

### The shape of three days

| Measure | Root session | Last 30 subagents (aggregate) |
| --- | --- | --- |
| `Bash` calls | 2,017 (74% of all tool calls) | 3,498 (94%) |
| `Agent` spawns | 315 | 8 |
| `SendMessage` | 245 | 2 |
| `Read`/`Edit`/`Write` | 34/1/30 | 95/13/50 |
| Assistant text blocks | 1,393 | — |
| Thinking blocks | 1,865 | — |
| Human-typed messages | 367 (the rest are task notifications and agent hand-backs) | — |

The root session barely touches files. It is a **control plane**: it spawns
subagents, runs `gh issue` commands (225 of them), `git` (290), `cargo test`
(230), `coder-new` (195, mostly solver runs late in the session), watches
task notifications, and relays messages between agents. The subagents are
where the code work happens, and they are 94% shell.

The 367 real human messages break into a handful of recurring intents:

- **Burn the issue list down** (the dominant one): "start burning down the
  open issue list as fast as possible, fanning out to max 5 subagents";
  later a session-scoped Stop hook: "Continue until you close all open
  issues. You can fan out up to 7 subagents at once"; finally the version
  that produced the solver: "Continue using our 'solver' flow
  programmatically… as you go, improve/edit our solver to be more reliable
  and cost efficient."
- **Analyze a trace**: at least nine `.atif.json` exports pasted with
  "analyze it", "what the fuck is this thing doing", "why is it using
  compute rplugin".
- **Screenshot → fix**: dozens of `[Image #N]` messages pointing at UI
  defects, each becoming a fix.
- **Deploy and release**: TestFlight, web staging/promote/shift, phone build
  57/58.
- **Issue bookkeeping**: review, summarize, close-as-wontdo, handoff
  comments, board status.

### The workflow the model re-derived every time

The session spawned at least 33 "Burn down #N" agents (issues 10559,
10903, 10921–10924, 10954–10963, 10970, 10996–11013), plus the UI-XX series
and assorted others. Every burn-down prompt is the same program written in
fresh prose. Quoting one verbatim:

> 1. Claim first: `openagents issue claim 10954`. If refused (another
>    session holds it), stop and report.
> 2. You are in an isolated git worktree. Rebase on fresh `origin/main`
>    before starting and before pushing. Push `HEAD:main`, rebase and retry
>    on rejection. Never force-push.
> 3. `export CARGO_TARGET_DIR=~/work/openagents-target-agent1`.
> 4. Verification is lean: `cargo fmt` + `cargo test -p <crates you edited>`.
> 5. If blocked or too big, land a coherent tested slice, comment what's
>    done/remaining, release the claim.
> 6. If code-complete on main: comment with commit hashes and what was
>    verified, `gh issue close`, `openagents issue done`.
> 7. Final report (short): commit hashes, verification, closed or not.

That prompt is a workflow document. The orchestrator model was spending
frontier-model tokens to re-author it, per issue, per spawn — while a
sampled burn-down agent then spent 32 of its 88 tool calls on `rg`/`grep`
exploration before writing a line of code. Both halves are expensive for
different reasons: the top because an LLM is writing what should be a
manifest, the bottom because the agent re-discovers what a briefing could
have told it.

### The phases, as actually executed

Collapsing the 33 burn-down runs, the solver runs, and the orchestrator's
own Bash history, the issue-to-PR episode has this shape:

| # | Phase | What the trace shows happening | Performed by today |
| --- | --- | --- | --- |
| 0 | Pick | `gh issue list`/`search`, read titles, avoid conflicts with in-flight work, prefer recent issues | Orchestrator LLM, then the Intaker |
| 1 | Intake | `gh issue view N --comments`, read parent issues and AGENTS.md, check `git`/`gh`/`claude` present, check the board | Intaker (Haiku) + in-process tools |
| 2 | Claim | `openagents issue claim N`, `issue-board.sh claim` | The agent, procedurally |
| 3 | Stage | `git worktree add`, `git fetch`, rebase on `origin/main`, set `CARGO_TARGET_DIR` | Code in the agent's hands |
| 4 | Localize | Read the issue, search the repo (tens of `rg`/`grep`/`sed` calls), open candidate files, decide what the change looks like | Briefer (Haiku), over a deterministic finder ranking |
| 5 | Specify | Turn the issue body + code reading into "the files to change, the shape, the tests, the checks" | Briefer's analysis |
| 6 | Implement | Edit files; run `run_check` compiles while working | Coder (Sonnet if `SIZE: small`, else Opus) |
| 7 | Check | Run required checks: touched-crate `check:`/`test:`, `wasm:` for browser crates, `links:docs` for Markdown | The harness, not a model |
| 8 | Diagnose | On a failed check: read the failure, decide whether the code or the briefing is at fault, write the hand-back | Tester (Haiku) |
| 9 | Repair | Apply the hand-back; bounded by `--rounds` | Coder or Briefer again |
| 10 | Land | `git add`/`commit`, `rebase`, `push`, open the PR on `coder/issue-N` | The harness (`setup::open_pr`) |
| 11 | Report | Comment with hashes and verification, close, board sync, handoff comment if partial | The agent, per the prompt's rules |
| 12 | Orchestrate | Fan out N of the above, watch, merge serially, resolve conflicts, release claims, keep the human posted | The orchestrator LLM |

Phases 3, 7, and 10 are already pure code in the solver. Phases 2, 5, 8,
and 12 are the ones the rest of this audit is about.

## Where the solver stands against that shape

The solver (`docs/coder/solver.md`, built inside this very session on
2026-10-11) is already the workflow-ization of phases 1–11:

- **Four constrained hands** replace the free agent: Intaker (Haiku,
  in-process `environment`/`issue` tools only), Briefer (Haiku, read tools),
  Coder (Sonnet for small, Opus for large and every hand-back), Tester
  (Haiku, read tools + `run_check`). No hand holds a shell; the Coder
  cannot wander; the Tester cannot edit.
- **The harness owns the outcome.** Required checks run in code after every
  Coder round; a clean run never pays for a Tester session. The verdict
  (`issue_run::verdict::judge`) reads check results, not model prose — a
  Tester that says `VERDICT: pass` while a check fails is overridden.
- **Fault classes are separated.** A "No space left on device"/linker/SIGKILL
  failure stops the run as an environment fault instead of handing back to
  the Coder (`hands::environment_fault`) — the exact failure class that
  wasted hand-backs before `d1c6e482eb`.
- **Already-broken tests are excused.** Failing tests rerun in a sibling
  baseline worktree at the base commit; a test that already fails there is
  not blamed on the change (`7e906995c1`). This is what unblocked #11356's
  merge.
- **Cost is real.** From the handoff's results table: $0.107 for a docs-link
  fix (#11338), $0.27 for a web copy change (#11319), ~$2 for ledger work
  (#11279), ~$2.5 for a 19-minute CLI task (#11282). Five PRs merged; the
  failures were a nested-workspace bug (fixed in the handoff's own
  companion commit) and a self-authored test failure.
- **The briefing underneath is already the System One path.** `issue_run
  ::decide` runs the deterministic finder (`scripts/filefind`, 9 stages:
  embedding, symbol, co-change, similar-issue, history, pairing, directory,
  recency, second-pass), then Jev questions per candidate file, then the
  briefing generator. The solver runs it with `--decider off` and puts a
  Haiku Briefer in front instead — the owner directed this explicitly:
  "instead of the decision model API… approximate that flow using a
  constrained Haiku-based agent," "i never said this should use jev, we are
  trying a claude-only version."

### What the solver does not yet do

These are the gaps between the observed manual loop and the shipped flow,
in the order the trace says they cost:

1. **The outer orchestration is still a model.** Listing candidate issues,
   noticing conflicts with in-flight work, choosing what to work next,
   watching N runs, merging their PRs in order, posting handoff comments —
   phases 0 and 12 above — all still live in an orchestrator's context.
   `solver-swarm` (#11346, committed) parallelizes the hands and gives them
   a channel, but the loop that feeds it issues is still chat.
2. **Intake is a model call for what is mostly a lookup.** The Intaker's
   substantive work — check `git`/`gh`/`claude` exist, check the checkout,
   fetch the issue — is three deterministic probes wrapped in a Haiku
   session because someone has to read the reply. The `INTAKE: ready |
   blocked` contract is a Noul-shaped judgment over code-collected state.
3. **Issue fitness is a judgment we currently make with prose.** The
   burn-down prompt rules — "is it blocked by an unfinished dependency? too
   big to finish? already done on main? a duplicate? conflicts with
   in-flight work?" — are exactly Choice/Noul questions over state the code
   already fetches (issue body, comments, open PRs, `git log --grep`). The
   21-day recency filter (`RECENT_DAYS`) is the same judgment hard-coded:
   "is this issue still about code that exists" is a question, not a date
   cutoff.
4. **The Briefer duplicates Jev's job, at ~100× the price per decision.**
   The owner's Claude-only direction is a fair experiment — a constrained
   Haiku agent is a honest proxy for a decision door — but the measured
   economics argue the other way for the parts that are genuinely
   judgments: file relevance (a Noul per candidate), SIZE (a Score),
   change-kind (a Choice). A Haiku Briefer session costs cents; the
   equivalent Jev batch costs tenths of a cent. Keep the Haiku Briefer for
   the generative part (writing the analysis, naming the shape of the
   change); take the classification back to Jev or keep it as a measured
   arm. This is precisely the "learned patterns vs hitting the llm every
   single time" trade the tweets describe — except our version should be
   **measured**, per the calibration doc, not asserted.
5. **The Tester's routing verdict is a judgment wearing a session.**
   `VERDICT: coder | briefer` is a binary classification over (failing
   check output, diff, briefing) — a Noul with a text sidecar. The reading
   of *what* to fix is generative and stays; the routing of *where* it goes
   is a decision.
6. **Hand-back notes lose structure.** The Tester writes prose; the next
   Coder re-reads it. `failing_output` already extracts error lines
   deterministically; the remaining generative step is diagnosis. A typed
   hand-back (`{target, failing_checks, evidence_lines, hypothesis}`) is
   what the `#11378` verify-with-repair port is for.
7. **The merge/land step is serial and manual.** The orchestrator rebases
   and pushes PRs one at a time; the swarm's `first`/`split`/`merge`
   negotiation records are the start of doing this in code. A landed-work
   ledger (which solver run touched which files, merged or not) is
   derivable from `events.jsonl` and `change.patch` — nobody has to read
   it.
8. **Trace analysis is the most repeated human request with no tool.**
   Nine-plus "analyze this .atif.json" requests in three days. Each was a
   bespoke read-through. An `atif stats`/`atif anomalies` surface (repeated
   calls, permission prompts, stalls, cost per phase) would turn each of
   those into a code answer — and is also the instrumentation for learning
   the job in the first place.

## The decomposition: code, judgment, session

Using the component vocabulary from `coder-components.md` — an *operation*
is deterministic code, a *judgment* is a typed System One answer, a
*session* is an executor, a *controller* is the policy that picks the next
component — the issue-to-PR episode re-sorts like this:

| Phase | Operation (code) | Judgment (Jev / constrained Haiku) | Session (executor) |
| --- | --- | --- | --- |
| 0 Pick | list open issues, claims, in-flight runs, board state | is this issue actionable now? does it conflict with in-flight work? is it stale/superseded? | — |
| 1 Intake | probe `git`/`gh`/`claude`, checkout cleanliness, fetch issue + comments | `INTAKE: ready/blocked` — is the run able to start? | — |
| 2 Claim | `openagents issue claim`, board move | — | — |
| 3 Stage | worktree, fetch, rebase, `CARGO_TARGET_DIR` | — | — |
| 4 Localize | finder stages (9, deterministic), candidate ranking | per-candidate relevance (Noul); is the top set sufficient? | gap-fill search when the ranking misses |
| 5 Specify | assemble briefing from selected evidence | SIZE small/large (Score); change-kind (Choice) | write the analysis |
| 6 Implement | file read/write plumbing, `run_check` compiles | — | the coding itself |
| 7 Check | run required checks; baseline-rerun excuse; environment-fault detection | — | — |
| 8 Diagnose | extract error lines, diff vs base | coder-vs-briefer route (Noul) | read failures, write the diagnosis |
| 9 Repair | apply hand-back bookkeeping, round counter | has the repair converged or is it looping? | the repair |
| 10 Land | commit, rebase, push, `gh pr create` | — | — |
| 11 Report | post comment, close, board sync, summary.json | — | — |
| 12 Orchestrate | queue, leases, merge order, retry policy | which queue item next? pause or continue a stuck run? | the handoff/summary a human reads |

The striking thing about this table is how little of the loop wants a
session at all: phases 5 (partially), 6, 8 (partially), and 9. Everything
else is code plus judgments. That is the same conclusion the AgentRun
harness reached from the other end: their `agent` nodes are a minority of
the node kinds, and the interpreter — code — owns the flow.

## What "learn the job as it runs" means here

Parcha's framing — observe traces, extract repeated structure, pin it as a
workflow — maps onto pieces we already own:

- **ATIF is the observation format.** Every solver run already writes
  `events.jsonl`, per-hand transcripts, `summary.json`, `change.patch`. The
  conformance corpus (#11375) gives those records a pinned reader/writer
  contract, which is the precondition for mining them.
- **The mining is phase attribution.** The question "which parts of this
  loop can be code" answers itself only if the trace records which part
  cost what. `summary.json`'s `solver.hands[]` (per-hand wall time, turns,
  tokens, USD) is exactly the right grain; the same per-phase attribution
  is what the tuner doc asks for ("record what each component received,
  returned, cost and changed in the trace").
- **The pattern to pin next is the hand-back.** We have N≈10 recorded runs.
  When a class of Tester's `coder:` hand-backs recurs ("test needs a
  fixture regenerated", "check the wasm half"), the recurrence is evidence
  for a new operation or a briefing fix — the learning loop the tweets
  describe, with measurement attached.
- **Calibration is the difference between learning and guessing.** The
  calibration audit already found ~40 hand-set thresholds and no live join
  of readings to outcomes. A solver emits perfect join material: every
  judgment it consumes (relevance, SIZE, verdict routes) can be recorded
  beside the run's eventual check results. That closes the loop TypeSafe's
  docs insist on: thresholds from our outcomes and our costs, not their
  defaults.
- **The honest version of "90% cheaper".** Our workload won't show 90%:
  issue-to-PR isn't 100,000 near-identical items, and the Coder hand is
  irreducibly generative. What the numbers support: eliminating intake,
  triage, verdict-routing and orchestration spend entirely (code +
  judgments at ~$0.0001 each), shrinking the Briefer to its generative
  residue, and never paying a Tester session on clean runs (already done).
  On the handoff's own data — where small fixes came in at $0.11–$0.27 and
  the failures were harness bugs, not model failures — the plausible bound
  is "cheapest verified pass" per `docs/coder/cheapest-verified-passes.md`,
  not a marketing multiplier.

## What stays in the model — and why

Being honest about the boundary matters more than the boundary itself:

- **Writing the analysis.** Turning an issue plus code reading into "the
  shape of the change" is open-ended generation over a large, variable
  context. A judgment can rank candidates; it cannot yet write the plan.
- **The coding itself.** The Coder hand is the workload's irreducible core.
- **Diagnosis.** Reading a novel failure and saying what it means stays
  generative; only the *route* (coder vs briefer vs environment) is a
  judgment.
- **Negotiation.** Swarm conflict resolution has a fixed rule for the
  common case (`first` wins, other rebases); genuinely contested overlaps
  still want reasoning, or a human.
- **Anything under-measured.** The calibration doc's rule stands: no
  threshold without labelled outcomes. A judgment we haven't measured is
  not cheaper, just less legible.

And one thing the trace warns against: **the model writing its own
control flow.** The orchestrator's most expensive habit was re-deriving
procedure in prose — the burn-down prompt rules, the merge ordering, the
"which issue next" scan — every turn, at frontier prices, inside a KV
cache that compaction kept evicting anyway. That is the "tyranny of the KV
cache" argument from episode 286 playing out in our own house: the model
was simultaneously the program and the state, and both kept paying for it.

## Recommended build order

Ordered by (observed spend removed) ÷ (work):

1. **ATIF analysis tooling.** `atif stats` / anomaly scans over solver and
   agent exports: repeated calls, permission prompts, stall windows, cost
   per hand. This is both the owner's most repeated manual request in the
   trace and the instrumentation every later step needs. Cheap: the crate
   is serde+sha2 and the records already exist.
2. **The conformance corpus (#11375) starting at ATIF.** Pin the reader/
   writer contract before the mining tools multiply.
3. **Intake and claim as code.** The Intaker's three probes plus
   `openagents issue claim` need no session. Keep the conversational
   Intaker only for the chat mode where a human is actually choosing.
4. **Issue-fitness judgments on the queue.** `actionable?`, `conflicts-
   with-in-flight?`, `already-done?` as batched questions over state the
   code already fetches — replacing both the 21-day heuristic and the
   orchestrator's re-read of every open issue. Feed the queue to the swarm.
5. **The schema→questions convention (#11379)** so the briefing path, the
   fitness questions, and future gates share one compile step.
6. **Measure the Haiku-Briefer arm against the Jev arm.** The owner's
   Claude-only experiment is valid and worth keeping as an arm; the decider
   was built to compare them. Record the join (judgments + outcomes) from
   day one per the calibration plan — the trace's real lesson is that
   unmeasured preferences ossify into defaults.
7. **Typed hand-backs and verify-with-repair (#11378).** `{target, failing
   checks, evidence, hypothesis}` instead of prose; bounded drives; the
   rubric stays host-owned.
8. **Effect discipline for the paid/irreversible steps (#11376).** Check
   runs are free; `git push`, PR creation, and (later) paid delegations are
   admitted effects and want the never-retry-unsettled contract.
9. **The merge queue in code.** Serial rebase/push/merge over landed
   solver runs, fed by the swarm's negotiation records, reporting the
   order in `summary.json`.
10. **Program-interpreter validator rules (#11377)** when the solver is
    expressed as a NIP-PRG program rather than hard-coded drive logic —
    reserved host state, parallel merge conflicts, decision sidecars — the
    form the whole thing eventually takes as a portable, publishable
    workflow.

Steps 1–4 are days, not weeks, and they remove the majority of the
non-generative spend the trace documents. Steps 5–10 are the "learns the
job" half, and they depend on the measurement floor the first steps pour.

## Appendix: the session in numbers

Computed from the raw JSONL on 2026-10-11; subagent aggregate covers the
30 most recent of 827 transcripts.

- Span: 2026-10-08T22:05Z → 2026-10-11T21:25Z (about 71 hours of wall time,
  with two usage-limit pauses and four context compactions).
- Records: 29,495 in the root file — 5,975 assistant turns, 3,521 user-side
  records (of which 815 carry text; 367 are actually typed by the human,
  the rest being task notifications, agent hand-backs, and hook output).
- Root tool calls: 2,717 — `Bash` 2,017, `Agent` 315, `SendMessage` 245,
  `TaskStop` 47, `Read` 33, `Write` 30, `ToolSearch` 14, `WebFetch` 9,
  `Edit` 1.
- Bash families in the root: `cargo test` 230, `gh issue` 225, `git` 290+89
  worktree, `coder-new` 195 (mostly `solver`), `gh pr` 47, `cargo build`
  38, `cargo check` 25, `scripts/*` 67, lease invocations ~14, solver runs
  14 direct.
- Subagent aggregate (last 30): `Bash` 3,498 of ~3,737 tool calls (94%);
  file tools under 5%.
- Burn-down subagents named in prompts: 33 distinct issues (10559, 10903,
  10921–10924, 10954–10963, 10970, 10996–11013), plus the 15-item UI-XX
  series and assorted one-offs.
- Sampled subagent anatomy: 88 tool calls, of which 32 were search
  (`rg`/`grep`/`sed` exploration), before and around a single-issue code
  change — localization consumed the plurality of its loop, matching the
  briefed-agent and Jev-probe findings elsewhere in this repository.
- Solver economics so far (handoff table): merged PRs at $0.107–$2.49 and
  1.5–25 min wall each; two recorded failures were harness bugs since
  fixed, not model failures.
