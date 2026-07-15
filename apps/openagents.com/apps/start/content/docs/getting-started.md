---
title: Getting started
description: Run the current OpenAgents Desktop candidate from source.
lastModified: 2026-07-15
sidebar:
  order: 2
---

## Availability

The accepted build is a signed and notarized macOS ARM64 release candidate. It is not a public release yet, so there is no supported public installer link in these docs.

## Run from source

The repository pins Node `24.13.1` and pnpm `11.10.0`. From a clean clone:

```bash
pnpm install
pnpm run dev:openagents-desktop
```

Development uses the isolated **OpenAgents Dev** profile and can run beside an installed production application. Stop the development process with **Control-C** so both Electron and the local Vite server shut down.

## Verify the package

The canonical package gate runs typechecking, tests, the production build, and Electron smoke coverage:

```bash
pnpm --dir apps/openagents-desktop run verify
```

That gate uses privacy-safe fixtures rather than ambient Codex history. A successful test run is engineering evidence; it is not release publication.

## Start a conversation

1. Open **New session**.
2. Choose or confirm the repository context.
3. Write the objective in the composer.
4. Send the turn and follow the timeline.
5. Resolve any question or approval in the focused decision surface.
6. Open **Review changes** when the turn produces repository changes.

OpenAgents uses the app-owned Codex runtime and ordinary logged-in Codex session. The initial local workflow does not require an OpenAgents account.
