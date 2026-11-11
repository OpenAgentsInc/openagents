# Antigravity task fit: which coding tasks Gemini through `agy` is good for

Date: 2026-11-11. Machine: coderos-4080. Issue: GitHub #13193 (the Experiments
section). Coder at d82c3ed9e8 (Antigravity engine from #13192), `agy` 1.5.6.

## Summary

Six tasks rebuilt from this repository's history, each run once through
`coder run` on five configurations: Antigravity with Gemini 4 Carbon, Gemini 4
Argon and Gemini 3.8 Flash (all `-medium`), and Claude Code with Sonnet 5.5 and
Opus 5.5. Codex was not run: its login was out of quota until 2026-11-13.

| Configuration | Passed | Gemini credits or list price | Per passing task | Median wall time |
| --- | --- | --- | --- | --- |
| Antigravity, `gemini-4-carbon-medium` | 4 of 6 | $7.38 | $1.85 | 713 s |
| Antigravity, `gemini-4-argon-medium` | 5 of 6 | $5.02 | $1.00 | 191 s |
| Antigravity, `gemini-3.8-flash-medium` | 5 of 6 | $4.34 | $0.87 | 375 s |
| Claude Code, `claude-sonnet-5-5` | 6 of 6 | $0.93 | $0.16 | 27 s |
| Claude Code, `claude-opus-5-5` | 6 of 6 | $2.11 | $0.35 | 38 s |

- Every configuration did the mechanical rename, the code reading and the
  AGENTS.md paragraph correctly.
- Argon and Flash fixed the bug and made the shared atlas change. Both got the
  feature's exit codes right but printed the failure message for the new
  warning.
- Why Carbon was slow, and what was fixed (GitHub #13227), is under "Why
  Carbon was slow" below: its long thinking, minutes-long waits for a first
  token and a dropped prompt cache in the `global` location. Coder now fails
  a turn agy gave up on, and coderos-4080 names the `us` location.
- Carbon hit the 15-minute limit twice (the bug fix and the atlas change). In
  each run it waited minutes for a model reply: once more than 5 minutes for a
  single reply, and more than 4 minutes for the first reply of a 12-line doc edit.
- Claude Code finished every task in under a minute. Argon took 2 to 14 times
  as long as Claude on the same task, and Carbon up to 70 times. Per passing
  task, the Gemini models cost 2.5 to 12 times as much at list prices.

Recommendation: keep Golemancy's rubric on Claude by default. Offer Antigravity
as an opt-in engine, not as a tier. Argon is the model to offer, for Haiku- and
Sonnet-tier work when Claude's plan limits are used up, because it runs on
Google Cloud credits. Don't use Carbon in Autopilot yet. Details are under
Recommendation.

This is 30 graded runs: one run per task and configuration, and one task per
kind. A difference of one pass between two configurations is noise. The speed
and cost gaps are large and the same in every task, so they are the most
trustworthy result here. The pass rates are not.

## Method

### Tasks

The tasks, their prompts, start commits and checks were written down before any
run (kept with the raw logs, below). Each check decides pass or fail without a
model.

| # | Kind | Start | Known-good answer | Check |
| --- | --- | --- | --- | --- |
| T1 | Mechanical edit: a renamed type | `b04d3d2f7f^` | b04d3d2f7f (`CsLog` to `SwLog` in `coder/tui/diff-cells-checks.coax`) | `coax check` reports no type error; only that file, at most 2 changed lines |
| T2 | Bug fix with a failing check | `3c7c914018^` | 3c7c914018, page side (`apps/openagents-web/everglade2-net.coax` wraps any monster yaw) | the commit's two assertions (a yaw of 9423 is taken as 3.14) inserted into the monsters check, run with `--only` |
| T3 | Feature in one Coax unit | `66c523bac5^` | 66c523bac5 (`coax/web/wasm-size.coax`: up to 2% over budget warns) | the `budget` command on five sizes (exit codes, and a warning on stderr naming the module), and `coax test` of the file with a new check |
| T4 | Reading and explaining code | `d82c3ed9e8` | the facts below, written before the runs | regexes on `EXPLAIN.md` for four required facts; no other file changed |
| T5 | Shared engine code, GPU texture limit | `b1bd8b09a6^` | b1bd8b09a6 (`components/raster/atlas.coax` widens the glyph atlas to stay within 8,192 rows) | a hidden check (3x fits at 8192 wide, 2x stays 4096, 6x does not fit) appended to the file; all of its checks pass |
| T6 | Doc edit in AGENTS.md style | `24d562c66e^` | 24d562c66e (the "several small issues" paragraph) | only additions, in the named place; a bold lead-in; all seven facts; lines up to 85 columns; no filler words; at most 190 words |

T4 is a real bug, found while setting up these runs. Since 7aa8d77f91 (#13183),
`coder run --engine claude --model claude-sonnet-5-5` runs on the CLI's default
model, Opus 5.5. The model reaches Claude Code as `--model` through the owner's
`ClOptions`, but the engine's own choice (`e[0].model`) stays at the unnamed
default. `e_note_start` records that empty id as `run_model`. When `initialize`
is answered, `e_take_models` (`claude_discover`) puts the CLI's resolved default
(`claude-opus-5-5`) first. `e_drift` then sees a change, and `e_take` sends
`set_model` for Opus ahead of the first message. The four required facts were
`e_take` with `set_model`, `e_drift` with `e_note_start` or `run_model`, the
catalog rebuild with the default first, and the options path for `--model`.
Naming 7aa8d77f91 or #13183 was recorded but not required.

No compiler task was run. The compiler commits that fit (for example
51de31e0c8, SPIR-V array samples) are from 2026-11-01. No published compiler
build exists for those trees, and a bootstrap takes far more than the
15-minute limit. T5 stands in for "shared engine or GPU code". No multi-file
refactor was run.

### Configurations and runs

- `coder/tui/coder.sh run PROMPT --engine antigravity --model gemini-4-{carbon,argon}-medium`
  or `gemini-3.8-flash-medium`, and `--engine claude --model claude-sonnet-5-5`
  or `claude-opus-5-5`, all with `--stream-json` and without `--ask`. Coder's
  default trust gives full access inside the worktree, as Autopilot runs. The
  ACP logs confirm that agy switched each session to the named model.
- Each run had a fresh worktree at the start commit, with the published compiler
  for that tree installed first. It had a 15-minute limit (`timeout 900`) and was
  never pushed. Every prompt began: "You are working in a throwaway git worktree
  made for an experiment. Edit files here directly. Do not create another
  worktree, do not commit, do not push, and do not comment on GitHub."
- T1 ran first on all five configurations to test the harness. The other 25 ran
  at the same time on coderos-4080 (28 cores, 125 GB). That load fell mostly on
  the providers. Gemini's long waits were waits for model replies, not for local
  commands.
- Claude Code effort was left at its default.

### What was measured

- Pass or fail: the check's output (`check.txt` in each run's folder).
- Finished on its own: Coder's `end` line, or the 15-minute limit.
- Wall time: from `coder run` start to exit. For the five T1 runs, `bc` was
  missing, so wall time is Coder's turn time plus 1 second.
- Steps: Coder's tool steps (`end.steps`). Model calls: for Claude, distinct
  assistant messages in Claude Code's session log. For Antigravity,
  `streamGenerateContent` calls to the run's model in agy's ACP log.
- Tokens: Antigravity's per-turn usage line (uncached input, cached input,
  output, and the thinking part of output). Carbon's two timed-out turns wrote
  no usage line, so their totals were read from agy's conversation store
  (`~/.gemini/antigravity-cli/conversations/ID.db`, `gen_metadata`). That
  reading matched the usage lines of two finished runs exactly. For Claude:
  input, 1-hour cache writes, cache reads and output, summed per message from
  Claude Code's session log.
- Cost:
  - Antigravity reports tokens, not cost. Prices are Vertex AI's on-demand
    global prices from the Cloud Billing catalog for project `openagentsgemini`,
    read on 2026-11-11, per million tokens:
    - Gemini 4 Carbon: input $4.00 (C47F-BE66-1A4E), cached $0.20
      (C3FF-AD89-C218), output $20.00 (1B77-85A0-B300). This is
      `coax/lib/gateway/rates.coax`'s card.
    - Gemini 4 Argon: the same prices (3BAE-62B7-C9B9, 3B3E-7322-E883,
      1E84-F91D-8B95).
    - Gemini 3.8 Flash: input $1.50 (2A35-AC68-14D6), cached $0.15
      (8658-6939-8BEC), output $7.50 (90FD-DC5F-DC55).
  - Output includes thinking: agy's total is input plus cached plus output.
  - `rates.coax` lists Flash at $0.75, $0.075 and $3.75. Those match the
    catalog's Flex, Batch and Off-Peak SKUs, not on-demand.
  - Coder's Claude stream doesn't carry Claude Code's `total_cost_usd`. Claude
    runs are priced at Anthropic's list prices: Opus 5.5 at $4 input, $8 per
    1-hour cache write, $0.20 cache read and $20 output; Sonnet 5.5 at $2, $4,
    $0.10 and $10. They ran on the owner's subscription, so these are
    list-price equivalents, not money spent.
- Not counted: with Carbon and Argon, agy also calls `gemini-3.1-flash-lite`
  for its own safety monitor (2 to 16 calls a run; Flash runs made 1). Those
  calls don't appear in agy's usage. They cost cents at Flash-Lite prices.

### Changes to the harness

- **Claude's `--model` (a local patch, not committed).** The bug in T4 meant
  `--model claude-sonnet-5-5` ran Opus. The Claude runs used a Coder with one
  changed line in `coax/lib/agent/engine/claude.coax`: `e_take` skips the drift
  `set_model` when an owner's options started the CLI
  (`&& e[0].opt.count == 0`). Each Claude run's session log names the model it
  ran: `claude-sonnet-5-5` or `claude-opus-5-5`, 6 runs each. Antigravity runs
  don't use that code.
- **The T6 grader (fixed after the runs).** It looked for "one commit per issue"
  case-sensitively and on one line. Sonnet wrapped "one" and "commit" across a
  line break. Flash wrote "One commit per issue". All five T6 runs were
  regraded with the fixed grader. The original verdicts are kept next to the
  new ones.

## Results

Tokens for Antigravity are uncached input / cached input / output (of which
thinking). For Claude they are input + cache writes / cache reads / output.

| Task | Config | Check | Finished | Wall | Steps | Model calls | Tokens | Cost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T1 | carbon | pass | yes | 98 s | 9 | 6 | 42k / 140k / 1k (326) | $0.22 |
| T1 | argon | pass | yes | 48 s | 5 | 4 | 32k / 65k / 696 (219) | $0.16 |
| T1 | flash | pass | yes | 49 s | 10 | 11 | 38k / 162k / 2k (1k) | $0.10 |
| T1 | sonnet | pass | yes | 14 s | 2 | 3 | 6+23k / 76k / 530 | $0.10 |
| T1 | opus | pass | yes | 11 s | 3 | 4 | 8+24k / 111k / 756 | $0.23 |
| T2 | carbon | fail | no (limit) | 900 s | - | 23 | 241k / 1487k / 16k (15k) | $1.58 |
| T2 | argon | pass | yes | 207 s | 11 | 14 | 87k / 661k / 8k (6k) | $0.65 |
| T2 | flash | pass | yes | 385 s | 41 | 42 | 218k / 2385k / 46k (42k) | $1.03 |
| T2 | sonnet | pass | yes | 26 s | 5 | 6 | 12+26k / 190k / 2k | $0.14 |
| T2 | opus | pass | yes | 38 s | 4 | 5 | 10+29k / 156k / 2k | $0.31 |
| T3 | carbon | pass | yes | 348 s | 19 | 15 | 185k / 785k / 14k (10k) | $1.18 |
| T3 | argon | fail | yes | 73 s | 7 | 7 | 92k / 159k / 2k (0) | $0.44 |
| T3 | flash | fail | yes | 375 s | 23 | 24 | 156k / 1230k / 49k (44k) | $0.78 |
| T3 | sonnet | pass | yes | 27 s | 7 | 4 | 8+35k / 136k / 2k | $0.18 |
| T3 | opus | pass | yes | 32 s | 3 | 4 | 8+34k / 134k / 2k | $0.34 |
| T4 | carbon | pass | yes | 531 s | 20 | 11 | 169k / 1206k / 13k (9k) | $1.18 |
| T4 | argon | pass | yes | 191 s | 12 | 7 | 217k / 472k / 8k (4k) | $1.12 |
| T4 | flash | pass | yes | 268 s | 31 | 32 | 198k / 1856k / 32k (28k) | $0.82 |
| T4 | sonnet | pass | yes | 39 s | 5 | 6 | 12+41k / 239k / 3k | $0.22 |
| T4 | opus | pass | yes | 45 s | 7 | 8 | 16+43k / 337k / 4k | $0.49 |
| T5 | carbon | fail | no (limit) | 900 s | - | 24 | 415k / 3029k / 29k (25k) | $2.85 |
| T5 | argon | pass | yes | 428 s | 36 | 19 | 437k / 1530k / 23k (14k) | $2.52 |
| T5 | flash | pass | yes | 506 s | 64 | 65 | 301k / 4387k / 53k (43k) | $1.51 |
| T5 | sonnet | pass | yes | 31 s | 5 | 5 | 10+35k / 170k / 3k | $0.19 |
| T5 | opus | pass | yes | 53 s | 10 | 11 | 22+39k / 458k / 5k | $0.51 |
| T6 | carbon | pass | yes | 713 s | 4 | 5 | 71k / 38k / 4k (3k) | $0.37 |
| T6 | argon | pass | yes | 46 s | 2 | 3 | 24k / 33k / 2k (978) | $0.14 |
| T6 | flash | pass | yes | 51 s | 5 | 6 | 31k / 77k / 6k (5k) | $0.10 |
| T6 | sonnet | pass | yes | 10 s | 2 | 3 | 6+21k / 78k / 714 | $0.10 |
| T6 | opus | pass | yes | 17 s | 4 | 5 | 10+21k / 147k / 1k | $0.22 |

Antigravity spend on Google Cloud credits: $16.74 for these 18 runs, plus under
$0.30 for the engine-check turns before them and the uncounted Flash-Lite
monitor calls. That is well under the $40 budget.

## What each model did well and badly

### Gemini 4 Argon

- **Good:** Argon was the most dependable Gemini, and the fastest of the three
  on every task. Its T4 answer was as good as the Claude answers. It
  named `e_take`, `cl_set_model` and the `control_request` frame, showed
  `e_drift` and `e_note_start` side by side, and traced the `--model` value
  through `cd_claude_options`, `tr_claude_options`, `e_own`, `e_options` and
  `cl_build_args`. It gave both sides of the comparison (`"claude-opus-5-5"`
  against `""`) and named 7aa8d77f91.
- **Bad (T3):** it used `size * 100 <= budget * 102` correctly, but printed the
  same line as a failure for the warning. A release log would show
  "wasm-size: everglade2 is 975000 bytes, over its budget of 970000" with exit
  0, and a reader can't tell a warning from a failure.
- **Bad (T2):** it also widened the guard rows' yaw handling (`wire_wrap` in the
  guard reader). The change is sensible but was not asked for.
