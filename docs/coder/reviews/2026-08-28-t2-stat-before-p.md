# Cycle review: T2 `--stat` before `-p` on tb2-quick

Date: 2026-08-28. Lane: proxy. Model: `glm-5.3-flash`. Suite: `tb2-quick`.
Lever: T2, staged in `surfaces/coder/system-prompt.v1.json` and
`surfaces/coder/tool-descriptions.v1.json`.

This file is a human record. `pnpm run coder:review` could not accept a
review: both after-runs produced no ATIF trajectory, so no
`trial:<task>#step-<id>` ref exists, and the command refuses any proposal
that does not cite one.

## What ran

Baseline, already on the store (`row:tb2-quick#2026-08-28T02:01:35.142Z`):
Flash 1 of 2, job `7695aa83-ece2-4083-8092-4968b6e738bf`. openssl accepted;
regex-log rejected after the inference proxy was unreachable.

After T2, two Harbor jobs of the same suite, model, and lane:

- `row:tb2-quick#2026-08-28T03:06:00.236Z` job `72928cbc-27aa-499f-aee2-3947854aeed6` — 0 of 2, gate failed.
- `row:tb2-quick#2026-08-28T03:18:11.677Z` job `8600a533-19e9-402f-91f8-0a8bddc73399` — 0 of 2, gate failed.

`trial:regex-log#outcome` and `trial:openssl-selfsigned-cert#outcome` on both
jobs are rejected. Each trial's `coder.txt` ends with
`https://openagents.com/api/inference/proxy could not be reached`. No tool
call ran. The T2 sentence does not appear in those logs: the agent never
opened a turn.

## Causality

The 1/2 → 0/2 drop is the lane, not T2. `ledger:T2` is a git-habit rule
(`--stat` before `-p`). These two tasks do not inspect git history, and
the agent did not run. A success-rate delta here does not confirm or refute
T2 (`ledger:M5`: tb2-quick is a selector, not a conclusion; `ledger:M3`:
keep the rows).

## Disposition

Keep the T2 surface change. Do not revert it on this evidence. Do not
promote or refute `ledger:T2`. Do not start T1 or T3 until a Flash
`tb2-quick` run completes a turn (the same proxy miss twice is the
runbook §8 lane-degradation signature; a third attempt this session is
not allowed).

A later cycle that actually runs the agent should re-measure T2 on a
git-forensics oracle (`tb2-cross-section` tasks 1–2) if `tb2-quick` still
cannot show a round-count delta.

## Refs

- `row:tb2-quick#2026-08-28T02:01:35.142Z`
- `row:tb2-quick#2026-08-28T03:06:00.236Z`
- `row:tb2-quick#2026-08-28T03:18:11.677Z`
- `trial:regex-log#outcome`
- `trial:openssl-selfsigned-cert#outcome`
- `ledger:T2`
- `ledger:M3`
- `ledger:M5`
- `diff:surfaces/coder/system-prompt.v1.json`
- `diff:surfaces/coder/tool-descriptions.v1.json`
