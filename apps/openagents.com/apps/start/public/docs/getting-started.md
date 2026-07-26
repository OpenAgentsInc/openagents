---
title: Availability and setup
description: Evaluate Omega without turning prerelease evidence into a support claim.
lastModified: 2026-07-25
sidebar:
  order: 2
---

## Current availability

Omega is in active development. OpenAgents has produced signed and notarized macOS ARM64 prerelease candidates. The current evidence does not prove a generally available or supported release.

The latest public observation record still identifies open brand, safety, and installed-journey gates. Do not use a prerelease tag as proof that these gates passed.

## Evaluate a candidate

1. Open the [Omega releases](https://github.com/OpenAgentsInc/omega/releases) page.
2. Read the notes for the exact candidate.
3. Confirm that the artifact matches your operating system and architecture.
4. Confirm the recorded digest and signing status before you run the artifact.
5. Keep Omega data separate from any Zed installation.
6. Report a result against the exact Omega version and artifact digest.

A candidate can be useful for evaluation when a release gate is still open. Do not present that candidate as a stable installer.

## Work from source

The [Omega repository](https://github.com/OpenAgentsInc/omega) owns the current source build, test, and package commands. Use the instructions at the exact source commit that you test. This website does not copy changing repository commands into a second authority.

Omega is a fork of Zed. The fork keeps native Rust and GPUI editor foundations. Omega must use its own application identity, data roots, credentials, service boundaries, package records, and release process.

## Before you report a result

Include these values:

- Omega version and source commit
- Artifact name and digest, when applicable
- Operating system and architecture
- The action that you performed
- The result that you observed
- Any open gate or limitation that affected the result

Remove credentials, private repository data, local paths, and private agent content from public evidence.
