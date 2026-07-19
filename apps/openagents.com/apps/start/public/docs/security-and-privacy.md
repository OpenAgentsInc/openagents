---
title: Security and privacy
description: The local authority, credential, and renderer boundaries of OpenAgents Desktop.
lastModified: 2026-07-19
sidebar:
  order: 5
---

## Local-first operation

The ordinary Desktop workroom remains useful without an OpenAgents account. Repository access and Codex execution are held by the desktop host, not by browser-style renderer code.

## Renderer boundary

The renderer receives schema-checked projections and typed intent keys. It does not receive raw bearer tokens or provider credentials. It also does not receive control tokens, process handles, or unrestricted filesystem access.

Electron runs with context isolation, no renderer Node integration, sandboxing, deny-by-default permissions, restricted navigation, and a closed preload bridge.

## Codex custody

The application uses its exact bundled Codex runtime rather than falling back to an arbitrary binary on `PATH`. Account details are bounded and obscured by default in settings. Sensitive values do not belong in logs, screenshots, public receipts, or docs.

## Repository data

Repository status and diffs are scoped to the active work context. Secret-shaped, oversized, binary, stale, or revoked material fails closed. Review does not grant mutation authority.

## Account linking

OpenAgents account linking is optional for the initial local workflow. When linked features are used, the host owns encrypted native-session custody. Credentials do not cross into the renderer or become part of the Codex transcript.
