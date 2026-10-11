# Harness design for coding agents: what the study found, and how Coder should improve itself

Source: Fan, Zhang, Ma, Hu, Wang, Song, Liu, Zamani, Wang. *An Empirical Study
of Harness Design for Coding Agents*. arXiv:2609.20804v1, 17 Sep 2026 (UMass
Amherst, Zoom, Emory, UNC Charlotte).

## What they did

They built a small ReAct harness, held its loop fixed, and varied three
parts: context management, planning, and the action space. Then they ran 176
matched settings:

- **Models:** Nemotron-3 at 30B, 120B and 550B (a capability axis within one
  family), plus Mistral-Medium-3.5-128B (a second family).
- **Benchmarks:** SWE-Bench Verified (500 tasks) and Terminal-Bench 2.1 (89
  tasks).
- **Windows:** 32k, 64k, 96k and 128k tokens.
- **Settings:** temperature 0 and at most 300 steps. They report success rate,
  cost per task at OpenRouter prices, and turn-level phase labels from an LLM
  judge.

Five context-management tiers:

| Tier | What it does |
| --- | --- |
| T0 | Nothing. The run dies when it overflows. |
| T1 | Elision: stale bulky tool outputs become short stubs. |
| T2 | Elision plus `recall_event`, which brings back a stubbed output. |
| T3 | LLM summarization of the middle of the history. |
| T4 | Elide at a soft threshold (0.6 of the window), summarize at a hard one (0.85). The preamble and the last ≥2 turns stay verbatim. |

- **Planning:** an `update_plan` tool, with the plan injected each turn rather
  than stored in history.
- **Action space:** a typed tool set (read/write/edit file, list/glob/grep, web
  fetch, bash) versus bash only.
- **Held fixed:** a workspace path guard, read-before-write with hash checks, a
  post-edit lint (ruff/pyflakes), and stuck detection. Stuck detection sends a
  reminder after 5 identical calls and ends the run after 8 identical failing
  calls.

## What they found

1. **Context management only matters when the window binds.** The gain of
   managed over unmanaged (SWE-Bench, averaged over models) falls from 35.7
   points at 32k to 15.9, 5.5 and 2.7 at 128k. Almost all of it comes from
   preventing overflow deaths: T0's overflow rate falls from 78.7% to 8.7% as
   the window grows. Once a run fits, management barely changes behaviour; it
   only makes trajectories longer.
2. **Cheap elision before summarization (T4) gives the best cost at about the
   same accuracy.** T4 had the lowest cost in 7 of 8 panels. It also had the
   lowest peak context at every budget, and called the summarizer less than T3
   did.
3. **Recall is dead weight.** In 56% of the runs that had `recall_event`, it
   was never called. The median use was zero calls. Recall was −0.36 points
   versus elision alone. Strong models almost never use it.
4. **Planning helps weak models and saves money for strong ones:**
   - **Nemotron-30B:** +11.6 points on SWE-Bench. Planning keeps the run alive
     until it reaches an edit: without a plan, 68.6% of runs stop without
     editing.
   - **Nemotron-550B and Mistral:** cost falls by about 30% and 32%, with
     accuracy down 0.4 to 2 points. Almost all the saving is redundant
     post-edit verification that the plan trims.
5. **The best tool set depends on the model and the task:**
   - **Weak models need typed tools:** Nemotron-30B scores 15 points worse on
     SWE-Bench with bash only. It emits tool calls that don't exist in a bash
     registry and dies within about 15 turns.
   - **Strong models are cheaper and as good or better with bash only:**
     Nemotron-550B gains 1.6 points and costs 53% less on SWE-Bench. Bash lets
     it bundle several operations into one call. Re-patches of an
     already-edited file fall from 4.6 to 1.5 per task, and whole-file rewrites
     rise from 51% to 76%.
   - **Mistral flips with the task:** typed tools are +23 points on SWE-Bench,
     but bash only is +6.7 on Terminal-Bench, which is shell-heavy.
