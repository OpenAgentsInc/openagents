# Haiku briefer: the opening stage of an issue run (#11323)

Status, 2026-10-12: an experiment, off by default, and it stays off.
Briefing recall on the 21 S2 issues was measured on this Mac; the end-to-end
pilot ran on oa-dev-env-4. Haiku's file list scores better on recall, but
runs briefed with it fixed fewer issues (see Results and Recommendation).

An issue run (`scripts/work/work_issue.py`, `openagents chat work`, the
bench's B arms) starts with a **briefing**: the files to change, with
excerpts, a change plan, the checks and the repo rules
(`scripts/bench/briefed-ab/briefing.py`). The main coding engine (Claude
through `crates/briefed-agent`, or Codex, #11308) then works from it. Today
the file list comes from `filefind` (#11210): git-mined indexes and a small
trained scorer, plus a decision-model step (the card tag decision, #11249,
asked of Jev) when the scorer is unsure. This replaces that decision step
with a small, constrained **Claude Haiku agent** that reads the code.

## What it does

`scripts/bench/briefed-ab/haiku_briefer.py`, in three steps:

1. **Candidates.** `filefind query --no-cards`: the deterministic stages
   only (embedding, symbols, co-change, similar issues, pairing rules), top
   15. About 2-3 s. No decision model.
2. **Haiku reads.** One `claude -p` run (the Claude Agent SDK's CLI, the
   process `crates/claude_agent_sdk` drives) in a checkout at the issue's
   base commit:
   - model `claude-haiku-5-5` (`OA_BRIEFER_MODEL`);
   - tools **Read, Grep, Glob, and Bash limited to `git log`**; nothing
     can write;
   - a fixed system prompt; no settings, CLAUDE.md, MCP servers, skills or
     session files (`--setting-sources ""`, `--strict-mcp-config`,
     `--disable-slash-commands`, `--no-session-persistence`);
   - hard caps: 14 turns, 120 s, $0.40 (levers `briefer_turns`,
     `briefer_secs`, `briefer_usd`);
   - **structured output** (`--json-schema`): `files` ranked, each with a
     role (change, test, context), a reason and line numbers;
     `likely_change`; `checks`; `acceptance` (the issue's points as
     testable sentences); `confidence`.
3. **Into the briefing.** Haiku's files replace the finder's list, with
   excerpts around the lines it named and its reason as the "Why". Its
   likely change, acceptance points and checks are added to the change
   plan. The checks the engine runs (`verify`) still come from the files'
   packages, as before. Paths that do not exist at the base are dropped. If
   Haiku fails, runs out of time or turns, or names no real file, the
   briefing falls back to the finder's own list and records why.

It runs on the Claude Code login already in the environment (subscription
or `CLAUDE_CODE_OAUTH_TOKEN`); `ANTHROPIC_API_KEY` is never passed. The
cost is the list price the CLI reports.

`haiku-solo` (bench only) gives Haiku no candidates: Haiku alone against
the finder.

## Where it plugs in

An engine-agnostic step: it changes only the briefing, so every engine
behind the briefing contract (#11308) gets it.

- `work_issue.py --briefer haiku` (or `OA_WORK_BRIEFER=haiku`, which
  `openagents chat work` and `POST /v1/work` pass through). Haiku reads a
  worktree of its own at the base; the result's `briefing.briefer` records
  its seconds, dollars, turns and any fallback. Default stays `filefind`.
  Making that worktree is a full checkout (about 25 s on this Mac for this
  repository); reusing the engine's own worktree, made right after, would
  remove it.
- Bench: lever `finder=haiku` (or `haiku-solo`) and arm `BH` in
  `ab.py` (B0 with the Haiku briefing). A BH trial's `cost_usd` includes
  the Haiku stage; `briefer_cost_usd` and `agent_cost_usd` show the split,
  and `wall_secs` includes its seconds.
- Not wired into the Rust `coder-new` issue-run's own decision cards
  (`crates/coder-new/src/issue_run/decide.rs`); that stays on filefind.

## Results

### Briefing recall, 21 S2 issues

Each issue replayed at the fix's parent in the bench's shallow worktree (the
fix is not visible). Recall: share of the fix's files (source and test) in
the briefing; source recall: its non-test files; top 1: the first listed file
is one the fix changed. Rows: `scripts/bench/briefed-ab/results/haiku-briefer-recall.csv`.

| Briefing files from | Files listed (mean) | Fix recall | Source recall | Top 1 is a fix file | Seconds (p50 / p95) | $ per briefing |
|---|---|---|---|---|---|---|
| filefind, top 8 (B0 today) | 8.0 | 0.60 | 0.71 | 0.43 | about 4 (whole briefing) | 0 |
| **Haiku briefer** (filefind candidates + Haiku) | **4.8** | **0.76** | **0.83** | **0.90** | 19.5 / 30.4 (+2.7 finder) | 0.006 |
| Haiku solo (no candidates) | 3.6 | 0.64 | 0.72 | 0.76 | 20.6 / 42.3 | 0.007 |

- Haiku answered all 21 with the candidates. Solo failed 3 of 21 (2 out of
  turns, 1 invalid answer); those count as zero here and fall back to
  filefind in a real run.
- With the candidates Haiku lists fewer files and more of the right ones:
  fix recall rose on 8 issues and fell on 2 (#10298, #10349: a source file
  of a three-to-five-file fix left out of a shorter list), and the first file it named was a fix file on 19 of 21
  (filefind: 9, solo: 16). The candidates matter: Haiku alone
  is about filefind's recall at half the list.
- Cost: $0.124 for the 21 (list price); about 10 turns and 20 s each.

### End-to-end pilot

`plans/haiku-briefer.json` (tag `hb1`, frozen in d8bfe34a64 before any
trial): six S2 issues (#10074, #10181, #10201, #10289, #10301, #10986), two
reps each, three arms: **A** (no briefing), **B0** (the briefing with
filefind's files) and **BH** (the same briefing with the Haiku briefer's
files). That is 36 trials. Everything ran on oa-dev-env-4 (GCE c3-standard-22,
`AB_HOST=local`) from 2026-10-11 23:29Z to 2026-10-12 09:35Z. The run paused
for several hours when the Claude login hit its usage limit, then went on.
Judging ran the same way as for earlier tags. Report:
`scripts/bench/briefed-ab/results/hb1.md`; rows:
`scripts/bench/briefed-ab/results/hb1-trials.csv`.

| Arm | Accepted (fix tests and judge) | Fix tests pass | Judge accepts | Total $ | $ per accepted | Median $ per trial | Median s | Median turns |
|---|---|---|---|---|---|---|---|---|
| A | 7/12 (0.58) | 8/12 | 8/12 | 9.81 | 1.40 | 0.68 | 157 | 12.5 |
| B0 | 6/12 (0.50) | 6/12 | 10/12 | 6.48 | **1.08** | 0.51 | 131 | 19 |
| BH | **2/12 (0.17)** | 2/12 | 7/12 | 5.22 | 2.61 | **0.32** | 198 | 12.5 |

Paired over issues (10,000 bootstrap resamples at the issue level, seed 11211):

- **BH vs B0:** accepted -0.33 (95% CI [-0.67, -0.08]), fix tests -0.33
  ([-0.67, -0.08]), judge accepts -0.25 ([-0.42, -0.08]). Cost per accepted
  issue is 142% higher ([+22%, +442%]). All three CIs exclude zero, so this
  is a clear loss even with 6 issues.
- **BH vs A:** accepted -0.42 ([-0.83, 0.00]); fix tests -0.50
  ([-0.83, -0.17]).
- **B0 vs A** (repeats the S2 question at this size): accepted -0.08
  ([-0.33, +0.17]), 23% cheaper per accepted issue (CI [-34%, +60%]). Neither
  difference is significant at 6 issues.

Why BH loses:

- **A shorter list means the agent does less.** BH briefings averaged 3.8
  files, against 8 for B0. With fewer files the agent read less and stopped
  sooner: median 12.5 turns against 19, and about 60% less cached input (median 180k tokens against 459k). Each
  trial cost less ($0.32 median against $0.51; the Haiku stage itself was
  $0.003 to $0.015). But the fixes it wrote were narrower.
- **It misses the fix's sibling files.** On #10074 Haiku listed only
  `autostart.rs`. Both BH reps left out the check that the parent folder is
  named `Contents`, so the reference test failed; both B0 reps passed. On
  #10301 Haiku left out `owner.rs` (where the fix goes). Both BH reps patched
  the store lock instead, and the judge rejected them. B0 also failed #10301.
- **Its passes fall into the gaps in reference matching.** Three
  issues (#10074, #10289, #10986) had BH fixes that the judge accepted but
  whose reference tests failed. Some were real gaps (#10074). Some were API
  shape: on #10289 the reference test names a `BANNED` constant that the BH
  fix never added. B0 shows the same pattern (judge 10/12, fix tests 6/12),
  but BH has it worse.
- The Haiku stage answered in all 12 BH trials (9 to 36 s). It never fell back to filefind and hung in none of the
  36 trials (the trim fix 0d50f9e25c, #11309, was in place).

So good recall at a short list (0.76 at 4.8 files) does not carry over to
results: the files Haiku cut were the ones the agent still needed for context.

## Recommendation

- **Do not adopt the Haiku briefer as the briefing's file step.** Keep the
  default `--briefer filefind` everywhere (`work_issue.py`,
  `openagents chat work`, `POST /v1/work`, the Rust issue-run). Keep
  `--briefer haiku` and the bench's `BH` arm as an opt-in experiment. Do not
  promote it.
- What Haiku is good at is **ranking**: its top file was a fix file 0.90 of
  the time, against 0.43 for filefind. The variant worth trying next uses Haiku
  only to reorder filefind's top 8 and lets it drop nothing. That keeps B0's
  context and puts the right file first. Pilot it as a new arm (for example
  `BR`) against B0 on the same six issues before any default changes.
- On this sample, the B0 briefing is still the cheapest per accepted issue
  ($1.08 against $1.40 for no briefing), but at 6 issues the edge is not
  significant. The S2 result (#11211) remains the main evidence for B0.

## Reproduce

```sh
cd scripts/bench/briefed-ab
python3 haiku_briefer.py 10074 10228            # recall rows, with filefind's candidates
python3 haiku_briefer.py --solo 10074           # Haiku alone
python3 briefing.py 10074 --finder haiku        # the whole briefing
AB_AGENT_BIN=... python3 ab.py batch --plan plans/haiku-briefer.json
python3 ab.py judge --tag hb1 && python3 ab.py report --tag hb1 --md
python3 -m unittest test_haiku_briefer.py
```
