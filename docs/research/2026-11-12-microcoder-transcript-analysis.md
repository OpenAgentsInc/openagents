# Microcoder against Antigravity on the tuning set, step by step

> Committed 2026-11-12 from the analyst's scratch copy. Run-folder names refer to the
> scratch harness records of #13231 and #13325 (not in the repository). The design
> comparison it cites (`harness-design.md`) quotes Antigravity's prompt and is not
> committed; its findings are summarised in [the plan](../coder/2026-11-12-microcoder-plan.md).

Written 2026-11-12. Read-only analysis of recorded runs; no model was called.
Scope: tasks T1–T6 and H1–H8 (the tuning set), Argon and Carbon, plus
Microcoder's later builds on those tasks (`mcnext/harness/runs`, excluding
N1–N6) and the two #13263 runs. The unseen tasks N1–N6 were opened only
after both verdict rounds ended; they are in section 3b.

Everything here can be recomputed with the scratch tools in
`analysis/tools/` (paths below are relative to the scratchpad,
`/tmp/claude-1000/-home-christopherdavid-openagents/bab287e8-af3b-4c52-9c34-241ada214efe/scratchpad/`):

- `analysis/tools/mc.py RUN` prints a Microcoder run step by step from
  `worker.ajr` (each action's rationale, edits, commands, views and every
  result) joined with `stream.jsonl` (tokens, cache, effort, timing).
- `analysis/tools/agy.py RUN` prints an Antigravity run step by step from
  its brain transcript (`~/.gemini/antigravity-cli/brain/<engine_session>/.system_generated/logs/transcript_full.jsonl`):
  every model call's tokens, cached tokens and tool calls, and each result.
- `analysis/tools/stats.py` writes per-run behaviour counts to
  `analysis/stats.jsonl`; `analysis/tl/*.txt` holds every run's timeline;
  `analysis/pairs.md` is the pair table below; `analysis/mcg-metrics.jsonl`
  is `mcg/harness/metrics.py`'s output.
