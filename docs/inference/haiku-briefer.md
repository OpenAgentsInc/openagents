# Haiku briefer: the opening stage of an issue run (#11323)

Status, 2026-10-11: an experiment, off by default. Measured on this Mac:
briefing recall on the 21 S2 issues, and a local end-to-end pilot (see
Results).

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

PENDING (plans/haiku-briefer.json, running).

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
