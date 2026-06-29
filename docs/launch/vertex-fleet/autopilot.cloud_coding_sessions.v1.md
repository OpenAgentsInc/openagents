# autopilot.cloud_coding_sessions.v1 — remote bridge subscribe leg

State: **red** (unchanged — no promise/registry edits in this change).

## Blocker advanced
`blocker.product_promises.pylon_remote_bridge_transport_missing` (partial).

## Gap found
The capability-scoped client bridge transport (`packages/autopilot-control-protocol/src/bridge-transport.ts`,
`BridgeTransport`) already wired `list`, `history`, `resolveDecision`, `cancel`,
`readArtifact`, `spawn`, `submitIntent`, `steerTurn`, `pause/resume`, and
`deployCloud` over `POST /bridge`. But the **cursor-resumable LIVE event poll**
leg was missing from the live transport object: the node accepts the
`session.subscribe` verb (`apps/pylon/src/node/control-server.ts:575`, returning
`control-sessions.events()` → `{ sessionRef, state, recentEvents }`), and the
envelope builder (`buildSubscribeEnvelope`) + a strict parser existed in
`bridge-subscribe-client.ts` — yet that module was neither exported from the
package `index.ts` nor wired into `BridgeTransport`. So a remote client
(Expo/web/desktop) had no transport-level way to stream a session's event tail
over the bridge. The node comment is explicit that this is the intended remote
path: "RN fetch can't consume the SSE stream cleanly... render a live
session-detail timeline by polling POST /command { session.events }."

## What I built
- `parseBridgeEventBatch(raw, sinceCursor)` in `bridge-subscribe-client.ts`: a
  pure, transport-agnostic parser for the node's `events()` projection that
  filters to rows newer than the cursor (dedup by `eventIndex`), sorts ascending,
  surfaces the timeline + receipt round-trip fields (`messageText`,
  `artifactRef`, `resultRef`), tolerates unknown phases/kinds (e.g. future
  `cloud.gce.*` lane events) by passing them through, and returns the resume
  cursor for the next poll (`-1` = nothing seen yet). New `BridgeEventBatch` /
  `BridgeSessionEventRow` types.
- `BridgeTransport.subscribe({ sessionRef, cursor? })` in `bridge-transport.ts`:
  sends `session.subscribe` with an observe-class capability and optional cursor,
  parses the response into a `BridgeEventBatch`, and throws on a non-ok response
  (e.g. the node's 403 when the credential lacks an observe capability). Adds no
  new authority — it reuses the stored-claims-enforced bridge credential.
- Exported `bridge-subscribe-client.js` from the package `index.ts` so clients
  can import the builder/parser/types.
- Tests: 5 new `parseBridgeEventBatch` cases + 1 `transport.subscribe` case
  (cursor passthrough, capability selection, 403 rejection). Full package suite:
  813 pass.

## Files
- `packages/autopilot-control-protocol/src/bridge-subscribe-client.ts`
- `packages/autopilot-control-protocol/src/bridge-subscribe-client.test.ts`
- `packages/autopilot-control-protocol/src/bridge-transport.ts`
- `packages/autopilot-control-protocol/src/bridge-transport.test.ts`
- `packages/autopilot-control-protocol/src/index.ts`

## Validation
- `bunx tsc -p tsconfig.json --noEmit` (workers/api): my change introduces **0**
  errors. There is **1 pre-existing, unrelated** error —
  `src/training-data-refinery.ts(18,3): error TS6133:
  'Cs336A4EvalDeltaMeasurementRef' is declared but its value is never read` —
  introduced by commit `859e65e05` (promise `training.data_refinery_corpus.v1`),
  in a file this change does not touch. Left as-is to keep the diff minimal and
  avoid cross-promise edits.
- `apps/openagents.com bun run check:deploy`: **passes** (exit 0).
- `git diff --check`: clean.

## What still remains for this blocker
- The remote bridge transport now has read (`list`/`history`/`readArtifact`),
  live subscribe, decision-resolve, and the steer write-verbs — but the
  **remote-reachability transport** itself (a phone reaching a Pylon node off the
  LAN: relay/tunnel addressing, #5000 system #39) is still the larger open piece.
  This change closes the client-side subscribe leg over an already-reachable
  `baseUrl`; it does not establish remote reachability.
- No Expo client consumes `subscribe` yet (Phase 2 read-only app, #5001).
- `cloud.gce.*` event-kind round-trip to the desktop timeline (#5005) and live
  GCE provisioning (`OA_CODEX_GCE_PROVISIONER=fake`) remain open and gate green.

Blocker stays listed (partially advanced, not cleared).
