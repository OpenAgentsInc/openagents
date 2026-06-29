# Local Apple FM Signed-Installer Smoke Gate

Date: 2026-06-29

Issue: #7022

Promise: `autopilot.local_apple_fm_tool_chat.v1`

Status: source-level gate added; owner-gated signed/notarized artifact and
from-install smoke still required.

## What This Adds

The repo now has a pure public-safe gate for the remaining green transition:

```sh
bun test apps/autopilot-desktop/tests/apple-fm-from-install-smoke.test.ts
```

The gate is
`apps/autopilot-desktop/src/shared/apple-fm-from-install-smoke.ts`. It keeps
the promise blocked unless the public-safe smoke facts prove all of:

- supported Apple Silicon host;
- signed and notarized installer;
- packaged Foundation Models helper verified inside the installed app bundle;
- helper launched from the install, reported ready, and was restarted by
  supervision;
- bounded local Apple FM chat/tool session completed;
- local mode did not fall back to hosted prompt handling;
- prompt/file/callback/token/path redaction passed;
- clean shutdown and restart were observed;
- a public-safe evidence ref exists.

## Current Promise State

This gate does not by itself clear either remaining blocker:

- `blocker.product_promises.local_apple_fm_signed_installer_recut_missing`
- `blocker.product_promises.local_apple_fm_helper_supervision_missing`

It makes the owner-gated evidence shape explicit so the eventual signed
installer smoke can move the promise green without broadening the claim.

## Required Owner Smoke

After cutting the signed/notarized macOS installer, run the existing local smoke
from a clean install on supported Apple Silicon:

```sh
bun run --cwd apps/autopilot-desktop smoke:apple-fm-local
```

Record only public-safe booleans/statuses from that run plus the release
artifact ref. Do not publish prompts, file contents, callback URLs, callback
tokens, bearer material, helper local paths, raw transcripts, or private machine
identifiers.

The claim remains limited to user-owned local Apple FM chat/tool use. It does
not imply compute resale, paid work, settlement, Codex parity, broad Apple
hardware support, or cloud fallback.