- `analysis/agy-system-prompt.txt` is Antigravity's system prompt as sent
  (recovered from `gen_metadata` in a run's `agy-conversation.db`).

Timing caveat: Coder flushes a Microcoder step's `attempt`, `tool` and
`result` lines together after the step's tools run, so a Microcoder "step
time" is model call plus that step's tools. Antigravity's transcript stamps
each model call and each tool result, so its model time can be separated
(1 s resolution). Round 4 and Carbon ran up to 8 runs at once, so absolute
seconds are noisy; counts of calls, reads, edits and tokens are not.

## 1. Summary

**Added after the verdict runs (section 3b): the unseen tasks N1–N6 confirm
the ranking and sharpen it.** Microcoder passed 8 of 12 against 10 of 12.
Its three wide-task failures (N3 v1, N4 v1, N4 v2) are the tuning set's
compaction thrash at full strength: 75–91 file reads a run, 83–85% of them
re-reads, 7–8 context shrinks, first edit at call 33–46 of 36–47, then the
128-command limit. Its passing N runs re-read 0–9 times (N3 v2: 35) and
edited by call 3–18. N1 v2 lost its 900 s to the same memory loss in
another form: a test-setup file it viewed at call 1 was compacted at call
2, so it guessed the setup API (`rs_reset`, `rs_setup`, `rs_seed`, none
exist) through four whole-file rewrites of 3–6K output tokens, 60–105 s
each; plus 4 failed edits and one 90 s silent stall. Microcoder's edit
failure rate on N was 23% (35 of 152) against Antigravity's 2% (3 of 151).
Antigravity's two failures were its own patterns: `coax mutate` after the
work was done (N1 v1, cut at 900 s) and three 120 s silences on
184K-token calls (N3 v1).

The two harnesses run the same model on the same endpoint
(`aiplatform.us.rep.googleapis.com`, project `openagentsgemini`), and in
practice with nearly the same tools: Antigravity used `read_file` (969
calls), `run_command` (554), `edit_file` (353) and `write_file` (23) across
86 runs, and nothing else of its ~16 declared tools but `manage_task` twice.
It has no search tool; it greps through `run_command` like Microcoder.

What differs is memory, request shape and what a result tells the model:

| | Microcoder | Antigravity |
| --- | --- | --- |
| Context per call (Carbon) | 29,390 input tokens, capped by the 256 KB journal request (~64K tokens); recent results 144 KB, then compacted to 24 KB | 100,586 input tokens, grows to 180–230K; never shrinks (3 shrinks in 822 Carbon calls) |
| Cached share of input | 37% (Argon r4), 48% (Carbon) | 79% (Argon r4), 91% (Carbon) |
| Cache hit when the prompt grows < 1K tokens | 41% of calls | 98% of calls |
| Context shrinks (compactions) | 98 in 673 Carbon calls; 35 in 379 Argon r4 calls | 3 in 822; 1 in 166 |
| Reads of a file already read in the run | 66% of reads (Carbon), 39% (Argon r4) | 33% (Carbon), 25% (Argon r4), mostly the next 800-line page |
| File view | whole file, 64 KB, **no line numbers**, no range | 800 lines / 46 KB a page, **numbered**, `StartLine`/`EndLine` |
| Long command output | head 6 KB + tail 6 KB in state; the middle is lost | truncated, and the full output saved to a file it can page through |
| What an applied edit returns | `{"edit": path, "applied": true}`; the edit's text is not kept anywhere in the state | "Made 1 replacement ... starting at line 510 (+17/-3 lines)", and its own call (with the replacement text) stays in the conversation |
| Edit failures | 39 of 563 (7%) | 1 of 374 (0.3%) |
| Tools per model call | 1.8 (Argon r4) – 2.4 (Carbon) | 1.3 – 1.4 |
| System prompt | ~1.1K tokens of coding workflow | ~12.5K tokens: identity, artifacts, subagents, skills, and the repo's AGENTS.md verbatim as "user rules"; no coding workflow at all |

Microcoder's main losses on the tuning set are one pathology in several
forms: **its working memory is small and forgets file contents, so on wide
tasks (and on Carbon especially) it reads, forgets, and reads again**
(compaction thrash), and every forgetting also breaks the cache, which is
where most of its money goes. Its edits fail when it changes a file a
second time, because the state never shows it what it wrote. Its wins come
from batching (edit + test + diff in one step), a short loop on small tasks,
and not doing repository rituals Antigravity does because AGENTS.md is in
its prompt (`coax mutate` in 28 of 86 Antigravity runs; 4 Carbon limit hits).

The biggest single lever is cost, not passes: 80% of Microcoder's spend is
input tokens and 52–63% of those were not cached. At Antigravity's cache rate
Microcoder's Carbon arm would have cost about $21 instead of $54, and its
Argon round 4 about $8.50 instead of $27.37 (section 5).

## 2. Outcomes on the tuning set, with the judge's faults removed

Most pass differences on these 14 tasks are the checks', as the -next doc
found. Read run by run:

| Task | What decided failures | Harness fault? |
| --- | --- | --- |
| H8 | Every run of both harnesses defines both constants and passes the hidden test; the old check wanted each constant at the start of a line. All 14 recorded H8 diffs contain `ENV_GLADE_DAY_ID ::` and `ENV_GLADE_DAY_KEYS ::` | No (judge) |
| T6 | "one commit per issue" wrapped across a line (MC w1, w2; agy f1, w1, c3) | No (judge) |
| H5 | MC w1 wrote the value in bold code; the regex misses it | No (judge) |
| H4 | regex facts: MC s1/s2 miss F1's expression form; agy f1 misses F2; MC carbon c1 misses F3's wording; MC carbon c3 omits `cast.coax` (a real miss, see P16) | Mostly judge; c3 is a search miss |
| T3 | the warning line does not contain "warn": MC f1, s2, w1, w3; agy w1. No run of either harness, except MC next-r1, ran the command it changed | Model judgment, both sides |
| T4 | MC s2 wrote a 22-word EXPLAIN.md and put the answer in its reply | MC, once in ~20 reading runs |
| T5 Carbon | MC c2 cut at 900 s with **no edit** (compaction thrash, P12); agy c2 cut at 900 s while running `coax mutate` and `components-check` | Both, different causes |
| H6 Argon | MC w2 hit 900 s after it had finished once: the medium re-ask un-finished it and it polled a background test (P1) | MC (round 4's re-ask, since reverted) |
| H4 / H7 Carbon | agy c1, c2 (H4) and c2 (H7) cut at 900 s: reading 69 files at 180K tokens a call; gold-plating with `coax mutate` | Antigravity |

So on real behaviour the tuning set separates the harnesses mainly by cost,
time and limit endings, not by passes. The pass-relevant weaknesses show up
as limit endings on wide tasks (T5 Carbon, #13263 twice, and per the -next
doc N3 and N4 on the command limit).

## 3. Matched pairs

Same task, same model, same round where one exists (round 4 `w*` for Argon,
`c*` for Carbon). "Cmds" counts shell commands; "Views/reads" counts
Microcoder `view` entries or Antigravity `read_file` calls (Microcoder's
`sed`/`cat` reads are commands). "Repeated" is the same command text, or the
same view (path and range), run again in the run.

| Pair | What | Run | Verdict | Wall s | Calls | Cost $ | Input tok/call | Cached | Tools/call | Cmds | Views/reads | Edits (failed) | Repeated cmds / reads | Test after last edit | Diff after last edit |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P1 | H6 Argon r4: both pass, MC hits the limit | `h6-mc-argon-w2` | PASS (limit) | 900 | 25 | 1.77 | 30,871 | 53% | 1.7 | 31 | 3 | 10 (2) | 7 / 0 | yes | yes |
| P1 |  | `h6-agy-argon-w1` | PASS | 155 | 13 | 1.12 | 81,328 | 80% | 1.1 | 4 | 6 | 4 (0) | 1 / 1 | yes | no |
| P2 | H6 Argon r4: both pass, MC 5x slower | `h6-mc-argon-w1` | PASS | 771 | 23 | 1.36 | 30,099 | 59% | 2.3 | 42 | 3 | 7 (1) | 11 / 0 | yes | yes |
| P2 |  | `h6-agy-argon-s1` | PASS | 184 | 12 | 1.08 | 79,963 | 79% | 1.1 | 3 | 5 | 5 (0) | 0 / 0 | yes | no |
| P3 | H7 Argon r4: both pass, MC 6x faster | `h7-mc-argon-w3` | PASS | 60 | 5 | 0.28 | 11,370 | 0% | 1.2 | 4 | 1 | 1 (0) | 1 / 0 | yes | yes |
| P3 |  | `h7-agy-argon-w1` | PASS | 384 | 19 | 1.05 | 36,407 | 83% | 0.9 | 7 | 1 | 10 (0) | 4 / 0 | no | yes |
| P4 | T2 Argon r4: both pass, MC 2.5x cost | `t2-mc-argon-w1` | PASS | 354 | 25 | 1.58 | 34,306 | 60% | 1.6 | 36 | 0 | 3 (0) | 1 / 0 | yes | yes |
| P4 |  | `t2-agy-argon-w1` | PASS | 93 | 10 | 0.64 | 47,584 | 72% | 0.9 | 2 | 4 | 2 (0) | 0 / 0 | yes | yes |
| P5 | T2 Argon: both pass, MC cheaper | `t2-mc-argon-w2` | PASS | 165 | 6 | 0.49 | 23,834 | 25% | 1.8 | 5 | 3 | 3 (0) | 0 / 0 | yes | yes |
| P5 |  | `t2-agy-argon-f2` | PASS | 182 | 11 | 1.28 | 79,755 | 72% | 1.1 | 2 | 8 | 2 (0) | 0 / 0 | yes | yes |
| P6 | T3 Argon: MC fails (no "warn"), agy passes | `t3-mc-argon-w3` | FAIL | 143 | 4 | 0.21 | 10,117 | 0% | 1.2 | 2 | 1 | 2 (0) | 0 / 0 | yes | yes |
| P6 |  | `t3-agy-argon-f1` | PASS | 71 | 7 | 0.45 | 35,796 | 63% | 0.9 | 2 | 2 | 2 (0) | 0 / 0 | yes | yes |
| P7 | T3 Argon r4: both fail | `t3-mc-argon-w1` | FAIL | 93 | 6 | 0.42 | 19,592 | 20% | 1.2 | 3 | 2 | 2 (0) | 0 / 0 | yes | yes |
| P7 |  | `t3-agy-argon-w1` | FAIL | 101 | 7 | 0.66 | 55,451 | 63% | 1.3 | 3 | 5 | 1 (0) | 1 / 0 | yes | yes |
| P8 | H4 Argon: MC passes cheaply, agy fails | `h4-mc-argon-w3` | PASS | 212 | 6 | 0.44 | 20,662 | 26% | 1.6 | 6 | 4 | 1 (0) | 0 / 0 | no | yes |
| P8 |  | `h4-agy-argon-f1` | FAIL | 203 | 20 | 1.94 | 125,913 | 86% | 2.3 | 10 | 35 | 1 (0) | 0 / 0 | no | yes |
| P9 | H4 Argon: MC fails, agy passes | `h4-mc-argon-s2` | FAIL | 201 | 16 | 1.62 | 33,273 | 31% | 2.9 | 41 | 4 | 1 (0) | 2 / 0 | no | yes |
| P9 |  | `h4-agy-argon-f2` | PASS | 275 | 16 | 1.40 | 59,349 | 73% | 1.2 | 8 | 11 | 1 (0) | 0 / 0 | no | yes |
| P10 | T4 Argon r2: MC fails (answer in reply), agy passes | `t4-mc-argon-s2` | FAIL | 57 | 6 | 0.63 | 25,182 | 0% | 2.0 | 7 | 4 | 1 (0) | 0 / 0 | no | yes |
| P10 |  | `t4-agy-argon-f2` | PASS | 164 | 8 | 1.23 | 115,022 | 76% | 2.0 | 3 | 12 | 1 (0) | 0 / 0 | no | yes |
| P11 | T5 Argon r4: both pass | `t5-mc-argon-w3` | PASS | 235 | 10 | 0.88 | 18,409 | 25% | 2.5 | 9 | 5 | 11 (7) | 3 / 1 | yes | yes |
| P11 |  | `t5-agy-argon-w1` | PASS | 301 | 17 | 1.58 | 93,592 | 81% | 2.2 | 5 | 22 | 8 (0) | 0 / 1 | yes | yes |
| P12 | T5 Carbon: MC cut at limit, agy passes | `t5-mc-carbon-c2` | FAIL (limit) | 900 | 22 | 2.99 | 33,616 | 27% | 3.8 | 34 | 54 | 0 (0) | 0 / 37 | no | no |
| P12 |  | `t5-agy-carbon-c1` | PASS | 585 | 48 | 2.84 | 153,171 | 96% | 1.5 | 27 | 20 | 17 (0) | 8 / 1 | yes | yes |
| P13 | T5 Carbon: both pass | `t5-mc-carbon-c1` | PASS | 329 | 29 | 2.57 | 30,265 | 38% | 3.1 | 40 | 41 | 10 (0) | 10 / 24 | yes | yes |
| P13 |  | `t5-agy-carbon-c3` | PASS | 856 | 49 | 3.39 | 180,578 | 97% | 1.6 | 29 | 28 | 19 (0) | 9 / 1 | yes | yes |
| P14 | T2 Carbon: both pass, MC 2.5x cost | `t2-mc-carbon-c1` | PASS | 465 | 30 | 2.67 | 36,071 | 50% | 2.3 | 54 | 10 | 4 (0) | 2 / 5 | yes | yes |
| P14 |  | `t2-agy-carbon-c1` | PASS | 204 | 22 | 1.05 | 90,736 | 93% | 0.9 | 4 | 11 | 5 (0) | 0 / 0 | yes | yes |
| P15 | H4 Carbon: MC passes, agy cut at limit | `h4-mc-carbon-c2` | PASS | 497 | 32 | 3.10 | 34,703 | 41% | 3.4 | 96 | 12 | 1 (0) | 0 / 2 | no | yes |
| P15 |  | `h4-agy-carbon-c1` | FAIL (limit) | 900 | 36 | 6.20 | 125,622 | 72% | 2.4 | 15 | 69 | 0 (0) | 0 / 0 | no | no |
| P16 | H4 Carbon: both fail | `h4-mc-carbon-c3` | FAIL | 596 | 37 | 3.22 | 35,688 | 52% | 3.0 | 88 | 20 | 2 (0) | 2 / 1 | no | yes |
| P16 |  | `h4-agy-carbon-c3` | FAIL | 687 | 31 | 3.20 | 152,595 | 89% | 2.9 | 11 | 71 | 0 (0) | 0 / 0 | no | no |
| P17 | H6 Carbon: both pass, MC 3x slower | `h6-mc-carbon-c2` | PASS | 719 | 31 | 1.99 | 34,952 | 65% | 2.6 | 69 | 3 | 8 (1) | 9 / 0 | yes | yes |
| P17 |  | `h6-agy-carbon-c3` | PASS | 243 | 20 | 1.20 | 100,653 | 93% | 1.4 | 9 | 10 | 8 (0) | 1 / 1 | yes | no |
| P18 | H7 Carbon: MC passes, agy cut at limit | `h7-mc-carbon-c2` | PASS | 538 | 10 | 0.95 | 23,634 | 52% | 2.0 | 13 | 2 | 5 (1) | 9 / 1 | yes | yes |
| P18 |  | `h7-agy-carbon-c2` | PASS check, FAIL rule (limit) | 900 | 22 | 0.74 | 44,617 | 91% | 1.0 | 7 | 7 | 9 (0) | 4 / 0 | yes | no |
| P19 | H1 Carbon: both pass, MC fewer calls | `h1-mc-carbon-c1` | PASS | 72 | 5 | 0.31 | 12,302 | 0% | 1.6 | 5 | 1 | 2 (0) | 2 / 0 | yes | yes |
| P19 |  | `h1-agy-carbon-c2` | PASS | 248 | 15 | 0.69 | 40,300 | 82% | 1.0 | 5 | 3 | 6 (0) | 1 / 0 | yes | yes |
| P20 | T6 Carbon: MC passes, agy fails (judge) | `t6-mc-carbon-c3` | PASS | 134 | 7 | 0.35 | 19,020 | 51% | 1.1 | 3 | 3 | 2 (0) | 1 / 0 | no | yes |
| P20 |  | `t6-agy-carbon-c3` | FAIL | 96 | 5 | 0.30 | 32,272 | 73% | 1.0 | 1 | 3 | 1 (0) | 0 / 0 | no | yes |

Run folders are under `mcg/harness/runs/`. All Microcoder runs above are the
round 2 build (78ab78f450, `s*`) or the round 4 build (c2e1e45103, `w*`,
`c*`: medium plan and medium finish re-ask).

### P1 and P2: H6 Argon (Microcoder 23–25 calls, 771–900 s; Antigravity 12–13 calls, 155–184 s)

`h6-mc-argon-w2` (timeline `analysis/tl/mcg-h6-mc-argon-w2.txt`):

1. Call 1 (medium plan): views `nipkeys.coax` (65,511 B, cut to 64 KB) and
   `nipkeys_sign.coax` (21 KB).
2. Calls 2–3: `sed -n '550,$p'` (cut to head+tail: 11,832 B shown), a 16 KB
   fixtures view, `sed -n '1100,1220p'`. Input 40.5K → 55.6K tokens.
3. **Call 4: input drops to 7,357 tokens.** The 144 KB log filled; all but
   the last 24 KB were compacted: both whole-file views became `{"view":
   path, "bytes": N}`.
4. Calls 4–9 re-read the same file in guessed ranges: `1221,$`, `1500,1650`,
   `900,1010`, `700,900`, `1,180`, then a grep. The rationales say why: "to
   see KyEntry, KyLedger, ky_consume" — what call 1 had shown whole. Six calls,
   ~240 s (step times 35, 58, 45, 82, 28, 8 s).
5. Call 10: writes a failing test first. Calls 11 and 15: **edits of its
   own test fail** ("old text is not in the file"); each costs a `sed` call
   (12, 16) to see what it wrote. Calls 17–18 fight the check's `#steps`
   budget.
6. Call 19: the fix. Calls 20–23: the full test needs more than the 30 s
   command limit, so it backgrounds it and polls (`sleep 25`).
7. Call 24 (794 s): finished at low. **Call 25, the medium re-ask, does not
   finish:** it starts a second background test, and call 26 polls with three
   `sleep 25`s until the 900 s limit.

`h6-agy-argon-w1`: reads `nipkeys.coax` in four numbered pages (800 lines /
46 KB each) and the fixtures in calls 1–4; context 91K tokens, kept for the
whole run, 80–97% cached from call 4 on. Edits the test (call 5), runs only
that test with `--only "idempotency"` (call 6), two fix edits (7–8), the
single test again (9), extends the test (10), then starts the full suite and
`check` as background tasks (11) and ends its turn; the system wakes it when
they finish (12–13). 13 calls, $1.12, 155 s. No re-read, no failed edit.

What the passing-fast side did: kept the file in context (cached), never
re-read, targeted one test, and let long tests run in the background.
Microcoder's later builds closed part of this: with the economy prompt and
120 s commands (`mcnext/.../h6-mc-argon-c1`, `-e1`) it took 7 calls, $0.77–0.87,
264–285 s, still 1.7x Antigravity's time, with one failed edit (call 5).

### P3: H7 Argon (Microcoder 5 calls, 60 s; Antigravity 19 calls, 384 s)

`h7-mc-argon-w3`: call 1 runs the tests and views `json.coax`; call 2 writes
`jv_write` with its `#test` in one 4 KB edit, runs the tests and `git diff`
in the same step; call 3 `git status`; call 4 finishes; call 5 is the
medium re-ask. Five calls, $0.28.

`h7-agy-argon-w1`: one tool per call. It writes a larger test, fails it,
fixes, reruns (calls 2–8), then rewrites its escape helper four times (calls
9, 11, 16, 17, each 4–4.5K output tokens), and runs `coax mutate` (call 10)
because AGENTS.md is in its prompt. 19 calls, $1.05.

What the winning side did: batched edit + test + diff in one step, wrote the
minimum the task asked, and stopped.

### P4 and P5: T2 Argon (Microcoder 6–25 calls)

`t2-mc-argon-w1`: 21 calls of searching before the first edit, mostly one
grep per call (calls 2, 4, 5, 6, 7, 8, 10, 11 each run one command) looking
for an existing angle-wrap helper. `wrap_angle` is grepped six times in
different forms (calls 8, 9, 11, 13, 14 and others), several with `| head -n
30/40` or cut at 12 KB. The rationales are plans ("Check how..."), never
findings. Input shrinks at call 15 (49.5K → 17.5K) and it re-reads lines
710–860 it had read at call 3. It edits at call 22, passes. 25 calls, $1.58,
354 s.

`t2-agy-argon-w1`: reads the target file in two pages, `everglade-net-core`
and the first 250 lines of `everglade-wire`, all numbered (calls 1–4), finds
`wire_wrap`, edits twice, runs the named check, `git diff`, done. 9 calls,
$0.64, 93 s.

`t2-mc-argon-w2` shows the same harness at its best: 6 calls, $0.49, 165 s,
cheaper than every Antigravity T2 run. The spread (6 to 25 calls, $0.40 to
$1.58 across Microcoder's T2 runs) is the cost of the search-and-forget
pattern when the model gets lost; Antigravity's spread is 10–11 calls.

### P6 and P7: T3 Argon (both fail the same way)

Every Microcoder and Antigravity T3 run does the same thing: view
`wasm-size.coax` (Antigravity also `wasm.sh`, sometimes `release.sh`,
`wasm-budgets.txt` and `INVARIANTS.md`), edit, run the file's tests, read
`git diff`, finish. Whether the warning line says "warning" is the model's
word choice: MC w3 kept the error's "over its budget" text; agy f1 wrote
`"wasm-size: warning: "`; agy w1 did not. The only run of either harness
that executed the command it changed (`next-t3-mc-argon-r1`, a temp budget
file and `truncate -s`) passed. No harness difference; a general "exercise
the changed behaviour once" habit would help both, but the evidence is one
run.

### P8 and P9: H4 Argon (reading task, judged by regex)

The failures here are regex misses (section 2). The process differs:
Antigravity reads 35 files whole (`h4-agy-argon-f1`, 126K tokens a call,
$1.94); Microcoder greps and reads ranges (`h4-mc-argon-w3`, 6 calls,
$0.44). On cost per pass Microcoder wins H4 Argon in round 4 (3 of 3 at
$0.44–1.29 against 2 of 3 at $1.40–2.77).

### P10: T4 Argon round 2 (Microcoder's one answer-placement fail)

`t4-mc-argon-s2` wrote a 22-word EXPLAIN.md and put the whole answer in its
`reply`. The schema's `reply` field invites it. Once in about 20 Microcoder
reading-task runs; not seen in round 4 or later builds.

### P11: T5 Argon r4 (both pass; Microcoder's blind edits)

`t5-mc-argon-w3`: call 1 views three files (14.8 + 43 + 8.8 KB) and greps;
call 2 views a 22 KB file; call 3 greps. **Call 4's input is 4,471 tokens**
(from 47,609): everything it had read was compacted. In the same call it
writes eight edits to `atlas.coax` from memory: **seven fail**. Call 5 views
the file again; call 6 rewrites an 8.4 KB block as one edit (8,654 output
tokens, 106 s); call 7 fixes a test; 8–10 diff and finish. 10 calls, $0.88.
`t5-agy-argon-w1`: 17 calls, $1.58, no failed edit; 22 numbered page reads.

### P12 and P13: T5 Carbon (Microcoder cut at 900 s with no edit; Antigravity passes)

`t5-mc-carbon-c2` (`analysis/tl/mcg-t5-mc-carbon-c2.txt`) is the clearest
case of compaction thrash. It views `atlas.coax`,
`apps/openagents-desktop/linux/main.c` and `components/gallery/linux/main.c`
together at calls 1, 4, 6, 8, 11, 14, 17, 19 and 22 (nine times), each time
with other 20–40 KB files, so each step's results push the previous ones
out. It never edits. Calls 9, 13, 16, 18, 20, 21 take 95–140 s each (Carbon
thinking at "low": up to 11.7K thinking tokens a call). Cut at 900 s, $2.99.

`t5-agy-carbon-c1`: 48 calls, context grows to ~153K tokens a call at 96%
cached, 17 edits, none failed, passes at 585 s for $2.84.

`t5-mc-carbon-c1` passed (329 s, 29 calls), but with 24 repeated views.

### P14: T2 Carbon (Microcoder $2.67, 30 calls; Antigravity $1.05, 22 calls)

`t2-mc-carbon-c1`: views `everglade2-net.coax` (50 KB) at calls 1, 4, 6 and
8; the input goes 36K → 47K → 34K → 66K → 39K → 69K → 14K → 54K → 5K
tokens as views push each other out. From call 10 it reads the same file in
`sed` ranges instead (calls 10–26, `1,250p`, `251,480p`, `621,1050p`,
`720,950p`, `780,865p`, `455,515p`, `715,785p`, `515,610p`, `700,725p`,
`950,1250p`, `350,460p`, `610,705p`, `250,355p`, `1250,1500p`, `520,565p`,
`715,850p`, ...). First edit at call 27. Three calls think 1,400–2,800
tokens. `t2-agy-carbon-c1` reads the same files in numbered pages and keeps
them.

### P15 and P16: H4 Carbon

Microcoder passed once (c2) and failed twice on regex facts; in c3 the miss
is real: its answer omits `cast.coax` from the files that define
`wrap_angle`. Its first repository-wide `grep -rn "wrap_angle" .` returned
more than 12 KB and was cut to head and tail, so the middle of the list was
lost; it then ran nine differently filtered `wrap_angle` greps across the
run (`analysis/tl/mcg-h4-mc-carbon-c3.txt`, lines 3–104). Antigravity read
69 files and grew to 180K tokens a call; two of its three runs hit the
limit, and its context stopped caching near the end (calls 33 and 35: 172K
and 177K uncached tokens), which is why `h4-agy-carbon-c1` cost $6.20.

### P17: H6 Carbon (Microcoder 719 s, 31 calls; Antigravity 243 s, 20 calls)

`h6-mc-carbon-c2`: 18 calls of `sed` ranges before the first edit, the same
regions again and again across compactions: lines 1–200 at call 6, 1–55 at
11, 45–85 at 16, 1–45 at 18; 830–875 at 11, 835–865 at 16, 840–885 at 23;
1540–1630 at 5, 1560–1625 at 10, 1580–1625 at 16, 1600–1645 at 20. Input
oscillates 55K → 8.7K, 49K → 12K, 51K → 19K tokens.

### P18: H7 Carbon (Antigravity's own failure mode)

`h7-agy-carbon-c2`: tests pass at call 20 (818 s), then it runs a filtered
`coax mutate` (call 21), and reads code again (22); cut at 900 s. Its
earlier calls paged through a saved 501-line test output. The behaviour
comes from AGENTS.md ("run `coax mutate` on it with zero survivors").
Microcoder's c2 passed with 10 calls.

### P19: H1 Carbon (Microcoder's short loop)

`h1-mc-carbon-c1`: 5 calls in 72 s (plan, edit + test + diff, finish,
re-ask). Antigravity: 9–15 calls, 98–248 s. All six pass.

### The #13263 runs (Carbon, real issue, both ended on a limit with no edit)

- `aaa-13263/` (round 4 build): 17 calls, 75 views, 53 commands, ended on
  the journal/run budget (from the -next doc; this folder has no journal).
- `mcnext/aaa/carbon1/` (seed, read budget, economy, 120 s): 28 calls, 10 of
  them with a shrunken context, 141 file reads of which 98 re-read a file
  already read. Its rationales from call 6 say so: "Let's check all files we
  viewed in compacted steps (everglade/README.md, ...)", "Let's read the
  files that were viewed in earlier compacted steps", "Read ... without
  truncation". `everglade/README.md` is read at calls 1, 6, 9, 11, 13, 15
  and 20. It batches 5–9 `cat`/`sed` reads a call, so one step fills the
  144 KB window by itself. The economy prompt's advice (search first, read
  ranges, write findings in the rationale) did not change this on Carbon.

### Every Microcoder run on the tuning set that hit a limit or timed out

| Run | Build | Ending | Cause |
| --- | --- | --- | --- |
| `h6-mc-argon-w2` | round 4 | 900 s (check passed) | compaction re-reads (calls 4–9), self-edit failures (11, 15), 30 s polling, then the medium re-ask un-finished a finished run |
| `t5-mc-carbon-c2` | round 4 | 900 s, no edit | compaction thrash (nine views of the same three files) and 95–140 s thinking calls |
| `h3-mc-carbon-c1` | round 4 | no `end` line at 406 s (check passed) | last step killed a hung `file-lock` helper with `pkill -9`; the stream ends without an ending. Not a time limit |
| `h4-mc-argon-f2` | ee6c2215d0 | crash: `state.coax:254: slice out of bounds` | fixed in 941212f68f |
| `t5-mc-argon-b` | early | journal request over 256 KB | fixed in ee6c2215d0 |
| `aaa-13263`, `mcnext/aaa/carbon1` | round 4; economy build | run budget; command limit (128), no edit | as above |

Long Microcoder runs that finished but took more than 600 s: H6 Argon s2
(676 s), w1 (771 s); H6 Carbon c1 (608 s), c2 (719 s); H4 Carbon c1 (602 s);
H3 Carbon c3 (606 s). All show the same re-read pattern.

## 3b. The unseen tasks N1–N6 (verdict rounds v1 and v2, Argon low)

Runs in `mcnext/harness/runs/n*-{mc,agy}-argon-v{1,2}`; prompts and checks
in `mcnext/unseen/`. Both builds as defined in the -next doc (Microcoder
93455e7032 rebased: seed, read budget, economy prompt, 120 s commands, no
thinking switches). Each pair started together. Timelines:
`analysis/tl/next-n*-*.txt`.

| Task | Harness | Tag | Wall s | Calls | Cost $ | Input tok/call | Cached | Verdict |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| N1 | Antigravity | v1 | 900 | 23 | ≥ 2.34 (store) | | | check passed; FAIL by the limit rule |
| N1 | Antigravity | v2 | 654 | 31 | 3.93 | 117,375 | 79% | PASS |
| N1 | Microcoder | v1 | 421 | 17 | 1.42 | 26,849 | 37% | PASS |
| N1 | Microcoder | v2 | 900 | 16 | 1.83 | 29,950 | 35% | FAIL (limit) |
| N2 | Antigravity | v1 | 526 | 24 | 1.81 | 110,206 | 89% | PASS |
| N2 | Antigravity | v2 | 528 | 17 | 1.22 | 78,866 | 85% | PASS |
| N2 | Microcoder | v1 | 297 | 7 | 0.76 | 25,086 | 12% | PASS |
| N2 | Microcoder | v2 | 330 | 7 | 0.93 | 26,241 | 12% | PASS |
| N3 | Antigravity | v1 | 567 | 8 | 3.45 | 184,057 | 44% | FAIL (stalls) |
| N3 | Antigravity | v2 | 857 | 25 | 3.97 | 148,776 | 80% | PASS |
| N3 | Microcoder | v1 | 711 | 36 | 3.84 | 39,593 | 41% | FAIL (command limit) |
| N3 | Microcoder | v2 | 570 | 26 | 2.64 | 35,103 | 38% | PASS |
| N4 | Antigravity | v1 | 683 | 21 | 3.60 | 148,548 | 78% | PASS |
| N4 | Antigravity | v2 | 525 | 14 | 1.62 | 147,151 | 88% | PASS |
| N4 | Microcoder | v1 | 685 | 45 | 4.04 | 39,051 | 50% | FAIL (command limit) |
| N4 | Microcoder | v2 | 895 | 47 | 4.02 | 36,374 | 51% | FAIL (command limit at 895 s) |
| N5 | Antigravity | v1 | 595 | 36 | 2.52 | 124,229 | 92% | PASS |
| N5 | Antigravity | v2 | 304 | 14 | 1.77 | 128,087 | 83% | PASS |
| N5 | Microcoder | v1 | 377 | 18 | 1.40 | 32,351 | 52% | PASS |
| N5 | Microcoder | v2 | 493 | 14 | 1.62 | 34,122 | 33% | PASS |
| N6 | Antigravity | v1 | 806 | 31 | 2.22 | 102,424 | 91% | PASS |
| N6 | Antigravity | v2 | 617 | 25 | 2.52 | 65,063 | 73% | PASS |
| N6 | Microcoder | v1 | 431 | 18 | 1.57 | 28,273 | 44% | PASS |
| N6 | Microcoder | v2 | 533 | 22 | 1.66 | 32,980 | 58% | PASS |

Behaviour per run (Microcoder: from the journal; "reads" are views plus
`sed`/`cat`/`head`/`tail` of a file, "re-reads" are reads of a file already
read in the run; "stalls" are calls the gateway cut after 90 s of silence
and retried; "big" are calls writing over 2,500 output tokens):

| Run | Verdict | Calls | Reads | Re-reads | Shrinks | First edit (call) | Edits (failed) | Commands | Stalls | Big |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| n1 mc v1 | PASS | 17 | 15 | 6 | 2 | 7 | 12 (2) | 39 | 1 | 1 |
| n1 mc v2 | FAIL | 16 | 15 | 5 | 3 | 8 | 15 (4) | 40 | 1 | 4 |
| n2 mc v1 | PASS | 7 | 6 | 0 | 1 | 3 | 8 (1) | 14 | 1 | 1 |
| n2 mc v2 | PASS | 7 | 7 | 0 | 1 | 3 | 12 (3) | 21 | 0 | 1 |
| n3 mc v1 | FAIL | 36 | 75 | 62 | 7 | 33 | 6 (2) | 119 | 3 | 1 |
| n3 mc v2 | PASS | 25 | 45 | 35 | 5 | 18 | 24 (3) | 85 | 0 | 2 |
| n4 mc v1 | FAIL | 45 | 91 | 77 | 8 | 46 | 5 (4) | 118 | 1 | 2 |
| n4 mc v2 | FAIL | 46 | 85 | 69 | 7 | 42 | 10 (1) | 113 | 0 | 2 |
| n5 mc v1 | PASS | 18 | 18 | 9 | 3 | 7 | 17 (1) | 44 | 0 | 1 |
| n5 mc v2 | PASS | 14 | 19 | 9 | 3 | 8 | 22 (5) | 61 | 0 | 1 |
| n6 mc v1 | PASS | 18 | 28 | 8 | 2 | 10 | 7 (1) | 43 | 0 | 3 |
| n6 mc v2 | PASS | 22 | 29 | 9 | 3 | 13 | 14 (8) | 67 | 0 | 1 |

| Run | Verdict | Calls | Tools/call | Reads (with a range) | Repeated reads | Edits (failed) | Commands | Ran `coax mutate` |
| --- | --- | ---: | ---: | --- | ---: | --- | ---: | --- |
| n1 agy v1 | FAIL (limit) | 22 | 1.8 | 20 (20) | 0 | 8 (0) | 11 | yes, calls 15–22 |
| n1 agy v2 | PASS | 27 | 2.1 | 35 (31) | 0 | 10 (0) | 11 | yes |
| n2 agy v1 | PASS | 24 | 1.3 | 14 (2) | 0 | 9 (1) | 8 | no |
| n2 agy v2 | PASS | 17 | 1.2 | 7 (0) | 0 | 7 (0) | 7 | no |
| n3 agy v1 | FAIL | 8 | 2.5 | 18 (18) | 0 | 1 (0) | 1 | no |
| n3 agy v2 | PASS | 24 | 2.1 | 28 (28) | 3 | 17 (0) | 6 | no |
| n4 agy v1 | PASS | 21 | 2.0 | 23 (23) | 0 | 14 (2) | 4 | no |
| n4 agy v2 | PASS | 14 | 2.2 | 18 (18) | 0 | 10 (0) | 3 | no |
| n5 agy v1 | PASS | 30 | 1.4 | 13 (2) | 0 | 17 (0) | 12 | yes |
| n5 agy v2 | PASS | 13 | 2.4 | 13 (3) | 0 | 14 (0) | 4 | no |
| n6 agy v1 | PASS | 28 | 1.5 | 21 (2) | 0 | 8 (0) | 14 | yes |
| n6 agy v2 | PASS | 18 | 2.1 | 15 (0) | 0 | 9 (0) | 14 | yes |

The split is clean: each of the three Microcoder runs that ended on the
command limit has 62–77 re-reads, 7–8 shrinks and a first edit after call
33; every pass but N3 v2 has at most 9 re-reads and a first edit by call
13. The fourth failure (N1 v2) has few re-reads but lost a file it needed
to compaction after one call (below). Antigravity never
re-read a range (0 repeated reads in 10 of 12 runs).

### N4: Microcoder failed both rounds, Antigravity passed both

The task: make the desk's sidebar and tabs build display lists in a skin;
touch `desk.coax` (1,237 lines), `skin.coax` and four skin files; keep
374 checks and the frame's vertices unchanged.

**Antigravity `n4-agy-argon-v1`** (`analysis/tl/next-n4-agy-argon-v1.txt`):

1. Call 1: seven `read_file`s in parallel: `skin.coax`, the four skins,
   `paint/list.coax`, `composer.coax` (first 800 lines). Every line numbered.
2. Call 2: five more in parallel: the rest of `composer.coax`, `desk.coax`
   lines 1–800, `gpu.coax`, `all.coax`, `skins-checks.coax`.
3. Calls 3–4: `desk.coax` 801–1237, `screen.coax`, `modes.coax`. After four
   calls it holds about 136K tokens: every file the task touches, whole.
4. Call 5: five `edit_file`s in one response, one per skin file, all applied.
5. Calls 6–8: two ranges it needed again (`desk.coax` 550–685, `picker.coax`),
   then desk edits (9, 11: a `+60/-44` rewrite of the two painters in one
   6.5K-token call). Two `edit_file`s failed (12, 14) and were fixed by one
   range read (13). Tests (16, 19), `components-check`, `git status`, done.
   21 calls, $3.60, 683 s. Calls 4–7 were slow and uncached (122–141K new
   tokens each, 46–130 s), which is the price of loading everything.

`n4-agy-argon-v2`: the same shape in 14 calls, $1.62, 525 s.

**Microcoder `n4-mc-argon-v1`** (`analysis/tl/next-n4-mc-argon-v1.txt`):

1. Call 1 `cat`s all seven skin and check files (64K tokens of input).
   Call 2's input is 23.5K: the step was compacted at once.
2. Calls 2–45 read the same files again and again, 2–7 `sed -n`/`cat`s a
   call: `desk.coax` 72 times in ranges (`450,820`, `821,1150`, `90,450`,
   `550,720`, `1,300`, `1150,1500`, `500,600`, `720,1050`, `570,695`,
   `100,320`, `1,105`, `520,695`, ...), the four skins five or six times
   each in full. The input goes 64K → 23K → 36K → 48K → 19K → 40K → 50K →
   30K → ... (8 shrinks). The rationales repeat the same intention: "Read
   lines 1 to 700 of desk.coax to see dsk_init, dsk_paint_sidebar ...",
   "Let's read every requirement of the task carefully and check all relevant
   lines", "Let's carefully analyze every requirement".
3. Call 46 (685 s): the first edits, five skin files at once. Four fail
   ("old text is not in the file"): they were written from slices that had
   been compacted. The 128th command or edit ends the run. 45 calls, $4.04.

`n4-mc-argon-v2`: the same: 7 shrinks, 69 re-reads, first edit at call
42 (686 s); one desk edit fails; call 44 rewrites a 6 KB block of
`desk.coax` (6.2K output tokens, 103 s) and it does not compile ("prim_text
takes 9 arguments, given 8": the signature it had read at call 13 was gone);
call 47 rewrites the block again (69 s) and the 128th effect ends the run
at 895 s.

**What Antigravity did that Microcoder didn't:** read every file the task
touches once, in numbered pages, early and in parallel, and kept it all for
the rest of the run; then made the edits from text it could still see. The
two Microcoder runs read more bytes than Antigravity did (85–91 reads
against 18–23) and still never had the files in view when they edited.
Removing the command count alone would not have saved them: v2 hit it at
895 s, v1 had 215 s left with every skin edit still to redo.

### N1 v2: where Microcoder's 900 s went

`n1-mc-argon-v2` (`analysis/tl/next-n1-mc-argon-v2.txt`), against its own
round 1 run (`v1`, passed in 421 s):

| Calls | Seconds | What |
| --- | ---: | --- |
| 1–5, 7 | ~110 | Reading. Call 1's input is 58K tokens (the seed and a view of `dodge.coax` and `dodge-checks.coax`, the file that shows how a check sets up a battle). Call 2's input is 9.8K: **compacted after one call** |
| 6 | 90 | A call that sent nothing for 90 s; the gateway cut and retried it |
| 8–9 | 73 | Writes `dash-checks.coax` (3.5 KB) and a fivebar test; both fail as intended |
| 10–12 | 192 | Writes `dash.coax` and the three `battle.coax` delegations. **Four of the battle edits fail** (calls 10 and 12): their `old` text came from `sed` slices read in call 1. Call 11 re-reads those lines |
| 13–16 | 315 | Rewrites the whole of `dash-checks.coax` four times (3.1K, 3.8K, 6.3K, 6.1K output tokens; 60, 63, 105, 86 s), each time guessing a battle-setup API: `rs_reset(b, ...)` with 4 arguments, `rs_setup`, then `rs_seed`. None exists; the setup it viewed in `dodge-checks.coax` at call 1 had been compacted at call 2. Calls 15–16 put the whole 6.4 KB file in `old` as well as in `new`, doubling the output |
| 17 | 45 | Greps for `rs_quiet_ft ::`; the run is cut at 900 s in the next call |

So about 315 s of the 900 went to regenerating a test file against an API
the run had seen and forgotten, 90 s to a silent call, and about 70 s to
the four failed edits and their re-read. The fix itself (`dash.coax` and
the delegations) was right: the hidden checks pass; the run fails because
its own `dash-checks.coax` does not compile. Round 1's run passed with the
same plan because its view of the setup survived long enough (2 shrinks,
first edit at call 7, one big call).

Antigravity's N1 v1 did the work by about 600 s (tests passing at call 14),
then spent calls 15–22 on `coax mutate` (what AGENTS.md asks of `#authority`
files) and was cut at 900 s. Its v2 passed in 654 s, also running mutate.

### N3: both failed round 1, both passed round 2

The task changes six JSON readers in six files (seven with the CLI test
file). Every failing run edited only `aethyr/forge/doc.coax`, the first
item, so the hidden check could not build (`seat_num` not declared).

- **Microcoder v1** (`next-n3-mc-argon-v1.txt`): 33 calls of reading before
  the first edit (75 reads, 62 re-reads, 7 shrinks); rationales like "Let's
  read lines 75-165 of aethyr/forge/doc.coax completely (using smaller sed
  ranges so nothing is cut)" and "read lines 1-165 ... in smaller slices so
  we see every single line": cut outputs drove re-reads in narrower slices.
  Three calls were silent for 90 s and retried (270 s). Then it worked item
  1 alone: edit `doc.coax`, discover from a book chapter that one check
  already existed, revert part of it (calls 34–38), and the 128th effect
  came at 711 s with one file done.
- **Microcoder v2**: 17 calls of reading (35 re-reads, 5 shrinks), no
  stalls, then **all six files edited in two calls** (18: 7 edits across 4
  files; 19: 11 edits across 7 files), then fixes until the tests passed:
  26 calls, 570 s, $2.64.
- **Antigravity v1**: read 18 ranges in parallel (2.5 tools a call), grew
  to 184K tokens a call with only 44% cached, then sent nothing for 120 s
  three times; Coder's watchdog cancelled the turn after two recoveries.
  8 calls, $3.45, one file edited.
- **Antigravity v2**: 25 calls, 28 ranged reads, 17 edits across all files,
  none failed; 857 s.

What changed between rounds was not the build or the task but the run's
path: for Microcoder, three stalls fewer and edits batched across all files
once it had read them (v2), against item-by-item work after a long reread
loop (v1); for Antigravity, no silent calls. Both v1 failures had the same
visible result (one file of six), from different causes. With the
economy prompt's advice "change each place as you find it", v1 still read
for 33 calls first.

### What N2, N5 and N6 add

Microcoder passed all six runs with no more calls and less money than
Antigravity on every one (N2: 7 calls, $0.76–0.93 against 17–24 calls,
$1.22–1.81). Antigravity ran `coax mutate` in 3 of these 6 runs. These are
the tasks where Microcoder's batching wins as on H7 and H1, and where its
working memory was not exceeded (at most 9 re-reads).

### Edits on N

Microcoder: 35 of 152 edits failed (23%), in 12 of 12 runs; N6 v2 lost 8
of 14. Antigravity: 3 of 151 (2%). The tuning set's pattern holds: failures
come after compaction (N4 v1 call 46) or on text the run had changed or
read in slices (N1 v2 calls 10 and 12).

## 4. Aggregate behaviour

### Calls, batching and how code is found

| Group | Runs | Calls/run | Tools/call | Cmds/run | Views or reads/run | Command mix (search / read / test / diff / other) |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| mc-argon round 2 (s) | 28 | 7.5 | 2.1 | 9.6 | 4.3 | .18 / .30 / .25 / .17 / .11 |
| mc-argon round 4 (w) | 42 | 9.2 | 1.8 | 10.9 | 4.0 | .25 / .28 / .22 / .16 / .09 |
| mc-carbon (c) | 42 | 16.2 | 2.4 | 29.3 | 10.4 | .23 / .40 / .17 / .12 / .08 |
| agy-argon round 4 (w) | 14 | 11.9 | 1.3 | 4.4 | 8.7 | .32 / – / .45 / .18 / .05 |
| agy-carbon (c) | 42 | 19.6 | 1.4 | 8.8 | 15.0 | .33 / – / .47 / .12 / .07 |

Microcoder reads through the shell (40% of its Carbon commands are
`cat`/`sed`/`head`) because its `view` has no ranges and no line numbers;
Antigravity reads with `read_file` (409 of its 630 Carbon reads used a line
range) and uses the shell for search and tests. 11% of Microcoder's command
outputs on Argon round 4 (82 of 751) and 8% on Carbon (146 of 1,830) were
cut to head and tail, mostly `sed` and `grep`; the middle cannot be
recovered except by running another command. Antigravity truncated 92 of
554 outputs, saved each in full to a file, and paged back into one 11 times.

First edit: the median call of the first edit is about the same (Carbon:
Microcoder call 9, Antigravity call 8; Argon: call 3 both), so Microcoder
does not explore longer before editing; it explores the same ground more
than once.

### Memory: shrinks and re-reads

| Group | Calls | Context shrinks (> 20%) | File reads | Re-reads of a file already read | Calls that only re-read | Their cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| mc-argon s | 210 | 19 | 192 | 69 (36%) | 29 | $1.70 of $14.63 |
| mc-argon w | 379 | 35 | 283 | 110 (39%) | 33 | $2.54 of $27.37 |
| mc-carbon c | 673 | 98 | 851 | 559 (66%) | 105 | $7.28 of $54.03 |
| mc later builds (Argon, 24 runs) | 136 | 15 | 91 | 26 (29%) | | |
| mc economy build, #13263 Carbon | 28 | 10 | 141 | 98 (70%) | | |
| agy-argon w | 166 | 1 | 120 | 30 (25%) | | |
| agy-carbon c | 822 | 3 | 597 | 196 (33%) | | |

"Calls that only re-read" are calls whose only actions were reads of files
already read; calls mixing a re-read with something new are not counted, so
it is a lower bound. Antigravity's re-reads are mostly the next page of a
file it is paging through.

### Edits

| | Edits | Failed | Failed on a file the run had already edited in an earlier step | In the same step after an applied edit to that file | On a first edit of the file |
| --- | ---: | ---: | ---: | ---: | ---: |
| Microcoder, all tuning runs | 563 | 39 (7%) | 30 | 8 | 1 |
| Antigravity, all tuning runs | 374 | 1 | | | |

24 Microcoder runs had at least one failed edit. The mechanism is visible
in `state.coax`: `mc_action_edit` records `{"edit": path, "applied": true}`
and nothing of the text; earlier views are "as they were then". So a second
edit to text the model itself wrote must reproduce it from nowhere (H6 w2
calls 11 and 15), and a batch of edits after a compaction is written from
memory (T5 w3 call 4).

### Cache

For each pair of consecutive calls in a run where the prompt grew:

| Growth over the previous call | Microcoder: calls / hit rate / cached share | Antigravity: calls / hit rate / cached share |
| --- | --- | --- |
| < 1K tokens | 267 / 41% / 48% | 393 / 98% / 95% |
| 1–2K | 120 / 64% / 67% | 180 / 98% / 95% |
| 2–4K | 168 / 73% / 66% | 177 / 98% / 94% |
| 4–10K | 314 / 61% / 53% | 219 / 97% / 89% |
| > 10K | 353 / 30% / 24% | 257 / 89% / 68% |
| prompt shrank | 221 / 2% / 4% | 4 / 0% |
| previous prompt < 20K tokens | 509 / 10% | 95 / 81% |

- Time between calls does not explain Microcoder's misses (hit rate 54–70%
  whether the gap is under 10 s or over 40 s, prompts over 8K tokens).
- Changing the thinking level between calls does: 7 of 87 such calls hit
  (8%), against 63% at the same level. Round 4's medium plan and medium
  re-ask paid full price for their prompts (e.g. `t3-mc-argon-w3` call 4:
  the same 13,660-token prompt, nothing cached).
- Same endpoint and model for both, so the difference is the request.
  Antigravity sends multi-turn `contents` (user, model function calls,
  function responses), declares tools, always uses `streamGenerateContent`,
  and starts every request with the same ~12.5K-token system prompt (over the
  4,096-token cache minimum). Microcoder sends one growing user message with
  the whole state as a JSON string, a ~1.1K-token system prompt, a
  `responseSchema`, and (in round 4) non-streaming calls at low. Which of
  these matters is not known; the -next doc's probe ruled out "one part vs
  several parts in one message" but not multi-turn turns, streaming or the
  constant prefix.

### Time

- Short calls (under 300 output tokens) take a median 7–9 s per Microcoder
  step against 5–7 s per Antigravity model call; the Microcoder figure
  includes fast tools. p90 28 s (Argon r4) against 14 s.
- Carbon: 50 Microcoder calls took over 60 s; most of those are long
  thinking at "low" (up to 11,734 thinking tokens), and they cluster in the
  thrashing runs (T5 c2: six calls of 95–140 s). Output per call: 807 tokens
  against Antigravity's 474.
- On small tasks Microcoder's few calls win outright (H1, H7, T1, H2: 4–6
  calls against 9–26). On H6, T2 and the Carbon wide tasks the extra calls
  from re-reading lose it.

### Verification and finishing

Both run the task's checks after the last edit in most runs (Microcoder 28
of 42 Argon r4, 27 of 42 Carbon; Antigravity 8 of 14 and 27 of 42) and read
`git diff` (Microcoder 41 of 42 and 40 of 42; Antigravity 9 of 14 and 31 of
42; reading tasks need no test). Microcoder decides it is done with
`finished: true` after its diff; Antigravity ends its turn with text and no
tool call, sometimes while a background test still runs (H6), relying on
the wake-up. Antigravity over-verifies: `coax mutate` in 28 of 86 runs (62
calls) and repo checks such as `components-check`, which AGENTS.md asks
for; that is where three of its four Carbon limit hits went.