- **Cost:** 1.4 to 13 times Sonnet's list price per task. Its uncached input
  per run is large (24k to 437k tokens): each turn resends much of the context
  uncached.

### Gemini 3.8 Flash

- **Good:** Flash passed the same tasks as Argon, at the lowest Gemini cost
  ($0.87 per pass). It was as cheap as Sonnet on T1 and T6.
- **Bad:** it works in many small steps: 41 steps on T2, 64 on T5, 31 on T4.
  It reads files a piece at a time with `grep` and `sed`, so it was 2 to 5
  times slower than Argon on the larger tasks. On T5 it read 4.4M cached tokens.
- **Bad (T3):** the same miss as Argon. Its #test was thorough (boundaries at
  budgets of 50, 100 and 970,000), but the stderr line is the failure line.

### Gemini 4 Carbon

- **Good:** when Carbon finished, its work was right. It was the only Gemini to
  print a real warning on T3, and its T4 and T6 answers passed.
- **Bad:** it was slow and unpredictable. T6, a 12-line paragraph, took 713 s,
  and agy's log shows more than 4 minutes before the first Carbon reply. T2
  waited more than 5 minutes between two replies and timed out without editing
  anything. T5 timed out partway through the edit, with `RA_WIDTH` still used
  in six places. Carbon was released on 2026-11-10. The waits look like
  provider capacity, not the agent. They still decide whether Autopilot can use
  it.
