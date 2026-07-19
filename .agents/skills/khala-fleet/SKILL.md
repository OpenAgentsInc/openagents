---
name: khala-fleet
description: Operate an OpenAgents Khala coding fleet safely. Use when the user asks to connect or list fleet accounts, spawn/delegate coding work to Codex or Claude workers, start/monitor/pause/drain/stop a sustained fleet run, burn down a backlog with parallel workers, verify fleet work actually completed, or diagnose dispatch failures like "0/1 available" or target_pylon_unavailable.
---

# Khala Fleet Management

This is the canonical, repo-scope `khala-fleet` skill. It is a launcher, not
the law: the authoritative procedure lives in this repo and wins whenever
anything disagrees. Read these first, in full, every invocation:

- The **Khala -> Pylon -> Codex Coding Delegation Runbook** section of
  `AGENTS.md` / `CLAUDE.md` at the repo root.
- `docs/khala-code/2026-07-02-khala-fleet-bundled-skill.md` (how this skill is
  bundled, discovered at repo scope, and materialized into
  `~/.agents/skills/khala-fleet/` by Khala Code Desktop).
- The fleet spec under `specs/khala-fleet-delegate/` and the
  `packages/khala-fleet-intents/` intents package.

Quick orientation while you open those:

- Fleet = the owner's linked Codex/Claude accounts, each an ISOLATED local
  worker reached through Khala -> Pylon -> assignment.
- Dispatch ladder: one bounded task (`codex_spawn` / `$PYLON khala request
  --workflow codex_agent_task` with pinned repo/commit/verify) → parallel
  wave (heartbeat capacity first, one claim per unit) → sustained fleet run
  (`fleet_run_start` / `fleet_run_status` / `fleet_run_control`).
- Done = closeout checklist ok + exact `token_usage_events` rows. Counter
  movement alone is NEVER completion evidence.
- Never run login flows against `~/.codex` or the owner's live `~/.claude`.
  worker auth uses isolated per-account homes only.