## 5. Microcoder's weaknesses, ranked

Ranked by what they cost across passes, dollars and seconds on these runs,
and by how likely they are to matter on wider, unseen tasks.

After N1–N6 the order by passes is W1 (three of four N failures, part of
the fourth), W3 (23% failed edits on N; part of N1 v2 and N4), then W2
(cost: Microcoder was already cheaper on N, $25.73 against at least
$30.97, while paying about four times more per input token), W6b (silent
calls, 630 s on N), W6c and W4–W5 as parts of W1 and W3. The sections keep
their original numbers.

### W1. A small working memory that forgets file contents (compaction thrash)

**Evidence.** On the unseen tasks: N4 v1 and v2 and N3 v1 (section 3b:
69–77 re-reads, 7–8 shrinks, first edit at call 33–46, command limit) and
N1 v2 (the setup it needed compacted after one call; 315 s of rewrites
against a guessed API). Three of Microcoder's four N failures, and part of
the fourth. On the tuning set: P12 `t5-mc-carbon-c2` (nine views of the same three files, no
edit, cut at 900 s); `mcnext/aaa/carbon1` (98 of 141 reads were re-reads,
no edit, command limit); `aaa-13263`; P1 `h6-mc-argon-w2` calls 4–9; P14
`t2-mc-carbon-c1` calls 1–26; P17 `h6-mc-carbon-c2` calls 2–18; P11
`t5-mc-argon-w3` call 4. The -next doc's N3 and N4 failures (command limit
after 36 and 45 calls, one file edited) have the same signature.

