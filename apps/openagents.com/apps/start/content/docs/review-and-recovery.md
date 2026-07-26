---
title: Review and evidence
description: Connect Omega work to exact changes, tests, and recovery state.
lastModified: 2026-07-25
sidebar:
  order: 5
---

## Review in project context

Omega should keep review next to the editor and the agent work that produced a change. The review surface must identify the project, files, and current worktree state.

Review can show:

- Exact file changes
- Commands and test outcomes
- Agent decisions and delegated work
- Evidence references
- Known gaps and failed checks

A generated summary is not a substitute for the underlying change or result.

## Authority boundary

A review view does not gain Git, deployment, payment, or release authority. Each action still uses its owning system and its current authorization check.

Stale, secret-shaped, oversized, binary, or unavailable content must fail visibly. The product must not copy private credentials or unrestricted host data into an agent timeline.

## Recovery

Omega should reconcile an interrupted run against its durable identity and evidence. It must not silently rerun provider work after a restart.

Use this recovery sequence:

1. Confirm the project and worktree.
2. Confirm the selected run or conversation.
3. Read the latest interruption or failure.
4. Check the exact file and test state.
5. Resume only through a visible action.
6. Start new work only when you intend to create a new run identity.

If Omega cannot prove the current state, it must show that the state is unknown or needs reconciliation.

## Current status

Review and recovery are active product areas. Current prerelease evidence does not prove every review, restart, and recovery path on a supported release.
