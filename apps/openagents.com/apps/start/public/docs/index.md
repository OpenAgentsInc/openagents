---
title: OpenAgents Desktop
description: A local-first workroom for durable, reviewable Codex work.
lastModified: 2026-07-15
sidebar:
  order: 1
---

OpenAgents Desktop is a local-first workroom around your ordinary Codex session. It keeps the active conversation central while session navigation, tools, decisions, and repository review remain close and legible.

The current macOS ARM64 build is a release candidate. A public download will follow release approval; this documentation does not turn candidate evidence into a release claim.

## What the workroom does

- Starts a new Codex conversation or resumes durable local history.
- Renders authored messages, plans, tools, decisions, changes, interruptions, and terminal outcomes in one causal timeline.
- Supports send, stop, steer, queue, questions, approvals, and plan review through typed commands.
- Shows bounded repository status and exact diffs without adding Git mutation controls.
- Restores interrupted work explicitly instead of silently rerunning it or substituting another session.

## What stays outside the MVP

OpenAgents Desktop is not a full editor, interactive terminal, browser, payment product, or autonomous deployment system. ProductSpec, Fleet, and an OpenAgents account are not prerequisites for an ordinary local Codex conversation.

## Where to go next

- [Run from source](/docs/getting-started) for the current developer path.
- [Use the workroom](/docs/workroom) for sessions, timeline, and composer behavior.
- [Review changes](/docs/review-and-recovery) for read-only repository review and recovery semantics.
- [Understand the boundary](/docs/security-and-privacy) for credential, renderer, and local-authority rules.
- [Explore Future / Advanced](/docs/future) for clearly labeled historical ideas and dormant design horizons.