- **Cost:** the highest per pass ($1.85), partly because the two timed-out runs
  still cost $4.43.

### Claude Sonnet 5.5 and Opus 5.5 (the baseline)

- Both passed all six, in 10 to 53 seconds, with 2 to 10 steps.
- Sonnet was the cheapest configuration overall ($0.16 per pass at list price).
- On T5 neither changed the Linux hosts. The prompt left that to their judgement
  ("if the hosts assume a 4096-wide atlas"). The known-good commit only made
  the hosts refuse an atlas that does not fit.

### Antigravity as an engine

- agy's turns ran fine through Coder: approvals, edits, diffs, usage lines, and
  `end` on every turn that finished.
- Two things to know:
  - agy writes long tool output to its own folder
    (`~/.gemini/antigravity-cli/brain/`) and reads it from there, outside the
    worktree.
  - A turn cut off by a limit writes no usage line, so Coder's ledger misses
    that spend. Its tokens are only in agy's conversation store.

## Cost per passing task

At list prices for Claude and on-demand catalog prices for Gemini, in this run
set:

| Configuration | Total | Passing tasks | Per passing task |
| --- | --- | --- | --- |
| Sonnet 5.5 | $0.93 | 6 | $0.16 |
| Opus 5.5 | $2.11 | 6 | $0.35 |
| Gemini 3.8 Flash (medium) | $4.34 | 5 | $0.87 |
| Gemini 4 Argon (medium) | $5.02 | 5 | $1.00 |
| Gemini 4 Carbon (medium) | $7.38 | 4 | $1.85 |

