# @openagentsinc/world-client

Effect client for the Cloudflare Verse World protocol.

This package is the only client seam desktop and web should import for Verse
world state. It speaks the Cloudflare Worker + Region Durable Object protocol
defined by `@openagentsinc/world-contract`. It has no backend-kind union and no
legacy generated binding import.

## API

- `connect(request)` opens a future world session through a typed
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
- `projectWorldMinimapReadout(input)` projects a `WorldReadModel` into
  minimap markers, compass coordinates, and a region/subzone readout. Pylons,
  run cores, assignment markers, and remote avatars all come from the same
  read model that the 3D scene consumes. Subzone labels keep a small hysteresis
  band to avoid boundary flicker.

`applyDeltaToReadModel` preserves absent-means-unchanged semantics, prunes
interest exits from every row map, applies settle patches without deleting
position rows, and records diagnostics as part of the model. `applyDeltaToState`
also keeps selected-target promotion state until the selected ref leaves
interest.

Tests use fake Effect transports so reconnect, stale cursor, command ack, delta,
and diagnostics behavior stays deterministic without Cloudflare credentials.

## Transport milestone signals (pipeline signals)

`createBrowserWorldTransport` accepts an optional `signalBus`
(`@openagentsinc/pipeline-signals`) publishing typed transport milestones:
`world.transport.socket_created` when the WebSocket is constructed and
`world.transport.command_pending` when a command has been sent and registered
for acknowledgement. Tests await these signals instead of sleeping or polling
socket internals. These are pipeline signals — deterministic
test/orchestration synchronization events — NOT the user-facing evidence
receipts carried in `WorldDelta.receipt`.