**How often, what it costs.** 98 context shrinks in 673 Carbon calls (15%),
35 in 379 Argon r4 calls; 66% of Carbon file reads re-read a file already
read (Antigravity 33%); at least 105 Carbon calls ($7.28, 13% of spend) and
33 Argon r4 calls ($2.54) did nothing but re-read. Passes: T5 Carbon c2 and
both #13263 runs on this set (and plausibly N3/N4). Time: most of the gap on
H6 Argon (771–900 s against 155–184 s), H6 Carbon (608–719 s against
243–386 s) and T2 Carbon (465–556 s against 204–683 s). Each shrink also
throws away the cached prefix (2% hit rate after a shrink).

**What Antigravity does.** Keeps everything (80–230K tokens a call) and
pays little for it because 90%+ is cached. It pages large files in 800-line
numbered pieces, so one read never floods its context. It also fails at the
far end: on H4 Carbon its context reached 180K tokens, stopped caching, and
the run ran out of time.

**Change.** Two parts, both inside the existing tools and state, the loop
unchanged:

1. *A per-step budget for what results add.* Show at most about 32 KB of
   results per step in the state: a view shows its first ~300 numbered
   lines (or the range asked for) and says which lines are left; a command
   output over ~8 KB keeps its head and tail and its full text goes to a
   file under the run's session folder, named in the cut line ("[... 41,200
   bytes left out: whole output in ~/.openagents/coder/sessions/ID/out/7-2.txt]").
   One step can then never empty the window by itself, which is what drives
   the thrash on Carbon (5–9 `cat`s or 3 whole views a step).