Claude's runs were on the owner's subscription, so their marginal cost is plan
quota, not dollars. Gemini's runs spend Google Cloud credits. Antigravity is
worth using when the Claude plan is the constraint, not to save money per task.

## Recommendation

### For Golemancy's rubric (`coax/lib/agent/tier.coax`)

- Keep the default: Haiku, Sonnet, Opus and Fable as they are. Nothing in these
  runs beats Sonnet at the Haiku and Sonnet tiers on pass rate, time or cost.
- Add Antigravity as a setting that swaps models, as #13193 plans, with
  `gemini-4-argon-medium` for the Haiku and Sonnet tiers. Mechanical edits,
  reading code, small bug fixes, single-unit features and docs are what Argon
  passed.
- Don't map the Opus or Fable tier to any Gemini yet. The only shared-engine
  task (T5) passed on Argon and Flash, but took 7 to 8 minutes against 30 to 50
  seconds, and no compiler task was run.
- Leave Flash out of the rubric. It passes about what Argon passes for slightly
  less money, but its 30- to 60-step runs cost more wall time than they save.
  One exception: a credits-only bulk pass of mechanical or doc edits.
- Leave Carbon out until its replies come back in seconds, not minutes. Measure
  it again in a week.
- Before any of this, fix the Coder bug in T4. If the Workshop's coding agent
  passes the tier's model through owner options, as `coder run` does, every
  Claude tier may be running on the CLI's default model today. That needs
  checking in `apps/everglade-desktop/next.coax`. Another agent is changing that
  file now, so it was not checked here.

