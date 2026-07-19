---
title: Review and recovery
description: Review exact changes and understand restart behavior.
lastModified: 2026-07-19
sidebar:
  order: 4
---

## Repository review

OpenAgents Desktop keeps repository context adjacent to the conversation. The review surface may show bounded status and an exact correlated diff for the active work context.

The MVP review boundary is intentionally read-only. It does not expose discard, commit, branch, push, pull-request, merge, or arbitrary command execution controls.

Review fails visibly when output is stale, revoked, secret-shaped, binary, oversized, or unavailable. Absolute filesystem paths and host credentials do not enter the renderer projection.

## Reload and restart

Renderer reload and application restart reconcile against durable session and run identity.

- The application restores confirmed work from its authoritative record.
- An incomplete stream remains interrupted or pending reconciliation.
- A proven history gap triggers an authoritative refetch.
- No restart silently reruns provider work.
- No stale response may overwrite a newer selection or subscription generation.

If the app cannot prove the current state, it should say so. `Unknown pending reconciliation` is safer and more truthful than optimistic completion.

## Recovery checklist

1. Confirm the selected session and repository context.
2. Read the latest interruption, gap, or connection row.
3. Reopen the exact change review if repository state is still available.
4. Resume only through the visible runtime action.
5. Start a new session only when you intend to create new work identity.
