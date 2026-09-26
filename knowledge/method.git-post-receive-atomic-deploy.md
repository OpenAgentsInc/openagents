---
id: method.git-post-receive-atomic-deploy
version: 1
kind: method
title: Deploy Git branches synchronously with a post-receive hook
summary: >-
  Use post-receive's ref-update stream to deploy only named branches, safely
  serialize concurrent pushes, and publish complete snapshots atomically.
  Applies to bare Git repositories serving branch contents through a web
  server.
tags: [git, deployment, hooks, nginx]
applies_when: >-
  A bare repository's post-receive hook updates web-visible content after
  pushes, especially when multiple branches, simultaneous pushes, or file
  deletions are possible.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - git-multibranch
  cites:
    - Git, githooks documentation, section “post-receive” (https://git-scm.com/docs/githooks)
    - The Open Group, POSIX.1-2017, rename(), section “DESCRIPTION” (https://pubs.opengroup.org/onlinepubs/9699919799/functions/rename.html)
    - Nginx, ngx_http_core_module, directive “alias” (https://nginx.org/en/docs/http/ngx_http_core_module.html#alias)
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Git invokes `post-receive` after updating refs and supplies one line per updated ref on standard input: old object ID, new object ID, and full ref name. Process every line rather than assuming one ref per push. Match exact full branch refs so tags and unrelated branches cannot accidentally deploy. A zero new object ID denotes ref deletion; remove that branch's published snapshot if applicable. Do not assume the default branch can be deleted through ordinary receive operations.

Serialize deployments across hook processes with an exclusive lock. For each eligible non-deletion update, export the exact received commit into a fresh staging directory, then publish it with an atomic same-filesystem rename or atomic symlink replacement. Never update a live document root in place: readers could otherwise observe a partial tree. Remove staging artifacts on errors and clean stale releases only after publication. A branch-specific immutable release plus an atomic pointer swap keeps publication separate from filesystem construction. Nginx can map a URL prefix to a distinct branch document root with `alias`; its trailing-slash behavior must agree with the location prefix.

Because the hook runs before the push is reported complete, synchronous deployments make a successful push a useful completion boundary. Git documents the hook's input format; POSIX rename documents atomic replacement semantics.

## How to check

Push updates to two supported branches in a single push, then read both served endpoints immediately after push returns and verify each matches its own committed tree. Update one branch with a changed and deleted file; check the new content is visible and the deleted path is absent. Push an unrelated ref and confirm no deployment changes. For a runnable input-parser smoke test, feed a synthetic ref-update record to the hook's parser and assert that only exact configured branch refs are selected.

Sources: Git, *githooks*, “post-receive”; POSIX.1-2017, *rename()*, “DESCRIPTION”; Nginx, *ngx_http_core_module*, “alias”.