### For Autopilot

- Use Antigravity (Argon) as the overflow engine when `coder usage` says Claude
  Code's plan is exhausted, not as the first choice.
- Give each Antigravity turn a time limit of 15 to 30 minutes. Count a turn that
  hits the limit as failed, and record its tokens from agy's conversation store,
  because no usage line comes.
- Keep a checking step after each Gemini turn that compares the change with the
  issue's stated behaviour. Both T3 failures were behaviour that passed their own
  #tests: the exit codes were right and the message was wrong.

## Why Carbon was slow (GitHub #13227)

Measured the same day on coderos-4080, Coder at cfa30d3534, agy 1.5.6.

### Where a run's time goes

agy keeps two records that split a run without a proxy:

- Its conversation store (`conversations/ID.db`, table `gen_metadata`) records
  each model call's time to the first token, its streaming time, and its
  tokens (uncached input, cached input, output, and the thinking part).
- Its ACP log (`log/acp-server-*.log`) records when each response began
  (`ResponseID`), each 429 and its retry, and each call to its safety monitor.

The time between one call's last token and the next call's request is
everything else: the tool run, agy's own work, and Coder's answer to a
permission request.

| Run | Wall | Waiting for the first token | Streaming | Between calls | Of which agy's safety monitor | 429s |
| --- | --- | --- | --- | --- | --- | --- |
| #13220, 40 Carbon calls | 1448 s | 524 s (36%) | 840 s (58%) | 81 s (6%) | 25 calls, 26 s | 1, 39 s |
| #13226 at 29 minutes, 53 calls | 1745 s | 1041 s (60%) | 509 s (29%) | 174 s (10%) | 29 calls, 33 s | 0 |

