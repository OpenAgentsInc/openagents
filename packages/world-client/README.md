# @openagentsinc/world-client

Effect client for the Cloudflare Verse World protocol.

This package is the only client seam desktop and web should import for Verse
world state. It speaks the Cloudflare Worker + Region Durable Object protocol
defined by `@openagentsinc/world-contract`; it has no backend-kind union and no
legacy generated binding import.

## API

- `connect(request)` opens the Cloudflare world session through a typed
  transport and applies any initial snapshot/diagnostic deltas.
- `subscribe(request)` requests a server-approved `WorldSubscriptionPlan` and
  carries the current cursor for resubscribe/reconnect paths.
- `callCommand(command)` sends a `WorldCommandEnvelope`, applies the returned
  delta, and exposes command receipt ack state for movement/input diagnostics.
- `applyDelta(delta)` updates the read-only `ClientWorld` mirror.
- `reconnect()` reconnects with the latest cursor and selected target refs.
- `disconnect()` closes the transport and marks the session disconnected.
- `diagnostics()` and `readModel()` expose public diagnostics and the
  WoC-style `WorldReadModel` consumed by render, HUD, minimap, and nameplates.

`applyDeltaToReadModel` preserves absent-means-unchanged semantics, prunes
interest exits from every row map, applies settle patches without deleting
position rows, and records diagnostics as part of the model. `applyDeltaToState`
also keeps selected-target promotion state until the selected ref leaves
interest.

Tests use fake Effect transports so reconnect, stale cursor, command ack, delta,
and diagnostics behavior stays deterministic without Cloudflare credentials.
