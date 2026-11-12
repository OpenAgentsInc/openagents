# Recursive Language Models in Coax: the first measurements

2026-11-12, GitHub #13367. The library and its design are in
[coax/lib/rlm/README.md](../../coax/lib/rlm/README.md); this page is what we ran and what it showed.

## The question

Can a Recursive Language Model (Zhang, Kraska and Khattab, arXiv 2512.24601), built in Coax on
our gateway, answer a question that depends on every part of a context far bigger than one
prompt, where a single call on the same model cannot? And what does it cost?

## The experiment: OOLONG-style counting

**The task.** OOLONG's trec_coarse split (arXiv 2511.02817), which the paper uses for work that
grows linearly with the context, rebuilt from its source data: 3,000 questions drawn with a seeded
shuffle from the TREC question-classification training set (Li and Roth; 5,452 labeled questions),
each on a line with a made-up date and user ID, the labels hidden. Six questions, one per label:
"how many of the questions should be labeled 'numeric value'?" (and abbreviation, description and
abstract concept, entity, human being, location). Each needs every one of the 3,000 lines
classified; no line answers it alone. The context is 296,244 bytes, 109,467 tokens on Gemini's
tokenizer. Scoring is OOLONG's for numbers: 0.75^|answer − truth| (1.0 exact, 0.75 one off, 0.056
ten off). `rlm oolong --data train_5500.label --n 3000 --seed 7` builds it.

**Three ways to ask, all with Gemini 4 Argon as the model that answers:**

- **cut**: one call with the context cut to its first 96,000 characters (35,400 tokens), the size
  of a 32K-token window as in the paper's Qwen3-8B setting, and of Microcoder's working state.
- **whole**: one call with the whole context. Argon's real window holds it, so this is the
  reference the paper could not run on GPT-5 for its larger tasks: it separates "it did not fit"
  from "it fit and the model still could not do it".
- **RLM**: Argon as the root (`low` thinking), Gemini 3.8 Flash for sub-calls (`low`), depth 1 as
  in the paper's main results, at most 20 turns and $1.20 a run.

## Results

The count each way gave, against the truth, and OOLONG's score. One run each.

| Label | Truth | Cut: count | Score | Whole: count | Score | RLM: count | Score |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| numeric value | 500 | 135 | 0 | 227 | 0 | 499 | 0.75 |
| abbreviation | 46 | 14 | 0 | 43 | 0.42 | 41 | 0.24 |
| description and abstract concept | 649 | 142 | 0 | 269 | 0 | 627 | 0 |
| entity | 677 | 160 | 0 | 243 | 0 | 663 | 0.02 |
| human being | 672 | 146 | 0 | 260 | 0 | 673 | 0.75 |
| location | 456 | 132 | 0 | 215 | 0 | 465 | 0.07 |
| **Mean** | | | **0.00** | | **0.07** | | **0.30** |
| **Mean error** | | **379** | | **291** | | **8.7** | |

| Way | Mean cost (card) | Mean time | Calls a task |
| --- | ---: | ---: | --- |
| Cut (one Argon call, 35K tokens in) | $0.14 | 7 s | 1 |
| Whole (one Argon call, 109K tokens in) | $0.44 | 13 s | 1 |
| RLM (Argon root, Flash sub-calls) | $0.76 | 427 s | 8 to 20 root, 30 to 76 sub |

- **The RLM gets the counts; neither single call does.** Its error is 1 to 22 (mean 8.7, 1.4% of
  the truth); the cut call counts only what it sees (about a third) and does not scale up; the
  whole-context call fits but still undercounts by more than half on five of six labels. This is
  the paper's OOLONG finding on our stack (GPT-5: 44.0 base, 56.0 RLM), sharper here because every
  task is an exact count.
- **What remains is labeling, not counting.** The RLM's misses are on the labels whose TREC
  boundaries are fuzzy (description against entity, 14 and 22 off); its sub-model's labels, not its
  arithmetic, decide them. OOLONG's 0.75^error metric gives 0.02 for 14 off, so the mean score
  (0.30) understates the gap the mean error shows.
- An earlier run on "numeric value" ended with no answer: at turn 13, with 498 found, Gemini
  returned `MALFORMED_FUNCTION_CALL` for a root reply and the driver ended the run ($0.65 spent).
  Fixed in f93fe15a05 (a refused root reply gets a note and the turn goes again, up to three in
  a row, as the reference's `max_errors`); the table's run on that label is the rerun.

## A real use on this repository

Question, over the twelve `.coax` files of `coax/lib/microcoder/` (236 KB, 92K tokens): "List
every limit, in bytes or in counts, on Microcoder's state and on what one step may add to it (the
constant, its value, its file, what it limits), then explain what happens to older steps when the
recent steps no longer fit."

| Way | Cost | Time | Calls | Answer |
| --- | ---: | ---: | --- | --- |
| RLM | $0.86 | 347 s | 19 root, 0 sub | All 11 limits in `state.coax`, the 3 in `action.coax`, 14 in `tools.coax`, 5 in `loop.coax` and `run.coax`, 2 in `repo-tools.coax`, the fixed buffer sizes and field cuts; compaction (both triggers), what a compacted step keeps, and `earlier` overflowing into `left_out` |
| Whole (one Argon call) | $0.40 | 37 s | 1 | The same constants and the same account of compaction, a little shorter |

Every constant and value both answers name checks against the source. Here the whole folder fits
Argon's window and the question is lookup, so one call was half the cost and nine times faster:
the RLM read the files itself through `print` (no sub-calls), 12 KB a turn. An RLM pays off when
the work grows with the context (OOLONG) or the context passes the window, not for lookups in
what one call can hold. For Microcoder the window that matters is its 37K-token state, not
Argon's (below).

