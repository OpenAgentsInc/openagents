# @openagentsinc/autopilot-control-protocol

The retained typed control and bridge protocol used by Pylon and compatible
OpenAgents services. It no longer owns a desktop/mobile application or native
build-and-release lane.

(Pylon is the internal node/runtime name; this package is the client-facing
protocol. The live wire schema tag remains `openagents.pylon.control.v0.3`.)

## Contents

- `control.ts` — Effect-Schema for the live control surface
  (`session.spawn/list/events/cancel`, session summaries, event frames,
  projection levels, health) + decoders.
- `bridge.ts` — the remote session bridge vocabulary (system #39): request
  verbs, event names, capability classes, request envelope, typed result
  status, scoped pairing credential claims, and capability gating helpers
  (`verbAllowedByCapabilities`, `isReadOnlyCapabilitySet`).
- `cursor.ts` — transport-agnostic stream cursor: `acceptEvent` (dedup +
  out-of-order rejection) and `needsResnapshot` (lag detection).
- `decision.ts` — exactly-once decision state machine (`resolveDecision`,
  `applyExternalResolution`).
- `fixtures.ts` (`./fixtures`) — shared node fixtures for cross-client tests.

## Test

```sh
pnpm test
```

Roadmap: `docs/autopilot-coder/2026-06-13-autopilot-clients-roadmap.md` (CL-0).
