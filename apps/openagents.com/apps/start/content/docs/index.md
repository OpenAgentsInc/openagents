---
title: Omega
description: The native OpenAgents workspace for projects, agents, code, review, and evidence.
lastModified: 2026-07-25
sidebar:
  order: 1
---

Omega is a native, Zed-based IDE from OpenAgents. It brings project work, agents, code, review, and evidence into one workspace.

Omega is in active development. Signed and notarized macOS ARM64 prerelease candidates exist for evaluation. The latest recorded candidate still has open installed-release gates. Omega is not a generally available or supported release.

## Product direction

Omega starts with the editor, project, buffer, terminal, and worktree model from Zed. OpenAgents adds agent work, review, policy, evidence, and run history without creating a second project authority.

The target experience keeps these parts together:

- Native project and editor work
- First-party and external agents
- Visible plans, tools, decisions, and file changes
- Code review and evidence for completed work
- Bounded automation that a user can inspect and stop

Not every target capability is complete in a prerelease candidate. These docs identify a direction or an open gate when current evidence does not prove availability.

## Availability

There is no supported public installer in these docs. A prerelease artifact is test evidence, not a stable release.

Use the [Omega repository](https://github.com/OpenAgentsInc/omega) for source, release records, open issues, and candidate notes. Read the exact candidate status before you install or evaluate an artifact.

## Documentation map

- [Availability and setup](/docs/getting-started) explains the current prerelease boundary.
- [Native workspace](/docs/workroom) explains how editor and agent work fit together.
- [Full Auto](/docs/full-auto) explains the current automation direction and its proof limits.
- [Review and evidence](/docs/review-and-recovery) explains review and recovery principles.
- [Security and privacy](/docs/security-and-privacy) explains authority, data, and service boundaries.
- [Troubleshooting](/docs/troubleshooting) gives safe checks for current candidates.

## Legacy documentation

Earlier public docs described a separate Electron application. That product content is retired from this documentation set. The current public product documentation is for Omega.