2. *A larger window, once W2 makes it cheap.* The window is capped by the
   journal's 256 KB request (`AF_REQUEST`, an `#authority` file, owner
   sign-off) and the 144 KB log. Antigravity's runs show the model works
   well at 100–150K tokens when cached. Raising the request to ~1 MB and
   the log to ~600 KB would make compaction rare on the tuning tasks.

**Expected effect.** Shrinks and re-reads on Carbon down to Antigravity's
level (re-read share ~33%); no "never edited" limit endings on tasks like
T5 Carbon and #13263; 2–5 fewer calls on H6/T2-style tasks.

**How to measure.** On tasks the change was not built from (after the
verdict, N1–N6 and fresh tasks; not T5/H6/T2/#13263, which it was built
from): context shrinks per run, re-read share, calls before the first
edit, limit endings, passes and $; plus #13263 as a regression probe only.

### W2. The request gets few implicit-cache hits

**Evidence.** Section 4's cache table: 41% hit rate for prompts that grew
under 1K tokens, against 98% for Antigravity on the same endpoint and
model; 10% for prompts under 20K tokens against 81%. Cached share 37%
(Argon r4) and 48% (Carbon) against 79% and 91%.

**What it costs.** Input is 86% of Microcoder's Argon r4 spend ($23.64 of
$27.37) and 80% of Carbon's ($43.30 of $54.03), and 52–63% of the input
was uncached. Repricing the same tokens at Antigravity's 92% cached share:

