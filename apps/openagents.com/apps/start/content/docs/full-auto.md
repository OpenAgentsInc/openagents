---
title: Full Auto
description: Let Codex continue bounded local work, turn after turn.
lastModified: 2026-07-19
sidebar:
  order: 4
---

Full Auto lets Codex keep choosing and completing the next useful task in the current repository. It is a per-session mode in the composer, not a separate agent, account, or cloud service.

## Turn it on

1. Open the Codex session and workspace you want to use.
2. Select **Full Auto** in the composer. It is off by default.

Full Auto starts immediately. On a new empty session, select the toggle. Codex then inspects the README, documentation, and open issues. It selects one useful action, completes it, and stops. If Full Auto is still enabled, the next continuation begins automatically.

The setting belongs to the session. OpenAgents stores it locally. OpenAgents binds it to the selected workspace and execution profile. The loop can continue after a renderer reload or application restart.

## Stop or steer the loop

Select **Full Auto** again to turn it off. Turning it off prevents the next automatic continuation. It does not cancel the turn already in progress. Use the normal stop control when you need to interrupt the active turn.

You can still review the conversation, plans, tools, subagents, file changes, and outcomes in the same causal timeline. Full Auto does not create a hidden background workroom.

## Safety boundary

Full Auto is high-trust local automation for the Codex lane:

- It uses the repository and Codex environment already granted to OpenAgents Desktop.
- It forces Codex's approval policy to `never` for automated turns so an unattended continuation does not wait forever for approval.
- Native questions and stray approval requests from unattended turns are declined instead of silently hanging the loop.
- A continuation runs only in the workspace that was bound when Full Auto was enabled. A missing or changed workspace disables the loop visibly.
- Dispatch is serialized and durably leased so overlapping reconciliation cannot intentionally start the same continuation twice.
- Failed dispatches are shown, retried with bounded backoff, and disable Full Auto after five consecutive failures.
- The loop stops after 20 automatic continuations. Turn it on again only after reviewing the result.

Full Auto does not add a second sandbox or a more restrictive permission model. Use it only in a repository where you are comfortable allowing the existing local Codex runtime to act without mid-turn approval prompts.

## Current scope

Full Auto is available for local Codex sessions in OpenAgents Desktop. It is not an agent-facing API, remote-control service, deployment authority, payment authority, or guarantee that a task is correct. Keep the workroom open when practical, review changes, and use repository tests and review evidence before accepting the result.