Coder answered agy's permission requests in a median of 10 ms (34 requests,
0.35 s in all, in #13220). Coder's engine is not where the time goes.

The time went to three things, all on the model's side.

1. **Long thinking at medium effort.** In #13220, two calls thought for 23k
   and 26k tokens before writing a file, and streamed for 289 s and 386 s at
   about 85 tokens a second. Those two calls are 675 s, 47% of the run. They
   are the 5- and 6-minute quiet gaps.
2. **Minutes before the first token.** Most calls start in 5 to 10 s.
   Some wait far longer for no reason in the request: 115 s and 86 s in
   #13220, 229 s (on 24k uncached tokens) and 157 s in #13226, 213 s in a
   one-line rename. These all came from the `global` location.
3. **A dropped prompt cache.** In `global`, a call sometimes resent the whole
   conversation uncached: in #13220, 3 calls of about 200k tokens each, 635k
   tokens that would have been 95% cheaper cached ($2.54 instead of $0.13),
   and in #13226, 2 calls and 263k tokens. Each of those calls also waited
   16 to 115 s for its first token.

The one 429 (`TPM quota exhausted. No spare capacity is currently available
to serve over-limit requests`) cost 39 s: 35 s for the refused request and
4 s before agy's retry, which then succeeded.

### What was ours, and fixed