6. **The thesis.** Harness design is a conditional systems problem. Choose
   each part for the target model, task type and budget; don't ship one
   default.

## Limits worth keeping in mind

- **Coverage:** one run per task. Planning and the action space were tested
  only at T4/128k.
- **Sample size:** Terminal-Bench has only 89 tasks, so most of its contrasts
  are not significant.
- **Python only:** SWE-Bench Verified is all Python, and the post-edit
  diagnostic was a Python linter.
- **Bundled change:** the action-space switch also changed instructions,
  file-state tracking and diagnostics, not just the tool count.
- **Few models:** three sizes of one family plus one other model. The
  crossover points need checking on the models we actually run (DeepSeek V4.1
  Flash, Gemini 3.8 Flash on Vertex, Claude through Claude Code, Clef/Jev for
  decisions).

## Where Coder stands against it

| Part | Coder today | Study's verdict |
| --- | --- | --- |
| Context | LLM summary of older turns at 80% of the window, newest turns kept (`long_session.rs`, #11179). No elision. | This is T3. T4 (stub bulky old outputs first, summarize only if still needed) costs less at the same accuracy. |
| Recall | None. | Correct; skip it. |
| Planning | No plan tool in Coder's own loop. Delegations to Claude Code and Codex use theirs. | A plan helps weak models finish and trims strong models' re-verification. Worth adding, with control over when it's on. |
| Action space | Typed tools (Read/Edit/Write/Grep/Glob, web, media) plus Run, always all offered. Read-before-edit with change detection. Since today, no wall around the repository. | Typed tools for weaker models; strong models do as well or better leaning on the shell. Should depend on the model, not be fixed. |
| Stuck detection | Not in Coder's provider loop. The ATIF export counts repeats after the fact: one session today had 50 repeated calls (24 Run, 18 Read, 8 Edit). | A reminder at 5 identical calls and a stop at 8 identical failures is cheap and well tested. |
| Post-edit diagnostics | None. The model runs `cargo check` itself, often through the build lease, at a cost of minutes. | A fast read-only check after each edit catches syntax and name errors before a test turn. For Rust, `rustfmt --check` on the file is instant; a scoped `cargo check` is too slow to run every time. |
| Briefing | The briefed agent (filefind briefing plus constrained Claude, `crates/briefed-agent`) measured −54% cost per accepted change at the same success rate. | Consistent with the paper: front-loading localization is the strong-model cost saver, the same thing planning did for Nemotron-550B. |

## A theory of how Coder improves itself

The paper's real lesson is not "use T4". It is that every harness part is a
bet whose payoff depends on **who is driving** (the model), **what the road
is** (the task type) and **how much fuel there is** (window and dollars).
Coder already runs many models (auto picks per class, flash is DeepSeek, Claude
Code and Codex are delegations), so one fixed harness is guaranteed to be wrong
for most of them. Self-improvement is therefore choosing the harness per
(model, task, budget), measured on Coder's own work, with the loop closed
automatically.

### 1. Make the harness a typed configuration, not code paths

One record per run: `context: t3|t4`, `plan: off|on`, `tools: typed|shell|both`,
`stuck: thresholds`, `diagnostics: off|fast`. The record goes in the ATIF
session header, so every export says which harness produced it. Defaults come
from the paper:

- T4 everywhere;
- plan on for models below a capability bar;
- typed tools for weak models, both for strong ones;
- stuck detection at 5/8;
- fast diagnostics on.

### 2. Measure every run the same way (now possible)

As of `6ab491f16a`, each model call's tokens, dollars and time are on the
export, with real timestamps and tool durations. Add the outcome (accepted
change, tests passed, the person's thumbs) and three phase labels computed
from tool calls, not an LLM: **localize** (reads and greps before the first
edit), **edit**, **verify** (test or check runs after the last edit). These are
exactly the signals the paper used to explain each effect:

- overflow deaths;
- runs that end before an edit;
- post-edit verification tails;
- re-patches of the same file;
- identical-call streaks.

### 3. Choose per run with a typed decision, not a rule table

Which configuration a run gets is a judgment: the model's measured capability,
whether the task is shell-heavy or repository-heavy, and the window and budget
left. Make it a Jev/Clef decision over the typed configuration, with the
features above as input. Workspace policy forbids keyword routing, and this
gets calibrated probabilities to learn from.

### 4. Learn from Coder's own traffic

Treat each choice as a bandit arm per (model, task class) cell:

- **Exploration:** run a small, bounded share of runs on a sibling
  configuration (one part changed).
- **Comparison:** compare cost per accepted change at equal success with paired
  tasks, the same McNemar logic the paper used. Gym suites
  (`crates/gym/suites/coder-turns-*`) give replayable paired tasks, and the
  goldens give regression guards.
- **Promotion:** promote a cell's default only on a significant win.
- **Visibility:** record every promotion as a doc entry and a commit, so a
  person can see and revert it.

### 5. Turn the measurements into harness changes, in this order

Cheapest and best evidenced first:

1. **Stuck detection in the provider loop.** A reminder at 5 identical calls,
   a stop at 8 identical failures. It's an hour of work, and today's exports
   show the waste.
2. **T4 context.** Stub bulky tool outputs older than the last two turns at
   60% of the window and summarize only at 85%. Keep compaction's existing
   summary path as the second stage.
3. **A plan tool** (`update_plan`, injected each turn, not stored), on by
   default for models the measurements mark as weak. Watch whether it cuts
   strong models' verification tails, as it did for Nemotron-550B and Mistral.
4. **Fast post-edit diagnostics**, read-only and instant (`rustfmt --check`,
   `tsc --noEmit` for a single file where cheap). Findings are appended to the
   Edit result.
5. **Action space per model:** keep typed tools for flash-class models and
   test shell-leaning prompts for strong ones. Re-patch counts and whole-file
   rewrites are the tell.

### 6. Guard against what the paper warns about

- **Don't keep machinery models don't use.** Recall was never called; check
  every new tool's call rate after a week and remove it if it's unused.
- **Re-measure after each model change.** An improvement for one model can be
  pure overhead for another. Every promotion is per cell, and cells are
  re-tested when a model version changes.
- **Measure cost and success together.** Planning lowered cost and slightly
  lowered accuracy for strong models. The objective is cost per accepted
  change at a success rate no worse than the incumbent's, not either number
  alone.

The loop, in one line: **record the harness, measure each run, decide the
harness per run with a typed judgment, test a sibling on a small share of
traffic, and promote only on paired wins.** That is Coder improving itself on
its own work, the same loop the self-improving-codebases product applies to a
customer's repository (`docs/product/self-improving-codebases.md`).

## Measured on Coder's own sessions

**T4 context (#11286).** The saved local sessions that read back (23 of 47),
replayed turn by turn under the old summary-only policy (T3, summary at 80%)
and under T4 (stubs at 60%, summary at 85%), with the plain local summary.
"Sent" is the estimated tokens the model reads at the start of each turn,
summed over all sessions.

| Window | Policy | Mean peak | Summaries | Sent |
| --- | --- | --- | --- | --- |
| 32k | T3 | 10,035 | 8 | 478,707 |
| 32k | T4 | 10,035 | 7 | 484,183 |
| 64k | T3 | 10,529 | 6 | 710,026 |
| 64k | T4 | 10,035 | 5 | 575,904 |
| 128k | T3 | 13,952 | 2 | 1,561,717 |
| 128k | T4 | 12,964 | 1 | 900,404 |

T4 called the summarizer less at every window and read 19% fewer tokens at
64k and 42% fewer at 128k; at 32k, where both summarize early, it was even
(+1%). Three sessions exceed 32k within one turn, which neither policy can
help, since both act between turns. Rerun with
`CODER_CONTEXT_REPLAY=<sessions dir> cargo test -p coder-new --lib
replay_saved_sessions -- --ignored --nocapture`.
