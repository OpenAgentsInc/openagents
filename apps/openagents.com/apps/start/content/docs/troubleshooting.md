---
title: Troubleshooting
description: Diagnose an Omega prerelease without widening authority or hiding an open gate.
lastModified: 2026-07-25
sidebar:
  order: 7
---

## A candidate does not start

Confirm the exact Omega version, artifact digest, operating system, and architecture. Read the candidate release notes before you change system policy.

Do not disable signing, quarantine, or security controls to make an unknown artifact run. Report a package or notarization failure against the exact artifact.

## The product shows Zed as the product

Record the Omega version, visible surface, and exact text or image. A retained internal compatibility identifier can be valid. User-facing Zed product, service, package, or data text can still be a release blocker.

Do not run an uninstall or migration command when its target is unclear.

## An agent or Full Auto is unavailable

Confirm the selected project, agent, provider, and execution environment. Read the visible failure before you retry.

Do not replace a failed provider with another provider unless you intend to change the route. Do not treat a source test or fixture as proof that an installed candidate completed the same journey.

## Review or evidence is incomplete

Confirm the worktree and candidate identity. Then inspect the exact changes, commands, tests, and open gates.

Do not convert an unknown, waived, or unperformed check into a pass.

## Report a reproducible issue

Open an issue in the [Omega repository](https://github.com/OpenAgentsInc/omega/issues). Include:

- Omega version and source commit
- Artifact name and digest, when applicable
- Operating system and architecture
- Reproduction steps
- The result that you expected
- The result that you observed

Remove credentials, private repository data, local paths, and private agent content before you attach evidence.