## How it behaved

- **The strategy was the paper's, every time.** Look at the context's start and end; split it
  into lines and index the questions; batch 100 to 150 questions into each Flash prompt (20 to 30
  prompts, one `llm_query_batched`), ask for `ID: LABEL` lines, parse and count them in code.
- **Then it verifies, a lot.** Four of six runs used 18 to 20 of their 20 turns: a second full
  labeling pass, comparing the passes, re-asking the disagreements, auditing borderline questions
  against the TREC guidelines, printing long lists back to itself. The first pass is already
  close: in the run that ended early, it counted 494 of 500 at turn 4 of 13. The root's own tokens are most of the
  cost (25K to 340K input tokens a run, against 53K to 142K for all sub-calls).
- **The language held up.** Across seven OOLONG runs and the repository run the model hit two
  interpreter errors: a regex with `|` (outside `coax/lib/text/regex.coax`'s subset) and a chained
  comparison (`a < b < c`); it rewrote both and went on. It used f-strings, comprehensions, dicts
  with `get`, `sorted`, `enumerate`, slicing and `llm_query_batched` freely; no block needed
  `def`, floats or imports.
- **Suspend and rerun was invisible to the model**, as designed: a block with one batch of 20 to
  30 prompts ran twice, its prompts served eight at a time in between.

## Cost

Card prices (Vertex on-demand: Argon $4.00 input, $0.20 cached, $20.00 output a million tokens;
Flash $1.50, $0.15, $7.50). Total spend $9.98 of the $15 budget:

| What | Spend |
| --- | ---: |
| OOLONG, cut, 6 calls | $0.85 |
| OOLONG, whole, 6 calls | $2.63 |
| OOLONG, RLM, 6 runs | $4.57 |
| OOLONG, RLM, the run that ended on `MALFORMED_FUNCTION_CALL` | $0.65 |
| Repository question, RLM and whole | $1.26 |
| Smoke run and the replay fixture's recording | $0.03 |

The RLM's mean $0.76 a task is 1.7 times the whole-context call. Most of it is the root
re-verifying; a turn note giving the spend and the turns left, and the reference's line "if you
are running out of turns, submit your best inference", should cut it (not built or measured).

## How this could plug into Microcoder

Microcoder's measured weakness on wide tasks is forgetting what it read
([2026-11-12-microcoder-next.md](2026-11-12-microcoder-next.md), "Economy"): its state keeps
about 140 KB (37K tokens) of recent results; a compaction keeps a step's rationale and only the
names of the files it viewed, so on a task spread over many files it views the same files again
(33 files viewed 75 times in one trace; Carbon read "again the files we viewed in compacted
steps" from its sixth call on and never edited). An RLM is the opposite arrangement: what was
read stays in an environment, outside the prompt, and the model keeps only what it asked for.
Not built (another agent is analysing Microcoder now), in the order to try:

1. **An `ask` action.** Next to `view` and `commands`, an action Microcoder can take:
   `ask: {paths: [...], question: "..."}`. The harness reads those files into an RLM's context
   (`rlm_set_files`) and runs `rlm_run` with Microcoder's own gateway plan and the run's remaining
   budget as `RlmLimits.max_usd_micros`; only the answer (which should name paths and lines) goes
   into the state. A wide question ("every place that sets X, with its line") costs a few hundred
   bytes of state instead of the files, and survives compaction because the answer is the
   step's result, which a compaction keeps the start of. This is the paper's setting exactly:
   Microcoder is the root, the files are `context`.
2. **A run-long environment.** One `Rs` per Microcoder run whose `files` grow as Microcoder views
   them; a compacted step then keeps "viewed X (in the environment)" and later steps `ask` about
   X instead of viewing it again. The interpreter's state (variables, lists, the memo) is already
   kept across blocks and turns; what is new is adding files during a run.
3. **Measure before keeping either:** on the tuning tasks and the wide Carbon issue from the
   Microcoder analysis, count whole-file views, re-reads, calls, cost and pass rate with and
   without `ask`. About $5, as that page estimates for its own next step.

What it would reuse as is: `coax/lib/rlm/run.coax` does no I/O except through `model.coax`, which
takes any `Gateway` and model for each role (Microcoder's Vertex plan included) and prices every
call on the same rates card, so an RLM's spend adds to a run's spend in the same micro-dollars;
`policy.coax` already refuses a call that would pass a cap.

## What was not verified

- **One run per cell.** No repeats, so no variance; the differences between the RLM and the single
  calls (errors of 1 to 22 against 100 to 500) are far outside any plausible run-to-run spread,
  but the RLM's differences between labels are not.
- **OOLONG rebuilt, not OOLONG itself.** Same source data and task shape (trec_coarse counting),
  our own sampling (3,000 questions, seed 7), dates and user IDs, and only the six per-label count
  questions (OOLONG also asks comparisons, most and least frequent, and per-user questions).
  OOLONG-Pairs, S-NIAH, BrowseComp-Plus and CodeQA were not run.
- **The cut baseline's window is ours** (96,000 characters, about 35K tokens), chosen as the
  paper's 32K-token setting and Microcoder's working state; Argon's real window held the whole
  context, which is why the whole-context call is reported too.
- **Depth 2 (child RLMs) was checked only on the scripted model** (`run.coax`'s recursion check),
  not live; every live run used depth 1, the paper's main setting.
- **The prompt was not tuned.** It is the reference's, adapted to our language; no ablations.
- The replay check (`coax/lib/rlm/replay.sh`) covers the live wire for a three-turn run with one
  batch of eight; longer live paths (retries, stalls, a refusal mid-batch) are covered only by the
  gateway's own checks and the scripted model.