- **A model call agy gave up on counted as a finished turn.** agy reports it
  as the turn's words (`⚠ Selected model is not supported in the selected
  location`, or a 429 after its retries), with `end_turn` and no usage.
  Coder logged `end finished` with 0 steps and exit 0, so a script or the
  Workshop took a failed turn for a done one. It now fails as `Antigravity:
  MESSAGE`, with the cause `provider` for quota and capacity, which starts
  routing's cooldown. Checked with the stand-in agy (`notice`).
- **No location was named.** `antigravity.env` on coderos-4080 named no
  `GOOGLE_CLOUD_LOCATION`, so agy used `global`. It now names `us`.

Nothing else in Coder's handling was slow. agy's safety monitor (a
`gemini-3.1-flash-lite` call of about 1 s before a shell command with
operators) is agy's own, about 2% of a run, and Coder has no switch for it.

### Location, effort and quota

Carbon and Argon are served in `global`, `us` and `eu`. Single regions
(`us-central1`, `us-east4`, `us-east5`, `us-south1`, `europe-west4`) answer
`404 NOT_FOUND`.

Each run below is one run of a task from this document's set, on Carbon at
medium effort unless named, all at the same time of day:

| Task | `global` | `us` | `us`, low effort |
| --- | --- | --- | --- |
| T1 rename | 301 s, 102 s | 62 s, 61 s | 95 s (`global`) |
| T6 doc paragraph | 81 s | 67 s | 202 s |
| T4 reading | 389 s | 280 s | 212 s |
| Calls waiting over 60 s for the first token | 2 of 32 (213 s, 108 s) | 0 of 29 (longest 22 s) | 3 of 18 (83 s, 54 s, 48 s) |
| Large cache misses | in #13220 and #13226 | none | none |

- **Location: use `us`.** In these runs it never waited a minute and never
  dropped the cache. `global` did both, here and in both issue runs. Four
  runs per location is a small sample, so the gap is a direction, not a
  guarantee.
- **Effort: low removes thinking, not the waits.** Low spent no thinking
  tokens (T4: 5.8k output against 14.9k, $1.00 against $1.28, streaming 21 s
  against 168 s), so #13220's two 5- and 6-minute calls would not happen.
  But the first-token waits stayed, and T6 took longer than at medium.
  Keep medium for code that needs judgement; use low for mechanical work.
- **Quota.** The 429 is Standard PayGo running out of shared capacity.
  Provisioned Throughput reserves capacity but is bought by the month, well
  over the $1,500 a month GCP budget, so it was not bought or measured. One
  429 in two long runs cost 39 s; agy's own retry covers it.

### Before and after

The same tasks as the first table, before (`global`, the task-fit runs
above, run 25 at a time) and after (`us`, the fixes above):

| Task | Carbon before | Carbon after | Argon before | Argon after |
| --- | --- | --- | --- | --- |
| T6 doc paragraph | 713 s | 67 s | 46 s | 32 s (`global`: 36 s) |
| T4 reading | 531 s | 280 s | 191 s | not rerun |

Part of the "before" is load: those runs went 25 at a time. Both "after"
Carbon runs passed (T6's paragraph was added in place; T4's EXPLAIN.md
names `e_take`, `set_model`, `e_drift` and `run_model`).

Spend for these experiments: $6.14 at the catalog prices above (17 runs and
two location probes), plus cents of agy's Flash-Lite monitor calls.

## Not run, and why

- **Codex:** its login answered "You've hit your usage limit … try again at
  Nov 13th, 2026 12:18 PM" on the first turn. It is left out.
- **A compiler task:** no published compiler build exists for the 2026-11-01
  compiler commits, and their bootstrap would exceed the time limit.
- **A multi-file refactor:** not run, to keep within time.
- **Repeats:** one run per cell. Two or three runs per cell would be needed
  before reading anything into a single pass or fail.

## Raw data

The raw `--stream-json` output, Coder session logs, Claude Code session logs,
agy ACP logs and conversation stores, diffs, check output and the harness
(`run.sh`, `run2.sh`, `checks/`, `metrics.py`, `tasks.md`) are kept outside the
repository on coderos-4080, at
`/tmp/claude-1000/-home-christopherdavid-openagents/bab287e8-af3b-4c52-9c34-241ada214efe/scratchpad/`.
They are not kept beyond this session's scratch space. The tables above are the
record.
