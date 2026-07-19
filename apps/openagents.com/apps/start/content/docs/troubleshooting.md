---
title: Troubleshooting
description: Diagnose common development and workroom states without widening authority.
lastModified: 2026-07-19
sidebar:
  order: 6
---

## The development app does not open

Confirm that the repository's pinned Node and pnpm versions are active, dependencies are installed, and port `5734` is free. Run the package verification gate before changing runtime policy:

```bash
pnpm --dir apps/openagents-desktop run verify
```

## Codex is unavailable or incompatible

Open **Settings** and use the Codex-specific maintenance action. Desktop checks the app-owned bundled runtime. Installing another global binary does not repair that authority. A failed update or re-probe remains failed until the exact bundled runtime passes.

## A turn appears interrupted

Do not create a replacement session immediately. Read the interruption or connection row, confirm the selected session, and use the visible resume or retry action. Reloading the renderer must not cause duplicate execution.

## Review is unavailable

The repository may have changed, the work context may be stale, or the requested output may have crossed a privacy or size bound. Refresh the active repository context and request a new bounded review rather than reaching around the host boundary.

## The sidebar or review drawer covers the conversation

At narrow widths the session rail and review drawer intentionally become overlays. Close the overlay with its control or Escape. The conversation and composer should remain usable at the supported minimum window.

## Report a reproducible issue

Open a [GitHub issue](https://github.com/OpenAgentsInc/openagents/issues) with the application version, operating system, visible state, and reproduction steps. Remove credentials, absolute private paths, repository secrets, and private transcript content before attaching evidence.