| Arm | Spend | At 92% cached |
| --- | ---: | ---: |
| mc-argon round 2 | $14.63 | $4.79 |
| mc-argon round 4 | $27.37 | $8.50 |
| mc-carbon | $54.03 | $20.83 |

Per input token Microcoder pays about $2.18 per million where Antigravity
pays about $0.50, so even a three times larger context (W1 part 2) would
cost less than today if it caches like Antigravity's. Uncached prefill is
also slower.

**What Antigravity does.** Multi-turn contents with function calls and
responses, a constant ~12.5K-token prefix, streaming for every call.

**Change.** A request-shape change, still one structured call per step:
send the state as turns, not one growing string: the system prompt;
a first user turn with facts and task; then per step a model turn (the
action JSON it returned) and a user turn (that step's results); compaction
replaces the oldest turns with one `earlier` user turn. Keep the thinking
level fixed for a whole run (already so since the revert).

**Measure it before building it.** The journal holds every request body
Microcoder sent (kind-1 records in `worker.ajr`). Replay one recorded run of
about 10 calls (e.g. `mcg/harness/runs/t2-mc-argon-w1` or
`h6-mc-carbon-c2`) to Vertex in three shapes: as recorded; as multi-turn
turns with the same text; and as recorded but streamed. Compare
`cachedContentTokenCount` call by call. About $1–3 a shape. Adopt the shape
only if it reaches ≥ 85% hits; then measure $ and cached share on
unseen tasks.

### W3. The model never sees what its edits did

**Evidence.** On N1–N6, 35 of 152 Microcoder edits failed (23%, in every
run) against 3 of 151 for Antigravity; N4 v1's first edit batch lost 4 of
5, N1 v2 lost 4 battle edits. On the tuning set, 39 of 563 Microcoder edits failed (7%) against 1 of 374 for
Antigravity; 30 of the 39 targeted a file the run had edited in an earlier
step, 8 more came after an applied edit in the same step, 1 was a first
edit. P1 `h6-mc-argon-w2` calls 11 and 15 (its own test text); P11
`t5-mc-argon-w3` call 4 (7 of 8 written from memory after a compaction);
`next-h6-mc-argon-c1` call 5 (second edit after the first changed the text).

**What it costs.** Each failure costs at least one more call to view the
text again and one to redo the edit: ~1–2 calls, 20–100 s and
$0.05–0.15 each, in 24 runs. Never a pass on its own in these runs, but it
adds to W1's call count where W1 already bites.

**What Antigravity does.** Its own `edit_file` call, with the full
replacement text, stays in the conversation, and the result names where it
landed ("starting at line 510 (+17/-3 lines)").

**Change.** In `mc_action_edit`: an applied edit's result carries the line
range and the new text with two lines around it, numbered (cut at ~2 KB);
a failed edit's result carries the nearest matching region (by the old
text's first line or longest common run), numbered, ±5 lines. Mark a
step's earlier views of an edited file as stale ("changed by step N").

**Expected effect.** Edit failures from 7% toward 1–2%; one or two calls
fewer in about one run in eight.

**How to measure.** Failed edits per edit, and calls per run, on unseen
tasks; the edit tests in `state.coax` check the shape.

### W4. Views without line numbers or ranges, and cut outputs that cannot be recovered

**Evidence.** P1 calls 3–8 guess `sed -n` ranges (`1100,1220`, `1221,$`,
`1500,1650`, `900,1010`, `700,900`, `1,180`) to find `ky_consume`; P14 calls
10–26 page a 50 KB file by guessed ranges; P16 runs nine `wrap_angle` greps
after the first one was cut to head and tail and dropped `cast.coax`'s line,
then omits `cast.coax` from its answer. 82 of 751 (Argon r4) and 146 of
1,830 (Carbon) command outputs were cut in the middle.

**What it costs.** Mostly calls and seconds (folded into W1's counts); one
fact on H4 Carbon c3.

**What Antigravity does.** `read_file` returns `N: line` with a range;
truncated command output is saved to a file it can page.

**Change.** Let `view` entries be `path` or `path:A-B`; number every
shown line; and the spill file of W1 part 1 for cut outputs. This is a
tools change; the action schema keeps `view` as a list of strings.

**Expected effect.** Fewer `sed` reads and repeated greps; fewer
`grep -n` calls just to learn line numbers.

**How to measure.** Commands per run that are reads, near-duplicate
commands per run (same program and file, different filter), facts missed
in reading tasks.

### W5. Rationales are plans, not findings (Carbon)

**Evidence.** Carbon rationales in `mcnext/aaa/carbon1` and P12–P17 say
what to read next ("Let's read ... without truncation"), not what was
found, even with the economy prompt asking for findings. When steps are
compacted the 1,200 kept rationale bytes therefore carry nothing to work
from.

**What it costs.** It is why W1's compactions force re-reads instead of
being survivable. Argon follows the economy advice better (later Argon
builds: 29% re-reads, few shrinks); Carbon does not.

**Change (the -next doc's proposal, kept as second line).** A required
`found` field in the action: paths, line numbers and exact text this step
learned; kept whole through compaction and shown first. It is a schema
change, so measure it after W1–W4: if W1's per-step budget and larger
window remove most compactions, this may not be needed.

### W6. Round 4's medium re-ask and thinking switch (removed; keep them out)

**Evidence.** P1: the run had finished at 794 s; the medium re-ask did not
finish and ran into the 900 s limit. Thinking-level switches drop the cache
hit rate to 8%. The -next doc reverted it (26c27d5714) for cost.

**Effect of keeping it out.** One call and an uncached prompt saved per
finish; one avoidable limit ending on H6.

### W6b. Silent calls cost 90 s each (unseen tasks)

**Evidence.** 7 Microcoder calls on N1–N6 sent nothing for 90 s and were
cut and retried by the gateway (N1 v1, N1 v2, N2 v1, N4 v1, and three in
N3 v1): 630 s in all, 270 s of N3 v1's 711 s. (The #13231 doc counts 4
such retries in the Carbon arm.) Antigravity had its own: three 120 s silences ended N3 v1,
and the watchdog fired in N4 v1 and v2.

**Change and measure.** The 90 s first-byte limit is the gateway's
(`plan.coax`). Whether a shorter first-byte limit for low-effort calls
(say 30 s, which no recorded low call needed to start streaming) loses
real answers can be read offline from the recorded attempts' timings before
it is changed. Measure: seconds lost to silent calls per run.

### W6c. Whole-block rewrites of its own files

**Evidence.** N1 v2 calls 13–16 (four rewrites of `dash-checks.coax`,
3–6K output tokens, 60–105 s each; two with the whole file as `old` too);
N4 v2 calls 44 and 47 (6 KB blocks, 103 s and 69 s); T5 Argon w3 call 6.
It follows from W1 and W3: without the file's current text in view, the
model rewrites the block it remembers writing.

**Change.** None of its own: W3's edit results (the new text, numbered)
and W1's budget remove the cause. The prompt can say that an empty `old`
replaces a whole file, so a rewrite never needs the old text copied.

### W7. Smaller ones

- *Answer in `reply` instead of the file the task names* (P10, once in ~20
  reading runs, round 2 build). Watch, no change.
- *30 s command limit* made it background and poll its tests (P1 calls
  20–26, H6 runs s1, s2, w1). Fixed by the 120 s limit (66b316de85).
- *Long Carbon thinking at "low"* (50 calls over 60 s, output 807 tokens a
  call against 474). Mostly in thrashing runs; expect it to fall with W1.

## 6. Microcoder's strengths to keep

- **Batching on short tasks.** Edit, test and `git diff` in one step: H1,
  H7, T1, T3 in 3–5 calls against 4–26 for Antigravity (P3: 5 calls, 60 s,
  $0.28 against 19 calls, 384 s, $1.05; P19: 5 calls, 72 s against 9–15).
- **It stops when the task is met.** No `coax mutate` or repository rituals
  unless asked (3 Microcoder runs ran mutate; 28 Antigravity runs did).
  Antigravity's three Carbon limit hits on H4/H7 and one on T5 came from
  reading everything and over-verifying; Microcoder had one Carbon limit hit.
- **Small prompts.** 24–29K input tokens a call against 64–107K, so even
  with poor caching Microcoder's total cost matched or beat Antigravity's.
  W2 would turn that into a large lead.
- **The seeded first step** saves a call on short tasks (later builds: T1,
  H7 in 2 calls).
- **Edits as structured data**: no shell-quoting failures since ddf0dae6c9,
  and edits are applied before the step's commands, so a step can edit and
  test together.
- **The economy prompt works on Argon.** Later Argon builds: 29% re-reads,
  15 shrinks in 136 calls, H4/T4/T5/H6 in 3–7 calls. It does not carry to
  Carbon on wide tasks (W1, W5).

## 7. Antigravity's weaknesses (for the record)

- One tool per call (1.3–1.4 tools a call): 2–4x Microcoder's calls on
  short tasks.
- AGENTS.md in its system prompt turns into rituals: `coax mutate` in 28 of
  86 runs, `components-check`, reading `INVARIANTS.md` (56 KB) on a one-file
  task (T3 w1). Three of its four Carbon limit hits.
- Unbounded reading on reading tasks: H4 Carbon read 69 files, reached
  180K tokens a call, lost its cache (172K uncached tokens on single calls),
  $6.20 and the limit.
- Gold-plating: rewrites a passing helper four times (P3), edits after
  tests pass (P18).

## 8. What to do, in order

Revised after N1–N6: passes on wide tasks first (W1, W3), then cost (W2).
The cache probe stays early because it is cheap and is the condition for
step 4.

| # | Change | Built from | Expected | Measure on | Cost to measure |
| --- | --- | --- | --- | --- | --- |
| 1 | Per-step result budget, numbered ranged views (`path:A-B`), spill files for cut output (W1.1, W4) | P1, P11–P17, aaa, N3, N4 | far fewer shrinks and re-reads; first edit before call ~15 on N3/N4-sized tasks; no limit endings with one file done | new unseen wide tasks (N3, N4 now seen); tuning set as regression | one round |
| 2 | Show edit results: new text with line numbers; nearest region on failure (W3) | 74 failed edits | edit failures 23% (N) / 7% (tuning) → a few % | same | rides on 1 |
| 3 | Replay probe of request shapes (W2) | cache table | decides whether multi-turn, streaming or a ≥4K stable prefix gets ≥ 85% hits | 1 recorded run, 3 shapes | ~$3–9 |
| 4 | Adopt the winning shape, then a larger request and log (W2, W1.2; owner sign-off for `AF_REQUEST`) | probe, W1 | cost −50–65% per token; compaction rare below ~150K tokens | unseen tasks, Carbon | one round |
| 5 | Shorter first-byte limit for low calls (W6b), if the recorded timings allow | N runs | −90 s per silent call | offline from records first | none |
| 6 | `found` field (W5) only if 1–4 leave thrashing | -next doc | survivable compactions | Carbon, #13263 | ~$5 |

Every change above is general (no task names or hints) and keeps one
structured call per step. Each was designed from tuning-set traces, so by
the owner's rule it stays only with a measured gain on tasks it was not
built from: N1–N6 after the verdict (counting N3 and N4 as seen, per the
-next doc's blinding note) and a fresh unseen set. Measure W2 first: it is
the cheapest to test, it changes cost on every task, and it makes W1's
larger window affordable.

## 9b. Against the design analyst's ranked changes (`analysis/harness-design.md`)

| Their item | What this evidence says |
| --- | --- |
| 1. Line-range, numbered views on the read budget | **Confirmed, and the top change.** N4: 72 `sed` reads of `desk.coax` a run in guessed ranges; Antigravity read it in two numbered pages and never again. Add the per-step cap (W1.1): without it a ranged view still lets one step of 5–7 reads flood the 144 KB window, which is what compacted N4's and N1 v2's first steps at once |
| 2. Count read-only commands as reads, or raise the effect limit | **Partly contradicted.** It treats the symptom. N4 v2 reached the command limit at 895 s, so without the count it would have hit the time limit 5 s later; N4 v1 reached it at 685 s with every skin edit still failed. The limit is where the thrash shows, not what causes it. Worth doing only together with 1 |
| 3. A required `found` field | **Plausible, not shown.** Rationales on N are plans ("Let's carefully analyze every requirement") as on Carbon; but Antigravity succeeds by keeping text, not notes, and N3 v2 passed with the same rationales once it batched its edits. Keep it behind 1, 2 (edit results) and the cache work, as they rank it |
| 4. Show the budgets in the notes | **Weak.** The N runs that hit the limit had no edit to make yet that would have passed; pacing tells the model to stop reading, not where its reads went. Cheap, so harmless |
| 5. Keep long output whole in a spill file | **Confirmed.** 8–11% of command outputs were cut in the middle; N3 v1's rationales re-read "in smaller slices so nothing is cut"; H4 Carbon c3 lost a fact to a cut grep |
| 6. AGENTS.md in the stable prefix | **Mixed.** It would lift the stable prefix over the 4,096-token minimum, one of the cache hypotheses (W2), and tell the model the repo's rules. But on this evidence AGENTS.md is also what makes Antigravity run `coax mutate` after the work is done: 28 of 86 tuning runs and 5 of 12 N runs, and the cause of its N1 v1 and H7 Carbon limit endings. If it goes in, measure the ritual commands it adds; the probe in W2 can test the prefix effect with neutral stable text first |
| 7. Edit results that say where | **Confirmed and under-ranked.** Edit failures are 23% on N (2% for Antigravity), 30 of 39 on the tuning set came on a file the run had already changed, and they produced the whole-block rewrites that took 60–105 s a call (W6c). Show the new text, not only the line numbers; on failure show the nearest region |
| 8. Larger state, last, because a cache miss on a big call is expensive | **Agree on the order, disagree on the reason's permanence.** Their cache model ("hits need about 2,050 new tokens a step") does not fit Antigravity's calls on the same endpoint: 98% hits when its prompt grew under 1K tokens (section 4). Microcoder's misses look like a property of its request, so measure the request shape first (W2); if it caches like Antigravity's, a larger state is cheap and becomes the direct fix for W1 |
| Not proposed: background commands | Agree: with 120 s, no N run backgrounded a test |
| Not proposed: a summarising compaction call | Agree for now: Antigravity compacted in none of its N runs; keeping text was enough |

Two causes they do not list: the 90 s silent calls (W6b; 630 s across the
12 Microcoder N runs) and edit failures as a driver of re-reads and
rewrites (they list edit results last but one).

## 9. Limits of this analysis

- Pass counts on these 14 tasks are mostly judge noise (section 2); the
  ranking uses calls, re-reads, edits, cache and limit endings instead.
- Microcoder step times include that step's tools; Antigravity's model
  times are at 1 s resolution; round 4 and Carbon ran under load.
- The cache mechanism is inferred from the two request shapes and the
  hit rates; only the replay probe can say which difference matters.
- Microcoder's `view` and `sed` reads are counted by file path, so a
  re-read of a different range of the same file counts as a re-read for
  both harnesses.
- Antigravity's transcript (`transcript_full.jsonl`) existed for all 86
  Antigravity runs; its system prompt was read from one run's
  `gen_metadata` (identical sections in others checked by spot reads).
