# Probe Local Sidecar Adoption

## Scope

This document records the first OpenAgents consumer shape above the shipped
Probe local runtime seam.

It is intentionally narrow:

- Autopilot keeps owning the coding shell
- Probe stays the runtime
- the first consumer path is local-first and same-machine

## Status

This is the shipped resolution of `openagents#4066`.

The umbrella issue was opened when the honest Phase 1 Probe seam was still
described as a local `probe-server` child process over stdio. That changed once
Probe shipped its local daemon attach path.

The current OpenAgents position is:

- prefer the local `probe-daemon` socket transport first
- auto-start that daemon when the socket is missing
- keep a direct stdio `probe-server` fallback for packaged-app and debug cases

That preserves the original boundary that mattered in `#4066`:

- Autopilot supervises a local Probe sidecar boundary instead of linking Probe
  runtime internals directly into app state
- the desktop still owns thread, pane, artifact, and operator projection
- the runtime contract stays local-first and typed

## Runtime Selection

The Probe lane is still explicitly gated.

- `OPENAGENTS_AUTOPILOT_RUNTIME=probe` selects the Probe-backed desktop lane
- any other value keeps the current Codex-backed lane

That keeps the adoption slice opt-in while the product mapping settles.

## Local Runtime Contract

Autopilot now talks to Probe over the shipped protocol seam instead of assuming
Probe runtime internals.

The desktop-side worker:

- prefers the local `probe-daemon` socket transport first
- auto-starts the daemon when the socket is missing
- falls back to a spawned stdio `probe-server` only when the daemon path cannot
  satisfy the first consumer flow
- keeps desktop-owned mention and image attachment forwarding explicit by
  rendering a manifest into the Probe prompt instead of pretending the Probe
  protocol already carries structured desktop attachments

The desktop binary now exposes the same hidden internal helper entrypoints that
`probe-client` expects:

- `__internal-probe-server`
- `__internal-probe-daemon`

That lets a packaged desktop app supervise the local Probe runtime without
requiring a separately installed CLI binary.

## Shutdown And Reconnect

The ownership split is explicit:

- daemon-backed connections do not kill the daemon when the desktop drops
- stdio fallback connections shut down the spawned child on drop
- if the Probe transport resets, the lane moves to a disconnected or error
  state and retries connection on the next command path

This keeps detached daemon sessions alive while still preventing orphaned
fallback child processes.

## Honest Failure States

The desktop lane now exposes failure classes through app state instead of
silently changing runtime behavior:

- Probe daemon missing or not listening
- daemon auto-start failure
- direct server spawn failure
- protocol initialize failure or version skew
- transport reset while attached

The current UI projection uses the existing Autopilot connection status plus
`last_error` and lane status text rather than inventing a second product shell.

## What This Does Not Claim Yet

This slice does not claim full Probe product parity.

It does not yet mean:

- full Probe-backed artifact generation parity with Codex
- hosted or cross-device Probe control
- multi-user shared-session behavior

It only establishes the correct local consumer seam for the next desktop
mapping work.
